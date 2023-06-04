use std::sync::Arc;

use anyhow::Context;
use slack_morphism::{
    prelude::{
        SlackClientEventsUserState, SlackCommandEvent, SlackCommandEventResponse, SlackHyperClient,
    },
    SlackMessageContent,
};

use crate::query;

pub async fn command_event_handler(
    event: SlackCommandEvent,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> Result<SlackCommandEventResponse, Box<dyn std::error::Error + Send + Sync>> {
    let channel_id_command = event.channel_id.clone();

    let nitter_url_str = event.text.clone().context("No text")?;
    let nitter_url_str_validate = validate_url(&nitter_url_str)?;
    let nitter_url = url::Url::parse(&nitter_url_str_validate).context("Invalid input.")?;

    query::insert_feed_channel(&channel_id_command, &nitter_url).await?;

    Ok(SlackCommandEventResponse::new(
        SlackMessageContent::new().with_text("Working on it".into()),
    ))
}

fn validate_url(nitter_url: &str) -> anyhow::Result<String> {
    match nitter_url {
        s if s.ends_with("/rss/") => Ok(s.to_string()),
        s if s.ends_with("/rss") => Ok(format!("{s}/")),
        _ => Err(anyhow::anyhow!("invalid input")),
    }
}
