use expect_test::{expect, Expect};
use sycamore::web::tags::*;
use sycamore::web::Portal2;

use super::*;

fn check(actual: impl FnOnce() -> View, expect: &Expect) {
    expect.assert_eq(&sycamore::render_to_string(actual));
}

mod hello_world {
    use super::*;
    fn v() -> View {
        p().children("Hello World!").into()
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
        div().children(p().children("Nested")).into()
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

mod multiple_nodes_at_same_depth {
    use super::*;
    fn v() -> View {
        div()
            .children((
                p().id("first").children("First"),
                p().id("second").children("Second"),
            ))
            .into()
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
            .into()
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
    fn v(state: ReadSignal<i32>) -> View {
        p().children(move || state.get()).into()
    }
    static EXPECT: Expect = expect![[r#"<p data-hk="0.0"><!--/-->0<!--/--></p>"#]];
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

            sycamore::hydrate_in_scope(|| v(*state), &c);

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
    fn v(state: ReadSignal<i32>) -> View {
        p().children(("Value: ", move || state.get(), "!")).into()
    }
    static EXPECT: Expect = expect![[r#"<p data-hk="0.0">Value: <!--/-->0<!--/-->!</p>"#]];
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

            sycamore::hydrate_in_scope(|| v(*state), &c);

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
    fn v(state: ReadSignal<i32>) -> View {
        ("Value: ", move || state.get(), "!").into()
    }
    static EXPECT: Expect = expect!["Value: <!--/-->0<!--/-->!"];
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

            sycamore::hydrate_in_scope(|| v(*state), &c);

            // Reactivity should work normally.
            state.set(1);
            assert_text_content!(c, "Value: 1!");
        });
    }
}

mod keyed_list {
    use super::*;
    fn v(state: ReadSignal<Vec<i32>>) -> View {
        view! {
            ul {
                Keyed(
                    list=state,
                    view=|i| view! { li { (i) } },
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

            sycamore::hydrate_in_scope(|| v(*state), &c);

            // Reactivity should work normally.
            state.set(vec![2, 1, 0]);
            assert_text_content!(query("ul"), "210");
        });
    }
}

mod indexed_list {
    use super::*;
    fn v(state: ReadSignal<Vec<i32>>) -> View {
        view! {
            ul {
                Indexed(
                    list=state,
                    view=|i| view! { li { (i) } },
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

            sycamore::hydrate_in_scope(|| v(*state), &c);

            // Reactivity should work normally.
            state.set(vec![2, 1, 0]);
            assert_text_content!(query("ul"), "210");
        });
    }
}

mod portal {
    use super::*;
    fn v(state: ReadSignal<bool>) -> View {
        view! {
            div(id="target")
            Portal2(selector="#target") {
                (if state.get() {
                    view! { "Hello from the other side!" }
                } else {
                    view! { }
                })
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

            sycamore::hydrate_in_scope(|| v(*state), &c);
        });
    }
}
