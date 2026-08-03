//! Order rules over the episode timeline (`TIME-3`).
//!
//! Production order is the listener's dimension (`docs/spec/03-time.md` §4–§5):
//! the sequence in which a listener meets the material. Some constraints attach
//! *there* rather than to story-time — the full-team floor, the
//! no-deep-time-in-the-opening-act rule, payoff-after-reveal. Those constraints
//! are **data**, declared as `rule … over order` and read by a **linear scan**
//! over the season in production order; they are never code.
//!
//! This module is the generic engine: it takes a slice of [`SlotView`] (one per
//! episode, in production order — the listener-ledger the episode fold
//! accumulated, projected to exactly the fields a rule reads) and a slice of
//! [`OrderRule`], and returns [`OrderFinding`]s. The adapter that turns a real
//! episode fold into [`SlotView`]s lives in `dli-world`
//! (`episode_order.rs`) — this crate never depends on the episode layer.
//!
//! # The `over order` rule vocabulary
//!
//! An order rule is `guard requires requirement`, plus one relational shorthand.
//! The vocabulary this engine owns (`docs/spec/03-time.md` §5):
//!
//! ```text
//! # a guarded slot/act bound — the guard selects slots, the requirement binds them
//! rule full-team-floor      over order : listener-has(configuration full-team) requires slot >= 10
//! rule no-deep-time-act-one over order : listener-learns(during elder-cycles _) requires act > 1
//!
//! # the relational shorthand — every payoff must have an earlier reveal of its tag
//! rule payoff-after-reveal  over order : payoff($p) at slot $j requires reveal($p) at slot $i, $i < $j
//! ```
//!
//! - **Guards** ([`SlotGuard`]): `listener-has(configuration <label>)` matches a
//!   slot whose accumulated listener configuration carries `<label>`;
//!   `listener-learns(during <era> _)` matches a slot at which the listener
//!   learns a fact anchored in `<era>`.
//! - **Requirements** ([`SlotRequirement`]): `slot >= N` / `slot <= N` bound the
//!   matched slot's production position; `act > N` / `act >= N` bound its
//!   narrative act. A rule asking about `act` on a slot with no known act is
//!   **indeterminate**, reported as its own finding (a ruling-request), never a
//!   silent pass.
//! - **Payoff-after-reveal** ([`OrderRuleBody::PayoffAfterReveal`]): the one
//!   relational form. Every payoff tag must be revealed at a slot at or before
//!   the payoff (setup and payoff in the same episode is legal; the spec's
//!   strict `$i < $j` is relaxed to `<=` deliberately — see the variant docs).
//!
//! # Standing-aware precedence
//!
//! Declaring an order is a ruling (`docs/spec/03-time.md` §3), so a `before`-fact
//! carries a standing like any fact. [`precedes_with_standing`] threads that
//! standing through precedence: a *merely proposed* `before`-fact yields
//! [`StandingOrder::PossiblyBefore`], never a hard [`StandingOrder::CertainlyBefore`].
//! The plain [`Chronology::precedes`] is standing-blind by design (it is the raw
//! graph reachability); this wrapper is the one to use when a checker must not
//! let an unratified edge harden a schedule.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::clause::Clause;
use dli_ast::decl::{RuleDecl, RuleDomain};
use dli_ast::span::Span;
use dli_core::ann::{Ann, Status, Theory};
use dli_core::valuation::Valuation;

use crate::chronology::{Chronology, TemporalVerdict, TimePoint};

