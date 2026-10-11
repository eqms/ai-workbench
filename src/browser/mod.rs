//! Browser preview module
//!
//! Provides functionality to open files in the system browser
//! and convert Markdown to HTML for preview.

pub mod iterm_transfer;
pub mod markdown;
pub mod opener;
pub mod pdf_export;
pub mod remote_open;
pub mod syntax;
pub mod template;
#[cfg(feature = "pdf-export")]
pub mod typst_pdf;

pub use markdown::*;
pub use opener::*;
pub use syntax::*;

/// Escape text for use in HTML element content and quoted attribute values.
///
/// File names, directory names and resolved paths come from whatever tree the
/// user browses, so a name like `<img src=x onerror=…>.rs` or a directory named
/// `x" onerror="…` must not become markup in the generated preview page.
pub(crate) fn html_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Convert heading or anchor text to a URL-style slug (GitHub-flavored Markdown convention).
///
/// Used by both the HTML exporter (for `id=` attributes) and the Typst/PDF exporter
/// (for Typst labels). Shared here to keep both exporters in sync.
pub(crate) fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c == ' ' || c == '-' || c == '_' {
                '-'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
