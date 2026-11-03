//! The functional-dependency primitive shared by every certain-layer FD check.
//!
//! A functional dependency is the rule "for this predicate, each subject carries
//! at most one value" — `rank(lyra)` is one rank, not several. Three checks in
//! the workspace ask the same question of a certain layer and must answer it the
//! same way, or an author is told a slot is fine on one surface and broken on
//! another:
//!
//! - the load/read check (`dli_world::check::continuity`'s `fd_violations`),
//!   over a compiled world's authored source facts;
//! - the in-memory fork merge (`dli_eval::fork`'s `certain_conflicts`), over the
//!   joined saturation of two forks;
//! - the durable-store merge (`dli_lorestore::merge`'s `certain_layer_conflicts`),
//!   over the recompiled merged world.
//!
//! They live in three crates at three layers, so the shared logic sinks here to
//! the common floor. This module owns the two pieces they must not drift on: the
//! value-distinctness key ([`fd_value_key`]) and the grouping that turns a stream
//! of certain `(pred, subject, value)` slots into violations ([`fd_violations`]),
//! plus the data-driven inference ([`inferred_functional`]) the delta-vs-base
//! callers union with the author's declared set. What differs between callers —
//! which atom layer to read (authored source vs saturated derived) and how to
//! render a finding vs a `Conflict` — stays with each caller, by design (DL-0075).

use std::collections::{BTreeMap, BTreeSet};

