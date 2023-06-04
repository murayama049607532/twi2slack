use anyhow::Context as _;
use dotenvy::dotenv;
use slack_morphism::{SlackApiToken, SlackApiTokenType, SlackApiTokenValue};
use std::env;

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
