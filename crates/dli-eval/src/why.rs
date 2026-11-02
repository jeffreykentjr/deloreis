//! Blame and explanation reads over the evaluated world (`ENG-1`).
//!
//! `--why` renders the support antichain; blame reads the defeater. The query
//! lane owns this module. Two reads live here:
//!
//! - [`Why`] — the positive explanation: a certain-or-possible atom's support
//!   antichain, each minimal witness rendered to claim names, with the smallest
//!   witness available as [`Why::min_why`].
//! - [`why_not`] — the negative explanation: for an atom that is *not* certain,
//!   the support monomial that came closest to certainty, the unruled variables
//!   still blocking it, and the defeater (blame) if the atom rests on a
//!   defeasible default.
//!
//! Both are pure reads over the symbolic saturation (`08-reasoning.md` §2, the
//! state tree is never walked): they inspect the support and defeater
//! polynomials the fixpoint already computed and never re-derive anything.

use dli_ast::term::{AtomExpr, Term, TimeRef};
use dli_core::ann::{Ann, Status, Theory};
use dli_core::atom::{Atom, Interner, Rail};
use dli_core::derivation::ClaimId;
use dli_core::posbool::{Monomial, PosBool};
use dli_core::valuation::Valuation;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::world::{GroundAtom, World};

/// The positive explanation of an atom: its support antichain rendered for a
/// reader.
///
/// The [`antichain`](Why::antichain) is the disjunction of minimal witnesses —
/// the same shape as [`crate::SupportIndex::why`], one inner vector of
/// [`ClaimId`]s per minimal monomial of the support (sign is dropped, matching
/// that seam). [`rendered`](Why::rendered) is the parallel human line for each
/// witness, resolved to claim names through the world's interner; a negative
/// literal renders with a `not ` prefix, and the unconditional witness (the
/// empty monomial) renders as `unconditional`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Why {
    /// The disjunction of minimal witnesses, each a conjunction of claim ids.
    pub antichain: Vec<Vec<ClaimId>>,
    /// The per-witness human rendering, index-aligned with `antichain`.
    pub rendered: Vec<String>,
}

impl Why {
    /// Build the explanation from a support polynomial and the interner that
    /// names its variables.
    ///
    /// Each monomial of the support becomes one witness. The witness order is
    /// the support's own canonical (absorbed, byte-stable) monomial order, so
    /// two logically-equal supports explain identically.
    ///
    /// ```
    /// use dli_core::atom::Interner;
    /// use dli_core::derivation::ClaimId;
    /// use dli_core::posbool::PosBool;
    /// use dli_eval::why::Why;
    ///
    /// let mut interner = Interner::new();
    /// let d = interner.intern("d-0007");
    /// let why = Why::of_support(&PosBool::pos(d), &interner);
    /// assert_eq!(why.antichain, vec![vec![ClaimId(0)]]);
    /// assert_eq!(why.rendered, vec!["d-0007".to_owned()]);
    /// ```
    #[must_use]
    pub fn of_support(support: &PosBool, interner: &Interner) -> Why {
        let mut antichain = Vec::new();
        let mut rendered = Vec::new();
        for monomial in support.monomials() {
            antichain.push(claim_ids(monomial));
            rendered.push(render_monomial(monomial, interner));
        }
        Why {
            antichain,
            rendered,
        }
    }

    /// The minimal witness: the smallest conjunction that carries the atom, ties
    /// broken lexicographically on the claim ids so the choice is deterministic.
    /// `None` only when the atom has no support at all (its support is false).
    ///
    /// ```
    /// use dli_core::atom::Interner;
    /// use dli_core::derivation::ClaimId;
    /// use dli_core::posbool::PosBool;
    /// use dli_eval::why::Why;
    ///
    /// let mut interner = Interner::new();
    /// let a = interner.intern("a");
    /// let b = interner.intern("b");
    /// let c = interner.intern("c");
    /// // support = {a} ∨ {b, c}: the singleton witness is the smaller one.
    /// let support = PosBool::pos(a).or_(&PosBool::pos(b).and_(&PosBool::pos(c)));
    /// let why = Why::of_support(&support, &interner);
    /// assert_eq!(why.min_why(), Some(&vec![ClaimId(0)]));
    /// ```
    #[must_use]
    pub fn min_why(&self) -> Option<&Vec<ClaimId>> {
        self.antichain
            .iter()
            .min_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)))
    }
}

