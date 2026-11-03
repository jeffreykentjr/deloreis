//! The world-check battery (`CHECK-1`): continuity and factcheck.
//!
//! A [`CompiledWorld`] is what [`compile`](crate::compile) produces: the loader's
//! status-gated [`WorldModel`] with its ledger valuation set and its symbolic
//! fixpoint computed, plus the negative-constraint store (the killed-token
//! guardrails). Two batteries read it:
//!
//! - [`run`] runs the world-internal continuity classes ([`continuity`]) — the
//!   self-consistency of the compiled world, needing no external delta.
//! - [`factcheck`](factcheck::factcheck) classifies a set of asserted claims as
//!   supported / contradicted / unsupported against the certain projection.
//!
//! Both speak one finding type, [`WorldFinding`]: a dli-world-local record with a
//! stable dotted [`code`](WorldFinding::code), a [`Severity`], a verification
//! [`Grade`], a domain-spoken message, its D-citations, and an optional source
//! [`Span`]. `dli-serve`'s wire `Finding` sits *above* this and is mapped from it
//! in a later wave; this type is the kernel-local truth.

pub mod continuity;
mod entity;
pub mod factcheck;

pub use continuity::{SlotDefeater, SlotHolding, SlotStateAt, identity_canon};
pub use dli_core::fd::fd_value_key;

use dli_core::grade::Grade;

use dli_ast::span::Span;
use dli_time::chronology::TimePoint;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::WorldModel;
use factcheck::KilledTokens;

/// A compiled, evaluable world: the loader's status-gated [`WorldModel`] with its
/// ledger valuation set and its fixpoint computed, plus the negative-constraint
/// store.
///
/// The distinction from a raw [`WorldModel`] is an invariant, not a convention. A
/// [`WorldModel`] straight out of [`Loader::load`](crate::loader::Loader::load)
/// has an *empty* ambient valuation — every ledger-marked claim reads
/// [`Possible`](dli_core::ann::Status::Possible) because no decision variable is
/// ruled. A `CompiledWorld` can only be built by [`compile`](crate::compile),
/// which rules every cited decision variable true (the ledger valuation) and runs
/// the symbolic fixpoint, so its certain projection is real. The check batteries
/// take a `CompiledWorld` by type so they cannot be handed an ungated world by
/// mistake.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CompiledWorld {
    /// The loader's world model: gated atoms, entities, packs, load findings, and
    /// the evaluable `world` (facts, ambient valuation, saturated relation).
    pub model: WorldModel,
    /// The negative-constraint store: killed tokens whose reappearance in a claim
    /// realizes rejected lore. Empty out of [`compile`](crate::compile) until a
    /// pack declares guardrails; attach a registry with
    /// [`with_killed_tokens`](CompiledWorld::with_killed_tokens).
    pub killed: KilledTokens,
}

impl CompiledWorld {
    /// Wrap a fully-compiled [`WorldModel`] (valuation set, fixpoint run) with an
    /// empty guardrail store. Only [`compile`](crate::compile) should call this;
    /// it is the constructor that stamps the "compiled" invariant.
    #[must_use]
    pub fn new(model: WorldModel) -> Self {
        Self {
            model,
            killed: KilledTokens::default(),
        }
    }

    /// Attach a killed-token guardrail store (the C-store), builder-style.
    ///
    /// The kernel is offline and reads no files, so the guardrail registry is
    /// supplied as data rather than loaded from disk: a consuming repo compiles
    /// its rejected ledger into a [`KilledTokens`] and attaches it here.
    #[must_use]
    pub fn with_killed_tokens(mut self, killed: KilledTokens) -> Self {
        self.killed = killed;
        self
    }

    /// The compiled world model.
    #[must_use]
    pub fn model(&self) -> &WorldModel {
        &self.model
    }
}

/// How much a [`WorldFinding`] must be attended to.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A violation: the world (or a draft) contradicts canon or asserts what it
    /// may not.
    Error,
    /// A defect worth a look that does not, on its own, break canon.
    Warn,
}

