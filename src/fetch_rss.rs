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
    query::{self, fetch_nitters, fetch_rss_urls},
    utils,
};

const SLEEP_EACH_FETCH_MINUTES: u64 = 5;
const SLEEP_ALL_FETCH_MINUTES: u64 = 20;

pub async fn feed_loop(client: Arc<SlackHyperClient>) -> anyhow::Result<()> {
    let nitters = fetch_nitters().await?;
    for nitter in nitters {
        tokio::spawn(feed_loop_nitter(Arc::clone(&client), nitter));
    }

    Ok(())
}
pub async fn feed_loop_nitter(client: Arc<SlackHyperClient>, nitter: String) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(SLEEP_ALL_FETCH_MINUTES * 60));

    loop {
        let rss_urls = fetch_rss_urls(&nitter).await.unwrap_or_default();

        let rss_urls_stream = futures::stream::iter(rss_urls);

        rss_urls_stream
            .filter_map(|url| async {
                let cli = Arc::clone(&client);
                async move { feed_send(cli, &url).await.ok() }.await
            })
            .collect::<()>()
            .await;

        interval.tick().await;
    }
}

async fn feed_send(client: Arc<SlackHyperClient>, url: &Url) -> anyhow::Result<()> {
    tokio::time::sleep(Duration::from_secs(SLEEP_EACH_FETCH_MINUTES * 60)).await;

    let (urls, channels) = fetch_twi_url(url).await?;
    send_message_channels(channels, &urls, client).await
}

async fn fetch_twi_url(nitter_rss_url: &Url) -> anyhow::Result<(Vec<Url>, Vec<SlackChannelId>)> {
    let raw_rss = reqwest::get(nitter_rss_url.clone()).await?;
    //println!("request occured. url: {:#?}", nitter_rss_url.domain());
    //utils::print_datetime();

    let rss_bytes = raw_rss.bytes().await?;

    let channel = Channel::read_from(&rss_bytes[..])?;
    let items = channel.items().to_vec();

    let last_date_rss = last_update(&items)?;

    let last_date_db = query::fetch_last_date(nitter_rss_url).await?;
    if last_date_db == last_date_rss {
        //println!("no update");
        return Err(anyhow::anyhow!("No update"));
    };

    let updated_tweets = if last_date_db.is_empty() {
        Vec::default()
    } else {
        updated_tweets(items, &last_date_db)
    };

    query::update_last_date(nitter_rss_url, &last_date_rss).await?;

    let feed_channels = query::fetch_channels(nitter_rss_url).await?;

    Ok((updated_tweets, feed_channels))
}

fn updated_tweets(items: Vec<Item>, last_date: &str) -> std::vec::Vec<url::Url> {
    let updated_items = items
        .into_iter()
        .take_while(|Item { pub_date, .. }| {
            pub_date.as_ref().map(std::string::String::as_str) != Some(last_date)
        })
        .filter_map(|Item { link, .. }| {
            link.and_then(|s| Url::parse(&s).ok())
                .and_then(|url| utils::nitter_url_to_twi(&url).ok())
        })
        .collect::<Vec<_>>();
    updated_items.into_iter().rev().collect::<Vec<_>>()
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