/// The negative explanation of an atom that is not certain: what stands between
/// it and assertability.
///
/// Produced by [`why_not`]. When the atom is unknown (no derivation for it),
/// [`known`](WhyNot::known) is false and the analysis fields are empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WhyNot {
    /// The atom under scrutiny, rendered `pred(a, b)`.
    pub atom: String,
    /// Whether the world derives this atom at all. False means unknown — absent
    /// from certain ∪ possible (`02-semantics.md` §4).
    pub known: bool,
    /// The atom's standing under the world's ambient valuation. `Dead` when
    /// unknown.
    pub status: Status,
    /// The live support monomial closest to certainty — the fewest unruled
    /// variables — as claim ids. Empty when the atom is unknown or every
    /// monomial is dead.
    pub closest: Vec<ClaimId>,
    /// The unruled (open) variables of that closest witness: the rulings that,
    /// once made, would carry the claim. Empty when nothing open blocks it (the
    /// obstacle is then a defeater, or the atom is dead).
    pub blocking: Vec<ClaimId>,
    /// The defeater claim, if the atom rests on a defeasible default a ruling
    /// has overridden or could override — blame.
    pub defeater: Option<ClaimId>,
    /// The human rendering: the blocking rulings and the defeater, resolved to
    /// claim names.
    pub rendered: Vec<String>,
}

/// Explain why an atom is not certain: the closest support monomial, the
/// unruled variables blocking it, and the defeater (blame) if one applies.
///
/// The analysis is a pure read over the atom's precomputed support and defeater
/// polynomials under the world's ambient valuation and theory — no
/// re-derivation. An atom the world does not derive returns a `known: false`
/// [`WhyNot`]; a certain atom returns one whose `blocking` is empty (there is
/// nothing to rule), with any live defeater surfaced as an at-risk note.
///
/// ```
/// use dli_ast::ident::Ident;
/// use dli_ast::term::{AtomExpr, Term};
/// use dli_core::ann::{Ann, Status};
/// use dli_core::atom::Atom;
/// use dli_core::posbool::PosBool;
/// use dli_eval::why::why_not;
/// use dli_eval::world::{GroundAtom, World};
///
/// let mut world = World::new();
/// // A fact resting on a single open event variable: possible, not certain.
/// let fact = GroundAtom { pred: "keeps".to_owned(), args: vec!["nara".to_owned()] };
/// world.assert_fact(fact, Ann::supported(PosBool::pos(Atom(0))));
/// let world = world.fixpoint().expect("stratifiable");
///
/// let goal = AtomExpr {
///     pred: Ident::synthetic("keeps"),
///     args: vec![Term::Sym(Ident::synthetic("nara"))],
/// };
/// let wn = why_not(&world, &goal);
/// assert_eq!(wn.status, Status::Possible);
/// // Exactly one open variable stands between the atom and certainty.
/// assert_eq!(wn.blocking.len(), 1);
/// ```
#[must_use]
pub fn why_not(world: &World, atom: &AtomExpr) -> WhyNot {
    let Some(ground) = GroundAtom::from_pattern(atom) else {
        return WhyNot::unknown(render_atom_expr(atom));
    };
    let Some(derived) = world.derived.iter().find(|d| d.atom == ground) else {
        return WhyNot::unknown(ground.render());
    };

    let valuation = &world.valuation;
    let theory = &world.theory;
    let ann = derived.ann();
    let status = ann.status(valuation, theory);

    // The live monomial nearest certainty: the fewest unruled literals, ties
    // broken lexicographically on the ruled-claim list for determinism.
    let closest_monomial = derived
        .support
        .monomials()
        .filter(|m| monomial_live(m, valuation, theory))
        .min_by(|a, b| {
            open_lit_count(a, valuation)
                .cmp(&open_lit_count(b, valuation))
                .then_with(|| claim_ids(a).cmp(&claim_ids(b)))
        });
    let (closest, blocking) = match closest_monomial {
        Some(m) => (claim_ids(m), open_claim_ids(m, valuation)),
        None => (Vec::new(), Vec::new()),
    };

    // Blame reads the *conditioned* defeater — the poison riding on the atom's
    // resolved-support (would-be-certain) monomials — never the atom-global pool.
    // The pool ORs in poison from paths that never approached certainty, so keying
    // the risk line on it warns "at risk" for an atom a clean independent path
    // makes robustly certain (defect a). A robustly-certain atom — Certain yet
    // `at_risk()` false — carries no blame to surface; a defeated Possible
    // conclusion still names the premise that defeated its would-be-certain path.
    let defeater = match status {
        Status::Certain if !ann.at_risk(valuation, theory) => None,
        Status::Certain | Status::Possible => {
            first_claim(&certain_path_defeater(&ann, valuation, theory))
        }
        Status::Dead => None,
    };

    let rendered = render_why_not(status, &blocking, defeater, &world.interner);
    WhyNot {
        atom: ground.render(),
        known: true,
        status,
        closest,
        blocking,
        defeater,
        rendered,
    }
}

impl WhyNot {
    /// The why-not of an atom the world does not derive.
    fn unknown(atom: String) -> WhyNot {
        WhyNot {
            atom,
            known: false,
            status: Status::Dead,
            closest: Vec::new(),
            blocking: Vec::new(),
            defeater: None,
            rendered: vec!["unknown — no derivation for this atom".to_owned()],
        }
    }
}

