// src/template.rs

use minijinja::{Environment, State, Value, path_loader};
use minijinja_contrib::add_to_environment;
use std::path::Path;
use time::OffsetDateTime;
use time::macros::format_description;
use tracing::instrument;

use crate::{
    asset_hash::AssetManifest,
    config::Config,
    content::{ContentItem, ContentMeta, PageInfo, get_excerpt_html},
    utils::{absolute_url, output_path_to_relative_url, output_path_to_url_path},
};

/// Format a date as "Month Day, Year" (e.g., "January 15, 2024")
fn format_date_long(date: &OffsetDateTime) -> String {
    // Format validated at compile time via macro
    const FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[month repr:long] [day], [year]");
    date.format(&FORMAT)
        .unwrap_or_else(|_| "Invalid date".to_string())
}

/// Filter to mark URL paths as safe for HTML rendering.
///
/// Minijinja's default HTML escaping converts forward slashes to `&#x2f;`
/// which breaks URL paths in href attributes. This filter marks the value
/// as safe, bypassing auto-escaping for URL paths.
///
/// Usage in templates: `{{ item.filename | url }}`
fn url_filter(value: &str) -> Value {
    Value::from_safe_string(value.to_string())
}

/// Filter to resolve asset paths to their hashed versions.
///
/// When asset hashing is enabled, this filter looks up the original path
/// in the asset manifest and returns the hashed URL. If not found or
/// hashing is disabled, returns the original path normalized.
///
/// Usage in templates: `{{ "static/css/style.css" | asset_hash }}`
fn asset_hash_filter(state: &State, path: &str) -> Value {
    // Normalize path for lookup: remove leading "/" and "static/"
    let normalized = path.trim_start_matches('/').trim_start_matches("static/");

    // Try to get the manifest from globals
    if let Some(manifest_value) = state.lookup("_asset_manifest")
        && let Ok(hashed) = manifest_value.get_item(&Value::from(normalized))
        && !hashed.is_undefined()
        && let Some(s) = hashed.as_str()
    {
        return Value::from_safe_string(s.to_string());
    }

    // Fallback: return normalized path
    let result = if path.starts_with("/static/") {
        path.to_string()
    } else if path.starts_with("static/") {
        format!("/{}", path)
    } else {
        format!("/static/{}", path)
    };
    Value::from_safe_string(result)
}

/// Configure common environment settings (filters, contrib functions)
fn configure_environment(env: &mut Environment<'static>) {
    add_to_environment(env);
    env.add_filter("url", url_filter);
    env.add_filter("asset_hash", asset_hash_filter);
}

/// Create a template environment with optional asset manifest.
///
/// This is the primary way to create environments for builds.
/// If a manifest is provided, it will be available to the asset_hash filter.
pub(crate) fn create_environment_with_manifest(
    template_dir: &str,
    manifest: Option<&AssetManifest>,
) -> Environment<'static> {
    let mut env = Environment::new();
    env.set_loader(path_loader(template_dir));
    configure_environment(&mut env);

    // Add manifest as a global if provided
    if let Some(m) = manifest {
        // Convert HashMap to a Value object for lookup
        env.add_global("_asset_manifest", Value::from_serialize(m));
    }

    env
}

/// Build page URL information for a rendered output path.
fn build_page_info_with_clean_urls(
    output_path: &Path,
    config: &Config,
    clean_urls: bool,
) -> PageInfo {
    let filename = output_path_to_relative_url(output_path, &config.site.output_dir, clean_urls);
    let url = output_path_to_url_path(output_path, &config.site.output_dir, clean_urls);
    let canonical_url = absolute_url(&config.site.domain, &url);

    PageInfo {
        filename,
        url,
        permalink: canonical_url.clone(),
        canonical_url,
    }
}

fn build_page_info(output_path: &Path, config: &Config) -> PageInfo {
    build_page_info_with_clean_urls(output_path, config, config.site.clean_urls)
}

fn build_index_page_info(output_path: &Path, config: &Config) -> PageInfo {
    // Index templates are always public directory URLs: /, /blog/, etc.
    build_page_info_with_clean_urls(output_path, config, true)
}