/// One episode's production-order projection: exactly the listener-ledger fields
/// an [`OrderRule`] reads, accumulated by the fold and handed to [`check`].
///
/// The engine is deliberately ignorant of *how* these sets are computed — the
/// adapter derives them from the episode fold (which cast has assembled, which
/// eras the listener has learned about, which promises open and close). Keeping
/// the semantics in the adapter is what lets this crate stay free of the episode
/// layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotView {
    /// The episode's identifier.
    pub episode: String,
    /// The episode's production slot — where the listener meets it.
    pub slot: u32,
    /// The episode's narrative act, if the adapter knows it. `None` makes any
    /// `act`-bound rule indeterminate over this slot rather than passing it.
    pub act: Option<u32>,
    /// The named listener configurations in force at this slot (e.g. `full-team`).
    pub configurations: BTreeSet<String>,
    /// The eras the listener learns a fact *about* at this slot (each fact's
    /// story anchor's era).
    pub learns_eras: BTreeSet<String>,
    /// The reveal tags introduced at this slot (a promise opened / a setup laid).
    pub reveals: BTreeSet<String>,
    /// The payoff tags discharged at this slot (a promise closed / a setup paid).
    pub payoffs: BTreeSet<String>,
}

impl SlotView {
    /// A bare slot view: an episode at a production slot, everything else empty.
    #[must_use]
    pub fn new(episode: impl Into<String>, slot: u32) -> Self {
        Self {
            episode: episode.into(),
            slot,
            ..Self::default()
        }
    }

    /// Set the narrative act, builder-style.
    #[must_use]
    pub fn with_act(mut self, act: u32) -> Self {
        self.act = Some(act);
        self
    }

    /// Add a listener configuration in force at this slot, builder-style.
    #[must_use]
    pub fn with_configuration(mut self, label: impl Into<String>) -> Self {
        self.configurations.insert(label.into());
        self
    }

    /// Add an era the listener learns about at this slot, builder-style.
    #[must_use]
    pub fn learns_during(mut self, era: impl Into<String>) -> Self {
        self.learns_eras.insert(era.into());
        self
    }

    /// Add a reveal tag introduced at this slot, builder-style.
    #[must_use]
    pub fn with_reveal(mut self, tag: impl Into<String>) -> Self {
        self.reveals.insert(tag.into());
        self
    }

    /// Add a payoff tag discharged at this slot, builder-style.
    #[must_use]
    pub fn with_payoff(mut self, tag: impl Into<String>) -> Self {
        self.payoffs.insert(tag.into());
        self
    }
}

/// The antecedent of a guarded order rule: which slots it applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SlotGuard {
    /// `listener-has(configuration <label>)` — slots whose listener
    /// configuration carries `<label>`.
    ListenerHasConfiguration(String),
    /// `listener-learns(during <era> _)` — slots at which the listener learns a
    /// fact anchored in `<era>`.
    ListenerLearnsDuringEra(String),
}

impl SlotGuard {
    /// Whether this guard selects `slot`.
    #[must_use]
    pub fn matches(&self, slot: &SlotView) -> bool {
        match self {
            Self::ListenerHasConfiguration(label) => slot.configurations.contains(label),
            Self::ListenerLearnsDuringEra(era) => slot.learns_eras.contains(era),
        }
    }
}

/// The consequent of a guarded order rule: the bound a matched slot must meet.
///
/// Strict surface operators normalize into these inclusive bounds at compile
/// time: `slot > N` becomes `SlotAtLeast(N + 1)`, `act > N` becomes
/// `ActAtLeast(N + 1)`, `slot < N` becomes `SlotAtMost(N - 1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SlotRequirement {
    /// The matched slot's production position must be at least `N`.
    SlotAtLeast(u32),
    /// The matched slot's production position must be at most `N`.
    SlotAtMost(u32),
    /// The matched slot's narrative act must be at least `N`.
    ActAtLeast(u32),
}

/// The body of an [`OrderRule`]: a guarded bound or the relational payoff form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum OrderRuleBody {
    /// `guard requires requirement`.
    Guarded {
        /// Which slots the rule constrains.
        guard: SlotGuard,
        /// The bound each matched slot must meet.
        requirement: SlotRequirement,
    },
    /// `payoff($p) at slot $j requires reveal($p) at slot $i, $i < $j`.
    ///
    /// Every payoff tag must be revealed at a slot **at or before** the payoff.
    /// A payoff whose only reveal comes later — or never — is a finding. The
    /// spec sketch writes the strict `$i < $j`; this engine relaxes it to `<=`
    /// so a promise set up and paid off inside one episode is legal (which is a
    /// real craft move, not an anachronism).
    PayoffAfterReveal,
}

