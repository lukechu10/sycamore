use sycamore::prelude::*;

#[component]
fn Counter() -> View {
    let counter = use_context::<Signal<i32>>();

    view! {
        p(class="value") { "Value: " (counter) }
    }
}

#[component]
pub fn Controls() -> View {
    let mut state = use_context::<Signal<i32>>();
    let increment = move |_| state += 1;
    let decrement = move |_| state -= 1;
    let reset = move |_| state.set(0);

    view! {
        button(on_click=decrement) { "-" }
        button(on_click=increment) { "+" }
        button(on_click=reset) { "Reset" }
    }
}

#[component]
fn App() -> View {
    let counter = create_signal(0i32);
    provide_context(counter);

    view! {
        div {
            "Context demo"
            Counter {}
            Controls {}
        }
    }
}

fn main() {
    sycamore::render(App);
}
