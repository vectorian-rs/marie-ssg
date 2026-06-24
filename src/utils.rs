// src/utils.rs

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use walkdir::WalkDir;

use crate::config::Config;

/// Converts text to a URL-friendly slug for use in HTML IDs and URL fragments.
///
/// The slugification process:
/// 1. Converts to lowercase
/// 2. Replaces spaces and underscores with hyphens
/// 3. Removes all non-alphanumeric characters (except hyphens)
/// 4. Collapses multiple consecutive hyphens into one
/// 5. Trims leading/trailing hyphens
///
/// # Arguments
/// * `text` - The text to slugify
///
/// # Returns
/// A URL-friendly slug string
///
/// # Examples
/// ```
/// use your_crate::slugify;
///
/// assert_eq!(slugify("Hello World"), "hello-world");
/// assert_eq!(slugify("My Section Title!"), "my-section-title");
/// assert_eq!(slugify("What's New?"), "whats-new");
/// ```
pub(crate) fn slugify(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    for c in text.chars() {
        match c {
            'A'..='Z' => result.push(c.to_ascii_lowercase()),
            'a'..='z' | '0'..='9' => result.push(c),
            ' ' | '_' => result.push('-'),
            '-' => result.push('-'),
            _ => {} // Skip other characters
        }
    }

    // Collapse multiple hyphens and trim
    let mut collapsed = String::with_capacity(result.len());
    let mut prev_hyphen = true; // Start true to trim leading hyphens

    for c in result.chars() {
        if c == '-' {
            if !prev_hyphen {
                collapsed.push('-');
            }
            prev_hyphen = true;
        } else {
            collapsed.push(c);
            prev_hyphen = false;
        }
    }

    // Trim trailing hyphen
    if collapsed.ends_with('-') {
        collapsed.pop();
    }

    collapsed
}

/// Extracts the stem from a filename, removing extension and any date prefix.
///
/// The stem is the meaningful identifier portion of the filename. For backwards
/// compatibility, this also strips YYYY-MM-DD- date prefixes if present.
///
/// # Arguments
/// * `filename` - The filename to extract stem from (e.g., "my-article.md")
///
/// # Returns
/// The filename stem without extension (e.g., "my-article")
///
/// # Examples
/// ```
/// use your_crate::extract_stem_from_filename;
///
/// assert_eq!(extract_stem_from_filename("2025-12-29-my-article.md"), "my-article");
/// assert_eq!(extract_stem_from_filename("my-article.md"), "my-article");
/// assert_eq!(extract_stem_from_filename("2025-12-29-my-article"), "my-article");
/// ```
pub(crate) fn extract_stem_from_filename(filename: &str) -> String {
    // Remove extension if present
    let name = filename
        .strip_suffix(".md")
        .or_else(|| filename.strip_suffix(".markdown"))
        .or_else(|| filename.strip_suffix(".html"))
        .unwrap_or(filename);

    // Check for date prefix pattern: YYYY-MM-DD-
    // Date prefix is exactly 11 characters: 4 digits + hyphen + 2 digits + hyphen + 2 digits + hyphen
    if name.len() > 11 {
        let potential_date = &name[..11];
        // Check if it matches YYYY-MM-DD- pattern
        if potential_date.len() == 11
            && potential_date.chars().nth(4) == Some('-')
            && potential_date.chars().nth(7) == Some('-')
            && potential_date.chars().nth(10) == Some('-')
            && potential_date[..4].chars().all(|c| c.is_ascii_digit())
            && potential_date[5..7].chars().all(|c| c.is_ascii_digit())
            && potential_date[8..10].chars().all(|c| c.is_ascii_digit())
        {
            return name[11..].to_string();
        }
    }

    name.to_string()
}

/// Strips HTML tags from a string, returning only the text content.
///
/// This is used to extract plain text from header content that may contain
/// inline formatting like `<strong>`, `<em>`, `<code>`, etc.
///
/// # Arguments
/// * `html` - The HTML string to strip tags from
///
/// # Returns
/// The text content without HTML tags
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    result
}