/// Build a MiniJinja page object whose URL fields are marked safe for attributes.
fn build_page_value(page: &PageInfo) -> Value {
    Value::from_iter([
        ("filename", Value::from_safe_string(page.filename.clone())),
        ("url", Value::from_safe_string(page.url.clone())),
        ("permalink", Value::from_safe_string(page.permalink.clone())),
        (
            "canonical_url",
            Value::from_safe_string(page.canonical_url.clone()),
        ),
    ])
}

/// Build a ContentItem from LoadedContent for template rendering.
///
/// This helper extracts the common logic for building template-ready content items,
/// computing URL information, excerpt, and formatted date.
fn build_content_item(lc: &crate::LoadedContent, config: &Config) -> ContentItem {
    let page = build_page_info(&lc.output_path, config);

    let excerpt = get_excerpt_html(
        &lc.content.data,
        "## Context",
        config.site.allow_dangerous_html,
    );

    ContentItem {
        html: lc.html.clone(),
        meta: lc.content.meta.clone(),
        formatted_date: format_date_long(&lc.content.meta.date),
        filename: page.filename,
        url: page.url,
        permalink: page.permalink,
        canonical_url: page.canonical_url,
        content_type: lc.content_type.clone(),
        excerpt,
    }
}

#[cfg(test)]
#[instrument(skip_all)]
pub(crate) fn render_index_from_loaded(
    env: &Environment,
    config: &Config,
    index_template_name: &str,
    loaded: Vec<&crate::LoadedContent>,
    all_content: Vec<&crate::LoadedContent>,
) -> Result<String, minijinja::Error> {
    render_index_from_loaded_impl(env, config, index_template_name, loaded, all_content, None)
}

#[instrument(skip_all)]
pub(crate) fn render_index_from_loaded_with_page(
    env: &Environment,
    config: &Config,
    index_template_name: &str,
    loaded: Vec<&crate::LoadedContent>,
    all_content: Vec<&crate::LoadedContent>,
    page_output_path: &Path,
) -> Result<String, minijinja::Error> {
    render_index_from_loaded_impl(
        env,
        config,
        index_template_name,
        loaded,
        all_content,
        Some(page_output_path),
    )
}

fn render_index_from_loaded_impl(
    env: &Environment,
    config: &Config,
    index_template_name: &str,
    loaded: Vec<&crate::LoadedContent>,
    all_content: Vec<&crate::LoadedContent>,
    page_output_path: Option<&Path>,
) -> Result<String, minijinja::Error> {
    let tmpl = env.get_template(index_template_name)?;

    let mut contents: Vec<ContentItem> = loaded
        .iter()
        .map(|lc| build_content_item(lc, config))
        .collect();
    contents.sort_by(|a, b| b.meta.date.cmp(&a.meta.date));

    let mut all_contents: Vec<ContentItem> = all_content
        .iter()
        .map(|lc| build_content_item(lc, config))
        .collect();
    all_contents.sort_by(|a, b| b.meta.date.cmp(&a.meta.date));

    let page = page_output_path
        .map(|path| build_page_value(&build_index_page_info(path, config)))
        .unwrap_or(Value::UNDEFINED);

    let context = Value::from_iter([
        ("config", Value::from_serialize(config)),
        ("contents", Value::from_serialize(&contents)),
        ("all_content", Value::from_serialize(&all_contents)),
        ("page", page),
    ]);

    tmpl.render(context)
}

#[instrument(skip_all)]
pub(crate) fn render_html(
    env: &Environment,
    html: &str,
    meta: &ContentMeta,
    config: &Config,
    content_template: &str,
    output_path: &Path,
) -> Result<String, minijinja::Error> {
    let tmpl = env.get_template(content_template)?;
    let page = build_page_info(output_path, config);

    let context = Value::from_iter([
        ("content", Value::from(html)),
        ("meta", Value::from_serialize(meta)),
        ("config", Value::from_serialize(config)),
        ("page", build_page_value(&page)),
    ]);

    tmpl.render(context)
}

