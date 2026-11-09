pub mod cleanup;
pub mod hydrate;
pub mod indexed;
pub mod keyed;
pub mod portal;
pub mod render;
pub mod svg;

mod utils;

use sycamore::prelude::*;
use utils::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element, Event, HtmlElement, HtmlInputElement};

wasm_bindgen_test_configure!(run_in_browser);

/// Returns a [`Element`] referencing the test container with the contents cleared.
fn test_container() -> Element {
    if document()
        .query_selector("test-container#test-container")
        .unwrap()
        .is_none()
    {
        document()
            .body()
            .unwrap()
            .insert_adjacent_html(
                "beforeend",
                r#"<test-container id="test-container"></test-container>"#,
            )
            .unwrap();
    }

    let container = query("test-container#test-container");

    container.set_inner_html(""); // erase contents from previous test runs

    container
}

#[wasm_bindgen_test]
fn empty_template() {
    sycamore::render_to(View::new, &test_container());
    assert_eq!(query("#test-container").inner_html(), "");
}

#[wasm_bindgen_test]
fn hello_world() {
    sycamore::render_to(
        || {
            view! {
                p { "Hello World!" }
            }
        },
        &test_container(),
    );
    assert_eq!(query("p").outer_html(), "<p>Hello World!</p>");
}

