use sycamore::prelude::*;
use sycamore::web::Suspense;

async fn sleep_ms(ms: u64) {
    is_ssr! {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }
    is_not_ssr! {
        let _ = ms;
    }
}

#[component]
async fn Delayed(delay_ms: u64, children: Children) -> View {
    sleep_ms(delay_ms).await;
    view! {
        (children)
    }
}

#[component]
fn DelayedText(delay_ms: u64) -> View {
    view! {
        Delayed(delay_ms=delay_ms) { p { "Loaded after " (delay_ms) "ms" } }
    }
}

#[component]
fn CounterButton() -> View {
    let mut state = create_signal(0);

    view! {
        button(r#type="button", on_click=move |_| state += 1) { "Click me: " (state) }
    }
}

#[component]
fn App() -> View {
    let delays = [1000, 2000, 1000];
    view! {
        p { strong { "SSR Streaming Demo" } }
        Indexed(
            list=delays.to_vec(),
            view=|delay_ms| view! {
                Suspense(
                    fallback=|| view! {
                        p { "Loading..." }
                    },
                ) { DelayedText(delay_ms=delay_ms) }
            },
        )
        p { strong { "A lot of suspense" } }
        p {
            "This loading bar is made up of 100 `<div>` elements, each streamed over from the server in real time"
        }
        div {
            style {
                "span { width: 1%; height: 10px; background-color: red; display: inline-block; }"
            }
            Indexed(
                list=(0..100).collect::<Vec<_>>(),
                view=|x| view! {
                    Suspense { Delayed(delay_ms=x * 5) { span {} } }
                },
            )
        }
        p { strong { "Nested Suspense" } }
        Suspense(
            fallback=|| view! {
                p { "Loading outer..." }
                p { "..." }
            },
        ) {
            DelayedText(delay_ms=1000)
            Suspense(
                fallback=|| view! {
                    p { "Loading inner..." }
                },
            ) { DelayedText(delay_ms=2000) }
        }
        p { strong { "Nested Suspense with inner finishing first" } }
        Suspense(
            fallback=|| view! {
                p { "Loading outer..." }
                p { "..." }
            },
        ) {
            DelayedText(delay_ms=2000)
            Suspense(
                fallback=|| view! {
                    p { "Loading inner..." }
                },
            ) { DelayedText(delay_ms=1000) }
        }

        p { strong { "Suspense content is hydrated" } }
        Suspense(
            fallback=|| view! {
                "Loading interactive content..."
            },
        ) { Delayed(delay_ms=1000) { CounterButton {} } }
    }
}

#[component]
pub fn Main() -> View {
    view! {
        html {
            head {
                sycamore::web::HydrationScript {}
                sycamore::web::NoHydrate {
                    link(
                        rel="preload",
                        href="/dist/ssr-streaming_bg.wasm",
                        r#as="fetch",
                        crossorigin="",
                    )
                    script(r#type="module") {
                        "import init from '/dist/ssr-streaming.js'; const wasm = await init({});"
                    }
                }
            }
            body { App {} }
        }
    }
}
