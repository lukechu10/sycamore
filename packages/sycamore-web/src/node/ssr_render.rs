use super::*;

/// The mode in which SSR is being run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsrMode {
    /// Synchronous mode.
    ///
    /// When a suspense boundary is hit, only the fallback is rendered.
    Sync,
    /// Blocking mode.
    ///
    /// When a suspense boundary is hit, rendering is paused until the suspense is resolved.
    Blocking,
    /// Streaming mode.
    ///
    /// When a suspense boundary is hit, the fallback is rendered. Once the suspense is resolved,
    /// the rendered HTML is streamed to the client.
    Streaming,
}

/// Render a [`View`] into a static [`String`]. Useful for rendering to a string on the server side.
#[must_use]
pub fn render_to_string(view: impl Into<LazyChildren>) -> String {
    is_not_ssr! {
        let _ = view;
        panic!("`render_to_string` only available in SSR mode");
    }
    is_ssr! {
        use std::cell::LazyCell;

        thread_local! {
            /// Use a static variable here so that we can reuse the same root for multiple calls to
            /// this function.
            static SSR_ROOT: LazyCell<RootHandle> = LazyCell::new(|| create_root(|| {}));
        }
        SSR_ROOT.with(|root| {
            root.dispose();
            root.run_in(|| {
                render_to_string_in_scope(view)
            })
        })
    }
}

/// Render a [`View`] into a static [`String`] in the current reactive scope.
///
/// Implementation detail of [`render_to_string`].
#[must_use]
pub fn render_to_string_in_scope(view: impl Into<LazyChildren>) -> String {
    is_not_ssr! {
        let _ = view;
        panic!("`render_to_string_in_scope` only available in SSR mode");
    }
    is_ssr! {
        let mut buf = String::new();

        let handle = create_child_scope(|| {
            let construction_mode = ConstructionMode::Hydrate(HydrationSession::new_ssr());
            provide_context(construction_mode);
            provide_context(SsrMode::Sync);

            let view = view.into().call();
            ssr_node::render_recursive_view(&view, &mut buf);
        });
        handle.dispose();
        buf
    }
}

/// Renders a [`View`] into a static [`String`] while awaiting for all suspense boundaries to
/// resolve. Useful for rendering to a string on the server side.
///
/// This sets the SSR mode to "blocking" mode. This means that rendering will wait until suspense
/// is resolved before returning.
///
/// # Example
/// ```
/// # use sycamore::prelude::*;
/// # use sycamore::web::render_to_string_await_suspense;
/// #[component]
/// async fn AsyncComponent() -> View {
///     // Do some async operations.   
///     # view! {}
/// }
///
/// # tokio_test::block_on(async move {
/// let ssr = render_to_string_await_suspense(AsyncComponent).await;
/// # })
/// ```
#[must_use]
#[cfg(feature = "suspense")]
pub async fn render_to_string_await_suspense(f: impl Into<LazyChildren>) -> String {
    is_not_ssr! {
        let _ = f;
        panic!("`render_to_string` only available in SSR mode");
    }
    is_ssr! {
        use std::cell::LazyCell;
        use futures::channel::oneshot;
        use sycamore_futures::{
            create_suspense_scope, provide_executor_scope, provide_suspense_tracker,
            use_is_loading_global,
        };

        thread_local! {
            static SSR_ROOT: LazyCell<RootHandle> = LazyCell::new(|| create_root(|| {}));
        }

        provide_executor_scope(async {
            let (tx, rx) = oneshot::channel();
            let mut view = View::new();
            let scope = SSR_ROOT.with(|root| root.run_in(|| {
                create_child_scope(|| {
                    provide_context(ConstructionMode::Hydrate(HydrationSession::new_ssr()));
                    provide_context(SsrMode::Blocking);
                    provide_context(SuspenseCounter::new());
                    provide_suspense_tracker();

                    // Also track async components outside an explicit Suspense boundary.
                    view = create_suspense_scope(|| f.into().call()).0;
                    let mut tx = Some(tx);
                    create_effect(move || {
                        if !use_is_loading_global()
                            && let Some(tx) = tx.take()
                        {
                            let _ = tx.send(());
                        }
                    });
                })
            }));
            let _cleanup = DisposeRenderScope(scope);
            rx.await.unwrap();

            // Dynamic parts still reference signals owned by this request.
            scope.run_in(|| {
                let mut buf = String::new();
                ssr_node::render_recursive_view(&view, &mut buf);
                buf
            })
        }).await
    }
}

