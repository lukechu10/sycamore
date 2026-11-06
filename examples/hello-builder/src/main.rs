//! Look ma, No `view!`!
//!
//! This example demonstrates the basics of the builder API for constructing views, as an
//! alternative to using the `view!` macro.

use sycamore::prelude::*;
use sycamore::web::tags::*;

#[component]
fn App() -> View {
    let name = create_signal(String::new());
    div()
        .children((
            h1().children((
                "Hello ",
                move || {
                    if !name.with(String::is_empty) {
                        span().children(name)
                    } else {
                        span().children("World")
                    }
                },
                "!",
            )),
            input().bind_value(name),
        ))
        .into()
}

fn main() {
    console_error_panic_hook::set_once();
    sycamore::render(App);
}
