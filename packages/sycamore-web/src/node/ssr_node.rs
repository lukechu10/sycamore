use std::any::{Any, TypeId};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;

use crate::*;

pub enum SsrNode {
    Element {
        tag: Cow<'static, str>,
        attributes: Vec<(Cow<'static, str>, Cow<'static, str>)>,
        bool_attributes: Vec<(Cow<'static, str>, bool)>,
        children: Vec<Self>,
        // NOTE: This field is boxed to avoid allocating memory for a field that is rarely used.
        inner_html: Option<Box<Cow<'static, str>>>,
        hk_key: Option<HydrationKey>,
    },
    TextDynamic {
        text: Arc<Mutex<String>>,
    },
    TextStatic {
        text: Cow<'static, str>,
    },
    Marker,
    /// SSR by default does not update to any dynamic changes in the view. This special node allows
    /// dynamically changing the view tree before it is rendered.
    ///
    /// This is used for updating the view with suspense content once it is resolved.
    Dynamic {
        view: Arc<Mutex<View<Self>>>,
    },
}

impl From<SsrNode> for View<SsrNode> {
    fn from(node: SsrNode) -> Self {
        View::from_node(node)
    }
}

impl ViewNode for SsrNode {
    fn append_child(&mut self, child: Self) {
        match self {
            Self::Element { children, .. } => {
                children.push(child);
            }
            _ => panic!("can only append child to an element"),
        }
    }

    fn create_dynamic_view<U: Into<View<Self>> + 'static>(
        mut f: impl FnMut() -> U + 'static,
    ) -> View<Self> {
        // If `view` is just a single text node, we can just return this node since text nodes are
        // specialized. Otherwise, we must create two marker nodes to represent start and end
        // respectively.
        if TypeId::of::<U>() == TypeId::of::<String>() {
            // TODO: Once the reactive graph is sync, we can replace this with a signal.
            let text = Arc::new(Mutex::new(String::new()));
            create_effect({
                let text = text.clone();
                move || {
                    let mut value = Some(f());
                    let value: &mut Option<String> =
                        (&mut value as &mut dyn Any).downcast_mut().unwrap();
                    *text.lock().unwrap() = value.take().unwrap();
                }
            });
            View::from(SsrNode::TextDynamic { text })
        } else {
            let start = Self::create_marker_node();
            let end = Self::create_marker_node();
            // TODO: Once the reactive graph is sync, we can replace this with a signal.
            let view = Arc::new(Mutex::new(View::new()));
            create_effect({
                let view = view.clone();
                move || {
                    let value = f();
                    *view.lock().unwrap() = value.into();
                }
            });
            View::from((start, Self::Dynamic { view }, end))
        }
    }
}

impl ViewHtmlNode for SsrNode {
    fn create_element(tag: Cow<'static, str>) -> Self {
        let hk_key = if IS_HYDRATING.get() {
            let reg: HydrationRegistry = use_context();
            Some(reg.next_key())
        } else {
            None
        };
        Self::Element {
            tag,
            attributes: Vec::new(),
            bool_attributes: Vec::new(),
            children: Vec::new(),
            inner_html: None,
            hk_key,
        }
    }

    fn create_element_ns(_namespace: &str, tag: Cow<'static, str>) -> Self {
        // SVG namespace is ignored in SSR mode.
        Self::create_element(tag)
    }

    fn create_text_node(text: Cow<'static, str>) -> Self {
        Self::TextStatic { text }
    }

    fn create_dynamic_text_node(text: Cow<'static, str>) -> Self {
        Self::TextDynamic {
            text: Arc::new(Mutex::new(text.to_string())),
        }
    }

    fn create_marker_node() -> Self {
        Self::Marker
    }

    fn set_attribute(&mut self, name: Cow<'static, str>, value: StringAttribute) {
        match self {
            Self::Element { attributes, .. } => {
                if let Some(value) = value.evaluate() {
                    attributes.push((name, value))
                }
            }
            _ => panic!("can only set attribute on an element"),
        }
    }

    fn set_bool_attribute(&mut self, name: Cow<'static, str>, value: BoolAttribute) {
        match self {
            Self::Element {
                bool_attributes, ..
            } => bool_attributes.push((name, value.evaluate())),
            _ => panic!("can only set attribute on an element"),
        }
    }

    fn set_property(&mut self, _name: Cow<'static, str>, _value: MaybeDyn<JsValue>) {
        // Noop in SSR mode.
    }

    fn set_event_handler(
        &mut self,
        _name: Cow<'static, str>,
        _handler: impl FnMut(web_sys::Event) + 'static,
    ) {
        // Noop in SSR mode.
    }

    fn set_inner_html(&mut self, inner_html: Cow<'static, str>) {
        match self {
            Self::Element {
                inner_html: slot, ..
            } => *slot = Some(Box::new(inner_html)),
            _ => panic!("can only set inner_html on an element"),
        }
    }

    fn as_web_sys(&self) -> &web_sys::Node {
        panic!("`as_web_sys()` is not supported in SSR mode")
    }

    fn from_web_sys(_node: web_sys::Node) -> Self {
        panic!("`from_web_sys()` is not supported in SSR mode")
    }
}

/// A list of all the void HTML elements. We need this to know how to render them to a string.
static VOID_ELEMENTS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr", "command", "keygen", "menuitem",
    ]
    .into_iter()
    .collect()
});

