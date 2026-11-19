use sycamore::prelude::*;

use crate::apis::types::StorySorting;
use crate::components::show_stories::ShowStories;

#[component]
pub async fn Stories(sorting: StorySorting) -> View {
    let data = crate::apis::get_stories(sorting).await;
    match data {
        Ok(stories) => view! {
            ShowStories(stories=stories)
        },
        Err(_) => view! {
            "Error fetching stories."
        },
    }
}
