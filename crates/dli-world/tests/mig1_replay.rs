//! MIG-1 — the `facts.yaml` read-through with stored-status cross-checks.
//!
//! Migration phase 1 (`docs/integration/migration.md` §4) reads the vendored
//! `facts.yaml` directly and, for every row, cross-checks the stored `status:`
//! field against the standing the **kernel** derives from the row's `decisions:`
//! under the wave-1 semantics. Any disagreement is a *finding*, never a silent
//! fix (the migration law); this test file is the standing record of that
//! cross-check plus its detection proof.
//!
//! # The wave-1 derivation
//!
//! A row's kernel standing is [`dli_core::ann::Ann::status`] over a support
//! polynomial built exactly as the loader builds it (`crates/dli-world/src/
//! loader.rs::compile_ann`), evaluated under the *cited-decisions-default-true*
//! valuation:
//!
//! - A **ledger** row (`ratified` / `provisional` / `decided`) is supported by
//!   the disjunction of its cited decisions; each cited decision is assumed
//!   ruled true, so the support is [`Status::Certain`]. A ledger row citing **no**
//!   decision has no ruling behind it, so its support is `P⁺ = 0` and it derives
//!   [`Status::Dead`] — fail-closed, the same theorem the loader enforces.
//! - A **proposed** row is gated by an *open* hypothesis variable that is never
//!   ruled in this valuation, so it derives [`Status::Possible`] — the shadow
//!   layer, structurally unassertable.
//! - A **rejected** row's support is `P⁺ = 0`: [`Status::Dead`].
//! - Any **other** status token cannot be resolved to a provenance regime; the
//!   row is flagged directly (the data-layer analogue of the corpus's
//!   `break-status-marker` operator).
//!
//! The stored status maps to the same three-valued lattice
//! (`ratified`/`provisional`/`decided` ⇒ Certain, `proposed` ⇒ Possible,
//! `rejected` ⇒ Dead); a disagreement is a row where the two differ.
//!
//! # Detection is proven separately from parity
//!
//! The pristine cross-check finds **zero** disagreements (the pinned corpus is
//! internally consistent). A harness that only ever reports agreement is a null
//! oracle (`migration.md` §3), so the `detects_*` tests seed the data-layer
//! analogues of three named corpus operators
//! (`fixtures/mutation/canon.yaml`) and assert each one turns the clean
//! cross-check into a finding. The zero above counts as parity only because the
//! same derivation demonstrably *catches* seeded divergence.

#![allow(clippy::unwrap_used, clippy::print_stdout, clippy::print_stderr)]

use std::fmt;

use serde::Deserialize;

use dli_core::ann::{Ann, Status, Theory};
use dli_core::atom::Interner;
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;

// ---------------------------------------------------------------------------
// The vendored source: the pinned facts.yaml, read directly (phase-1 read-through).
// ---------------------------------------------------------------------------

const PINNED_FACTS_YAML: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/vendor/fantasy-40f6679/canon/data/facts.yaml"
);

/// The facts.yaml the replay reads. Defaults to the pinned vendored snapshot;
/// the standing differential harness (`scripts/differential-oracle.sh`)
/// overrides it with `MIG1_FACTS_YAML` to re-run the read-through over a
/// mutated *temp copy* of the corpus and prove the catch (detection is proven
/// separately from parity — `migration.md` §3).
fn facts_path() -> String {
    std::env::var("MIG1_FACTS_YAML").unwrap_or_else(|_| PINNED_FACTS_YAML.to_owned())
}

/// The `facts.yaml` document, deserialized through the fields MIG-1 needs. Extra
/// fields (`name`, `kind`, `value`, `note`, `machine_check`, …) are ignored:
/// the read-through cross-checks provenance standing, not payload shape.
#[derive(Debug, Deserialize)]
struct FactsDoc {
    #[serde(default)]
    entities: Vec<EntityRow>,
    #[serde(default)]
    relations: Vec<RelationRow>,
    #[serde(default)]
    facts: Vec<FactRow>,
    /// Guardrail constraints carry `decisions:` but no ratification `status:`;
    /// they are counted for the census but are outside the status cross-check.
    #[serde(default)]
    constraints: Vec<serde_yaml_ng::Value>,
}

