//! Coreference closure: the canonical-representative map for a `same-as` class
//! (WS-1, identity kernel).
//!
//! `same-as` is symmetric and transitive; the merge itself is synthesized
//! Datalog rules over the evaluable world (`dli_eval::world::synthesize_alias_rules`)
//! — this module is the **second**, and last, mechanism the identity feature
//! needs, mirroring [`crate::fd`]'s role as the shared primitive under the checks
//! that read the *authored* layer rather than the derived one. A check that wants
//! to treat two aliased names as one slot builds a [`canon_map`] once from the
//! world's certain `same-as` pairs and canonicalizes through [`canon`] before
//! grouping — never re-deriving the closure itself.
//!
//! **Why union-find is enough without feeding it the derived closure.**
//! Connectivity is transitive by construction: unioning only the direct pairs a
//! caller supplies computes the same equivalence classes as unioning the full
//! symmetric-transitive closure would. So a caller may pass just the *authored*
//! `same-as` pairs (not the rule-derived symmetric/transitive copies) and get
//! back exactly the classes the synthesized rules would derive — the two
//! mechanisms agree by the mathematics of connectivity, not by construction
//! effort.

use std::collections::BTreeMap;

/// Build the canonical-representative map for a set of symmetric identity
/// pairs.
///
/// A union-find over the given `(a, b)` pairs — undirected, so the caller need
/// not supply both `(a, b)` and `(b, a)`. The representative of each resulting
/// class is its **lexicographically smallest member**, independent of the order
/// pairs arrive in (permutation-stable): whichever of two roots is smaller
/// always wins a union, so by induction the eventual root of any connected
/// component is that component's global minimum, however the unions were
/// ordered.
///
/// A self-pair (`a == a`) is ignored — it asserts nothing a union-find needs.
/// The result holds **only the non-trivial entries**: a name that maps to
/// itself (a singleton class, or a name no pair ever mentioned) is omitted, so
/// the map is as small as possible and [`canon`] returns the name unchanged for
/// anything absent from it. In particular, an empty `pairs` iterator returns an
/// empty map — the identity function everywhere, which is what makes a world
/// with zero `same-as` declarations byte-identical to one with none of this
/// machinery at all.
///
/// Deterministic throughout: `BTreeMap` iteration and lexicographic tie-break,
/// no hashing, no input-order dependence.
#[must_use]
pub fn canon_map<I: IntoIterator<Item = (String, String)>>(pairs: I) -> BTreeMap<String, String> {
    let mut parent: BTreeMap<String, String> = BTreeMap::new();

    for (a, b) in pairs {
        if a == b {
            continue;
        }
        parent.entry(a.clone()).or_insert_with(|| a.clone());
        parent.entry(b.clone()).or_insert_with(|| b.clone());
        let ra = find(&mut parent, &a);
        let rb = find(&mut parent, &b);
        if ra == rb {
            continue;
        }
        // Lexicographic-min representative: the smaller root absorbs the
        // larger, so the class root is always the component's global minimum.
        if ra < rb {
            parent.insert(rb, ra);
        } else {
            parent.insert(ra, rb);
        }
    }

    let members: Vec<String> = parent.keys().cloned().collect();
    let mut out = BTreeMap::new();
    for member in members {
        let root = find(&mut parent, &member);
        if root != member {
            out.insert(member, root);
        }
    }
    out
}

/// Find the root of `x`'s tree, compressing every visited node's parent link to
/// point directly at the root. `x` must already be a key of `parent` (every
/// caller in this module inserts a self-loop for a fresh name before calling).
fn find(parent: &mut BTreeMap<String, String>, x: &str) -> String {
    let mut root = x.to_owned();
    while let Some(next) = parent.get(&root) {
        if next == &root {
            break;
        }
        root.clone_from(next);
    }
    let mut cur = x.to_owned();
    while cur != root {
        let next = parent.get(&cur).cloned().unwrap_or_else(|| cur.clone());
        parent.insert(cur, root.clone());
        cur = next;
    }
    root
}

