//! Futures support for reactive scopes.

#![deny(missing_debug_implementations)]
#![warn(missing_docs)]

mod suspense;

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Future;
use futures::future::abortable;
use futures::stream::Abortable;
use pin_project::pin_project;
use sycamore_reactive::{NodeHandle, on_cleanup, use_current_scope};

pub use self::suspense::*;

/// If running on `wasm32` target, does nothing. Otherwise creates a new `tokio::task::LocalSet`
/// scope.
pub async fn provide_executor_scope<U>(fut: impl Future<Output = U>) -> U {
    #[cfg(all(target_arch = "wasm32", not(sycamore_force_ssr)))]
    {
        fut.await
    }
    #[cfg(any(not(target_arch = "wasm32"), sycamore_force_ssr))]
    {
        let local = tokio::task::LocalSet::new();
        local.run_until(fut).await
    }
}

/// Spawns a `!Send` future.
///
/// This will not auto cancel the task if the scope in which it is created is destroyed.
/// For this purpose, use [`spawn_local_scoped`] instead.
pub fn spawn_local(fut: impl Future<Output = ()> + 'static) {
    #[cfg(any(not(target_arch = "wasm32"), sycamore_force_ssr))]
    tokio::task::spawn_local(fut);
    #[cfg(all(target_arch = "wasm32", not(sycamore_force_ssr)))]
    wasm_bindgen_futures::spawn_local(fut);
}

/// Spawns a `!Send` future on the current scope.
///
/// If the scope is destroyed before the future is completed, it is aborted immediately. This
/// ensures that it is impossible to access any values referencing the scope after they are
/// destroyed.
#[cfg_attr(debug_assertions, track_caller)]
pub fn spawn_local_scoped(fut: impl Future<Output = ()> + 'static) {
    let scoped = ScopedFuture::new_in_current_scope(fut);
    spawn_local(scoped);
}

/// A wrapper that runs the future on the current scope.
#[pin_project]
struct ScopedFuture<T> {
    #[pin]
    task: Abortable<T>,
    scope: NodeHandle,
}

impl<T: Future> Future for ScopedFuture<T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        this.scope.run_in(move || this.task.poll(cx).map(|_| ()))
    }
}

impl<T: Future> ScopedFuture<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn new_in_current_scope(f: T) -> Self {
        let (abortable, handle) = abortable(f);
        on_cleanup(move || handle.abort());

        let scope = use_current_scope();

        Self {
            task: abortable,
            scope,
        }
    }
}