#[derive(Debug, Deserialize)]
struct EntityRow {
    id: String,
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RelationRow {
    subject: String,
    kind: String,
    object: String,
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FactRow {
    entity: String,
    key: String,
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
}

/// One provenance-bearing row, flattened across the three sections into the
/// single shape the cross-check consumes.
#[derive(Debug, Clone)]
struct Row {
    /// A human-readable locator for reporting (`entity gwern`,
    /// `relation located_in(trefpen, gwern)`, `fact protagonist.given-name`).
    locator: String,
    /// The stored ratification status token, verbatim.
    status: String,
    /// The cited decision ids, verbatim.
    decisions: Vec<String>,
}

fn load_doc() -> FactsDoc {
    let path = facts_path();
    let text = std::fs::read_to_string(&path).expect("facts.yaml is present");
    serde_yaml_ng::from_str(&text).expect("facts.yaml deserializes through the MIG-1 view")
}

/// The three sections flattened into provenance rows, in file order.
fn load_rows() -> Vec<Row> {
    let doc = load_doc();
    let mut rows = Vec::new();
    for e in doc.entities {
        rows.push(Row {
            locator: format!("entity {}", e.id),
            status: e.status,
            decisions: e.decisions,
        });
    }
    for r in doc.relations {
        rows.push(Row {
            locator: format!("relation {}({}, {})", r.kind, r.subject, r.object),
            status: r.status,
            decisions: r.decisions,
        });
    }
    for f in doc.facts {
        rows.push(Row {
            locator: format!("fact {}.{}", f.entity, f.key),
            status: f.status,
            decisions: f.decisions,
        });
    }
    rows
}

// ---------------------------------------------------------------------------
// The derivation: kernel standing vs stored status (the wave-1 semantics).
// ---------------------------------------------------------------------------

/// A row's three-valued standing, projected off [`Status`] so stored and derived
/// sides compare in one lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Certain,
    Possible,
    Dead,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Outcome::Certain => "certain",
            Outcome::Possible => "possible",
            Outcome::Dead => "dead",
        };
        f.write_str(s)
    }
}

fn to_outcome(status: Status) -> Outcome {
    match status {
        Status::Certain => Outcome::Certain,
        Status::Possible => Outcome::Possible,
        Status::Dead => Outcome::Dead,
    }
}

/// The standing the kernel derives from a row's `status:`/`decisions:` under the
/// cited-decisions-default-true valuation. `None` means the status token cannot
/// be resolved to a provenance regime — itself a finding.
fn derive_outcome(status: &str, decisions: &[String]) -> Option<Outcome> {
    let mut interner = Interner::new();
    let mut nu = Valuation::new();
    let support = match status {
        "ratified" | "provisional" | "decided" => {
            // A ledger claim is supported by *any one* of its cited decisions;
            // each cited decision is assumed ruled true (default-true). Empty
            // citations leave the support identically false — the fail-closed
            // theorem: a ledger claim with no ruling has nowhere to stand.
            let mut acc = PosBool::FALSE;
            for decision in decisions {
                let atom = interner.intern(decision.trim());
                nu.set(atom, true);
                acc = acc.or_(&PosBool::pos(atom));
            }
            acc
        }
        "proposed" => {
            // Gated by an open hypothesis variable, never ruled in this
            // valuation: structurally shadow (Possible), never Certain.
            PosBool::pos(interner.intern("h.open"))
        }
        "rejected" => PosBool::FALSE,
        _ => return None,
    };
    Some(to_outcome(
        Ann::supported(support).status(&nu, &Theory::new()),
    ))
}