/// Adds anchor links to HTML headers (h1-h6) for URL fragment navigation.
///
/// Transforms headers like `<h2>My Section</h2>` into:
/// `<h2 id="my-section"><a href="#my-section">My Section</a></h2>`
///
/// Handles duplicate slugs by appending -1, -2, etc.
///
/// # Arguments
/// * `html` - The HTML content to process
///
/// # Returns
/// The HTML with anchor links added to all headers
pub(crate) fn add_header_anchors(html: &str) -> String {
    let mut result = String::with_capacity(html.len() + html.len() / 4);
    let mut slug_counts: HashMap<String, usize> = HashMap::new();
    let mut remaining = html;

    while let Some(start_pos) = remaining.find("<h") {
        // Add everything before this header
        result.push_str(&remaining[..start_pos]);
        remaining = &remaining[start_pos..];

        // Check if this is actually a header tag (h1-h6)
        if remaining.len() < 4 {
            result.push_str(remaining);
            break;
        }

        let level_char = remaining.chars().nth(2);
        let after_level = remaining.chars().nth(3);

        if let (Some(level @ '1'..='6'), Some(c)) = (level_char, after_level)
            && (c == '>' || c == ' ')
        {
            // Find the closing tag
            let close_tag = format!("</h{}>", level);
            if let Some(close_pos) = remaining.find(&close_tag) {
                // Find the end of opening tag
                if let Some(open_end) = remaining[..close_pos].find('>') {
                    let content = &remaining[open_end + 1..close_pos];
                    let text_content = strip_html_tags(content);
                    let base_slug = slugify(&text_content);

                    // Handle duplicate slugs
                    let slug = if let Some(count) = slug_counts.get(&base_slug) {
                        format!("{}-{}", base_slug, count)
                    } else {
                        base_slug.clone()
                    };
                    *slug_counts.entry(base_slug).or_insert(0) += 1;

                    // Build the new header with anchor
                    result.push_str(&format!(
                        "<h{} id=\"{}\"><a href=\"#{}\">{}</a></h{}>",
                        level, slug, slug, content, level
                    ));

                    remaining = &remaining[close_pos + close_tag.len()..];
                    continue;
                }
            }
        }

        // Not a valid header, just copy the character and continue
        if let Some(c) = remaining.chars().next() {
            result.push(c);
            remaining = &remaining[c.len_utf8()..];
        }
    }

    // Add any remaining content
    result.push_str(remaining);
    result
}

/// Extracts the content type from a file path relative to the content directory.
///
/// The content type is determined by the first directory component after stripping
/// the content directory prefix from the file path. If the path cannot be stripped
/// or has no directory components, it defaults to "page".
///
/// # Arguments
/// * `file` - The full path to the content file
/// * `content_dir` - The base content directory path
///
/// # Returns
/// A string representing the content type (e.g., "projects", "blog", "page")
///
/// # Examples
/// ```
/// use std::path::PathBuf;
/// use your_crate::get_content_type;
///
/// let file = PathBuf::from("src/content/projects/local-rs.md");
/// let content_type = get_content_type(&file, "src/content");
/// assert_eq!(content_type, "projects");
/// ```
///
/// # Behavior
/// - If the file is not under the content directory, returns "page"
/// - If the file is directly in the content directory (no subdirectory), returns "page"
/// - The content directory prefix is stripped case-sensitively
#[rustfmt::skip]
pub(crate) fn get_content_type(file: &Path, content_dir: &str) -> String {
    file.strip_prefix(content_dir)                              // removes src/content
        .ok()                                                   // Convert Result to Option
        .and_then(|rel_path| rel_path.components().next())      // gets the next dir (projects)
        .and_then(|comp| comp.as_os_str().to_str())             // converts  that to &str
        .unwrap_or("page")                                      // or gets "page"
        .to_string()                                            // as  string
}

