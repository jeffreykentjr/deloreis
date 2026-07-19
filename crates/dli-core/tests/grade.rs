#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Property and stability coverage for the grade lattice and its rollups.

use dli_core::Grade;
use proptest::prelude::*;

fn arb_grade() -> impl Strategy<Value = Grade> {
    prop_oneof![
        Just(Grade::Asserted),
        Just(Grade::Attested),
        Just(Grade::Tested),
        Just(Grade::Checked),
    ]
}

proptest! {
    /// A one-element chain rolls up to that element (weakest identity on a
    /// singleton).
    #[test]
    fn weakest_singleton_is_identity(g in arb_grade()) {
        prop_assert_eq!(Grade::weakest([g]), g);
    }

    /// Idempotent under duplication: any run of one grade rolls up to it.
    #[test]
    fn weakest_idempotent(g in arb_grade(), n in 1usize..8) {
        prop_assert_eq!(Grade::weakest(std::iter::repeat_n(g, n)), g);
    }

    /// Order-insensitive: min over a permuted chain is unchanged (chains are
    /// ordered, but min does not care).
    #[test]
    fn weakest_order_insensitive(mut chain in prop::collection::vec(arb_grade(), 1..8)) {
        let forward = Grade::weakest(chain.iter().copied());
        chain.reverse();
        let backward = Grade::weakest(chain.iter().copied());
        prop_assert_eq!(forward, backward);
    }

    /// min-not-max: the rollup is the weakest link, never the strongest.
    #[test]
    fn weakest_picks_the_weakest_link(chain in prop::collection::vec(arb_grade(), 1..8)) {
        let rolled = Grade::weakest(chain.iter().copied());
        let weakest = chain.iter().copied().min().unwrap();
        let strongest = chain.iter().copied().max().unwrap();
        prop_assert_eq!(rolled, weakest);
        prop_assert!(rolled <= strongest);
    }

    /// `asserted` dominates nothing in the join — it never lifts a max — while
    /// it always wins the meet, being the lattice bottom.
    #[test]
    fn asserted_never_lifts_join(g in arb_grade()) {
        prop_assert_eq!(Grade::join_alt([Grade::Asserted, g]), g);
        prop_assert_eq!(Grade::weakest([Grade::Asserted, g]), Grade::Asserted);
    }
}

/// No amount of model consensus lifts the grade: folding N `attested` grades
/// stays `attested`, by either combinator (07 §3).
#[test]
fn consensus_never_lifts_the_grade() {
    for n in 1usize..12 {
        assert_eq!(
            Grade::weakest(std::iter::repeat_n(Grade::Attested, n)),
            Grade::Attested
        );
        let joined = Grade::join_alt(std::iter::repeat_n(Grade::Attested, n));
        assert_eq!(joined, Grade::Attested);
        assert!(joined <= Grade::Attested);
    }
}

#[test]
fn weakest_empty_is_the_top() {
    // The min identity: an empty *within-procedure* chain applies no weakening.
    assert_eq!(Grade::weakest(std::iter::empty()), Grade::Checked);
}

#[test]
fn join_alt_empty_is_the_bottom() {
    assert_eq!(Grade::join_alt(std::iter::empty()), Grade::Asserted);
}

#[test]
fn ord_is_the_lattice() {
    assert!(Grade::Asserted < Grade::Attested);
    assert!(Grade::Attested < Grade::Tested);
    assert!(Grade::Tested < Grade::Checked);
    assert_eq!(
        Grade::ALL,
        [
            Grade::Asserted,
            Grade::Attested,
            Grade::Tested,
            Grade::Checked
        ]
    );
}

#[test]
fn serde_is_byte_stable() {
    for (grade, token) in [
        (Grade::Asserted, "\"asserted\""),
        (Grade::Attested, "\"attested\""),
        (Grade::Tested, "\"tested\""),
        (Grade::Checked, "\"checked\""),
    ] {
        assert_eq!(serde_json::to_string(&grade).unwrap(), token);
        let back: Grade = serde_json::from_str(token).unwrap();
        assert_eq!(back, grade);
    }
}

#[test]
fn display_and_from_str_match_serde() {
    for grade in Grade::ALL {
        assert_eq!(grade.to_string(), grade.as_str());
        assert_eq!(
            format!("\"{grade}\""),
            serde_json::to_string(&grade).unwrap()
        );
        assert_eq!(grade.as_str().parse::<Grade>().unwrap(), grade);
    }
}

#[test]
fn from_str_rejects_unknown_tokens() {
    assert!("proven".parse::<Grade>().is_err());
    assert!("Checked".parse::<Grade>().is_err()); // case-sensitive, matches serde
    assert!(String::new().parse::<Grade>().is_err());
}
