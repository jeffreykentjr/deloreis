//! RFC 4180-compatible flat export over the shared profile projection.

use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_world::model::LoadedAtom;

use crate::profile::{ExportError, ProfileSpec};
use crate::projection;

/// A producer-facing CSV artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CsvExport {
    /// Profile identity that produced the rows.
    pub profile: String,
    /// Exported model layer.
    pub layer: String,
    /// Media type of [`Self::content`].
    pub content_type: String,
    /// Number of data rows, excluding the header.
    pub row_count: usize,
    /// UTF-8 RFC 4180 CSV with CRLF record endings.
    pub content: String,
}

/// Compile selected certain-layer atoms into deterministic CSV.
pub fn compile(
    spec: &ProfileSpec,
    layer: &str,
    atoms: &[&LoadedAtom],
) -> Result<CsvExport, ExportError> {
    let rows = projection::rows(spec, atoms)?;
    let mut content = String::from("id,kind,predicate,arguments,claim,cited\r\n");
    for row in &rows {
        let arguments =
            serde_json::to_string(&row.arguments).map_err(|error| ExportError::Render {
                target: "csv".to_owned(),
                detail: error.to_string(),
            })?;
        let cited = serde_json::to_string(&row.cited).map_err(|error| ExportError::Render {
            target: "csv".to_owned(),
            detail: error.to_string(),
        })?;
        let fields = [
            row.id.as_str(),
            row.kind.as_str(),
            row.predicate.as_str(),
            arguments.as_str(),
            row.claim.as_str(),
            cited.as_str(),
        ];
        for (index, field) in fields.iter().enumerate() {
            if index > 0 {
                content.push(',');
            }
            write_field(&mut content, field);
        }
        content.push_str("\r\n");
    }
    Ok(CsvExport {
        profile: spec.qualified(),
        layer: layer.to_owned(),
        content_type: "text/csv; charset=utf-8".to_owned(),
        row_count: rows.len(),
        content,
    })
}

fn write_field(out: &mut String, field: &str) {
    let escaped;
    let field = if field.starts_with(['=', '+', '-', '@', '\t', '\r', '\n']) {
        escaped = format!("'{field}");
        escaped.as_str()
    } else {
        field
    };
    if field.contains([',', '"', '\r', '\n']) {
        out.push('"');
        for character in field.chars() {
            if character == '"' {
                out.push('"');
            }
            out.push(character);
        }
        out.push('"');
    } else {
        let _ = write!(out, "{field}");
    }
}
