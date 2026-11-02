//! Store-backed timeline reads agree with the certain era projection.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;
use std::{fs, sync::Arc};

#[test]
fn timeline_show_orders_events_after_reopening_the_store() {
    let source = "scope storybook\nratify d-0001\nera early < late ratified d-0001\nevent first during early ratified d-0001\nevent second during late ratified d-0001\n";
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
    let frame = json!({"dli":"2.0.0","id":"test","op":"dli.timeline.show","params":{"document":"storybook","at":"first"}});
    let response =
        dli_serve::serve::respond(&frame.to_string(), &mut sessions, &mut author).unwrap();
    let response = serde_json::to_value(response).unwrap();
    assert_eq!(response["ok"], true, "{response}");
    assert!(
        response["result"]["cursor"]["after"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "second"),
        "{response}"
    );
}
