use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use sycamore::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::Array;
use web_sys::{Element, Event, HtmlAnchorElement, HtmlBaseElement, UrlSearchParams};

use crate::Route;

/// A router integration provides the methods for adapting a router to a certain environment (e.g.
/// history API).
pub trait Integration {
    /// Get the current pathname.
    fn current_pathname(&self) -> String;

    /// Add a callback for listening to the `popstate` event.
    fn on_popstate(&self, f: Box<dyn FnMut()>);

    /// Get the handler for clicks that bubble to the document while the router is mounted.
    fn click_handler(&self) -> Box<dyn Fn(web_sys::MouseEvent)>;
}

thread_local! {
    static PATHNAME: Cell<Option<MutSignal<String>>> = const { Cell::new(None) };
    static QUERY: Cell<Option<MutSignal<()>>> = const { Cell::new(None) };
}

/// A router integration that uses the
/// [HTML5 History API](https://developer.mozilla.org/en-US/docs/Web/API/History_API) to keep the
/// UI in sync with the URL.
#[derive(Default, Debug)]
pub struct HistoryIntegration {
    /// This field is to prevent downstream users from creating a new `HistoryIntegration` without
    /// the `new` method.
    _internal: (),
}

impl HistoryIntegration {
    /// Create a new [`HistoryIntegration`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl Integration for HistoryIntegration {
    fn current_pathname(&self) -> String {
        window().location().pathname().unwrap_throw()
    }

    fn on_popstate(&self, f: Box<dyn FnMut()>) {
        let closure = Closure::wrap(f);
        window()
            .add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref())
            .unwrap_throw();
        closure.forget();
    }

    fn click_handler(&self) -> Box<dyn Fn(web_sys::MouseEvent)> {
        Box::new(|ev| {
            if ev.default_prevented()
                || ev.button() != 0
                || ev.meta_key()
                || ev.ctrl_key()
                || ev.shift_key()
                || ev.alt_key()
            {
                return;
            }

            let Some(target) = ev
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
            else {
                return;
            };
            if let Some(a) = target
                .closest("a[href]")
                .unwrap_throw()
                .and_then(|link| link.dyn_into::<HtmlAnchorElement>().ok())
            {
                let location = window().location();

                // Leave browser-managed navigation alone.
                let target = a.target();
                if (!target.is_empty() && !target.eq_ignore_ascii_case("_self"))
                    || a.has_attribute("download")
                    || a.rel()
                        .split_ascii_whitespace()
                        .any(|rel| rel.eq_ignore_ascii_case("external"))
                {
                    return;
                }

                let origin = a.origin();
                let a_pathname = a.pathname();
                let hash = a.hash();
                let query = a.search();

                if location.origin() == Ok(origin) {
                    if location.pathname().as_ref() != Ok(&a_pathname) {
                        // Same origin, different path. Navigate to new page.
                        ev.prevent_default();
                        PATHNAME.with(|pathname| {
                            // Update History API.
                            let history = window().history().unwrap_throw();
                            history
                                .push_state_with_url(&JsValue::UNDEFINED, "", Some(&a_pathname))
                                .unwrap_throw();
                            window().scroll_to_with_x_and_y(0.0, 0.0);

                            let pathname = pathname.get().unwrap_throw();
                            let path = a_pathname
                                .strip_prefix(&base_pathname())
                                .unwrap_or(&a_pathname);
                            pathname.set(path.to_string());
                        });
                    } else if location.search().as_ref() != Ok(&query) {
                        // Same origin, same pathname, different query.
                        ev.prevent_default();
                        let history = window().history().unwrap_throw();
                        if query.is_empty() {
                            history
                                .push_state_with_url(&JsValue::UNDEFINED, "", Some(&a.href()))
                                .unwrap_throw();
                        } else {
                            let history = window().history().unwrap_throw();
                            history
                                .push_state_with_url(&JsValue::UNDEFINED, "", Some(&query))
                                .unwrap_throw();
                        }
                        QUERY.with(|query| query.get().unwrap_throw().update(|_| {}));
                    } else if location.hash().as_ref() != Ok(&hash) {
                        // Same origin, same pathname, same query, different hash. Use default
                        // browser behavior.
                        if hash.is_empty() {
                            ev.prevent_default();
                            let history = window().history().unwrap_throw();
                            history
                                .push_state_with_url(&JsValue::UNDEFINED, "", Some(&a.href()))
                                .unwrap_throw();
                            window()
                                .dispatch_event(&Event::new("hashchange").unwrap())
                                .unwrap_throw();
                        }
                    } else {
                        // Same page. Do nothing.
                        ev.prevent_default();
                    }
                }
            }
        })
    }
}