/// One world-check finding: a dotted class code, a severity, a verification
/// grade, a domain-spoken message, its ledger citations, and an optional source
/// span.
///
/// The [`code`](WorldFinding::code) is `family.item` — `continuity.unknown-entity`,
/// `factcheck.contradicted` — with the family naming the check battery and the
/// item porting the legacy violation class where one exists. Codes are stable:
/// clients and the wire layer match on them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorldFinding {
    /// The stable dotted class code (`family.item`).
    pub code: String,
    /// How much the finding must be attended to.
    pub severity: Severity,
    /// The verification grade of the check that produced it. Every deterministic
    /// kernel check re-derives exactly, so findings carry
    /// [`Grade::Checked`].
    pub grade: Grade,
    /// A domain-spoken description (charter koan 9: errors are lore).
    pub message: String,
    /// Ledger citations backing the finding (`d-nnnn`), when it turns on a ruled
    /// fact.
    pub citations: Vec<String>,
    /// The source location of the offending claim, when known.
    pub span: Option<Span>,
    /// A structured payload a consumer can reconstruct the verdict from,
    /// without parsing [`message`](WorldFinding::message) — the WS-2 rider.
    /// `None` for every finding this crate has always raised (the message is
    /// the only report); populated only by the specific classes documented
    /// to carry one (`continuity.fd-violation` and its derived sibling,
    /// `continuity.fluent-contradiction`, `continuity.fluent-order-
    /// undeclared`). The wire layer's `Finding.data` mirrors this field and
    /// gates it on the negotiated contract version — this field itself is
    /// unconditional; version-gating is a wire concern, not a kernel one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl WorldFinding {
    /// A deterministic [`Grade::Checked`] finding
    /// with no span and no citations.
    #[must_use]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            grade: Grade::Checked,
            message: message.into(),
            citations: Vec::new(),
            span: None,
            data: None,
        }
    }

    /// A deterministic [`Severity::Warn`] finding with no span and no citations.
    #[must_use]
    pub fn warn(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warn,
            ..Self::error(code, message)
        }
    }

    /// Attach ledger citations, builder-style.
    #[must_use]
    pub fn with_citations(mut self, citations: Vec<String>) -> Self {
        self.citations = citations;
        self
    }

    /// Attach a source span, builder-style.
    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Attach a structured payload, builder-style (WS-2's `Finding.data` rider).
    #[must_use]
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Override the verification grade, builder-style. Deterministic checks stay
    /// [`Grade::Checked`]; a domain battery whose verdict rests on stored model
    /// judgment (the language layer's `register-fit`, say) lowers it to
    /// [`Grade::Attested`].
    #[must_use]
    pub fn with_grade(mut self, grade: Grade) -> Self {
        self.grade = grade;
        self
    }

    /// The finding's class family — the part of [`code`](WorldFinding::code)
    /// before the first dot (`continuity`, `factcheck`).
    #[must_use]
    pub fn family(&self) -> &str {
        self.code.split_once('.').map_or(&self.code, |(fam, _)| fam)
    }
}

/// Run the world-internal check battery over a compiled world.
///
/// This is the self-consistency report: it needs no external draft, only the
/// compiled world itself. It surfaces the loader's fail-closed findings
/// (`load.missing-provenance`, `load.malformed-provenance`,
/// `load.duplicate-declaration`, …) — fail-closed provenance *is* the
/// assertability gate — and `continuity.rejected-lore` for every certain fact
/// that realizes a killed token. The claim-relative classes (`unknown-entity`,
/// `proposed-as-fact`, `intra-episode-contradiction`, and the fact-checker's
/// `contradicted` verdict) run over a draft's claims in
/// [`check_draft`](crate::check_draft), not here. Findings are returned sorted by
/// `(code, message)` for a deterministic report.
///
/// The language layer's `language::check` composes here once
/// `WorldModel::language` lands (lead pass): it returns `Vec<WorldFinding>` too,
/// so the aggregator simply extends this list.
///
/// ```
/// use dli_world::compile;
/// use dli_world::pack::Pack;
/// use dli_world::loader::example_docs;
/// // `example_docs` declares the decisions it cites (`ratify`), so a clean world
/// // reports nothing (B1: a citation to an undeclared decision would fail closed).
/// let pack = Pack { name: "camelot".to_owned(), documents: example_docs() };
/// let world = compile(&pack).expect("compiles");
/// assert!(dli_world::check::run(&world).is_empty());
/// ```
#[must_use]
pub fn run(world: &CompiledWorld) -> Vec<WorldFinding> {
    continuity::run(world)
}

/// The inertia-aware point-in-time projection of one `(pred, subject)` fluent
/// slot (`author.as_of`, contract 1.9.0, DL-0082): what holds, and why, at a
/// query point — including a silent inertial transition's derived boundary,
/// which no other read exposes (`docs/design-notes/inertia-fluent-
/// persistence.md`). Never touches the eval query path or the derived
/// (saturated-rule) layer; see [`continuity::slot_state_at`] for the full
/// projection discipline.
#[must_use]
pub fn slot_state_at(
    world: &CompiledWorld,
    pred: &str,
    subject: &str,
    at: TimePoint,
) -> SlotStateAt {
    continuity::slot_state_at(world, pred, subject, at)
}
