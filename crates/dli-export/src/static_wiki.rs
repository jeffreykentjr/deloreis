//! Static Markdown wiki bundle over the shared profile projection.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_world::model::LoadedAtom;

use crate::profile::{ExportError, ProfileSpec};
use crate::projection::{self, ProjectionRow};

/// One materializable Markdown page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WikiPage {
    /// Relative output path.
    pub path: String,
    /// Human-readable page title.
    pub title: String,
    /// UTF-8 Markdown content.
    pub content: String,
}

/// A deterministic static-wiki bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StaticWikiExport {
    /// Profile identity that produced the pages.
    pub profile: String,
    /// Exported model layer.
    pub layer: String,
    /// Index page plus one page per selected predicate.
    pub pages: Vec<WikiPage>,
}

/// Compile selected certain-layer atoms into a static Markdown wiki bundle.
pub fn compile(
    spec: &ProfileSpec,
    layer: &str,
    atoms: &[&LoadedAtom],
) -> Result<StaticWikiExport, ExportError> {
    let rows = projection::rows(spec, atoms)?;
    let mut by_predicate: BTreeMap<&str, Vec<&ProjectionRow>> = BTreeMap::new();
    for row in &rows {
        by_predicate
            .entry(row.predicate.as_str())
            .or_default()
            .push(row);
    }

    let mut index = String::from("# Lore index\n\n");
    let mut pages = Vec::with_capacity(by_predicate.len() + 1);
    for (predicate, predicate_rows) in by_predicate {
        let path = format!("predicate-{}.md", path_component(predicate));
        let _ = writeln!(index, "- [{}]({path})", escape_markdown(predicate));
        pages.push(WikiPage {
            path,
            title: predicate.to_owned(),
            content: render_page(predicate, &predicate_rows),
        });
    }
    pages.insert(
        0,
        WikiPage {
            path: "index.md".to_owned(),
            title: "Lore index".to_owned(),
            content: index,
        },
    );
    Ok(StaticWikiExport {
        profile: spec.qualified(),
        layer: layer.to_owned(),
        pages,
    })
}

fn render_page(predicate: &str, rows: &[&ProjectionRow]) -> String {
    let mut page = format!("# {}\n\n", escape_markdown(predicate));
    for row in rows {
        let _ = write!(
            page,
            "- **{}**: `{}`",
            escape_markdown(&row.id),
            code_text(&row.claim)
        );
        if !row.arguments.is_empty() {
            let arguments = row
                .arguments
                .iter()
                .map(|(name, value)| format!("{name}={}", code_text(value)))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = write!(page, " ({arguments})");
        }
        if !row.cited.is_empty() {
            let cited = row
                .cited
                .iter()
                .map(|citation| escape_markdown(citation))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = write!(page, " [cited: {cited}]");
        }
        page.push('\n');
    }
    page
}

fn escape_markdown(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\\', "\\\\")
        .replace(['\r', '\n'], " ")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('#', "\\#")
        .replace('|', "\\|")
}

fn code_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('`', "&#96;")
        .replace(['\r', '\n'], " ")
}

fn path_component(text: &str) -> String {
    let mut encoded = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}
