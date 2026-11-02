//! As-of wire queries bind the projected world and preserve temporal support.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::{Value, json};
use std::{fs, sync::Arc};

fn query(source: &str, at: &str) -> Value {
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let pack = dli_world::pack::Pack {
        name: "storybook".into(),
        documents: vec![document],
    };
    let root = tempfile::tempdir().unwrap();
    let store = dli_lorestore::Lorestore::open(root.path().join("store")).unwrap();
    store.ensure_pack_imported(&pack).unwrap();
    drop(store);
    fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"storybook\"\n[documents.storybook]\nstore = \"store\"\n",
    )
    .unwrap();
    let project = dli_serve::project::Project::load(root.path()).unwrap();
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::new(project));
    let mut author = dli_serve::author::store::Store::default();
    let frame = json!({"dli":"2.0.0","id":"test","op":"dli.query","params":{"document":"storybook","goal_atom":{"pred":"home","args":[{"Sym":"wren"},{"Sym":"old"}]},"as_of":at}});
    serde_json::to_value(
        dli_serve::serve::respond(&frame.to_string(), &mut sessions, &mut author).unwrap(),
    )
    .unwrap()
}

#[test]
fn distinct_temporal_bounds_bind_distinct_projected_worlds() {
    let prefix = "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nbefore first -> second ratified d-0001\n";
    let earlier = query(
        &format!("{prefix}state wren.home = old since first ratified d-0001\n"),
        "first",
    );
    let later = query(
        &format!("{prefix}state wren.home = old since second ratified d-0001\n"),
        "first",
    );
    assert_eq!(earlier["ok"], true, "{earlier}");
    assert_eq!(later["ok"], true, "{later}");
    assert_eq!(earlier["result"]["rows"].as_array().unwrap().len(), 1);
    assert!(later["result"]["rows"].as_array().unwrap().is_empty());
    assert_ne!(earlier["result"]["root_hash"], later["result"]["root_hash"]);
}

#[test]
fn unresolved_inertial_order_has_a_specific_error() {
    let source = "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nevent cutoff ratified d-0001\nbefore first -> second ratified d-0001\nbefore first -> cutoff ratified d-0001\ninertial home ratified d-0001\nstate wren.home = old since first ratified d-0001\nstate wren.home = new since second ratified d-0001\n";
    let response = query(source, "cutoff");
    assert_eq!(response["ok"], false, "{response}");
    assert_eq!(
        response["error"]["code"], "temporal-indeterminate",
        "{response}"
    );
}
