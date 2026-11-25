use sycamore::prelude::*;

#[component]
async fn AsyncContent() -> View {
    // Simulate reading some data from a server.
    is_ssr! {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    view! {
        p { "Suspensed component" }
        sycamore::web::NoHydrate {
            (move || {
                view! {
                    p { "Server only content" }
                }
            })
        }
    }
}

#[component]
fn App() -> View {
    view! {
        html {
            head {
                meta(charset="utf-8")
                meta(name="viewport", content="width=device-width, initial-scale=1")
                sycamore::web::HydrationScript {}
                sycamore::web::CargoSycamoreMeta {}
            }
            body { sycamore::web::Suspense { (AsyncContent) } }
        }
    }
}

#[cfg_ssr]
#[tokio::main]
async fn main() {
    // Create index.html from template.html and insert the rendered HTML.
    let html = sycamore::render_to_string_await_suspense(App).await;
    std::fs::write("index.html", format!("<!DOCTYPE html>{html}"))
        .expect("failed to write index.html");
    println!("Wrote index.html");
}

#[cfg_not_ssr]
fn main() {
    console_error_panic_hook::set_once();

    let document = document();
    sycamore::hydrate_to(App, &document);
}
