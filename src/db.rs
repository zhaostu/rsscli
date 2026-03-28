use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

use crate::models::{Article, Feed, NewArticle, NewFeed};

pub struct DbClient {
    pub conn: Connection,
}

impl DbClient {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path).context("Failed to open SQLite database")?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS feeds (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT UNIQUE NOT NULL,
                title TEXT,
                etag TEXT,
                last_modified TEXT
            );

            CREATE TABLE IF NOT EXISTS articles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                feed_id INTEGER NOT NULL,
                guid TEXT UNIQUE NOT NULL,
                title TEXT NOT NULL,
                url TEXT,
                summary TEXT,
                content TEXT,
                published_at DATETIME,
                is_read BOOLEAN NOT NULL DEFAULT 0,
                FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE
            );
            "#,
        )
        .context("Failed to initialize database schema")?;

        Ok(())
    }

    pub fn insert_feed(&self, feed: &NewFeed) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO feeds (url, title, etag, last_modified) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(url) DO UPDATE SET 
                title = excluded.title,
                etag = CASE WHEN excluded.etag IS NOT NULL THEN excluded.etag ELSE feeds.etag END,
                last_modified = CASE WHEN excluded.last_modified IS NOT NULL THEN excluded.last_modified ELSE feeds.last_modified END",
            params![feed.url, feed.title, feed.etag, feed.last_modified],
        )?;
        let id = self.conn.query_row(
            "SELECT id FROM feeds WHERE url = ?1",
            params![feed.url],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn get_all_feeds(&self) -> Result<Vec<Feed>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, url, title, etag, last_modified FROM feeds")?;
        let feeds = stmt
            .query_map([], |row| {
                Ok(Feed {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    etag: row.get(3)?,
                    last_modified: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(feeds)
    }

    pub fn update_feed_metadata(
        &self,
        id: i64,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE feeds SET etag = ?1, last_modified = ?2 WHERE id = ?3",
            params![etag, last_modified, id],
        )?;
        Ok(())
    }

    pub fn delete_feed(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM feeds WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn insert_article(&self, article: &NewArticle) -> Result<usize> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM articles WHERE guid = ?1)",
            params![article.guid],
            |row| row.get(0),
        )?;

        self.conn.execute(
            "INSERT INTO articles (feed_id, guid, title, url, summary, content, published_at, is_read)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)
             ON CONFLICT(guid) DO UPDATE SET 
                title = excluded.title, 
                url = excluded.url, 
                summary = excluded.summary, 
                content = CASE WHEN excluded.content IS NOT NULL THEN excluded.content ELSE articles.content END,
                published_at = excluded.published_at",
            params![
                article.feed_id,
                article.guid,
                article.title,
                article.url,
                article.summary,
                article.content,
                article.published_at,
            ],
        )?;

        Ok(if exists { 0 } else { 1 })
    }

    pub fn get_articles(&self, unread_only: bool) -> Result<Vec<Article>> {
        let query = if unread_only {
            "SELECT id, feed_id, guid, title, url, summary, content, published_at, is_read FROM articles WHERE is_read = 0"
        } else {
            "SELECT id, feed_id, guid, title, url, summary, content, published_at, is_read FROM articles"
        };

        let mut stmt = self.conn.prepare(query)?;
        let articles = stmt
            .query_map([], |row| {
                Ok(Article {
                    id: row.get(0)?,
                    feed_id: row.get(1)?,
                    guid: row.get(2)?,
                    title: row.get(3)?,
                    url: row.get(4)?,
                    summary: row.get(5)?,
                    content: row.get(6)?,
                    published_at: row.get(7)?,
                    is_read: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(articles)
    }

    pub fn mark_read_all(&self) -> Result<()> {
        self.conn.execute("UPDATE articles SET is_read = 1", [])?;
        Ok(())
    }

    pub fn mark_read(&self, id: i64) -> Result<()> {
        self.conn
            .execute("UPDATE articles SET is_read = 1 WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_articles_except(&self, feed_id: i64, seen_guids: &[String]) -> Result<usize> {
        if seen_guids.is_empty() {
            let rows = self
                .conn
                .execute("DELETE FROM articles WHERE feed_id = ?1", params![feed_id])?;
            return Ok(rows);
        }

        let query = format!(
            "DELETE FROM articles WHERE feed_id = ?1 AND guid NOT IN ({})",
            seen_guids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );

        let mut params: Vec<rusqlite::types::Value> = vec![feed_id.into()];
        for guid in seen_guids {
            params.push(guid.clone().into());
        }

        let rows = self
            .conn
            .execute(&query, rusqlite::params_from_iter(params))?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn setup_in_memory_db() -> DbClient {
        DbClient::new(":memory:").expect("Failed to create in-memory DB")
    }

    #[test]
    fn test_feed_operations() {
        let db = setup_in_memory_db();
        let new_feed = NewFeed {
            url: "http://example.com/rss".to_string(),
            title: Some("Example Feed".to_string()),
            etag: None,
            last_modified: None,
        };

        let id = db.insert_feed(&new_feed).expect("Failed to insert feed");
        assert!(id > 0);

        let feeds = db.get_all_feeds().expect("Failed to get feeds");
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].url, "http://example.com/rss");
        assert_eq!(feeds[0].title, Some("Example Feed".to_string()));

        db.delete_feed(id).expect("Failed to delete feed");
        let feeds = db.get_all_feeds().expect("Failed to get feeds");
        assert_eq!(feeds.len(), 0);
    }

    #[test]
    fn test_article_operations() {
        let db = setup_in_memory_db();
        let feed_id = db
            .insert_feed(&NewFeed {
                url: "http://example.com/rss".to_string(),
                title: Some("Example Feed".to_string()),
                etag: None,
                last_modified: None,
            })
            .expect("Failed to insert feed");

        let article = NewArticle {
            feed_id,
            guid: "guid1".to_string(),
            title: "Article 1".to_string(),
            url: Some("http://example.com/a1".to_string()),
            summary: Some("Summary 1".to_string()),
            content: Some("Content 1".to_string()),
            published_at: Some(Utc::now()),
        };

        let inserted = db
            .insert_article(&article)
            .expect("Failed to insert article");
        assert_eq!(inserted, 1);

        // Try inserting the same article again
        let reinserted = db
            .insert_article(&article)
            .expect("Failed to re-insert article");
        assert_eq!(reinserted, 0);

        let articles = db.get_articles(false).expect("Failed to get articles");
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].guid, "guid1");
        assert_eq!(articles[0].is_read, false);

        db.mark_read(articles[0].id)
            .expect("Failed to mark as read");
        let unread = db.get_articles(true).expect("Failed to get unread");
        assert_eq!(unread.len(), 0);

        let all = db.get_articles(false).expect("Failed to get all");
        assert_eq!(all.len(), 1);
        assert!(all[0].is_read);
    }

    #[test]
    fn test_pruning_logic() {
        let db = setup_in_memory_db();
        let feed_id = db
            .insert_feed(&NewFeed {
                url: "test".to_string(),
                title: None,
                etag: None,
                last_modified: None,
            })
            .expect("Insert feed");

        db.insert_article(&NewArticle {
            feed_id,
            guid: "keep".to_string(),
            title: "K".to_string(),
            url: None,
            summary: None,
            content: None,
            published_at: None,
        })
        .expect("Insert K");

        db.insert_article(&NewArticle {
            feed_id,
            guid: "prune".to_string(),
            title: "P".to_string(),
            url: None,
            summary: None,
            content: None,
            published_at: None,
        })
        .expect("Insert P");

        let deleted = db
            .delete_articles_except(feed_id, &["keep".to_string()])
            .expect("Prune");
        assert_eq!(deleted, 1);

        let remaining = db.get_articles(false).expect("Get remaining");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].guid, "keep");
    }
}
