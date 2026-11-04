//! Static Markdown wiki bundle over the shared profile projection.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_core::ann::{Status, Theory};
use dli_core::atom::{Interner, Rail};
use dli_core::valuation::Valuation;
use dli_world::model::{LoadedAtom, Provenance};

use crate::profile::{ExportError, ProfileSpec, ProvenanceMode};
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

type SourceRow = (
    ProjectionRow,
    &'static str,
    Option<&'static str>,
    Option<String>,
);

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
        let _ = writeln!(
            index,
            "- [{}]({})",
            escape_markdown(predicate),
            link_path(&path)
        );
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

/// Compile an explicitly requested source-claim view, including unresolved claims.
/// Each status describes the individual source's support, not an aggregate canon verdict.
pub fn compile_all(
    spec: &ProfileSpec,
    layer: &str,
    atoms: &[&LoadedAtom],
    valuation: &Valuation,
    theory: &Theory,
) -> Result<StaticWikiExport, ExportError> {
    compile_all_with_names(spec, layer, atoms, valuation, theory, None)
}

/// Compile source claims with named support variables when available.
pub fn compile_all_with_names(
    spec: &ProfileSpec,
    layer: &str,
    atoms: &[&LoadedAtom],
    valuation: &Valuation,
    theory: &Theory,
    interner: Option<&Interner>,
) -> Result<StaticWikiExport, ExportError> {
    let mut groups: BTreeMap<String, Vec<SourceRow>> = BTreeMap::new();
    for atom in atoms {
        let status = match atom.ann.status(valuation, theory) {
            Status::Certain => "certain",
            Status::Possible => "possible",
            Status::Dead => "dead",
        };
        let provenance = if spec.carries_provenance == ProvenanceMode::DevMetadata {
            Some(match atom.kind {
                Provenance::Proposed => "proposed",
                Provenance::Decided => "decided",
                Provenance::Ratified => "ratified",
                Provenance::Provisional => "provisional",
                Provenance::Unprovenanced => "unprovenanced",
            })
        } else {
            None
        };
        let support = if provenance == Some("proposed") {
            interner.and_then(|names| proposed_support(atom, names))
        } else {
            None
        };
        for row in projection::rows(spec, &[*atom])? {
            groups.entry(row.predicate.clone()).or_default().push((
                row,
                status,
                provenance,
                support.clone(),
            ));
        }
    }
    let mut index = String::from(
        "# Lore index\n\nThis view includes source claims with certain, possible, or dead support.\n\n",
    );
    let mut pages = Vec::with_capacity(groups.len() + 1);
    for (predicate, mut rows) in groups {
        rows.sort_by(|a, b| {
            (&a.0.id, &a.0.claim, &a.0.cited, a.1, a.2, &a.3)
                .cmp(&(&b.0.id, &b.0.claim, &b.0.cited, b.1, b.2, &b.3))
        });
        rows.dedup();
        let path = format!("predicate-{}.md", path_component(&predicate));
        let _ = writeln!(
            index,
            "- [{}]({})",
            escape_markdown(&predicate),
            link_path(&path)
        );
        let mut content = format!(
            "# {}\n\n[Back to index](index.md)\n\n",
            escape_markdown(&predicate)
        );
        for (row, status, provenance, support) in rows {
            let _ = write!(content, "- `{}` — **{status}**", code_text(&row.claim));
            if let Some(provenance) = provenance {
                let _ = write!(content, " (provenance: {provenance})");
            }
            if let Some(support) = support {
                let _ = write!(content, " (support: {})", escape_markdown(&support));
            }
            if !row.cited.is_empty() {
                let citations = row
                    .cited
                    .iter()
                    .map(|citation| escape_markdown(citation))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = write!(content, " (decisions: {citations})");
            }
            content.push('\n');
        }
        pages.push(WikiPage {
            path,
            title: predicate,
            content,
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

fn proposed_support(atom: &LoadedAtom, interner: &Interner) -> Option<String> {
    let paths = atom
        .ann
        .support
        .monomials()
        .map(|path| {
            let terms = path
                .lits()
                .map(|literal| {
                    let name = interner
                        .resolve(literal.atom)
                        .map_or_else(|| format!("atom#{}", literal.atom.0), str::to_owned);
                    if literal.rail == Rail::Neg {
                        format!("not {name}")
                    } else {
                        name
                    }
                })
                .collect::<Vec<_>>();
            if terms.is_empty() {
                "true".to_owned()
            } else {
                terms.join(" and ")
            }
        })
        .collect::<Vec<_>>();
    if paths.is_empty() {
        None
    } else {
        Some(
            paths
                .into_iter()
                .map(|path| format!("({path})"))
                .collect::<Vec<_>>()
                .join(" or "),
        )
    }
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

// Paths contain literal percent-encoded bytes as a safe filename. Escape the
// percent signs in links so URL decoding reaches that literal filename.
fn link_path(path: &str) -> String {
    path.replace('%', "%25")
}
