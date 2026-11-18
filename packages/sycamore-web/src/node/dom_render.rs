use super::*;

/// Render a [`View`] into the DOM.
/// Alias for [`render_to`] with `parent` being the `<body>` tag.
pub fn render(view: impl Into<Children>) {
    render_to(view, &document().body().unwrap());
}

/// Render a [`View`] under a `parent` node.
/// For rendering under the `<body>` tag, use [`render`] instead.
pub fn render_to(view: impl Into<Children>, parent: &web_sys::Node) {
    // Do not call the destructor function, effectively leaking the scope.
    let _ = create_root(|| render_in_scope(view, parent));
}

/// Render a [`View`] under a `parent` node, in a way that can be cleaned up.
///
/// This function is intended to be used for injecting an ephemeral sycamore view into a
/// non-sycamore app (for example, a file upload modal where you want to cancel the upload if the
/// modal is closed).
///
/// It is, however, preferable to have a single call to [`render`] or [`render_to`] at the top
/// level of your app long-term. For rendering a view that will never be unmounted from the dom,
/// use [`render_to`] instead. For rendering under the `<body>` tag, use [`render`] instead.
///
/// It is expected that this function will be called inside a reactive root, usually created using
/// [`create_root`].
pub fn render_in_scope(view: impl Into<Children>, parent: &web_sys::Node) {
    if is_ssr!() {
        panic!("`render_in_scope` is not available in SSR mode");
    } else {
        let mut parent = HtmlNode::from_web_sys(parent.clone());
        parent.append_view(view.into().call());
    }
}

/// Render a [`View`] under a `parent` node by reusing existing nodes (client side
/// hydration).
///
/// Alias for [`hydrate_to`] with `parent` being the `<body>` tag.
/// For rendering without hydration, use [`render`](super::render) instead.
#[cfg(feature = "hydrate")]
pub fn hydrate(view: impl Into<Children>) {
    hydrate_to(view, &document().body().unwrap());
}

/// Render a [`View`] under a `parent` node by reusing existing nodes (client side
/// hydration).
///
/// For rendering under the `<body>` tag, use [`hydrate`] instead.
/// For rendering without hydration, use [`render`](super::render) instead.
/// With streaming SSR, call this after the response has finished applying its fragments.
#[cfg(feature = "hydrate")]
pub fn hydrate_to(view: impl Into<Children>, parent: &web_sys::Node) {
    // Do not call the destructor function, effectively leaking the scope.
    let _ = create_root(|| hydrate_in_scope(view, parent));
}

/// Render a [`View`] under a `parent` node, in a way that can be cleaned up.
///
/// This function is intended to be used for injecting an ephemeral sycamore view into a
/// non-sycamore app (for example, a file upload modal where you want to cancel the upload if the
/// modal is closed).
///
/// It is expected that this function will be called inside a reactive root, usually created using
/// [`create_root`].
#[cfg(feature = "hydrate")]
pub fn hydrate_in_scope(view: impl Into<Children>, parent: &web_sys::Node) {
    is_ssr! {
        let _ = view;
        let _ = parent;
        panic!("`hydrate_in_scope` is not available in SSR mode");
    }
    is_not_ssr! {
        // Get mode from HydrationScript.
        let mode = js_sys::Reflect::get(&window(), &"__sycamore_ssr_mode".into()).unwrap();
        let mode = if mode.is_undefined() {
            SsrMode::Sync
        } else if mode == "blocking" {
            SsrMode::Blocking
        } else if mode == "streaming" {
            SsrMode::Streaming
        } else {
            panic!("invalid SSR mode {mode:?}")
        };

        if mode == SsrMode::Streaming
            && js_sys::Reflect::get(&window(), &"__sycamore_stream_complete".into())
                .unwrap()
                .as_bool() != Some(true)
        {
            panic!("streaming hydration requires the SSR stream to finish first");
        }

        let session = HydrationSession::new_hydrate(parent.clone());
        let mut parent = HtmlNode::from_web_sys(parent.clone());

        provide_context(mode);
        provide_context(ConstructionMode::Hydrate(session));

        parent.append_view(view.into().call());
        parent.finish_hydrating_children();
        session.finish();
    }
}