/// The standing the *stored* status field claims, in the same lattice. `None`
/// means the token is not a recognized ratification marker.
fn expected_outcome(status: &str) -> Option<Outcome> {
    match status {
        "ratified" | "provisional" | "decided" => Some(Outcome::Certain),
        "proposed" => Some(Outcome::Possible),
        "rejected" => Some(Outcome::Dead),
        _ => None,
    }
}

/// A single stored-vs-derived disagreement.
#[derive(Debug, Clone)]
struct Disagreement {
    locator: String,
    status: String,
    expected: Option<Outcome>,
    derived: Option<Outcome>,
    reason: String,
}

impl fmt::Display for Disagreement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] status='{}': {}",
            self.locator, self.status, self.reason
        )
    }
}

/// Cross-check one row: `None` when stored and derived standings agree.
fn cross_check(row: &Row) -> Option<Disagreement> {
    let expected = expected_outcome(&row.status);
    let derived = derive_outcome(&row.status, &row.decisions);
    let reason = match (expected, derived) {
        (Some(e), Some(d)) if e == d => return None,
        (Some(e), Some(d)) => format!(
            "stored '{}' implies {e}, kernel derives {d} \
             (a ledger claim with no live ruling)",
            row.status
        ),
        (None, _) => format!(
            "status '{}' is not a recognized ratification marker — it cannot \
             resolve to a provenance regime",
            row.status
        ),
        (Some(e), None) => format!(
            "stored '{}' implies {e}, but the kernel cannot derive a standing",
            row.status
        ),
    };
    Some(Disagreement {
        locator: row.locator.clone(),
        status: row.status.clone(),
        expected,
        derived,
        reason,
    })
}

/// Every disagreement across a row set, in file order.
fn disagreements(rows: &[Row]) -> Vec<Disagreement> {
    rows.iter().filter_map(cross_check).collect()
}

// ---------------------------------------------------------------------------
// The census: the pinned corpus shape (guards against fixture drift).
// ---------------------------------------------------------------------------

#[test]
fn vendored_facts_row_census_is_pinned() {
    if std::env::var_os("MIG1_FACTS_YAML").is_some() {
        // The pinned census only describes the frozen snapshot; the harness's
        // detection leg overrides the path with a mutated temp copy, where these
        // counts deliberately no longer hold.
        return;
    }
    let doc = load_doc();
    assert_eq!(doc.entities.len(), 23, "entities rows (pinned 40f6679)");
    assert_eq!(doc.relations.len(), 12, "relations rows (pinned 40f6679)");
    assert_eq!(doc.facts.len(), 156, "facts rows (pinned 40f6679)");
    assert_eq!(
        doc.constraints.len(),
        3,
        "guardrail constraints (no status)"
    );

    let rows = load_rows();
    assert_eq!(
        rows.len(),
        191,
        "cross-checked provenance rows = 23 + 12 + 156"
    );

    let proposed = rows.iter().filter(|r| r.status == "proposed").count();
    let provisional = rows.iter().filter(|r| r.status == "provisional").count();
    let ratified = rows.iter().filter(|r| r.status == "ratified").count();
    assert_eq!(proposed, 7, "proposed shadow rows");
    assert_eq!(provisional, 9, "provisional ledger rows");
    assert_eq!(ratified, 175, "ratified ledger rows");
    assert_eq!(
        proposed + provisional + ratified,
        rows.len(),
        "no other status token"
    );
}

// ---------------------------------------------------------------------------
// The characterization: the pristine cross-check (the parity baseline).
// ---------------------------------------------------------------------------

