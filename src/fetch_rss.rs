use std::sync::Arc;

use anyhow::Context;
use futures::StreamExt;
use rss::{Channel, Item};
use scraper::{Html, Selector};
use slack_morphism::{prelude::SlackHyperClient, SlackChannelId};

use tokio::time::Duration;
use url::Url;

use crate::{
    query::{self, fetch_nitters, fetch_rss_urls},
    send_message, utils,
};

const SLEEP_EACH_FETCH_MINUTES: u64 = 5;

pub async fn feed_loop(client: Arc<SlackHyperClient>) -> anyhow::Result<()> {
    let nitters = fetch_nitters().await?;
    for nitter in nitters {
        tokio::spawn(feed_loop_nitter(Arc::clone(&client), nitter));
    }

    Ok(())
}
pub async fn feed_loop_nitter(client: Arc<SlackHyperClient>, nitter: String) -> anyhow::Result<()> {
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
    }
}

async fn feed_send(client: Arc<SlackHyperClient>, url: &Url) -> anyhow::Result<()> {
    tokio::time::sleep(Duration::from_secs(SLEEP_EACH_FETCH_MINUTES * 60)).await;

    let account = utils::url_to_account(url)?.to_string();

    let rss_channel = fetch_rss(url).await?;
    let twi_info = get_twi_info(&rss_channel, account)?;
    let (urls, channels) = fetch_twi_url(url, rss_channel).await?;

    send_message::send_to_channels(channels, &urls, client, twi_info).await
}

async fn fetch_rss(nitter_rss_url: &Url) -> anyhow::Result<Channel> {
    let raw_rss = reqwest::get(nitter_rss_url.clone()).await?;
    //println!("request occured. url: {:#?}", nitter_rss_url.domain());
    //utils::_print_datetime();

    let rss_bytes = raw_rss.bytes().await?;

    let channel = Channel::read_from(&rss_bytes[..])?;

    Ok(channel)
}
async fn fetch_twi_url(
    nitter_rss_url: &Url,
    rss_channel: Channel,
) -> anyhow::Result<(Vec<Tweet>, Vec<SlackChannelId>)> {
    let items = rss_channel.items().to_vec();

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

#[derive(Debug)]
pub struct TwiInfo {
    pub icon_url: Url,
    pub display_name: String,
    pub account: String,
}
fn get_twi_info(rss_channel: &Channel, account: String) -> anyhow::Result<TwiInfo> {
    let display_name_raw = rss_channel.title();
    let display_name = utils::validate_display_name(display_name_raw, &account);
    let icon_url_str = rss_channel.image().context("invalid input")?.url();
    let icon_url = url::Url::parse(icon_url_str)?;

    Ok(TwiInfo {
        icon_url,
        display_name,
        account,
    })
}
#[derive(Debug)]
pub struct Tweet {
    pub twi_url: Url,
    pub pics: Vec<Url>,
}
fn updated_tweets(items: Vec<Item>, last_date: &str) -> std::vec::Vec<Tweet> {
    let updated_items = items
        .into_iter()
        .take_while(|Item { pub_date, .. }| {
            pub_date.as_ref().map(std::string::String::as_str) != Some(last_date)
        })
        .filter_map(
            |Item {
                 link, description, ..
             }| {
                let url_opt = link
                    .and_then(|s| Url::parse(&s).ok())
                    .and_then(|url| utils::nitter_url_to_twi(&url).ok());
                if let Some(url) = url_opt {
                    let pics = description.map_or(Vec::default(), |des| fetch_twi_images(&des));
                    Some(Tweet { twi_url: url, pics })
                } else {
                    None
                }
            },
        )
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

pub fn fetch_twi_images(description: &str) -> Vec<Url> {
    let nitter_imgs = fetch_nitter_images(description);
    let twi_pic = Url::parse("https://pbs.twimg.com/").unwrap();

    let twi_imgs = nitter_imgs
        .iter()
        .filter_map(|nit_url| {
            let id_opt = utils::get_image_id(nit_url);
            id_opt.and_then(|id| twi_pic.join(&id).ok())
        })
        .collect::<Vec<_>>();
    twi_imgs
}

pub fn fetch_nitter_images(description: &str) -> Vec<Url> {
    let fragment = Html::parse_document(description);
    let selector = Selector::parse("img[src]").unwrap();

    let urls = fragment
        .select(&selector)
        .map(|elem| {
            elem.value()
                .attr("src")
                .map(|url_str| url_str.replace("%2F", "/"))
        })
        .filter_map(|src| src.and_then(|url| Url::parse(&url).ok()))
        .collect::<Vec<_>>();

    urls
}
