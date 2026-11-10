use sycamore::prelude::*;

#[component]
fn MyComponent(value: Signal<i32>) -> View {
    view! {
        div(class="my-component") {
            "My component"
            p { "Value: " (value) }
        }
    }
}

#[component]
fn App() -> View {
    let mut state = create_signal(0);

    let increment = move |_| state += 1;

    view! {
        div {
            "Component demo"

            MyComponent(value=*state)
            MyComponent(value=*state)

            button(on_click=increment) { "+" }
        }
    }
}

fn main() {
    sycamore::render(App);
}