// Note: Template rendering functions accept an Environment parameter for testability.
// Production code uses init_environment() to get a static singleton, while tests
// can create custom environments with test-specific template directories.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LoadedContent;
    use crate::config::{Config, SiteConfig};
    use crate::content::ContentMeta;
    use minijinja::Environment;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use time::macros::datetime;

    /// Helper to create a test Config
    fn create_test_config(template_dir: &str, output_dir: &str) -> Config {
        Config {
            site: SiteConfig {
                title: "Test Site".to_string(),
                tagline: "A test tagline".to_string(),
                domain: "example.com".to_string(),
                author: "Test Author".to_string(),
                content_dir: "content".to_string(),
                output_dir: output_dir.to_string(),
                template_dir: template_dir.to_string(),
                static_dir: "static".to_string(),
                site_index_template: "index.html".to_string(),
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
            content: HashMap::new(),
            dynamic: HashMap::new(),
            redirects: HashMap::new(),
        }
    }

    /// Helper to create a test ContentMeta
    fn create_test_meta() -> ContentMeta {
        ContentMeta {
            title: "Test Article".to_string(),
            date: datetime!(2024-01-15 10:00:00 -5),
            author: "Test Author".to_string(),
            tags: vec!["rust".to_string(), "testing".to_string()],
            template: None,
            cover: None,
            extra: std::collections::HashMap::new(),
            extra_js: vec![],
            draft: false,
        }
    }

    #[test]
    fn test_render_html_with_simple_template() {
        // Create a temporary directory for templates
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("test.html");

        // Write a simple template - use 'safe' filter to render unescaped HTML
        std::fs::write(
            &template_path,
            "<h1>{{ meta.title }}</h1><div>{{ content | safe }}</div>",
        )
        .unwrap();

        // Create environment and config
        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        add_to_environment(&mut env);
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        // Test rendering
        let meta = create_test_meta();
        let html = "<p>Test content</p>";
        let result = render_html(
            &env,
            html,
            &meta,
            &config,
            "test.html",
            &PathBuf::from("output/blog/test.html"),
        );

        assert!(result.is_ok());
        let rendered = result.unwrap();
        assert!(rendered.contains("<h1>Test Article</h1>"));
        assert!(rendered.contains("<p>Test content</p>"));
    }

    #[test]
    fn test_render_html_with_metadata_fields() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("full.html");

        std::fs::write(
            &template_path,
            r#"<article>
                <h1>{{ meta.title }}</h1>
                <p>By {{ meta.author }}</p>
                <div>{{ content | safe }}</div>
                <p>Tags: {{ meta.tags | join(", ") }}</p>
            </article>"#,
        )
        .unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        add_to_environment(&mut env);
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        let meta = create_test_meta();
        let result = render_html(
            &env,
            "<p>Body</p>",
            &meta,
            &config,
            "full.html",
            &PathBuf::from("output/blog/test.html"),
        );

        assert!(result.is_ok());
        let rendered = result.unwrap();
        assert!(rendered.contains("Test Article"));
        assert!(rendered.contains("Test Author"));
        assert!(rendered.contains("rust, testing"));
    }

    #[test]
    fn test_render_html_exposes_page_url_fields() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("page.html");

        std::fs::write(
            &template_path,
            r#"<link rel="canonical" href="{{ page.canonical_url }}">
<meta property="og:url" content="{{ page.permalink }}">
<span data-url="{{ page.url }}" data-filename="{{ page.filename }}"></span>"#,
        )
        .unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");
        let meta = create_test_meta();

        let rendered = render_html(
            &env,
            "<p>Body</p>",
            &meta,
            &config,
            "page.html",
            &PathBuf::from("output/blog/test.html"),
        )
        .unwrap();

        assert!(rendered.contains(r#"href="https://example.com/blog/test.html""#));
        assert!(rendered.contains(r#"content="https://example.com/blog/test.html""#));
        assert!(rendered.contains(r#"data-url="/blog/test.html""#));
        assert!(rendered.contains(r#"data-filename="blog/test.html""#));
        assert!(
            !rendered.contains("&#x2f;"),
            "URL fields should render safely: {rendered}"
        );
    }

    #[test]
    fn test_render_html_exposes_clean_page_url_fields() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("page.html");
        std::fs::write(
            &template_path,
            "{{ page.url }}|{{ page.filename }}|{{ page.canonical_url }}",
        )
        .unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        let mut config = create_test_config(temp_dir.path().to_str().unwrap(), "output");
        config.site.clean_urls = true;
        let meta = create_test_meta();

        let rendered = render_html(
            &env,
            "<p>Body</p>",
            &meta,
            &config,
            "page.html",
            &PathBuf::from("output/blog/test/index.html"),
        )
        .unwrap();

        assert_eq!(
            rendered,
            "/blog/test/|blog/test/|https://example.com/blog/test/"
        );
    }

    #[test]
    fn test_render_index_exposes_directory_page_url_fields() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("index.html");
        std::fs::write(
            &template_path,
            "{{ page.url }}|{{ page.filename }}|{{ page.canonical_url }}",
        )
        .unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        let content_type_index = render_index_from_loaded_with_page(
            &env,
            &config,
            "index.html",
            vec![],
            vec![],
            &PathBuf::from("output/blog/index.html"),
        )
        .unwrap();
        assert_eq!(content_type_index, "/blog/|blog/|https://example.com/blog/");

        let site_index = render_index_from_loaded_with_page(
            &env,
            &config,
            "index.html",
            vec![],
            vec![],
            &PathBuf::from("output/index.html"),
        )
        .unwrap();
        assert_eq!(site_index, "/||https://example.com/");
    }

    #[test]
    fn test_datetimeformat_filter_available() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("date.html");

        std::fs::write(&template_path, "<p>{{ meta.date | datetimeformat }}</p>").unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        add_to_environment(&mut env);

        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        let meta = create_test_meta();
        let result = render_html(
            &env,
            "<p>Body</p>",
            &meta,
            &config,
            "date.html",
            &PathBuf::from("output/blog/test.html"),
        );

        let rendered = result.expect("datetimeformat filter should render");
        assert!(rendered.contains("Jan 15 2024"));
    }

    #[test]
    fn test_render_html_missing_template() {
        let temp_dir = TempDir::new().unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        let meta = create_test_meta();
        let result = render_html(
            &env,
            "<p>Test</p>",
            &meta,
            &config,
            "nonexistent.html",
            &PathBuf::from("output/blog/test.html"),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nonexistent.html"));
    }

    #[test]
    fn test_render_index_from_loaded_empty_list() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("index.html");

        std::fs::write(
            &template_path,
            "<h1>{{ config.site.title }}</h1><p>{{ contents | length }} items</p>",
        )
        .unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        let result = render_index_from_loaded(&env, &config, "index.html", vec![], vec![]);

        assert!(result.is_ok());
        let rendered = result.unwrap();
        assert!(rendered.contains("Test Site"));
        assert!(rendered.contains("0 items"));
    }

    #[test]
    fn test_render_index_from_loaded_with_content() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("index.html");

        std::fs::write(
            &template_path,
            r#"<h1>{{ config.site.title }}</h1>
            {% for item in contents %}
            <article>
                <h2>{{ item.meta.title }}</h2>
                <p>{{ item.formatted_date }}</p>
            </article>
            {% endfor %}"#,
        )
        .unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        // Create test LoadedContent
        let loaded = LoadedContent {
            path: PathBuf::from("test.md"),
            content: crate::content::Content {
                meta: create_test_meta(),
                data: "# Test".to_string(),
            },
            html: "<h1>Test</h1>".to_string(),
            content_type: "blog".to_string(),
            output_path: PathBuf::from("output/blog/test.html"),
        };

        let result =
            render_index_from_loaded(&env, &config, "index.html", vec![&loaded], vec![&loaded]);

        assert!(result.is_ok());
        let rendered = result.unwrap();
        assert!(rendered.contains("Test Site"));
        assert!(rendered.contains("Test Article"));
        assert!(rendered.contains("January 15, 2024"));
    }

    #[test]
    fn test_render_index_sorts_by_date_descending() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("index.html");

        std::fs::write(
            &template_path,
            r#"{% for item in contents %}{{ item.meta.title }},{% endfor %}"#,
        )
        .unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        // Create three LoadedContent items with different dates
        let mut meta_old = create_test_meta();
        meta_old.title = "Old Post".to_string();
        meta_old.date = datetime!(2024-01-01 10:00:00 -5);

        let mut meta_new = create_test_meta();
        meta_new.title = "New Post".to_string();
        meta_new.date = datetime!(2024-12-15 10:00:00 -5);

        let mut meta_mid = create_test_meta();
        meta_mid.title = "Mid Post".to_string();
        meta_mid.date = datetime!(2024-06-15 10:00:00 -5);

        let loaded_old = LoadedContent {
            path: PathBuf::from("old.md"),
            content: crate::content::Content {
                meta: meta_old,
                data: "# Old".to_string(),
            },
            html: "<h1>Old</h1>".to_string(),
            content_type: "blog".to_string(),
            output_path: PathBuf::from("output/blog/old.html"),
        };

        let loaded_new = LoadedContent {
            path: PathBuf::from("new.md"),
            content: crate::content::Content {
                meta: meta_new,
                data: "# New".to_string(),
            },
            html: "<h1>New</h1>".to_string(),
            content_type: "blog".to_string(),
            output_path: PathBuf::from("output/blog/new.html"),
        };

        let loaded_mid = LoadedContent {
            path: PathBuf::from("mid.md"),
            content: crate::content::Content {
                meta: meta_mid,
                data: "# Mid".to_string(),
            },
            html: "<h1>Mid</h1>".to_string(),
            content_type: "blog".to_string(),
            output_path: PathBuf::from("output/blog/mid.html"),
        };

        // Pass in non-sorted order
        let result = render_index_from_loaded(
            &env,
            &config,
            "index.html",
            vec![&loaded_old, &loaded_new, &loaded_mid],
            vec![&loaded_old, &loaded_new, &loaded_mid],
        );

        assert!(result.is_ok());
        let rendered = result.unwrap();
        // Should be sorted newest first: New, Mid, Old
        assert_eq!(rendered, "New Post,Mid Post,Old Post,");
    }

    #[test]
    fn test_render_index_with_excerpt() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("index.html");

        std::fs::write(
            &template_path,
            r#"{% for item in contents %}<div>{{ item.excerpt }}</div>{% endfor %}"#,
        )
        .unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        let loaded = LoadedContent {
            path: PathBuf::from("test.md"),
            content: crate::content::Content {
                meta: create_test_meta(),
                data: "# Title\n\n## Context\n\nThis is the excerpt.".to_string(),
            },
            html: "<h1>Title</h1>".to_string(),
            content_type: "blog".to_string(),
            output_path: PathBuf::from("output/blog/test.html"),
        };

        let result =
            render_index_from_loaded(&env, &config, "index.html", vec![&loaded], vec![&loaded]);

        assert!(result.is_ok());
        let rendered = result.unwrap();
        assert!(rendered.contains("This is the excerpt"));
    }

    #[test]
    fn test_render_index_filename_not_escaped_with_url_filter() {
        // Test that the `url` filter prevents forward slash escaping in URL paths.
        // Without the filter, minijinja escapes "/" to "&#x2f;" in href attributes.
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("index.html");

        // Template with href using filename with the `url` filter
        std::fs::write(
            &template_path,
            r#"{% for item in contents %}<a href="/{{ item.filename | url }}">{{ item.meta.title }}</a>{% endfor %}"#,
        )
        .unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        add_to_environment(&mut env);
        env.add_filter("url", url_filter);
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        let loaded = LoadedContent {
            path: PathBuf::from("test.md"),
            content: crate::content::Content {
                meta: create_test_meta(),
                data: "# Test".to_string(),
            },
            html: "<h1>Test</h1>".to_string(),
            content_type: "articles".to_string(),
            output_path: PathBuf::from("output/articles/2024-01-15-test.html"),
        };

        let result =
            render_index_from_loaded(&env, &config, "index.html", vec![&loaded], vec![&loaded]);

        assert!(result.is_ok());
        let rendered = result.unwrap();

        // With the url filter, forward slash MUST NOT be escaped to &#x2f;
        assert!(
            rendered.contains("href=\"/articles/2024-01-15-test.html\""),
            "Forward slash was incorrectly escaped. Got: {}",
            rendered
        );
        assert!(
            !rendered.contains("&#x2f;"),
            "Found escaped forward slash (&#x2f;) in output: {}",
            rendered
        );
    }

    #[test]
    fn test_render_index_filename_escaped_without_url_filter() {
        // Test demonstrating the default escaping behavior without the url filter.
        // This documents the issue that the url filter fixes.
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("index.html");

        // Template WITHOUT the url filter
        std::fs::write(
            &template_path,
            r#"{% for item in contents %}<a href="/{{ item.filename }}">link</a>{% endfor %}"#,
        )
        .unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        add_to_environment(&mut env);
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        let loaded = LoadedContent {
            path: PathBuf::from("test.md"),
            content: crate::content::Content {
                meta: create_test_meta(),
                data: "# Test".to_string(),
            },
            html: "<h1>Test</h1>".to_string(),
            content_type: "articles".to_string(),
            output_path: PathBuf::from("output/articles/test.html"),
        };

        let result =
            render_index_from_loaded(&env, &config, "index.html", vec![&loaded], vec![&loaded]);

        assert!(result.is_ok());
        let rendered = result.unwrap();

        // Without the url filter, minijinja escapes the forward slash
        assert!(
            rendered.contains("&#x2f;"),
            "Expected escaped forward slash without url filter. Got: {}",
            rendered
        );
    }

    #[test]
    fn test_render_index_with_all_content() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("index.html");

        std::fs::write(
            &template_path,
            r#"<h1>{{ config.site.title }}</h1>
            <p>Filtered: {{ contents | length }} items</p>
            <p>All: {{ all_content | length }} items</p>
            {% for item in all_content %}
            <span class="all-item">{{ item.meta.title }}</span>
            {% endfor %}"#,
        )
        .unwrap();

        let mut env = Environment::new();
        env.set_loader(path_loader(temp_dir.path()));
        let config = create_test_config(temp_dir.path().to_str().unwrap(), "output");

        // Create two test LoadedContent items
        let mut meta1 = create_test_meta();
        meta1.title = "First Post".to_string();
        meta1.date = datetime!(2024-01-15 10:00:00 -5);

        let mut meta2 = create_test_meta();
        meta2.title = "Second Post".to_string();
        meta2.date = datetime!(2024-02-15 10:00:00 -5);

        let loaded1 = LoadedContent {
            path: PathBuf::from("first.md"),
            content: crate::content::Content {
                meta: meta1,
                data: "# First".to_string(),
            },
            html: "<h1>First</h1>".to_string(),
            content_type: "blog".to_string(),
            output_path: PathBuf::from("output/blog/first.html"),
        };

        let loaded2 = LoadedContent {
            path: PathBuf::from("second.md"),
            content: crate::content::Content {
                meta: meta2,
                data: "# Second".to_string(),
            },
            html: "<h1>Second</h1>".to_string(),
            content_type: "blog".to_string(),
            output_path: PathBuf::from("output/blog/second.html"),
        };

        // Pass only first item in filtered list, but both in all_content
        let result = render_index_from_loaded(
            &env,
            &config,
            "index.html",
            vec![&loaded1],
            vec![&loaded1, &loaded2],
        );

        assert!(result.is_ok());
        let rendered = result.unwrap();
        assert!(rendered.contains("Filtered: 1 items"));
        assert!(rendered.contains("All: 2 items"));
        assert!(rendered.contains("First Post"));
        assert!(rendered.contains("Second Post"));
    }
}
