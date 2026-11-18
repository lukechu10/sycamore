use expect_test::{Expect, expect};
use sycamore::web::Portal;
use sycamore::web::tags::*;

use super::*;

fn check(actual: impl Into<Children>, expect: &Expect) {
    expect.assert_eq(&sycamore::render_to_string(actual));
}

mod hello_world {
    use super::*;
    fn v() -> View {
        p().children("Hello World!").build()
    }
    static EXPECT: Expect = expect![[r#"<p data-hk="0.0">Hello World!</p>"#]];
    #[test]
    fn ssr() {
        check(v, &EXPECT);
    }
    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());

        sycamore::hydrate_to(v, &c);

        assert_text_content!(query("p"), "Hello World!");
    }
}

mod hydrate_recursive {
    use super::*;
    fn v() -> View {
        div().children(p().children("Nested")).build()
    }
    static EXPECT: Expect = expect![[r#"<div data-hk="0.0"><p data-hk="0.1">Nested</p></div>"#]];
    #[test]
    fn ssr() {
        check(v, &EXPECT);
    }
    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());

        sycamore::hydrate_to(v, &c);

        assert_text_content!(query("p"), "Nested");
    }
}

mod chained_children {
    use super::*;

    fn v() -> View {
        p().children("café").children("").children("!").build()
    }

    static EXPECT: Expect = expect![[r#"<p data-hk="0.0">café!</p>"#]];

    #[test]
    fn ssr() {
        check(v, &EXPECT);
    }

    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());
        let p = query("p");
        let text = p.first_child().unwrap();

        sycamore::hydrate_to(v, &c);

        assert_eq!(query("p"), p);
        assert_eq!(p.first_child(), Some(text));
        assert_text_content!(p, "café!");
    }
}

mod no_hydrate {
    use sycamore::web::{HydrationScript, NoHydrate};

    use super::*;

    fn v(count: MutSignal<i32>) -> View {
        view! {
            HydrationScript {}
            NoHydrate { p { "Server only" } }
            NoHydrate {}
            button(on_click=move |_| count.set(count.get_untracked() + 1)) { (count) }
        }
    }

    static EXPECT: Expect = expect![[
        r#"<!--opaque-range--><script></script><!--/opaque-range--><!--opaque-range--><p>Server only</p><!--/opaque-range--><!--opaque-range--><!--/opaque-range--><button data-hk="0.0"><!--[-->0<!--]--></button>"#
    ]];

    #[test]
    fn ssr() {
        check(|| v(create_signal(0)), &EXPECT);
    }

    #[wasm_bindgen_test]
    fn preserves_server_content_and_hydrates_the_next_sibling() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());
        let paragraph = query("p");
        let text = paragraph.first_child();
        let button = query("button");

        sycamore::hydrate_to(|| v(create_signal(0)), &c);

        assert_eq!(c.inner_html(), EXPECT.data());
        assert_eq!(query("button"), button);
        button.unchecked_ref::<HtmlElement>().click();
        assert_text_content!(button, "1");
        assert_eq!(query("p"), paragraph);
        assert_eq!(paragraph.first_child(), text);
    }

    #[wasm_bindgen_test]
    fn ignores_opaque_comments_when_matching_an_enclosing_dynamic_range() {
        let c = test_container();
        let html = "<!--[--><!--opaque-range--><!--]--><!--/opaque-range--><!--]-->";
        c.set_inner_html(html);

        sycamore::hydrate_to(|| View::from_dynamic(|| view! { NoHydrate {} }), &c);

        assert_eq!(c.inner_html(), html);
    }

    #[wasm_bindgen_test]
    fn renders_children_in_csr() {
        let c = test_container();
        sycamore::render_to(|| view! { NoHydrate { p { "Rendered in CSR" } } }, &c);
        assert_eq!(c.inner_html(), "<p>Rendered in CSR</p>");
    }
}

mod no_ssr {
    use sycamore::web::NoSsr;
    use sycamore::web::js_sys::Promise;
    use wasm_bindgen::JsValue;

    use super::*;

    fn v() -> View {
        let count = create_signal(0);
        view! {
            button(on_click=move |_| count.set(count.get_untracked() + 1)) { "Increment" }
            "before"
            NoSsr { (count) }
            "after"
        }
    }

    static EXPECT: Expect = expect![[
        r#"<button data-hk="0.0">Increment</button>before<no-ssr data-hk="0.1"></no-ssr>after"#
    ]];

