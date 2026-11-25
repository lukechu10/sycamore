//! SSR tests. These live in an integration test (rather than inside `sycamore-web`) so that they
//! only ever see a single copy of `sycamore-web`. See the `sycamore-web` dev-dependency cycle.
#![cfg(not(target_arch = "wasm32"))]

mod node {
    use expect_test::{Expect, expect};
    use sycamore::prelude::*;
    use sycamore::web::{LazyChildren, NoHydrate, render_to_string};

    fn check(view: impl Into<LazyChildren>, expect: Expect) {
        let actual = render_to_string(view);
        expect.assert_eq(&actual);
    }

    #[test]
    fn hello_world() {
        check(move || "Hello, world!", expect!["Hello, world!"]);
    }

    #[test]
    fn render_escaped_text() {
        check(
            move || "<script>alert('xss')</script>",
            expect!["&lt;script&gt;alert('xss')&lt;/script&gt;"],
        );
    }

    #[test]
    fn render_inner_html() {
        check(
            move || div().dangerously_set_inner_html("<p>hello</p>"),
            expect![[r#"<div data-hk="0.0"><p>hello</p></div>"#]],
        );
    }

    #[test]
    fn no_hydrate_omits_keys_for_the_entire_subtree() {
        check(
            || {
                sycamore_macro::view! {
                    NoHydrate {
                        (move || view! { div { p { "Static" } } })
                    }
                    p { "Hydratable" }
                }
            },
            expect![[
                r#"<!--opaque-range--><div><p>Static</p></div><!--/opaque-range--><p data-hk="0.0">Hydratable</p>"#
            ]],
        );
    }

    #[test]
    fn render_void_element() {
        check(br, expect![[r#"<br data-hk="0.0">"#]]);
        check(
            move || input().value("value"),
            expect![[r#"<input value="value" data-hk="0.0">"#]],
        );
    }

    #[test]
    fn fragments() {
        check(
            move || (p().children("1"), p().children("2"), p().children("3")),
            expect![[r#"<p data-hk="0.0">1</p><p data-hk="0.1">2</p><p data-hk="0.2">3</p>"#]],
        );
    }

    #[test]
    fn indexed() {
        check(
            move || {
                sycamore_macro::view! {
                    ul {
                        Indexed(
                            list=vec![1, 2],
                            view=|i| sycamore_macro::view! {
                                li { (i) }
                            },
                        )
                    }
                }
            },
            expect![[
                r#"<ul data-hk="0.0"><!--[--><!--[--><li data-hk="0.1">1</li><!--]--><!--[--><li data-hk="0.2">2</li><!--]--><!--]--></ul>"#
            ]],
        );
    }

    #[test]
    fn bind() {
        // bind always attaches to a JS prop so it is not rendered in SSR.
        check(
            move || {
                let value = create_signal(String::new());
                sycamore_macro::view! {
                    input(bind_value=value)
                }
            },
            expect![[r#"<input data-hk="0.0">"#]],
        );
    }

    #[test]
    fn svg_element() {
        check(
            move || {
                sycamore_macro::view! {
                    svg(xmlns="http://www.w2.org/2000/svg") { rect {} }
                }
            },
            expect![[
                r#"<svg xmlns="http://www.w2.org/2000/svg" data-hk="0.0"><rect data-hk="0.1"></rect></svg>"#
            ]],
        );
        check(
            move || {
                sycamore_macro::view! {
                    svg_a {}
                }
            },
            expect![[r#"<a data-hk="0.0"></a>"#]],
        );
    }

    #[test]
    fn signal_view_build_can_create_signals() {
        #[derive(Clone)]
        struct ReactiveText(u32);

        impl Build<View> for ReactiveText {
            fn build(self) -> View {
                View::from(create_signal(self.0).get_untracked())
            }
        }

        check(
            || {
                let value = create_signal(ReactiveText(0));
                let view = View::from(*value);
                value.set(ReactiveText(1));
                view
            },
            expect!["<!--[-->1<!--]-->"],
        );
    }

    #[test]
    fn dynamic_text() {
        check(
            move || {
                let value = create_signal(0);
                let view = sycamore_macro::view! {
                    p { (value) }
                };
                value.set(1);
                view
            },
            expect![[r#"<p data-hk="0.0"><!--[-->1<!--]--></p>"#]],
        );
    }
}

#[cfg(feature = "suspense")]
mod render {
    use expect_test::expect;
    use futures::channel::oneshot;
    use sycamore::prelude::*;
    use sycamore::web::{
        Suspense, render_to_string, render_to_string_await_suspense, render_to_string_stream,
    };

    #[component]
    async fn AsyncComponent(receiver: oneshot::Receiver<()>) -> View {
        receiver.await.unwrap();
        view! {
            p { "Hello, async!" }
        }
    }

    #[component]
    fn App(receiver: oneshot::Receiver<()>) -> View {
        view! {
            Suspense(fallback=|| "fallback") { (move || view! { AsyncComponent(receiver=receiver) }) }
        }
    }

    #[test]
    fn render_to_string_renders_fallback() {
        let (sender, receiver) = oneshot::channel();
        let res = render_to_string(move || {
            view! {
                App(receiver=receiver)
            }
        });
        assert_eq!(res, "<!--[--><!--[-->fallback<!--]--><!--]-->");
        assert!(sender.send(()).is_err(), "receiver should be dropped");
    }

    #[tokio::test]
    async fn render_to_string_await_suspense_works() {
        let (sender, receiver) = oneshot::channel();
        let ssr = render_to_string_await_suspense(move || {
            view! {
                App(receiver=receiver)
            }
        });
        futures::pin_mut!(ssr);
        assert!(futures::poll!(&mut ssr).is_pending());

        sender.send(()).unwrap();
        let res = ssr.await;

        let expect = expect![[
            r#"<!--[--><!--[--><!--[--><!--async:1--><p data-hk="1.0">Hello, async!</p><!--]--><!--]--><!--]-->"#
        ]];
        expect.assert_eq(&res);
    }

    #[tokio::test]
    async fn blocking_requests_have_independent_scopes() {
        let (first_tx, first_rx) = oneshot::channel();
        let (second_tx, second_rx) = oneshot::channel();
        let first = render_to_string_await_suspense(move || view! { App(receiver=first_rx) });
        let second = render_to_string_await_suspense(move || view! { App(receiver=second_rx) });
        futures::pin_mut!(first, second);
        assert!(futures::poll!(&mut first).is_pending());
        assert!(futures::poll!(&mut second).is_pending());

        second_tx.send(()).unwrap();
        assert!(second.await.contains("Hello, async!"));
        assert!(futures::poll!(&mut first).is_pending());
        first_tx.send(()).unwrap();
        assert!(first.await.contains("Hello, async!"));
    }

    #[tokio::test]
    async fn dropping_stream_cancels_pending_suspense() {
        use futures::StreamExt;

        tokio::task::LocalSet::new()
            .run_until(async {
                for poll_shell in [false, true] {
                    let (sender, receiver) = oneshot::channel();
                    let mut stream = Box::pin(render_to_string_stream(move || {
                        view! { App(receiver=receiver) }
                    }));
                    if poll_shell {
                        assert!(stream.next().await.unwrap().contains("fallback"));
                        tokio::task::yield_now().await;
                        assert!(!sender.is_canceled());
                    }
                    drop(stream);

                    for _ in 0..8 {
                        if sender.is_canceled() {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                    assert!(
                        sender.is_canceled(),
                        "a pending async component survived disconnect"
                    );
                }
            })
            .await;
    }

    #[tokio::test]
    async fn streaming_preserves_outer_anchors_and_serializes_before_disposal() {
        use futures::StreamExt;

        tokio::task::LocalSet::new().run_until(async {
            let (sender, receiver) = oneshot::channel();
            let ssr = render_to_string_stream(move || view! { App(receiver=receiver) });
            futures::pin_mut!(ssr);

            let shell = ssr.next().await.unwrap();
            assert!(shell.contains(r#"<!--[--><suspense-start data-key="1"></suspense-start>fallback<suspense-end data-key="1"></suspense-end><!--]-->"#));
            sender.send(()).unwrap();

            let fragment = ssr.next().await.unwrap();
            assert_eq!(
                fragment,
                r#"<template id="sycamore-suspense-1"><!--[--><!--[--><!--async:2--><p data-hk="2.0">Hello, async!</p><!--]--><!--]--></template><script>__sycamore_suspense(1)</script>"#,
            );
            assert_eq!(ssr.next().await.unwrap(), "<script>__sycamore_finish_stream()</script>");
            assert!(ssr.next().await.is_none());
        }).await;
    }
}