/// Recursively finds all markdown files in the specified content directory.
///
/// This function traverses the directory tree starting from `content_dir` and
/// collects paths to all files with `.md` or `.markdown` extensions. The search
/// is performed recursively through all subdirectories.
///
/// # Arguments
/// * `content_dir` - The root directory path to search for markdown files
///
/// # Returns
/// A vector of `PathBuf` objects representing the absolute paths to all
/// found markdown files.
///
/// # Examples
/// ```
/// use std::path::PathBuf;
/// use your_crate::find_markdown_files;
///
/// // Assuming directory structure:
/// // content/
/// //   index.md
/// //   blog/
/// //     post1.md
/// //     post2.markdown
/// let files = find_markdown_files("content");
/// assert!(files.iter().any(|p| p.ends_with("index.md")));
/// assert!(files.iter().any(|p| p.ends_with("post1.md")));
/// assert!(files.iter().any(|p| p.ends_with("post2.markdown")));
/// ```
///
/// # Notes
/// - The search is case-sensitive for file extensions
/// - Only regular files are considered (symlinks and directories are ignored)
/// - The function returns an empty vector if the directory doesn't exist or
///   contains no markdown files
/// - Hidden files and directories (starting with `.`) are included in the search
pub(crate) fn find_markdown_files(content_dir: &str) -> Vec<PathBuf> {
    let mut markdown_files = Vec::new();

    let walkdir = WalkDir::new(content_dir);

    for entry in walkdir.into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if entry.file_type().is_file()
            && let Some(ext) = path.extension()
            && (ext == "md" || ext == "markdown")
        {
            markdown_files.push(path.to_path_buf());
        }
    }

    markdown_files
}

/// Resolves a URL pattern by replacing placeholders with actual values.
///
/// Supports the following placeholders:
/// - `{stem}` - The stem extracted from the filename (without extension)
/// - `{date}` - Full date in YYYY-MM-DD format from meta.date
/// - `{year}` - Year (4 digits) from meta.date
/// - `{month}` - Month (2 digits, zero-padded) from meta.date
/// - `{day}` - Day (2 digits, zero-padded) from meta.date
///
/// # Arguments
/// * `pattern` - The URL pattern template (e.g., "{year}/{month}/{day}/{stem}")
/// * `filename` - The original filename (e.g., "my-article.md")
/// * `date` - The publication date from meta.date
///
/// # Returns
/// The resolved URL pattern with all placeholders replaced
///
/// # Examples
/// ```
/// use time::macros::datetime;
///
/// let date = datetime!(2025-12-12 02:02:02 UTC);
/// let result = resolve_url_pattern("{date}-{stem}", "my-article.md", &date);
/// assert_eq!(result, "2025-12-12-my-article");
///
/// let result = resolve_url_pattern("{year}/{month}/{day}/{stem}", "my-article.md", &date);
/// assert_eq!(result, "2025/12/12/my-article");
/// ```
pub(crate) fn resolve_url_pattern(pattern: &str, filename: &str, date: &OffsetDateTime) -> String {
    let stem = extract_stem_from_filename(filename);
    let year = format!("{:04}", date.year());
    let month = format!("{:02}", date.month() as u8);
    let day = format!("{:02}", date.day());
    let date_str = format!("{}-{}-{}", year, month, day);

    pattern
        .replace("{stem}", &stem)
        .replace("{date}", &date_str)
        .replace("{year}", &year)
        .replace("{month}", &month)
        .replace("{day}", &day)
}

