//! Implementation of rendering backend.

use std::fmt;
use std::num::NonZeroU32;

use crate::*;

is_not_ssr!(
    mod dom_node;
    #[cfg(feature = "hydrate")]
    mod hydrate_node;
);
is_ssr!(
    mod ssr_node;
);
mod dom_render;
mod ssr_render;

// We add this so that we get IDE support in Rust Analyzer.
#[cfg(rust_analyzer)]
mod dom_node;
#[cfg(rust_analyzer)]
mod hydrate_node;

#[cfg_not_ssr]
pub use dom_node::*;
pub use dom_render::*;
#[cfg_not_ssr]
#[cfg(feature = "hydrate")]
pub use hydrate_node::*;
#[cfg_ssr]
pub use ssr_node::*;
pub use ssr_render::*;

/// A trait that should be implemented for anything that represents an HTML node.
pub trait ViewHtmlNode: ViewNode {
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
    /// Create a new HTML marker (comment) node.
    fn create_marker_node() -> Self;

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
}

/// A trait for unwrapping a type into an `HtmlNode`.
pub trait AsHtmlNode {
    fn as_html_node(&mut self) -> &mut HtmlNode;
}

thread_local! {
    /// Whether we are in hydration mode or not.
    pub(crate) static IS_HYDRATING: Cell<bool> = const { Cell::new(false) };
}

/// Returns whether we are currently hydrating or not.
pub fn is_hydrating() -> bool {
    IS_HYDRATING.with(Cell::get)
}

/// A struct for keeping track of state used for hydration.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HydrationRegistry {
    next_key: Signal<HydrationKey>,
}

// This is only used when hydrating.
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
impl HydrationRegistry {
    pub fn new() -> Self {
        HydrationRegistry {
            next_key: create_signal(HydrationKey {
                suspense: 0,
                element: 0,
            }),
        }
    }

    /// Get the next hydration key and increment the internal state. This new key will be unique.
    pub fn next_key(self) -> HydrationKey {
        let key = self.next_key.get_untracked();
        self.next_key.set_silent(HydrationKey {
            suspense: key.suspense,
            element: key.element + 1,
        });
        key
    }

    /// Run the given function within a suspense scope.
    ///
    /// This sets the suspense key to the passed value and resets the element key to 0.
    pub fn in_suspense_scope<T>(suspense: NonZeroU32, f: impl FnOnce() -> T) -> T {
        let mut ret = None;
        create_child_scope(|| {
            provide_context(HydrationRegistry {
                next_key: create_signal(HydrationKey {
                    suspense: suspense.get(),
                    element: 0,
                }),
            });
            ret = Some(f());
        });
        ret.unwrap()
    }
}

impl Default for HydrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HydrationKey {
    /// Suspense key, or 0 if not in a suspense boundary.
    pub suspense: u32,
    /// Element key.
    pub element: u32,
}

impl fmt::Display for HydrationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.suspense, self.element)
    }
}

impl HydrationKey {
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.split('.');
        let suspense = parts.next()?.parse().ok()?;
        let element = parts.next()?.parse().ok()?;
        Some(HydrationKey { suspense, element })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_hydration_key() {
        let key = HydrationKey {
            suspense: 1,
            element: 2,
        };
        assert_eq!(key.to_string(), "1.2");
    }

    #[test]
    fn parse_hydration_key() {
        assert_eq!(
            HydrationKey::parse("1.2"),
            Some(HydrationKey {
                suspense: 1,
                element: 2
            })
        );
        assert_eq!(HydrationKey::parse("1"), None);
    }
}
