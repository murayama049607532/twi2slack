use core::fmt;
use std::sync::Arc;

use anyhow::Context;
use futures::{future, sink::Feed, StreamExt, TryStreamExt};
use rss::{Channel, Item};
use slack_morphism::{
    prelude::{SlackApiChatPostMessageRequest, SlackClientEventsUserState, SlackHyperClient},
    SlackApiToken, SlackApiTokenType, SlackChannelId, SlackMessageContent,
};
use sqlx::FromRow;
use tokio::time::Duration;
use url::Url;

use crate::{
    query::{self, fetch_rss_urls},
    utils,
};

const SLEEP_EACH_FETCH_MINUTES: u64 = 3;
const SLEEP_ALL_FETCH_MINUTES: u64 = 10;

pub async fn feed_loop(client: Arc<SlackHyperClient>) -> anyhow::Result<()> {
    loop {
        let rss_urls = fetch_rss_urls().await?;
        let rss_urls_stream = futures::stream::iter(rss_urls);

        let last_tweets = rss_urls_stream
            .map(|url| async move {
                let twi_ch = fetch_twi_url(&url).await?;
                tokio::time::sleep(Duration::from_secs(SLEEP_EACH_FETCH_MINUTES * 60)).await;
                anyhow::Ok(twi_ch)
            })
            .then(|s| s)
            .try_collect::<Vec<_>>()
            .await?;

        let last_tweets_stream = futures::stream::iter(last_tweets);
        last_tweets_stream
            .map(|(url, channels)| async {
                let cli = Arc::clone(&client);
                async move { send_message_channels(channels, url.as_str(), cli).await }.await
            })
            .then(|s| s)
            .try_collect::<()>()
            .await?;

        let mut interval = tokio::time::interval(Duration::from_secs(SLEEP_ALL_FETCH_MINUTES * 60));
        interval.tick().await;
    }
}

// RSSが更新されていれば、Ok(tweet_url, Vec<channel>), いなければErr
async fn fetch_twi_url(nitter_rss_url: &Url) -> anyhow::Result<(Url, Vec<SlackChannelId>)> {
    let raw_rss = reqwest::get(nitter_rss_url.clone()).await?.bytes().await?;

    let mut channel = Channel::read_from(&raw_rss[..])?;
    let rss::Item {
        link: Some(last_nitweet_url_str),
        pub_date: Some(pub_date),
        ..
    } = channel.items.swap_remove(0) else{return Err(anyhow::anyhow!("Invalid item"));};

    let last_date = query::fetch_last_date(nitter_rss_url).await?;
    if pub_date == last_date {
        return Err(anyhow::anyhow!("No update"));
    };

    let last_nitweet_url = Url::parse(&last_nitweet_url_str)?;
    let last_tweet_url = nitter_url_to_twi(&last_nitweet_url)?;

    let feed_channels = query::fetch_channels(nitter_rss_url).await?;

    Ok((last_tweet_url, feed_channels))
}

fn nitter_url_to_twi(nitter_url: &Url) -> anyhow::Result<Url> {
    let twitter_host = Url::parse("https://twitter.com/")?;
    let tweet_path = nitter_url.path();

    let tweet_url = twitter_host.join(tweet_path)?;

    Ok(tweet_url)
}

async fn send_message_channels(
    channels: Vec<SlackChannelId>,
    text: &str,
    client: Arc<SlackHyperClient>,
) -> anyhow::Result<()> {
    let token = utils::get_token(&SlackApiTokenType::Bot)?;
    let channel_stream = futures::stream::iter(channels);
    channel_stream
        .map(|channel| async { send_message(channel, text, Arc::clone(&client), &token).await })
        .then(|s| s)
        .try_collect::<()>()
        .await?;
    Ok(())
}

async fn send_message(
    channel: SlackChannelId,
    text: &str,
    client: Arc<SlackHyperClient>,
    token: &SlackApiToken,
) -> anyhow::Result<()> {
    let session = client.open_session(token);
    let content = SlackMessageContent::new().with_text(text.to_string());
    let req = SlackApiChatPostMessageRequest::new(channel, content);
    let _message_res = session
        .chat_post_message(&req)
        .await
        .context("failed to post message.")?;

    Ok(())
}
