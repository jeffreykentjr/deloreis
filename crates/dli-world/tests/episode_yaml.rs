//! The committed episodes example must deserialize through the real types
//! and fold correctly — pinning `worlds/tpth/episodes/saltmarket-episodes.yaml`
//! against type drift. Episodes ride as payload data until their surface form
//! lands (TIME-3); this test is that payload's contract.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_world::episode::{Episode, fold};

fn example_yaml() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../worlds/tpth/episodes/saltmarket-episodes.yaml"
    );
    std::fs::read_to_string(path).expect("worlds/tpth/episodes/saltmarket-episodes.yaml exists")
}

#[test]
fn committed_episode_example_deserializes_and_folds() {
    let episodes: Vec<Episode> =
        serde_yaml_ng::from_str(&example_yaml()).expect("the committed YAML matches the types");
    assert_eq!(episodes.len(), 3, "the example carries three episodes");

    let flashbacks = episodes
        .iter()
        .flat_map(|e| &e.scenes)
        .filter(|s| s.scene_anchor.is_some())
        .count();
    assert_eq!(flashbacks, 1, "exactly one flashback scene");

    // The fold must run in production order without panicking.
    let _fold = fold(&episodes);
}

#[test]
fn corrupted_enum_tag_is_a_clean_error() {
    let corrupted = example_yaml().replace("knower: Listener", "knower: Audience");
    let outcome: Result<Vec<Episode>, _> = serde_yaml_ng::from_str(&corrupted);
    assert!(
        outcome.is_err(),
        "an unknown knower tag must not deserialize"
    );
}
