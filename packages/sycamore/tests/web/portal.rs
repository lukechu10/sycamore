use sycamore::web::Portal;

use super::*;

#[wasm_bindgen_test]
fn test_portal() {
    let test_container = test_container();

    let portal_target = document().create_element("div").unwrap();
    portal_target.set_id("portal-target");
    test_container.append_child(&portal_target).unwrap();

    let root = document().create_element("div").unwrap();
    test_container.append_child(&root).unwrap();

    let _ = create_root(|| {
        let switch = create_signal(true);
        sycamore::render_in_scope(
            move || {
                view! {
                    if switch.get() {
                        Portal(selector="#portal-target") { "Hello from the other side!" }
                    }
                }
            },
            &root,
        );
    });
}