#[wasm_bindgen_test]
fn hello_world_noderef() {
    let _ = create_root(|| {
        let p_ref = NodeRef::new();

        sycamore::render_in_scope(
            move || {
                view! {
                    p(r#ref=p_ref) { "Hello World!" }
                }
            },
            &test_container(),
        );

        assert_eq!(
            &p_ref.get().unchecked_into::<HtmlElement>().outer_html(),
            "<p>Hello World!</p>"
        );
    });
}

#[wasm_bindgen_test]
fn interpolation() {
    let text = "Hello Sycamore!";
    sycamore::render_to(
        move || {
            view! {
                p { (text) }
            }
        },
        &test_container(),
    );

    assert_text_content!(query("p"), "Hello Sycamore!");
}

#[wasm_bindgen_test]
fn template_interpolation() {
    sycamore::render_to(
        || {
            let text = view! {
                "Hello Sycamore!"
            };
            view! {
                p { (text) }
            }
        },
        &test_container(),
    );
    assert_text_content!(query("p"), "Hello Sycamore!");
}

#[wasm_bindgen_test]
fn template_interpolation_if_else() {
    let _ = create_root(|| {
        let show = create_signal(true);
        let node = view! {
            p {
                if show.get() {
                    "Hello Sycamore!"
                } else {
                    "Hidden"
                }
            }
        };
        sycamore::render_in_scope(|| node, &test_container());
        assert_text_content!(query("p"), "Hello Sycamore!");

        show.set(false);
        assert_text_content!(query("p"), "Hidden");

        show.set(true);
        assert_text_content!(query("p"), "Hello Sycamore!");
    });
}

#[wasm_bindgen_test]
fn template_interpolation_if_else_with_sibling() {
    let _ = create_root(|| {
        let show = create_signal(true);
        let node = view! {
            div { "Before" }
            if show.get() {
                p { "Hello Sycamore!" }
            } else {
                p { "Hidden" }
            }
        };
        sycamore::render_in_scope(|| node, &test_container());
        assert_text_content!(query("p"), "Hello Sycamore!");

        show.set(false);
        assert_text_content!(query("p"), "Hidden");

        show.set(true);
        assert_text_content!(query("p"), "Hello Sycamore!");
    });
}

#[wasm_bindgen_test]
fn template_interpolation_nested_reactivity() {
    let _ = create_root(|| {
        let count = create_signal(0);
        let text = view! {
            p { (count.get()) }
        };
        let node = view! {
            p { (text) }
        };

        sycamore::render_in_scope(|| node, &test_container());
        let p = query("p");
        assert_text_content!(p, "0");

        count.set(1);
        assert_text_content!(p, "1");
    });
}

#[wasm_bindgen_test]
fn reactive_text() {
    let _ = create_root(|| {
        let count = create_signal(0);

        let node = view! {
            p { (count.get()) }
        };

        sycamore::render_in_scope(|| node, &test_container());
        let p = query("p");

        assert_text_content!(p, "0");

        count.set(1);
        assert_text_content!(p, "1");
    });
}

#[wasm_bindgen_test]
fn reactive_text_do_not_destroy_previous_children() {
    let _ = create_root(|| {
        let count = create_signal(0);

        let node = view! {
            p { "Value: " (count.get()) }
        };

        sycamore::render_in_scope(|| node, &test_container());
        let p = query("p");

        assert_text_content!(p, "Value: 0");

        count.set(1);
        assert_text_content!(p, "Value: 1");
    });
}

#[wasm_bindgen_test]
fn reactive_attribute() {
    let _ = create_root(|| {
        let count = create_signal(0);

        let node = view! {
            span(data-attribute=move || count.get().to_string())
        };

        sycamore::render_in_scope(|| node, &test_container());
        let span = query("span");

        assert_eq!(span.get_attribute("data-attribute").unwrap(), "0");

        count.set(1);
        assert_eq!(span.get_attribute("data-attribute").unwrap(), "1");
    });
}

// Expressions are evaluated once unless explicitly wrapped in a closure.
#[wasm_bindgen_test]
fn attribute_expressions_are_not_implicitly_reactive() {
    let _ = create_root(|| {
        let count = create_signal(0);
        let checked = create_signal(true);
        let node = view! {
            input(
                value=count.get().to_string(),
                data-count=count.get().to_string(),
                "quoted-count"=count.get().to_string(),
                prop:checked=checked.get(),
            )
        };
        sycamore::render_in_scope(|| node, &test_container());
        let input: HtmlInputElement = query_into("input");

        count.set(1);
        checked.set(false);
        assert_eq!(input.get_attribute("value").unwrap(), "0");
        assert_eq!(input.get_attribute("data-count").unwrap(), "0");
        assert_eq!(input.get_attribute("quoted-count").unwrap(), "0");
        assert!(input.checked());
    });
}

#[wasm_bindgen_test]
fn explicit_reactive_attribute_expressions() {
    let _ = create_root(|| {
        let count = create_signal(0);
        let node = view! {
            input(
                value=move || count.get().to_string(),
                "quoted-count"=move || count.get().to_string(),
            )
        };
        sycamore::render_in_scope(|| node, &test_container());
        let input = query("input");
        assert_eq!(input.get_attribute("value").unwrap(), "0");
        assert_eq!(input.get_attribute("quoted-count").unwrap(), "0");

        count.set(1);
        assert_eq!(input.get_attribute("value").unwrap(), "1");
        assert_eq!(input.get_attribute("quoted-count").unwrap(), "1");
    });
}

#[wasm_bindgen_test]
fn component_passthrough_attributes_require_explicit_closures() {
    #[component(extend_element!(html:input))]
    fn Input(attributes: Attributes) -> View {
        view! { input(..attributes) }
    }

    let _ = create_root(|| {
        let count = create_signal(0);
        let checked = create_signal(true);
        let node = view! {
            Input(
                id="static-input",
                data-count=count.get().to_string(),
                "quoted-count"=count.get().to_string(),
                prop:checked=checked.get(),
            )
            Input(
                id="reactive-input",
                data-count=move || count.get().to_string(),
                "quoted-count"=move || count.get().to_string(),
                prop:checked=move || checked.get(),
            )
        };
        sycamore::render_in_scope(|| node, &test_container());
        let static_input: HtmlInputElement = query_into("#static-input");
        let reactive_input: HtmlInputElement = query_into("#reactive-input");
        assert_eq!(reactive_input.get_attribute("data-count").unwrap(), "0");
        assert!(reactive_input.checked());

        count.set(1);
        checked.set(false);
        assert_eq!(static_input.get_attribute("data-count").unwrap(), "0");
        assert_eq!(static_input.get_attribute("quoted-count").unwrap(), "0");
        assert!(static_input.checked());
        assert_eq!(reactive_input.get_attribute("data-count").unwrap(), "1");
        assert_eq!(reactive_input.get_attribute("quoted-count").unwrap(), "1");
        assert!(!reactive_input.checked());
    });
}

#[wasm_bindgen_test]
fn optional_attribute() {
    let _ = create_root(|| {
        let attr = create_signal(None);

        sycamore::render_in_scope(
            move || {
                view! {
                    span(data-attribute=attr)
                }
            },
            &test_container(),
        );

        let span = query("span");
        assert_eq!(span.outer_html(), "<span></span>");

        attr.set(Some("value"));
        assert_eq!(span.outer_html(), "<span data-attribute=\"value\"></span>");
    });
}

#[wasm_bindgen_test]
fn reactive_property() {
    let _ = create_root(|| {
        let indeterminate = create_signal(true);

        let node = view! {
            input(r#type="checkbox", prop:indeterminate=move || indeterminate.get())
        };

        sycamore::render_in_scope(|| node, &test_container());
        let input: HtmlInputElement = query_into("input");

        assert!(input.indeterminate());

        indeterminate.set(false);
        assert!(!input.indeterminate());
    });
}

#[wasm_bindgen_test]
fn static_property() {
    let _ = create_root(|| {
        let node = view! {
            input(prop:checked=true)
        };

        sycamore::render_in_scope(|| node, &test_container());
        let input: HtmlInputElement = query_into("input");

        assert!(input.checked());
    });
}

#[wasm_bindgen_test]
fn two_way_bind_to_props() {
    let _ = create_root(|| {
        let value = create_signal(String::new());

        let node = view! {
            input(bind_value=value)
            p { (value.get_clone()) }
        };

        sycamore::render_in_scope(|| node, &test_container());
        let input: HtmlInputElement = query_into("input");

        value.set("abc".to_string());
        assert_eq!(input.value(), "abc");

        input.set_value("def");
        input.dispatch_event(&Event::new("input").unwrap()).unwrap();
        assert_eq!(value.get_clone().as_str(), "def");
    });
}

#[wasm_bindgen_test]
fn two_way_bind_to_value_as_number() {
    let _ = create_root(|| {
        let value = create_signal(1.0);

        let node = view! {
            input(r#type="range", bind_value_as_number=value) // Note that type must be "range" or "number"
        };

        sycamore::render_in_scope(|| node, &test_container());
        let input: HtmlInputElement = query_into("input");
        assert_eq!(input.value_as_number(), 1.0);

        value.set(2.0);
        assert_eq!(input.value_as_number(), 2.0);

        input.set_value_as_number(3.0);
        input.dispatch_event(&Event::new("input").unwrap()).unwrap();
        assert_eq!(value.get(), 3.0);
    });
}

#[wasm_bindgen_test]
fn noderefs() {
    let _ = create_root(|| {
        let noderef = create_node_ref();
        let node = view! {
            div { input(r#ref=noderef) }
        };

        sycamore::render_in_scope(|| node, &test_container());
        let input_ref = query("input");

        assert_eq!(input_ref, noderef.get().unchecked_into());
    });
}

#[wasm_bindgen_test]
fn noderef_reactivity_test() {
    let _ = create_root(|| {
        let counter = create_signal(0);
        let node_ref: NodeRef = create_node_ref();

        let _ = view! {
            div(r#ref=node_ref)
        };

        create_effect(move || {
            node_ref.get();
            counter.set(counter.get() + 1);
        });
        assert_eq!(counter.get(), 1);

        let _ = view! {
            div(r#ref=node_ref)
        };

        assert_eq!(counter.get(), 2);
    });
}

#[wasm_bindgen_test]
fn fragments() {
    let _ = create_root(|| {
        let node = view! {
            p { "1" }
            p { "2" }
            p { "3" }
        };
        sycamore::render_in_scope(|| node, &test_container());

        assert_text_content!(query("#test-container"), "123");
    });
}

#[wasm_bindgen_test]
fn fragments_text_nodes() {
    let _ = create_root(|| {
        let node = view! {
            "1"
            "2"
            "3"
        };

        sycamore::render_in_scope(|| node, &test_container());

        assert_text_content!(query("#test-container"), "123");
    });
}
