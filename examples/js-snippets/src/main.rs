use sycamore::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(module = "/js/index.js")]
extern "C" {
    fn multiply(a: f64, b: f64) -> f64;
}

#[component]
fn App() -> View {
    let a = create_signal(1_f64);
    let b = create_signal(1_f64);
    let product = create_signal(1_f64);

    create_effect(move || {
        product.set(multiply(a.get(), b.get()));
    });

    view! {
        input(r#type="number", bind_value_as_number=a)
        span { "*" }
        input(r#type="number", bind_value_as_number=b)
        span { "=" }
        span { (product) }
    }
}

fn main() {
    sycamore::render(App);
}
