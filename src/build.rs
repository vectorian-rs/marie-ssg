// src/build.rs

use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::path::PathBuf;
use tracing::{debug, info, instrument};

use crate::asset_hash::{export_manifest_to_json, hash_static_assets};
use crate::config::Config;
use crate::content::{Content, convert_content_with_highlighting, load_content};
use crate::error::RunError;
use crate::output::{copy_static_files, write_output_file};
use crate::template::{
    create_environment_with_manifest, render_html, render_index_from_loaded_with_page,
};
use crate::utils::{
    build_output_path, find_markdown_files, get_content_type, get_content_type_template,
    resolve_url_pattern,
};
use crate::{rss, sitemap};

/// Loaded content ready for rendering
#[derive(Debug)]
pub(crate) struct LoadedContent {
    pub(crate) path: PathBuf,
    pub(crate) content: Content,
    pub(crate) html: String,
    pub(crate) content_type: String,
    pub(crate) output_path: PathBuf,
}

/// The main entry point for the application logic.
pub(crate) fn build(config_file: &str, include_drafts: bool) -> Result<(), RunError> {
    let config = Config::load_from_file(config_file)?;

    // Hash static assets if enabled
    let manifest = if config.site.asset_hashing_enabled {
        Some(hash_static_assets(
            &config.site.static_dir,
            &config.site.output_dir,
        )?)
    } else {
        None
    };

    // Export manifest to JSON if path is configured
    if let (Some(manifest), Some(path)) = (&manifest, &config.site.asset_manifest_path) {
        export_manifest_to_json(manifest, path)?;
    }

    let env = create_environment_with_manifest(&config.site.template_dir, manifest.as_ref());
    run_build(config_file, &config, &env, include_drafts)
}

/// Build with detailed tracing spans for flamechart profiling.
#[instrument(name = "build", skip_all)]
pub(crate) fn build_with_spans(config_file: &str, include_drafts: bool) -> Result<(), RunError> {
    let config = Config::load_from_file(config_file)?;

    // Hash static assets if enabled
    let manifest = if config.site.asset_hashing_enabled {
        Some(hash_static_assets(
            &config.site.static_dir,
            &config.site.output_dir,
        )?)
    } else {
        None
    };

    // Export manifest to JSON if path is configured
    if let (Some(manifest), Some(path)) = (&manifest, &config.site.asset_manifest_path) {
        export_manifest_to_json(manifest, path)?;
    }

    let env = create_environment_with_manifest(&config.site.template_dir, manifest.as_ref());
    run_build_with_spans(config_file, &config, &env, include_drafts)
}

/// Build with a fresh template environment (for watch mode).
pub(crate) fn build_fresh(config_file: &str, include_drafts: bool) -> Result<(), RunError> {
    let config = Config::load_from_file(config_file)?;

    // Hash static assets if enabled
    let manifest = if config.site.asset_hashing_enabled {
        Some(hash_static_assets(
            &config.site.static_dir,
            &config.site.output_dir,
        )?)
    } else {
        None
    };

    // Export manifest to JSON if path is configured
    if let (Some(manifest), Some(path)) = (&manifest, &config.site.asset_manifest_path) {
        export_manifest_to_json(manifest, path)?;
    }

    let env = create_environment_with_manifest(&config.site.template_dir, manifest.as_ref());
    run_build(config_file, &config, &env, include_drafts)
}

/// Get the list of file paths/directories to watch for changes.
pub(crate) fn get_paths_to_watch(config_file: &str, config: &Config) -> Vec<String> {
    vec![
        config_file.to_string(),
        config.site.content_dir.clone(),
        config.site.template_dir.clone(),
        config.site.static_dir.clone(),
    ]
}

