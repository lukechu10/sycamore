use sycamore::prelude::*;
use sycamore::web::{StringAttribute, extend_element};

#[component(extend_element!(html:button))]
fn CustomButton(
    #[prop(into)] id: StringAttribute,
    #[prop(default)] attributes: Attributes,
    children: Children,
) -> View {
    console_log!("Intercepted `id` attribute: {:?}", id.get_clone());

    view! {
        button(id=id, ..attributes) { (children) }
    }
}

#[component]
fn App() -> View {
    view! {
        div {
            p { "Passthrough attributes demo" }

            div {
                CustomButton(
                    id="button1",
                    r#type="button",
                    on_click=|_| console_log!("Button 1 clicked!"),
                ) { "Button 1" }
                CustomButton(
                    id="button2",
                    r#type="button",
                    class="red-button",
                    style="background-color:red;",
                    prop:disabled=true,
                    on_click=|_| console_log!("Button 2 clicked!"),
                ) { "Button 2" }
            }
        }
    }
}

fn main() {
    sycamore::render(App);
}
