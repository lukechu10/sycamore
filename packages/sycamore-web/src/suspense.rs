//! Components for suspense.

use std::future::Future;
use std::num::NonZeroU32;

use sycamore_core::Callback;
use sycamore_futures::{
    create_detached_suspense_scope, create_suspense_scope, create_suspense_task,
};

use crate::*;

/// `Suspense` lets you wait for `async` tasks to complete before rendering the UI. This is useful
/// for asynchronous data-fetching or other asynchronous tasks.
///
/// `Suspense` is deeply integrated with async components. Async components that are nested under
/// the `Suspense` component will not be rendered until they are resolved. Having multiple async
/// components will have the effect that the final UI will only be rendered once all individual
/// async components are rendered. This is useful for showing a loading indicator while the data is
/// being loaded.
///
/// # Example
/// ```
/// use sycamore::prelude::*;
/// use sycamore::web::Suspense;
///
/// #[component]
/// async fn AsyncComp() -> View {
///     view! { "Hello Suspense!" }
/// }
///
/// #[component]
/// fn App() -> View {
///     view! {
///         Suspense(fallback=|| view! { "Loading..." }) {
///             (AsyncComp)
///         }
///     }
/// }
/// ```
#[component]
pub fn Suspense(
    /// The fallback [`View`] to display while the child nodes are being awaited.
    fallback: LazyChildren,
    children: LazyChildren,
    /// The component will automatically update this signal with the `is_loading` state.
    ///
    /// This is only updated in non-SSR mode.
    mut set_is_loading: Callback<bool>,
) -> View {
    is_ssr! {
        use futures::FutureExt;

        let _ = &mut set_is_loading;
        let mode = try_use_context::<SsrMode>().unwrap_or(SsrMode::Sync);
        match mode {
            SsrMode::Sync => {
                anchored_branch(fallback.call())
            }
            SsrMode::Blocking => {
                let (content, _) = create_suspense_scope(move || children.call());
                anchored_branch(content)
            }
            SsrMode::Streaming => {
                let key = use_suspense_key();
                let (content, scope) = create_suspense_scope(move || children.call());
                let owner = use_current_scope();
                let stream = use_context::<SuspenseStream>();
                stream.futures.borrow_mut().push(async move {
                    // This future is polled by the shared streaming pump. Keep any
                    // reactive work in the boundary's scope, not the pump's scope.
                    let ready = owner.run_in(|| {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        let mut tx = Some(tx);
                        let loading = scope.is_loading();
                        create_effect(move || {
                            if !loading.get() && let Some(tx) = tx.take() {
                                let _ = tx.send(());
                            }
                        });
                        rx
                    });
                    ready.await.unwrap();
                    SuspenseFragment::new(key, anchored(content))
                }.boxed_local());

                let fallback = provide_context_in_new_scope(ConstructionMode::Create, || {
                    provide_context(SsrMode::Sync);
                    view! {
                        suspense-start(data-key=key.to_string())
                        (fallback)
                        suspense-end(data-key=key.to_string())
                    }
                });
                anchored(fallback)
            }
        }
    }
    is_not_ssr! {
        let hydrating = ConstructionMode::current().is_hydrating();
        let initial_fallback = hydrating.then(|| {
            try_use_context::<SsrMode>().unwrap_or(SsrMode::Sync) == SsrMode::Sync
        });

        let mut content = None;
        let owner = create_child_scope(|| {
            let (view, scope) = if initial_fallback == Some(true) {
                // Sync SSR contains only the fallback. The children have no nodes to claim.
                provide_context_in_new_scope(ConstructionMode::Create, || {
                    create_suspense_scope(move || children.call())
                })
            } else {
                create_suspense_scope(move || children.call())
            };
            content = Some((view, scope.is_loading()));
        });
        let (content, loading) = content.unwrap();
        owner.run_in(|| create_effect(move || set_is_loading(loading.get())));

        let fallback = if initial_fallback == Some(true) {
            Branch::Ready(owner.run_in(|| fallback.call()))
        } else {
            Branch::Lazy(fallback)
        };
        View::new_from_parts(smallvec::smallvec![ViewPart::Suspense(Rc::new(SuspenseView {
            owner,
            loading,
            initial_fallback,
            fallback: std::cell::RefCell::new(Some(fallback)),
            content: std::cell::RefCell::new(Some(Branch::Ready(content))),
        }))])
    }
}

is_ssr! {
    fn anchored(view: View) -> View {
        View::from((HtmlNode::create_start_marker(), view, HtmlNode::create_end_marker()))
    }

    fn anchored_branch(view: View) -> View {
        anchored(anchored(view))
    }
}

is_not_ssr! {
    pub(crate) enum Branch {
        Ready(View),
        Lazy(LazyChildren),
    }

    pub(crate) struct SuspenseView {
        pub owner: NodeHandle,
        pub loading: Signal<bool>,
        /// The branch that SSR actually rendered, independent of client loading state.
        pub initial_fallback: Option<bool>,
        pub fallback: std::cell::RefCell<Option<Branch>>,
        pub content: std::cell::RefCell<Option<Branch>>,
    }

    impl SuspenseView {
        pub(crate) fn branch(&self, fallback: bool) -> &std::cell::RefCell<Option<Branch>> {
            if fallback { &self.fallback } else { &self.content }
        }
    }
}

