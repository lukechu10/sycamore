//! Utilities related to client side hydration.

use std::collections::HashMap;
use std::fmt;

use sycamore_reactive::*;
use wasm_bindgen::JsCast;

/// Keeps track of whether we are creating new DOM nodes or hydrating existing ones.
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
        if cfg!(not(feature = "hydrate")) {
            Self::Create
        } else {
            try_use_context::<Self>().unwrap_or(Self::Create)
        }
    }

    pub fn is_hydrating(&self) -> bool {
        matches!(self, Self::Hydrate(_))
    }
}

/// A hydration session keeps track of all elements that need to be hydrated, as well as the current
/// position in the hydration process.
#[derive(Clone, Copy)]
pub struct HydrationSession {
    elements: MutSignal<HashMap<HydrationKey, web_sys::Element>>,
    cursor: MutSignal<Option<HydrationCursor>>,
    next_key: MutSignal<HydrationKey>,
}

impl HydrationSession {
    /// CLaim an element with the given hydration key. If the element exists, it will be removed
    /// from the session and returned. If it does not exist, `None` will be returned.
    pub fn claim_element(&self, key: HydrationKey) -> Option<web_sys::Element> {
        self.elements.update(|elements| elements.remove(&key))
    }

    /// Create a new [`HydrationSession`] from a parent node.
    /// This will scan all descendent nodes of the parent node for `data-hk` attributes and store
    /// them in a map.
    pub fn new(parent: web_sys::Node) -> Self {
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
            elements.insert(key, element);
        }

        Self {
            elements: create_signal(elements),
            cursor: create_signal(None),
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
}

/// Represents the unconsumed suffix of a child sequence.
///
/// ```text
/// parent:
///     already consumed | start | ... remaining children ... | end | ...
/// ```
pub struct HydrationCursor {
    parent: web_sys::Node,
    start: Option<web_sys::Node>,
    end: Option<web_sys::Node>,
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
