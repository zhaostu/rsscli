use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rsscli")]
#[command(about = "A CLI for managing and fetching RSS feeds, optimized for local agents", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Optional path to the database file
    #[arg(short, long, global = true)]
    pub db_path: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage feeds (add, remove, list, import)
    Feed {
        #[command(subcommand)]
        command: FeedCommands,
    },
    /// Refresh all feeds and store new articles in the database
    Refresh,
    /// Print articles (defaults to unread only)
    Print {
        /// Output format
        #[arg(short, long, value_enum, default_value_t = Format::Json)]
        format: Format,

        /// Print all articles (including read ones)
        #[arg(short, long)]
        all: bool,
    },
    /// Mark articles as read
    MarkRead {
        /// Mark all articles as read
        #[arg(long)]
        all: bool,

        /// Mark a specific article as read by its ID
        #[arg(long)]
        article_id: Option<i64>,
    },
}

#[derive(Subcommand)]
pub enum FeedCommands {
    /// Add a single feed URL
    Add {
        /// URL of the RSS/Atom feed
        url: String,
    },
    /// Remove a feed by its ID
    Remove {
        /// ID of the feed to remove
        id: i64,
    },
    /// List all added feeds
    List,
    /// Import feeds from an OPML file
    Import {
        /// Path to the OPML file
        path: PathBuf,
    },
}

#[derive(clap::ValueEnum, Clone, Debug, Default, PartialEq)]
pub enum Format {
    #[default]
    Json,
    Markdown,
}