/// A compiled `rule … over order`: a name and a body. The engine treats every
/// rule handed to [`check`] as active; filtering by the rule's own ratification
/// standing (a proposed rule that should not yet fire) is the caller's job, done
/// before the rule reaches here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OrderRule {
    /// The rule's name (from `rule <name> over order …`).
    pub name: String,
    /// What the rule requires.
    pub body: OrderRuleBody,
}

impl OrderRule {
    /// A guarded rule, built directly.
    #[must_use]
    pub fn guarded(
        name: impl Into<String>,
        guard: SlotGuard,
        requirement: SlotRequirement,
    ) -> Self {
        Self {
            name: name.into(),
            body: OrderRuleBody::Guarded { guard, requirement },
        }
    }

    /// The payoff-after-reveal rule, built directly.
    #[must_use]
    pub fn payoff_after_reveal(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            body: OrderRuleBody::PayoffAfterReveal,
        }
    }

    /// Compile a parsed `rule … over order` declaration into an [`OrderRule`].
    ///
    /// The declaration must carry [`RuleDomain::Order`] and exactly one body
    /// clause (the `: guard requires requirement` line, whose leading `:` is
    /// stripped). The clause text is retained on the typed rule body (LS-9); the
    /// order-rule micro-grammar — `payoff($p)`/`reveal($p)` shorthand, the guard
    /// interiors (`configuration <label>`, `during <era> _`), and the numeric
    /// requirement — is this module's own domain parser, no longer `dli_syntax`.
    ///
    /// ```
    /// use dli_syntax::parse_document;
    /// use dli_ast::decl::Decl;
    /// use dli_time::order_rules::{OrderRule, OrderRuleBody, SlotGuard, SlotRequirement};
    ///
    /// let (doc, _diags) = parse_document(
    ///     "rule full-team-floor over order ratified d-0162\n\
    ///      \x20 : listener-has(configuration full-team) requires slot >= 10\n",
    /// );
    /// let Decl::Rule(decl) = &doc.decls[0] else { panic!("expected a rule") };
    /// let rule = OrderRule::compile(decl).expect("compiles");
    /// assert_eq!(rule.name, "full-team-floor");
    /// assert_eq!(
    ///     rule.body,
    ///     OrderRuleBody::Guarded {
    ///         guard: SlotGuard::ListenerHasConfiguration("full-team".to_owned()),
    ///         requirement: SlotRequirement::SlotAtLeast(10),
    ///     }
    /// );
    /// ```
    pub fn compile(decl: &RuleDecl) -> Result<Self, OrderRuleError> {
        if decl.domain != Some(RuleDomain::Order) {
            return Err(OrderRuleError::new(
                format!("rule '{}' is not `over order`", decl.name.text),
                decl.name.span,
            ));
        }
        let clause = decl
            .body
            .clauses
            .first()
            .map(|rc| &rc.clause)
            .ok_or_else(|| {
                OrderRuleError::new(
                    format!("order rule '{}' has no body clause", decl.name.text),
                    decl.name.span,
                )
            })?;
        let body = parse_body(clause)?;
        Ok(Self {
            name: decl.name.text.clone(),
            body,
        })
    }
}

/// Why an `over order` declaration failed to compile into an [`OrderRule`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OrderRuleError {
    /// A domain-spoken explanation of the malformed rule.
    pub message: String,
    /// Where in source the offending declaration sits.
    pub span: Span,
}

impl OrderRuleError {
    fn new(message: String, span: Span) -> Self {
        Self { message, span }
    }
}

/// What an [`OrderFinding`] reports — the shape of the order violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OrderFindingKind {
    /// The matched slot sits earlier than a `slot >= N` bound allows.
    SlotTooEarly,
    /// The matched slot sits later than a `slot <= N` bound allows.
    SlotTooLate,
    /// The matched slot's act is earlier than an `act > N` bound allows.
    ActTooEarly,
    /// The rule bounds an act, but this slot's act is unknown: the answer turns
    /// on an act no one has assigned — declare it or accept (a ruling-request).
    ActIndeterminate,
    /// A payoff tag has no reveal at or before the payoff's slot.
    MissingPriorReveal,
}

