//! Async resources integrated with suspense.

use std::future::Future;
use std::ops::Deref;

use futures::future::{FutureExt, LocalBoxFuture};
use sycamore_futures::{SuspenseScope, SuspenseTaskGuard};

use crate::*;

/// Represents a asynchronous resource.
#[derive(Clone, Copy)]
pub struct Resource<T: 'static> {
    /// The current value of the resource.
    ///
    /// This will initially be `None` while the resource is first fetched. For subsequent fetches,
    /// the resource will still contain the previous value while the new value is fetched.
    value: Signal<Option<T>>,
    /// Whether the resource is currently loading or not.
    is_loading: Signal<bool>,
    /// The function that fetches the resource.
    #[allow(clippy::complexity)]
    refetch: Signal<Box<dyn FnMut() -> LocalBoxFuture<'static, T>>>,
    /// A list of all the suspense scopes in which the resource is accessed.
    scopes: Signal<Vec<SuspenseScope>>,
    /// A list of suspense guards that are currently active.
    guards: Signal<Vec<SuspenseTaskGuard>>,
}

impl<T: 'static> Resource<T> {
    /// Create a new resource. By itself, this doesn't do anything.
    fn new<F, Fut>(mut refetch: F) -> Self
    where
        F: FnMut() -> Fut + 'static,
        Fut: Future<Output = T> + 'static,
    {
        Self {
            value: create_signal(None),
            is_loading: create_signal(true),
            refetch: create_signal(Box::new(move || refetch().boxed_local())),
            scopes: create_signal(Vec::new()),
            guards: create_signal(Vec::new()),
        }
    }

    /// Attach handlers to always call the refetch function to get the latest value.
    fn always_refetch(self) -> Self {
        create_effect(move || {
            self.is_loading.set(true);
            // Take all the scopes and create a new guard.
            for scope in self.scopes.take() {
                let guard = SuspenseTaskGuard::from_scope(scope);
                self.guards.update(|guards| guards.push(guard));
            }

            let fut = self.refetch.update_silent(|f| f());

            sycamore_futures::create_suspense_task(async move {
                let value = fut.await;
                batch(move || {
                    self.value.set(Some(value));
                    self.is_loading.set(false);
                    // Now, drop all the guards to resolve suspense.
                    self.guards.update(|guards| guards.clear());
                });
            });
        });

        self
    }

    /// Returns whether we are currently loading a new value or not.
    pub fn is_loading(&self) -> bool {
        self.is_loading.get()
    }
}

/// Hijack deref so that we can track where the resource is being accessed.
impl<T: 'static> Deref for Resource<T> {
    type Target = ReadSignal<Option<T>>;

    fn deref(&self) -> &Self::Target {
        // If we are already loading, add a new suspense guard. Otherwise, register the scope so
        // that we can create a new guard when loading.
        if self.is_loading.get() {
            let guard = SuspenseTaskGuard::new();
            self.guards.update(|guards| guards.push(guard));
        } else if let Some(scope) = try_use_context::<SuspenseScope>() {
            self.scopes.update(|scopes| scopes.push(scope));
        }

        &self.value
    }
}

/// Create a resource value that is fetched on both client and server.
///
/// If the resource has any dependencies, it is recommended to use [`on`] to make them explicit.
/// This will ensure that the dependencies are tracked since reactive variables inside async
/// contexts are not tracked automatically.
pub fn create_isomorphic_resource<F, Fut, T>(f: F) -> Resource<T>
where
    F: FnMut() -> Fut + 'static,
    Fut: Future<Output = T> + 'static,
    T: 'static,
{
    Resource::new(f).always_refetch()
}

/// Create a resource value that is fetched only on the client.
///
/// On the server, the resource will forever be in the loading state.
///
/// If the resource has any dependencies, it is recommended to use [`on`] to make them explicit.
/// This will ensure that the dependencies are tracked since reactive variables inside async
/// contexts are not tracked automatically.
pub fn create_client_resource<F, Fut, T>(f: F) -> Resource<T>
where
    F: FnMut() -> Fut + 'static,
    Fut: Future<Output = T> + 'static,
    T: 'static,
{
    let resource = Resource::new(f);
    if is_not_ssr!() {
        resource.always_refetch()
    } else {
        resource
    }
}

#[cfg(test)]
mod tests {
    use futures::channel::oneshot;
    use sycamore_futures::provide_executor_scope;

    use super::*;

    #[tokio::test]
    async fn create_isomorphic_resource_works() {
        provide_executor_scope(async {
            let (tx, rx) = oneshot::channel();
            let mut tx = Some(tx);
            let mut resource = None;
            let mut value = None;

            let root = create_root(|| {
                value = Some(create_signal(123));
                resource = Some(create_isomorphic_resource(on(
                    value.unwrap(),
                    move || async move { value.unwrap().get() },
                )));
                // Resource should be `None` initially.
                assert_eq!(resource.unwrap().get(), None);
                assert!(resource.unwrap().is_loading());

                // Signal when the resource is loaded.
                create_effect(move || {
                    if !resource.unwrap().is_loading()
                        && let Some(tx) = tx.take()
                    {
                        tx.send(()).unwrap();
                    }
                })
            });

            rx.await.unwrap();
            root.run_in(move || {
                // Now resource should have the value.
                assert_eq!(resource.unwrap().get(), Some(123));
                assert!(!resource.unwrap().is_loading());

                // Now trigger a refetch.
                value.unwrap().set(456);
                assert_eq!(
                    resource.unwrap().get(),
                    Some(123),
                    "resource should keep old value until new value is loaded"
                );
                assert!(resource.unwrap().is_loading());
            });
        })
        .await;
    }
}
