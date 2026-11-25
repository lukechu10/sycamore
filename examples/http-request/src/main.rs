use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use sycamore::prelude::*;
use sycamore::web::Suspense;

// API that counts visits to the web-page
const API_BASE_URL: &str = "https://abacus.jasoncameron.dev/hit";

#[derive(Serialize, Deserialize, Default, Debug)]
struct Visits {
    value: u64,
}

async fn fetch_visits(id: &str) -> Result<Visits, gloo_net::Error> {
    let url = format!("{API_BASE_URL}/{id}/http-request");
    let resp = Request::get(&url).send().await?;

    resp.json::<Visits>().await
}

#[component]
async fn VisitsCount() -> View {
    let id = "sycamore-visits-counter";
    let visits = fetch_visits(id).await.unwrap_or_default();

    view! {
        p {
            "Total visits: "
            span { (visits.value) }
        }
    }
}

#[component]
fn App() -> View {
    view! {
        div {
            p { "Page Visit Counter" }
            Suspense(
                fallback=|| {
                    view! {
                        "Loading..."
                    }
                },
            ) { (VisitsCount) }
        }
    }
}

fn main() {
    console_error_panic_hook::set_once();
    sycamore::render(App);
}