/// Builds the output file path from a resolved URL pattern.
///
/// Combines the output directory, content type, and resolved pattern to create
/// the final output path. When `clean_urls` is enabled, appends `/index.html`;
/// otherwise appends `.html`.
///
/// # Arguments
/// * `content_type` - The content type directory (e.g., "articles", "blog")
/// * `resolved_pattern` - The resolved URL pattern (e.g., "2025-12-12/my-article")
/// * `output_dir` - The output directory (e.g., "dist")
/// * `clean_urls` - Whether to use clean URL structure
///
/// # Returns
/// The full output path as a PathBuf
///
/// # Examples
/// ```
/// // clean_urls = false
/// let path = build_output_path("articles", "my-article", "dist", false);
/// assert_eq!(path, PathBuf::from("dist/articles/my-article.html"));
///
/// // clean_urls = true
/// let path = build_output_path("articles", "2025-12-12/my-article", "dist", true);
/// assert_eq!(path, PathBuf::from("dist/articles/2025-12-12/my-article/index.html"));
/// ```
pub(crate) fn build_output_path(
    content_type: &str,
    resolved_pattern: &str,
    output_dir: &str,
    clean_urls: bool,
) -> PathBuf {
    let base = PathBuf::from(output_dir)
        .join(content_type)
        .join(resolved_pattern);

    if clean_urls {
        base.join("index.html")
    } else {
        base.with_extension("html")
    }
}

/// Converts an output path to a site-root-relative URL path without a leading slash.
///
/// For clean URLs, `blog/post/index.html` becomes `blog/post/`. Otherwise the
/// HTML filename is preserved, for example `blog/post.html`.
pub(crate) fn output_path_to_relative_url(
    output_path: &Path,
    output_dir: &str,
    clean_urls: bool,
) -> String {
    let relative_path = output_path.strip_prefix(output_dir).unwrap_or(output_path);

    let raw_path = relative_path.to_string_lossy().replace('\\', "/");

    if clean_urls {
        if raw_path == "index.html" {
            return String::new();
        }

        raw_path
            .strip_suffix("/index.html")
            .map(|s| format!("{}/", s))
            .unwrap_or(raw_path)
    } else {
        raw_path
    }
}

/// Converts an output path to a root-relative URL path with a leading slash.
pub(crate) fn output_path_to_url_path(
    output_path: &Path,
    output_dir: &str,
    clean_urls: bool,
) -> String {
    let relative = output_path_to_relative_url(output_path, output_dir, clean_urls);

    if relative.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", relative.trim_start_matches('/'))
    }
}

/// Returns the site's absolute base URL from a configured domain.
pub(crate) fn site_base_url(domain: &str) -> String {
    let trimmed = domain.trim().trim_end_matches('/');

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    }
}

/// Builds an absolute URL for a root-relative path.
pub(crate) fn absolute_url(domain: &str, path: &str) -> String {
    let base = site_base_url(domain);
    let path = if path.is_empty() {
        "/"
    } else if path.starts_with('/') {
        path
    } else {
        return format!("{}/{}", base, path);
    };

    format!("{}{}", base, path)
}

/// Retrieves the template path for a specific content type from the configuration.
///
/// This function looks up the configured template for a given content type in the
/// site configuration. If the content type is not configured or doesn't exist,
/// it falls back to the default template.
///
/// # Arguments
/// * `config` - Reference to the site configuration containing content type definitions
/// * `content_type` - The content type to look up (e.g., "blog", "projects", "page")
///
/// # Returns
/// A string containing the template file path for the specified content type.
/// Returns "default.html" if the content type is not found in the configuration.
///
/// # Examples
/// ```
/// use your_crate::{get_content_type_template, Config, ContentTypeConfig};
/// use std::collections::HashMap;
///
/// // Create a test configuration
/// let mut config = Config {
///     content_types: HashMap::from([
///         ("blog".to_string(), ContentTypeConfig {
///             content_template: "blog_post.html".to_string(),
///             index_template: "blog_index.html".to_string(),
///             output_naming: Some("default".to_string()),
///         }),
///         ("projects".to_string(), ContentTypeConfig {
///             content_template: "project.html".to_string(),
///             index_template: "projects_index.html".to_string(),
///             output_naming: Some("default".to_string()),
///         }),
///     ]),
///     // ... other config fields
/// #     title: "Test".to_string(),
/// #     tagline: "Test".to_string(),
/// #     domain: "test.com".to_string(),
/// #     author: "Test".to_string(),
/// #     output_dir: "out".to_string(),
/// #     content_dir: "content".to_string(),
/// #     template_dir: "templates".to_string(),
/// #     static_dir: "static".to_string(),
/// #     site_index_template: "index.html".to_string(),
/// };
///
/// // Get template for configured content type
/// let template = get_content_type_template(&config, "blog");
/// assert_eq!(template, "blog_post.html");
///
/// // Fallback for unknown content type
/// let default_template = get_content_type_template(&config, "unknown");
/// assert_eq!(default_template, "default.html");
/// ```
///
/// # Notes
/// - The returned template path is relative to the template directory
/// - Content type lookup is case-sensitive
/// - The fallback template "default.html" should exist in the template directory
/// - This function only returns the template path; it does not validate if the template file exists
pub(crate) fn get_content_type_template(config: &Config, content_type: &str) -> String {
    config
        .content
        .get(content_type)
        .map(|ct| ct.content_template.as_str())
        .unwrap_or("default.html") // fallback template
        .to_string()
}

