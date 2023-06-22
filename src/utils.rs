use anyhow::Context as _;
use base64::{engine::general_purpose, Engine as _};
use chrono::Local;
use dotenvy::dotenv;
use regex::Regex;
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

pub fn is_retweet(tweet_url: &Url, nitter_account: &str) -> bool {
    url_to_account(tweet_url).map_or(false, |twi_account| twi_account != nitter_account)
}
pub fn account_to_twitter_profile(account: &str) -> anyhow::Result<Url> {
    let twitter_host = Url::parse("https://fxtwitter.com/")?;
    let tweet_url = twitter_host.join(account)?;

    Ok(tweet_url)
}

pub fn validate_display_name(display_name: &str, account: &str) -> String {
    let end_pattern = format!(" / @{account}");
    display_name.trim_end_matches(&end_pattern).to_string()
}
pub fn _print_datetime() {
    let datetime = Local::now();

    print!("{datetime:#?}");
}
// expected output: media/{id}.jpg
pub fn get_image_id(url_src: &Url) -> Option<String> {
    let re = Regex::new(r#"/([^/]+)$"#).unwrap();
    let captures = re.captures(url_src.as_str())?;
    let id_raw = captures.get(1)?.as_str();

    if id_raw.starts_with("media") {
        Some(id_raw.replace("%2F", "/"))
    } else {
        decode_base64(id_raw).ok()
    }
}

fn decode_base64(id_base64: &str) -> anyhow::Result<String> {
    let bytes = general_purpose::STANDARD.decode(id_base64)?;
    let decoded_id = String::from_utf8(bytes)?;

    Ok(decoded_id)
}