#[test]
fn pristine_facts_have_zero_status_disagreements() {
    let rows = load_rows();
    let ledger = rows
        .iter()
        .filter(|r| matches!(r.status.as_str(), "ratified" | "provisional" | "decided"))
        .count();
    let proposed = rows.iter().filter(|r| r.status == "proposed").count();
    let dis = disagreements(&rows);

    // Surface the full cross-check for `--nocapture` transcripts.
    eprintln!(
        "MIG-1 replay @ 40f6679: {} rows cross-checked ({ledger} ledger, {proposed} proposed), \
         {} disagreement(s)",
        rows.len(),
        dis.len()
    );
    for d in &dis {
        eprintln!("  DISAGREEMENT: {d}");
    }

    // First-run characterization: every ledger row cites a live ruling and every
    // proposed row cites none, so the stored status and the derived standing
    // agree on every row — ZERO disagreements. This is the parity BASELINE, not a
    // null oracle: the `detects_*` tests prove the same derivation catches seeded
    // divergence. Were this nonzero, each case would be pinned here with an
    // explanation (a disagreement is a finding, never a silent fix).
    assert_eq!(
        dis.len(),
        0,
        "the pinned facts.yaml has no stored-vs-derived status disagreement"
    );
}

// ---------------------------------------------------------------------------
// Detection proven separately: the data-layer analogues of three named corpus
// operators (fixtures/mutation/canon.yaml) must each become a finding.
// ---------------------------------------------------------------------------

#[test]
fn detects_flip_proposed_to_ratified() {
    // `flip-proposed-to-ratified` (canon.yaml): promote a proposed row to
    // ratified with no decision citation. The forged promotion has no ruling
    // behind it, so the kernel derives Dead, not the claimed Certain.
    let mut rows = load_rows();
    let target = rows
        .iter_mut()
        .find(|r| r.status == "proposed")
        .expect("a proposed row exists to promote");
    assert!(
        target.decisions.is_empty(),
        "the proposed row cites nothing"
    );
    target.status = "ratified".to_owned(); // no decisions added — the forgery

    let dis = disagreements(&rows);
    assert_eq!(
        dis.len(),
        1,
        "exactly the seeded row disagrees; the rest stay clean"
    );
    assert_eq!(dis[0].derived, Some(Outcome::Dead));
    assert_eq!(dis[0].expected, Some(Outcome::Certain));
}

#[test]
fn detects_forged_promotion_strip_decisions() {
    // `forged-promotion` (canon.yaml): a ratified claim whose citation is
    // stripped keeps its ledger status but loses its ruling. It must derive Dead.
    let mut rows = load_rows();
    let target = rows
        .iter_mut()
        .find(|r| r.status == "ratified" && !r.decisions.is_empty())
        .expect("a cited ratified row exists");
    target.decisions.clear();

    let dis = disagreements(&rows);
    assert_eq!(dis.len(), 1, "only the stripped row disagrees");
    assert_eq!(dis[0].derived, Some(Outcome::Dead));
    assert_eq!(dis[0].expected, Some(Outcome::Certain));
}

#[test]
fn detects_break_status_marker() {
    // `break-status-marker` (canon.yaml): corrupt the status so it no longer
    // resolves to a provenance regime. Its exact corruption shape drops the
    // decision into the token; the resulting status is unrecognized and is
    // flagged directly (expected standing = None).
    let mut rows = load_rows();
    rows[0].status = "ratified D-0003".to_owned();

    let dis = disagreements(&rows);
    assert_eq!(dis.len(), 1, "only the corrupted row disagrees");
    assert!(
        dis[0].expected.is_none(),
        "an unresolvable status is a finding"
    );
    assert!(
        dis[0].derived.is_none(),
        "the kernel cannot derive a standing either"
    );
}

#[test]
fn detection_is_not_a_null_oracle() {
    // The guard the whole file rests on: the pristine corpus is clean, so any of
    // the three seeded operators is the *sole* cause of the finding it raises.
    let pristine = disagreements(&load_rows());
    assert!(
        pristine.is_empty(),
        "pristine baseline must be clean for detection to mean anything"
    );
}
