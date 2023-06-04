use anyhow::Context;
use slack_morphism::SlackChannelId;
use sqlx::{migrate::MigrateDatabase, FromRow, Pool, Sqlite, SqlitePool};
use url::Url;

use crate::utils;

const DB_URL: &str = "last-items.db";

#[derive(Debug, FromRow)]
pub struct LastDate {
    date: String,
}

#[derive(Debug, FromRow)]
pub struct FeedChannel {
    channel: String,
}
#[derive(Debug, FromRow)]
pub struct RSSUrl {
    rss_url: String,
}

pub async fn setup_db() -> anyhow::Result<()> {
    if Sqlite::database_exists(DB_URL).await? {
        return Ok(());
    }
    Sqlite::create_database(DB_URL).await?;

    let pool = SqlitePool::connect(DB_URL).await?;
    let _last_item = sqlx::query(
        "CREATE TABLE IF NOT EXISTS last_item
(
    rss_url TEXT NOT NULL PRIMARY KEY,
    account TEXT NOT NULL,
    date TEXT NOT NULL DEFAULT ''
);",
    )
    .execute(&pool)
    .await?;

    let _feed_channel = sqlx::query(
        "CREATE TABLE IF NOT EXISTS feed_channel
(
rss_url TEXT NOT NULL,
channel TEXT NOT NULL,
PRIMARY KEY (rss_url, channel)
);",
    )
    .execute(&pool)
    .await?;

    Ok(())
}

pub async fn fetch_last_date(rss_url: &Url) -> anyhow::Result<String> {
    let pool = SqlitePool::connect(DB_URL).await?;

    let last_date = sqlx::query_as::<_, LastDate>(
        "
    SELECT date
    FROM last_item
    WHERE rss_url = $1
    ",
    )
    .bind(rss_url.as_str())
    .fetch_one(&pool)
    .await?
    .date;

    Ok(last_date)
}
pub async fn fetch_rss_urls() -> anyhow::Result<Vec<Url>> {
    let pool = SqlitePool::connect(DB_URL).await?;

    let rss_urls = sqlx::query_as::<_, RSSUrl>(
        "
    SELECT li.rss_url
    FROM last_item li INNER JOIN feed_channel fc
    ON li.rss_url = fc.rss_url
    ",
    )
    .fetch_all(&pool)
    .await?
    .iter()
    .map(|s| {
        let url = Url::parse(&s.rss_url)?;
        anyhow::Ok(url)
    })
    .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(rss_urls)
}
pub async fn fetch_channels(rss_url: &Url) -> anyhow::Result<Vec<SlackChannelId>> {
    let pool = SqlitePool::connect(DB_URL).await?;

    let channels = sqlx::query_as::<_, FeedChannel>(
        "
    SELECT channel
    FROM feed_channel
    WHERE rss_url = $1
    ",
    )
    .bind(rss_url.as_str())
    .fetch_all(&pool)
    .await?
    .into_iter()
    .map(|r| SlackChannelId::new(r.channel))
    .collect::<Vec<_>>();

    Ok(channels)
}
pub async fn insert_feed_channel(channel: &SlackChannelId, url: &Url) -> anyhow::Result<()> {
    let pool = SqlitePool::connect(DB_URL).await?;

    let _query = sqlx::query(
        "
    INSERT INTO feed_channel  (rss_url, channel)
    VALUES ($1, $2);
    ",
    )
    .bind(url.as_str())
    .bind(channel.to_string())
    .execute(&pool)
    .await?;

    Ok(())
}

pub async fn insert_last_item(url: &Url) -> anyhow::Result<()> {
    let pool = SqlitePool::connect(DB_URL).await?;
    let account = utils::url_to_account(url)?;

    let _query = sqlx::query(
        "
    INSERT INTO last_item (rss_url, account)
    VALUES ($1, $2);",
    )
    .bind(url.as_str())
    .bind(account)
    .execute(&pool)
    .await?;

    Ok(())
}

pub async fn remove_rss(channel: &SlackChannelId, account: &str) -> anyhow::Result<()> {
    let pool = SqlitePool::connect(DB_URL).await?;

    let _query = sqlx::query(
        "
    DELETE 
    FROM feed_channel
    WHERE rss_url IN 
        ( SELECT fc.rss_url
        FROM feed_channel fc INNER JOIN last_item li
        ON fc.rss_url = li.rss_url
        WHERE li.account = $1 AND fc.channel = $2);
    ",
    )
    .bind(account)
    .bind(channel.to_string())
    .execute(&pool)
    .await?;

    Ok(())
}

pub async fn update_last_date(rss_url: &Url, date: &str) -> anyhow::Result<()> {
    let pool = SqlitePool::connect(DB_URL).await?;

    let _query = sqlx::query(
        "
    UPDATE  last_item
    SET date = $1
    WHERE rss_url = $2
    ",
    )
    .bind(date)
    .bind(rss_url.as_str())
    .execute(&pool)
    .await?;

    Ok(())
}
