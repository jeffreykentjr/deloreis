//! Shared flat projection for data-oriented export targets.
//!
//! CSV and static-wiki exports consume the same deterministic rows. A profile
//! selects predicates, optionally assigns an id template per predicate, chooses
//! positional fields (`arg0`, `arg1`, ...), and controls whether citations ride
//! with the result.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_eval::world::GroundAtom;
use dli_world::model::LoadedAtom;

use crate::profile::{ExportError, ProfileSpec, ProvenanceMode, RenderRule};

/// Whether a projected predicate was selected as a sort or relation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectionKind {
    /// A selected entity-sort declaration.
    Sort,
    /// A selected fact or relation.
    Relation,
}

impl ProjectionKind {
    /// Stable text used by tabular renderers.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sort => "sort",
            Self::Relation => "relation",
        }
    }
}

/// One selected, rendered certain-layer atom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionRow {
    /// Profile-rendered stable row id.
    pub id: String,
    /// Whether the profile selected this predicate as a sort or relation.
    pub kind: ProjectionKind,
    /// Canonical predicate name.
    pub predicate: String,
    /// Positional arguments retained by the profile.
    pub arguments: BTreeMap<String, String>,
    /// Canonical ground claim.
    pub claim: String,
    /// Cited rulings, present only in dev-metadata mode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cited: Vec<String>,
}

/// Project selected atoms into deterministic, target-neutral rows.
///
/// Empty `sorts` and `relations` together mean "all predicates". Once either
/// list is non-empty, the union is exact. Render-rule fields use positional
/// names (`arg0`, `arg1`, ...); an unknown field or template placeholder is a
/// profile error rather than a silently empty column.
pub fn rows(spec: &ProfileSpec, atoms: &[&LoadedAtom]) -> Result<Vec<ProjectionRow>, ExportError> {
    let wildcard = spec.selects.sorts.is_empty() && spec.selects.relations.is_empty();
    let with_provenance = matches!(spec.carries_provenance, ProvenanceMode::DevMetadata);
    let mut rows = Vec::new();

    for atom in atoms {
        let predicate = atom.atom.pred.text.as_str();
        let kind = if spec.selects.sorts.iter().any(|name| name == predicate) {
            ProjectionKind::Sort
        } else if wildcard || spec.selects.relations.iter().any(|name| name == predicate) {
            ProjectionKind::Relation
        } else {
            continue;
        };
        let claim = GroundAtom::from_pattern(&atom.atom)
            .map(|ground| ground.render())
            .ok_or_else(|| ExportError::Render {
                target: spec.target.to_string(),
                detail: format!("selected atom `{predicate}` contains a non-ground argument"),
            })?;
        let available = atom
            .atom
            .args
            .iter()
            .enumerate()
            .filter_map(|(index, term)| {
                render_term(term).map(|value| (format!("arg{index}"), value))
            })
            .collect::<BTreeMap<_, _>>();
        let rule = spec.renders.get(predicate);
        let arguments = retained_arguments(spec, predicate, rule, &available)?;
        let id = match rule {
            Some(rule) if !rule.id.is_empty() => {
                render_id(spec, predicate, &rule.id, &claim, &available)?
            }
            _ => claim.clone(),
        };
        let mut cited = if with_provenance {
            atom.citations.clone()
        } else {
            Vec::new()
        };
        cited.sort();
        cited.dedup();
        rows.push(ProjectionRow {
            id,
            kind,
            predicate: predicate.to_owned(),
            arguments,
            claim,
            cited,
        });
    }

    rows.sort_by(|left, right| {
        (&left.predicate, &left.id, &left.claim).cmp(&(&right.predicate, &right.id, &right.claim))
    });
    rows.dedup();
    Ok(rows)
}

/// Render a profile id template from canonical row fields.
pub(crate) fn render_id(
    spec: &ProfileSpec,
    predicate: &str,
    template: &str,
    claim: &str,
    arguments: &BTreeMap<String, String>,
) -> Result<String, ExportError> {
    let mut fields = arguments.clone();
    fields.insert("predicate".to_owned(), predicate.to_owned());
    fields.insert("claim".to_owned(), claim.to_owned());
    render_template(spec, predicate, template, &fields)
}

pub(crate) fn render_template(
    spec: &ProfileSpec,
    predicate: &str,
    template: &str,
    fields: &BTreeMap<String, String>,
) -> Result<String, ExportError> {
    let mut rendered = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        if rest[..open].contains('}') {
            return Err(invalid_profile(
                spec,
                format!("render template `{template}` has an unmatched `}}`"),
            ));
        }
        rendered.push_str(&rest[..open]);
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('}') else {
            return Err(invalid_profile(
                spec,
                format!("render template `{template}` has an unmatched `{{`"),
            ));
        };
        let field = &after_open[..close];
        if field.is_empty() || field.contains('{') {
            return Err(invalid_profile(
                spec,
                format!("render template `{template}` has an invalid placeholder"),
            ));
        }
        let value = fields.get(field).ok_or_else(|| {
            invalid_profile(
                spec,
                format!(
                    "render template `{template}` names unavailable field `{field}` for `{predicate}`"
                ),
            )
        })?;
        rendered.push_str(value);
        rest = &after_open[close + 1..];
    }
    if rest.contains('}') {
        return Err(invalid_profile(
            spec,
            format!("render template `{template}` has an unmatched `}}`"),
        ));
    }
    rendered.push_str(rest);
    if rendered.is_empty() {
        return Err(invalid_profile(
            spec,
            format!("render template for `{predicate}` produced an empty id"),
        ));
    }
    Ok(rendered)
}

fn retained_arguments(
    spec: &ProfileSpec,
    predicate: &str,
    rule: Option<&RenderRule>,
    available: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, ExportError> {
    let Some(rule) = rule else {
        return Ok(available.clone());
    };
    if rule.keep.is_empty() {
        return Ok(available.clone());
    }
    let mut retained = BTreeMap::new();
    for field in &rule.keep {
        let value = available.get(field).ok_or_else(|| {
            invalid_profile(
                spec,
                format!("render rule for `{predicate}` keeps unavailable field `{field}`"),
            )
        })?;
        retained.insert(field.clone(), value.clone());
    }
    Ok(retained)
}

fn invalid_profile(spec: &ProfileSpec, detail: String) -> ExportError {
    ExportError::InvalidProfile {
        profile: spec.qualified(),
        detail,
    }
}

fn render_term(term: &dli_ast::term::Term) -> Option<String> {
    use dli_ast::term::{Term, TimeRef};
    match term {
        Term::Sym(id) => Some(id.text.clone()),
        Term::Str(text) => Some(text.clone()),
        Term::Int(value) => Some(value.to_string()),
        Term::Time(TimeRef::Era(id)) => Some(format!("era:{}", id.text)),
        Term::Time(TimeRef::Event(id)) => Some(format!("event:{}", id.text)),
        Term::Var(_) => None,
    }
}
