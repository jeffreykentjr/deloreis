#![allow(missing_docs)]

use xtask::retired_vocabulary::scan_text;

#[test]
fn retired_terms_are_found_without_matching_neighbors() {
    let hits = scan_text("Door H is retired; HUMAN-ONLY is stale.\nDoorway is fine.");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].1, "door h");
    assert_eq!(hits[1].1, "human-only");
}