/// The key the FD/contradiction check compares values by: lowercased, with a
/// hyphen/underscore/whitespace run that sits **between two other characters**
/// folded to a single space, and leading/trailing whitespace trimmed. `gold`/
/// `Gold` and `bell-song`/`bell song`/`Bell  Song` collapse to one value, while
/// every other character (digits, `.`, `/`, `:` …) survives — so `2.5` and `2-5`
/// stay two distinct values and `gold`/`golden` stay distinct (never stems).
///
/// **Only interior separators fold, and a leading/trailing hyphen or underscore
/// is kept.** A separator's word-joining meaning only applies between words; at
/// an edge it is part of the value (a sign, a prefix). So `-5` and `5` stay
/// distinct, as do `_gold`/`gold` and `5-`/`5` — folding them would silently drop
/// a real contradiction (charter koan 4), the one thing this key must never do.
/// (Leading/trailing *whitespace* is trimmed, being cosmetic; a leading/trailing
/// `-`/`_` is not.)
///
/// It is deliberately narrower than the killed-token guardrail's liberal fold
/// (which folds *every* non-alphanumeric for ban-evasion resistance); that fold
/// lives in `dli-world` and is not this function.
#[must_use]
pub fn fd_value_key(value: &str) -> String {
    // Lowercase, then trim leading/trailing whitespace (cosmetic) — but not a
    // leading/trailing `-`/`_`, which is part of the value.
    let lowered = value.to_lowercase();
    let chars: Vec<char> = lowered.trim().chars().collect();
    let is_sep = |c: char| c == '-' || c == '_' || c.is_whitespace();

    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if is_sep(chars[i]) {
            let mut j = i;
            while j < chars.len() && is_sep(chars[j]) {
                j += 1;
            }
            if i == 0 || j == chars.len() {
                // Edge run: keep the value-bearing `-`/`_`, drop only whitespace.
                for &c in &chars[i..j] {
                    if !c.is_whitespace() {
                        out.push(c);
                    }
                }
            } else {
                // Interior run: one word separator, whatever its spelling.
                out.push(' ');
            }
            i = j;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// A functional-dependency violation on a certain layer: one subject carrying two
/// or more key-distinct values for a predicate held functional.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FdViolation {
    /// The functional predicate whose dependency is broken.
    pub pred: String,
    /// The subject carrying more than one value.
    pub subject: String,
    /// The distinct values, one raw spelling per [`fd_value_key`], in key order.
    /// Always length ≥ 2.
    pub values: Vec<String>,
}

/// Group certain `(pred, subject, value)` slots by slot, keying value distinctness
/// on [`fd_value_key`], and report every slot on a **functional** predicate that
/// holds two or more distinct values — a functional-dependency break.
///
/// `functional` is the set of predicates to check; the caller supplies it (the
/// author's declared set, optionally unioned with [`inferred_functional`]). A
/// predicate outside the set raises nothing, whatever its shape: canon's data
/// alone cannot tell a functional slip from a genuine many-to-many relation, so
/// the ruling is the caller's, not this primitive's (koan 3).
///
/// Distinctness keys on [`fd_value_key`], so `gold`/`Gold` count once; the raw
/// spelling first seen for each key is kept for the caller's message. Output is
/// deterministic — sorted by `(pred, subject)`.
#[must_use]
pub fn fd_violations<I>(certain: I, functional: &BTreeSet<String>) -> Vec<FdViolation>
where
    I: IntoIterator<Item = (String, String, String)>,
{
    // pred -> subject -> (normalized key -> first raw spelling).
    let mut by_pred: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>> = BTreeMap::new();
    for (pred, subject, value) in certain {
        if !functional.contains(&pred) {
            continue;
        }
        by_pred
            .entry(pred)
            .or_default()
            .entry(subject)
            .or_default()
            .entry(fd_value_key(&value))
            .or_insert(value);
    }
    let mut out = Vec::new();
    for (pred, subjects) in by_pred {
        for (subject, values) in subjects {
            if values.len() < 2 {
                continue;
            }
            out.push(FdViolation {
                pred: pred.clone(),
                subject,
                values: values.into_values().collect(),
            });
        }
    }
    out
}

/// The predicates a certain layer proves functional by its own shape: every
/// predicate that appears in at least one certain binary slot and whose every
/// subject carries at most one [`fd_value_key`]-distinct value.
///
/// This is the data-driven inference the delta-vs-base checks (fork/merge, and
/// the read side's claim-relative `predicate_is_functional`) union with the
/// author's declared set. It is deliberately **not** used by the world-internal
/// load check over canon-against-itself: there a first-seen two-valued subject is
/// as readily a many-to-many hub as a slip, so that check requires a declaration
/// (koan 3, DL-0074). Inference is sound only as a *base* against which a *delta*
/// adds a second value.
#[must_use]
pub fn inferred_functional<I>(certain: I) -> BTreeSet<String>
where
    I: IntoIterator<Item = (String, String, String)>,
{
    let mut by_pred: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    for (pred, subject, value) in certain {
        by_pred
            .entry(pred)
            .or_default()
            .entry(subject)
            .or_default()
            .insert(fd_value_key(&value));
    }
    by_pred
        .into_iter()
        .filter(|(_, subjects)| {
            !subjects.is_empty() && subjects.values().all(|values| values.len() <= 1)
        })
        .map(|(pred, _)| pred)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn slot(pred: &str, subject: &str, value: &str) -> (String, String, String) {
        (pred.to_owned(), subject.to_owned(), value.to_owned())
    }

    #[test]
    fn value_key_folds_only_case_and_interior_separators() {
        assert_eq!(fd_value_key("Gold"), fd_value_key("gold"));
        assert_eq!(fd_value_key("bell-song"), fd_value_key("bell song"));
        assert_eq!(fd_value_key("bell_song"), fd_value_key("Bell  Song"));
        assert_eq!(fd_value_key(" gold "), fd_value_key("gold")); // edge whitespace is cosmetic
        // Genuinely distinct values must never merge (koan 4).
        assert_ne!(fd_value_key("gold"), fd_value_key("golden"));
        assert_ne!(fd_value_key("2.5"), fd_value_key("2-5"));
        assert_ne!(fd_value_key("a/b"), fd_value_key("a b"));
    }

    #[test]
    fn value_key_keeps_leading_and_trailing_separators() {
        // A leading/trailing `-`/`_` is part of the value — folding it would
        // silently drop a real contradiction (koan 4). Regression pins for the
        // edge-fold bug the fork/merge paths inherited (issue 59 red-team F1).
        assert_ne!(
            fd_value_key("-5"),
            fd_value_key("5"),
            "leading sign is significant"
        );
        assert_ne!(
            fd_value_key("5-"),
            fd_value_key("5"),
            "trailing hyphen is significant"
        );
        assert_ne!(fd_value_key("_gold"), fd_value_key("gold"));
        assert_ne!(fd_value_key("gold_"), fd_value_key("gold"));
        assert_ne!(fd_value_key("--x"), fd_value_key("x"));
        assert_ne!(
            fd_value_key("-5"),
            fd_value_key("-6"),
            "the digit still counts"
        );
        // But interior folding is unaffected.
        assert_eq!(fd_value_key("a-b-c"), fd_value_key("a b c"));
    }

    #[test]
    fn violations_fire_only_on_functional_predicates() {
        let certain = vec![
            slot("rank", "lyra", "gold"),
            slot("rank", "lyra", "silver"),
            slot("adjacent", "camelot", "north"),
            slot("adjacent", "camelot", "south"),
        ];
        let out = fd_violations(certain, &set(&["rank"]));
        assert_eq!(out.len(), 1, "only the declared-functional slot: {out:?}");
        assert_eq!(out[0].pred, "rank");
        assert_eq!(out[0].subject, "lyra");
        assert_eq!(out[0].values, vec!["gold".to_owned(), "silver".to_owned()]);
    }

    #[test]
    fn cosmetic_drift_is_one_value_not_a_violation() {
        let certain = vec![slot("rank", "lyra", "gold"), slot("rank", "lyra", "Gold")];
        assert!(
            fd_violations(certain, &set(&["rank"])).is_empty(),
            "gold and Gold are one value"
        );
    }

    #[test]
    fn distinct_values_still_violate() {
        // `2.5` vs `2-5` are distinct (only `-`/`_`/space fold), so this is real.
        let certain = vec![slot("width", "gate", "2.5"), slot("width", "gate", "2-5")];
        let out = fd_violations(certain, &set(&["width"]));
        assert_eq!(out.len(), 1, "distinct values are not merged: {out:?}");
    }

    #[test]
    fn inference_excludes_multivalued_and_keys_on_value() {
        let certain = vec![
            slot("rank", "lyra", "gold"),
            slot("rank", "fen", "silver"),
            slot("rank", "gwen", "Gold"), // cosmetic dup of a value, still ≤1 per subject
            slot("adjacent", "camelot", "north"),
            slot("adjacent", "camelot", "south"), // two values → not functional
        ];
        let inferred = inferred_functional(certain);
        assert!(
            inferred.contains("rank"),
            "rank is functional: {inferred:?}"
        );
        assert!(
            !inferred.contains("adjacent"),
            "adjacent is many-to-many: {inferred:?}"
        );
    }

    #[test]
    fn inference_folds_cosmetic_drift_within_a_subject() {
        // One subject with `gold` and `Gold` is still single-valued once keyed.
        let certain = vec![slot("rank", "lyra", "gold"), slot("rank", "lyra", "Gold")];
        assert!(
            inferred_functional(certain).contains("rank"),
            "cosmetic drift does not make a subject multi-valued"
        );
    }
}
