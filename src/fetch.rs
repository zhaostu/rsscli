use anyhow::{Context, Result};
use feed_rs::parser;
use reqwest::Client;
use std::fs;
use std::path::Path;
use tokio::sync::mpsc;
use tokio::task;

use crate::db::DbClient;
use crate::models::{NewArticle, NewFeed};

pub enum RefreshEvent {
    Article(NewArticle),
    FeedFinished(i64, Vec<String>),
    Metadata(i64, Option<String>, Option<String>),
}

pub async fn add_feed(db: &DbClient, url: &str) -> Result<()> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (compatible; rsscli/0.1.0; +https://github.com/zhaostu/rsscli)")
        .build()?;
    let response = client
        .get(url)
        .header("Accept", "application/rss+xml, application/atom+xml, application/xml, text/xml, application/json")
        .send()
        .await
        .context("Failed to fetch feed")?
        .error_for_status()
        .context("Server returned an error status")?;

    let etag = response
        .headers()
        .get("etag")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let last_modified = response
        .headers()
        .get("last-modified")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    let content = response
        .bytes()
        .await
        .context("Failed to read feed content")?;
    let feed = parser::parse(&content[..]).context("Failed to parse feed")?;

    let new_feed = NewFeed {
        url: url.to_string(),
        title: feed.title.map(|t| t.content),
        etag,
        last_modified,
    };

    db.insert_feed(&new_feed)?;
    println!(
        "Successfully added feed: {}",
        new_feed.title.unwrap_or_else(|| "Unknown".to_string())
    );
    Ok(())
}

pub fn import_opml(db: &DbClient, path: &Path) -> Result<()> {
    let content = fs::read_to_string(path).context("Failed to read OPML file")?;
    let opml = opml::OPML::from_str(&content).context("Failed to parse OPML file")?;

    for outline in opml.body.outlines {
        if let Some(xml_url) = outline.xml_url {
            let new_feed = NewFeed {
                url: xml_url,
                title: Some(outline.text.clone()),
                etag: None,
                last_modified: None,
            };
            if let Err(e) = db.insert_feed(&new_feed) {
                eprintln!("Failed to import feed {}: {}", new_feed.url, e);
            } else {
                println!(
                    "Imported feed: {}",
                    new_feed.title.unwrap_or_else(|| "Unknown".to_string())
                );
            }
        }

        for nested in outline.outlines {
            if let Some(xml_url) = nested.xml_url {
                let new_feed = NewFeed {
                    url: xml_url,
                    title: Some(nested.text.clone()),
                    etag: None,
                    last_modified: None,
                };
                if let Err(e) = db.insert_feed(&new_feed) {
                    eprintln!("Failed to import feed {}: {}", new_feed.url, e);
                } else {
                    println!(
                        "Imported feed: {}",
                        new_feed.title.unwrap_or_else(|| "Unknown".to_string())
                    );
                }
            }
        }
    }
    Ok(())
}

pub async fn refresh_feeds(db: &DbClient) -> Result<()> {
    let feeds = db.get_all_feeds()?;
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (compatible; rsscli/0.1.0; +https://github.com/zhaostu/rsscli)")
        .build()?;

    let (tx, mut rx) = mpsc::channel(100);
    let mut handles = Vec::new();

    for feed in feeds {
        let client = client.clone();
        let tx = tx.clone();

        let handle = task::spawn(async move {
            let res = async {
                let mut request = client.get(&feed.url)
                    .header("Accept", "application/rss+xml, application/atom+xml, application/xml, text/xml, application/json");

                if let Some(etag) = &feed.etag {
                    request = request.header("If-None-Match", etag);
                }
                if let Some(last_modified) = &feed.last_modified {
                    request = request.header("If-Modified-Since", last_modified);
                }

                let response = request.send().await?
                    .error_for_status()?;

                if response.status() == reqwest::StatusCode::NOT_MODIFIED {
                    return Ok::<(), anyhow::Error>(());
                }

                let etag = response
                    .headers()
                    .get("etag")
                    .and_then(|h| h.to_str().ok())
                    .map(|s| s.to_string());
                let last_modified = response
                    .headers()
                    .get("last-modified")
                    .and_then(|h| h.to_str().ok())
                    .map(|s| s.to_string());

                if etag.is_some() || last_modified.is_some() {
                    let _ = tx
                        .send(RefreshEvent::Metadata(feed.id, etag, last_modified))
                        .await;
                }

                let content = response.bytes().await?;
                let parsed = parser::parse(&content[..])?;

                let mut guids = Vec::new();
                for entry in parsed.entries {
                    guids.push(entry.id.clone());

                    let summary = entry.summary.map(|s| s.content);
                    let content = entry.content.and_then(|c| c.body);

                    let article = NewArticle {
                        feed_id: feed.id,
                        guid: entry.id.clone(),
                        title: entry
                            .title
                            .map(|t| t.content)
                            .unwrap_or_else(|| "Untitled".to_string()),
                        url: entry.links.into_iter().next().map(|l| l.href),
                        summary,
                        content,
                        published_at: entry.published,
                    };

                    if tx.send(RefreshEvent::Article(article)).await.is_err() {
                        return Ok(());
                    }
                }

                let _ = tx.send(RefreshEvent::FeedFinished(feed.id, guids)).await;
                Ok::<(), anyhow::Error>(())
            };

            if let Err(e) = res.await {
                eprintln!("Error refreshing feed {}: {}", feed.url, e);
            }
        });
        handles.push(handle);
    }

    drop(tx);

    let mut new_articles_count = 0;
    let mut deleted_articles_count = 0;

    while let Some(event) = rx.recv().await {
        match event {
            RefreshEvent::Article(article) => {
                if let Ok(rows_inserted) = db.insert_article(&article) {
                    new_articles_count += rows_inserted;
                }
            }
            RefreshEvent::FeedFinished(feed_id, guids) => {
                if let Ok(rows_deleted) = db.delete_articles_except(feed_id, &guids) {
                    deleted_articles_count += rows_deleted;
                }
            }
            RefreshEvent::Metadata(feed_id, etag, last_modified) => {
                let _ = db.update_feed_metadata(feed_id, etag, last_modified);
            }
        }
    }

    for handle in handles {
        let _ = handle.await;
    }

    println!(
        "Imported {} new articles. Purged {} articles no longer in feeds.",
        new_articles_count, deleted_articles_count
    );

    Ok(())
}
