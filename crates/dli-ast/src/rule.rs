//! Typed interior of a `rule` declaration body (LS-9 expert-tier structuring).
//!
//! Before LS-9 a `rule` carried `body: Vec<Clause>` — whitespace-normalized raw
//! text that `dli-world` (world-derivation rules) and `dli-time` (order rules)
//! re-parsed at semantic-analysis time, well past the load boundary. The front
//! end now structures the body at parse time:
//!
//! - a world-rule `head :- body` datalog clause is pre-parsed into [`Datalog`],
//!   so the loader reads typed AST instead of calling a parse function;
//! - an order/season clause keeps its whitespace-normalized text — those
//!   micro-grammars are owned by their domain engines (`dli-time`,
//!   `dli-ledger`), which read the retained text without re-entering
//!   `dli_syntax`.
//!
//! Retaining the normalized clause text alongside the optional [`Datalog`]
//! keeps the canonical surface exact: the formatter renders the text verbatim,
//! so parse → format is byte-stable (charter koan 7, DL-0003). The parser is the
//! single producer of the pre-parsed structure; downstream crates are readers.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::clause::Clause;
use crate::literal::BodyLiteral;
use crate::term::{AtomExpr, Term, TimeRef};

/// The structured body of a [`RuleDecl`](crate::decl::RuleDecl).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct RuleBody {
    /// The structured block-body clauses (indented or braced), in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clauses: Vec<RuleClause>,
    /// The inline `head : body` clause carried on the rule line itself, present
    /// only when the header holds a `:`/`:-` separator and the block body is
    /// empty. It is rendered from the header tokens, never as a body line, so
    /// the surface round-trips; the loader reads its structure here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline: Option<RuleClause>,
}

impl RuleBody {
    /// Whether the body carries no block clauses and no inline clause.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clauses.is_empty() && self.inline.is_none()
    }
}

/// One structured clause of a rule body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleClause {
    /// The whitespace-normalized clause text and its span. Retained so the
    /// formatter round-trips exactly and the order/season domain engines read
    /// their own micro-grammar off it without re-entering the front end.
    pub clause: Clause,
    /// The world-rule datalog structure, present when this clause is a clean
    /// `head :- body` (or `head : body`) derivation rule. `None` for an
    /// order/season domain clause or a clause that is not clean datalog — the
    /// world loader turns a `None` in a world rule into a malformed-rule
    /// finding, exactly as it did when it re-parsed the text itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datalog: Option<Datalog>,
}

impl RuleClause {
    /// Construct a clause with no datalog structure (an order/season/other
    /// clause carried as text).
    #[must_use]
    pub fn text(clause: Clause) -> Self {
        Self {
            clause,
            datalog: None,
        }
    }
}

/// A datalog derivation rule: one head atom and its body conjunction of
/// literals. Produced by the front end from a `head :- body` clause; consumed by
/// the evaluator's typed rule intake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Datalog {
    /// The head atom derived when the body holds.
    pub head: AtomExpr,
    /// The body conjunction of literals (a negated literal is `not <atom>`).
    pub body: Vec<BodyLiteral>,
}

impl Datalog {
    /// Render this rule as parser-normalized `head :- body` text.
    ///
    /// The one point of truth for a **typed** producer's retained clause
    /// text — a wire-authored `author.add_rule` call, most notably, which
    /// receives `head`/`body` already structured and never carries a text
    /// field. `dli_syntax::parser::parse_rule_clause` reparses this spelling
    /// byte-for-byte back into an equal [`Datalog`] (a syntax-build oracle
    /// pins the round trip); this crate has no `syntax`-feature dependency
    /// to reach that parser itself, so this is a from-scratch renderer, not
    /// a call into the formatter. It intentionally matches
    /// `dli_syntax::format`'s own `AtomExpr`/`Term` rendering convention,
    /// which is a **verbatim clause-text re-emitter** and never derives text
    /// from `Datalog` structure itself — so this is the only renderer either
    /// crate has for a `Datalog`, not a second implementation to drift from
    /// a first.
    #[must_use]
    pub fn render(&self) -> String {
        let body = self
            .body
            .iter()
            .map(render_body_literal)
            .collect::<Vec<_>>()
            .join(", ");
        format!("{} :- {body}", render_atom(&self.head))
    }
}

fn render_body_literal(lit: &BodyLiteral) -> String {
    if lit.negated {
        format!("not {}", render_atom(&lit.atom))
    } else {
        render_atom(&lit.atom)
    }
}

fn render_atom(atom: &AtomExpr) -> String {
    let args: Vec<String> = atom.args.iter().map(render_term).collect();
    format!("{}({})", atom.pred.text, args.join(", "))
}

fn render_term(term: &Term) -> String {
    match term {
        Term::Var(v) => format!("${}", v.text),
        Term::Sym(s) => s.text.clone(),
        Term::Str(s) => quote(s),
        Term::Int(n) => n.to_string(),
        Term::Time(TimeRef::Era(x) | TimeRef::Event(x)) => x.text.clone(),
    }
}

fn quote(text: &str) -> String {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ident::Ident;

    fn var(name: &str) -> Term {
        Term::Var(Ident::synthetic(name))
    }

    fn sym(name: &str) -> Term {
        Term::Sym(Ident::synthetic(name))
    }

    fn atom(pred: &str, args: Vec<Term>) -> AtomExpr {
        AtomExpr {
            pred: Ident::synthetic(pred),
            args,
        }
    }

    #[test]
    fn renders_a_positive_only_clause() {
        let datalog = Datalog {
            head: atom("reachable", vec![var("x"), var("z")]),
            body: vec![BodyLiteral::pos(atom("adjacent", vec![var("x"), var("z")]))],
        };
        assert_eq!(datalog.render(), "reachable($x, $z) :- adjacent($x, $z)");
    }

    #[test]
    fn renders_negation_and_multiple_body_literals() {
        let datalog = Datalog {
            head: atom("ward-holds", vec![var("g")]),
            body: vec![
                BodyLiteral::pos(atom("seated", vec![var("g")])),
                BodyLiteral::not(atom("breached", vec![var("g")])),
            ],
        };
        assert_eq!(
            datalog.render(),
            "ward-holds($g) :- seated($g), not breached($g)"
        );
    }

    #[test]
    fn renders_symbols_strings_and_integers() {
        let datalog = Datalog {
            head: atom("rank", vec![sym("nara"), Term::Str("gold rank".to_owned())]),
            body: vec![BodyLiteral::pos(atom("count", vec![Term::Int(2)]))],
        };
        assert_eq!(datalog.render(), "rank(nara, \"gold rank\") :- count(2)");
    }
}
