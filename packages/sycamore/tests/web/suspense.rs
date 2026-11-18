use sycamore::web::js_sys::{Function, Object, Promise, Reflect};
use sycamore::web::{Suspense, wrap_async_component};
use wasm_bindgen::JsValue;

use super::*;

fn gate() -> (Promise, Function) {
    let mut resolve = None;
    let promise = Promise::new(&mut |ready, _| resolve = Some(ready));
    (promise, resolve.unwrap())
}

fn resolve(gate: Function) {
    gate.call0(&JsValue::UNDEFINED).unwrap();
}

async fn flush_tasks() {
    for _ in 0..4 {
        Promise::resolve(&JsValue::UNDEFINED).await.unwrap();
    }
}

fn delayed_text(gate: Promise) -> View {
    wrap_async_component(move || async move {
        gate.await.unwrap();
        "resolved".build()
    })
}

#[wasm_bindgen_test]
async fn csr_reveals_content_when_ready() {
    let c = test_container();
    let (pending, ready) = gate();
    sycamore::render_to(
        move || view! { Suspense(fallback=|| "fallback", children=Children::new(move || delayed_text(pending))) },
        &c,
    );
    assert_text_content!(c, "fallback");

    resolve(ready);
    flush_tasks().await;
    assert_text_content!(c, "resolved");
}

#[wasm_bindgen_test]
fn retains_mounted_content_while_a_new_task_is_pending() {
    use std::cell::Cell;
    use std::rc::Rc;

    use sycamore::futures::SuspenseTaskGuard;

    let c = test_container();
    let content_scope = Rc::new(Cell::new(None));
    let mut value = None;
    let _root = create_root(|| {
        let value_signal = create_signal(0);
        value = Some(value_signal);
        let content_scope = Rc::clone(&content_scope);
        sycamore::render_in_scope(
            move || {
                view! {
                    Suspense(fallback=|| "fallback", children=Children::new(move || {
                        content_scope.set(Some(use_current_scope()));
                        view! { button { "stable" } (value_signal) }
                    }))
                }
            },
            &c,
        );
    });
    let button = query("button");
    assert_text_content!(c, "stable0");

    let scope = content_scope.get().unwrap();
    scope.run_in(|| {
        let pending = SuspenseTaskGuard::new();
        assert_text_content!(c, "fallback");
        value.unwrap().set(1);
        drop(pending);
    });

    assert_text_content!(c, "stable1");
    assert_eq!(query("button"), button);
}

#[wasm_bindgen_test]
async fn sync_hydration_adopts_fallback_then_reveals_content() {
    let c = test_container();
    c.set_inner_html("<!--[--><!--[--><p data-hk=\"0.0\">fallback</p><!--]--><!--]-->");
    let fallback = query("p");
    let (pending, ready) = gate();
    sycamore::hydrate_to(
        move || {
            view! {
                Suspense(fallback=|| view! { p { "fallback" } }, children=Children::new(move || delayed_text(pending)))
            }
        },
        &c,
    );
    assert_eq!(query("p"), fallback);

    resolve(ready);
    flush_tasks().await;
    assert_text_content!(c, "resolved");
}

fn interactive(gate: Promise) -> View {
    wrap_async_component(move || async move {
        gate.await.unwrap();
        let count = create_signal(0);
        view! {
            button(on_click=move |_| count.set(count.get_untracked() + 1)) { (count) }
        }
    })
}

#[wasm_bindgen_test]
async fn blocking_hydration_preserves_content_and_resolves_in_any_order() {
    let c = test_container();
    c.set_inner_html(concat!(
        "<!--[--><!--[-->",
        "<!--[--><!--async:1--><button data-hk=\"1.0\"><!--[-->0<!--]--></button><!--]-->",
        "<!--[--><!--async:2--><button data-hk=\"2.0\"><!--[-->0<!--]--></button><!--]-->",
        "<!--]--><!--]-->",
    ));
    let first = query("button:first-of-type");
    let second = query("button:last-of-type");
    let (pending_first, ready_first) = gate();
    let (pending_second, ready_second) = gate();
    let window = window();
    let property = "__sycamore_ssr_mode".into();
    Reflect::set(&window, &property, &"blocking".into()).unwrap();
    sycamore::hydrate_to(
        move || {
            view! {
                Suspense(
                    fallback=|| "fallback",
                    children=Children::new(move || View::from((interactive(pending_first), interactive(pending_second)))),
                )
            }
        },
        &c,
    );
    Reflect::delete_property(window.unchecked_ref::<Object>(), &property).unwrap();
    assert_text_content!(c, "00");

    resolve(ready_second);
    flush_tasks().await;
    second.unchecked_ref::<HtmlElement>().click();
    assert_text_content!(c, "01");
    assert_eq!(query("button:last-of-type"), second);

    resolve(ready_first);
    flush_tasks().await;
    first.unchecked_ref::<HtmlElement>().click();
    assert_text_content!(c, "11");
    assert_eq!(query("button:first-of-type"), first);
}

