use std::time::Duration;

use sycamore::easing;
use sycamore::motion::create_tweened_signal;
use sycamore::prelude::*;

#[component]
fn App() -> View {
    let rotate = create_tweened_signal(0f64, Duration::from_millis(800), easing::quad_inout);

    view! {
        button(disabled=rotate.is_tweening(), on_click=move |_| rotate.set(rotate.get() + 0.5)) {
            "Half rotate..."
        }
        button(disabled=rotate.is_tweening(), on_click=move |_| rotate.set(rotate.get() + 1.0)) {
            "Rotate!"
        }
        button(disabled=rotate.is_tweening(), on_click=move |_| rotate.set(rotate.get() + 2.0)) {
            "Rotate twice!!"
        }
        button(disabled=rotate.is_tweening(), on_click=move |_| rotate.set(rotate.get() + 3.0)) {
            "Rotate thrice!!!"
        }
        svg(height="500", width="500", xmlns="http://www.w3.org/2000/svg") {
            rect(
                x="100",
                y="100",
                width="100",
                height="100",
                fill="red",
                transform=format!("rotate({}, 150, 150)", (rotate.get() * 360.0) as u32),
            )
        }
    }
}

fn main() {
    sycamore::render(App);
}
