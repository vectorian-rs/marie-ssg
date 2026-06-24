//src/content.rs

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;
use time::OffsetDateTime;
use tracing::{debug, error, instrument};

use crate::syntax::highlight_html;
use crate::utils::add_header_anchors;

/// Creates markdown parsing options with optional dangerous HTML support.
fn markdown_options(allow_dangerous_html: bool) -> markdown::Options {
    markdown::Options {
        compile: markdown::CompileOptions {
            allow_dangerous_html,
            // Disable GFM tag filter when dangerous HTML is allowed
            // (otherwise <style>, <script>, etc. are still escaped)
            gfm_tagfilter: !allow_dangerous_html,
            ..markdown::CompileOptions::gfm()
        },
        ..markdown::Options::gfm()
    }
}

/// Represents a complete content piece with metadata and raw markdown data.
///
/// This struct combines the parsed metadata with the actual
/// markdown content of a file. It serves as the primary data structure
/// for processing individual content files throughout the application.
#[derive(Debug)]
pub(crate) struct Content {
    /// The parsed metadata containing title, date, author, etc.
    pub meta: ContentMeta,
    /// The raw markdown content body as read from the file.
    pub data: String,
}

/// Metadata for content pieces, typically stored in `.meta.toml` files.
///
/// This struct represents metadata that accompanies each
/// markdown content file. It contains essential information about the
/// content such as title, publication date, author, and tags.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ContentMeta {
    /// Title of the content piece (article, post, page, etc.)
    pub title: String,
    /// Publication date of the content (recommended format: YYYY-MM-DD)
    #[serde(with = "time::serde::rfc3339")]
    pub date: OffsetDateTime,
    /// Author of the content
    pub author: String,
    /// List of tags/categories associated with the content
    pub tags: Vec<String>,
    /// Optional custom template to use for rendering this content
    /// If not specified, a default template will be used
    #[serde(default)]
    pub template: Option<String>,
    /// Optional cover image URL/path for this content
    #[serde(default)]
    pub cover: Option<String>,
    /// Additional custom fields defined in [extra] section of metadata
    /// Access in templates via meta.extra.field_name
    #[serde(default)]
    pub extra: HashMap<String, String>,
    /// JavaScript files to load for this content
    /// Access in templates via meta.extra_js
    #[serde(default)]
    pub extra_js: Vec<String>,
    /// Whether this content is a draft (excluded from builds unless --include-drafts)
    #[serde(default)]
    pub draft: bool,
}

/// Template-ready URL information for the current rendered page.
///
/// `url` is a root-relative URL path (for example, `/blog/post/`).
/// `permalink` and `canonical_url` are absolute URLs and intentionally aliases,
/// so templates can use either naming convention.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct PageInfo {
    /// Output path relative to the site root, without a leading slash.
    pub(crate) filename: String,
    /// Root-relative URL path, with a leading slash.
    pub(crate) url: String,
    /// Absolute URL for this page.
    pub(crate) permalink: String,
    /// Absolute canonical URL for this page.
    pub(crate) canonical_url: String,
}

/// Processed content item ready for template rendering and output.
///
/// This struct contains the fully processed content including converted HTML,
/// formatted metadata, and derived information like content type and filename.
/// It's primarily used when passing content data to templates for rendering.
#[derive(Debug, Serialize)]
pub(crate) struct ContentItem {
    /// The HTML-rendered content body
    pub(crate) html: String,
    /// The parsed metadata
    pub(crate) meta: ContentMeta,
    /// Human-readable formatted date string
    pub(crate) formatted_date: String,
    /// Output filename for this content piece
    pub(crate) filename: String,
    /// Root-relative URL path for this content piece
    pub(crate) url: String,
    /// Absolute URL for this content piece
    pub(crate) permalink: String,
    /// Absolute canonical URL for this content piece
    pub(crate) canonical_url: String,
    /// Content type category (e.g., "blog", "projects", "page")
    pub(crate) content_type: String,
    /// HTML excerpt extracted from the content
    pub(crate) excerpt: String,
}