/// A monomial's literals as claim ids, sign dropped, in canonical order.
fn claim_ids(monomial: &Monomial) -> Vec<ClaimId> {
    monomial.lits().map(|lit| ClaimId(lit.atom.0)).collect()
}

/// The first literal's claim id of a polynomial's first monomial, if any — the
/// single blamed premise a why-not names.
fn first_claim(poly: &PosBool) -> Option<ClaimId> {
    poly.monomials()
        .next()
        .and_then(|m| m.lits().next())
        .map(|lit| ClaimId(lit.atom.0))
}

/// The poison riding on the atom's resolved-support monomials — the conditioned
/// defeater view (`docs/spec/02-semantics.md §4`): the OR of the defeater pinned
/// to each support monomial that holds on resolved events alone. Open monomials
/// contribute nothing, so poison from a path that never approached certainty is
/// never blamed (the atom-global pool would); `FALSE` when no resolved-support
/// monomial carries a defeater.
fn certain_path_defeater(ann: &Ann, valuation: &Valuation, theory: &Theory) -> PosBool {
    let mut acc = PosBool::FALSE;
    for monomial in ann.support.monomials() {
        if monomial.is_certain_under(valuation) && theory.respects(monomial) {
            acc = acc.or_(ann.conditions.defeater_of(monomial));
        }
    }
    acc
}

/// The open (unruled) literals' atoms of a monomial, as claim ids.
fn open_claim_ids(monomial: &Monomial, valuation: &Valuation) -> Vec<ClaimId> {
    monomial
        .lits()
        .filter(|lit| valuation.get(lit.atom).is_none())
        .map(|lit| ClaimId(lit.atom.0))
        .collect()
}

/// How many of a monomial's literals are unruled under the valuation — the
/// distance-to-certainty measure.
fn open_lit_count(monomial: &Monomial, valuation: &Valuation) -> usize {
    monomial
        .lits()
        .filter(|lit| valuation.get(lit.atom).is_none())
        .count()
}

/// Whether a monomial can still hold in some admissible world: not internally
/// contradictory, no literal resolved against it, and exclusivity-respecting.
/// This mirrors the possibility half of the status derivation over the public
/// monomial predicates.
fn monomial_live(monomial: &Monomial, valuation: &Valuation, theory: &Theory) -> bool {
    !monomial.is_contradictory() && monomial.is_live_under(valuation) && theory.respects(monomial)
}

/// Render one support monomial to a human line via the interner.
fn render_monomial(monomial: &Monomial, interner: &Interner) -> String {
    let parts: Vec<String> = monomial
        .lits()
        .map(|lit| {
            let name = resolve_claim(ClaimId(lit.atom.0), interner);
            match lit.rail {
                Rail::Pos => name,
                Rail::Neg => format!("not {name}"),
            }
        })
        .collect();
    if parts.is_empty() {
        "unconditional".to_owned()
    } else {
        parts.join(" and ")
    }
}

/// Render the human lines of a why-not from its blocking set and defeater.
fn render_why_not(
    status: Status,
    blocking: &[ClaimId],
    defeater: Option<ClaimId>,
    interner: &Interner,
) -> Vec<String> {
    let mut lines = Vec::new();
    if status == Status::Certain {
        lines.push("already certain".to_owned());
    }
    if !blocking.is_empty() {
        let names: Vec<String> = blocking
            .iter()
            .map(|claim| resolve_claim(*claim, interner))
            .collect();
        lines.push(format!("awaits ruling: {}", names.join(", ")));
    }
    if let Some(claim) = defeater {
        let verb = if status == Status::Certain {
            "at risk from"
        } else {
            "defeated by"
        };
        lines.push(format!("{verb}: {}", resolve_claim(claim, interner)));
    }
    if lines.is_empty() {
        lines.push(match status {
            Status::Dead => "dead — no admissible support".to_owned(),
            _ => "not certain".to_owned(),
        });
    }
    lines
}

/// Resolve a claim id to its interned name, or a stable `claim#n` fallback.
fn resolve_claim(claim: ClaimId, interner: &Interner) -> String {
    interner
        .resolve(Atom(claim.0))
        .map_or_else(|| format!("claim#{}", claim.0), ToOwned::to_owned)
}

/// Render an atom pattern `pred(a, b)`, variables shown by name.
fn render_atom_expr(atom: &AtomExpr) -> String {
    let args: Vec<String> = atom.args.iter().map(render_term).collect();
    format!("{}({})", atom.pred.text, args.join(", "))
}

/// Render a single term the way the evaluator grounds constants.
fn render_term(term: &Term) -> String {
    match term {
        Term::Var(id) | Term::Sym(id) => id.text.clone(),
        Term::Str(s) => s.clone(),
        Term::Int(i) => i.to_string(),
        Term::Time(TimeRef::Era(id)) => format!("era:{}", id.text),
        Term::Time(TimeRef::Event(id)) => format!("event:{}", id.text),
    }
}