/// One order-rule violation, named where it occurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OrderFinding {
    /// The rule that flagged the violation.
    pub rule: String,
    /// The offending episode.
    pub episode: String,
    /// The offending episode's production slot.
    pub slot: u32,
    /// The shape of the violation.
    pub kind: OrderFindingKind,
    /// A domain-spoken description a reader can act on.
    pub message: String,
}

/// Check a season's slots against its order rules, by linear scan.
///
/// Each guarded rule is one pass over the slots (match the guard, test the
/// bound); the payoff-after-reveal rule is two passes (gather the earliest
/// reveal of each tag, then check every payoff against it). The findings come
/// out in rule order, and within a rule in slot order.
///
/// ```
/// use dli_time::order_rules::{
///     check, OrderFindingKind, OrderRule, SlotGuard, SlotRequirement, SlotView,
/// };
///
/// // The full team assembles at slot 6, but the floor rule wants slot >= 10.
/// let slots = vec![
///     SlotView::new("e5", 5),
///     SlotView::new("e6", 6).with_configuration("full-team"),
/// ];
/// let rules = vec![OrderRule::guarded(
///     "full-team-floor",
///     SlotGuard::ListenerHasConfiguration("full-team".to_owned()),
///     SlotRequirement::SlotAtLeast(10),
/// )];
///
/// let findings = check(&slots, &rules);
/// assert_eq!(findings.len(), 1);
/// assert_eq!(findings[0].episode, "e6");
/// assert_eq!(findings[0].kind, OrderFindingKind::SlotTooEarly);
/// ```
#[must_use]
pub fn check(slots: &[SlotView], rules: &[OrderRule]) -> Vec<OrderFinding> {
    let mut findings = Vec::new();
    for rule in rules {
        match &rule.body {
            OrderRuleBody::Guarded { guard, requirement } => {
                check_guarded(&rule.name, guard, *requirement, slots, &mut findings);
            }
            OrderRuleBody::PayoffAfterReveal => {
                check_payoff_after_reveal(&rule.name, slots, &mut findings);
            }
        }
    }
    findings
}

/// One linear scan for a guarded rule: for each slot the guard selects, test the
/// bound and record a finding when it fails.
fn check_guarded(
    rule: &str,
    guard: &SlotGuard,
    requirement: SlotRequirement,
    slots: &[SlotView],
    findings: &mut Vec<OrderFinding>,
) {
    for slot in slots {
        if !guard.matches(slot) {
            continue;
        }
        if let Some((kind, message)) = evaluate(rule, guard, requirement, slot) {
            findings.push(OrderFinding {
                rule: rule.to_owned(),
                episode: slot.episode.clone(),
                slot: slot.slot,
                kind,
                message,
            });
        }
    }
}

/// Test a matched slot against a requirement, returning the violation if any.
fn evaluate(
    rule: &str,
    guard: &SlotGuard,
    requirement: SlotRequirement,
    slot: &SlotView,
) -> Option<(OrderFindingKind, String)> {
    let subject = describe_guard(guard);
    match requirement {
        SlotRequirement::SlotAtLeast(floor) if slot.slot < floor => Some((
            OrderFindingKind::SlotTooEarly,
            format!(
                "episode '{}' {subject} at slot {}, but rule '{rule}' requires slot >= {floor}",
                slot.episode, slot.slot
            ),
        )),
        SlotRequirement::SlotAtMost(ceiling) if slot.slot > ceiling => Some((
            OrderFindingKind::SlotTooLate,
            format!(
                "episode '{}' {subject} at slot {}, but rule '{rule}' requires slot <= {ceiling}",
                slot.episode, slot.slot
            ),
        )),
        SlotRequirement::ActAtLeast(floor) => match slot.act {
            Some(act) if act < floor => Some((
                OrderFindingKind::ActTooEarly,
                format!(
                    "episode '{}' {subject} in act {act}, but rule '{rule}' requires act >= {floor}",
                    slot.episode
                ),
            )),
            None => Some((
                OrderFindingKind::ActIndeterminate,
                format!(
                    "episode '{}' {subject}, but rule '{rule}' bounds the act and this episode's \
                     act is undeclared — assign one or accept",
                    slot.episode
                ),
            )),
            Some(_) => None,
        },
        SlotRequirement::SlotAtLeast(_) | SlotRequirement::SlotAtMost(_) => None,
    }
}

