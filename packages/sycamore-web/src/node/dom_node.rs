use sycamore_core::Build;
use wasm_bindgen::intern;

use crate::*;

/// View backend for rendering to the browser DOM.
pub struct DomNode {
    pub(crate) raw: web_sys::Node,

    #[cfg(feature = "hydrate")]
    origin: NodeOrigin,
}

/// Where the DOM node came from.
#[cfg(feature = "hydrate")]
enum NodeOrigin {
    /// Created from scratch.
    Fresh,
    /// Adopted from the server-rendered HTML.
    Adopted,
    /// A text node that was created on the server and has not been hydrated yet.
    /// Text nodes are handled specially because they cannot carry hydration keys.
    PendingHydrationText,
}

impl Build<View> for DomNode {
    fn build(self) -> View {
        View::from_node(self)
    }
}

impl DomNode {
    /// Create a new `DomNode` from a raw web-sys node.
    fn fresh(raw: web_sys::Node) -> Self {
        Self {
            raw,
            #[cfg(feature = "hydrate")]
            origin: NodeOrigin::Fresh,
        }
    }

    fn adopt(raw: web_sys::Node) -> Self {
        Self {
            raw,
            #[cfg(feature = "hydrate")]
            origin: NodeOrigin::Adopted,
        }
    }

    fn validate_element(existing: &web_sys::Element, tag: &str) {
        if cfg!(debug_assertions) {
            let existing_tag = existing.tag_name().to_lowercase();
            if existing_tag != tag {
                panic!(
                    "expected element with tag `{tag}`, but found element with tag `{existing_tag}`"
                );
            }
        }
    }
}

impl ViewHtmlNode for DomNode {
    fn create_element(tag: Cow<'static, str>) -> Self {
        match ConstructionMode::current() {
            ConstructionMode::Create => {
                Self::fresh(document().create_element(intern(&tag)).unwrap().into())
            }
            ConstructionMode::Hydrate(session) => {
                let key = session.next_key();
                let element = session
                    .claim_element(key)
                    .expect("could not find element with hydration key");
                Self::validate_element(&element, &tag);
                Self::adopt(element.into())
            }
        }
    }

    fn create_element_ns(namespace: &'static str, tag: Cow<'static, str>) -> Self {
        match ConstructionMode::current() {
            ConstructionMode::Create => Self::fresh(
                document()
                    .create_element_ns(Some(namespace), intern(&tag))
                    .unwrap()
                    .into(),
            ),
            ConstructionMode::Hydrate(session) => {
                let key = session.next_key();
                let element = session
                    .claim_element(key)
                    .expect("could not find element with hydration key");
                Self::validate_element(&element, &tag);
                Self::adopt(element.into())
            }
        }
    }

    fn create_text_node(text: Cow<'static, str>) -> Self {
        match ConstructionMode::current() {
            ConstructionMode::Create => Self::fresh(document().create_text_node(&text).into()),
            ConstructionMode::Hydrate(_session) => Self {
                raw: document().create_text_node(&text).into(),
                #[cfg(feature = "hydrate")]
                origin: NodeOrigin::PendingHydrationText,
            },
        }
    }

    fn create_start_marker() -> Self {
        Self {
            raw: document().create_comment("[").into(),
            #[cfg(feature = "hydrate")]
            origin: NodeOrigin::Fresh,
        }
    }

    fn create_end_marker() -> Self {
        Self {
            raw: document().create_comment("]").into(),
            #[cfg(feature = "hydrate")]
            origin: NodeOrigin::Fresh,
        }
    }

    fn set_attribute(&mut self, name: Cow<'static, str>, value: StringAttribute) {
        // FIXME: use setAttributeNS if SVG
        if let Some(value) = value.as_static() {
            if let Some(value) = value {
                self.raw
                    .unchecked_ref::<web_sys::Element>()
                    .set_attribute(&name, value)
                    .unwrap();
            }
        } else {
            let node = self.raw.clone().unchecked_into::<web_sys::Element>();
            create_effect(move || match value.get_clone() {
                Some(value) => node.set_attribute(&name, &value).unwrap(),
                None => node.remove_attribute(&name).unwrap(),
            });
        }
    }

    fn set_bool_attribute(&mut self, name: Cow<'static, str>, value: BoolAttribute) {
        // FIXME: use setAttributeNS if SVG
        if let Some(value) = value.as_static() {
            if *value {
                self.raw
                    .unchecked_ref::<web_sys::Element>()
                    .set_attribute(&name, "")
                    .unwrap();
            }
        } else {
            let node = self.raw.clone().unchecked_into::<web_sys::Element>();
            create_effect(move || {
                if value.get() {
                    node.set_attribute(&name, "").unwrap();
                } else {
                    node.remove_attribute(&name).unwrap();
                }
            });
        }
    }

