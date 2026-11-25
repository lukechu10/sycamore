use sycamore::prelude::*;

#[component]
pub fn Component() -> View {
    view! {
        div {}
    }
}

#[component]
pub fn PropsComponent(prop: &'static str) -> View {
    let _ = prop;
    view! {
        div {}
    }
}

#[component]
pub fn AllDefaultPropsComponent(#[prop(default)] prop: u32) -> View {
    let _ = prop;
    view! {
        div {}
    }
}

#[component]
pub fn OptionalPropsComponent(optional: Option<u32>) -> View {
    let _ = optional;
    view! {
        div {}
    }
}

#[component]
pub fn ComponentWithChildren(children: Children) -> View {
    view! {
        div { (children.view) }
    }
}

#[component]
pub fn NestedComponentWithChildren(children: Children) -> View {
    view! {
        ComponentWithChildren {
            (children.view)
            Component {}
        }
    }
}

#[component]
fn Panel(children: Children, footer: Children) -> View {
    view! {
        section { (children.view) }
        div { (footer.view) }
    }
}

#[component(extend_element!(html:input))]
pub fn AttributesComponent(attributes: Attributes) -> View {
    view! {
        input(..attributes)
    }
}

fn compile_pass() {
    let _ = create_root(|| {
        let _: View = view! {
            Component {}
        };

        let prop = "prop";
        let _: View = view! {
            PropsComponent(prop=prop)
        };

        let _: View = view! {
            AllDefaultPropsComponent(prop=123)
        };
        let _: View = view! {
            AllDefaultPropsComponent {}
        };

        let _: View = view! {
            OptionalPropsComponent(optional=123)
        };
        let _: View = view! {
            OptionalPropsComponent {}
        };

        let _: View = view! {
            ComponentWithChildren { Component {} }
        };
        let _: View = view! {
            ComponentWithChildren { div {} }
        };
        let _: View = view! {
            ComponentWithChildren {
                div {}
                div {}
            }
        };
        let _: View = view! {
            ComponentWithChildren { Component {} }
        };
        let _: View = view! {
            ComponentWithChildren {}
        };

        let _: View = view! {
            Panel {
                p { "Before" }
                :footer { button { "Save" } button { "Cancel" } }
                p { "After" }
            }
            Panel { :footer {} }
        };

        let _: View = view! {
            AttributesComponent(class="test")
        };
        let str_signal = create_signal(String::new());
        let _: View = view! {
            AttributesComponent(bind_value=str_signal)
        };
        let on_click = |_| {};
        let _: View = view! {
            AttributesComponent(on_click=on_click)
        };
        let bool_signal = create_signal(false);
        let _: View = view! {
            AttributesComponent(disabled=false, checked=bool_signal.get())
        };
        // input specific attribute.
        let _: View = view! {
            AttributesComponent(value="text")
        };

        let _: View = view! {
            NestedComponentWithChildren { Component {} }
        };
    });
}

fn main() {}
