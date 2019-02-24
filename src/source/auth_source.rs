use crossbeam::atomic::ArcCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use futures::sync::oneshot;
use crossbeam::queue::SegQueue;

use futures::{Future, Async};
use tokio::executor::spawn;

use hyper::body::Body;
use hyper::Client;
use hyper::client::connect::Connect;

use crate::B2Error;
use crate::api::authorize::{authorize, B2Credentials, B2AuthFuture, B2Authorization};

/// Asynchronously handle reauthentication.
///
/// Unlike an [`UploadUrl`], any number of threads can simultaneously share a
/// [`B2Authorization`], so this type will only have a single active authorization at any
/// time. If an authorization is marked as expired, only a single authenticate request
/// will be sent, regardless of the number of threads asking for the authorization.
///
/// This type can be cloned in order to share it between threads. All clones share the
/// same internal authorization store.
///
/// [`UploadUrl`]: ../api/files/upload/struct.UploadUrl.html
/// [`B2Authorization`]: ../api/authorize/struct.B2Authorization.html
pub struct AuthSource<C> {
    inner: Arc<AuthInner<C>>,
}

// With derive it adds a C: Clone where bound.
impl<C> Clone for AuthSource<C> {
    fn clone(&self) -> Self {
        AuthSource {
            inner: self.inner.clone(),
        }
    }
}

impl<C> AuthSource<C>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    pub fn new(credentials: B2Credentials, client: Client<C, Body>) -> Self {
        AuthSource {
            inner: Arc::new(AuthInner::new(credentials, client)),
        }
    }
    /// Returns a future that resolves to a `B2Authorization`.
    ///
    /// If there is an active authorization, this future completes immediately.
    ///
    /// If no active authorization is found, the future resolves when the currently
    /// running re-auth task completes. (if no re-auth task is running, one is started)
    ///
    /// [`reauthenticate`]: #method.reauthenticate
    pub fn authentication(&self) -> AuthSourceFuture<C> {
        AuthSourceFuture {
            state: AuthSourceFutureState::NotRequested(self.inner.clone())
        }
    }
    /// Tell the `AuthSource` that this authorization has expired.
    ///
    /// This only removes the active authorization, and does not start a re-auth task.
    pub fn reauthenticate(&self, auth: &B2Authorization) {
        match *self.inner.state.get() {
            AuthState::ActiveAuth(ref active_auth) => {
                if auth != active_auth {
                    return;
                }
            },
            AuthState::NoActiveAuth() => {},
        }
        AuthInner::start_reauth(&self.inner);
    }
    /// Returns the currently active authorization, if there is one.
    pub fn try_get_active_authentication(&self) -> Option<B2Authorization> {
        match *self.inner.state.get() {
            AuthState::ActiveAuth(ref auth) => Some(auth.clone()),
            AuthState::NoActiveAuth() => None,
        }
    }
    /// Checks whether we currently have an active authorization.
    pub fn has_active_auth(&self) -> bool {
        match *self.inner.state.get() {
            AuthState::ActiveAuth(_) => true,
            AuthState::NoActiveAuth() => false,
        }
    }
    /// Set the current authorization in this `AuthSource`.
    ///
    /// If there is a pending reauth task when this is called, the provided authentication
    /// will be replaced by the result of the reauthentication once it completes.
    pub fn provide_auth(&self, auth: B2Authorization) {
        self.inner.set_state(Ok(auth));
    }
}
/// A future that resolves to an authentication.
///
/// Created from an [`AuthSource`].
///
/// [`AuthSource`]: struct.AuthSource.html
pub struct AuthSourceFuture<C> {
    state: AuthSourceFutureState<C>,
}
enum AuthSourceFutureState<C> {
    NotRequested(Arc<AuthInner<C>>),
    Waiting(oneshot::Receiver<Result<B2Authorization, Arc<B2Error>>>),
    Done(),
}

// The state is only accessed through the poll method which takes self by &mut.
unsafe impl<C> Send for AuthSourceFutureState<C> {}
unsafe impl<C> Sync for AuthSourceFutureState<C> {}

