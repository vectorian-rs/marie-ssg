// src/guide.rs

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prints the Marie SSG guide to stdout
pub(crate) fn print_guide() {
    print!(
        r####"# Marie SSG Guide

Marie is a static site generator that converts markdown files with TOML metadata into HTML pages.

## Quick Start

```bash
marie-ssg build              # Build the site
marie-ssg build -c prod.toml # Build with custom config
marie-ssg build --include-drafts  # Include draft content in build
marie-ssg watch              # Watch and rebuild on changes (macOS)
marie-ssg watch --include-drafts  # Watch mode with drafts included
marie-ssg flame              # Build with profiling, output flamechart.svg
marie-ssg flame --time       # Profile with Chrome DevTools JSON output
marie-ssg guide              # Show this guide
```

## Project Structure

```
my-site/
├── site.toml           # Site configuration
├── content/            # Markdown content files
│   ├── blog/
│   │   ├── hello.md
│   │   └── hello.meta.toml
│   └── pages/
│       ├── about.md
│       └── about.meta.toml
├── templates/          # Jinja-style templates
│   ├── base.html
│   ├── post.html
│   └── blog_index.html
├── static/             # Static assets (CSS, images, fonts)
│   ├── css/
│   └── images/
└── output/             # Generated site (created by build)
```

## Configuration (site.toml)

```toml
[site]
title = "My Website"
tagline = "A personal blog"
domain = "example.com"
author = "Your Name"
content_dir = "content"
output_dir = "output"
template_dir = "templates"
static_dir = "static"
site_index_template = "index.html"

# Optional features (defaults shown)
syntax_highlighting_enabled = true   # Enable code block highlighting
syntax_highlighting_theme = "github_dark"
sitemap_enabled = true               # Generate sitemap.xml
rss_enabled = true                   # Generate feed.xml
rss_full_content = false             # Include full HTML in RSS via <content:encoded>
allow_dangerous_html = false         # Allow raw HTML in markdown (for <figure>, inline SVGs, etc.)
header_uri_fragment = false          # Add anchor links to headers for URL fragment navigation
clean_urls = false                   # Output as slug/index.html for SEO-friendly URLs (/blog/post/ instead of /blog/post.html)
asset_hashing_enabled = false        # Hash CSS/JS files for cache busting (style.css → style.a1b2c3d4.css)
# asset_manifest_path = "dist/asset-manifest.json"  # Export manifest to JSON (optional)

# Files copied to output root (e.g., favicon)
[site.root_static]
"favicon.ico" = "favicon.ico"
"robots.txt" = "robots.txt"

# Content type configurations
[content.blog]
index_template = "blog_index.html"
content_template = "post.html"
output_naming = "date"      # Prefix output with date (YYYY-MM-DD-stem.html)
rss_include = true          # Include in RSS feed (default: true)

[content.pages]
index_template = "pages_index.html"
content_template = "page.html"
rss_include = false         # Exclude from RSS feed

# Custom variables for templates
[dynamic]
github_url = "https://github.com/user"
twitter = "@username"
```

## Content Files

Each markdown file needs a companion `.meta.toml` file:

**content/blog/hello.md:**
```markdown
# Hello World

This is my first post.

## Context

This section becomes the excerpt for RSS feeds and index pages.

## Main Content

The rest of your article...
```

**content/blog/hello.meta.toml:**
```toml
title = "Hello World"
date = "2024-01-15T10:00:00+00:00"  # RFC 3339 format
author = "Your Name"
tags = ["intro", "blog"]
template = "custom.html"             # Optional: override default template
cover = "/images/hello-cover.jpg"    # Optional: cover image for social sharing
extra_js = ["static/js/chart.js"]    # Optional: JavaScript files for this article

[extra]
reading_time = "5 min"               # Custom fields go in [extra] section
category = "tutorials"
```

### Metadata Fields

| Field    | Required | Description                                              |
|----------|----------|----------------------------------------------------------|
| title    | Yes      | Article title                                            |
| date     | Yes      | Publication date (RFC 3339: `YYYY-MM-DDTHH:MM:SS+00:00`) |
| author   | Yes      | Author name                                              |
| tags     | Yes      | Array of tags (can be empty: `[]`)                       |
| draft    | No       | Exclude from builds (use `--include-drafts` to include)  |
| template | No       | Override the content type's default template             |
| cover    | No       | Cover image URL/path for social sharing                  |
| extra_js | No       | JavaScript files to load (array, e.g., `["js/chart.js"]`)|
| [extra]  | No       | Custom key-value fields (access via `meta.extra.key`)    |

## Templates (Jinja2/Minijinja)

Templates use Jinja2 syntax via the Minijinja library.

### Available Context

**In content templates (`post.html`):**
- `content` - Rendered HTML content
- `meta.title`, `meta.date`, `meta.author`, `meta.tags`
- `page.url` - Root-relative current page URL (e.g., `/blog/hello/`)
- `page.filename` - Current page output path without leading slash
- `page.permalink` / `page.canonical_url` - Absolute current page URL
- `config.site.title`, `config.site.author`, etc.
- `config.dynamic.github_url`, etc.

**In index templates (`blog_index.html`):**
- `contents` - List of ContentItem for this content type
- `all_content` - List of all ContentItem across all types
- `config` - Full site configuration
- `page.url`, `page.permalink`, `page.canonical_url` - Current index page URLs

### ContentItem Properties

```jinja
{{% for item in contents %}}
  <h2>{{{{ item.meta.title }}}}</h2>
  <time>{{{{ item.formatted_date }}}}</time>
  <p>{{{{ item.excerpt | safe }}}}</p>
  <a href="/{{{{ item.filename | url }}}}">Read more</a>
{{% endfor %}}
```

| Property              | Description                                        |
|-----------------------|----------------------------------------------------|
| `item.html`           | Full rendered HTML content                         |
| `item.meta.title`     | Article title                                      |
| `item.meta.date`      | Date object                                        |
| `item.meta.author`    | Author name                                        |
| `item.meta.tags`      | List of tags                                       |
| `item.meta.cover`     | Cover image URL/path (if set)                      |
| `item.meta.extra_js`  | JavaScript files array (iterate with for loop)     |
| `item.meta.extra.*`   | Custom fields (e.g., `item.meta.extra.reading_time`) |
| `item.formatted_date` | Human-readable date (e.g., "January 15, 2024")     |
| `item.filename`       | Output path (e.g., `blog/hello/` with clean_urls)  |
| `item.url`            | Root-relative URL path (e.g., `/blog/hello/`)      |
| `item.permalink`      | Absolute URL                                       |
| `item.canonical_url`  | Absolute canonical URL                             |
| `item.content_type`   | Content type (e.g., "blog")                        |
| `item.excerpt`        | HTML excerpt from "## Context" section             |

### Filters

- `| safe` - Render HTML without escaping
- `| url` - URL-encode for href attributes
- `| datetimeformat("%Y-%m-%d")` - Format dates
- `| asset_hash` - Resolve asset path to hashed version (requires `asset_hashing_enabled = true`)

### Template Example

```html
{{% extends "base.html" %}}

{{% block content %}}
<article>
  <h1>{{{{ meta.title }}}}</h1>
  <time>{{{{ meta.date | datetimeformat("%B %d, %Y") }}}}</time>
  <div class="content">{{{{ content | safe }}}}</div>
</article>

{{%- for script in meta.extra_js %}}
<script src="{{{{ script | asset_hash }}}}"></script>
{{%- endfor %}}
{{% endblock %}}
```

## Features

### Syntax Highlighting

Code blocks with language hints are highlighted automatically.

Supported languages: bash, css, html, javascript, json, python, rust, toml, typescript, yaml

Themes: `github_dark` (default), `monokai`, and others from Autumnus.

### Sitemap Generation

Automatically generates `sitemap.xml` with all pages when `sitemap_enabled = true`.

### RSS Feed Generation

Generates `feed.xml` with RSS 2.0 format when `rss_enabled = true`.
- Control per content type with `rss_include = true/false`
- Uses "## Context" section as excerpt
- Set `rss_full_content = true` to include full article HTML via `<content:encoded>` (for syndication to Dev.to, Hashnode, etc.)

### Header Anchor Links

When `header_uri_fragment = true`, headers (h1-h6) get anchor links for URL fragment navigation.

**Before:** `<h2>My Section</h2>`
**After:** `<h2 id="my-section"><a href="#my-section">My Section</a></h2>`

This enables:
- Direct linking to sections: `https://example.com/page#my-section`
- Clickable headers for easy link copying

### Clean URLs

When `clean_urls = true`, content is output with SEO-friendly directory structure.

**URL Pattern Placeholders:**

| Placeholder | Source | Example |
|-------------|--------|---------|
| `{{stem}}` | filename stem (without extension) | `agentic-project-management` |
| `{{date}}` | meta.date (YYYY-MM-DD) | `2025-12-12` |
| `{{year}}` | meta.date | `2025` |
| `{{month}}` | meta.date | `12` |
| `{{day}}` | meta.date | `12` |

**Example Input:**
```
File: content/blog/agentic-project-management.md
meta.date: 2025-12-12T02:02:02Z
```

**URL Output Formats:**

| url_pattern | clean_urls | Output |
|-------------|------------|--------|
| `{{stem}}` | false | /blog/agentic-project-management.html |
| `{{stem}}` | true | /blog/agentic-project-management/index.html |
| `{{date}}-{{stem}}` | true | /blog/2025-12-12-agentic-project-management/index.html |
| `{{date}}/{{stem}}` | true | /blog/2025-12-12/agentic-project-management/index.html |
| `{{year}}/{{month}}/{{day}}/{{stem}}` | true | /blog/2025/12/12/agentic-project-management/index.html |

**Example configuration:**
```toml
[site]
clean_urls = true

[content.blog]
index_template = "blog_index.html"
content_template = "post.html"
url_pattern = "{{date}}-{{stem}}"  # Flexible URL pattern
```

**Backwards Compatibility:** `output_naming = "date"` maps to `url_pattern = "{{date}}-{{stem}}"`

Benefits:
- Flexible URL structure with placeholders
- Date from meta.date (not filename)
- Cleaner, more shareable URLs
- Trailing slash convention (modern SSG standard)
- Sitemap and RSS URLs automatically updated

### URL Redirects

Configure explicit URL redirects for migrations, renames, or restructures:

```toml
[redirects]
"/blog/old-slug/" = "/blog/2024-01-15-new-slug/"
"/articles/legacy/" = "/blog/legacy/"
"/about-us/" = "/about/"
```

Each mapping generates an HTML file at the "from" path with a meta-refresh redirect to the "to" URL. Benefits:
- Pure static HTML, works on any hosting
- Instant redirect (meta refresh content=0)
- SEO-friendly with rel=canonical
- No server configuration required

### Asset Hashing (Cache Busting)

When `asset_hashing_enabled = true`, CSS and JS files get content-based hashes in their filenames for cache busting.

**How it works:**
1. Marie computes an 8-character BLAKE3 hash from each CSS/JS file's content
2. Files are copied with hashed names: `style.css` → `style.a1b2c3d4.css`
3. A manifest maps original paths to hashed paths
4. Old hashed files are cleaned up on rebuild

**Usage in templates:**

```html
<!-- Before (hardcoded, cache problems) -->
<link rel="stylesheet" href="/static/css/style.css" />
<script src="/static/js/app.js"></script>

<!-- After (using asset_hash filter) -->
<link rel="stylesheet" href="{{{{ "static/css/style.css" | asset_hash }}}}" />
<script src="{{{{ "static/js/app.js" | asset_hash }}}}"></script>
```

**Output:**
```html
<link rel="stylesheet" href="/static/css/style.a1b2c3d4.css" />
<script src="/static/js/app.b5c6d7e8.js"></script>
```

**Benefits:**
- Set long cache headers (e.g., `Cache-Control: max-age=31536000`)
- Browsers automatically fetch new versions when content changes
- No manual cache busting (no `?v=123` query strings needed)
- Only CSS and JS files are hashed; images and fonts are unchanged

**Note:** If `asset_hashing_enabled = false` (default), the `asset_hash` filter returns the original path unchanged.

**Exporting the manifest:**

Optionally export the asset manifest to a JSON file for use by service workers, external build tools, or debugging:

```toml
[site]
asset_hashing_enabled = true
asset_manifest_path = "dist/asset-manifest.json"
```

Output (`asset-manifest.json`):
```json
{{
  "css/style.css": "/static/css/style.a1b2c3d4.css",
  "js/app.js": "/static/js/app.b5c6d7e8.js"
}}
```

### Watch Mode (macOS)

Automatically rebuilds when files change:
```bash
marie-ssg watch
```

## Output

After build, your site is in the output directory:

```
output/
├── index.html          # Site homepage
├── sitemap.xml         # Sitemap (if enabled)
├── feed.xml            # RSS feed (if enabled)
├── favicon.ico         # Root static files
├── static/             # Copied static assets
├── blog/
│   ├── index.html      # Blog index
│   └── 2024-01-15-hello.html
└── pages/
    ├── index.html
    └── about.html
```

### Flamechart Profiling

Generate profiling output to visualize build performance:
```bash
marie-ssg flame                    # Output: flamechart.svg (default)
marie-ssg flame --svg              # Explicit SVG flamegraph
marie-ssg flame --fold             # Output: flamechart.folded (for speedscope)
marie-ssg flame --fold --svg       # Output both formats
marie-ssg flame --time             # Output: flamechart.json (Chrome DevTools)
marie-ssg flame -o build           # Custom output base path
marie-ssg flame -c prod.toml       # Custom config
```

Output formats:
- `--svg`: Interactive SVG flamegraph (default if no flags specified)
- `--fold`: Folded stacks format for speedscope or inferno
- `--time`: Chrome DevTools JSON for timeline view with timestamps

Open SVG in a browser, load `.folded` in speedscope, or import `.json` in Chrome DevTools (Performance tab).

## Tips

1. **Date prefix**: Use `output_naming = "date"` to prefix files with publication date
2. **Excerpts**: Add a "## Context" section for RSS/index excerpts
3. **Custom templates**: Override per-article with `template` in metadata
4. **Dynamic vars**: Add custom variables in `[dynamic]` for use in templates

---
Generated by marie-ssg {version}
"####,
        version = VERSION
    );
}