    #[test]
    fn ssr() {
        check(v, &EXPECT);
    }

    async fn assert_dynamic_content_updates(c: &Element) {
        // Let NoSsr's on_mount callback run before inspecting its contents.
        Promise::resolve(&JsValue::UNDEFINED).await.unwrap();
        assert!(c.query_selector("no-ssr").unwrap().is_none());
        assert_text_content!(c, "Incrementbefore0after");

        query("button").unchecked_ref::<HtmlElement>().click();
        assert_text_content!(c, "Incrementbefore1after");
    }

    #[wasm_bindgen_test]
    async fn stays_reactive_after_hydration() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());
        let button = query("button");
        sycamore::hydrate_to(v, &c);

        assert_dynamic_content_updates(&c).await;
        assert_eq!(query("button"), button);
    }

    #[wasm_bindgen_test]
    async fn stays_reactive_in_csr() {
        let c = test_container();
        sycamore::render_to(v, &c);

        assert_dynamic_content_updates(&c).await;
    }
}

mod multiple_nodes_at_same_depth {
    use super::*;
    fn v() -> View {
        div()
            .children((
                p().id("first").children("First"),
                p().id("second").children("Second"),
            ))
            .build()
    }
    static EXPECT: Expect = expect![[
        r#"<div data-hk="0.0"><p id="first" data-hk="0.1">First</p><p id="second" data-hk="0.2">Second</p></div>"#
    ]];
    #[test]
    fn ssr() {
        check(v, &EXPECT);
    }
    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());

        sycamore::hydrate_to(v, &c);

        assert_text_content!(query("div>p#first"), "First");
        assert_text_content!(query("div>p#second"), "Second");
    }
}

mod top_level_fragment {
    use super::*;
    fn v() -> View {
        (
            p().id("first").children("First"),
            p().id("second").children("Second"),
        )
            .build()
    }
    static EXPECT: Expect = expect![[
        r#"<p id="first" data-hk="0.0">First</p><p id="second" data-hk="0.1">Second</p>"#
    ]];
    #[test]
    fn ssr() {
        check(v, &EXPECT);
    }
    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());

        sycamore::hydrate_to(v, &c);

        // Hydration should not change inner html.
        assert_text_content!(query("p#first"), "First");
        assert_text_content!(query("p#second"), "Second");
    }
}

mod dynamic {
    use super::*;
    fn v(state: Signal<i32>) -> View {
        p().children(move || state.get()).build()
    }
    static EXPECT: Expect = expect![[r#"<p data-hk="0.0"><!--[-->0<!--]--></p>"#]];
    #[test]
    fn ssr() {
        check(|| v(*create_signal(0)), &EXPECT);
    }
    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());

        let _ = create_root(|| {
            let state = create_signal(0);

            sycamore::hydrate_in_scope(move || v(*state), &c);

            assert_text_content!(query("p"), "0");

            // Reactivity should work normally.
            state.set(1);
            assert_text_content!(query("p"), "1");

            // P tag should still be the SSR-ed node, not a new node.
            assert_eq!(query("p").get_attribute("data-hk").as_deref(), Some("0.0"));
        });
    }
}

mod dynamic_with_siblings {
    use super::*;
    fn v(state: Signal<i32>) -> View {
        p().children(("Value: ", move || state.get(), "!")).build()
    }
    static EXPECT: Expect = expect![[r#"<p data-hk="0.0">Value: <!--[-->0<!--]-->!</p>"#]];
    #[test]
    fn ssr() {
        check(|| v(*create_signal(0)), &EXPECT);
    }
    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());

        let _ = create_root(|| {
            let state = create_signal(0);

            sycamore::hydrate_in_scope(move || v(*state), &c);

            // Reactivity should work normally.
            state.set(1);
            assert_text_content!(query("p"), "Value: 1!");

            // P tag should still be the SSR-ed node, not a new node.
            assert_eq!(query("p").get_attribute("data-hk").as_deref(), Some("0.0"));
        });
    }
}

mod top_level_dynamic_with_siblings {
    use super::*;
    fn v(state: Signal<i32>) -> View {
        ("Value: ", move || state.get(), "!").build()
    }
    static EXPECT: Expect = expect!["Value: <!--[-->0<!--]-->!"];
    #[test]
    fn ssr() {
        check(|| v(*create_signal(0)), &EXPECT);
    }
    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());

        let _ = create_root(|| {
            let state = create_signal(0);

            sycamore::hydrate_in_scope(move || v(*state), &c);

            // Reactivity should work normally.
            state.set(1);
            assert_text_content!(c, "Value: 1!");
        });
    }
}