#[cfg(test)]
mod tests {
    use crate::config::ContentTypeConfig;

    use super::*;
    use std::collections::HashMap;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn create_test_config() -> Config {
        let mut content_types = HashMap::new();
        content_types.insert(
            "projects".to_string(),
            ContentTypeConfig {
                content_template: "project.html".to_string(),
                index_template: "projects_index.html".to_string(),
                url_pattern: None,
                output_naming: Some("default".to_string()),
                rss_include: None,
            },
        );
        content_types.insert(
            "blog".to_string(),
            ContentTypeConfig {
                content_template: "blog_post.html".to_string(),
                index_template: "blog_index.html".to_string(),
                url_pattern: None,
                output_naming: Some("default".to_string()),
                rss_include: None,
            },
        );

        Config {
            site: crate::config::SiteConfig {
                title: "Test Site".to_string(),
                tagline: "Hello world".to_string(),
                domain: "test.com".to_string(),
                author: "Test Author".to_string(),
                output_dir: "out".to_string(),
                content_dir: "src/content".to_string(),
                template_dir: "templates".to_string(),
                static_dir: "static".to_string(),
                site_index_template: "site_index.html".to_string(),
                syntax_highlighting_enabled: true,
                syntax_highlighting_theme: crate::syntax::DEFAULT_THEME.to_string(),
                root_static: HashMap::new(),
                sitemap_enabled: true,
                rss_enabled: true,
                allow_dangerous_html: false,
                header_uri_fragment: false,
                clean_urls: false,
                rss_full_content: false,
                asset_hashing_enabled: false,
                asset_manifest_path: None,
            },
            content: content_types,
            dynamic: HashMap::new(),
            redirects: HashMap::new(),
        }
    }

    #[test]
    fn test_get_content_type_extracts_directory() {
        let input = PathBuf::from("src/content/projects/local-rs.md");
        let result = get_content_type(&input, "src/content");

        assert_eq!(result, "projects");
    }

    #[test]
    fn test_get_content_type_falls_back_to_page() {
        let input = PathBuf::from("different/path/file.md");
        let result = get_content_type(&input, "src/content");

        assert_eq!(result, "page");
    }

    #[test]
    fn test_get_content_type_template_returns_configured_template() {
        let config = create_test_config();
        let result = get_content_type_template(&config, "projects");

        assert_eq!(result, "project.html");
    }

    #[test]
    fn test_get_content_type_template_falls_back_to_default() {
        let config = create_test_config();
        let result = get_content_type_template(&config, "unknown");

        assert_eq!(result, "default.html");
    }

    #[test]
    fn test_get_content_type_nested_directory() {
        // Test file in nested directory structure
        let input = PathBuf::from("src/content/blog/tech/rust/post.md");
        let result = get_content_type(&input, "src/content");
        assert_eq!(result, "blog");
    }