    fn set_property(&mut self, name: Cow<'static, str>, value: MaybeDyn<JsValue>) {
        if let Some(value) = value.as_static() {
            assert!(js_sys::Reflect::set(&self.raw, &name.as_ref().into(), value).unwrap_throw())
        } else {
            let node = self.raw.clone().unchecked_into::<web_sys::Element>();
            create_effect(move || {
                assert!(
                    js_sys::Reflect::set(&node, &name.as_ref().into(), &value.get_clone())
                        .unwrap_throw()
                )
            });
        }
    }

    fn set_event_handler(
        &mut self,
        name: Cow<'static, str>,
        handler: impl FnMut(web_sys::Event) + 'static,
    ) {
        let cb = Closure::wrap(Box::new(handler) as Box<dyn FnMut(_)>);
        let node = self.raw.clone();
        node.add_event_listener_with_callback(&name, cb.as_ref().unchecked_ref())
            .unwrap();
        on_cleanup(move || {
            node.remove_event_listener_with_callback(&name, cb.as_ref().unchecked_ref())
                .unwrap();
            drop(cb);
        });
    }

    fn set_inner_html(&mut self, inner_html: Cow<'static, str>) {
        self.raw
            .unchecked_ref::<web_sys::Element>()
            .set_inner_html(&inner_html);
    }

    fn as_web_sys(&self) -> &web_sys::Node {
        &self.raw
    }

    fn from_web_sys(node: web_sys::Node) -> Self {
        Self {
            raw: node,
            #[cfg(feature = "hydrate")]
            origin: NodeOrigin::Fresh,
        }
    }

    fn append_view(&mut self, view: View) {
        match ConstructionMode::current() {
            ConstructionMode::Create => insert_view(&self.raw, &view, None),
            ConstructionMode::Hydrate(_session) => hydrate_view(&self.raw, &view),
        }
    }
}

/// Insert a view into the DOM before the `before` node.
///
/// If `before` is `None`, the view will be appended to the end of the parent node.
///
/// # Panics
/// - Panics if `before` is not a child of `parent`.
/// - Panics if `parent` is a text node.
fn insert_view(parent: &web_sys::Node, view: &View, before: Option<&web_sys::Node>) {
    for part in &view.parts {
        match part {
            ViewPart::Node(node) => {
                if let Some(before) = before {
                    parent
                        .insert_before(node.as_web_sys(), Some(before))
                        .unwrap();
                } else {
                    parent.append_child(node.as_web_sys()).unwrap();
                }
            }
            ViewPart::Dynamic(signal) => {
                let start = HtmlNode::create_start_marker();
                let end = HtmlNode::create_end_marker();
                if let Some(before) = before {
                    parent
                        .insert_before(start.as_web_sys(), Some(before))
                        .unwrap();
                    parent
                        .insert_before(end.as_web_sys(), Some(before))
                        .unwrap();
                } else {
                    parent.append_child(start.as_web_sys()).unwrap();
                    parent.append_child(end.as_web_sys()).unwrap();
                }

                insert_dynamic_view(parent, start.as_web_sys(), *signal, end.as_web_sys());
            }
        }
    }
}

/// Insert a dynamic view into the DOM. This will create an effect that will update the view
/// whenever the signal changes.
///
/// The view will be inserted into the DOM in between the `before` and `after` nodes.
///
/// # Panics
/// - Panics if the `before` and `after` nodes are not initially siblings in the DOM.
/// - Panics if `before` and `after` are not children of `parent`.
/// - Panics if `parent` is a text node.
fn insert_dynamic_view(
    parent: &web_sys::Node,
    start: &web_sys::Node,
    content: Signal<View>,
    end: &web_sys::Node,
) {
    debug_assert!(
        &start.parent_node().unwrap() == parent,
        "`start` node is not a child of `parent`"
    );
    debug_assert!(
        &end.parent_node().unwrap() == parent,
        "`end` node is not a child of `parent`"
    );
    debug_assert!(
        &start.next_sibling().unwrap() == end,
        "`start` and `end` nodes are not siblings"
    );

    let parent = parent.clone();
    let start = start.clone();
    let end = end.clone();
    create_effect_initial(move || {
        content.with(|view| insert_view(&parent, view, Some(&end)));
        (
            Box::new(move || {
                // Clean up the old nodes before inserting the new ones.
                let mut next = start.next_sibling().unwrap();
                while &next != &end {
                    let tmp = next.next_sibling().unwrap();
                    parent.remove_child(&next).unwrap();
                    next = tmp;
                }

                content.with(|view| insert_view(&parent, view, Some(&end)));
            }),
            (),
        )
    })
}

fn hydrate_view(parent: &web_sys::Node, view: &View) {
    todo!()
}
