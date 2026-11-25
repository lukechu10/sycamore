use sycamore::prelude::*;

fn compile_pass() {
    let _ = create_root(|| {
        let _: View = view! {
            p {}
        };
        let _: View = view! {
            custom-element {}
        };

        let _: View = view! {
            p {}
        };
        let _: View = view! {
            custom-element {}
        };

        let _: View = view! {
            p(class="my-class")
        };
        let _: View = view! {
            p(class="my-class", id="my-id")
        };
        let _: View = view! {
            p("attr-42"="my-value")
        };

        let _: View = view! {
            button(class="my-btn", on_click=|_| {})
        };
        let _: View = view! {
            button(class="my-btn", aria-hidden="true")
        };

        let _: View = view! {
            p(dangerously_set_inner_html="<span>Test</span>")
        };

        let paragraph = |text: &'static str| p().children(text);
        let _: View = view! {
            paragraph(Some("initial text").unwrap(); class="my-class")
            paragraph("no attributes";)
        };

        let attributes = Attributes::default();
        let _: View = view! {
            p(..attributes)
        };

        // view! should correctly parenthesize the (1 + 2) when borrowing.
        let _: View = view! {
            p { (1 + 2) }
        };

        // view! should accept the pattern "-ref-" in an attribute name.
        let _: View = view! {
            p(class="my-class", data-ref-me="my-value")
        };
    });
}

#[derive(Default)]
struct DirectiveWidget(View);

impl DirectiveWidget {
    fn apply<T>(self, setter: impl FnOnce(Self, T) -> Self, value: T) -> Self {
        setter(self, value)
    }

    fn label(self, _label: &str) -> Self {
        self
    }

    fn content(self, children: impl Into<Children>) -> Self {
        Self(children.into().view)
    }

    fn build(self) -> View {
        self.0
    }
}

fn directives(label: Option<&str>) -> Option<View> {
    // `?` must return from this function, not from a generated closure.
    Some(view! {
        DirectiveWidget::default(apply:label = label?) {
            :apply:content { p { "Content" } }
        }
    })
}

fn main() {}
