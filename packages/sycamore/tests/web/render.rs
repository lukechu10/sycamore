use super::*;

#[wasm_bindgen_test]
fn interpolated_text() {
    let _ = create_root(|| {
        let name = create_signal("world");
        let value = create_signal(1.0);
        let width = 6;
        let node = view! {
            p { "hello {name} {value:>width$.1} / {value.get():.1}" }
        };
        sycamore::render_in_scope(|| node, &test_container());
        assert_text_content!(query("p"), "hello world    1.0 / 1.0");

        name.set("Sycamore");
        value.set(12.5);
        assert_text_content!(query("p"), "hello Sycamore   12.5 / 12.5");
    });
}

#[wasm_bindgen_test]
fn dyn_view_static() {
    let _ = create_root(|| {
        let node: View = View::from_dynamic(move || {
            view! {
                div { "Test" }
            }
        });

        sycamore::render_in_scope(|| node, &test_container());
        assert_text_content!(query("div"), "Test");
    });
}

#[wasm_bindgen_test]
fn dyn_fragment() {
    let _ = create_root(|| {
        let num = create_signal(0);

        let node = view! {
            "before"
            p { (num.get()) }
            "after"
        };

        sycamore::render_in_scope(|| node, &test_container());
        let test_container = query("test-container");

        assert_text_content!(test_container, "before0after");

        num.set(1);

        assert_text_content!(test_container, "before1after");
    });
}

#[wasm_bindgen_test]
fn dyn_nested() {
    let _ = create_root(|| {
        let node: View = View::from_dynamic(move || {
            View::from_dynamic(move || {
                view! {
                    div { "Test" }
                }
            })
        });

        sycamore::render_in_scope(|| node, &test_container());
        assert_text_content!(query("div"), "Test");
    });
}

#[wasm_bindgen_test]
fn dyn_scoped_nested() {
    let _ = create_root(|| {
        let num = create_signal(0);

        let node: View = View::from_dynamic(move || {
            View::from_dynamic(move || {
                view! {
                    div { (num.get()) }
                }
            })
        });

        sycamore::render_in_scope(|| node, &test_container());
        assert_text_content!(query("div"), "0");
        num.set(1);
        assert_text_content!(query("div"), "1");
    });
}

#[wasm_bindgen_test]
fn regression_572() {
    let _ = create_root(|| {
        let signal = create_signal(0);

        sycamore::render_in_scope(
            move || {
                View::from_dynamic(move || {
                    View::from_dynamic(move || {
                        signal.track();
                        View::new()
                    })
                })
            },
            &test_container(),
        );
        signal.set(0);
    });
}
