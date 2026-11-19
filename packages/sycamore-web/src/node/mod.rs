//! Implementation of rendering backend.

use crate::*;

#[cfg(any(feature = "hydrate", not(target_arch = "wasm32"), sycamore_force_ssr))]
fn is_raw_text_element(tag: &str) -> bool {
    matches!(
        tag,
        "script" | "style" | "xmp" | "iframe" | "noembed" | "noframes" | "noscript" | "plaintext"
    )
}

// mod dom_node;
// pub type HtmlNode = dom_node::DomNode;

is_not_ssr!(
    mod dom_list;
    mod dom_node;
    #[cfg(feature = "suspense")]
    mod dom_suspense;
    #[cfg(feature = "suspense")]
    pub(crate) use dom_suspense::DomSuspense;

    pub use dom_node::*;

    /// A type alias for the rendering backend.
    pub type HtmlNode = DomNode;
);
is_ssr!(
    mod ssr_node;
    pub use ssr_node::*;
    /// A type alias for the rendering backend.
    pub type HtmlNode = SsrNode;
);

mod dom_render;
mod hydrate;
mod ssr_render;

pub use dom_render::*;
pub use hydrate::*;
pub use ssr_render::*;

/// A trait that should be implemented for anything that represents an HTML node.
pub trait ViewHtmlNode: Sized {
    /// Create a new HTML element.
    fn create_element(tag: Cow<'static, str>) -> Self;
    /// Create a new HTML element with a XML namespace.
    fn create_element_ns(namespace: &'static str, tag: Cow<'static, str>) -> Self;
    /// Create a new HTML text node.
    fn create_text_node(text: Cow<'static, str>) -> Self;
    /// Create a new HTML text node whose value will be changed dynamically.
    fn create_dynamic_text_node(text: Cow<'static, str>) -> Self {
        Self::create_text_node(text)
    }
    /// Create a new start marker node (comment node).
    fn create_start_marker() -> Self;

    /// Create a new end marker node (comment node).
    fn create_end_marker() -> Self;

    /// Set an HTML attribute.
    fn set_attribute(&mut self, name: Cow<'static, str>, value: StringAttribute);
    /// Set a boolean HTML attribute.
    fn set_bool_attribute(&mut self, name: Cow<'static, str>, value: BoolAttribute);
    /// Set a JS property on an element.
    fn set_property(&mut self, name: Cow<'static, str>, value: MaybeDyn<JsValue>);
    /// Set an event handler on an element.
    fn set_event_handler(
        &mut self,
        name: Cow<'static, str>,
        handler: impl FnMut(web_sys::Event) + 'static,
    );
    /// Set the inner HTML value of an element.
    fn set_inner_html(&mut self, inner_html: Cow<'static, str>);

    /// Return the raw web-sys node.
    fn as_web_sys(&self) -> &web_sys::Node;
    /// Wrap a raw web-sys node.
    fn from_web_sys(node: web_sys::Node) -> Self;

    /// Append a [`View`] to this node.
    /// This view could contain a single node, or multiple nodes, some which are dynamic.
    ///
    /// This method should also handle reactive updates to the view in the case of dynamic parts.
    fn append_view(&mut self, view: View);
}

/// A trait for unwrapping a type into an `HtmlNode`.
pub trait AsHtmlNode {
    fn as_html_node(&mut self) -> &mut HtmlNode;
}
