//! Markdown to HTML conversion for browser preview

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::browser::template::TemplateContext;
use crate::config::DocumentConfig;

/// Build HTML document dynamically from DocumentConfig
fn build_html_template(doc: &DocumentConfig) -> String {
    let ctx = TemplateContext::new(doc);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{title}}</title>
    <style>
        :root {{
            color-scheme: light dark;
        }}
        {base_body}
        {typography}
        {code}
        {blockquote}
        img {{ max-width: 100%; height: auto; }}
        {table}
        {link}
        ul, ol {{ padding-left: 2em; }}
        li {{ margin: 0.25em 0; }}
        hr {{ border: none; border-top: 1px solid #eee; margin: 2em 0; }}
        {footer}
        {dark_mode}
    </style>
</head>
<body>
{{content}}
<div class="footer">
    {footer_text}
</div>
</body>
</html>"#,
        base_body = ctx.base_body_css(),
        typography = ctx.typography_css(),
        code = ctx.code_css(),
        blockquote = ctx.blockquote_css(),
        table = ctx.table_css(),
        link = ctx.link_css(),
        footer = ctx.footer_css(),
        dark_mode = ctx.dark_mode_css(),
        footer_text = ctx.footer_text(),
    )
}

/// Convert a single markdown file to an HTML string without writing to disk.
/// Returns (html_string, secure_temp_file).
/// The `NamedTempFile` is created with O_EXCL but not yet written; content is
/// written after link rewriting in the caller.
fn convert_single_md(
    md_path: &Path,
    doc: &DocumentConfig,
    project_name: &str,
) -> Result<(String, tempfile::NamedTempFile)> {
    use pulldown_cmark::{html, Event, Options, Parser};

    let md_content = std::fs::read_to_string(md_path)?;
    let title = md_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Preview");

    // Get the directory of the markdown file for resolving relative paths
    let md_dir = md_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Enable all markdown extensions
    let options = Options::all();
    let parser = Parser::new_ext(&md_content, options);
    let events: Vec<Event> = parser.collect();
    let events = neutralize_raw_html(events);
    let events = inject_heading_ids(events);

    let mut html_content = String::new();
    html::push_html(&mut html_content, events.into_iter());

    // Convert relative image paths to absolute file:// URLs
    let html_content = fix_image_paths(&html_content, &md_dir);

    let template = build_html_template(doc);
    let html = template
        .replace("{title}", &crate::browser::html_escape(title))
        .replace("{content}", &html_content);

    // Create secure temp file (O_EXCL) — content written after link rewriting
    let tmp = crate::browser::pdf_export::default_preview_file(md_path, project_name)?;

    Ok((html, tmp))
}

/// Collect relative `.md` link hrefs from rendered HTML.
/// Returns raw href values like `"USAGE.md"`, `"./INSTALL.md#section"`.
fn collect_md_links(html: &str) -> Vec<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"<a\s[^>]*?href="([^"]*\.md(?:#[^"]*)?)"[^>]*>"#).expect("valid regex")
    });

    let mut links = Vec::new();
    for cap in LINK_RE.captures_iter(html) {
        let href = &cap[1];
        // Skip absolute URLs and absolute paths
        if href.starts_with("http://")
            || href.starts_with("https://")
            || href.starts_with("file://")
            || href.starts_with('/')
        {
            continue;
        }
        links.push(href.to_string());
    }
    links
}

/// Rewrite relative `.md` links in HTML to point to converted temp HTML files.
/// `link_map` maps normalized md filenames (e.g. `"USAGE.md"`) to their temp HTML paths.
fn fix_md_links(html: &str, link_map: &HashMap<String, PathBuf>) -> String {
    use regex::Regex;
    use std::sync::LazyLock;

    static LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(<a\s[^>]*?href=")([^"]*\.md)(#[^"]*)?("[^>]*>)"#).expect("valid regex")
    });

    LINK_RE
        .replace_all(html, |caps: &regex::Captures| {
            let prefix = &caps[1];
            let md_href = &caps[2];
            let fragment = caps.get(3).map_or("", |m| m.as_str());
            let suffix = &caps[4];

            // Skip absolute URLs
            if md_href.starts_with("http://")
                || md_href.starts_with("https://")
                || md_href.starts_with("file://")
                || md_href.starts_with('/')
            {
                return caps[0].to_string();
            }

            // Normalize: strip leading "./"
            let normalized = md_href.trim_start_matches("./");
            if let Some(html_path) = link_map.get(normalized) {
                let file_url =
                    crate::browser::html_escape(&format!("file://{}", html_path.display()));
                format!("{}{}{}{}", prefix, file_url, fragment, suffix)
            } else {
                caps[0].to_string()
            }
        })
        .to_string()
}