/// A human phrase for what a guard says about a slot, used in finding messages.
fn describe_guard(guard: &SlotGuard) -> String {
    match guard {
        SlotGuard::ListenerHasConfiguration(label) => {
            format!("has the '{label}' configuration")
        }
        SlotGuard::ListenerLearnsDuringEra(era) => {
            format!("has the listener learn about era '{era}'")
        }
    }
}

/// Two linear scans for payoff-after-reveal: gather the earliest reveal slot per
/// tag, then flag any payoff whose tag was revealed later, or never.
fn check_payoff_after_reveal(rule: &str, slots: &[SlotView], findings: &mut Vec<OrderFinding>) {
    let mut earliest_reveal: BTreeMap<&str, u32> = BTreeMap::new();
    for slot in slots {
        for tag in &slot.reveals {
            earliest_reveal
                .entry(tag.as_str())
                .and_modify(|at| *at = (*at).min(slot.slot))
                .or_insert(slot.slot);
        }
    }
    for slot in slots {
        for tag in &slot.payoffs {
            let revealed_at_or_before = earliest_reveal
                .get(tag.as_str())
                .is_some_and(|&reveal| reveal <= slot.slot);
            if !revealed_at_or_before {
                findings.push(OrderFinding {
                    rule: rule.to_owned(),
                    episode: slot.episode.clone(),
                    slot: slot.slot,
                    kind: OrderFindingKind::MissingPriorReveal,
                    message: format!(
                        "episode '{}' pays off '{tag}' at slot {}, but nothing reveals '{tag}' at \
                         or before that slot",
                        slot.episode, slot.slot
                    ),
                });
            }
        }
    }
}

// --- the rule-body parser ----------------------------------------------------

/// Parse a rule's body clause (`: guard requires requirement`) into a body.
fn parse_body(clause: &Clause) -> Result<OrderRuleBody, OrderRuleError> {
    let text = strip_clause_head(&clause.text);
    let (guard_text, requirement_text) = text.split_once(" requires ").ok_or_else(|| {
        OrderRuleError::new(
            format!("order rule body '{text}' has no `requires`"),
            clause.span,
        )
    })?;
    let guard_text = guard_text.trim();
    let requirement_text = requirement_text.trim();

    // The relational shorthand: a `payoff(...)` guard against a `reveal(...)`
    // requirement, both read as atoms through the shared parser.
    let payoff = leading_atom_pred(guard_text);
    let reveal = leading_atom_pred(requirement_text);
    if payoff.as_deref() == Some("payoff") && reveal.as_deref() == Some("reveal") {
        return Ok(OrderRuleBody::PayoffAfterReveal);
    }

    let guard = parse_guard(guard_text, clause.span)?;
    let requirement = parse_requirement(requirement_text, clause.span)?;
    Ok(OrderRuleBody::Guarded { guard, requirement })
}

/// Strip a leading `:` or `:-` clause marker and surrounding whitespace.
fn strip_clause_head(text: &str) -> &str {
    let text = text.trim();
    let text = text
        .strip_prefix(":-")
        .or_else(|| text.strip_prefix(':'))
        .unwrap_or(text);
    text.trim()
}

/// The predicate name of the leading `pred(args…)` atom in `text`; `None` when
/// `text` does not open with a `pred(…)` group. This is the order-rule
/// micro-grammar's own reader (LS-9): it extracts the predicate token that
/// precedes the balanced argument group [`leading_atom_text`] already isolated,
/// so the crate no longer re-enters `dli_syntax` to name an atom it will parse
/// itself.
fn leading_atom_pred(text: &str) -> Option<String> {
    let atom_text = leading_atom_text(text)?;
    let pred = atom_text.split('(').next()?.trim();
    (!pred.is_empty()).then(|| pred.to_owned())
}