/// `Transition` is like [`Suspense`] except that it keeps the previous content visible until the
/// new content is ready.
#[component]
pub fn Transition(
    /// The fallback [`View`] to display while the child nodes are being awaited.
    fallback: LazyChildren,
    children: LazyChildren,
    /// The component will automatically update this signal with the `is_loading` state.
    ///
    /// This is only updated in non-SSR mode.
    set_is_loading: Callback<bool>,
) -> View {
    view! {
        Suspense(
            fallback=fallback,
            children=LazyChildren::new(move || {
                view! {
                    TransitionInner(children=children, set_is_loading=set_is_loading)
                }
            }),
        )
    }
}

/// Only trigger outer suspense on initial render. In subsequent renders, capture the suspense
/// scope.
#[component]
fn TransitionInner(children: LazyChildren, mut set_is_loading: Callback<bool>) -> View {
    // We create a detached suspense scope here to not create a deadlock with the outer
    // suspense.
    let (children, scope) = create_detached_suspense_scope(move || children.call());
    // Trigger the outer suspense scope. Note that this is only triggered on the initial render
    // and future renders will be captured by the inner suspense scope.
    create_suspense_task(scope.until_finished());

    let is_loading = scope.is_loading();
    create_effect(move || {
        set_is_loading(is_loading.get());
    });

    view! {
        (children)
    }
}

/// Convert an async component to a regular sync component. Also wraps the async component inside a
/// suspense scope so that content is properly suspended.
pub fn wrap_async_component<F: Future<Output = View>>(f: impl FnOnce() -> F + 'static) -> View {
    let mode = try_use_context::<SsrMode>().unwrap_or(SsrMode::Sync);
    if is_ssr!() && mode == SsrMode::Sync {
        // Synchronous SSR has no executor. Leave an empty range for the client.
        return View::from_dynamic(View::new);
    }
    #[cfg(any(feature = "hydrate", not(target_arch = "wasm32"), sycamore_force_ssr))]
    if ConstructionMode::current().is_hydrating() && mode != SsrMode::Sync {
        return deferred_async_view(f);
    }

    let pending = create_signal(View::new());
    create_suspense_task(async move {
        pending.set(f().await);
    });
    View::from_dynamic(move || {
        pending.track();
        pending.take_silent()
    })
}

/// Async SSR emits a namespace with its resolved range. The client receives that
/// namespace from the range itself, so sibling tasks may resolve in any order.
#[cfg(any(feature = "hydrate", not(target_arch = "wasm32"), sycamore_force_ssr))]
#[derive(Clone, Copy)]
pub(crate) struct DeferredView {
    pub content: Signal<Option<View>>,
    #[cfg(any(not(target_arch = "wasm32"), sycamore_force_ssr))]
    pub namespace: NonZeroU32,
    #[cfg(all(target_arch = "wasm32", not(sycamore_force_ssr)))]
    pub session: MutSignal<Option<HydrationSession>>,
}

#[cfg(any(feature = "hydrate", not(target_arch = "wasm32"), sycamore_force_ssr))]
fn deferred_async_view<F: Future<Output = View>>(f: impl FnOnce() -> F + 'static) -> View {
    let content = create_signal(None);
    let deferred = DeferredView {
        content: *content,
        #[cfg(any(not(target_arch = "wasm32"), sycamore_force_ssr))]
        namespace: use_suspense_key(),
        #[cfg(all(target_arch = "wasm32", not(sycamore_force_ssr)))]
        session: create_signal(None),
    };

    create_suspense_task(async move {
        let session = {
            is_ssr! { Some(HydrationSession::new_ssr_namespace(deferred.namespace)) }
            is_not_ssr! { deferred.session.get_untracked() }
        };
        let mut future = None;
        let scope = create_child_scope(|| {
            provide_context(session.map_or(ConstructionMode::Create, ConstructionMode::Hydrate));
            future = Some(Box::pin(f()));
        });
        // Own the pinned box in `poll_fn` instead of borrowing it across `.await`
        // to avoid the async aliasing issue in rust-lang/rust#63818.
        let mut future = future.unwrap();
        let view = std::future::poll_fn(move |cx| scope.run_in(|| future.as_mut().poll(cx))).await;
        content.set(Some(view));
    });

    View::new_from_parts(smallvec::smallvec![ViewPart::Deferred(deferred)])
}

/// Represents a streamed suspense view fragment.
#[cfg_ssr]
pub(crate) struct SuspenseFragment {
    pub key: NonZeroU32,
    pub view: View,
}

#[cfg_ssr]
impl SuspenseFragment {
    pub fn new(key: NonZeroU32, view: View) -> Self {
        Self { key, view }
    }
}

/// Context for passing streaming suspense fragments in SSR mode.
#[cfg_ssr]
#[derive(Clone)]
pub(crate) struct SuspenseStream {
    pub futures: std::rc::Rc<
        std::cell::RefCell<
            futures::stream::FuturesUnordered<
                futures::future::LocalBoxFuture<'static, SuspenseFragment>,
            >,
        >,
    >,
}

/// Global counter for providing suspense key.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SuspenseCounter {
    next: MutSignal<NonZeroU32>,
}

impl SuspenseCounter {
    pub(crate) fn new() -> Self {
        Self {
            next: create_signal(NonZeroU32::new(1).unwrap()),
        }
    }
}

/// Get the next suspense key.
pub fn use_suspense_key() -> NonZeroU32 {
    let counter = try_use_context::<SuspenseCounter>()
        .unwrap_or_else(|| use_global_scope().run_in(|| use_context_or_else(SuspenseCounter::new)));

    let next = counter.next.get_untracked();
    counter.next.set_silent(next.checked_add(1).unwrap());
    next
}
