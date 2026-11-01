use sycamore::prelude::*;

fn compile_fail() {
    let _ = create_root(|| {
        let _: View = view! { button };
        let _: View = view! { button(disabled) };
        let _: View = view! { button(prop:disabled) };
        let _: View = view! {
            button(unknown:directive="123")
        };
        let _: View = view! {
            unknownelement {}
        };
        let _: View = view! {
            div(..unknown_attributes)
        };
        let _: View = view! { button(a.b.c="123") };
        let _: View = view! { * };

        let _: View = view! {
            p(dangerously_set_inner_html="<span>Test</span>") { "Error" }
        };
    });
}

fn main() {}