/// The substring of `text` from its start through the first balanced `(…)`
/// group, e.g. `payoff($p)` out of `payoff($p) at slot $j`.
fn leading_atom_text(text: &str) -> Option<&str> {
    let open = text.find('(')?;
    let mut depth = 0usize;
    for (offset, ch) in text[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[..=open + offset]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse a guard: `listener-has(configuration <label>)` or
/// `listener-learns(during <era> _)`.
fn parse_guard(text: &str, span: Span) -> Result<SlotGuard, OrderRuleError> {
    let (pred, interior) = split_call(text).ok_or_else(|| {
        OrderRuleError::new(format!("guard '{text}' is not a `pred(…)` form"), span)
    })?;
    let words: Vec<&str> = interior.split_whitespace().collect();
    match pred {
        "listener-has" => match words.as_slice() {
            ["configuration", label] => {
                Ok(SlotGuard::ListenerHasConfiguration((*label).to_owned()))
            }
            _ => Err(OrderRuleError::new(
                format!("listener-has guard expects `configuration <label>`, got '{interior}'"),
                span,
            )),
        },
        "listener-learns" => match words.as_slice() {
            ["during", era] | ["during", era, "_"] => {
                Ok(SlotGuard::ListenerLearnsDuringEra((*era).to_owned()))
            }
            _ => Err(OrderRuleError::new(
                format!("listener-learns guard expects `during <era> _`, got '{interior}'"),
                span,
            )),
        },
        other => Err(OrderRuleError::new(
            format!("unknown order-rule guard '{other}'"),
            span,
        )),
    }
}

/// Split `pred(interior)` into `(pred, interior)`, trimming both.
fn split_call(text: &str) -> Option<(&str, &str)> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }
    Some((text[..open].trim(), text[open + 1..close].trim()))
}

/// Parse a requirement: `slot >= N`, `slot <= N`, `slot > N`, `slot < N`,
/// `act > N`, or `act >= N`, normalizing strict operators to inclusive bounds.
fn parse_requirement(text: &str, span: Span) -> Result<SlotRequirement, OrderRuleError> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let [key, op, value] = words.as_slice() else {
        return Err(OrderRuleError::new(
            format!("requirement '{text}' is not `<key> <op> <number>`"),
            span,
        ));
    };
    let value: u32 = value.parse().map_err(|_| {
        OrderRuleError::new(format!("requirement bound '{value}' is not a number"), span)
    })?;
    match (*key, *op) {
        ("slot", ">=") => Ok(SlotRequirement::SlotAtLeast(value)),
        ("slot", ">") => Ok(SlotRequirement::SlotAtLeast(value.saturating_add(1))),
        ("slot", "<=") => Ok(SlotRequirement::SlotAtMost(value)),
        ("slot", "<") => Ok(SlotRequirement::SlotAtMost(value.saturating_sub(1))),
        ("act", ">=") => Ok(SlotRequirement::ActAtLeast(value)),
        ("act", ">") => Ok(SlotRequirement::ActAtLeast(value.saturating_add(1))),
        _ => Err(OrderRuleError::new(
            format!("unsupported requirement '{text}'"),
            span,
        )),
    }
}

// --- standing-aware precedence (carry-item R2-G) -----------------------------

