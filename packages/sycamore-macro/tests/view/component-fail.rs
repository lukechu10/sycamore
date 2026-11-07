use sycamore::prelude::*;

#[component]
pub fn PropsComponent(prop: &'static str) -> View {
    let _ = prop;
    view! {
        div {}
    }
}

#[component]
fn Component() -> View {
    view! {
        div {}
    }
}

#[component(extend_element!(html:div))]
pub fn AttributesComponent(attributes: Attributes) -> View {
    view! {
        div(..attributes)
    }
}

fn compile_fail() {
    let _ = create_root(|| {
        let _: View = view! {
            UnknownComponent {}
        };
        let _: View = view! {
            UnknownComponent {}
        };

        let _: View = view! { Component };
        let _: View = view! {
            Component(not_a_prop=1)
        };

        let _: View = view! {
            PropsComponent {}
        };
        let _: View = view! {
            PropsComponent {}
        };
        let _: View = view! {
            PropsComponent(prop=123)
        };
        let _: View = view! { PropsComponent { prop: "123" } }; // Legacy syntax.

        let _: View = view! {
            AttributesComponent(class=123)
        }; // Wrong type
    });
}

fn main() {}
