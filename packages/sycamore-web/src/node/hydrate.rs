//! Utilities related to client side hydration.

use std::collections::HashMap;
use std::fmt;
#[cfg(all(
    feature = "suspense",
    any(feature = "hydrate", not(target_arch = "wasm32"), sycamore_force_ssr)
))]
use std::num::NonZeroU32;

use sycamore_reactive::*;
use wasm_bindgen::JsCast;

/// Comment values that delimit content the client must leave untouched.
#[cfg(any(feature = "hydrate", not(target_arch = "wasm32"), sycamore_force_ssr))]
pub(crate) const OPAQUE_RANGE_MARKERS: (&str, &str) = ("opaque-range", "/opaque-range");

/// Keeps track of whether we are creating new DOM nodes or hydrating existing ones.
///
/// This type is intended to be used as a context value, and should be accessed via
/// [`ConstructionMode::current()`].
#[derive(Clone, Copy)]
pub enum ConstructionMode {
    Create,
    Hydrate(HydrationSession),
}

impl ConstructionMode {
    /// Try to get the current construction mode from the reactive context.
    ///
    /// To set the hydration session, use [`provide_context`](sycamore_reactive::provide_context)
    /// with a [`ConstructionMode`].
    pub fn current() -> Self {
        // SSR emits hydratable HTML regardless of whether client hydration is compiled in.
        if cfg!(all(
            target_arch = "wasm32",
            not(sycamore_force_ssr),
            not(feature = "hydrate")
        )) {
            return Self::Create;
        }

        match try_use_context::<Self>() {
            Some(Self::Hydrate(session)) if session.active.get_untracked() => {
                Self::Hydrate(session)
            }
            _ => Self::Create,
        }
    }

    pub fn is_hydrating(&self) -> bool {
        matches!(self, Self::Hydrate(_))
    }
}

/// A hydration session tracks which elements still need to be claimed and allocates hydration keys.
#[derive(Clone, Copy)]
pub struct HydrationSession {
    elements: MutSignal<HashMap<HydrationKey, web_sys::Element>>,
    active: MutSignal<bool>,
    next_key: MutSignal<HydrationKey>,
    #[cfg(all(feature = "hydrate", target_arch = "wasm32", not(sycamore_force_ssr)))]
    mount_callbacks: MutSignal<Vec<Box<dyn FnOnce()>>>,
}

impl HydrationSession {
    /// Claim an element with the given hydration key. If the element exists, it will be removed
    /// from the session and returned. If it does not exist, `None` will be returned.
    pub fn claim_element(&self, key: HydrationKey) -> Option<web_sys::Element> {
        self.elements.update(|elements| elements.remove(&key))
    }

    /// Create a new [`HydrationSession`] from a parent node for hydration.
    /// This will scan all descendent nodes of the parent node for `data-hk` attributes and store
    /// them in a map.
    pub fn new_hydrate(parent: web_sys::Node) -> Self {
        // Get all nodes with `data-hk` attribute.
        let existing_nodes = parent
            .unchecked_ref::<web_sys::Element>()
            .query_selector_all("[data-hk]")
            .unwrap();

        let mut elements = HashMap::new();
        let len = existing_nodes.length();
        for i in 0..len {
            let element = existing_nodes
                .get(i)
                .unwrap()
                .unchecked_into::<web_sys::Element>();
            let hk = element.get_attribute("data-hk").unwrap();
            let key = HydrationKey::parse(&hk).expect("could not parse hydration key");
            if key.suspense == 0 {
                elements.insert(key, element);
            }
        }

        Self::new(elements, 0)
    }

    /// Create a new [`HydrationSession`] for SSR rendering. This will not scan any nodes, and will
    /// not be able to claim any elements. Instead, this will only be used to generate new hydration
    /// keys for elements that are created during SSR rendering.
    pub fn new_ssr() -> Self {
        Self::new(HashMap::new(), 0)
    }

    #[cfg(all(
        feature = "suspense",
        any(not(target_arch = "wasm32"), sycamore_force_ssr)
    ))]
    pub(crate) fn new_ssr_namespace(namespace: NonZeroU32) -> Self {
        Self::new(HashMap::new(), namespace.get())
    }

    fn new(elements: HashMap<HydrationKey, web_sys::Element>, namespace: u32) -> Self {
        Self {
            elements: create_signal(elements),
            active: create_signal(true),
            next_key: create_signal(HydrationKey {
                suspense: namespace,
                element: 0,
            }),
            #[cfg(all(feature = "hydrate", target_arch = "wasm32", not(sycamore_force_ssr)))]
            mount_callbacks: create_signal(Vec::new()),
        }
    }

    #[cfg(all(
        feature = "hydrate",
        feature = "suspense",
        target_arch = "wasm32",
        not(sycamore_force_ssr)
    ))]
    pub(crate) fn is_deferred(self) -> bool {
        self.next_key.get_untracked().suspense != 0
    }

    /// Index only the elements owned by this independently resolved subtree.
    #[cfg(all(
        feature = "hydrate",
        feature = "suspense",
        target_arch = "wasm32",
        not(sycamore_force_ssr)
    ))]
    pub(crate) fn new_hydrate_range(
        mut next: Option<web_sys::Node>,
        end: &web_sys::Node,
        namespace: NonZeroU32,
    ) -> Self {
        let mut elements = HashMap::new();
        let mut add = |element: web_sys::Element| {
            if let Some(key) = element
                .get_attribute("data-hk")
                .and_then(|key| HydrationKey::parse(&key))
                && key.suspense == namespace.get()
            {
                elements.insert(key, element);
            }
        };
        while let Some(node) = next {
            if &node == end {
                break;
            }
            next = node.next_sibling();
            if let Ok(element) = node.dyn_into::<web_sys::Element>() {
                let descendants = element.query_selector_all("[data-hk]").unwrap();
                add(element);
                for i in 0..descendants.length() {
                    add(descendants.get(i).unwrap().unchecked_into());
                }
            }
        }
        Self::new(elements, namespace.get())
    }

    #[cfg(all(feature = "hydrate", target_arch = "wasm32", not(sycamore_force_ssr)))]
    pub(crate) fn queue_mount(self, f: impl FnOnce() + 'static) {
        self.mount_callbacks
            .update_silent(|callbacks| callbacks.push(Box::new(f)));
    }

    /// End the client claim phase without disposing the reactive scopes created during
    /// hydration. Later construction in those scopes falls back to `Create`.
    #[cfg(all(feature = "hydrate", target_arch = "wasm32", not(sycamore_force_ssr)))]
    pub(crate) fn finish(self) {
        self.active.set_silent(false);
        self.elements.set_silent(HashMap::new());
        for callback in self.mount_callbacks.take_silent() {
            crate::queue_microtask(callback);
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
}

/// A unique key for a node that is used for hydration.
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