mod nested_dynamic_ranges {
    use super::*;

    fn v(show_inner: Signal<bool>, state: Signal<i32>) -> View {
        (
            "before",
            View::from_dynamic(move || {
                if show_inner.get() {
                    View::from_dynamic(move || match state.get() {
                        0 => View::new(),
                        n => n.build(),
                    })
                } else {
                    span().children("new").build()
                }
            }),
            "after",
        )
            .build()
    }

    static EXPECT: Expect = expect!["before<!--[--><!--[--><!--]--><!--]-->after"];

    #[test]
    fn ssr() {
        check(|| v(*create_signal(true), *create_signal(0)), &EXPECT);
    }

    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());
        let before = c.first_child();
        let opening_anchor = before.as_ref().unwrap().next_sibling();
        let after = c.last_child();

        let _ = create_root(|| {
            let show_inner = create_signal(true);
            let state = create_signal(0);
            sycamore::hydrate_in_scope(move || v(*show_inner, *state), &c);

            assert_eq!(c.inner_html(), EXPECT.data());
            state.set(1);
            assert_text_content!(c, "before1after");

            // This branch creates a fresh element instead of claiming a server node.
            show_inner.set(false);
            state.set(2);
            assert_text_content!(c, "beforenewafter");
            show_inner.set(true);
            assert_text_content!(c, "before2after");

            assert_eq!(c.first_child(), before);
            assert_eq!(before.unwrap().next_sibling(), opening_anchor);
            assert_eq!(c.last_child(), after);
        });
    }
}

mod keyed_list {
    use super::*;
    fn v(state: Signal<Vec<i32>>) -> View {
        view! {
            ul {
                Keyed(
                    list=state,
                    view=|i| view! {
                        li { (i) }
                    },
                    key=|i| *i,
                )
            }
        }
    }
    static EXPECT: Expect = expect![[
        r#"<ul data-hk="0.0"><!--/--><li data-hk="0.1">0</li><li data-hk="0.2">1</li><li data-hk="0.3">2</li><!--/--></ul>"#
    ]];
    #[test]
    fn ssr() {
        check(|| v(*create_signal(vec![0, 1, 2])), &EXPECT);
    }
    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());

        let _ = create_root(|| {
            let state = create_signal(vec![0, 1, 2]);

            sycamore::hydrate_in_scope(move || v(*state), &c);

            // Reactivity should work normally.
            state.set(vec![2, 1, 0]);
            assert_text_content!(query("ul"), "210");
        });
    }
}

mod indexed_list {
    use super::*;
    fn v(state: Signal<Vec<i32>>) -> View {
        view! {
            ul {
                Indexed(
                    list=state,
                    view=|i| view! {
                        li { (i) }
                    },
                )
            }
        }
    }
    static EXPECT: Expect = expect![[
        r#"<ul data-hk="0.0"><!--/--><li data-hk="0.1">0</li><li data-hk="0.2">1</li><li data-hk="0.3">2</li><!--/--></ul>"#
    ]];
    #[test]
    fn ssr() {
        check(|| v(*create_signal(vec![0, 1, 2])), &EXPECT);
    }
    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());

        let _ = create_root(|| {
            let state = create_signal(vec![0, 1, 2]);

            sycamore::hydrate_in_scope(move || v(*state), &c);

            // Reactivity should work normally.
            state.set(vec![2, 1, 0]);
            assert_text_content!(query("ul"), "210");
        });
    }
}

mod portal {
    use super::*;
    fn v(state: Signal<bool>) -> View {
        view! {
            div(id="target")
            Portal(selector="#target") {
                if state.get() {
                    "Hello from the other side!"
                }
            }
        }
    }
    static EXPECT: Expect = expect![[r#"<div id="target" data-hk="0.0"></div>"#]];
    #[test]
    fn ssr() {
        check(|| v(*create_signal(true)), &EXPECT);
    }
    #[wasm_bindgen_test]
    fn test() {
        let c = test_container();
        c.set_inner_html(EXPECT.data());

        let _ = create_root(|| {
            let state = create_signal(true);

            sycamore::hydrate_in_scope(move || v(*state), &c);
        });
    }
}
