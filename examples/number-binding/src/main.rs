use sycamore::prelude::*;

#[component]
fn App() -> View {
    let value = create_signal(10.0);

    view! {
        p { (format!("{:.2}", value.get())) }

        input(r#type="range", min="1", step="0.25", max="10", bind_value_as_number=value)
        br {}
        input(r#type="number", min="1", step="0.25", max="10", bind_value_as_number=value)
    }
}

fn main() {
    sycamore::render(App);
}
