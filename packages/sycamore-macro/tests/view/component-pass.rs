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
pub fn OptionalPropsComponent(
    #[prop(default, setter(strip_option))] optional: Option<u32>,
    implicit: Option<u32>,
) -> View {
    let _ = optional;
    let _ = implicit;
    view! {
        div {}
    }
}

#[component]
pub fn ComponentWithChildren(children: Children) -> View {
    view! {
        div { (children) }
    }
}

#[component]
pub fn NestedComponentWithChildren(children: Children) -> View {
    view! {
        ComponentWithChildren {
            (children)
            Component {}
        }
    }
}

#[component]
pub async fn AsyncComponentWithPropsDestructuring(children: Children) -> View {
    children.call()
}

#[component]
pub fn AttributesComponent(#[prop(attributes(html, input))] attributes: Attributes) -> View {
    view! {
        input(..attributes)
    }
}

fn compile_pass() {
    let _ = create_root(|| {
        let _: View = view! {
            Component {}
        };
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
            AllDefaultPropsComponent {}
        };

        let _: View = view! {
            OptionalPropsComponent(optional=123)
        };
        let _: View = view! {
            OptionalPropsComponent(implicit=123)
        };
        let _: View = view! {
            OptionalPropsComponent(optional=123, implicit=123)
        };
        let _: View = view! {
            OptionalPropsComponent {}
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
            ComponentWithChildren { Component {} }
        };
        let _: View = view! {
            ComponentWithChildren {}
        };
        let _: View = view! {
            ComponentWithChildren {}
        };
        let _: View = view! {
            ComponentWithChildren {}
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
            AsyncComponentWithPropsDestructuring { Component {} }
        };

        let _: View = view! {
            NestedComponentWithChildren { Component {} }
        };
    });
}

fn main() {}
