use std::{env, fmt::format, sync::Arc};

use anyhow::Context;
use dotenvy::dotenv;
use slack_morphism::{
    prelude::{
        SlackClientEventsUserState, SlackCommandEvent, SlackCommandEventResponse, SlackHyperClient,
    },
    SlackMessageContent,
};
use url::Url;

use crate::{query, utils};

pub async fn command_event_handler(
    event: SlackCommandEvent,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> Result<SlackCommandEventResponse, Box<dyn std::error::Error + Send + Sync>> {
    let channel_id_command = event.channel_id.clone();

    let text = event.text.clone().context("No text")?;
    let mut args = text.split_whitespace();
    let first_arg = args.next().context("No text")?;

    let content = match first_arg {
        "remove" => {
            let account = args.next().context("Invalid input")?;
            query::remove_rss(&channel_id_command, account).await?;

            SlackMessageContent::new().with_text(format!("@{account} の収集を停止します。"))
        }
        add => {
            let nitter_url_or_account = url::Url::parse(add).context("Invalid input.");

            let nitter_url = if let Ok(url) = nitter_url_or_account {
                url
            } else {
                account_to_default_nitter_rss_url(add)?
            };

            query::insert_last_item(&nitter_url).await?;
            query::insert_feed_channel(&channel_id_command, &nitter_url).await?;

            let account = utils::url_to_account(&nitter_url)?;

            SlackMessageContent::new().with_text(format!("@{account} の収集を開始します。"))
        }
    };

    Ok(SlackCommandEventResponse::new(content))
}

fn account_to_default_nitter_rss_url(account: &str) -> anyhow::Result<Url> {
    dotenv().ok();
    let default_url_str = env::var("DEFAULT_NITTER_URL")?;
    let default_url = url::Url::parse(&default_url_str)?;

    let add_seg = format!("{account}/rss");
    let nitter_rss_url = default_url.join(&add_seg)?;

    Ok(nitter_rss_url)
}