/// Gets the base pathname from `document.baseURI`.
fn base_pathname() -> String {
    match document().query_selector("base[href]") {
        Ok(Some(base)) => {
            let base = base.unchecked_into::<HtmlBaseElement>().href();

            let url = web_sys::Url::new(&base).unwrap_throw();
            let mut pathname = url.pathname();
            // Strip trailing `/` character from the pathname.
            pathname.ends_with('/');
            pathname.pop(); // Pop the `/` character.
            pathname
        }
        _ => "".to_string(),
    }
}

/// The Sycamore router component. This component expects to be used inside a browser environment.
/// For server environments, see [`StaticRouter`].
///
/// Click handling is delegated to the document, so links added after the router mounts also work.
/// Use `rel="external"` on links that should bypass [`HistoryIntegration`].
#[component]
pub fn Router<R, F, I>(view: F, integration: I) -> View
where
    R: Route + 'static,
    F: FnOnce(Signal<R>) -> View + 'static,
    I: Integration + 'static,
{
    view! {
        RouterBase(
            view=view,
            integration=integration,
            // The derive macro makes this the `#[not_found]` route (always present)
            route=R::default(),
        )
    }
}

/// A lower-level router component that takes an instance of your [`Route`] type. This is designed
/// for `struct` [`Route`]s, which can be used to store additional information along with routes.
///
/// This is a very specific use-case, and you probably actually want [`Router`]!
#[component]
pub fn RouterBase<R, F, I>(view: F, integration: I, route: R) -> View
where
    R: Route + 'static,
    F: FnOnce(Signal<R>) -> View + 'static,
    I: Integration + 'static,
{
    let integration = Rc::new(integration);
    let base_pathname = base_pathname();

    PATHNAME.with(|pathname| {
        assert!(
            pathname.get().is_none(),
            "cannot have more than one Router component initialized"
        );
        // Get initial url from window.location.
        let path = integration.current_pathname();
        let path = path.strip_prefix(&base_pathname).unwrap_or(&path);
        pathname.set(Some(create_signal(path.to_string())));
        QUERY.set(Some(create_signal(())));
    });
    let pathname = PATHNAME.with(|p| p.get().unwrap_throw());

    // Set PATHNAME to None when the Router is destroyed.
    on_cleanup(|| PATHNAME.with(|pathname| pathname.set(None)));

    // Listen to popstate event.
    integration.on_popstate(Box::new({
        let integration = integration.clone();
        move || {
            let path = integration.current_pathname();
            let path = path.strip_prefix(&base_pathname).unwrap_or(&path);
            if pathname.with(|pathname| pathname != path) {
                pathname.set(path.to_string());
            }
        }
    }));
    let route_signal = create_memo(move || pathname.with(|pathname| route.match_path(pathname)));
    on_mount(move || {
        let document = document();
        let handler: Closure<dyn FnMut(web_sys::MouseEvent)> =
            Closure::new(integration.click_handler());
        let callback = handler.as_ref().unchecked_ref();
        document
            .add_event_listener_with_callback("click", callback)
            .unwrap_throw();
        on_cleanup(move || {
            document
                .remove_event_listener_with_callback("click", handler.as_ref().unchecked_ref())
                .unwrap_throw();
        });
    });
    view(*route_signal)
}

/// A router that only renders once with the given `route`.
///
/// This is useful for SSR where we want the HTML to be rendered instantly instead of waiting for
/// the route preload to finish loading.
#[component]
pub fn StaticRouter<R, F>(view: F, route: R) -> View
where
    R: Route + 'static,
    F: Fn(Signal<R>) -> View + 'static,
{
    view! {
        StaticRouterBase(view=view, route=route)
    }
}

/// Implementation detail for [`StaticRouter`]. The extra component is needed to make sure hydration
/// keys are consistent.
#[component]
fn StaticRouterBase<R, F>(view: F, route: R) -> View
where
    R: Route + 'static,
    F: Fn(Signal<R>) -> View + 'static,
{
    view(*create_signal(route))
}

