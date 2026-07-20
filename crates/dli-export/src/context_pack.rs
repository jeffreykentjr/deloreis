//! Export backend: the context-pack profile (`EXP-2`).
//!
//! The audio-script context pack is **export backend number one, already in
//! production** (spec 09 §3): the pack builder's certain-layer default and
//! fail-closed behavior, recognised as what they are and generalised. This module
//! reproduces the shape today's fantasy context-pack builder emits.
//!
//! # Parity with the fantasy `pack.py` builder
//!
//! The oracle is `src/fantasy/canon/pack.py` (pinned SHA `998ce598`). Its builder
//! is: keep only in-scope content → sort deterministically by `(status rank,
//! path)` → concatenate `<!-- canon: {path} ({status}) -->`-headed blocks → drop
//! whole blocks from the tail when the token budget is exceeded → estimate tokens
//! as `len(text) // 4`. This module reproduces **that algorithm exactly**:
//!
//! | fantasy `pack.py`            | here                                    |
//! |-----------------------------|-----------------------------------------|
//! | canon file (`relpath`)      | a source **pack** (the atom's `pack`)   |
//! | file-level `status` filter  | the **certain-layer** filter (upstream) |
//! | `PackedFile{relpath,status}`| [`PackedUnit`] `{unit, layer}`          |
//! | `estimate_tokens = len//4`  | [`estimate_tokens`]                      |
//! | tail-drop over budget       | identical                               |
//! | `<!-- canon: … -->` header  | identical                               |
//!
//! **Byte-parity of `text` is not the parity claim** and cannot be: the fantasy
//! corpus is status-marked markdown, this one is the certain atoms of a compiled
//! world. The parity is *structural* — the same builder, so a consumer that ate
//! the fantasy pack eats this one. The fail-closed status filter is not repeated
//! here; it already happened in [`certain_atoms`](crate::profile::certain_atoms),
//! so a proposed atom can never reach this builder.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_eval::world::GroundAtom;
use dli_world::model::LoadedAtom;

use crate::profile::{ProfileSpec, ProvenanceMode};

/// The default token budget, matching `pack.py`'s `DEFAULT_BUDGET_TOKENS`.
pub const DEFAULT_BUDGET_TOKENS: usize = 30_000;

/// The cheap deterministic token estimate `pack.py` uses: characters // 4.
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// One unit's placement in (or exclusion from) the pack — the analogue of
/// `pack.py`'s `PackedFile`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackedUnit {
    /// The unit's identity — a source pack name (the `relpath` analogue).
    pub unit: String,
    /// The layer it was drawn from (`certain` / `fork`) — the `status` analogue.
    pub layer: String,
    /// The estimated token cost of the unit's rendered block.
    pub est_tokens: usize,
}

/// The assembled context pack plus its inclusion/exclusion manifest — the shape
/// `pack.py`'s `ContextPack` emits.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextPack {
    /// The concatenated pack text: `<!-- canon: … -->`-headed blocks joined by a
    /// blank line.
    pub text: String,
    /// The units that fit the budget, in emission order.
    pub included: Vec<PackedUnit>,
    /// The units dropped from the tail because the budget was exceeded.
    pub skipped: Vec<PackedUnit>,
    /// The estimated token cost of the whole pack (`len(text) // 4`).
    pub est_tokens: usize,
}

/// Build a context pack from a run of certain atoms under the default budget.
#[must_use]
pub fn build(spec: &ProfileSpec, layer: &str, atoms: &[&LoadedAtom]) -> ContextPack {
    build_with_budget(spec, layer, atoms, DEFAULT_BUDGET_TOKENS)
}

/// Build a context pack under an explicit token budget.
///
/// Atoms are grouped into blocks by their source pack, each block headed
/// `<!-- canon: {pack} ({layer}) -->`; blocks sort by pack name (all share the one
/// export layer); over budget, whole blocks drop from the tail, exactly as
/// `pack.py` does.
///
/// ```
/// use dli_export::context_pack::{build_with_budget, estimate_tokens};
/// use dli_export::profile::{certain_atoms, Registry, ExportTarget};
/// use dli_world::{compile, pack::Pack, loader::example_docs};
///
/// let world = compile(&Pack { name: "saltmarket".to_owned(), documents: example_docs() })
///     .expect("compiles");
/// let atoms = certain_atoms(&world);
/// let registry = Registry::builtin();
/// let spec = registry.default_for(ExportTarget::ContextPack).unwrap();
/// let pack = build_with_budget(spec, "certain", &atoms, 30_000);
/// // The est_tokens invariant holds by construction.
/// assert_eq!(pack.est_tokens, estimate_tokens(&pack.text));
/// // A zero budget includes nothing and skips everything present.
/// let empty = build_with_budget(spec, "certain", &atoms, 0);
/// assert!(empty.included.is_empty());
/// assert!(empty.text.is_empty());
/// ```
#[must_use]
pub fn build_with_budget(
    spec: &ProfileSpec,
    layer: &str,
    atoms: &[&LoadedAtom],
    budget_tokens: usize,
) -> ContextPack {
    // Group by source pack, deterministically (BTreeMap keys sort). Within a pack,
    // atoms sort by their rendered form.
    let mut by_pack: std::collections::BTreeMap<&str, Vec<&LoadedAtom>> =
        std::collections::BTreeMap::new();
    for atom in atoms {
        by_pack.entry(atom.pack.as_str()).or_default().push(atom);
    }

    let mut candidates: Vec<(PackedUnit, String)> = Vec::new();
    for (pack, mut members) in by_pack {
        members.sort_by_key(|atom| render_atom(atom));
        let block = render_block(pack, layer, &members, spec.carries_provenance);
        let cost = estimate_tokens(&block);
        candidates.push((
            PackedUnit {
                unit: pack.to_owned(),
                layer: layer.to_owned(),
                est_tokens: cost,
            },
            block,
        ));
    }

    let mut included = Vec::new();
    let mut skipped = Vec::new();
    let mut kept_blocks = Vec::new();
    let mut running = 0usize;
    let mut over = false;
    for (unit, block) in candidates {
        if over || running + unit.est_tokens > budget_tokens {
            over = true;
            skipped.push(unit);
        } else {
            running += unit.est_tokens;
            included.push(unit);
            kept_blocks.push(block);
        }
    }

    let text = kept_blocks.join("\n\n");
    let est_tokens = estimate_tokens(&text);
    ContextPack {
        text,
        included,
        skipped,
        est_tokens,
    }
}

/// Render one pack's block: the canon header plus one line per atom.
fn render_block(
    pack: &str,
    layer: &str,
    members: &[&LoadedAtom],
    provenance: ProvenanceMode,
) -> String {
    let mut block = format!("<!-- canon: {pack} ({layer}) -->");
    for atom in members {
        block.push('\n');
        block.push_str(&render_atom(atom));
        if matches!(provenance, ProvenanceMode::DevMetadata) && !atom.citations.is_empty() {
            block.push_str("  ; cited ");
            block.push_str(&atom.citations.join(", "));
        }
    }
    block
}

/// Render an atom's `pred(args)` form; a non-ground atom (never a fact) falls
/// back to its predicate.
fn render_atom(atom: &LoadedAtom) -> String {
    GroundAtom::from_pattern(&atom.atom)
        .map_or_else(|| atom.atom.pred.text.clone(), |ground| ground.render())
}