    #[test]
    fn test_find_markdown_files() {
        // Create temporary directory structure
        let temp_dir = tempdir().unwrap();
        let content_dir = temp_dir.path();

        // Create test files and directories
        fs::create_dir(content_dir.join("blog")).unwrap();
        fs::create_dir(content_dir.join("projects")).unwrap();

        // Create markdown files
        File::create(content_dir.join("index.md"))
            .unwrap()
            .write_all(b"# Index")
            .unwrap();
        File::create(content_dir.join("blog/post1.md"))
            .unwrap()
            .write_all(b"# Post 1")
            .unwrap();
        File::create(content_dir.join("blog/post2.markdown"))
            .unwrap()
            .write_all(b"# Post 2")
            .unwrap();
        File::create(content_dir.join("projects/readme.md"))
            .unwrap()
            .write_all(b"# Project")
            .unwrap();

        // Create non-markdown files (should be ignored)
        File::create(content_dir.join("style.css"))
            .unwrap()
            .write_all(b"body {}")
            .unwrap();
        File::create(content_dir.join("blog/script.js"))
            .unwrap()
            .write_all(b"console.log()")
            .unwrap();

        let result = find_markdown_files(content_dir.to_str().unwrap());

        assert_eq!(result.len(), 4);
        assert!(result.iter().any(|p| p.ends_with("index.md")));
        assert!(result.iter().any(|p| p.ends_with("post1.md")));
        assert!(result.iter().any(|p| p.ends_with("post2.markdown")));
        assert!(result.iter().any(|p| p.ends_with("readme.md")));

        // Verify non-markdown files are not included
        assert!(!result.iter().any(|p| p.ends_with("style.css")));
        assert!(!result.iter().any(|p| p.ends_with("script.js")));
    }

    #[test]
    fn test_find_markdown_files_empty_directory() {
        let temp_dir = tempdir().unwrap();
        let content_dir = temp_dir.path();

        let result = find_markdown_files(content_dir.to_str().unwrap());
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_find_markdown_files_no_markdown() {
        let temp_dir = tempdir().unwrap();
        let content_dir = temp_dir.path();

        File::create(content_dir.join("index.txt"))
            .unwrap()
            .write_all(b"text")
            .unwrap();
        File::create(content_dir.join("style.css"))
            .unwrap()
            .write_all(b"css")
            .unwrap();

        let result = find_markdown_files(content_dir.to_str().unwrap());
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_slugify_basic() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("My Section Title"), "my-section-title");
    }

