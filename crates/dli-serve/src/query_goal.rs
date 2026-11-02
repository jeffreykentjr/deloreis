//! Shared parsing for query goals carried on the worker wire.
//!
//! This is the adapter behind the *string* `goal` — the text-on-wire remnant
//! whose 0.6.3 typed successor is `goal_atom`. LS-7 gates it: without the
//! `syntax` feature there is no parser, [`parse`] resolves nothing, and each
//! caller refuses the request by naming `goal_atom` (see [`UNTYPED_GOAL`]).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use dli_ast::term::AtomExpr;

/// How a caller explains a `goal` string it could not turn into an atom. With
/// the front end linked the string is malformed and the repair is to write a
/// well-formed one; without it the repair is the typed successor.
#[cfg(feature = "syntax")]
pub(crate) const UNTYPED_GOAL: &str = "write a rendered goal like serves(lancelot, ?who)";
#[cfg(not(feature = "syntax"))]
pub(crate) const UNTYPED_GOAL: &str = "this build links no `.dli` front end, so a rendered goal string cannot be read; send the \
     typed `goal_atom` successor instead";

/// Parse the wire form `pred(a, ?name, "literal")` through the real syntax
/// parser. The wire uses `?name`; the language AST uses `$name`.
///
/// Without the `syntax` feature there is no parser to run, so no goal string
/// resolves and every caller falls to its `goal_atom` refusal.
pub(crate) fn parse(text: &str) -> Option<AtomExpr> {
    let text = text.trim();
    let open = text.find('(')?;
    if open == 0 || !text.ends_with(')') {
        return None;
    }
    parse_rewritten(&rewrite_variables(text))
}

#[cfg(feature = "syntax")]
fn parse_rewritten(text: &str) -> Option<AtomExpr> {
    dli_syntax::parse_clause_atom(&dli_ast::clause::Clause {
        text: text.to_owned(),
        span: dli_ast::span::Span::synthetic(),
    })
}

#[cfg(not(feature = "syntax"))]
fn parse_rewritten(_text: &str) -> Option<AtomExpr> {
    None
}

fn rewrite_variables(text: &str) -> String {
    let named = named_variables(text);
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    let mut escaped = false;
    let mut fresh = 0_u32;

    while let Some(ch) = chars.next() {
        if quoted {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
            continue;
        }
        if ch == '"' {
            quoted = true;
            out.push(ch);
            continue;
        }
        if ch != '?' {
            out.push(ch);
            continue;
        }

        out.push('$');
        if chars
            .peek()
            .is_some_and(|next| next.is_alphanumeric() || matches!(next, '_' | '-'))
        {
            while chars
                .peek()
                .is_some_and(|next| next.is_alphanumeric() || matches!(next, '_' | '-'))
            {
                out.push(chars.next().expect("peeked query variable character"));
            }
        } else {
            loop {
                let candidate = format!("__query_{fresh}");
                fresh += 1;
                if !named.contains(&candidate) {
                    let _ = write!(out, "{candidate}");
                    break;
                }
            }
        }
    }
    out
}

fn named_variables(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if quoted {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
            continue;
        }
        if ch == '"' {
            quoted = true;
            continue;
        }
        if ch != '?'
            || chars
                .peek()
                .is_none_or(|next| !next.is_alphanumeric() && !matches!(next, '_' | '-'))
        {
            continue;
        }
        let mut name = String::new();
        while chars
            .peek()
            .is_some_and(|next| next.is_alphanumeric() || matches!(next, '_' | '-'))
        {
            name.push(chars.next().expect("peeked query variable character"));
        }
        names.insert(name);
    }
    names
}

#[cfg(test)]
mod tests {
    /// The wire-adapter tests exercise the parse itself, so they belong to the
    /// build that has one; on the default build the adapter's whole behaviour
    /// is "resolves nothing", which `dli-serve`'s `default_build` acceptance
    /// test asserts on the wire. The gate is a nested module so both attributes
    /// stay outer ones and the `#[cfg(test)]` above is the literal form the
    /// boundary scanner recognizes.
    #[cfg(feature = "syntax")]
    mod with_front_end {
        use dli_ast::term::Term;

        use crate::query_goal::parse;

        #[test]
        fn parses_named_and_anonymous_wire_variables() {
            let atom = parse("knows(?who, ?)").expect("wire goal parses");
            assert!(matches!(&atom.args[0], Term::Var(var) if var.text == "who"));
            assert!(matches!(&atom.args[1], Term::Var(var) if var.text == "__query_0"));
        }

        #[test]
        fn anonymous_variables_do_not_alias_legal_named_variables() {
            let atom = parse("pair(?, ?__query_0)").expect("wire goal parses");
            assert!(matches!(&atom.args[0], Term::Var(var) if var.text == "__query_1"));
            assert!(matches!(&atom.args[1], Term::Var(var) if var.text == "__query_0"));
        }

        #[test]
        fn parses_quoted_literals_with_delimiters() {
            let atom = parse(r#"role(nara, "warden, river")"#).expect("wire goal parses");
            assert!(matches!(&atom.args[1], Term::Str(text) if text == "warden, river"));
        }

        #[test]
        fn question_marks_inside_strings_are_data() {
            let atom = parse(r#"asks(nara, "why?")"#).expect("wire goal parses");
            assert!(matches!(&atom.args[1], Term::Str(text) if text == "why?"));
        }
    }
}
