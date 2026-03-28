use anyhow::Result;
use std::io::Write;

use crate::cli::Format;
use crate::models::Article;

pub fn export_articles(articles: &[Article], format: Format) -> Result<()> {
    let mut stdout = std::io::stdout();
    write_articles(&mut stdout, articles, format)
}

pub fn write_articles<W: Write>(
    writer: &mut W,
    articles: &[Article],
    format: Format,
) -> Result<()> {
    match format {
        Format::Json => {
            let json = serde_json::to_string_pretty(articles)?;
            writeln!(writer, "{}", json)?;
        }
        Format::Markdown => {
            for article in articles {
                writeln!(
                    writer,
                    "## [{}]({})",
                    article.title,
                    article.url.as_deref().unwrap_or("")
                )?;
                if let Some(date) = article.published_at {
                    writeln!(writer, "*Published at: {}*", date.to_rfc2822())?;
                }
                writeln!(writer)?;
                if let Some(summary) = &article.summary {
                    writeln!(writer, "**Summary:**\n{}\n", summary)?;
                }
                if let Some(content) = &article.content {
                    writeln!(writer, "**Content:**\n{}\n", content)?;
                }
                writeln!(writer, "\n---\n")?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Article;
    use chrono::Utc;

    #[test]
    fn test_write_articles_json() {
        let articles = vec![Article {
            id: 1,
            feed_id: 1,
            guid: "g1".to_string(),
            title: "T1".to_string(),
            url: Some("U1".to_string()),
            summary: Some("S1".to_string()),
            content: None,
            published_at: Some(Utc::now()),
            is_read: false,
        }];

        let mut buf = Vec::new();
        write_articles(&mut buf, &articles, Format::Json).expect("Write failed");
        let output = String::from_utf8(buf).expect("Not UTF-8");
        assert!(output.contains("\"title\": \"T1\""));
        assert!(output.contains("\"guid\": \"g1\""));
    }

    #[test]
    fn test_write_articles_markdown() {
        let articles = vec![Article {
            id: 1,
            feed_id: 1,
            guid: "g1".to_string(),
            title: "T1".to_string(),
            url: Some("U1".to_string()),
            summary: Some("S1".to_string()),
            content: Some("C1".to_string()),
            published_at: None,
            is_read: false,
        }];

        let mut buf = Vec::new();
        write_articles(&mut buf, &articles, Format::Markdown).expect("Write failed");
        let output = String::from_utf8(buf).expect("Not UTF-8");
        assert!(output.contains("## [T1](U1)"));
        assert!(output.contains("**Summary:**\nS1"));
        assert!(output.contains("**Content:**\nC1"));
    }
}
