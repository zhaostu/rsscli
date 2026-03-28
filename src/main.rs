mod cli;
mod db;
mod export;
mod fetch;
mod models;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use crate::cli::{Cli, Commands, FeedCommands};
use crate::db::DbClient;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let db_path = cli.db_path.unwrap_or_else(|| {
        let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("rsscli");
        std::fs::create_dir_all(&path).ok();
        path.push("rsscli.db");
        path
    });

    let db = DbClient::new(&db_path)?;

    match cli.command {
        Commands::Feed { command } => match command {
            FeedCommands::Add { url } => {
                fetch::add_feed(&db, &url).await?;
            }
            FeedCommands::Import { path } => {
                fetch::import_opml(&db, &path)?;
            }
            FeedCommands::List => {
                let feeds = db.get_all_feeds()?;
                println!("{:<5} {:<30} {:<30}", "ID", "Title", "URL");
                println!("{}", "-".repeat(65));
                for feed in feeds {
                    println!(
                        "{:<5} {:<30} {:<30}",
                        feed.id,
                        feed.title.as_deref().unwrap_or("Unknown"),
                        feed.url
                    );
                }
            }
            FeedCommands::Remove { id } => {
                db.delete_feed(id)?;
                println!("Removed feed with ID: {}", id);
            }
        },
        Commands::Refresh => {
            fetch::refresh_feeds(&db).await?;
        }
        Commands::Print { format, all } => {
            let unread_only = !all;
            let articles = db.get_articles(unread_only)?;
            export::export_articles(&articles, format)?;
        }
        Commands::MarkRead { all, article_id } => {
            if all {
                db.mark_read_all()?;
                println!("Marked all articles as read.");
            } else if let Some(id) = article_id {
                db.mark_read(id)?;
                println!("Marked article {} as read.", id);
            } else {
                println!("Please specify either --all or --article-id <ID>");
            }
        }
    }

    Ok(())
}