impl<C> Future for AuthSourceFuture<C>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    type Item = B2Authorization;
    type Error = B2Error;
    fn poll(&mut self) -> Result<Async<B2Authorization>, B2Error> {
        let state = std::mem::replace(&mut self.state, AuthSourceFutureState::Done());
        let (res, state) = match state {
            AuthSourceFutureState::NotRequested(inner) => {
                match AuthInner::try_get_auth(&inner) {
                    Some(auth) => (Ok(Async::Ready(auth)), AuthSourceFutureState::Done()),
                    None => {
                        let future = AuthInner::reauth(&inner);
                        self.state = AuthSourceFutureState::Waiting(future);
                        return self.poll();
                    },
                }
            },
            AuthSourceFutureState::Waiting(mut future) => match future.poll() {
                Ok(Async::Ready(res)) => match res {
                    Ok(auth) => (Ok(Async::Ready(auth)), AuthSourceFutureState::Done()),
                    Err(err) => (Err(err.into()), AuthSourceFutureState::Done()),
                },
                Ok(Async::NotReady) => (Ok(Async::NotReady),
                    AuthSourceFutureState::Waiting(future)),
                Err(_err) => {
                    // For this to happen, the notify queue in AuthInner must have been
                    // dropped before it was emptied.
                    //
                    // However: If we are in this state, an AuthTask has been spawned on
                    // the executor, and the AuthTask has an Arc<AuthInner>, so for the
                    // queue to be dropped, the AuthTask must have been dropped before it
                    // finished.
                    //
                    // However! In the drop impl for AuthTask, the queue is emptied if
                    // it is dropped before it is finished.
                    //
                    // Note that if the AuthState is forgotten with mem::forget, the Arc
                    // isn't dropped, and the queue is leaked, and therefore this wont
                    // happen in that case either.
                    unreachable!()
                },
            },
            AuthSourceFutureState::Done() => panic!("Poll called on finished future."),
        };
        self.state = state;
        res
    }
}

struct AuthInner<C> {
    credentials: B2Credentials,
    client: Client<C, Body>,
    notify: SegQueue<oneshot::Sender<Result<B2Authorization, Arc<B2Error>>>>,
    reauth_lock: AtomicBool,
    state: ArcCell<AuthState>,
}
impl<C> AuthInner<C>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    fn new(credentials: B2Credentials, client: Client<C>) -> Self {
        AuthInner {
            credentials,
            client,
            notify: SegQueue::new(),
            reauth_lock: AtomicBool::new(false),
            state: ArcCell::new(Arc::new(AuthState::NoActiveAuth())),
        }
    }
    fn try_get_auth(arc: &Arc<Self>) -> Option<B2Authorization> {
        match *arc.state.get() {
            AuthState::ActiveAuth(ref auth) => Some(auth.clone()),
            AuthState::NoActiveAuth() => None,
        }
    }
    fn start_reauth(arc: &Arc<Self>) {
        if !arc.reauth_lock.compare_and_swap(false, true, Ordering::Acquire) {
            // we got the lock. Let's reauth
            arc.state.set(Arc::new(AuthState::NoActiveAuth()));
            let auth_future = authorize(&arc.credentials, &arc.client);
            spawn(AuthTask(auth_future, arc.clone()));
        }
    }
    fn reauth(arc: &Arc<Self>)
        -> oneshot::Receiver<Result<B2Authorization, Arc<B2Error>>>
    {
        AuthInner::start_reauth(arc);
        let (notify, on_notify) = oneshot::channel();
        arc.notify.push(notify);
        on_notify
    }
}
impl<C> AuthInner<C> {
    fn set_state(&self, state: Result<B2Authorization, Arc<B2Error>>) {
        let new_state = Arc::new(match state {
            Ok(auth) => AuthState::ActiveAuth(auth),
            Err(_err) => AuthState::NoActiveAuth(),
        });
        self.state.set(new_state);
    }
}

enum AuthState {
    ActiveAuth(B2Authorization),
    NoActiveAuth(),
}

struct AuthTask<C>(B2AuthFuture, Arc<AuthInner<C>>);
impl<C> AuthTask<C> {
    fn finish(&self, res: Result<B2Authorization, Arc<B2Error>>) -> Result<Async<()>, ()> {
        self.1.set_state(res.clone());
        self.1.reauth_lock.store(false, Ordering::Release);
        while let Some(to_notify) = self.1.notify.try_pop() {
            // ignore closed receivers
            // it just means they've been dropped
            let _ = to_notify.send(res.clone());
        }
        Ok(Async::Ready(()))
    }
}
impl<C> Drop for AuthTask<C> {
    fn drop(&mut self) {
        if !self.0.is_done() {
            let msg = "Executor dropped AuthTask before it was completed.";
            let err = std::io::Error::new(std::io::ErrorKind::Other, msg);
            let _ = self.finish(Err(Arc::new(err.into())));
        }
    }
}
impl<C> Future for AuthTask<C> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        match self.0.poll() {
            Ok(Async::Ready(auth)) => self.finish(Ok(auth)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => self.finish(Err(Arc::new(err))),
        }
    }
}
