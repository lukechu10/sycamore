use sycamore::web::Portal;
use sycamore::web::js_sys::Promise;
use wasm_bindgen::JsValue;

use super::*;

#[wasm_bindgen_test]
async fn updates_dynamic_content_and_removes_only_the_portal_range() {
    let c = test_container();
    sycamore::render_to(
        || {
            let count = create_signal(0);
            let show = create_signal(true);
            view! {
                div(id="portal-target") { "before" }
                button(id="increment", on_click=move |_| count.set(count.get_untracked() + 1)) { "Increment" }
                button(id="close", on_click=move |_| show.set(false)) { "Close" }
                if show.get() {
                    Portal(selector="#portal-target") { (count) }
                }
            }
        },
        &c,
    );

    Promise::resolve(&JsValue::UNDEFINED).await.unwrap();
    let target = query("#portal-target");
    assert_eq!(
        target.inner_html(),
        "before<!--[--><!--[-->0<!--]--><!--]-->"
    );

    let before = target.first_child().unwrap();
    let after = document().create_text_node("after").into();
    target.append_child(&after).unwrap();
    query("#increment").unchecked_ref::<HtmlElement>().click();
    assert_text_content!(target, "before1after");

    query("#close").unchecked_ref::<HtmlElement>().click();
    assert_eq!(target.inner_html(), "beforeafter");
    assert_eq!(target.first_child(), Some(before));
    assert_eq!(target.last_child(), Some(after));
}
