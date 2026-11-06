use std::any::{Any, TypeId};

use wasm_bindgen::intern;

use crate::*;

/// View backend for rendering to the browser DOM.
pub struct DomNode {
    pub(crate) raw: web_sys::Node,
}

impl Build<View<DomNode>> for DomNode {
    fn build(self) -> View<DomNode> {
        View::from_node(self)
    }
}

impl ViewNode for DomNode {
    fn append_child(&mut self, child: Self) {
        self.raw.append_child(&child.raw).unwrap();
    }

    fn create_dynamic_view<U: Into<View<Self>> + 'static>(
        f: impl FnMut() -> U + 'static,
    ) -> View<Self> {
        _create_dynamic_view(f)
    }
}

/// Internal implementation that is shared between `DomNode` and `HydrateNode`.
pub(crate) fn _create_dynamic_view<T: ViewHtmlNode, U: Into<View<T>> + 'static>(
    mut f: impl FnMut() -> U + 'static,
) -> View<T> {
    // If `view` is just a single text node, we can just return this node and set up an
    // effect to update its text value without ever creating more nodes.
    if TypeId::of::<U>() == TypeId::of::<String>() {
        create_effect_initial(move || {
            let mut value = Some(f());
            let text: &mut Option<String> = (&mut value as &mut dyn Any).downcast_mut().unwrap();
            let view = View::from_node(T::create_dynamic_text_node(text.take().unwrap().into()));
            debug_assert_eq!(
                view.nodes.len(),
                1,
                "dynamic text view should have exactly one text node"
            );
            let node = view.nodes[0].as_web_sys().clone();
            (
                Box::new(move || {
                    let text = f();
                    let text = (&text as &dyn Any).downcast_ref::<String>().unwrap();
                    node.set_text_content(Some(text));
                }),
                view,
            )
        })
    } else {
        let start = T::create_marker_node();
        let start_node = start.as_web_sys().clone();
        let end = T::create_marker_node();
        let end_node = end.as_web_sys().clone();
        let view = create_effect_initial(move || {
            let view = f().into();
            (
                Box::new(move || {
                    let new = f().into();
                    if let Some(parent) = start_node.parent_node() {
                        // Clear all the old nodes away.
                        let old = utils::get_nodes_between(&start_node, &end_node);
                        for node in old {
                            parent.remove_child(&node).unwrap();
                        }
                        // Insert the new nodes in their place.
                        for node in new.nodes {
                            parent
                                .insert_before(node.as_web_sys(), Some(&end_node))
                                .unwrap();
                        }
                    } else if cfg!(debug_assertions) {
                        console_warn!("cannot update a dynamic view if it is not mounted");
                    }
                }),
                view,
            )
        });

        View::from((start, view, end))
    }
}

impl ViewHtmlNode for DomNode {
    fn create_element(tag: Cow<'static, str>) -> Self {
        Self {
            raw: document().create_element(intern(&tag)).unwrap().into(),
        }
    }

    fn create_element_ns(namespace: &'static str, tag: Cow<'static, str>) -> Self {
        Self {
            raw: document()
                .create_element_ns(Some(namespace), intern(&tag))
                .unwrap()
                .into(),
        }
    }

    fn create_text_node(text: Cow<'static, str>) -> Self {
        Self {
            raw: document().create_text_node(&text).into(),
        }
    }

    fn create_marker_node() -> Self {
        Self {
            raw: document().create_comment("").into(),
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
        Self { raw: node }
    }
}
