use anyhow::Context as _;
use chrono::Local;
use dotenvy::dotenv;
use slack_morphism::{SlackApiToken, SlackApiTokenType, SlackApiTokenValue};
use std::env;
use url::Url;

pub fn get_token(token_type: &SlackApiTokenType) -> anyhow::Result<SlackApiToken> {
    dotenv().ok();
    let token_key = match token_type {
        SlackApiTokenType::App => "SLACK_APP_TOKEN",
        SlackApiTokenType::Bot => "SLACK_BOT_TOKEN",
        SlackApiTokenType::User => "SLACK_USER_TOKEN",
    };
    let token_value: SlackApiTokenValue = env::var(token_key).context("token is missing.")?.into();
    let app_token = SlackApiToken::new(token_value);
    Ok(app_token)
}

pub fn nitter_url_to_twi(nitter_url: &Url) -> anyhow::Result<Url> {
    let twitter_host = Url::parse("https://twitter.com/")?;
    let tweet_path = nitter_url.path();

    let tweet_url = twitter_host.join(tweet_path)?;

    Ok(tweet_url)
}

pub fn nitter_url_to_nitter(nitter_url: &Url) -> anyhow::Result<&str> {
    let nitter = nitter_url.domain().context("invalid url")?;
    Ok(nitter)
}

pub fn url_to_account(nitter_url: &Url) -> anyhow::Result<&str> {
    let account = nitter_url
        .path_segments()
        .and_then(|mut s| s.next())
        .context("invalid input")?;
    Ok(account)
}

pub fn _print_datetime() {
    let datetime = Local::now();

    print!("{datetime:#?}");
}