#[wasm_bindgen_test]
async fn deferred_mount_callbacks_wait_for_adoption_and_cancel_on_dispose() {
    use std::cell::Cell;
    use std::rc::Rc;

    for cancel in [false, true] {
        let c = test_container();
        c.set_inner_html("<!--[--><!--async:1--><button data-hk=\"1.0\">server</button><!--]-->");
        let server_node = query("button");
        let (pending, ready) = gate();
        let started = Rc::new(Cell::new(false));
        let mounted = Rc::new(Cell::new(false));
        let clicked = Rc::new(Cell::new(false));
        let window = window();
        let property = "__sycamore_ssr_mode".into();
        Reflect::set(&window, &property, &"blocking".into()).unwrap();
        let root = create_root(|| {
            let started = Rc::clone(&started);
            let mounted = Rc::clone(&mounted);
            let clicked = Rc::clone(&clicked);
            sycamore::hydrate_in_scope(
                move || {
                    wrap_async_component(move || async move {
                        let view =
                            view! { button(on_click=move |_| clicked.set(true)) { "server" } };
                        on_mount(move || mounted.set(true));
                        started.set(true);
                        pending.await.unwrap();
                        view
                    })
                },
                &c,
            );
        });
        Reflect::delete_property(window.unchecked_ref::<Object>(), &property).unwrap();

        flush_tasks().await;
        assert!(started.get());
        assert!(!mounted.get());
        server_node.unchecked_ref::<HtmlElement>().click();
        assert!(!clicked.get());
        if cancel {
            root.dispose();
        }
        resolve(ready);
        flush_tasks().await;
        assert_eq!(mounted.get(), !cancel);
        assert_eq!(query("button"), server_node);
        server_node.unchecked_ref::<HtmlElement>().click();
        assert_eq!(clicked.get(), !cancel);
        if !cancel {
            root.dispose();
        }
    }
}

#[wasm_bindgen_test]
async fn nested_stream_fragments_patch_and_hydrate_in_place() {
    fn script(parent: &Element, source: &str) {
        let script = document().create_element("script").unwrap();
        script.set_text_content(Some(source));
        parent.append_child(&script).unwrap();
    }

    let c = test_container();
    c.set_inner_html(
        r#"<!--[--><suspense-start data-key="1"></suspense-start>outer fallback<suspense-end data-key="1"></suspense-end><!--]-->"#,
    );
    script(
        &c,
        include_str!("../../../sycamore-web/src/node/suspense_stream.js"),
    );
    // Deliver the nested fragment first; it must wait for its parent's markers.
    c.insert_adjacent_html(
        "beforeend",
        r#"<template id="sycamore-suspense-2"><!--[--><!--[--><!--async:3--><button data-hk="3.0"><!--[-->0<!--]--></button><!--]--><!--]--></template>"#,
    ).unwrap();
    script(&c, "__sycamore_suspense(2)");
    assert_text_content!(c, "outer fallback");

    c.insert_adjacent_html(
        "beforeend",
        r#"<template id="sycamore-suspense-1"><!--[--><!--[--><suspense-start data-key="2"></suspense-start>inner fallback<suspense-end data-key="2"></suspense-end><!--]--><!--]--></template>"#,
    ).unwrap();
    script(&c, "__sycamore_suspense(1)");
    script(&c, "__sycamore_finish_stream()");
    let button = query("button");
    assert_text_content!(c, "0");

    let (pending, ready) = gate();
    let window = window();
    let property = "__sycamore_ssr_mode".into();
    Reflect::set(&window, &property, &"streaming".into()).unwrap();
    sycamore::hydrate_to(
        move || {
            view! {
                Suspense(fallback=|| "outer fallback") {
                    Suspense(fallback=|| "inner fallback", children=Children::new(move || interactive(pending)))
                }
            }
        },
        &c,
    );
    let global = window.unchecked_ref::<Object>();
    Reflect::delete_property(global, &property).unwrap();
    Reflect::delete_property(global, &"__sycamore_stream_complete".into()).unwrap();

    resolve(ready);
    flush_tasks().await;
    button.unchecked_ref::<HtmlElement>().click();
    assert_text_content!(c, "1");
    assert_eq!(query("button"), button);
}
