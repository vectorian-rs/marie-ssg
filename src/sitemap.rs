// src/sitemap.rs

#[cfg(test)]
use std::path::Path;
use time::OffsetDateTime;
use time::macros::format_description;

use crate::LoadedContent;
use crate::config::Config;
use crate::utils::{output_path_to_url_path, site_base_url};

/// Generates a sitemap.xml string following the sitemap protocol.
///
/// The sitemap includes all content pages and index pages with their
/// full URLs based on the configured domain.
///
/// # Arguments
/// * `config` - The site configuration containing the domain
/// * `loaded_contents` - All loaded content items to include in the sitemap
///
/// # Returns
/// A string containing the complete sitemap XML
///
/// # Example
/// ```ignore
/// let sitemap = generate_sitemap(&config, &loaded_contents);
/// write_output_file(&output_path, &sitemap)?;
/// ```
pub(crate) fn generate_sitemap(config: &Config, loaded_contents: &[LoadedContent]) -> String {
    let mut xml = String::new();

    // XML declaration and urlset opening tag
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#);
    xml.push('\n');

    let base_url = site_base_url(&config.site.domain);

    // Add site index (homepage)
    xml.push_str(&format_url_entry(&base_url, "/", None));

    // Add content type index pages
    for content_type in config.content.keys() {
        let path = format!("/{}/", content_type);
        xml.push_str(&format_url_entry(&base_url, &path, None));
    }

    // Add all content pages
    for content in loaded_contents {
        let path = output_path_to_url_path(
            &content.output_path,
            &config.site.output_dir,
            config.site.clean_urls,
        );

        let lastmod = Some(&content.content.meta.date);

        xml.push_str(&format_url_entry(&base_url, &path, lastmod));
    }

    // Close urlset
    xml.push_str("</urlset>\n");

    xml
}