/// Look up `name`'s canonical representative in a [`canon_map`] result, or
/// `name` itself when it is not a member of any class (a singleton, or a name
/// the map never saw). A no-op — the identity function — whenever `map` is
/// empty, which is the behavior-neutrality guarantee: a world with no
/// `same-as` declarations canonicalizes every name to itself.
#[must_use]
pub fn canon<'a>(map: &'a BTreeMap<String, String>, name: &'a str) -> &'a str {
    map.get(name).map_or(name, String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> String {
        text.to_owned()
    }

    #[test]
    fn empty_input_is_empty_map() {
        let map = canon_map(Vec::<(String, String)>::new());
        assert!(map.is_empty());
    }

    #[test]
    fn self_pair_is_ignored() {
        let map = canon_map([(s("a"), s("a"))]);
        assert!(map.is_empty(), "a self-pair asserts nothing: {map:?}");
    }

    #[test]
    fn direct_pair_canonicalizes_to_the_lexicographic_min() {
        let map = canon_map([(s("robert"), s("bob"))]);
        assert_eq!(canon(&map, "robert"), "bob");
        assert_eq!(
            canon(&map, "bob"),
            "bob",
            "the representative maps to itself, so it is omitted from the map (non-trivial only)"
        );
        assert!(!map.contains_key("bob"));
    }

    #[test]
    fn chain_closes_transitively() {
        // a=b, b=c should merge {a, b, c} into one class rooted at the min.
        let map = canon_map([(s("charlie"), s("bob")), (s("bob"), s("alice"))]);
        assert_eq!(canon(&map, "charlie"), "alice");
        assert_eq!(canon(&map, "bob"), "alice");
        assert_eq!(canon(&map, "alice"), "alice");
    }

    #[test]
    fn cycle_terminates_and_settles_on_the_min() {
        // a=b, b=c, c=a — a cycle must not loop the union-find forever.
        let map = canon_map([
            (s("charlie"), s("bob")),
            (s("bob"), s("alice")),
            (s("alice"), s("charlie")),
        ]);
        assert_eq!(canon(&map, "charlie"), "alice");
        assert_eq!(canon(&map, "bob"), "alice");
    }

    #[test]
    fn self_merge_derives_nothing_new() {
        let map = canon_map([(s("alice"), s("bob")), (s("alice"), s("alice"))]);
        assert_eq!(canon(&map, "alice"), "alice");
        assert_eq!(canon(&map, "bob"), "alice");
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn disjoint_pairs_stay_disjoint() {
        let map = canon_map([(s("bob"), s("robert")), (s("liz"), s("beth"))]);
        assert_eq!(canon(&map, "robert"), "bob");
        assert_eq!(canon(&map, "liz"), "beth");
        // No cross-class contamination.
        assert_ne!(canon(&map, "robert"), canon(&map, "liz"));
    }

    #[test]
    fn permutation_of_pairs_does_not_change_the_result() {
        let forward = canon_map([
            (s("zed"), s("yara")),
            (s("yara"), s("xan")),
            (s("xan"), s("amara")),
        ]);
        let reversed = canon_map([
            (s("xan"), s("amara")),
            (s("yara"), s("xan")),
            (s("zed"), s("yara")),
        ]);
        assert_eq!(forward, reversed);
        for name in ["zed", "yara", "xan", "amara"] {
            assert_eq!(canon(&forward, name), "amara");
        }
    }

    #[test]
    fn undirected_pair_order_does_not_matter() {
        let ab = canon_map([(s("bob"), s("robert"))]);
        let ba = canon_map([(s("robert"), s("bob"))]);
        assert_eq!(ab, ba);
    }

    #[test]
    fn canon_of_absent_name_is_itself() {
        let map = canon_map([(s("bob"), s("robert"))]);
        assert_eq!(canon(&map, "camelot"), "camelot");
    }

    #[test]
    fn large_class_merges_to_one_root_regardless_of_arrival_order() {
        let map = canon_map([
            (s("mordred"), s("m")),
            (s("arthur"), s("a")),
            (s("a"), s("mordred")),
            (s("m"), s("arthur")),
        ]);
        let root = canon(&map, "mordred").to_owned();
        for name in ["mordred", "m", "arthur", "a"] {
            assert_eq!(canon(&map, name), root, "every member shares one root");
        }
        assert_eq!(root, "a", "the global lexicographic minimum wins");
    }
}