/// Error types that can occur during content loading and processing.
///
/// These errors cover various failure scenarios when working with content files,
/// including I/O issues, TOML parsing errors, and markdown conversion problems.
#[derive(Error, Debug)]
pub(crate) enum ContentError {
    /// I/O error when reading or processing content files
    #[error("I/O error processing file {path:?}: {source}")]
    Io {
        /// Path to the file that caused the I/O error
        path: PathBuf,
        /// The underlying I/O error
        #[source]
        source: std::io::Error,
    },
    /// TOML parsing error in metadata files
    #[error("TOML parsing error in metadata file {path:?}: {source}")]
    TomlParse {
        /// Path to the TOML file that failed to parse
        path: PathBuf,
        /// The underlying TOML parsing error
        #[source]
        source: toml::de::Error,
    },
    /// Markdown parsing or conversion failure
    #[error("Markdown parsing failed for file {path:?}: {message}")]
    MarkdownParsingFailed {
        /// Path to the markdown file that failed to parse
        path: PathBuf,
        /// Detailed error message from the markdown parser
        message: String,
    },
    /// Syntax highlighting failure
    #[error("Syntax highlighting failed for file {path:?}: {message}")]
    SyntaxHighlighting {
        /// Path to the file that caused the highlighting error
        path: PathBuf,
        /// Detailed error message from the syntax highlighter
        message: String,
    },
}

/// Loads both metadata and content from a markdown file.
///
/// This function reads a markdown file and its corresponding `.meta.toml` file,
/// returning a complete `Content` struct with both the parsed metadata and
/// raw markdown content.
///
/// # Arguments
/// * `path` - Path to the markdown file to load
///
/// # Returns
/// `Result<Content, ContentError>` - The loaded content or an error
///
/// # Errors
/// Returns `ContentError::Io` if the markdown file cannot be read.
/// Returns `ContentError::TomlParse` if the metadata file cannot be parsed.
///
/// # Examples
/// ```
/// # use std::path::PathBuf;
/// # use your_crate::content::load_content;
/// let content = load_content(&PathBuf::from("content/blog/post.md"))?;
/// println!("Title: {}", content.meta.title);
/// ```
#[instrument(skip_all)]
pub(crate) fn load_content(path: &PathBuf) -> Result<Content, ContentError> {
    // 1. Load the metadata from the corresponding `.meta.toml` file.
    let meta = load_metadata(path)?;

    // 2. Read the entire markdown file content into a string.
    debug!("io::read ← {:?}", path);
    let data = fs::read_to_string(path).map_err(|e| ContentError::Io {
        path: path.clone(),
        source: e,
    })?;
    debug!("io::read {} bytes", data.len());

    Ok(Content { meta, data })
}

/// Loads metadata from a `.meta.toml` file corresponding to a markdown file.
///
/// For a given markdown file path, this function looks for and parses the
/// associated metadata file (e.g., `post.md` -> `post.meta.toml`).
///
/// # Arguments
/// * `markdown_path` - Path to the markdown file
///
/// # Returns
/// `Result<ContentMeta, ContentError>` - The parsed metadata or an error
///
/// # Errors
/// Returns `ContentError::Io` if the metadata file cannot be read.
/// Returns `ContentError::TomlParse` if the TOML content cannot be parsed.
///
/// # Examples
/// ```
/// # use std::path::PathBuf;
/// # use your_crate::content::load_metadata;
/// let meta = load_metadata(&PathBuf::from("content/blog/post.md"))?;
/// println!("Author: {}", meta.author);
/// ```
pub(crate) fn load_metadata(markdown_path: &Path) -> Result<ContentMeta, ContentError> {
    // hello-world.md" -> "hello-world.meta.toml"
    let meta_path = markdown_path.with_extension("meta.toml");
    debug!("io::read ← {:?}", meta_path);
    let meta_content = fs::read_to_string(&meta_path).map_err(|e| ContentError::Io {
        path: meta_path.clone(),
        source: e,
    })?;
    debug!("io::read {} bytes", meta_content.len());

    let metadata: ContentMeta =
        toml::from_str(&meta_content).map_err(|e| ContentError::TomlParse {
            path: meta_path,
            source: e,
        })?;

    Ok(metadata)
}

