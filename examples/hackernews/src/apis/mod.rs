pub mod types;

use anyhow::Result;
use async_recursion::async_recursion;
use futures::future::join_all;
use gloo_net::http::Request;
use types::*;

/// Hacker News API url.
pub static BASE_API_URL: &str = "https://hacker-news.firebaseio.com/v0/";
pub static TOP_STORIES: &str = "topstories.json";
pub static NEW_STORIES: &str = "newstories.json";
pub static BEST_STORIES: &str = "beststories.json";
pub static SHOW_STORIES: &str = "showstories.json";
pub static ITEM_API: &str = "item/";
pub static USER_API: &str = "user/";

const STORIES_COUNT: usize = 30;
const COMMENT_DEPTH: i64 = 3;

pub async fn get_story_preview(id: i64) -> Result<StoryItem> {
    let url = format!("{}{}{}.json", BASE_API_URL, ITEM_API, id);
    Ok(Request::get(&url).send().await?.json().await?)
}

pub async fn get_stories(story_sorting: StorySorting) -> Result<Vec<StoryItem>> {
    let stories_api = match story_sorting {
        StorySorting::Top => TOP_STORIES,
        StorySorting::New => NEW_STORIES,
        StorySorting::Best => BEST_STORIES,
        StorySorting::Show => SHOW_STORIES,
    };
    let url = format!("{}{}", BASE_API_URL, stories_api);
    let stories_ids = Request::get(&url).send().await?.json::<Vec<i64>>().await?;

    let story_futures = stories_ids[..usize::min(stories_ids.len(), STORIES_COUNT)]
        .iter()
        .map(|&story_id| get_story_preview(story_id));
    let stories = join_all(story_futures)
        .await
        .into_iter()
        .filter_map(|story| story.ok())
        .collect();
    Ok(stories)
}

pub async fn get_story(id: i64) -> Result<StoryPageData> {
    let url = format!("{}{}{}.json", BASE_API_URL, ITEM_API, id);
    let mut story = Request::get(&url)
        .send()
        .await?
        .json::<StoryPageData>()
        .await?;
    let comment_futures = story.item.kids.iter().map(|&id| get_comment(id));
    let comments = join_all(comment_futures)
        .await
        .into_iter()
        .filter_map(|c| c.ok())
        .collect();

    story.comments = comments;
    Ok(story)
}

#[async_recursion(?Send)]
pub async fn get_comment_with_depth(id: i64, depth: i64) -> Result<Comment> {
    let url = format!("{}{}{}.json", BASE_API_URL, ITEM_API, id);
    let mut comment = Request::get(&url).send().await?.json::<Comment>().await?;
    if depth > 0 {
        let sub_comments_futures = comment
            .kids
            .iter()
            .map(|story_id| get_comment_with_depth(*story_id, depth - 1));
        comment.sub_comments = join_all(sub_comments_futures)
            .await
            .into_iter()
            .filter_map(|c| c.ok())
            .collect();
    }
    Ok(comment)
}

pub async fn get_comment(comment_id: i64) -> Result<Comment> {
    let comment = get_comment_with_depth(comment_id, COMMENT_DEPTH).await?;
    Ok(comment)
}

pub async fn get_user_page(user_id: &str) -> Result<UserData> {
    let url = format!("{}{}{}.json", BASE_API_URL, USER_API, user_id);
    let mut user = Request::get(&url).send().await?.json::<UserData>().await?;

    let first_story_ids = &user.submitted[..usize::min(user.submitted.len(), STORIES_COUNT)];
    let story_futures = first_story_ids
        .iter()
        .map(|&story_id| get_story_preview(story_id));

    user.stories = join_all(story_futures)
        .await
        .into_iter()
        .filter_map(|story| story.ok())
        .collect();

    Ok(user)
}