/// The standing-aware answer to *does one event precede another?* — precedence
/// read through the ratification standing of the `before`-facts on the path.
///
/// The plain [`Chronology::precedes`] is standing-blind: it answers `Before` for
/// a merely-proposed `before`-fact exactly as for a ratified one, because it is
/// raw graph reachability. That is wrong for any checker that must not let an
/// unratified edge harden a conclusion. This verdict keeps the two apart:
/// `Certainly*` only when every edge on the connecting path is certain,
/// `Possibly*` when a path exists but rests on at least one merely-possible edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StandingOrder {
    /// A path of only certain `before`-facts orders `earlier` before `later`.
    CertainlyBefore,
    /// A path of only certain `before`-facts orders `later` before `earlier`.
    CertainlyAfter,
    /// A path orders `earlier` before `later`, but rests on a proposed edge: it
    /// holds only if that ruling lands. Never hardens to [`Self::CertainlyBefore`].
    PossiblyBefore,
    /// A path orders `later` before `earlier`, but rests on a proposed edge.
    PossiblyAfter,
    /// No `before`-fact path relates the two, under any standing.
    Unordered,
}

/// Whether `earlier` precedes `later`, read through the standing of the
/// `before`-facts that would order them.
///
/// `anns[i]` is the ruling that declared `chronology.edges[i]` — the parallel
/// table the loader keeps (the same convention as
/// [`Chronology::find_cycle`](crate::chronology::Chronology::find_cycle)). An
/// edge with no entry in `anns` is treated as certain (a declared, unconditional
/// order); a `Dead` edge is dropped from both readings. Precedence is computed
/// twice — once over the certain edges alone, once over every live edge — and
/// the strongest true reading wins.
///
/// ```
/// use dli_core::ann::{Ann, Theory};
/// use dli_core::atom::Atom;
/// use dli_core::posbool::PosBool;
/// use dli_core::valuation::Valuation;
/// use dli_time::chronology::{Chronology, TimePoint};
/// use dli_time::order_rules::{precedes_with_standing, StandingOrder};
///
/// // breaking(0) -> tithe(1): a *proposed* before-fact (support on an open event).
/// let chrono = Chronology { edges: vec![(TimePoint(0), TimePoint(1))] };
/// let proposed = Ann::supported(PosBool::pos(Atom(7))); // event 7 still open
/// let nu = Valuation::new();
/// let theory = Theory::new();
///
/// // A proposed edge answers *possibly* before — never hard Before.
/// assert_eq!(
///     precedes_with_standing(&chrono, &[proposed], &nu, &theory, TimePoint(0), TimePoint(1)),
///     StandingOrder::PossiblyBefore,
/// );
///
/// // A ratified (unconditional) edge answers certainly before.
/// let ratified = Ann::unconditional();
/// assert_eq!(
///     precedes_with_standing(&chrono, &[ratified], &nu, &theory, TimePoint(0), TimePoint(1)),
///     StandingOrder::CertainlyBefore,
/// );
/// ```
#[must_use]
pub fn precedes_with_standing(
    chronology: &Chronology,
    anns: &[Ann],
    valuation: &Valuation,
    theory: &Theory,
    earlier: TimePoint,
    later: TimePoint,
) -> StandingOrder {
    let certain = filter_edges(chronology, anns, valuation, theory, |status| {
        status == Status::Certain
    });
    match certain.precedes(earlier, later) {
        TemporalVerdict::Before => return StandingOrder::CertainlyBefore,
        TemporalVerdict::After => return StandingOrder::CertainlyAfter,
        TemporalVerdict::Unordered => {}
    }
    let live = filter_edges(chronology, anns, valuation, theory, |status| {
        status != Status::Dead
    });
    match live.precedes(earlier, later) {
        TemporalVerdict::Before => StandingOrder::PossiblyBefore,
        TemporalVerdict::After => StandingOrder::PossiblyAfter,
        TemporalVerdict::Unordered => StandingOrder::Unordered,
    }
}

/// A sub-chronology of the edges whose declaring ruling's standing satisfies
/// `keep`. An edge past the end of `anns` is treated as certain.
fn filter_edges(
    chronology: &Chronology,
    anns: &[Ann],
    valuation: &Valuation,
    theory: &Theory,
    keep: impl Fn(Status) -> bool,
) -> Chronology {
    let edges = chronology
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            let status = anns
                .get(*index)
                .map_or(Status::Certain, |ann| ann.status(valuation, theory));
            keep(status)
        })
        .map(|(_, &edge)| edge)
        .collect();
    Chronology { edges }
}