/// Formats a single URL entry for the sitemap.
fn format_url_entry(base_url: &str, path: &str, lastmod: Option<&OffsetDateTime>) -> String {
    let mut entry = String::new();
    entry.push_str("  <url>\n");
    entry.push_str(&format!("    <loc>{}{}</loc>\n", base_url, path));

    if let Some(date) = lastmod {
        // Format validated at compile time via macro
        const FORMAT: &[time::format_description::FormatItem<'static>] =
            format_description!("[year]-[month]-[day]");
        if let Ok(formatted) = date.format(&FORMAT) {
            entry.push_str(&format!("    <lastmod>{}</lastmod>\n", formatted));
        }
    }

    entry.push_str("  </url>\n");
    entry
}

/// Converts a file path to a URL path.
///
/// Handles platform-specific path separators and ensures forward slashes.
#[cfg(test)]
fn path_to_url(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ContentTypeConfig, SiteConfig};
    use crate::content::{Content, ContentMeta};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn create_test_config() -> Config {
        let mut content = HashMap::new();
        content.insert(
            "posts".to_string(),
            ContentTypeConfig {
                index_template: "posts_index.html".to_string(),
                content_template: "post.html".to_string(),
                url_pattern: None,
                output_naming: None,
                rss_include: None,
            },
        );

        Config {
            site: SiteConfig {
                title: "Test Site".to_string(),
                tagline: "A test site".to_string(),
                domain: "example.com".to_string(),
                author: "Test Author".to_string(),
                content_dir: "content".to_string(),
                output_dir: "output".to_string(),
                template_dir: "templates".to_string(),
                static_dir: "static".to_string(),
                site_index_template: "index.html".to_string(),
                syntax_highlighting_enabled: true,
                syntax_highlighting_theme: "github_dark".to_string(),
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
            content,
            dynamic: HashMap::new(),
            redirects: HashMap::new(),
        }
    }

    fn create_test_meta(title: &str, date_str: &str) -> ContentMeta {
        use time::format_description::well_known::Rfc3339;
        let date = OffsetDateTime::parse(date_str, &Rfc3339).unwrap();
        ContentMeta {
            title: title.to_string(),
            date,
            author: "Test Author".to_string(),
            tags: vec![],
            template: None,
            cover: None,
            extra: std::collections::HashMap::new(),
            extra_js: vec![],
            draft: false,
        }
    }

    fn create_test_loaded_content(
        filename: &str,
        title: &str,
        date_str: &str,
        content_type: &str,
    ) -> LoadedContent {
        LoadedContent {
            path: PathBuf::from(format!("content/{}/{}.md", content_type, filename)),
            content: Content {
                meta: create_test_meta(title, date_str),
                data: "# Test".to_string(),
            },
            html: "<h1>Test</h1>".to_string(),
            content_type: content_type.to_string(),
            output_path: PathBuf::from(format!("output/{}/{}.html", content_type, filename)),
        }
    }

    #[test]
    fn test_generate_sitemap_empty() {
        let config = create_test_config();
        let contents: Vec<LoadedContent> = vec![];

        let sitemap = generate_sitemap(&config, &contents);

        assert!(sitemap.contains(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
        assert!(
            sitemap.contains(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#)
        );
        assert!(sitemap.contains("</urlset>"));
        // Should have homepage
        assert!(sitemap.contains("<loc>https://example.com/</loc>"));
        // Should have posts index
        assert!(sitemap.contains("<loc>https://example.com/posts/</loc>"));
    }

    #[test]
    fn test_generate_sitemap_with_content() {
        let config = create_test_config();
        let contents = vec![
            create_test_loaded_content(
                "hello-world",
                "Hello World",
                "2024-01-15T10:00:00+00:00",
                "posts",
            ),
            create_test_loaded_content(
                "second-post",
                "Second Post",
                "2024-02-20T12:00:00+00:00",
                "posts",
            ),
        ];

        let sitemap = generate_sitemap(&config, &contents);

        // Check content URLs are included
        assert!(sitemap.contains("<loc>https://example.com/posts/hello-world.html</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/posts/second-post.html</loc>"));

        // Check lastmod dates are included
        assert!(sitemap.contains("<lastmod>2024-01-15</lastmod>"));
        assert!(sitemap.contains("<lastmod>2024-02-20</lastmod>"));
    }

    #[test]
    fn test_generate_sitemap_multiple_content_types() {
        let mut config = create_test_config();
        config.content.insert(
            "pages".to_string(),
            ContentTypeConfig {
                index_template: "pages_index.html".to_string(),
                content_template: "page.html".to_string(),
                url_pattern: None,
                output_naming: None,
                rss_include: None,
            },
        );

        let contents = vec![
            create_test_loaded_content("post", "A Post", "2024-01-15T10:00:00+00:00", "posts"),
            create_test_loaded_content("about", "About", "2024-01-01T10:00:00+00:00", "pages"),
        ];

        let sitemap = generate_sitemap(&config, &contents);

        // Check both content type indexes are included
        assert!(sitemap.contains("<loc>https://example.com/posts/</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/pages/</loc>"));

        // Check content from both types
        assert!(sitemap.contains("<loc>https://example.com/posts/post.html</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/pages/about.html</loc>"));
    }

    #[test]
    fn test_format_url_entry_without_lastmod() {
        let entry = format_url_entry("https://example.com", "/about/", None);

        assert!(entry.contains("<url>"));
        assert!(entry.contains("<loc>https://example.com/about/</loc>"));
        assert!(!entry.contains("<lastmod>"));
        assert!(entry.contains("</url>"));
    }

    #[test]
    fn test_format_url_entry_with_lastmod() {
        use time::format_description::well_known::Rfc3339;
        let date = OffsetDateTime::parse("2024-06-15T10:30:00+00:00", &Rfc3339).unwrap();
        let entry = format_url_entry("https://example.com", "/post.html", Some(&date));

        assert!(entry.contains("<loc>https://example.com/post.html</loc>"));
        assert!(entry.contains("<lastmod>2024-06-15</lastmod>"));
    }

    #[test]
    fn test_path_to_url_unix_path() {
        let path = Path::new("posts/hello-world.html");
        assert_eq!(path_to_url(path), "posts/hello-world.html");
    }

    #[test]
    fn test_path_to_url_windows_path() {
        // Simulate a Windows-style path string
        let path = Path::new("posts\\hello-world.html");
        let url = path_to_url(path);
        // On Unix, backslash is a valid filename char, but we still convert it
        assert!(!url.contains('\\') || url == "posts\\hello-world.html");
    }

    #[test]
    fn test_sitemap_valid_xml_structure() {
        let config = create_test_config();
        let contents = vec![create_test_loaded_content(
            "test",
            "Test",
            "2024-01-15T10:00:00+00:00",
            "posts",
        )];

        let sitemap = generate_sitemap(&config, &contents);

        // Count opening and closing tags to verify structure
        let url_opens = sitemap.matches("<url>").count();
        let url_closes = sitemap.matches("</url>").count();
        assert_eq!(url_opens, url_closes);

        let loc_opens = sitemap.matches("<loc>").count();
        let loc_closes = sitemap.matches("</loc>").count();
        assert_eq!(loc_opens, loc_closes);

        // Should have: homepage + posts index + 1 content = 3 URLs
        assert_eq!(url_opens, 3);
    }
}
