//! Shared parsing for query goals carried on the worker wire.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use dli_ast::clause::Clause;
use dli_ast::span::Span;
use dli_ast::term::AtomExpr;
use dli_syntax::parse_clause_atom;

/// Parse the wire form `pred(a, ?name, "literal")` through the real syntax
/// parser. The wire uses `?name`; the language AST uses `$name`.
pub(crate) fn parse(text: &str) -> Option<AtomExpr> {
    let text = text.trim();
    let open = text.find('(')?;
    if open == 0 || !text.ends_with(')') {
        return None;
    }
    parse_clause_atom(&Clause {
        text: rewrite_variables(text),
        span: Span::synthetic(),
    })
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
    use dli_ast::term::Term;

    use super::parse;

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
        let atom = parse(r#"role(mara, "warden, river")"#).expect("wire goal parses");
        assert!(matches!(&atom.args[1], Term::Str(text) if text == "warden, river"));
    }

    #[test]
    fn question_marks_inside_strings_are_data() {
        let atom = parse(r#"asks(mara, "why?")"#).expect("wire goal parses");
        assert!(matches!(&atom.args[1], Term::Str(text) if text == "why?"));
    }
}