/// Renders a [`View`] to a stream.
///
/// This sets the SSR mode to "streaming" mode. This means that the initial HTML with fallbacks is
/// sent first and then the suspense fragments are streamed as they are resolved.
///
/// The streamed suspense fragments are in the form of HTML template elements and a small script
/// that moves the template elements into the right area of the DOM.
///
/// # Executor
///
/// This function (unlike [`render_to_string_await_suspense`]) does not automatically create an
/// executor. You must provide the executor yourself by using `tokio::task::LocalSet`.
///
/// # Example
/// ```
/// # use sycamore::prelude::*;
/// # use sycamore::web::{render_to_string_stream, Suspense};
/// # use futures::StreamExt;
/// #[component]
/// async fn AsyncComponent() -> View {
///     // Do some async operations.   
///     # view! {}
/// }
///
/// #[component]
/// fn App() -> View {
///     view! {
///         Suspense(fallback=|| "Loading...") {
///             (AsyncComponent)
///         }
///     }
/// }
///
/// # tokio_test::block_on(async move {
/// // Create a channel for sending the created stream from the local set.
/// let (tx, rx) = tokio::sync::oneshot::channel();
/// tokio::task::spawn_blocking(|| {
///     let handle = tokio::runtime::Handle::current();
///     handle.block_on(async move {
///         let local = tokio::task::LocalSet::new();
///         local.run_until(async move {
///             let stream = render_to_string_stream(App);
///             tx.send(stream).ok().unwrap();
///         }).await;
///         // Run the remaining tasks in the local set.
///         local.await;
///     });
/// });
/// let stream = rx.await.unwrap();
/// tokio::pin!(stream);
/// while let Some(string) = stream.next().await {
///     // Send the string to the client.
///     // Usually, the web framework already supports converting a stream into a response.
/// }
/// # })
/// ```
#[cfg(feature = "suspense")]
pub fn render_to_string_stream(
    view: impl Into<LazyChildren>,
) -> impl futures::Stream<Item = String> + Send {
    is_not_ssr! {
        let _ = view;
        panic!("`render_to_string` only available in SSR mode");
        #[allow(unreachable_code)] // TODO: never type cannot be coerced into `impl Stream` somehow.
        futures::stream::empty()
    }
    is_ssr! {
        use std::cell::{LazyCell, RefCell};
        use std::rc::Rc;

        use futures::StreamExt;
        use futures::stream::FuturesUnordered;

        thread_local! {
            static SSR_ROOT: LazyCell<RootHandle> = LazyCell::new(|| create_root(|| {}));
        }
        let mut buf = String::new();
        let futures = Rc::new(RefCell::new(FuturesUnordered::new()));
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let (response, disconnected) = futures::channel::oneshot::channel::<()>();

        SSR_ROOT.with(|root| root.run_in(|| {
            let scope = create_child_scope(|| {
                provide_context(ConstructionMode::Hydrate(HydrationSession::new_ssr()));
                provide_context(SsrMode::Streaming);
                provide_context(SuspenseCounter::new());
                sycamore_futures::provide_suspense_tracker();
                provide_context(SuspenseStream { futures: futures.clone() });

                let view = view.into().call();
                ssr_node::render_recursive_view(&view, &mut buf);
            });

            scope.run_in(|| {
                let cleanup = DisposeRenderScope(scope);
                sycamore_futures::spawn_local_scoped(async move {
                    let _cleanup = cleanup;
                    let pump = async move {
                        let mut pending = futures.take();
                        while let Some(fragment) = pending.next().await {
                            // Serialize on the render thread while the scope is still alive.
                            if tx.unbounded_send(render_suspense_fragment(fragment)).is_err() {
                                return;
                            }
                            pending.extend(futures.take());
                        }
                    };
                    // Dropping the response wakes this task even if no fragment ever resolves.
                    // The scope guard must remain on the render thread.
                    let _ = futures::future::select(disconnected, std::pin::pin!(pump)).await;
                });
            });
        }));

        // Keep the outer comment anchors and replace only their interior. Queue a
        // fragment if its parent has not inserted the corresponding markers yet.
        static SUSPENSE_REPLACE_SCRIPT: &str = include_str!("suspense_stream.js");
        async_stream::stream! {
            let _response = response;
            let mut initial = String::new();
            initial.push_str("<!doctype html>");
            initial.push_str(&buf);
            initial.push_str("<script>");
            initial.push_str(SUSPENSE_REPLACE_SCRIPT);
            initial.push_str("</script>");
            yield initial;

            while let Some(html) = rx.next().await {
                yield html;
            }
            yield "<script>__sycamore_finish_stream()</script>".into();
        }
    }
}

#[cfg_ssr]
#[cfg(feature = "suspense")]
struct DisposeRenderScope(NodeHandle);

#[cfg_ssr]
#[cfg(feature = "suspense")]
impl Drop for DisposeRenderScope {
    fn drop(&mut self) {
        self.0.run_in(|| self.0.dispose());
    }
}

#[cfg_ssr]
#[cfg(feature = "suspense")]
fn render_suspense_fragment(SuspenseFragment { key, view }: SuspenseFragment) -> String {
    use std::fmt::Write;

    let mut buf = String::new();
    write!(&mut buf, "<template id=\"sycamore-suspense-{key}\">",).unwrap();
    ssr_node::render_recursive_view(&view, &mut buf);
    write!(
        &mut buf,
        "</template><script>__sycamore_suspense({key})</script>"
    )
    .unwrap();

    buf
}
