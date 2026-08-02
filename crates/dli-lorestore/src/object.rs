//! The stored fact object model — the new language definition (DL-0041,
//! roadmap §LS-1).
//!
//! A [`FactObject`] is the content-addressed, schema-versioned stored
//! representation of one ground fact. It carries the same two-channel
//! annotation (`P⁺`, `P⁻`) the evaluator produces, but with three identity
//! fields that the in-memory `Fact` type does not:
//!
//! - [`schema_version`](FactObject::schema_version) — for upcast-on-read
//! - [`authority`](FactObject::authority) — unset in sovereign deployments
//! - [`claim_id`](FactObject::claim_id) — stable across revisions
//!
//! The stored polynomial form is **absorption-normalized irredundant monotone
//! DNF** with sorted monomials — the same normal form the evaluator produces
//! at every join (§4 of the roadmap). Because irredundant monotone DNF is
//! unique for a monotone Boolean function, equal provenance means equal bytes
//! means equal hash — the invariant that makes content-addressing sound.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION;
use crate::identity::{Authority, ClaimId};
use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_eval::world::GroundAtom;

/// One poisoned support-monomial / defeater pair, the per-path defeater
/// condition. Reuses the `CondPairWire` idiom from `dli_core::ann` so the
/// wire form is the same single-monomial `path` polynomial plus its `poison`.
///
/// The array is empty for every positive-only atom, so positive datalog is
/// byte-identical — the field is omitted from serialization when empty.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct CondPair {
    /// The poisoned support monomial, as a single-monomial polynomial.
    pub path: PosBool,
    /// The defeater polynomial `P⁻` pinned to that path.
    pub poison: PosBool,
}

impl CondPair {
    /// Construct a condition pair from a single-monomial support and its
    /// defeater.
    #[must_use]
    pub fn new(path: PosBool, poison: PosBool) -> Self {
        Self { path, poison }
    }
}

/// The kind of expert-tier declaration a [`FactObject::expert_body`] belongs to.
///
/// The coupling audit (`dli-syntax-coupling-audit.md`) found eleven declaration
/// types whose bodies are captured as `Vec<Clause>` (whitespace-normalized raw
/// text), re-parsed downstream at semantic-analysis time. In v1 the fact-object
/// schema carries these bodies as marked strings — deterministic to hash, but
/// semantically opaque — and the `schema_version` + upcast machinery is how
/// they later deepen to typed interiors (LS-9). Each variant is tagged so the
/// upcast knows which declaration type to deepen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExpertDeclKind {
    /// `rule <name> [over <domain>] … { body }`
    Rule,
    /// `fork <question> { body }`
    Fork,
    /// `practice <name> { body }`
    Practice,
    /// `dialect <name> { body }`
    Dialect,
    /// `draft <name> { body }`
    Draft,
    /// `export <profile> { body }`
    Export,
    /// `name <name> { body }`
    Name,
    /// `variant <path> { body }`
    Variant,
    /// `ratify <d-id> { body }`
    Ratify,
    /// `principle <name> { body }`
    Principle,
    /// `proof <name> { body }`
    Proof,
    /// `kind <name>` — a sort declaration.
    Kind,
    /// `state …` — a fluent-state declaration.
    State,
    /// `cycle <name> …` — a cycle declaration.
    Cycle,
    /// `question <name> { options }` — carried whole so option families and
    /// implied atoms regenerate on recompile.
    Question,
    /// `hypothesis <name>` — carried so `suppose` targets survive the store.
    Hypothesis,
    /// `use <path>` — an import declaration.
    Use,
    /// A lexicon `word` declaration.
    Word,
    /// A `namepart` declaration.
    NamePart,
    /// An `attest` declaration.
    Attest,
    /// A `term` declaration.
    Term,
    /// A `banned` declaration.
    Banned,
    /// A `narration` declaration.
    Narration,
    /// An `entity` declaration that carries more than its ground atom —
    /// a display label, a time clause, or indented slots — and so cannot be
    /// reconstructed from the atom object alone.
    Entity,
    /// A `before` ordering declaration — carried because its ground atom is
    /// indistinguishable from an ordinary `before(a, b)` fact, and only the
    /// declaration form adds a chronology edge.
    Before,
}

/// A declaration carried as a marked string in v1 (the expert-tier debt rule,
/// roadmap §LS-1).
///
/// The `kind` + `name` tags identify which declaration type this body belongs
/// to, so LS-9's typed upcasts know what interior to deepen. In the v1
/// encoding `body` holds exactly one element: the canonical JSON serialization
/// of the whole [`Decl`](dli_ast::decl::Decl) — deterministic to hash, exactly
/// reconstructable by serde, semantically opaque to the store. (For the eleven
/// string-bodied expert-tier types that JSON in turn embeds their
/// whitespace-normalized clause strings — debt inside debt, both deepened by
/// LS-9.) `head` is reserved for header tokens and empty in the v1 encoding.
/// This is explicitly **named, versioned debt** — the field is marked so
/// nobody mistakes an embedded encoding for a structured interior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExpertBody {
    /// Which of the eleven expert-tier declaration kinds this body is.
    pub kind: ExpertDeclKind,
    /// The declaration name (or the decision id for `Ratify`).
    pub name: String,
    /// Header tokens after the name (e.g. `severity error`, `speaker protagonist
    /// in spoken-proof`, `over order`). Empty for declarations with no header.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub head: Vec<String>,
    /// The whitespace-normalized clause body lines, in source order.
    pub body: Vec<String>,
}

impl ExpertBody {
    /// Construct an expert body for a declaration.
    #[must_use]
    pub fn new(kind: ExpertDeclKind, name: &str, head: Vec<String>, body: Vec<String>) -> Self {
        Self {
            kind,
            name: name.to_owned(),
            head,
            body,
        }
    }
}

