#![warn(clippy::pedantic)]

mod command_event_handler;
mod fetch_rss;
mod query;
mod utils;

use slack_morphism::prelude::*;
use std::sync::Arc;

async fn socket_mode_process(
    client: Arc<SlackHyperClient>,
    app_token: Arc<SlackApiToken>,
) -> anyhow::Result<()> {
    println!("socket_mode start");
    let socket_mode_callbacks = SlackSocketModeListenerCallbacks::new()
        .with_command_events(command_event_handler::command_event_handler);
    let listner_environment = Arc::new(
        SlackClientEventsListenerEnvironment::new(client.clone()).with_error_handler(error_handler),
    );
    let socket_mode_listner = SlackClientSocketModeListener::new(
        &SlackClientSocketModeConfig::new(),
        listner_environment.clone(),
        socket_mode_callbacks,
    );

    socket_mode_listner.listen_for(&app_token).await?;
    socket_mode_listner.serve().await;
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn error_handler(
    err: Box<dyn std::error::Error + Send + Sync>,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> http::StatusCode {
    println!("err:{err:#?}");
    http::StatusCode::OK
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    query::setup_db().await?;

    let app_token = Arc::new(utils::get_token(&SlackApiTokenType::App)?);
    let client = Arc::new(SlackClient::new(SlackClientHyperConnector::new()));

    tokio::spawn(socket_mode_process(
        Arc::clone(&client),
        Arc::clone(&app_token),
    ));

    fetch_rss::feed_loop(client).await?;

    Ok(())
}