/// Navigates to the specified `url`. The url should have the same origin as the app.
///
/// This is useful for imperatively navigating to an url when using an anchor tag (`<a>`) is not
/// possible/suitable (e.g. when submitting a form).
///
/// # Panics
/// This function will `panic!()` if a [`Router`] has not yet been created.
pub fn navigate(url: &str) {
    let history = window().history().unwrap_throw();
    history
        .push_state_with_url(&JsValue::UNDEFINED, "", Some(url))
        .unwrap_throw();
    navigate_no_history(url);
}

/// Navigates to the specified `url` without adding a new history entry. Instead, this replaces the
/// current location with the new `url`. The url should have the same origin as the app.
///
/// This is useful for imperatively navigating to an url when using an anchor tag (`<a>`) is not
/// possible/suitable (e.g. when submitting a form).
///
/// # Panics
/// This function will `panic!()` if a [`Router`] has not yet been created.
pub fn navigate_replace(url: &str) {
    let history = window().history().unwrap_throw();
    history
        .replace_state_with_url(&JsValue::UNDEFINED, "", Some(url))
        .unwrap_throw();
    navigate_no_history(url);
}

/// Navigates to the specified `url` without touching the history API.
///
/// This means that the url will not be updated and will continue to show the previous value.
///
/// # Panics
/// This function will `panic!()` if a [`Router`] has not yet been created.
pub fn navigate_no_history(url: &str) {
    window().scroll_to_with_x_and_y(0.0, 0.0);
    update_pathname(url);
}

/// Internal function for getting the global pathname variable and updating it with the given `url`.
///
/// # Panics
/// This function will `panic!()` if a [`Router`] has not yet been created.
fn update_pathname(url: &str) {
    PATHNAME.with(|pathname| {
        assert!(
            pathname.get().is_some(),
            "cannot navigate outside of a Router",
        );

        let pathname = pathname.get().unwrap_throw();
        let path = url.strip_prefix(&base_pathname()).unwrap_or(url);
        pathname.set(path.to_string());
    })
}

/// Preform a "soft" refresh of the current page.
///
/// Unlike a "hard" refresh which corresponds to clicking on the refresh button, this simply forces
/// a re-render of the view for the current page.
///
/// # Panic
/// This function will `panic!()` if a [`Router`] has not yet been created.
pub fn refresh() {
    PATHNAME.with(|pathname| {
        assert!(
            pathname.get().is_some(),
            "cannot refresh outside of a Router",
        );

        window().scroll_to_with_x_and_y(0.0, 0.0);

        pathname.get().unwrap_throw().update(|_| {});
    });
}

/// Creates a Signal that tracks the url query provided.
pub fn use_search_query(query: &'static str) -> Signal<Option<String>> {
    *PATHNAME.with(|pathname| {
        assert!(
            pathname.get().is_some(),
            "cannot get query outside of a Router",
        );

        let pathname = pathname.get().unwrap_throw();

        create_memo(move || {
            QUERY.with(|query| query.get().unwrap_throw()).track();
            pathname.track();
            UrlSearchParams::new_with_str(&window().location().search().unwrap_throw())
                .unwrap_throw()
                .get(query)
        })
    })
}

/// Creates a Signal that tracks the url query string.
pub fn use_search_queries() -> Signal<HashMap<String, String>> {
    *PATHNAME.with(|pathname| {
        assert!(
            pathname.get().is_some(),
            "cannot get query outside of a Router",
        );

        let pathname = pathname.get().unwrap_throw();

        create_memo(move || {
            QUERY.with(|query| query.get().unwrap_throw()).track();
            pathname.track();
            UrlSearchParams::new_with_str(&window().location().search().unwrap_throw())
                .unwrap_throw()
                .entries()
                .into_iter()
                .map(|e| {
                    let e: Array = e.unwrap_throw().into();
                    let e = e
                        .into_iter()
                        .map(|s| s.as_string().unwrap_throw())
                        .collect::<Vec<String>>();
                    (e[0].clone(), e[1].clone())
                })
                .collect()
        })
    })
}