/// Core build logic that accepts a template environment.
fn run_build(
    config_file: &str,
    config: &Config,
    env: &minijinja::Environment,
    include_drafts: bool,
) -> Result<(), RunError> {
    debug!("config::load ← {}", config_file);

    // 0. Copy static files first
    //
    copy_static_files(config)?;

    // 1. Find all markdown files in `config.content_dir`.
    //
    let files = find_markdown_files(&config.site.content_dir);
    debug!("content::scan found {} files", files.len());

    // 2. Loading all content
    //
    let start = std::time::Instant::now();

    let loaded_contents: Vec<LoadedContent> = files
        .into_par_iter() // Parallel iterator - consumes Vec for owned PathBufs
        .map(|file| -> Result<LoadedContent, RunError> {
            debug!("content::load ← {}", file.display());

            let content_type = get_content_type(&file, &config.site.content_dir);
            let content = load_content(&file)?;
            let html = convert_content_with_highlighting(
                &content,
                &file, // Pass reference - no clone needed
                config.site.syntax_highlighting_enabled,
                &config.site.syntax_highlighting_theme,
                config.site.allow_dangerous_html,
                config.site.header_uri_fragment,
            )?;

            // Get URL pattern for this content type
            // Priority: url_pattern (new) > output_naming (deprecated) > default
            let pattern = config
                .content
                .get(&content_type)
                .and_then(|ct| {
                    ct.url_pattern.clone().or_else(|| {
                        // Backwards compatibility: map output_naming to url_pattern
                        match ct.output_naming.as_deref() {
                            Some("date") => Some("{date}-{stem}".to_string()),
                            _ => None,
                        }
                    })
                })
                .unwrap_or_else(|| "{stem}".to_string());

            // Get the filename from the file path
            let filename = file.file_name().and_then(|f| f.to_str()).unwrap_or("index");

            // Resolve the URL pattern using meta.date
            let resolved = resolve_url_pattern(&pattern, filename, &content.meta.date);

            // Build the output path
            let output_path = build_output_path(
                &content_type,
                &resolved,
                &config.site.output_dir,
                config.site.clean_urls,
            );

            Ok(LoadedContent {
                path: file, // Move owned PathBuf - no clone needed
                content,
                html,
                content_type,
                output_path,
            })
        })
        .collect::<Result<Vec<_>, _>>()?; // Collect Results, fail fast on error

    // Filter out draft content unless --include-drafts is set
    let loaded_contents: Vec<LoadedContent> = if include_drafts {
        loaded_contents
    } else {
        loaded_contents
            .into_iter()
            .filter(|lc| !lc.content.meta.draft)
            .collect()
    };

    info!(
        "content::load {} files in {:.2?}",
        loaded_contents.len(),
        start.elapsed()
    );

    // 3. Write individual pages
    //
    for loaded in &loaded_contents {
        info!(
            "content::render {} → {}",
            loaded.path.display(),
            loaded.output_path.display()
        );

        let content_template = get_content_type_template(config, &loaded.content_type);
        let rendered = render_html(
            env,
            &loaded.html,
            &loaded.content.meta,
            config,
            &content_template,
            &loaded.output_path,
        )?;
        write_output_file(&loaded.output_path, &rendered)?;
    }

    // 4. Render content type indexes
    //
    for (content_type, v) in config.content.iter() {
        info!("index::render {} → {}", content_type, v.index_template);

        let filtered: Vec<_> = loaded_contents
            .iter()
            .filter(|lc| &lc.content_type == content_type)
            .collect();

        let output_path = PathBuf::from(&config.site.output_dir)
            .join(content_type)
            .join("index.html");

        let index_rendered = render_index_from_loaded_with_page(
            env,
            config,
            &v.index_template,
            filtered,
            loaded_contents.iter().collect(),
            &output_path,
        )?;
        write_output_file(&output_path, &index_rendered)?;
    }

    // 5. Render site index
    //
    let site_index_path = PathBuf::from(&config.site.output_dir).join("index.html");
    let site_index_rendered = render_index_from_loaded_with_page(
        env,
        config,
        &config.site.site_index_template,
        loaded_contents.iter().collect(),
        loaded_contents.iter().collect(),
        &site_index_path,
    )?;
    info!("index::render site → {}", site_index_path.display());
    write_output_file(&site_index_path, &site_index_rendered)?;

    // 6. Generate sitemap.xml (if enabled)
    //
    if config.site.sitemap_enabled {
        let sitemap_xml = sitemap::generate_sitemap(config, &loaded_contents);
        write_output_file(
            &PathBuf::from(&config.site.output_dir).join("sitemap.xml"),
            &sitemap_xml,
        )?;
        info!("sitemap::write → sitemap.xml");
    }

    // 7. Generate RSS feed (if enabled)
    //
    if config.site.rss_enabled {
        let rss_xml = rss::generate_rss(config, &loaded_contents);
        write_output_file(
            &PathBuf::from(&config.site.output_dir).join("feed.xml"),
            &rss_xml,
        )?;
        info!("rss::write → feed.xml");
    }

    // 8. Generate redirect HTML files (if configured)
    //
    if !config.redirects.is_empty() {
        for (from_path, to_path) in &config.redirects {
            let redirect_html =
                crate::redirect::generate_redirect_html(to_path, &config.site.domain);
            let output_path =
                crate::redirect::get_redirect_output_path(from_path, &config.site.output_dir);
            write_output_file(&output_path, &redirect_html)?;
            info!("redirect::write {} → {}", from_path, to_path);
        }
    }

    info!("build::complete ✓");
    Ok(())
}

