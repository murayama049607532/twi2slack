use std::sync::Arc;

use anyhow::Context;
use futures::{StreamExt, TryStreamExt};
use rss::{Channel, Item};
use slack_morphism::{
    prelude::{SlackApiChatPostMessageRequest, SlackHyperClient},
    SlackApiToken, SlackApiTokenType, SlackChannelId, SlackMessageContent,
};

use tokio::time::Duration;
use url::Url;

use crate::{
    query::{self, fetch_rss_urls},
    utils,
};

const SLEEP_EACH_FETCH_MINUTES: u64 = 10;
const SLEEP_ALL_FETCH_MINUTES: u64 = 15;

pub async fn feed_loop(client: Arc<SlackHyperClient>) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(SLEEP_ALL_FETCH_MINUTES * 60));

    loop {
        let rss_urls = fetch_rss_urls().await.unwrap_or_default();

        let rss_urls_stream = futures::stream::iter(rss_urls);

        let last_tweets = rss_urls_stream
            .filter_map(|url| async move {
                tokio::time::sleep(Duration::from_secs(SLEEP_EACH_FETCH_MINUTES * 60)).await;
                fetch_twi_url(&url).await.ok()
            })
            .collect::<Vec<_>>()
            .await;

        let last_tweets_stream = futures::stream::iter(last_tweets);
        last_tweets_stream
            .filter_map(|(url, channels)| async {
                let cli = Arc::clone(&client);
                async move { send_message_channels(channels, &url, cli).await.ok() }.await
            })
            .collect::<()>()
            .await;

        interval.tick().await;
    }
}

async fn fetch_twi_url(nitter_rss_url: &Url) -> anyhow::Result<(Vec<Url>, Vec<SlackChannelId>)> {
    let raw_rss = reqwest::get(nitter_rss_url.clone()).await?;

    let rss_bytes = raw_rss.bytes().await?;

    let channel = Channel::read_from(&rss_bytes[..])?;
    let items = channel.items().to_vec();

    let last_date_rss = last_update(&items)?;

    let last_date_db = query::fetch_last_date(nitter_rss_url).await?;
    if last_date_db == last_date_rss {
        println!("no update");
        return Err(anyhow::anyhow!("No update"));
    };

    let updated_tweets = updated_tweets(items, &last_date_db);
    query::update_last_date(nitter_rss_url, &last_date_rss).await?;

    let feed_channels = query::fetch_channels(nitter_rss_url).await?;

    Ok((updated_tweets, feed_channels))
}

fn updated_tweets(items: Vec<Item>, last_date: &str) -> std::vec::Vec<url::Url> {
    items
        .into_iter()
        .take(3)
        .take_while(|Item { pub_date, .. }| {
            pub_date.as_ref().map(std::string::String::as_str) != Some(last_date)
        })
        .filter_map(|Item { link, .. }| {
            link.and_then(|s| Url::parse(&s).ok())
                .and_then(|url| utils::nitter_url_to_twi(&url).ok())
        })
        .collect::<Vec<_>>()
}
fn last_update(items: &[Item]) -> anyhow::Result<String> {
    let last_date = items
        .get(0)
        .context("no item")?
        .pub_date()
        .context("no pub date")?
        .to_string();
    Ok(last_date)
}
async fn send_message_channels(
    channels: Vec<SlackChannelId>,
    urls: &[Url],

    client: Arc<SlackHyperClient>,
) -> anyhow::Result<()> {
    let token = utils::get_token(&SlackApiTokenType::Bot)?;
    let channel_stream = futures::stream::iter(channels);
    channel_stream
        .map(|channel| async { send_tweets(channel, urls, Arc::clone(&client), &token).await })
        .then(|s| s)
        .try_collect::<()>()
        .await?;
    Ok(())
}

async fn send_tweets(
    channel: SlackChannelId,
    urls: &[Url],
    client: Arc<SlackHyperClient>,
    token: &SlackApiToken,
) -> anyhow::Result<()> {
    let tweets = urls
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    let reqs = tweets
        .into_iter()
        .map(|twi| SlackMessageContent::new().with_text(twi))
        .map(|content| {
            SlackApiChatPostMessageRequest::new(channel.clone(), content)
                .with_username("Twitter".to_string())
        })
        .collect::<Vec<_>>();
    let mut req_stream = futures::stream::iter(reqs);

    let session = client.open_session(token);

    while let Some(req) = req_stream.next().await {
        let _message_res = session
            .chat_post_message(&req)
            .await
            .context("failed to post message.")?;
    }
    Ok(())
}
