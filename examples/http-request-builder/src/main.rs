use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use sycamore::prelude::*;
use sycamore::web::Suspense;
use sycamore::web::tags::*;

// API that counts visits to the web-page
const API_BASE_URL: &str = "https://abacus.jasoncameron.dev/hit";

#[derive(Serialize, Deserialize, Default, Debug)]
struct Visits {
    value: u64,
}

async fn fetch_visits(id: &str) -> Result<Visits, gloo_net::Error> {
    let url = format!("{API_BASE_URL}/{id}/http-request-builder");
    let resp = Request::get(&url).send().await?;

    resp.json::<Visits>().await
}

#[component]
async fn VisitsCount() -> View {
    let id = "sycamore-builder-visits-counter";
    let visits = fetch_visits(id).await.unwrap_or_default();

    p().children(("Total Visits: ", span().children(move || visits.value)))
        .build()
}

#[component]
fn App() -> View {
    div()
        .children((
            p().children("Page Visit Counter"),
            Suspense().fallback(|| "Loading").children(VisitsCount),
        ))
        .build()
}

fn main() {
    console_error_panic_hook::set_once();
    sycamore::render(App);
}
