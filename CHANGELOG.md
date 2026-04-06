# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.6.0] - 2026-03-13

### Added
- Full content RSS support and RSS best practices

## [1.5.0] - 2026-01-13

### Added
- Draft content support with `--include-drafts` CLI flag
- Integration tests for draft content

## [1.4.0] - 2026-01-11

### Added
- Data flow documentation with SVG diagrams
- Multiple output formats for flame command
- `extra_js` field for per-article JavaScript files
- Flamechart profiling command (`marie-ssg flame`)
- Flexible `url_pattern` system and URL redirects

## [1.3.0] - 2026-01-08

### Added
- Asset manifest JSON export
- Content-based asset hashing for cache busting

## [1.2.0] - 2026-01-07

### Added
- Clean URLs feature (`clean_urls` config option)
- `cover` and `extra` fields in ContentMeta
- Configuration section in README

### Fixed
- `meta.extra` field serialization

## [1.1.0] - 2026-01-06

### Added
- `cover` and `extra` fields to ContentMeta

## [1.0.1] - 2026-01-05

### Added
- `header_uri_fragment` config option for anchor links
- GitHub Actions release workflow

## [1.0.0] - 2026-01-03

### Added
- `allow_dangerous_html` site config option
- `guide` subcommand for LLM/human onboarding
- RSS feed generation (`feed.xml`)

### Changed
- Improved logging format with module::function prefixes

## [0.9.0] - 2025-12-30

### Added
- Test coverage with cargo-tarpaulin
- Sitemap.xml generation

### Changed
- Migrated from chrono to time and kiters crates
- Migrated from basic-toml to toml crate
- Reduced binary size from 80MB to 9MB (89% reduction)

### Fixed
- UTF-8 preservation in HTML entity unescaping
- Proper error handling replacing `.expect()` panics

## [0.8.0] - 2025-12-24

### Added
- Sitemap.xml generation with `sitemap_enabled` config
- Comprehensive examples/site.toml

## [0.7.0] - 2025-12-22

### Added
- Syntax highlighting support
- `all_content` template variable for tag counting
- Watch mode with fsevent for automatic rebuilds on macOS

### Fixed
- Watch mode not reloading templates on change

## [0.6.0] - 2025-12-22

### Added
- Pass `all_content` variable to templates for tag counting

## [0.3.0] - 2025-10-14

### Added
- Root static files support for favicon.ico and robots.txt

## [0.2.0] - 2025-10-10

### Added
- Initial public release with markdown-to-HTML rendering

[1.6.0]: https://github.com/vectorian-rs/marie-ssg/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/vectorian-rs/marie-ssg/compare/v1.4.0...v1.5.0
[1.4.0]: https://github.com/vectorian-rs/marie-ssg/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/vectorian-rs/marie-ssg/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/vectorian-rs/marie-ssg/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/vectorian-rs/marie-ssg/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/vectorian-rs/marie-ssg/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/vectorian-rs/marie-ssg/compare/v0.9.0...v1.0.0
[0.9.0]: https://github.com/vectorian-rs/marie-ssg/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/vectorian-rs/marie-ssg/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/vectorian-rs/marie-ssg/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/vectorian-rs/marie-ssg/compare/v0.3.0...v0.6.0
[0.3.0]: https://github.com/vectorian-rs/marie-ssg/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/vectorian-rs/marie-ssg/releases/tag/v0.2.0