/// The stored fact object: a ground atom with its two-channel annotation.
///
/// This is the normative stored form of a fact (the schema that `schema_for!`
/// emits and the gate checks for drift). It is the persistent analogue of
/// `dli_eval::world::Fact` — same `atom`, `support`, `defeaters`, `grade`,
/// but with `schema_version`, `authority`, `claim_id` added and the
/// `DefeaterConditions` flattened to a `Vec<CondPair>` for a self-describing
/// stored shape.
///
/// **Three-way identity split** (DL-0041, roadmap §LS-1):
/// - The content hash (computed over these fields) never embeds a parent
///   pointer — authority distinctness is handled by `authority` participating
///   in the hash, so the same proposition by two authorities is two objects.
/// - `claim_id` is stable across revisions for claim continuity.
/// - Supersession history lives in `LineageEdge` objects with their own
///   `(P⁺, P⁻)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FactObject {
    /// Schema version; bumped when the stored shape changes.
    pub schema_version: u32,
    /// The authority that asserted this fact. `None` in single-sovereign
    /// deployments (DEF-4 unchanged). Participates in the content hash so two
    /// authorities asserting the same proposition produce distinct objects.
    pub authority: Authority,
    /// A stable minted claim id for continuity across revisions. Not derived
    /// from content — the content hash does not reference it.
    pub claim_id: ClaimId,
    /// The ground atom this fact asserts.
    pub atom: GroundAtom,
    /// `P⁺` — the support polynomial: under which hypotheses/rulings the claim
    /// holds. Absorption-normalized irredundant monotone DNF, monomials and
    /// variables in lexicographic order. The empty polynomial (`PosBool::FALSE`)
    /// is the fail-closed reading — no valuation can make the fact hold.
    pub support: PosBool,
    /// `P⁻` — the atom-global defeater polynomial: what could rule the claim
    /// dead. Same canonical DNF form. `FALSE` for positive-only atoms.
    pub defeaters: PosBool,
    /// Per-monomial defeater conditions — the poison, conditioned per support
    /// monomial. Empty (omitted) for every positive-only atom.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<CondPair>,
    /// The grade at which this fact is known: `checked > tested > attested >
    /// asserted` (DL-0006).
    pub grade: Grade,
    /// The expert-tier clause body, if this fact originates from one of the
    /// eleven declaration types whose body is captured as raw text. `None` for
    /// gentle-subset facts. Present only in v1; deepened to typed interiors in
    /// LS-9 via the upcast machinery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expert_body: Option<ExpertBody>,
}

impl FactObject {
    /// Construct a fact object with sovereign authority and v1 schema.
    ///
    /// The `support` and `defeaters` polynomials must already be in
    /// absorption-normalized irredundant monotone DNF — the `PosBool` type
    /// guarantees this on construction (its constructors call `absorb`).
    #[must_use]
    pub fn new(
        claim_id: ClaimId,
        atom: GroundAtom,
        support: PosBool,
        defeaters: PosBool,
        conditions: Vec<CondPair>,
        grade: Grade,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            authority: Authority::sovereign(),
            claim_id,
            atom,
            support,
            defeaters,
            conditions,
            grade,
            expert_body: None,
        }
    }

    /// Construct a fact object carrying an expert-tier clause body.
    #[must_use]
    pub fn with_expert_body(
        claim_id: ClaimId,
        atom: GroundAtom,
        support: PosBool,
        defeaters: PosBool,
        conditions: Vec<CondPair>,
        grade: Grade,
        expert_body: ExpertBody,
    ) -> Self {
        Self {
            expert_body: Some(expert_body),
            ..Self::new(claim_id, atom, support, defeaters, conditions, grade)
        }
    }

    /// Whether this fact is fail-closed: its support is identically false, so
    /// no valuation can ever make it hold (spec 02 §3).
    #[must_use]
    pub fn is_fail_closed(&self) -> bool {
        self.support.is_false()
    }

    /// The content hash of this fact object — over the **content fields only**:
    /// `authority` (so two authorities asserting the same proposition produce
    /// distinct objects), `atom`, `support`, `defeaters`, `conditions`, `grade`,
    /// and `expert_body`.  Excluded: `schema_version` (versioning tag, not
    /// content) and `claim_id` (a stable continuity handle that must survive
    /// revision without staling the digest).  Computed via JCS
    /// canonicalization + SHA-256 (DL-0041, roadmap §LS-1).
    ///
    /// This is the field that participates in `Commit`/content addressing — a
    /// parent pointer is deliberately *not* embedded in the hash; storage
    /// ancestry lives in the commit DAG only.
    pub fn content_hash(
        &self,
    ) -> Result<crate::identity::ContentHash, crate::canonical::CanonicalError> {
        /// The subset of `FactObject` fields whose canonical bytes define
        /// content addressability.  `claim_id` and `schema_version` are excluded:
        /// the former is a continuity handle, the latter a version tag — neither
        /// changes the proposition or its provenance.
        #[derive(serde::Serialize)]
        struct HashableContent<'a> {
            authority: &'a crate::identity::Authority,
            atom: &'a dli_eval::world::GroundAtom,
            support: &'a PosBool,
            defeaters: &'a PosBool,
            conditions: &'a Vec<CondPair>,
            grade: &'a Grade,
            #[serde(skip_serializing_if = "Option::is_none")]
            expert_body: &'a Option<ExpertBody>,
        }
        let content = HashableContent {
            authority: &self.authority,
            atom: &self.atom,
            support: &self.support,
            defeaters: &self.defeaters,
            conditions: &self.conditions,
            grade: &self.grade,
            expert_body: &self.expert_body,
        };
        crate::canonical::content_hash(&content)
    }
}