/// Creates a Signal that tracks the url fragment.
pub fn use_location_hash() -> Signal<String> {
    *PATHNAME.with(|pathname| {
        assert!(
            pathname.get().is_some(),
            "cannot get hash outside of a Router"
        );

        let pathname = pathname.get().unwrap_throw();

        let on_hashchange = create_signal(());
        window()
            .add_event_listener_with_callback(
                "hashchange",
                Closure::wrap(Box::new(move || {
                    on_hashchange.update(|_| {});
                }) as Box<dyn FnMut()>)
                .into_js_value()
                .unchecked_ref(),
            )
            .unwrap_throw();

        create_memo(move || {
            on_hashchange.track();
            pathname.track();
            window().location().hash().unwrap_throw()
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(target_arch = "wasm32", not(sycamore_force_ssr)))]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg(all(target_arch = "wasm32", not(sycamore_force_ssr)))]
    #[wasm_bindgen_test::wasm_bindgen_test]
    async fn delegates_clicks_to_links_mounted_later_and_cleans_up() {
        use wasm_bindgen_futures::JsFuture;
        use web_sys::HtmlElement;
        use web_sys::js_sys::Promise;

        #[derive(Route, Clone, Copy)]
        enum Routes {
            #[to("/")]
            Home,
            #[not_found]
            NotFound,
        }

        struct TestIntegration(Rc<Cell<u32>>);

        impl Integration for TestIntegration {
            fn current_pathname(&self) -> String {
                "/".to_string()
            }

            fn on_popstate(&self, _: Box<dyn FnMut()>) {}

            fn click_handler(&self) -> Box<dyn Fn(web_sys::MouseEvent)> {
                let clicks = Rc::clone(&self.0);
                Box::new(move |event| {
                    event.prevent_default();
                    clicks.set(clicks.get() + 1);
                })
            }
        }

        let document = document();
        let container = document.create_element("div").unwrap();
        document.body().unwrap().append_child(&container).unwrap();
        let clicks = Rc::new(Cell::new(0));
        let link = Cell::new(None);
        let root = create_root(|| {
            let current = create_signal(0);
            link.set(Some(current));
            let integration = TestIntegration(Rc::clone(&clicks));
            sycamore::render_in_scope(
                move || {
                    view! {
                        Router(
                            integration=integration,
                            view=move |_: Signal<Routes>| view! {
                                if current.get() == 1 {
                                    a(href="#first") { "First" }
                                } else if current.get() == 2 {
                                    a(href="#second") { "Second" }
                                }
                            },
                        )
                    }
                },
                &container,
            );
        });
        JsFuture::from(Promise::resolve(&JsValue::UNDEFINED))
            .await
            .unwrap();

        let current = link.get().unwrap();
        let get_link = || {
            container
                .query_selector("a")
                .unwrap()
                .unwrap()
                .unchecked_into::<HtmlElement>()
        };
        current.set(1);
        get_link().click();
        assert_eq!(clicks.get(), 1);

        current.set(2);
        let second = get_link();
        second.click();
        assert_eq!(clicks.get(), 2);

        root.dispose();
        // Keep a connected target even if disposing the view removed its range.
        second.remove_attribute("href").unwrap();
        container.append_child(&second).unwrap();
        second.click();
        assert_eq!(clicks.get(), 2);
        container.remove();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn static_router() {
        #[derive(Route, Clone, Copy)]
        enum Routes {
            #[to("/")]
            Home,
            #[to("/about")]
            About,
            #[not_found]
            NotFound,
        }

        #[component]
        fn Comp(path: String) -> View {
            let route = Routes::match_route(
                // The user would never use this directly, so they'd never have to do this trick
                // It doesn't matter which variant we provide here, it just needs to conform to
                // `&self` (designed for `struct`s, as in Perseus' router)
                &Routes::Home,
                &path
                    .split('/')
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>(),
            );

            view! {
                StaticRouter(
                    route=route,
                    view=|route: Signal<Routes>| match route.get_untracked() {
                        Routes::Home => view! {
                            "Home"
                        },
                        Routes::About => view! {
                            "About"
                        },
                        Routes::NotFound => view! {
                            "Not Found"
                        },
                    },
                )
            }
        }

        assert_eq!(
            sycamore::render_to_string(|| view! { Comp(path="/".to_string()) }),
            "Home"
        );

        assert_eq!(
            sycamore::render_to_string(|| view! { Comp(path="/about".to_string()) }),
            "About"
        );

        assert_eq!(
            sycamore::render_to_string(|| view! { Comp(path="/404".to_string()) }),
            "Not Found"
        );
    }
}