/// Convert markdown file to HTML, including all directly referenced .md files.
/// Returns a Vec of `NamedTempFile` handles (primary first, then dependencies).
/// Handles are kept alive so the browser can open them; files are auto-deleted on drop.
/// All inter-document .md links are rewritten to point to the generated HTML files.
pub fn markdown_to_html(
    md_path: &Path,
    doc: &DocumentConfig,
    project_name: &str,
) -> Result<Vec<tempfile::NamedTempFile>> {
    use std::io::Write;

    // Phase 1: Convert primary file
    let (primary_html, primary_tmp) = convert_single_md(md_path, doc, project_name)?;

    // Get source directory for resolving relative paths
    let md_dir = md_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let canonical_md_dir = md_dir
        .canonicalize()
        .unwrap_or_else(|_| md_dir.to_path_buf());

    // Collect all referenced .md links from primary HTML
    let md_links = collect_md_links(&primary_html);

    // Build link map: normalized md filename -> temp HTML path (PathBuf for link rewriting)
    // Also collect (html_string, NamedTempFile) for each dependency
    let mut link_map: HashMap<String, PathBuf> = HashMap::new();
    let mut dep_files: Vec<(String, tempfile::NamedTempFile)> = Vec::new();

    for href in &md_links {
        // Strip fragment for file resolution
        let bare_href = href.split('#').next().unwrap_or(href);
        let normalized = bare_href.trim_start_matches("./");

        // Skip if already processed
        if link_map.contains_key(normalized) {
            continue;
        }

        // Resolve relative path with security guard
        let abs_candidate = md_dir.join(normalized);
        let resolved = match abs_candidate.canonicalize() {
            Ok(r) => r,
            Err(_) => continue, // File doesn't exist, leave link unchanged
        };

        // Path traversal guard: must be under the source directory
        if !resolved.starts_with(&canonical_md_dir) {
            continue;
        }

        // Convert the referenced .md file
        match convert_single_md(&resolved, doc, project_name) {
            Ok((dep_html, dep_tmp)) => {
                link_map.insert(normalized.to_string(), dep_tmp.path().to_path_buf());
                dep_files.push((dep_html, dep_tmp));
            }
            Err(_) => continue, // Conversion failed, leave link unchanged
        }
    }

    // Add primary path to link map after dep paths are collected (not needed for link rewriting
    // since links only point outward from primary, but record for completeness)
    let primary_path = primary_tmp.path().to_path_buf();

    // Phase 2: Rewrite .md links in ALL HTML files and write to disk

    // Write primary file
    let primary_html = fix_md_links(&primary_html, &link_map);
    let mut primary_tmp = primary_tmp;
    primary_tmp.write_all(primary_html.as_bytes())?;
    primary_tmp.flush()?;

    // Collect all NamedTempFile handles (primary first)
    let _ = primary_path; // suppress unused warning — path used conceptually above
    let mut all_handles = vec![primary_tmp];

    // Write dependency files
    for (dep_html, mut dep_tmp) in dep_files {
        let dep_html = fix_md_links(&dep_html, &link_map);
        dep_tmp.write_all(dep_html.as_bytes())?;
        dep_tmp.flush()?;
        all_handles.push(dep_tmp);
    }

    Ok(all_handles)
}

