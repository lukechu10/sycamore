use std::cell::{LazyCell, RefCell};
use std::collections::HashMap;

use sycamore_core::Build;

use super::dom_node::DomNode;
use super::*;

/// View backend for rendering to the browser DOM.
pub struct HydrateNode(NodeState);

/// The state of a hydrated node.
enum NodeState {
    /// A fully hydrated node. Elements are always hydrated.
    Hydrated(DomNode),
    /// A text node that is dynamic. Replace the original text node with this one when hydrating.
    TextDynamic(DomNode),
    /// A static text node. No need to be hydrated.
    TextStatic,
    /// A marker node that has not been hydrated yet. The node will be hydrated when it is
    /// appended.
    Marker(DomNode),
}

impl NodeState {
    /// Unwraps the `DomNode` if the node is hydrated. Panics otherwise.
    #[cfg_attr(debug_assertions, track_caller)]
    fn unwrap(self) -> DomNode {
        match self {
            Self::Hydrated(node) | Self::TextDynamic(node) | Self::Marker(node) => node,
            _ => panic!("node is not hydrated"),
        }
    }

    /// Tries to get a reference to the `DomNode` if the node is hydrated. Panics otherwise.
    #[cfg_attr(debug_assertions, track_caller)]
    fn unwrap_ref(&self) -> &DomNode {
        match self {
            Self::Hydrated(node) | Self::TextDynamic(node) | Self::Marker(node) => node,
            _ => panic!("node is not hydrated"),
        }
    }

    /// Tries to get a mutable reference to the `DomNode` if the node is hydrated. Panics otherwise.
    #[cfg_attr(debug_assertions, track_caller)]
    fn unwrap_mut(&mut self) -> &mut DomNode {
        match self {
            Self::Hydrated(node) | Self::TextDynamic(node) | Self::Marker(node) => node,
            _ => panic!("node is not hydrated"),
        }
    }
}

impl Build<View> for HydrateNode {
    fn build(self) -> View {
        View::from_node(self)
    }
}

fn check_node(node: &web_sys::Node, tag: &str, key: HydrationKey) {
    if cfg!(debug_assertions) {
        let node = node.unchecked_ref::<web_sys::Element>();
        node.set_attribute("data-hydrated", "").unwrap();

        assert_eq!(node.node_type(), web_sys::Node::ELEMENT_NODE);
        assert_eq!(node.get_attribute("data-hk"), Some(key.to_string()));
        let actual_tag = node.tag_name().to_lowercase();
        if actual_tag != tag {
            console_warn!(
                "hydration tag name mismatch at `{key}`: expected '{tag}', got '{actual_tag}'"
            );
        }
    }
}

impl ViewHtmlNode for HydrateNode {
    fn create_element(tag: Cow<'static, str>) -> Self {
        if IS_HYDRATING.get() {
            let reg: HydrationRegistry = use_context();
            let key = reg.next_key();
            let node = HYDRATE_NODES
                .with(|nodes| nodes.borrow_mut().remove(&key))
                .unwrap_or_else(|| {
                    panic!("node with hk `{key}` not found, expected a <{tag}> element")
                });
            check_node(node.as_web_sys(), &tag, key);
            node
        } else {
            Self(NodeState::Hydrated(DomNode::create_element(tag)))
        }
    }

    fn create_element_ns(namespace: &'static str, tag: Cow<'static, str>) -> Self {
        if IS_HYDRATING.get() {
            let reg: HydrationRegistry = use_context();
            let key = reg.next_key();
            let node = HYDRATE_NODES
                .with(|nodes| nodes.borrow_mut().remove(&key))
                .unwrap_or_else(|| panic!("node with hk `{key}` not found"));
            if cfg!(debug_assertions) {
                node.as_web_sys()
                    .unchecked_ref::<web_sys::Element>()
                    .set_attribute("data-hydrated", "")
                    .unwrap();
            }
            node
        } else {
            Self(NodeState::Hydrated(DomNode::create_element_ns(
                namespace, tag,
            )))
        }
    }

