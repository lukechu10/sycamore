use crate::*;

/// A trait that represents an attribute that can be set. This is not "attribute" in the HTML spec
/// sense. It can also represent JS properties (and possibly more ...) that can be set on an HTML
/// element.
pub trait AttributeValue: AttributeValueBoxed + 'static {
    fn set_self(self, el: &mut HtmlNode, name: Cow<'static, str>);
}

/// Type alias representing a possibly dynamic string value.
pub type StringAttribute = MaybeDyn<Option<Cow<'static, str>>>;
impl AttributeValue for StringAttribute {
    fn set_self(self, el: &mut HtmlNode, name: Cow<'static, str>) {
        el.set_attribute(name, self);
    }
}

/// Type alias representing a possibly dynamic boolean value.
pub type BoolAttribute = MaybeDyn<bool>;
impl AttributeValue for BoolAttribute {
    fn set_self(self, el: &mut HtmlNode, name: Cow<'static, str>) {
        el.set_bool_attribute(name, self);
    }
}

impl AttributeValue for MaybeDyn<JsValue> {
    fn set_self(self, el: &mut HtmlNode, name: Cow<'static, str>) {
        el.set_property(name, self);
    }
}

/// Trait used to implement `AttributeValue` for `Box<dyn AttributeValue>`.
#[doc(hidden)]
pub trait AttributeValueBoxed: 'static {
    fn set_self_boxed(self: Box<Self>, el: &mut HtmlNode, name: Cow<'static, str>);
}

impl<T> AttributeValueBoxed for T
where
    T: AttributeValue,
{
    fn set_self_boxed(self: Box<Self>, el: &mut HtmlNode, name: Cow<'static, str>) {
        self.set_self(el, name);
    }
}

impl AttributeValue for Box<dyn AttributeValue> {
    fn set_self(self, el: &mut HtmlNode, name: Cow<'static, str>) {
        self.set_self_boxed(el, name);
    }
}

/// Implemented for all types that can accept attributes ([`AttributeValue`]).
pub trait SetAttribute {
    fn set_attribute(&mut self, name: &'static str, value: impl AttributeValue);
    fn set_event_handler(
        &mut self,
        name: &'static str,
        value: impl FnMut(web_sys::Event) + 'static,
    );
}

impl<T> SetAttribute for T
where
    T: AsHtmlNode,
{
    fn set_attribute(&mut self, name: &'static str, value: impl AttributeValue) {
        value.set_self(self.as_html_node(), name.into());
    }

    fn set_event_handler(
        &mut self,
        name: &'static str,
        value: impl FnMut(web_sys::Event) + 'static,
    ) {
        self.as_html_node().set_event_handler(name.into(), value);
    }
}

/// A special prop type that can be used to spread attributes onto an element.
#[derive(Default)]
pub struct Attributes {
    values: Vec<(Cow<'static, str>, Box<dyn AttributeValue>)>,
    #[allow(clippy::type_complexity)]
    event_handlers: Vec<(Cow<'static, str>, Box<dyn FnMut(web_sys::Event)>)>,
}

impl SetAttribute for Attributes {
    fn set_attribute(&mut self, name: &'static str, value: impl AttributeValue) {
        self.values.push((name.into(), Box::new(value)));
    }

    fn set_event_handler(
        &mut self,
        name: &'static str,
        value: impl FnMut(web_sys::Event) + 'static,
    ) {
        self.event_handlers.push((name.into(), Box::new(value)));
    }
}

impl Attributes {
    /// Create a new empty [`Attributes`] instance.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_self(self, el: &mut HtmlNode) {
        for (name, value) in self.values {
            value.set_self(el, name);
        }
        for (name, handler) in self.event_handlers {
            el.set_event_handler(name, handler);
        }
    }
}

#[cfg(test)]
mod tests {
    use expect_test::{Expect, expect};

    use super::*;

    fn check<T: Into<View>>(view: impl FnOnce() -> T, expect: Expect) {
        let actual = render_to_string(move || view().into());
        expect.assert_eq(&actual);
    }

    #[test]
    fn attributes_apply_self() {
        let mut attributes = Attributes::new();
        attributes.set_attribute("class", StringAttribute::from("test-class"));
        attributes.set_attribute("id", StringAttribute::from(move || "test-id"));

        check(
            move || crate::tags::div().spread(attributes),
            expect![[r#"<div class="test-class" id="test-id" data-hk="0.0"></div>"#]],
        );
    }

    #[test]
    fn attributes_apply_self_macro() {
        let mut attributes = Attributes::new();
        attributes.set_attribute("class", StringAttribute::from("test-class"));
        attributes.set_attribute("id", StringAttribute::from(move || "test-id"));

        check(
            move || view! { div(..attributes) },
            expect![[r#"<div class="test-class" id="test-id" data-hk="0.0"></div>"#]],
        );
    }
}
