use std::sync::Arc;

use anyhow::Context;
use futures::{StreamExt, TryStreamExt};
use slack_morphism::{
    prelude::{SlackApiChatPostMessageRequest, SlackHyperClient},
    SlackApiToken, SlackApiTokenType, SlackChannelId, SlackMessageContent,
};

use crate::{
    fetch_rss::{Tweet, TwiInfo},
    utils,
};

pub async fn send_to_channels(
    channels: Vec<SlackChannelId>,
    urls: &[Tweet],
    client: Arc<SlackHyperClient>,
    twi_info: TwiInfo,
) -> anyhow::Result<()> {
    let token = utils::get_token(&SlackApiTokenType::Bot)?;
    let channel_stream = futures::stream::iter(channels);
    channel_stream
        .map(|channel| async {
            send_tweets(channel, urls, Arc::clone(&client), &token, &twi_info).await
        })
        .then(|s| s)
        .try_collect::<()>()
        .await?;
    Ok(())
}

async fn send_tweets(
    channel: SlackChannelId,
    tweets: &[Tweet],
    client: Arc<SlackHyperClient>,
    token: &SlackApiToken,
    twi_info: &TwiInfo,
) -> anyhow::Result<()> {
    let TwiInfo {
        display_name,
        icon_url,
        account,
    } = twi_info;

    let tweets_contents = tweets
        .iter()
        .map(|tweet| {
            let content_main_str = if utils::is_retweet(&tweet.twi_url, account) {
                retweet_text(tweet, account, display_name).unwrap_or(tweet.twi_url.to_string())
            } else {
                tweet.twi_url.to_string()
            };
            let mut content_main = vec![SlackMessageContent::new().with_text(content_main_str)];
            let mut imgs = tweet_imgs_contents(tweet);

            content_main.append(&mut imgs);
            content_main
        })
        .collect::<Vec<_>>();

    let reqs = tweets_contents
        .into_iter()
        .flat_map(|contents| {
            contents
                .into_iter()
                .map(|content| {
                    SlackApiChatPostMessageRequest::new(channel.clone(), content)
                        .with_username(display_name.to_string())
                        .with_icon_url(icon_url.to_string())
                })
                .collect::<Vec<_>>()
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

fn tweet_imgs_contents(tweet: &Tweet) -> Vec<SlackMessageContent> {
    let pics = &tweet.pics;
    match pics.len() {
        0 | 1 => Vec::new(),
        _ => pics
            .iter()
            .skip(1)
            .map(|url| SlackMessageContent::new().with_text(url.to_string()))
            .collect::<Vec<_>>(),
    }
}

// retweet 時にプロフィールのリンクが展開されてしまう問題への姑息な対応
// 解決次第コメントアウトを外す
fn retweet_text(tweet: &Tweet, account: &str, display_name: &str) -> anyhow::Result<String> {
    let _twi_profile_url = utils::account_to_twitter_profile(account)?;
    let text = format!("{}\n{display_name} retweeted:", tweet.twi_url.as_str());
    // let text = format!(
    //     "{}\n<{}|{display_name}> retweeted:",
    //     tweet.twi_url.as_str(),
    //     twi_profile_url.as_str()
    // );

    Ok(text)
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;

    #[test]
    fn retweet_text_test() {
        let twi_url_str = "https://twitter.com/test/status/0000";
        let twi_url = Url::parse(twi_url_str).unwrap();
        let tweet = Tweet {
            twi_url,
            pics: Vec::default(),
        };
        let account = "test";
        let display_name = "tester";

        let rt_text = retweet_text(&tweet, account, display_name).unwrap();

        assert_eq!(
            "https://twitter.com/test/status/0000\n<https://twitter.com/test|tester> retweeted:",
            rt_text
        );
    }
}