/// Fix relative image paths in HTML by converting them to absolute file:// URLs
fn fix_image_paths(html: &str, base_dir: &Path) -> String {
    use regex::Regex;

    // Static regex: compiled once, cannot fail on this known-good pattern
    use std::sync::LazyLock;
    static IMG_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"<img\s+([^>]*?)src="([^"]+)"([^>]*)>"#).expect("valid regex")
    });
    let img_re = &*IMG_RE;

    img_re
        .replace_all(html, |caps: &regex::Captures| {
            let before = &caps[1];
            let src = &caps[2];
            let after = &caps[3];

            // Skip if already an absolute URL (http://, https://, file://, data:)
            if src.starts_with("http://")
                || src.starts_with("https://")
                || src.starts_with("file://")
                || src.starts_with("data:")
            {
                return caps[0].to_string();
            }

            // Resolve relative path to absolute with path traversal guard
            let abs_path = base_dir.join(src);
            if let Ok(resolved) = abs_path.canonicalize() {
                let canonical_base = base_dir
                    .canonicalize()
                    .unwrap_or_else(|_| base_dir.to_path_buf());
                if resolved.starts_with(&canonical_base) {
                    // The resolved path contains directory names from the
                    // browsed tree; a `"` in one would end the attribute.
                    let file_url =
                        crate::browser::html_escape(&format!("file://{}", resolved.display()));
                    format!(r#"<img {}src="{}"{}>"#, before, file_url, after)
                } else {
                    caps[0].to_string()
                }
            } else {
                caps[0].to_string()
            }
        })
        .to_string()
}

/// Inject `id` attributes into heading events so that internal anchor links work.
///
/// pulldown-cmark does not auto-generate `id` on headings — it only sets `id`
/// when the Markdown source uses explicit `{ #id }` attribute syntax.
/// This function walks the event stream, collects heading text, slugifies it,
/// and injects the `id` into the `Tag::Heading` start event.
fn inject_heading_ids(mut events: Vec<pulldown_cmark::Event>) -> Vec<pulldown_cmark::Event> {
    use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

    let mut i = 0;
    while i < events.len() {
        if let Event::Start(Tag::Heading { .. }) = &events[i] {
            // Collect heading text from subsequent events up to the End tag
            let mut text_buf = String::new();
            let mut j = i + 1;
            while j < events.len() {
                match &events[j] {
                    Event::Text(t) | Event::Code(t) => text_buf.push_str(t),
                    Event::End(TagEnd::Heading(_)) => break,
                    _ => {}
                }
                j += 1;
            }

            let slug = crate::browser::slugify(&text_buf);
            if !slug.is_empty() {
                // Re-match to extract fields, then replace the event
                if let Event::Start(Tag::Heading {
                    level,
                    classes,
                    attrs,
                    ..
                }) = &events[i]
                {
                    let level = *level;
                    let classes = classes.clone();
                    let attrs = attrs.clone();
                    events[i] = Event::Start(Tag::Heading {
                        level,
                        id: Some(CowStr::from(slug)),
                        classes,
                        attrs,
                    });
                }
            }
        }
        i += 1;
    }
    events
}

/// Neutralize raw HTML embedded directly in Markdown source.
///
/// `pulldown_cmark::html::push_html` always HTML-escapes `Event::Text`, so
/// converting `Event::Html`/`Event::InlineHtml` into `Event::Text` (same
/// inner string, different variant) defangs any `<script>`/`<iframe>`/raw
/// HTML embedded directly as literal HTML in `.md` source — it is rendered
/// as visible escaped text instead of being interpreted as live HTML by the
/// browser that opens the `file://` preview.
///
/// `Tag::Image`/`Tag::Link` events — real Markdown `![alt](src)`/`[text](url)`
/// syntax — are kept, so `fix_image_paths()` and `fix_md_links()`, which
/// regex-match the `<img>`/`<a>` tags those render into, keep working. Only a
/// destination with a script-capable scheme (`javascript:`, `vbscript:`,
/// `data:`) is replaced by `#`: pulldown-cmark escapes the URL but does not
/// filter schemes, so `[click](javascript:…)` would run on click.
fn neutralize_raw_html(events: Vec<pulldown_cmark::Event>) -> Vec<pulldown_cmark::Event> {
    use pulldown_cmark::{CowStr, Event, Tag};

    events
        .into_iter()
        .map(|event| match event {
            Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) if is_script_url(&dest_url) => Event::Start(Tag::Link {
                link_type,
                dest_url: CowStr::Borrowed("#"),
                title,
                id,
            }),
            other => other,
        })
        .collect()
}