    fn create_text_node(text: Cow<'static, str>) -> Self {
        if IS_HYDRATING.get() {
            Self(NodeState::TextStatic)
        } else {
            Self(NodeState::Hydrated(DomNode::create_text_node(text)))
        }
    }

    fn create_dynamic_text_node(text: Cow<'static, str>) -> Self {
        if IS_HYDRATING.get() {
            Self(NodeState::TextDynamic(DomNode::create_text_node(text)))
        } else {
            Self(NodeState::Hydrated(DomNode::create_text_node(text)))
        }
    }

    fn create_marker_node() -> Self {
        // Marker nodes are not hydrated until they are appended.
        Self(NodeState::Marker(DomNode::create_marker_node()))
    }

    fn set_attribute(&mut self, name: Cow<'static, str>, value: StringAttribute) {
        // FIXME: use setAttributeNS if SVG
        if IS_HYDRATING.get() {
            // Noop if value is static since attributes are already set.
            if value.as_static().is_none() {
                let node = self
                    .as_web_sys()
                    .clone()
                    .unchecked_into::<web_sys::Element>();
                create_effect_initial(move || {
                    let _ = value.track(); // Track dependencies of value.
                    (
                        Box::new(move || match value.get_clone() {
                            Some(value) => node.set_attribute(&name, &value).unwrap(),
                            None => node.remove_attribute(&name).unwrap(),
                        }),
                        (),
                    )
                });
            }
        } else {
            self.0.unwrap_mut().set_attribute(name, value);
        }
    }

    fn set_bool_attribute(&mut self, name: Cow<'static, str>, value: BoolAttribute) {
        // FIXME: use setAttributeNS if SVG
        if IS_HYDRATING.get() {
            if value.as_static().is_none() {
                let node = self
                    .as_web_sys()
                    .clone()
                    .unchecked_into::<web_sys::Element>();
                create_effect_initial(move || {
                    let _ = value.track(); // Track dependencies of f.
                    (
                        Box::new(move || {
                            if value.get() {
                                node.set_attribute(&name, "").unwrap();
                            } else {
                                node.remove_attribute(&name).unwrap();
                            }
                        }),
                        (),
                    )
                });
            }
        } else {
            self.0.unwrap_mut().set_bool_attribute(name, value);
        }
    }

    fn set_property(&mut self, name: Cow<'static, str>, value: MaybeDyn<JsValue>) {
        self.0.unwrap_mut().set_property(name, value);
    }

    fn set_event_handler(
        &mut self,
        name: Cow<'static, str>,
        handler: impl FnMut(web_sys::Event) + 'static,
    ) {
        self.0.unwrap_mut().set_event_handler(name, handler);
    }

    fn set_inner_html(&mut self, inner_html: Cow<'static, str>) {
        // If we are hydrating, inner HTML should already be set.
        if !IS_HYDRATING.get() {
            self.0.unwrap_mut().set_inner_html(inner_html);
        }
    }

    #[cfg_attr(debug_assertions, track_caller)]
    fn as_web_sys(&self) -> &web_sys::Node {
        self.0.unwrap_ref().as_web_sys()
    }

    fn from_web_sys(node: web_sys::Node) -> Self {
        Self(NodeState::Hydrated(DomNode::from_web_sys(node)))
    }

    fn append_view(&mut self, view: View) {
        self.0.unwrap_mut().append_view(view);
    }
}

thread_local! {
    /// A list of nodes to be hydrated. The `Vec` should be sorted in reverse order of hydration
    /// key. Every time a node is hydrated, it should be popped from this list.
    pub(crate) static HYDRATE_NODES: LazyCell<RefCell<HashMap<HydrationKey, HydrateNode>>> = LazyCell::new(Default::default);
}