/// Convert markdown content to HTML with optional syntax highlighting.
///
/// This function converts markdown to HTML and can apply syntax highlighting
/// to code blocks if `highlighting_enabled` is true.
///
/// # Arguments
/// * `content` - The markdown content with metadata
/// * `path` - Path to the source file (for error reporting)
/// * `highlighting_enabled` - Whether to apply syntax highlighting
/// * `theme` - The theme to use for highlighting (if enabled)
/// * `allow_dangerous_html` - Whether to allow raw HTML in markdown
/// * `header_uri_fragment` - Whether to add anchor links to headers
///
/// # Returns
/// `Result<String, ContentError>` where the string is HTML content.
///
/// # Errors
/// Returns `ContentError::MarkdownParsingFailed` if markdown conversion fails,
/// or `ContentError::SyntaxHighlighting` if highlighting fails.
///
/// # Examples
/// ```ignore
/// # use std::path::PathBuf;
/// # use time::OffsetDateTime;
/// # let content = Content {
/// #     meta: ContentMeta {
/// #         title: "Test".to_string(),
/// #         date: OffsetDateTime::now_utc(),
/// #         author: "Author".to_string(),
/// #         tags: vec![],
/// #         template: None,
/// #     },
/// #     data: "# Hello World\n\n```rust\nfn main() {}\n```".to_string(),
/// # };
/// let html = convert_content_with_highlighting(&content, Path::new("test.md"), true, "github_dark", false, false);
/// assert!(html.contains("<h1>Hello World</h1>"));
/// ```
#[instrument(skip_all)]
pub(crate) fn convert_content_with_highlighting(
    content: &Content,
    path: &Path,
    highlighting_enabled: bool,
    theme: &str,
    allow_dangerous_html: bool,
    header_uri_fragment: bool,
) -> Result<String, ContentError> {
    // Convert markdown to HTML
    let mut html = match markdown::to_html_with_options(
        &content.data,
        &markdown_options(allow_dangerous_html),
    ) {
        Ok(html) => html,
        Err(e) => {
            error!("Markdown parsing failed: {}", e);
            return Err(ContentError::MarkdownParsingFailed {
                path: path.to_path_buf(),
                message: e.to_string(),
            });
        }
    };

    // Add header anchor links if enabled
    if header_uri_fragment {
        html = add_header_anchors(&html);
    }

    // Apply syntax highlighting if enabled
    if highlighting_enabled {
        match highlight_html(&html, theme) {
            Ok(highlighted) => Ok(highlighted),
            Err(e) => {
                error!("Syntax highlighting failed: {}", e);
                // We could fall back to unhighlighted HTML, but for now we'll error
                Err(ContentError::SyntaxHighlighting {
                    path: path.to_path_buf(),
                    message: e.to_string(),
                })
            }
        }
    } else {
        Ok(html)
    }
}