/// True for URL schemes a browser would execute or render as a document.
/// Browsers ignore ASCII whitespace and control characters inside the scheme
/// (`java\tscript:` still runs), so those are dropped before comparing.
fn is_script_url(url: &str) -> bool {
    let scheme: String = url
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && !c.is_control())
        .take_while(|&c| c != ':')
        .collect::<String>()
        .to_ascii_lowercase();
    url.contains(':') && matches!(scheme.as_str(), "javascript" | "vbscript" | "data")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md_to_html_fragment(md: &str) -> String {
        use pulldown_cmark::{html, Event, Options, Parser};

        let options = Options::all();
        let parser = Parser::new_ext(md, options);
        let events: Vec<Event> = parser.collect();
        let events = neutralize_raw_html(events);
        let events = inject_heading_ids(events);
        let mut out = String::new();
        html::push_html(&mut out, events.into_iter());
        out
    }

    #[test]
    fn test_raw_script_tag_is_escaped_not_executed() {
        let html = md_to_html_fragment("Hello <script>alert(1)</script> World");
        assert!(!html.contains("<script>"), "got: {}", html);
        assert!(html.contains("alert"), "got: {}", html);
    }

    #[test]
    fn test_script_scheme_links_are_neutralized() {
        let html = md_to_html_fragment(
            "[a](javascript:alert(1)) [b](JavaScript:alert(1)) [c](data:text/html,x) \
             [ok](https://example.com) [doc](USAGE.md)",
        );
        assert!(!html.to_lowercase().contains("javascript:"), "got: {html}");
        assert!(!html.contains("data:text/html"), "got: {html}");
        assert!(
            html.contains(r#"href="https://example.com""#),
            "got: {html}"
        );
        assert!(html.contains(r#"href="USAGE.md""#), "got: {html}");
    }

    #[test]
    fn test_is_script_url_ignores_case_and_embedded_whitespace() {
        assert!(is_script_url("javascript:alert(1)"));
        assert!(is_script_url(" JaVa\tScRiPt:alert(1)"));
        assert!(is_script_url("vbscript:x"));
        assert!(is_script_url("data:text/html;base64,AAAA"));
        assert!(!is_script_url("https://example.com/javascript:x"));
        assert!(!is_script_url("javascript.md"));
        assert!(!is_script_url("#section"));
    }

    /// The Markdown file's own directory is part of every resolved image path,
    /// and a `"` in it would end the `src` attribute.
    #[cfg(unix)]
    #[test]
    fn test_image_path_with_quote_in_directory_stays_inside_attribute() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(r#"x" onerror="alert(1)"#);
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("pic.png"), b"png").unwrap();

        let out = fix_image_paths(r#"<img src="pic.png" alt="p" />"#, &dir);
        assert!(!out.contains(r#"" onerror=""#), "got: {out}");
        assert!(out.contains("&quot; onerror=&quot;"), "got: {out}");
        assert!(out.contains("pic.png"), "got: {out}");
    }

    #[cfg(unix)]
    #[test]
    fn test_markdown_title_is_escaped() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("<svg onload=alert(1)>.md");
        std::fs::write(&md, "# Hi\n").unwrap();
        let (html, _tmp) = convert_single_md(&md, &DocumentConfig::default(), "test").unwrap();
        assert!(!html.contains("<svg onload"), "title became markup");
        assert!(html.contains("&lt;svg onload=alert(1)&gt;"));
    }

    #[test]
    fn test_markdown_link_syntax_still_renders_anchor() {
        let html = md_to_html_fragment("[text](http://example.com)");
        assert!(
            html.contains(r#"href="http://example.com""#),
            "got: {}",
            html
        );
    }

    #[test]
    fn test_markdown_image_syntax_still_renders_img() {
        let html = md_to_html_fragment("![alt](pic.png)");
        assert!(html.contains("<img"), "got: {}", html);
        assert!(html.contains(r#"src="pic.png""#), "got: {}", html);
    }

    #[test]
    fn test_heading_id_injected() {
        let html = md_to_html_fragment("## Overview");
        assert!(html.contains("id=\"overview\""), "got: {}", html);
    }

    #[test]
    fn test_heading_id_slug_spaces() {
        let html = md_to_html_fragment("## Hello World");
        assert!(html.contains("id=\"hello-world\""), "got: {}", html);
    }

    #[test]
    fn test_heading_id_drops_special_chars() {
        let html = md_to_html_fragment("## C++ & Rust");
        assert!(html.contains("id=\"c-rust\""), "got: {}", html);
    }

    #[test]
    fn test_anchor_link_has_target() {
        let md = "## Overview\n\n[go to overview](#overview)";
        let html = md_to_html_fragment(md);
        assert!(html.contains("id=\"overview\""), "missing id: {}", html);
        assert!(
            html.contains("href=\"#overview\""),
            "missing href: {}",
            html
        );
    }

    #[test]
    fn test_multiple_heading_levels() {
        let md = "# Title\n## Section\n### Subsection";
        let html = md_to_html_fragment(md);
        assert!(html.contains("id=\"title\""), "got: {}", html);
        assert!(html.contains("id=\"section\""), "got: {}", html);
        assert!(html.contains("id=\"subsection\""), "got: {}", html);
    }

    #[test]
    fn test_heading_with_inline_code() {
        let html = md_to_html_fragment("## Use `fmt::Display`");
        assert!(html.contains("id=\"use-fmtdisplay\""), "got: {}", html);
    }

    // --- Tests for collect_md_links ---

    #[test]
    fn test_collect_md_links_simple() {
        let html = r#"<a href="USAGE.md">Usage</a>"#;
        let links = collect_md_links(html);
        assert_eq!(links, vec!["USAGE.md"]);
    }

    #[test]
    fn test_collect_md_links_with_fragment() {
        let html = r#"<a href="INSTALL.md#section">Install</a>"#;
        let links = collect_md_links(html);
        assert_eq!(links, vec!["INSTALL.md#section"]);
    }

    #[test]
    fn test_collect_md_links_ignores_absolute() {
        let html = r#"<a href="https://example.com/foo.md">ext</a>
                       <a href="/absolute.md">abs</a>
                       <a href="file:///tmp/local.md">file</a>"#;
        let links = collect_md_links(html);
        assert!(links.is_empty(), "got: {:?}", links);
    }

    #[test]
    fn test_collect_md_links_dotslash() {
        let html = r#"<a href="./USAGE.md">Usage</a>"#;
        let links = collect_md_links(html);
        assert_eq!(links, vec!["./USAGE.md"]);
    }

    // --- Tests for fix_md_links ---

    #[test]
    fn test_fix_md_links_rewrites() {
        let html = r#"<a href="USAGE.md">Usage</a>"#;
        let mut map = HashMap::new();
        map.insert(
            "USAGE.md".to_string(),
            PathBuf::from("/tmp/project-usage-01.01.2026.html"),
        );
        let result = fix_md_links(html, &map);
        assert!(
            result.contains("file:///tmp/project-usage-01.01.2026.html"),
            "got: {}",
            result
        );
    }

    #[test]
    fn test_fix_md_links_preserves_fragment() {
        let html = r#"<a href="USAGE.md#shortcuts">Shortcuts</a>"#;
        let mut map = HashMap::new();
        map.insert(
            "USAGE.md".to_string(),
            PathBuf::from("/tmp/project-usage-01.01.2026.html"),
        );
        let result = fix_md_links(html, &map);
        assert!(
            result.contains("file:///tmp/project-usage-01.01.2026.html#shortcuts"),
            "got: {}",
            result
        );
    }

    #[test]
    fn test_fix_md_links_leaves_unknown() {
        let html = r#"<a href="UNKNOWN.md">Unknown</a>"#;
        let map = HashMap::new();
        let result = fix_md_links(html, &map);
        assert_eq!(result, html);
    }

    #[test]
    fn test_fix_md_links_normalizes_dotslash() {
        let html = r#"<a href="./USAGE.md">Usage</a>"#;
        let mut map = HashMap::new();
        map.insert(
            "USAGE.md".to_string(),
            PathBuf::from("/tmp/project-usage-01.01.2026.html"),
        );
        let result = fix_md_links(html, &map);
        assert!(
            result.contains("file:///tmp/project-usage-01.01.2026.html"),
            "got: {}",
            result
        );
    }
}
