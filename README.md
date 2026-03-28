# rsscli

`rsscli` is a fast, locally-run RSS feed CLI written in Rust. Designed specifically with local AI agents and automation scripts in mind, it provides robust capabilities for managing feeds, refreshing content concurrently, and exporting structured unread articles.

## Features
- **Fast Concurrent Fetching**: Leverages `tokio` and `reqwest` to update all your feeds in parallel.
- **Easy Import/Export**: Supports adding individual feeds or batch importing from OPML files.
- **Agent-Ready Output**: Export unread articles to `JSON` (for programmatic consumption) or `Markdown` (ideal for LLM context windows).
- **SQLite Storage**: All data is efficiently managed locally via `rusqlite`.

## Installation

Ensure you have [Rust and Cargo installed](https://rustup.rs/). Then clone the repository and install:

```bash
git clone https://github.com/zhaostu/rsscli.git
cd rsscli
cargo install --path .
```

## Usage

The CLI stores its database locally (usually in `~/.local/share/rsscli/rsscli.db` on Linux, or equivalent data directories on other OSs). You can override this using the `--db-path` global flag.

### Managing Feeds

All feed management is grouped under the `feed` command.

Add a single RSS or Atom feed:
```bash
rsscli feed add https://blog.rust-lang.org/feed.xml
```

List all feeds to find their IDs:
```bash
rsscli feed list
```

Remove a feed by its ID:
```bash
rsscli feed remove 1
```

Import a list of feeds from an OPML file:
```bash
rsscli feed import feeds.opml
```

### Refreshing Articles

Fetch the latest articles across all feeds concurrently:
```bash
rsscli refresh
```

### Printing Content

By default, `print` only outputs unread articles:
```bash
rsscli print --format json
```

To see everything (including read articles), use the `--all` flag:
```bash
rsscli print --format markdown --all
```

### Managing Read State

Once you or your automation agent has consumed the articles, you can mark them as read so they won't appear in future unread-only prints.

Mark all currently stored articles as read:
```bash
rsscli mark-read --all
```

Mark a specific article as read:
```bash
rsscli mark-read --article-id 42
```

## Contributing

Contributions are welcome! Please ensure your code passes `cargo clippy` and `cargo fmt` before submitting a pull request.

## License

MIT