    #[test]
    fn test_slugify_special_characters() {
        assert_eq!(slugify("What's New?"), "whats-new");
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("C++ Programming"), "c-programming");
    }

    #[test]
    fn test_slugify_underscores() {
        assert_eq!(slugify("hello_world"), "hello-world");
        assert_eq!(slugify("my_section_title"), "my-section-title");
    }

    #[test]
    fn test_slugify_multiple_spaces() {
        assert_eq!(slugify("Hello   World"), "hello-world");
        assert_eq!(slugify("  Leading spaces"), "leading-spaces");
        assert_eq!(slugify("Trailing spaces  "), "trailing-spaces");
    }

    #[test]
    fn test_slugify_numbers() {
        assert_eq!(slugify("Version 2.0"), "version-20");
        assert_eq!(slugify("123 Test"), "123-test");
    }

    #[test]
    fn test_slugify_empty_and_special_only() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("!!!"), "");
        assert_eq!(slugify("---"), "");
    }

    #[test]
    fn test_add_header_anchors_basic() {
        let html = "<h1>Hello World</h1>";
        let result = add_header_anchors(html);
        assert_eq!(
            result,
            "<h1 id=\"hello-world\"><a href=\"#hello-world\">Hello World</a></h1>"
        );
    }

    #[test]
    fn test_add_header_anchors_all_levels() {
        let html = "<h1>H1</h1><h2>H2</h2><h3>H3</h3><h4>H4</h4><h5>H5</h5><h6>H6</h6>";
        let result = add_header_anchors(html);
        assert!(result.contains("<h1 id=\"h1\"><a href=\"#h1\">H1</a></h1>"));
        assert!(result.contains("<h2 id=\"h2\"><a href=\"#h2\">H2</a></h2>"));
        assert!(result.contains("<h3 id=\"h3\"><a href=\"#h3\">H3</a></h3>"));
        assert!(result.contains("<h4 id=\"h4\"><a href=\"#h4\">H4</a></h4>"));
        assert!(result.contains("<h5 id=\"h5\"><a href=\"#h5\">H5</a></h5>"));
        assert!(result.contains("<h6 id=\"h6\"><a href=\"#h6\">H6</a></h6>"));
    }

    #[test]
    fn test_add_header_anchors_with_inline_formatting() {
        let html = "<h2><strong>Bold</strong> and <em>italic</em></h2>";
        let result = add_header_anchors(html);
        assert_eq!(
            result,
            "<h2 id=\"bold-and-italic\"><a href=\"#bold-and-italic\"><strong>Bold</strong> and <em>italic</em></a></h2>"
        );
    }

    #[test]
    fn test_add_header_anchors_duplicate_slugs() {
        let html = "<h2>Introduction</h2><p>Some text</p><h2>Introduction</h2><p>More text</p><h2>Introduction</h2>";
        let result = add_header_anchors(html);
        assert!(result.contains("id=\"introduction\""));
        assert!(result.contains("id=\"introduction-1\""));
        assert!(result.contains("id=\"introduction-2\""));
    }

    #[test]
    fn test_add_header_anchors_preserves_other_content() {
        let html = "<p>Before</p><h2>Title</h2><p>After</p>";
        let result = add_header_anchors(html);
        assert!(result.contains("<p>Before</p>"));
        assert!(result.contains("<p>After</p>"));
        assert!(result.contains("<h2 id=\"title\"><a href=\"#title\">Title</a></h2>"));
    }

    #[test]
    fn test_add_header_anchors_no_headers() {
        let html = "<p>Just a paragraph</p><div>And a div</div>";
        let result = add_header_anchors(html);
        assert_eq!(result, html);
    }

    #[test]
    fn test_add_header_anchors_html_like_text() {
        // Ensure we don't match things like <hr> or <head>
        let html = "<hr><p>Test</p>";
        let result = add_header_anchors(html);
        assert_eq!(result, html);
    }

    // Tests for extract_stem_from_filename
    #[test]
    fn test_extract_stem_from_filename_with_date_prefix() {
        assert_eq!(
            extract_stem_from_filename("2025-12-29-my-article.md"),
            "my-article"
        );
        assert_eq!(
            extract_stem_from_filename("2024-01-15-hello-world.md"),
            "hello-world"
        );
    }

    #[test]
    fn test_extract_stem_from_filename_without_date_prefix() {
        assert_eq!(extract_stem_from_filename("my-article.md"), "my-article");
        assert_eq!(extract_stem_from_filename("hello-world.md"), "hello-world");
    }

    #[test]
    fn test_extract_stem_from_filename_different_extensions() {
        assert_eq!(
            extract_stem_from_filename("2025-12-29-article.markdown"),
            "article"
        );
        assert_eq!(
            extract_stem_from_filename("2025-12-29-article.html"),
            "article"
        );
        assert_eq!(extract_stem_from_filename("2025-12-29-article"), "article");
    }

    #[test]
    fn test_extract_stem_from_filename_edge_cases() {
        // Invalid date format - should keep as is
        assert_eq!(
            extract_stem_from_filename("2025-1-29-article.md"),
            "2025-1-29-article"
        );
        // Too short to have date prefix
        assert_eq!(extract_stem_from_filename("short.md"), "short");
        // Exactly 11 chars but not a date
        assert_eq!(
            extract_stem_from_filename("abcd-ef-gh-article.md"),
            "abcd-ef-gh-article"
        );
    }

    // Tests for resolve_url_pattern
    #[test]
    fn test_resolve_url_pattern_filename_only() {
        use time::macros::datetime;
        let date = datetime!(2025-12-12 02:02:02 UTC);

        let result = resolve_url_pattern("{stem}", "my-article.md", &date);
        assert_eq!(result, "my-article");
    }

    #[test]
    fn test_resolve_url_pattern_date_prefix() {
        use time::macros::datetime;
        let date = datetime!(2025-12-12 02:02:02 UTC);

        let result = resolve_url_pattern("{date}-{stem}", "my-article.md", &date);
        assert_eq!(result, "2025-12-12-my-article");
    }

    #[test]
    fn test_resolve_url_pattern_full_hierarchy() {
        use time::macros::datetime;
        let date = datetime!(2025-12-12 02:02:02 UTC);

        let result = resolve_url_pattern("{year}/{month}/{day}/{stem}", "my-article.md", &date);
        assert_eq!(result, "2025/12/12/my-article");
    }

    #[test]
    fn test_resolve_url_pattern_year_month() {
        use time::macros::datetime;
        let date = datetime!(2025-06-15 10:30:00 UTC);

        let result = resolve_url_pattern("{year}-{month}/{stem}", "hello-world.md", &date);
        assert_eq!(result, "2025-06/hello-world");
    }

    #[test]
    fn test_resolve_url_pattern_strips_date_from_filename() {
        use time::macros::datetime;
        // The filename has a date prefix, but we use meta.date for the pattern
        let date = datetime!(2025-12-12 02:02:02 UTC);

        let result = resolve_url_pattern("{date}-{stem}", "2024-01-15-old-article.md", &date);
        // Should use meta.date (2025-12-12) and strip date from filename
        assert_eq!(result, "2025-12-12-old-article");
    }

    #[test]
    fn test_resolve_url_pattern_date_directory() {
        use time::macros::datetime;
        let date = datetime!(2025-12-12 02:02:02 UTC);

        let result = resolve_url_pattern("{date}/{stem}", "my-article.md", &date);
        assert_eq!(result, "2025-12-12/my-article");
    }

    #[test]
    fn test_resolve_url_pattern_zero_padded_months() {
        use time::macros::datetime;
        let date = datetime!(2025-01-05 10:00:00 UTC);

        let result = resolve_url_pattern("{year}/{month}/{day}/{stem}", "post.md", &date);
        assert_eq!(result, "2025/01/05/post");
    }

    // Tests for build_output_path
    #[test]
    fn test_build_output_path_no_clean_urls() {
        let result = build_output_path("articles", "my-article", "dist", false);
        assert_eq!(result, PathBuf::from("dist/articles/my-article.html"));
    }

    #[test]
    fn test_build_output_path_with_clean_urls() {
        let result = build_output_path("articles", "my-article", "dist", true);
        assert_eq!(result, PathBuf::from("dist/articles/my-article/index.html"));
    }

    #[test]
    fn test_build_output_path_with_date_prefix_no_clean_urls() {
        let result = build_output_path("articles", "2025-12-12-my-article", "dist", false);
        assert_eq!(
            result,
            PathBuf::from("dist/articles/2025-12-12-my-article.html")
        );
    }

    #[test]
    fn test_build_output_path_with_date_prefix_clean_urls() {
        let result = build_output_path("articles", "2025-12-12-my-article", "dist", true);
        assert_eq!(
            result,
            PathBuf::from("dist/articles/2025-12-12-my-article/index.html")
        );
    }

    #[test]
    fn test_build_output_path_with_nested_pattern() {
        let result = build_output_path("articles", "2025/12/12/my-article", "dist", true);
        assert_eq!(
            result,
            PathBuf::from("dist/articles/2025/12/12/my-article/index.html")
        );
    }

    #[test]
    fn test_build_output_path_with_date_directory_no_clean_urls() {
        let result = build_output_path("articles", "2025-12-12/my-article", "dist", false);
        assert_eq!(
            result,
            PathBuf::from("dist/articles/2025-12-12/my-article.html")
        );
    }
}
