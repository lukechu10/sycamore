use sycamore::prelude::*;

#[component]
fn Counter() -> View {
    let mut state = create_signal(0i32);
    let increment = move |_| state += 1;
    let decrement = move |_| state -= 1;
    let reset = move |_| state.set(0);
    view! {
        div {
            p { "Value: " (state) }
            button(on_click=increment) { "+" }
            button(on_click=decrement) { "-" }
            button(on_click=reset) { "Reset" }
        }
    }
}

#[component]
fn Hello() -> View {
    let name = create_signal(String::new());
    let is_empty = create_selector(move || !name.with(String::is_empty));

    view! {
        div {
            p {
                "Hello "
                if is_empty.get() {
                    span { (name) }
                } else {
                    span { "World" }
                }
                "!"
            }
            input(bind:value=name)
        }
    }
}

#[component]
fn App() -> View {
    view! {
        sycamore::web::HydrationScript {}
        p { "Hydration" }
        br {}
        Hello {}
        br {}
        Counter {}

        sycamore::web::NoHydrate { p { "This paragraph is not hydrated!" } }
        sycamore::web::NoSsr { p { "This paragraph is only rendered on the client side" } }
    }
}

fn main() {
    if is_not_ssr!() {
        console_error_panic_hook::set_once();
        sycamore::hydrate(App);
    } else {
        // Create index.html from template.html and insert the rendered HTML.
        let html = sycamore::render_to_string(App);
        let template =
            std::fs::read_to_string("template.html").expect("failed to read template.html");
        let rendered = template.replace("%sycamore.html%", &html);
        std::fs::write("index.html", rendered).expect("failed to write index.html");
        println!("Wrote index.html");
    }
}
