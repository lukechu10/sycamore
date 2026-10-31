use sycamore::prelude::*;

#[component]
fn App() -> View {
    let name = create_signal(String::new());

    let handle_change = move |_| {};

    view! {
        div {
            h1 {
                "Hello "
                if !name.with(String::is_empty) {
                    span { (name) }
                } else {
                    span { "World" }
                }
                "!"
            }

            input(placeholder="What is your name?", on_input=handle_change)
        }
    }
}

fn main() {
    let s = sycamore::render_to_string(App);
    println!("{s}");
}