/// Recursively render `node` by appending to `buf`.
pub(crate) fn render_recursive(node: &SsrNode, buf: &mut String) {
    match node {
        SsrNode::Element {
            tag,
            attributes,
            bool_attributes,
            children,
            inner_html,
            hk_key,
        } => {
            buf.push('<');
            buf.push_str(tag);
            for (name, value) in attributes {
                buf.push(' ');
                buf.push_str(name);
                buf.push_str("=\"");
                html_escape::encode_double_quoted_attribute_to_string(value, buf);
                buf.push('"');
            }
            for (name, value) in bool_attributes {
                if *value {
                    buf.push(' ');
                    buf.push_str(name);
                }
            }

            if let Some(hk_key) = hk_key {
                buf.push_str(" data-hk=\"");
                buf.push_str(&hk_key.to_string());
                buf.push('"');
            }
            buf.push('>');

            let is_void = VOID_ELEMENTS.contains(tag.as_ref());

            if is_void {
                assert!(
                    children.is_empty() && inner_html.is_none(),
                    "void elements cannot have children or inner_html"
                );
                return;
            }
            if let Some(inner_html) = inner_html {
                assert!(
                    children.is_empty(),
                    "inner_html and children are mutually exclusive"
                );
                buf.push_str(inner_html);
            } else {
                for child in children {
                    render_recursive(child, buf);
                }
            }

            if !is_void {
                buf.push_str("</");
                buf.push_str(tag);
                buf.push('>');
            }
        }
        SsrNode::TextDynamic { text } => {
            buf.push_str("<!--t-->"); // For dynamic text, add a marker for hydrating it.
            html_escape::encode_text_to_string(text.lock().unwrap().as_str(), buf);
            buf.push_str("<!-->"); // End of dynamic text.
        }
        SsrNode::TextStatic { text } => {
            html_escape::encode_text_to_string(text, buf);
        }
        SsrNode::Marker => {
            buf.push_str("<!--/-->");
        }
        SsrNode::Dynamic { view } => {
            render_recursive_view(&view.lock().unwrap(), buf);
        }
    }
}

/// Recursively render a [`View`] to a string by calling `render_recursive` on each node.
pub(crate) fn render_recursive_view(view: &View, buf: &mut String) {
    for node in &view.nodes {
        render_recursive(node, buf);
    }
}

#[cfg(test)]
mod tests {
    use expect_test::{expect, Expect};

    use super::*;
    use crate::tags::*;

    fn check<T: Into<View>>(view: impl FnOnce() -> T, expect: Expect) {
        let actual = render_to_string(move || view().into());
        expect.assert_eq(&actual);
    }

    #[test]
    fn hello_world() {
        check(move || "Hello, world!", expect!["Hello, world!"]);
    }

    #[test]
    fn render_escaped_text() {
        check(
            move || "<script>alert('xss')</script>",
            expect!["&lt;script&gt;alert('xss')&lt;/script&gt;"],
        );
    }

    #[test]
    fn render_inner_html() {
        check(
            move || div().dangerously_set_inner_html("<p>hello</p>"),
            expect![[r#"<div data-hk="0.0"><p>hello</p></div>"#]],
        );
    }

    #[test]
    fn render_void_element() {
        check(br, expect![[r#"<br data-hk="0.0">"#]]);
        check(
            move || input().value("value"),
            expect![[r#"<input value="value" data-hk="0.0">"#]],
        );
    }

    #[test]
    fn fragments() {
        check(
            move || (p().children("1"), p().children("2"), p().children("3")),
            expect![[r#"<p data-hk="0.0">1</p><p data-hk="0.1">2</p><p data-hk="0.2">3</p>"#]],
        );
    }

    #[test]
    fn indexed() {
        check(
            move || {
                sycamore_macro::view! {
                    ul {
                        Indexed(
                            list=vec![1, 2],
                            view=|i| sycamore_macro::view! { li { (i) } },
                        )
                    }
                }
            },
            expect![[
                r#"<ul data-hk="0.0"><!--/--><li data-hk="0.1">1</li><li data-hk="0.2">2</li><!--/--></ul>"#
            ]],
        );
    }

    #[test]
    fn bind() {
        // bind always attaches to a JS prop so it is not rendered in SSR.
        check(
            move || {
                let value = create_signal(String::new());
                sycamore_macro::view! {
                    input(bind:value=value)
                }
            },
            expect![[r#"<input data-hk="0.0">"#]],
        );
    }

    #[test]
    fn svg_element() {
        check(
            move || {
                sycamore_macro::view! {
                    svg(xmlns="http://www.w2.org/2000/svg") {
                        rect()
                    }
                }
            },
            expect![[
                r#"<svg xmlns="http://www.w2.org/2000/svg" data-hk="0.0"><rect data-hk="0.1"></rect></svg>"#
            ]],
        );
        check(
            move || {
                sycamore_macro::view! {
                    svg_a()
                }
            },
            expect![[r#"<a data-hk="0.0"></a>"#]],
        );
    }

    #[test]
    fn dynamic_text() {
        check(
            move || {
                let value = create_signal(0);
                let view = sycamore_macro::view! {
                    p { (value) }
                };
                value.set(1);
                view
            },
            expect![[r#"<p data-hk="0.0"><!--/-->1<!--/--></p>"#]],
        );
    }
}
