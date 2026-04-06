## Project Overview

**Marie SSG** is a static site generator written in Rust that converts markdown files with TOML metadata into HTML pages using Jinja-style templates. It follows a pipeline architecture: load content in parallel, render through templates, write output.

**Key characteristics:**

- Single-purpose tool focused on doing one thing well
- Parallel content loading with Rayon
- Jinja-style templating with Minijinja
- Syntax highlighting with Autumnus (10 languages)
- Watch mode support on macOS

## Development Environment

### Prerequisites

Install [mise](https://mise.jdx.dev/) for tool version management:

```bash
# macOS (Homebrew)
brew install mise

# Or using the install script
curl https://mise.run | sh
```

### Tooling Setup

The project uses **mise** for task management and tool versioning. See `mise.toml` for the complete task reference.

```bash
# Install required tools (Rust)
mise install

# Verify installation
mise run verify
```

**Required tools (automatically installed by mise):**

- Rust 1.90.0

**Key mise tasks:**

```bash
mise run fmt          # Format code with cargo fmt
mise run lint         # Lint with Clippy (fails on warnings)
mise run tests        # Run all tests with output
mise run verify       # Full verification (lint + tests)
mise run coverage     # Run tests with coverage (requires cargo-tarpaulin)
mise run build-dev    # Build development version
mise run build-prod   # Build release version
mise run audit        # Security audit on dependencies
mise run check-deps   # Run audit + find unused dependencies
```

## Configuration

See [examples/site.toml](examples/site.toml) for a complete configuration reference.

### Key Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `clean_urls` | bool | `false` | Output as `post/index.html` for SEO-friendly URLs |
| `rss_enabled` | bool | `true` | Generate RSS feed (feed.xml) |
| `sitemap_enabled` | bool | `true` | Generate sitemap.xml |
| `header_uri_fragment` | bool | `false` | Add anchor links to headers |
| `allow_dangerous_html` | bool | `false` | Allow raw HTML in markdown |
| `syntax_highlighting_enabled` | bool | `true` | Enable code syntax highlighting |

### URL Output Formats

The URL format is controlled by `url_pattern` (per content type) and `clean_urls` (site-level).

**Data Sources:**

| Component | Source |
|-----------|--------|
| `{stem}` | filename stem (without extension) |
| `{date}`, `{year}`, `{month}`, `{day}` | meta.date from `.meta.toml` |
| content type directory | parent directory of content file |

**Example Input:**
```
File: content/articles/agentic-project-management.md
meta.date: 2025-12-12T02:02:02Z
```

**Output Examples:**

| `url_pattern` | `clean_urls` | Output |
|---------------|--------------|--------|
| `{stem}` | `false` | `/articles/agentic-project-management.html` |
| `{stem}` | `true` | `/articles/agentic-project-management/index.html` |
| `{date}-{stem}` | `false` | `/articles/2025-12-12-agentic-project-management.html` |
| `{date}-{stem}` | `true` | `/articles/2025-12-12-agentic-project-management/index.html` |
| `{date}/{stem}` | `true` | `/articles/2025-12-12/agentic-project-management/index.html` |
| `{year}/{month}/{day}/{stem}` | `true` | `/articles/2025/12/12/agentic-project-management/index.html` |

```toml
[site]
clean_urls = true

[content.articles]
index_template = "articles_index.html"
content_template = "article.html"
url_pattern = "{date}-{stem}"  # Flexible URL pattern
```

**Backwards Compatibility:** `output_naming = "date"` maps to `url_pattern = "{date}-{stem}"`

### URL Redirects

Configure explicit URL redirects for migrations, renames, or restructures:

```toml
[redirects]
"/articles/old-slug/" = "/articles/2025-12-29-new-slug/"
"/blog/legacy-post/" = "/articles/legacy-post/"
"/about-us/" = "/about/"
```

Generates HTML redirect files with meta-refresh at each "from" path, redirecting to the "to" URL. Works on any static hosting (S3, CloudFront, GitHub Pages, nginx).

## Examples

See [examples](examples/)

## Context

See [agents](AGENTS.md)

## Version History

### v1.5.0 (2026-01-13)

- Added draft content support
  - Set `draft = true` in content metadata to exclude from builds
  - Use `--include-drafts` flag with build/watch to include draft content
  - Enables work-in-progress content without publishing

### v1.4.0 (2026-01-10)

- Added flexible `url_pattern` system for URL customization
  - Placeholders: `{stem}`, `{date}`, `{year}`, `{month}`, `{day}`
  - Date from meta.date, stem from filename
  - Examples: `{date}-{stem}`, `{year}/{month}/{day}/{stem}`
  - Backwards compatible: `output_naming = "date"` maps to `{date}-{stem}`
- Added `[redirects]` config section for explicit URL redirects
  - Map old URLs to new URLs for migrations
  - Generates HTML meta-refresh redirect files
  - Works on any static hosting platform
- Added `marie-ssg flame` command for performance profiling
  - Generates interactive flamechart SVG
  - Visualize time spent in each build phase
  - Usage: `marie-ssg flame -c site.toml -o flamechart.svg`
- Updated documentation with URL output format table

### v1.3.0 (2026-01-08)

- Added content-based asset hashing for cache busting
  - CSS/JS files get content hash in filename (e.g., `style.a1b2c3d4.css`)
  - Enable via `asset_hashing_enabled = true`
- Added optional `asset_manifest_path` to export hash mappings as JSON

### v1.2.0 (2026-01-07)

- Added `clean_urls` config option for SEO-friendly URL structure
  - Outputs `content-type/name/index.html` instead of `content-type/name.html`
  - URLs become `/articles/my-post/` instead of `/articles/my-post.html`
  - Sitemap and RSS feed URLs updated accordingly
- Updated examples with all available configuration options
- Updated CLAUDE.md/AGENTS.md with complete configuration reference
- Fixed `meta.extra` field access in templates (requires `[extra]` section in metadata)

### v1.1.0 (2026-01-06)

- Added optional `cover` field to ContentMeta for cover image URLs/paths
- Added `extra` field for arbitrary custom metadata via `[extra]` section in `.meta.toml`
- Access custom fields in templates via `meta.extra.fieldname`

### v1.0.0 (2026-01-03)

- Added `allow_dangerous_html` config option for raw HTML in markdown (SVG, figure tags, etc.)
- Improved logging format with `module::function` prefixes for consistency
- Added symbols for visual scanning: `←` (read), `→` (write), `✓` (unchanged)
- INFO level now shows each rendered file and index page
- Added static file copy summary at INFO level
- Added detailed IO debug logging (`io::read`, `io::write`, `io::copy`)
- Shortened `watch::change` INFO output to show only event_id

### v0.9.0 (2025-12-30)

- Migrated from `chrono` to `time` crate for datetime handling
- Added `kiters` crate for UTC timestamp formatting in tracing
- Enhanced tracing with EnvFilter support and UTC timestamps
- Migrated from `basic-toml` to `toml` crate (~2x faster parsing)
- Reduced binary size from 80MB to 9MB (89% reduction) by limiting syntax highlighting languages
- Restored missing mise tasks (`tests`, `verify`)
- Added TOML parser benchmark and migration documentation

### v0.8.1 (2025-12-24)

- Added `sitemap_enabled` configuration option
- Added retroactive PRDs and workflow documentation
- Added comprehensive `examples/site.toml`
- Added performance benchmarks for HTML unescaping

### v0.8.0 (2025-12-24)

- Added automatic `sitemap.xml` generation

### v0.7.1 (2025-12-24)

- Added `url` filter to prevent forward slash escaping in templates

### v0.7.0 (2025-12-23)

- Added syntax highlighting for code blocks using Autumnus
- Supported languages: Rust, Python, JavaScript, TypeScript, HTML, CSS, Bash, JSON, TOML, YAML
- Configurable themes with `github_dark` as default
- Enable via `syntax_highlighting_enabled` and `syntax_highlighting_theme` in site config

### v0.6.0 (2025-12-22)

- Added `all_content` variable to index templates for tag counting

### v0.5.0 (2025-12-20)

- Added `watch` subcommand for automatic rebuilds (macOS)
- Moved build to subcommand structure (`marie-ssg build`)
- Added `-V` / `--version` flag

### v0.4.0 (2025-12-15)

- Reached 86% test coverage with integration tests
- Added HTML DOM validation in tests using `scraper`
- Refactored template rendering for testability

### v0.3.0 (2025-10-14)

- Added template caching with `OnceLock`
- Introduced nested config structure
- Added `[dynamic]` section for custom template variables

### v0.2.0 (2025-10-10)

- Parallel content loading with Rayon
- Single-pass content loading (66% fewer I/O operations)
- Build timing instrumentation

### v0.1.0

- Initial release with core SSG functionality

## License

MIT