/// Extracts an HTML excerpt from markdown content using a specified pattern.
///
/// This function searches for a specific pattern (typically "## Context") in
/// markdown content and extracts everything from that pattern until the next
/// heading. The extracted markdown is then converted to HTML.
///
/// # Arguments
/// * `markdown` - The markdown content to search for excerpts
/// * `summary_pattern` - The pattern to use for identifying excerpt sections
///
/// # Returns
/// `String` - The HTML-rendered excerpt, or empty string if pattern not found
///
/// # Examples
/// ```
/// # use your_crate::content::get_excerpt_html;
/// let markdown = r#"
/// Some introductory text.
///
/// ## Summary
/// This is the excerpt text.
///
/// ## Main Content
/// The rest of the content.
/// "#;
///
/// let excerpt = get_excerpt_html(markdown, "## Summary", false);
/// assert!(excerpt.contains("This is the excerpt text"));
/// assert!(!excerpt.contains("Main Content"));
/// ```
pub(crate) fn get_excerpt_html(
    markdown: &str,
    summary_pattern: &str,
    allow_dangerous_html: bool,
) -> String {
    // Find the start of the summary section
    if let Some(start_idx) = markdown.find(summary_pattern) {
        // Ensure we don't panic if summary_pattern is at the end
        if start_idx + summary_pattern.len() >= markdown.len() {
            return String::new();
        }

        let content_after_summary = &markdown[start_idx + summary_pattern.len()..];

        // Find the next heading (## or ###) or end of content
        let end_idx = content_after_summary
            .find("\n##")
            .or_else(|| content_after_summary.find("\n###"))
            .or_else(|| content_after_summary.find("\n# ")) // Also catch single # headings
            .unwrap_or(content_after_summary.len());

        let excerpt_markdown = content_after_summary[..end_idx].trim();

        // Convert the excerpt markdown to HTML with better error handling
        match markdown::to_html_with_options(
            excerpt_markdown,
            &markdown_options(allow_dangerous_html),
        ) {
            Ok(html) => html,
            Err(e) => {
                tracing::warn!("Failed to convert excerpt to HTML: {}", e);
                String::new()
            }
        }
    } else {
        String::new() // Return empty string if no summary found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use time::macros::datetime;

    // Helper function to create test metadata
    fn create_test_metadata() -> ContentMeta {
        ContentMeta {
            title: "Test Post".to_string(),
            date: datetime!(2023-12-15 10:30:00 +5), // UTC+5
            author: "Test Author".to_string(),
            tags: vec!["rust".to_string(), "testing".to_string()],
            template: Some("custom.html".to_string()),
            cover: Some("/images/test-cover.jpg".to_string()),
            extra: HashMap::new(),
            extra_js: vec![],
            draft: false,
        }
    }

    #[test]
    fn test_get_excerpt_html_with_summary() {
        let markdown = r#"
Some introductory text.

## Summary
This is the excerpt text that should be extracted.
It can have **bold** and *italic* formatting.

## Main Content
The rest of the content goes here.
"#;

        let excerpt = get_excerpt_html(markdown, "## Summary", false);
        assert!(excerpt.contains("This is the excerpt text"));
        assert!(excerpt.contains("<strong>bold</strong>"));
        assert!(excerpt.contains("<em>italic</em>"));
        assert!(!excerpt.contains("Main Content")); // Should stop before next heading
    }

    #[test]
    fn test_get_excerpt_html_no_summary() {
        let markdown = r#"
Some content without a summary section.

## Another Heading
Just regular content.
"#;

        let excerpt = get_excerpt_html(markdown, "## Summary", false);
        assert_eq!(excerpt, "");
    }

    #[test]
    fn test_get_excerpt_html_summary_at_end() {
        let markdown = r#"
## Summary
This is the only content.
"#;

        let excerpt = get_excerpt_html(markdown, "## Summary", false);
        assert!(excerpt.contains("This is the only content"));
    }

    #[test]
    fn test_get_excerpt_html_with_different_headings() {
        let markdown = r#"
## Summary
Excerpt content here.

### Subheading
This should not be included.
"#;

        let excerpt = get_excerpt_html(markdown, "## Summary", false);
        assert!(excerpt.contains("Excerpt content here"));
        assert!(!excerpt.contains("Subheading"));
    }

    #[test]
    fn test_get_excerpt_html_with_single_hash_heading() {
        let markdown = r#"
## Summary
Excerpt content.

# Main Heading
Should not be included.
"#;

        let excerpt = get_excerpt_html(markdown, "## Summary", false);
        assert!(excerpt.contains("Excerpt content"));
        assert!(!excerpt.contains("Main Heading"));
    }

    #[test]
    fn test_get_excerpt_html_empty_input() {
        let excerpt = get_excerpt_html("", "## Summary", false);
        assert_eq!(excerpt, "");
    }

    #[test]
    fn test_get_excerpt_html_pattern_not_found() {
        let markdown = "Just some regular content without the pattern.";
        let excerpt = get_excerpt_html(markdown, "## Summary", false);
        assert_eq!(excerpt, "");
    }

    #[test]
    fn test_convert_content_with_highlighting_enabled() {
        let content = Content {
            meta: create_test_metadata(),
            data: "# Hello World\n\n```rust\nfn main() {\n    println!(\"test\");\n}\n```"
                .to_string(),
        };

        let result = convert_content_with_highlighting(
            &content,
            Path::new("test.md"),
            true,
            "github_dark",
            false,
            false,
        );
        assert!(result.is_ok());
        let html = result.unwrap();
        assert!(html.contains("<h1>Hello World</h1>"));
        // Should have highlighted the code block
        assert!(html.contains("fn"));
        assert!(html.contains("main"));
        assert!(html.contains("language-rust"));
    }

    #[test]
    fn test_convert_content_with_highlighting_disabled() {
        let content = Content {
            meta: create_test_metadata(),
            data: "# Hello World\n\n```rust\nfn main() {}\n```".to_string(),
        };

        let result = convert_content_with_highlighting(
            &content,
            Path::new("test.md"),
            false,
            "github_dark",
            false,
            false,
        );
        assert!(result.is_ok());
        let html = result.unwrap();
        assert!(html.contains("<h1>Hello World</h1>"));
        // Should have plain code block without highlighting
        assert!(html.contains("<pre><code"));
        // Should not have inline styles from highlighting
        assert!(!html.contains("style="));
    }

    #[test]
    fn test_convert_content_with_highlighting_unknown_language() {
        let content = Content {
            meta: create_test_metadata(),
            data: "# Test\n\n```unknownlang\nsome code\n```".to_string(),
        };

        let result = convert_content_with_highlighting(
            &content,
            Path::new("test.md"),
            true,
            "github_dark",
            false,
            false,
        );
        // Should still succeed - unknown languages fall back to plain text
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_metadata_success() {
        let temp_dir = tempdir().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let meta_path = temp_dir.path().join("test.meta.toml");

        // Create metadata file with proper RFC 3339 date format
        let meta_content = r#"
    title = "Test Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["rust", "testing"]
    template = "custom.html"
    "#;

        File::create(&meta_path)
            .unwrap()
            .write_all(meta_content.as_bytes())
            .unwrap();

        let result = load_metadata(&md_path);
        assert!(
            result.is_ok(),
            "Failed to load metadata: {:?}",
            result.err()
        );

        let meta = result.unwrap();
        assert_eq!(meta.title, "Test Post");
        assert_eq!(meta.author, "Test Author");
        assert_eq!(meta.tags, vec!["rust", "testing"]);
        assert_eq!(meta.template, Some("custom.html".to_string()));

        // Verify the date was parsed correctly
        let expected_date = datetime!(2023-12-15 10:30:00 +5); // UTC+5
        assert_eq!(meta.date, expected_date);
    }

    #[test]
    fn test_load_metadata_file_not_found() {
        let temp_dir = tempdir().unwrap();
        let md_path = temp_dir.path().join("nonexistent.md");

        let result = load_metadata(&md_path);
        assert!(result.is_err());

        if let Err(ContentError::Io { path, source: _ }) = result {
            assert!(path.ends_with("nonexistent.meta.toml"));
        } else {
            panic!("Expected Io error");
        }
    }

    #[test]
    fn test_load_metadata_invalid_toml() {
        let temp_dir = tempdir().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let meta_path = temp_dir.path().join("test.meta.toml");

        // Create invalid TOML
        File::create(&meta_path)
            .unwrap()
            .write_all(b"invalid toml content [")
            .unwrap();

        let result = load_metadata(&md_path);
        assert!(result.is_err());

        if let Err(ContentError::TomlParse { path, source: _ }) = result {
            assert!(path.ends_with("test.meta.toml"));
        } else {
            panic!("Expected TomlParse error");
        }
    }

    #[test]
    fn test_load_content_success() {
        let temp_dir = tempdir().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let meta_path = temp_dir.path().join("test.meta.toml");

        // Create metadata file with proper date format
        let meta_content = r#"
    title = "Test Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["rust", "testing"]
    "#;

        File::create(&meta_path)
            .unwrap()
            .write_all(meta_content.as_bytes())
            .unwrap();

        // Create markdown file
        File::create(&md_path)
            .unwrap()
            .write_all(b"# Test Content\n\nThis is test content.")
            .unwrap();

        let result = load_content(&md_path);
        assert!(result.is_ok(), "Failed to load content: {:?}", result.err());

        let content = result.unwrap();
        assert_eq!(content.meta.title, "Test Post");
        assert_eq!(content.data, "# Test Content\n\nThis is test content.");

        // Verify date was parsed correctly
        let expected_date = datetime!(2023-12-15 10:30:00 +5); // UTC+5
        assert_eq!(content.meta.date, expected_date);
    }

    #[test]
    fn test_load_content_metadata_file_not_found() {
        let temp_dir = tempdir().unwrap();
        let md_path = temp_dir.path().join("test.md");

        // Create markdown file but no metadata file
        File::create(&md_path)
            .unwrap()
            .write_all(b"# Test Content")
            .unwrap();

        let result = load_content(&md_path);
        assert!(result.is_err());

        if let Err(ContentError::Io { path, source: _ }) = result {
            assert!(path.ends_with("test.meta.toml"));
        } else {
            panic!("Expected Io error for metadata file");
        }
    }

    #[test]
    fn test_content_meta_serialization_deserialization() {
        let meta = create_test_metadata();

        // Serialize to TOML
        let toml_string = toml::to_string(&meta).unwrap();
        assert!(toml_string.contains("title = \"Test Post\""));
        assert!(toml_string.contains("tags = [\"rust\", \"testing\"]"));

        // Deserialize back
        let deserialized: ContentMeta = toml::from_str(&toml_string).unwrap();
        assert_eq!(deserialized.title, meta.title);
        assert_eq!(deserialized.author, meta.author);
        assert_eq!(deserialized.tags, meta.tags);
    }

    #[test]
    fn test_content_meta_without_template() {
        // Use proper RFC 3339 date format for TOML
        let meta_content = r#"
    title = "Test Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["rust"]
    "#;

        let meta: ContentMeta = toml::from_str(meta_content).unwrap();
        assert_eq!(meta.template, None);
        assert_eq!(meta.title, "Test Post");
        assert_eq!(meta.author, "Test Author");
        assert_eq!(meta.tags, vec!["rust"]);

        // Verify date was parsed
        let expected_date = datetime!(2023-12-15 10:30:00 +5); // UTC+5
        assert_eq!(meta.date, expected_date);
    }

    #[test]
    fn test_content_meta_with_cover() {
        let meta_content = r#"
    title = "Test Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["rust"]
    cover = "/images/hero.jpg"
    "#;

        let meta: ContentMeta = toml::from_str(meta_content).unwrap();
        assert_eq!(meta.cover, Some("/images/hero.jpg".to_string()));
    }

    #[test]
    fn test_content_meta_without_cover() {
        let meta_content = r#"
    title = "Test Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["rust"]
    "#;

        let meta: ContentMeta = toml::from_str(meta_content).unwrap();
        assert_eq!(meta.cover, None);
    }

    #[test]
    fn test_content_meta_with_extra_fields() {
        let meta_content = r#"
    title = "Test Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["rust"]

    [extra]
    custom_field = "custom value"
    another_field = "another value"
    "#;

        let meta: ContentMeta = toml::from_str(meta_content).unwrap();
        assert_eq!(
            meta.extra.get("custom_field"),
            Some(&"custom value".to_string())
        );
        assert_eq!(
            meta.extra.get("another_field"),
            Some(&"another value".to_string())
        );
    }

    #[test]
    fn test_content_meta_with_cover_and_extra_fields() {
        let meta_content = r#"
    title = "Test Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["rust"]
    cover = "/images/cover.png"

    [extra]
    subtitle = "A great subtitle"
    category = "tutorials"
    "#;

        let meta: ContentMeta = toml::from_str(meta_content).unwrap();
        assert_eq!(meta.cover, Some("/images/cover.png".to_string()));
        assert_eq!(
            meta.extra.get("subtitle"),
            Some(&"A great subtitle".to_string())
        );
        assert_eq!(meta.extra.get("category"), Some(&"tutorials".to_string()));
        // Ensure known fields are not in extra
        assert_eq!(meta.extra.get("title"), None);
        assert_eq!(meta.extra.get("cover"), None);
    }

    #[test]
    fn test_content_meta_with_extra_js() {
        let meta_content = r#"
    title = "Data Visualization Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["viz"]
    extra_js = ["static/js/d3.min.js", "static/js/chart.js"]
    "#;

        let meta: ContentMeta = toml::from_str(meta_content).unwrap();
        assert_eq!(meta.extra_js.len(), 2);
        assert_eq!(meta.extra_js[0], "static/js/d3.min.js");
        assert_eq!(meta.extra_js[1], "static/js/chart.js");
    }

    #[test]
    fn test_content_meta_without_extra_js_defaults_to_empty() {
        let meta_content = r#"
    title = "Simple Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["rust"]
    "#;

        let meta: ContentMeta = toml::from_str(meta_content).unwrap();
        assert!(meta.extra_js.is_empty());
    }

    #[test]
    fn test_content_meta_with_extra_js_and_extra_fields() {
        let meta_content = r#"
    title = "Full Featured Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["viz", "data"]
    cover = "/images/cover.png"
    extra_js = ["static/js/viz.js"]

    [extra]
    reading_time = "5 min"
    "#;

        let meta: ContentMeta = toml::from_str(meta_content).unwrap();
        assert_eq!(meta.extra_js.len(), 1);
        assert_eq!(meta.extra_js[0], "static/js/viz.js");
        assert_eq!(meta.cover, Some("/images/cover.png".to_string()));
        assert_eq!(meta.extra.get("reading_time"), Some(&"5 min".to_string()));
    }

    #[test]
    fn test_get_excerpt_html_with_custom_pattern() {
        let markdown = r#"
Some text.

<!-- excerpt -->
This is a custom excerpt pattern.

## Content
Main content.
"#;

        let excerpt = get_excerpt_html(markdown, "<!-- excerpt -->", false);
        assert!(excerpt.contains("This is a custom excerpt pattern"));
        assert!(!excerpt.contains("Main content"));
    }

    #[test]
    fn test_load_content_read_error() {
        let temp_dir = tempdir().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let meta_path = temp_dir.path().join("test.meta.toml");

        // Create metadata file
        let meta_content = r#"
    title = "Test Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = []
    "#;
        File::create(&meta_path)
            .unwrap()
            .write_all(meta_content.as_bytes())
            .unwrap();

        // Ensure markdown file does NOT exist
        if md_path.exists() {
            std::fs::remove_file(&md_path).unwrap();
        }

        let result = load_content(&md_path);
        assert!(result.is_err());

        if let Err(ContentError::Io { path, source: _ }) = result {
            assert_eq!(path, md_path);
        } else {
            panic!("Expected Io error for content file");
        }
    }

    #[test]
    fn test_get_excerpt_html_no_end_heading() {
        let markdown = r#"
## Summary
This is the excerpt.
This continues until the end of the string.
"#;
        let excerpt = get_excerpt_html(markdown, "## Summary", false);
        assert!(excerpt.contains("This is the excerpt"));
        assert!(excerpt.contains("end of the string"));
    }

    #[test]
    fn test_get_excerpt_html_exact_match_end() {
        let markdown = "## Summary";
        let excerpt = get_excerpt_html(markdown, "## Summary", false);
        assert_eq!(excerpt, "");
    }

    #[test]
    fn test_convert_content_with_dangerous_html_disabled() {
        let content = Content {
            meta: create_test_metadata(),
            data:
                "# Test\n\n<figure><img src=\"test.jpg\"><figcaption>Caption</figcaption></figure>"
                    .to_string(),
        };

        let result = convert_content_with_highlighting(
            &content,
            Path::new("test.md"),
            false,
            "github_dark",
            false, // dangerous HTML disabled
            false,
        );
        assert!(result.is_ok());
        let html = result.unwrap();
        // HTML should be escaped when dangerous HTML is disabled
        assert!(html.contains("&lt;figure&gt;"));
    }

    #[test]
    fn test_convert_content_with_dangerous_html_enabled() {
        let content = Content {
            meta: create_test_metadata(),
            data:
                "# Test\n\n<figure><img src=\"test.jpg\"><figcaption>Caption</figcaption></figure>"
                    .to_string(),
        };

        let result = convert_content_with_highlighting(
            &content,
            Path::new("test.md"),
            false,
            "github_dark",
            true, // dangerous HTML enabled
            false,
        );
        assert!(result.is_ok());
        let html = result.unwrap();
        // HTML should be preserved when dangerous HTML is enabled
        assert!(html.contains("<figure>"));
        assert!(html.contains("<figcaption>"));
    }

    #[test]
    fn test_get_excerpt_html_with_dangerous_html_disabled() {
        let markdown = "## Summary\n\n<div class=\"custom\">Custom content</div>";
        let excerpt = get_excerpt_html(markdown, "## Summary", false);
        // HTML should be escaped
        assert!(excerpt.contains("&lt;div"));
    }

    #[test]
    fn test_get_excerpt_html_with_dangerous_html_enabled() {
        let markdown = "## Summary\n\n<div class=\"custom\">Custom content</div>";
        let excerpt = get_excerpt_html(markdown, "## Summary", true);
        // HTML should be preserved
        assert!(excerpt.contains("<div class=\"custom\">"));
    }

    #[test]
    fn test_convert_content_with_header_uri_fragment_enabled() {
        let content = Content {
            meta: create_test_metadata(),
            data: "# Main Title\n\nSome text.\n\n## Section One\n\nMore text.\n\n## Section Two\n\nEven more text.".to_string(),
        };

        let result = convert_content_with_highlighting(
            &content,
            Path::new("test.md"),
            false,
            "github_dark",
            false,
            true, // header_uri_fragment enabled
        );
        assert!(result.is_ok());
        let html = result.unwrap();

        // Headers should have id attributes and anchor links
        assert!(html.contains("id=\"main-title\""));
        assert!(html.contains("href=\"#main-title\""));
        assert!(html.contains("id=\"section-one\""));
        assert!(html.contains("href=\"#section-one\""));
        assert!(html.contains("id=\"section-two\""));
        assert!(html.contains("href=\"#section-two\""));
    }

    #[test]
    fn test_convert_content_with_header_uri_fragment_disabled() {
        let content = Content {
            meta: create_test_metadata(),
            data: "# Main Title\n\nSome text.".to_string(),
        };

        let result = convert_content_with_highlighting(
            &content,
            Path::new("test.md"),
            false,
            "github_dark",
            false,
            false, // header_uri_fragment disabled
        );
        assert!(result.is_ok());
        let html = result.unwrap();

        // Headers should NOT have id attributes when disabled
        assert!(!html.contains("id=\"main-title\""));
        assert!(html.contains("<h1>Main Title</h1>"));
    }

    #[test]
    fn test_content_meta_draft_defaults_to_false() {
        let meta_content = r#"
    title = "Test Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["rust"]
    "#;

        let meta: ContentMeta = toml::from_str(meta_content).unwrap();
        assert!(!meta.draft, "draft should default to false");
    }

    #[test]
    fn test_content_meta_with_draft_true() {
        let meta_content = r#"
    title = "Draft Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["rust"]
    draft = true
    "#;

        let meta: ContentMeta = toml::from_str(meta_content).unwrap();
        assert!(meta.draft, "draft should be true when explicitly set");
    }

    #[test]
    fn test_content_meta_with_draft_false() {
        let meta_content = r#"
    title = "Published Post"
    date = "2023-12-15T10:30:00+05:00"
    author = "Test Author"
    tags = ["rust"]
    draft = false
    "#;

        let meta: ContentMeta = toml::from_str(meta_content).unwrap();
        assert!(!meta.draft, "draft should be false when explicitly set");
    }
}