/// Core build logic with detailed tracing spans for profiling.
#[instrument(name = "run_build", skip_all)]
fn run_build_with_spans(
    config_file: &str,
    config: &Config,
    env: &minijinja::Environment,
    include_drafts: bool,
) -> Result<(), RunError> {
    debug!("config::load ← {}", config_file);

    // 0. Copy static files first
    let _static_span = tracing::info_span!("copy_static_files").entered();
    copy_static_files(config)?;
    drop(_static_span);

    // 1. Find all markdown files
    let _scan_span = tracing::info_span!("find_markdown_files").entered();
    let files = find_markdown_files(&config.site.content_dir);
    debug!("content::scan found {} files", files.len());
    drop(_scan_span);

    // 2. Loading all content (parallel)
    let _load_span = tracing::info_span!("load_content", count = files.len()).entered();
    let start = std::time::Instant::now();

    let loaded_contents: Vec<LoadedContent> = files
        .into_par_iter()
        .map(|file| -> Result<LoadedContent, RunError> {
            let _file_span = tracing::info_span!("process_file").entered();

            let content_type = get_content_type(&file, &config.site.content_dir);
            let content = load_content(&file)?;
            let html = convert_content_with_highlighting(
                &content,
                &file,
                config.site.syntax_highlighting_enabled,
                &config.site.syntax_highlighting_theme,
                config.site.allow_dangerous_html,
                config.site.header_uri_fragment,
            )?;

            let pattern = config
                .content
                .get(&content_type)
                .and_then(|ct| {
                    ct.url_pattern
                        .clone()
                        .or_else(|| match ct.output_naming.as_deref() {
                            Some("date") => Some("{date}-{stem}".to_string()),
                            _ => None,
                        })
                })
                .unwrap_or_else(|| "{stem}".to_string());

            let filename = file.file_name().and_then(|f| f.to_str()).unwrap_or("index");
            let resolved = resolve_url_pattern(&pattern, filename, &content.meta.date);
            let output_path = build_output_path(
                &content_type,
                &resolved,
                &config.site.output_dir,
                config.site.clean_urls,
            );

            Ok(LoadedContent {
                path: file,
                content,
                html,
                content_type,
                output_path,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    drop(_load_span);

    // Filter out draft content unless --include-drafts is set
    let loaded_contents: Vec<LoadedContent> = if include_drafts {
        loaded_contents
    } else {
        loaded_contents
            .into_iter()
            .filter(|lc| !lc.content.meta.draft)
            .collect()
    };

    info!(
        "content::load {} files in {:.2?}",
        loaded_contents.len(),
        start.elapsed()
    );

    // 3. Write individual pages
    let _render_span = tracing::info_span!("render_pages", count = loaded_contents.len()).entered();
    for loaded in &loaded_contents {
        let _page_span = tracing::info_span!("render_page").entered();
        let content_template = get_content_type_template(config, &loaded.content_type);
        let rendered = render_html(
            env,
            &loaded.html,
            &loaded.content.meta,
            config,
            &content_template,
            &loaded.output_path,
        )?;
        write_output_file(&loaded.output_path, &rendered)?;
    }
    drop(_render_span);

    // 4. Render content type indexes
    let _index_span = tracing::info_span!("render_indexes").entered();
    for (content_type, v) in config.content.iter() {
        let _ct_span =
            tracing::info_span!("render_content_type_index", content_type = %content_type)
                .entered();

        let filtered: Vec<_> = loaded_contents
            .iter()
            .filter(|lc| &lc.content_type == content_type)
            .collect();

        let output_path = PathBuf::from(&config.site.output_dir)
            .join(content_type)
            .join("index.html");

        let index_rendered = render_index_from_loaded_with_page(
            env,
            config,
            &v.index_template,
            filtered,
            loaded_contents.iter().collect(),
            &output_path,
        )?;
        write_output_file(&output_path, &index_rendered)?;
    }
    drop(_index_span);

    // 5. Render site index
    let _site_index_span = tracing::info_span!("render_site_index").entered();
    let site_index_path = PathBuf::from(&config.site.output_dir).join("index.html");
    let site_index_rendered = render_index_from_loaded_with_page(
        env,
        config,
        &config.site.site_index_template,
        loaded_contents.iter().collect(),
        loaded_contents.iter().collect(),
        &site_index_path,
    )?;
    write_output_file(&site_index_path, &site_index_rendered)?;
    drop(_site_index_span);

    // 6. Generate sitemap.xml
    if config.site.sitemap_enabled {
        let _sitemap_span = tracing::info_span!("generate_sitemap").entered();
        let sitemap_xml = sitemap::generate_sitemap(config, &loaded_contents);
        write_output_file(
            &PathBuf::from(&config.site.output_dir).join("sitemap.xml"),
            &sitemap_xml,
        )?;
    }

    // 7. Generate RSS feed
    if config.site.rss_enabled {
        let _rss_span = tracing::info_span!("generate_rss").entered();
        let rss_xml = rss::generate_rss(config, &loaded_contents);
        write_output_file(
            &PathBuf::from(&config.site.output_dir).join("feed.xml"),
            &rss_xml,
        )?;
    }

    // 8. Generate redirect HTML files
    if !config.redirects.is_empty() {
        let _redirect_span =
            tracing::info_span!("generate_redirects", count = config.redirects.len()).entered();
        for (from_path, to_path) in &config.redirects {
            let redirect_html =
                crate::redirect::generate_redirect_html(to_path, &config.site.domain);
            let output_path =
                crate::redirect::get_redirect_output_path(from_path, &config.site.output_dir);
            write_output_file(&output_path, &redirect_html)?;
        }
    }

    info!("build::complete ✓");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_paths_to_watch() {
        let toml = r#"
[site]
title = "Test Site"
tagline = "A test tagline"
domain = "example.com"
author = "Test Author"
output_dir = "output"
content_dir = "content"
template_dir = "templates"
static_dir = "static"
site_index_template = "index.html"
"#;
        let config = crate::config::Config::from_str(toml).unwrap();
        let config_file = "site.toml";

        let paths = get_paths_to_watch(config_file, &config);

        // Should contain 4 paths: config file + 3 dirs
        assert_eq!(paths.len(), 4);
        assert!(paths.contains(&"site.toml".to_string()));
        assert!(paths.contains(&"content".to_string()));
        assert!(paths.contains(&"templates".to_string()));
        assert!(paths.contains(&"static".to_string()));
    }
}
