//! Store-backed projects: a document whose source form is a lorestore (DM-2,
//! DL-0055).
//!
//! The load-bearing property is that this path is **syntax-free** — these tests
//! run in the default build lane, where `dli_syntax` is not linked and a `.dli`
//! project refuses at load. A store-backed project must load, compile, and
//! serve there, because that is the build the consumer's corpus pivot runs on.
//!
//! The two modes are the other half. A document that pins `head` is a replay:
//! it stays at that commit no matter what lands on the branch afterwards. A
//! document that does not pin follows the branch tip, which is what an
//! authoring server wants — materialize, restart, and the reload sees the work.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use dli_ast::annot::{Annotation, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, EntityDecl, EntityKind, FactDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_lorestore::Lorestore;
use dli_serve::project::Project;
use dli_world::pack::Pack;

fn temp(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    fs::remove_dir_all(&root).ok();
    fs::create_dir_all(&root).unwrap();
    root
}

fn id(text: &str) -> Ident {
    Ident {
        text: text.to_owned(),
        span: Span::synthetic(),
    }
}

fn proposed() -> Annotation {
    Annotation {
        marker: ProvenanceMarker::Proposed,
        citations: Vec::new(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

/// A pack naming the given people, so one commit's world can be told from
/// another's by who is in it. Advancing the branch means importing a superset —
/// the shape a materialize leaves behind, adding declarations rather than
/// replacing the world (a replacement is refused as a foreign base).
fn pack_with(scope: &str, people: &[&str]) -> Pack {
    Pack {
        name: scope.to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id(scope),
                span: Span::synthetic(),
            }),
            decls: people
                .iter()
                .flat_map(|person| {
                    [
                        Decl::Entity(EntityDecl {
                            kind: EntityKind::Person,
                            name: id(person),
                            label: None,
                            time: None,
                            slots: Vec::new(),
                            provenance: Some(proposed()),
                        }),
                        // A relation as well as the entity, so the world has a
                        // declared predicate a query can name.
                        Decl::Fact(FactDecl {
                            atom: AtomExpr {
                                pred: id("sworn"),
                                args: vec![Term::Sym(id(person))],
                            },
                            triple: false,
                            provenance: Some(proposed()),
                        }),
                    ]
                })
                .collect(),
        }],
    }
}

/// Seed a lorestore under `root/store` holding one pack, and return its dir.
fn seed_store(root: &Path, scope: &str, person: &str) -> PathBuf {
    let dir = root.join("store");
    fs::create_dir_all(&dir).unwrap();
    let store = Lorestore::open(dir.clone()).expect("open store");
    store
        .ensure_pack_imported(&pack_with(scope, &[person]))
        .expect("import pack");
    dir
}

fn write_manifest(root: &Path, body: &str) {
    fs::write(
        root.join("deloreis.toml"),
        format!("version = 1\nname = \"store-project\"\n{body}"),
    )
    .unwrap();
}

fn head_of(dir: &Path) -> String {
    let store = Lorestore::open(dir.to_path_buf()).expect("open store");
    store
        .branch_head(dli_lorestore::MAIN_REF)
        .expect("read head")
        .expect("the seeded store has a head")
        .as_str()
        .to_owned()
}

/// The people a loaded document declares, as a sorted list — enough to tell one
/// commit's world from another's.
fn people(project: &Project, doc: &str) -> Vec<String> {
    let document = project.documents().find(|(id, _)| *id == doc).unwrap().1;
    let mut names: Vec<String> = document
        .pack
        .documents
        .iter()
        .flat_map(|d| &d.decls)
        .filter_map(|decl| match decl {
            Decl::Entity(entity) => Some(entity.name.text.clone()),
            _ => None,
        })
        .collect();
    names.sort();
    names
}

#[test]
fn a_store_backed_project_loads_and_compiles() {
    let root = temp("store-backed-loads");
    seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");

    let project = Project::load(&root).expect("a store-backed project loads");
    assert_eq!(people(&project, "storybook"), vec!["bedivere".to_owned()]);
}

#[test]
fn the_fingerprint_is_the_store_head_not_a_workspace_digest() {
    let root = temp("store-backed-fingerprint");
    let dir = seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");

    let project = Project::load(&root).expect("loads");
    let document = project
        .documents()
        .find(|(id, _)| *id == "storybook")
        .unwrap()
        .1;

    assert_eq!(
        document.source_digest,
        head_of(&dir),
        "a store-backed document is fingerprinted by its resolved head"
    );
    assert!(
        document.source_digest.starts_with("sha256:"),
        "the store's content-hash regime, not the `jcs:` workspace regime: {}",
        document.source_digest
    );
}

#[test]
fn a_pinned_head_serves_that_commit_and_does_not_follow_the_branch() {
    let root = temp("store-backed-pinned");
    let dir = seed_store(&root, "storybook", "bedivere");
    let pinned = head_of(&dir);

    // Advance the branch past the pin.
    let store = Lorestore::open(dir.clone()).expect("open store");
    store
        .import_pack(&pack_with("storybook", &["bedivere", "perin"]))
        .expect("second import advances the branch");
    assert_ne!(head_of(&dir), pinned, "the branch moved");

    write_manifest(
        &root,
        &format!("[documents.storybook]\nstore = \"store\"\nhead = \"{pinned}\"\n"),
    );
    let project = Project::load(&root).expect("a pinned project loads");

    let document = project
        .documents()
        .find(|(id, _)| *id == "storybook")
        .unwrap()
        .1;
    assert_eq!(
        document.source_digest, pinned,
        "a pinned document serves the commit the manifest names, not the tip"
    );
    assert!(document.store_pinned, "the document reports itself pinned");
    assert_eq!(
        people(&project, "storybook"),
        vec!["bedivere".to_owned()],
        "the world is the pinned commit's, without the later import"
    );
}

/// A pin fixes the served replay, not the origin branch an author explicitly
/// writes to. Its workspace therefore needs the live branch interner when the
/// branch advanced beyond the pinned commit.
#[test]
fn a_pinned_project_can_materialize_after_its_origin_branch_advances() {
    use std::sync::Arc;

    let root = temp("store-backed-pinned-authors");
    let dir = seed_store(&root, "storybook", "bedivere");
    let pinned = head_of(&dir);
    let lorestore = Lorestore::open(dir).expect("open store");
    lorestore
        .import_pack(&pack_with("storybook", &["bedivere", "perin"]))
        .expect("advance origin branch");
    write_manifest(
        &root,
        &format!("[documents.storybook]\nstore = \"store\"\nhead = \"{pinned}\"\n"),
    );

    let project = Arc::new(Project::load(&root).expect("pinned project loads"));
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
    let mut author = author_store_for(&project);
    let opened = send(
        &serde_json::json!({"dli":"2.0.0","id":"open","op":"author.open",
                            "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let workspace = opened["result"]["workspace"]
        .as_str()
        .expect("workspace id")
        .to_owned();
    let staged = send(
        &serde_json::json!({"dli":"2.0.0","id":"stage","op":"author.add_fact",
                            "params":{"workspace":workspace,"entity":"bedivere",
                                      "key":"color","value":"red",
                                      "provenance":{"kind":"proposed","origin":"pinned-author"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(staged["ok"], true, "the pinned workspace stages: {staged}");
    let materialized = send(
        &serde_json::json!({"dli":"2.0.0","id":"commit","op":"author.materialize",
                            "params":{"workspace":workspace}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        materialized["ok"], true,
        "a pinned replay can still append to its advanced origin branch: {materialized}"
    );
    let reopened = send(
        &serde_json::json!({"dli":"2.0.0","id":"reopen","op":"author.open",
                            "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let reopened_workspace = reopened["result"]["workspace"].as_str().unwrap();
    let status = send(
        &serde_json::json!({"dli":"2.0.0","id":"status","op":"author.status",
                            "params":{"workspace":reopened_workspace}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(status["result"]["root_hash"], pinned, "{status}");
}

#[test]
fn bulk_decisions_from_a_pinned_replay_append_to_the_live_origin() {
    use std::sync::Arc;

    let root = temp("store-backed-pinned-bulk-decisions");
    let dir = seed_store(&root, "storybook", "bedivere");
    let pinned = head_of(&dir);
    let lorestore = Lorestore::open(dir.clone()).expect("open store");
    lorestore
        .import_pack(&pack_with("storybook", &["bedivere", "perin"]))
        .expect("advance origin branch");
    let live_before = lorestore
        .branch_head(dli_lorestore::MAIN_REF)
        .expect("read live head")
        .expect("live head exists");
    write_manifest(
        &root,
        &format!("[documents.storybook]\nstore = \"store\"\nhead = \"{pinned}\"\n"),
    );

    let project = Arc::new(Project::load(&root).expect("pinned project loads"));
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
    let mut author = author_store_for(&project);
    let opened = send(
        &serde_json::json!({"dli":"2.0.0","id":"open","op":"author.open",
                            "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let workspace = opened["result"]["workspace"]
        .as_str()
        .expect("workspace id");
    let recorded = send(
        &serde_json::json!({"dli":"2.0.0","id":"bulk","op":"author.record_decisions",
                            "params":{"workspace":workspace,
                                      "records":[{"decision":"d-pinned"}]}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(recorded["ok"], true, "bulk record succeeds: {recorded}");
    assert_eq!(recorded["result"]["recorded"], 1);

    let live_after = lorestore
        .branch_head(dli_lorestore::MAIN_REF)
        .expect("read live head")
        .expect("live head exists");
    assert_ne!(live_after, live_before, "the live origin branch advances");
    assert!(
        lorestore
            .decisions_at(&live_after)
            .expect("read live decisions")
            .contains(&"d-pinned".to_owned()),
        "the decision lands on the origin, not an import copy"
    );
    assert_eq!(
        project
            .documents()
            .find(|(id, _)| *id == "storybook")
            .expect("pinned document")
            .1
            .source_digest,
        pinned,
        "the served replay remains pinned while authoring targets its origin"
    );
}

/// A pinned replay intentionally authors to its live origin, but opening the
/// workspace still captures one whole-ref fence. An intervening writer must
/// therefore refuse materialize rather than silently append the staged work to
/// that newer origin state.
#[test]
fn a_pinned_workspace_refuses_an_origin_advance_after_open() {
    use std::sync::Arc;

    let root = temp("store-backed-pinned-write-fence");
    let dir = seed_store(&root, "storybook", "bedivere");
    let pinned = head_of(&dir);
    let lorestore = Lorestore::open(dir).expect("open store");
    lorestore
        .import_pack(&pack_with("storybook", &["bedivere", "perin"]))
        .expect("advance origin before the pinned workspace opens");
    write_manifest(
        &root,
        &format!("[documents.storybook]\nstore = \"store\"\nhead = \"{pinned}\"\n"),
    );

    let project = Arc::new(Project::load(&root).expect("pinned project loads"));
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
    let mut author = author_store_for(&project);
    let opened = send(
        &serde_json::json!({"dli":"2.0.0","id":"open","op":"author.open",
                            "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let workspace = opened["result"]["workspace"]
        .as_str()
        .expect("workspace id")
        .to_owned();

    // A separate authority advances the ref after the workspace captures its
    // live-origin snapshot, without changing the interner shape.
    let ref_after_open = lorestore
        .branch_ref(dli_lorestore::MAIN_REF)
        .expect("read live ref")
        .expect("live ref exists");
    let commit: dli_lorestore::Commit = lorestore
        .read_object(&ref_after_open.commit)
        .expect("read live commit")
        .expect("live commit exists");
    lorestore
        .commit(
            commit.interner,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            commit.decisions,
            commit.pack_name,
        )
        .expect("authority advances the origin after open");

    let staged = send(
        &serde_json::json!({"dli":"2.0.0","id":"stage","op":"author.add_fact",
                            "params":{"workspace":workspace,"entity":"bedivere",
                                      "key":"color","value":"red",
                                      "provenance":{"kind":"proposed","origin":"pinned-race"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(staged["ok"], true, "the workspace stages: {staged}");
    let materialized = send(
        &serde_json::json!({"dli":"2.0.0","id":"commit","op":"author.materialize",
                            "params":{"workspace":workspace}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        materialized["error"]["code"], "stale-fence",
        "the post-open origin advance fences materialize: {materialized}"
    );
}

#[test]
fn branch_mode_follows_the_tip_across_a_reload() {
    let root = temp("store-backed-branch-tip");
    let dir = seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");

    let before = Project::load(&root).expect("loads");
    assert_eq!(people(&before, "storybook"), vec!["bedivere".to_owned()]);
    let first_head = before
        .documents()
        .find(|(id, _)| *id == "storybook")
        .unwrap()
        .1
        .source_digest
        .clone();

    // A write lands on the branch — the shape a materialize leaves behind.
    let store = Lorestore::open(dir.clone()).expect("open store");
    store
        .import_pack(&pack_with("storybook", &["bedivere", "perin"]))
        .expect("second import advances the branch");
    drop(store);

    // Restart: a fresh load of the same manifest sees the new head.
    let after = Project::load(&root).expect("reloads");
    let second_head = after
        .documents()
        .find(|(id, _)| *id == "storybook")
        .unwrap()
        .1
        .source_digest
        .clone();

    assert_ne!(
        first_head, second_head,
        "an unpinned document follows the branch across a restart"
    );
    assert_eq!(second_head, head_of(&dir));
    assert!(
        people(&after, "storybook").contains(&"perin".to_owned()),
        "the reloaded world contains the newly committed work"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn bulk_decisions_survive_reopen_with_identical_why_and_canon() {
    use std::sync::Arc;

    let root = temp("store-backed-bulk-reopen-canon");
    let dir = seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");

    let project = Arc::new(Project::load(&root).expect("project loads"));
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
    let mut author = author_store_for(&project);
    let opened = send(
        &serde_json::json!({"dli":"2.0.0","id":"open","op":"author.open",
                            "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let workspace = opened["result"]["workspace"]
        .as_str()
        .expect("workspace id")
        .to_owned();
    let recorded = send(
        &serde_json::json!({"dli":"2.0.0","id":"bulk","op":"author.record_decisions",
                            "params":{"workspace":workspace,
                                      "records":[{"decision":"d-reopen"}]}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(recorded["ok"], true, "record succeeds: {recorded}");
    let added = send(
        &serde_json::json!({"dli":"2.0.0","id":"fact","op":"author.add_fact",
                            "params":{"workspace":workspace,
                                      "entity":"bedivere","key":"rank","value":"captain",
                                      "provenance":{"kind":"cited",
                                                    "decisions":["d-reopen"],
                                                    "state":"ratified"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(added["ok"], true, "cited fact stages: {added}");
    let live_why = send(
        &serde_json::json!({"dli":"2.0.0","id":"why-live","op":"author.why",
                            "params":{"workspace":workspace,
                                      "goal_atom":{"pred":"rank",
                                                   "args":[{"Sym":"bedivere"},
                                                           {"Sym":"captain"}]}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(live_why["ok"], true, "live why succeeds: {live_why}");
    let materialized = send(
        &serde_json::json!({"dli":"2.0.0","id":"materialize","op":"author.materialize",
                            "params":{"workspace":workspace,"rendered":false}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        materialized["ok"], true,
        "materialize succeeds: {materialized}"
    );
    drop(author);
    drop(sessions);
    drop(project);

    let reopened = |suffix: &str| -> (serde_json::Value, serde_json::Value) {
        let project = Arc::new(Project::load(&root).expect("project reopens"));
        let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
        let mut author = author_store_for(&project);
        let canon = send(
            &serde_json::json!({"dli":"2.0.0","id":format!("canon-{suffix}"),
                                "op":"dli.corpus.export",
                                "params":{"document":"storybook","format":"canon"}}),
            &mut sessions,
            &mut author,
        );
        assert_eq!(canon["ok"], true, "canon export succeeds: {canon}");
        let opened = send(
            &serde_json::json!({"dli":"2.0.0","id":format!("open-{suffix}"),
                                "op":"author.open","params":{"scope":"storybook"}}),
            &mut sessions,
            &mut author,
        );
        let workspace = opened["result"]["workspace"]
            .as_str()
            .expect("workspace id");
        let why = send(
            &serde_json::json!({"dli":"2.0.0","id":format!("why-{suffix}"),
                                "op":"author.why",
                                "params":{"workspace":workspace,
                                          "goal_atom":{"pred":"rank",
                                                       "args":[{"Sym":"bedivere"},
                                                               {"Sym":"captain"}]}}}),
            &mut sessions,
            &mut author,
        );
        assert_eq!(why["ok"], true, "reopened why succeeds: {why}");
        (canon["result"]["canon"].clone(), why["result"].clone())
    };

    let (first_canon, first_why) = reopened("first");
    let (second_canon, second_why) = reopened("second");
    assert_eq!(first_canon, second_canon, "canon is stable across reopen");
    assert_eq!(first_why, second_why, "why is stable across reopen");
    assert_eq!(
        live_why["result"]["supporting_decisions"], first_why["supporting_decisions"],
        "the reopened world preserves the bulk-recorded support"
    );
    assert!(
        first_why["supporting_decisions"]
            .as_array()
            .expect("supporting decisions")
            .contains(&serde_json::json!("d-reopen"))
    );
    let lorestore = Lorestore::open(dir).expect("reopen lorestore");
    let head = lorestore
        .branch_head(dli_lorestore::MAIN_REF)
        .expect("read head")
        .expect("head exists");
    assert!(
        lorestore
            .decisions_at(&head)
            .expect("read decisions")
            .contains(&"d-reopen".to_owned())
    );
}

#[test]
fn a_store_backed_document_reports_its_origin_for_attach() {
    let root = temp("store-backed-origin");
    let dir = seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");

    let project = Project::load(&root).expect("loads");
    let document = project
        .documents()
        .find(|(id, _)| *id == "storybook")
        .unwrap()
        .1;

    assert_eq!(
        document.store_origin.as_deref(),
        Some(fs::canonicalize(&dir).unwrap().as_path()),
        "the serve layer attaches this store, not an import copy"
    );
    assert!(!document.store_pinned, "no head was pinned");
}

#[test]
fn declaring_both_sources_and_store_is_refused() {
    let root = temp("store-and-sources");
    seed_store(&root, "storybook", "bedivere");
    fs::write(root.join("storybook.dli"), "scope storybook\n").unwrap();
    write_manifest(
        &root,
        "[documents.storybook]\nsources = [\"storybook.dli\"]\nstore = \"store\"\n",
    );

    let error = Project::load(&root).expect_err("both source forms is ambiguous");
    let message = error.to_string();
    assert!(
        message.contains("both `sources` and `store`"),
        "the refusal names the conflict: {message}"
    );
}

#[test]
fn declaring_neither_sources_nor_store_is_refused() {
    let root = temp("store-neither-form");
    write_manifest(&root, "[documents.storybook]\n");

    let error = Project::load(&root).expect_err("a document needs a source form");
    let message = error.to_string();
    assert!(
        message.contains("neither `sources` nor `store`"),
        "the refusal names what is missing: {message}"
    );
}

#[test]
fn a_head_without_a_store_is_refused() {
    let root = temp("store-head-without-store");
    fs::write(root.join("storybook.dli"), "scope storybook\n").unwrap();
    write_manifest(
        &root,
        "[documents.storybook]\nsources = [\"storybook.dli\"]\nhead = \"sha256:00\"\n",
    );

    let error = Project::load(&root).expect_err("a pin without a store means nothing");
    assert!(
        error.to_string().contains("`head` without `store`"),
        "the refusal explains why the pin is meaningless: {error}"
    );
}

#[test]
fn a_malformed_head_is_refused_by_name() {
    let root = temp("store-bad-head");
    seed_store(&root, "storybook", "bedivere");
    write_manifest(
        &root,
        "[documents.storybook]\nstore = \"store\"\nhead = \"not-a-hash\"\n",
    );

    let error = Project::load(&root).expect_err("a head must be a store commit hash");
    assert!(
        error.to_string().contains("not a `sha256:<hex>`"),
        "the refusal names the expected form: {error}"
    );
}

#[test]
fn a_store_outside_the_project_root_is_refused() {
    let root = temp("store-escape");
    seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"../elsewhere\"\n");

    let error = Project::load(&root).expect_err("a store path may not escape the project");
    let message = error.to_string();
    assert!(
        message.contains("store") || message.contains("escape") || message.contains("invalid"),
        "the refusal is about the path: {message}"
    );
}

// ── the consumer's five read ops, over a store-backed project ───────────────

/// Five core operations for reading lore. A store-backed
/// project must answer all of them on the **default build**, because that is
/// the whole capability the corpus pivot depends on: serve a corpus that was
/// never `.dli` text, from a build that cannot parse `.dli` text.
///
/// Each op is asserted to *answer* — `ok: true`, no typed error. Their verdicts
/// are pinned by their own contract fixtures; what is under test here is that
/// the store-backed source form reaches the same machinery.
#[test]
fn a_store_backed_project_answers_core_read_ops() {
    use std::sync::Arc;

    use dli_serve::author::store::Store as AuthorStore;
    use dli_serve::envelope::Response;
    use dli_serve::serve::respond;
    use dli_serve::session::Sessions;

    let root = temp("store-five-read-ops");
    seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");

    let project = Project::load(&root).expect("a store-backed project loads");
    let mut sessions = Sessions::from_project(Arc::new(project));
    let mut store = AuthorStore::default();

    let send =
        |frame: serde_json::Value, sessions: &mut Sessions, store: &mut AuthorStore| -> Response {
            let line = serde_json::to_string(&frame).unwrap();
            respond(&line, sessions, store).expect("a well-formed frame is answered")
        };

    // 1. dli.check — compile and check the document the store reconstructed.
    let checked = send(
        serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.check",
                           "params":{"document":"storybook"}}),
        &mut sessions,
        &mut store,
    );
    assert!(checked.ok, "dli.check answered: {:?}", checked.error);

    // 2. dli.query — the typed `goal_atom`; the rendered `goal` is a killed
    //    field at major >= 1, so the 1.1.0 caller sends the typed form.
    let queried = send(
        serde_json::json!({"dli":"2.0.0","id":"2","op":"dli.query",
                           "params":{"document":"storybook",
                                     "goal_atom":{"pred":"sworn",
                                                  "args":[{"Sym":"bedivere"}]}}}),
        &mut sessions,
        &mut store,
    );
    assert!(queried.ok, "dli.query answered: {:?}", queried.error);

    // 3. dli.name.check — the kernel half of the naming oracle.
    let vetted = send(
        serde_json::json!({"dli":"2.0.0","id":"3","op":"dli.name.check",
                           "params":{"name":"bedivere","dialect":"common"}}),
        &mut sessions,
        &mut store,
    );
    assert!(vetted.ok, "dli.name.check answered: {:?}", vetted.error);

    // 4. dli.export.run — the context-pack profile off the built-in registry.
    let exported = send(
        serde_json::json!({"dli":"2.0.0","id":"4","op":"dli.export.run",
                           "params":{"document":"storybook","profile":"context-pack"}}),
        &mut sessions,
        &mut store,
    );
    assert!(exported.ok, "dli.export.run answered: {:?}", exported.error);
}

// ── the corpus exchange, in both lanes (DM-3, DL-0056) ──────────────────────

/// Whether this build links the `.dli` front end.
const SYNTAX: bool = cfg!(feature = "syntax");

/// A store-backed project serving one seeded document, plus the wire driver.
fn corpus_server(
    name: &str,
) -> (
    std::path::PathBuf,
    dli_serve::session::Sessions,
    dli_serve::author::store::Store,
) {
    use std::sync::Arc;

    let root = temp(name);
    seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a store-backed project loads");
    (
        root,
        dli_serve::session::Sessions::from_project(Arc::new(project)),
        dli_serve::author::store::Store::default(),
    )
}

fn send(
    frame: &serde_json::Value,
    sessions: &mut dli_serve::session::Sessions,
    store: &mut dli_serve::author::store::Store,
) -> serde_json::Value {
    let line = serde_json::to_string(frame).unwrap();
    let response =
        dli_serve::serve::respond(&line, sessions, store).expect("a well-formed frame is answered");
    serde_json::to_value(&response).unwrap()
}

/// Construct the production author store for a freshly loaded project. Keeping
/// this beside the restart regressions makes the test exercise the same
/// store-origin and snapshot registration path as `deloreis serve`.
fn author_store_for(project: &Project) -> dli_serve::author::store::Store {
    let packs = project.documents().map(|(id, document)| {
        (
            id.to_owned(),
            document.pack.clone(),
            document.killed.clone(),
        )
    });
    let mut author = dli_serve::author::store::Store::with_packs(packs)
        .expect("the project compiles for authoring");
    for (id, document) in project.documents() {
        if let Some(origin) = document.store_origin.as_ref() {
            author
                .register_store_origin(
                    id.to_owned(),
                    origin.clone(),
                    document.store_pinned,
                    document.store_commit.clone().expect("store commit"),
                    document.store_valuation.clone(),
                    document.store_unassigned.clone(),
                    document.store_ref_digest.clone(),
                )
                .expect("store valuation maps by name");
        }
    }
    author
        .attach_lorestore(project.root().join(".dli-lorestore"))
        .expect("attach project lorestores");
    author
}

/// Asking for a `.dli` rendering from a build that links no renderer is a **loud
/// refusal naming the ways forward** — never a silent empty array, which would
/// read to the caller as "this corpus is empty" (DL-0056).
///
/// The JSON-first default is deliberately untouched: `format` absent or
/// `objects` returns the fact objects on either build, and only an *explicit*
/// request for a rendering can refuse.
#[test]
fn a_rendered_export_refuses_loudly_where_no_renderer_is_linked() {
    let (_root, mut sessions, mut store) = corpus_server("corpus-rendered-export");

    let rendered = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.export",
                            "params":{"document":"storybook","format":"rendered"}}),
        &mut sessions,
        &mut store,
    );
    if SYNTAX {
        let lines = rendered["result"]["rendered"].as_array().unwrap();
        assert!(
            !lines.is_empty(),
            "a syntax build renders the export: {rendered}"
        );
    } else {
        assert_eq!(
            rendered["error"]["code"], "not-wired",
            "a rendering from a renderer-free build refuses: {rendered}"
        );
        let message = rendered["error"]["message"].as_str().unwrap();
        assert!(
            message.contains("objects") && message.contains("syntax"),
            "the refusal names both ways forward, got: {message}"
        );
    }

    // The default is JSON-first on both builds, and stays a success.
    let objects = send(
        &serde_json::json!({"dli":"2.0.0","id":"2","op":"dli.corpus.export",
                            "params":{"document":"storybook"}}),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        objects["ok"], true,
        "the objects default answers: {objects}"
    );
    assert!(
        !objects["result"]["objects"].as_array().unwrap().is_empty(),
        "the export carries fact objects: {objects}"
    );
    assert!(
        objects["result"]["rendered"].is_null(),
        "no rendering is emitted unless it was asked for: {objects}"
    );
}

/// The canon projection is a 1.4.0 wire addition. Older negotiated versions
/// retain the released closed-enum schema refusal, while 1.4.0 exposes the
/// store-derived projection without changing the objects default.
#[test]
fn canon_export_is_available_on_contract_two() {
    let (_root, mut sessions, mut store) = corpus_server("corpus-canon-version");

    let old = send(
        &serde_json::json!({"dli":"1.3.0","id":"1","op":"dli.corpus.export",
                            "params":{"document":"storybook","format":"canon"}}),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        old["ok"], false,
        "1.3.0 keeps the canon schema refusal: {old}"
    );
    assert_eq!(old["error"]["code"], "killed-form", "{old}");
    assert!(
        old["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("2.0.0"),
        "the refusal names the released enum surface: {old}"
    );

    let current = send(
        &serde_json::json!({"dli":"2.0.0","id":"2","op":"dli.corpus.export",
                            "params":{"document":"storybook","format":"canon"}}),
        &mut sessions,
        &mut store,
    );
    assert_eq!(current["ok"], true, "1.4.0 serves canon export: {current}");
    assert!(
        current["result"]["canon"].is_object(),
        "the 1.4.0 response carries the canon projection: {current}"
    );
}

/// The corpus round-trip is lossless, and that is a stronger claim than
/// "the objects match": a stored fact records its support as `±<index>` into the
/// commit's interner, and its standing is read against the commit's decision
/// set. Carry only the objects and the same bytes re-import as a different
/// world — `decided d-1` becomes `proposed assumes __decl`. So the export emits
/// objects **and** interner **and** decisions, and importing the three
/// reproduces the commit hash itself.
#[test]
fn an_exported_corpus_reimports_to_the_same_commit() {
    let (_root, mut sessions, mut store) = corpus_server("corpus-roundtrip-source");
    let exported = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.export",
                            "params":{"document":"storybook"}}),
        &mut sessions,
        &mut store,
    );
    let result = &exported["result"];
    assert!(
        !result["interner"].as_array().unwrap().is_empty(),
        "the export carries the interner: {exported}"
    );

    // A second project whose store is seeded with the same pack, so the import
    // is an append onto an identical head rather than into a vacuum.
    let (_root2, mut sessions2, mut store2) = corpus_server("corpus-roundtrip-target");
    let imported = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.import",
                            "params":{"document":"storybook","mode":"append",
                                      "objects":result["objects"],
                                      "interner":result["interner"],
                                      "decisions":result["decisions"]}}),
        &mut sessions2,
        &mut store2,
    );
    assert_eq!(imported["ok"], true, "the import answers: {imported}");

    let reexported = send(
        &serde_json::json!({"dli":"2.0.0","id":"2","op":"dli.corpus.export",
                            "params":{"document":"storybook"}}),
        &mut sessions2,
        &mut store2,
    );
    assert_eq!(
        reexported["result"]["objects"], result["objects"],
        "the round trip is object-identical"
    );
    assert_eq!(
        reexported["result"]["interner"], result["interner"],
        "the round trip preserves the interner"
    );
    assert_eq!(
        reexported["result"]["decisions"], result["decisions"],
        "the round trip preserves the decision set"
    );
    // Not the commit hash: an append onto a populated store chains a child
    // commit, so its hash differs from the root commit it carries the same state
    // as. Commit-level identity is asserted where the target store can be
    // genuinely empty — `dli_lorestore::corpus_exchange`.
}

/// An `objects` import without the interner is refused rather than guessed at.
/// This is the seeded-divergence case for the round trip above: the refusal is
/// what stands between a caller and silently rewritten provenance.
#[test]
fn an_objects_import_without_its_interner_is_refused() {
    let (_root, mut sessions, mut store) = corpus_server("corpus-import-no-interner");
    let exported = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.export",
                            "params":{"document":"storybook"}}),
        &mut sessions,
        &mut store,
    );
    let refused = send(
        &serde_json::json!({"dli":"2.0.0","id":"2","op":"dli.corpus.import",
                            "params":{"document":"storybook","mode":"append",
                                      "objects":exported["result"]["objects"]}}),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        refused["error"]["code"], "schema-violation",
        "an objects import without its interner refuses: {refused}"
    );
}

/// The consumer's corpus pivot, rehearsed end to end.
///
/// A pack carrying both walls the consumer's read ops hit — a nameable relation
/// predicate for `dli.query`, an explicit dialect for `dli.name.check` — goes
/// into a store, is served from that store, and is exported. The exported
/// triple is then imported into a **genuinely empty** store, which is where the
/// commit hash itself can be compared: same objects, same interner, same
/// decisions, therefore the same commit. Anything dropped from the exchange
/// shows up here as a different hash.
#[test]
fn the_corpus_pivot_round_trips_to_the_same_commit() {
    use dli_ast::language::DialectBody;

    let root = temp("corpus-pivot-rehearsal");
    let mut pack = pack_with("storybook", &["bedivere"]);
    let decls = &mut pack.documents[0].decls;
    // A ratified decision, so the commit carries a decision set and an
    // interner entry — the two tables the exchange has to carry.
    decls.insert(
        0,
        Decl::Ratify(dli_ast::decl::RatifyDecl {
            decision: id("d-1"),
            body: dli_ast::language::RatifyBody::default(),
        }),
    );
    decls.push(Decl::Dialect(dli_ast::decl::DialectDecl {
        name: id("hedgerow"),
        label: None,
        body: DialectBody::default(),
        provenance: Some(proposed()),
    }));
    // One fact grounded in that decision: standing is what the decision set
    // decides, so dropping it changes what this fact means.
    decls.push(Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: id("keeps"),
            args: vec![Term::Sym(id("bedivere")), Term::Sym(id("hedgerow"))],
        },
        triple: false,
        provenance: Some(Annotation {
            marker: ProvenanceMarker::Decided,
            citations: vec![dli_ast::annot::CitationRef { id: id("d-1") }],
            condition: None,
            tags: Vec::new(),
            span: Span::synthetic(),
        }),
    }));

    let source_dir = root.join("source-store");
    fs::create_dir_all(&source_dir).unwrap();
    let source = Lorestore::open(source_dir.clone()).expect("open the source store");
    let head = source
        .ensure_pack_imported(&pack)
        .expect("seed the source store");

    // What `dli.corpus.export` emits: the three parts of the corpus.
    let hashes = source.fact_hashes_from(&head).expect("fold the facts");
    let objects: Vec<dli_lorestore::FactObject> = hashes
        .iter()
        .map(|hash| source.read_object(hash).unwrap().expect("object present"))
        .collect();
    let interner = source.interner_at(&head).expect("read the interner");
    let decisions = source.decisions_at(&head).expect("read the decisions");
    assert!(
        !interner.is_empty() && !decisions.is_empty(),
        "the seeded corpus exercises both carried tables"
    );

    // What `dli.corpus.import` does with them, into an empty store.
    let target_dir = root.join("target-store");
    fs::create_dir_all(&target_dir).unwrap();
    let target = Lorestore::open(target_dir).expect("open the target store");
    let written: Vec<dli_lorestore::ContentHash> = objects
        .iter()
        .map(|object| target.write_object(object).expect("write object"))
        .collect();
    let landed = target
        .commit(
            interner.clone(),
            written,
            Vec::new(),
            Vec::new(),
            decisions.clone(),
            "storybook".to_owned(),
        )
        .expect("import the corpus");

    assert_eq!(
        landed, head,
        "the exported corpus re-imports to the same commit"
    );
    assert_eq!(
        target.reconstruct_at(&landed).expect("reconstruct"),
        source.reconstruct_at(&head).expect("reconstruct"),
        "and reconstructs the same world"
    );

    // Dropping either carried table is the seeded divergence: the same objects
    // must NOT land on the same commit without them.
    let bare_dir = root.join("bare-store");
    fs::create_dir_all(&bare_dir).unwrap();
    let bare = Lorestore::open(bare_dir).expect("open the bare store");
    let bare_written: Vec<dli_lorestore::ContentHash> = objects
        .iter()
        .map(|object| bare.write_object(object).expect("write object"))
        .collect();
    let bare_landed = bare
        .commit(
            interner,
            bare_written,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            "storybook".to_owned(),
        )
        .expect("commit without the decisions");
    assert_ne!(
        bare_landed, head,
        "an import that drops the decision set must not claim the same commit"
    );
}

/// An append that drops the decision set must be refused, not committed.
///
/// Standing is read against the **tip** commit's decision set, so a commit that
/// lands with `decisions: []` demotes every fact already in the store — a
/// `decided d-1` fact reconstructs as `proposed`. Nothing the caller sent said
/// so; the omission alone did it. This is the same silent-demotion class the
/// interner guard closes, on the other carried table (DL-0056).
#[test]
fn an_append_that_drops_the_decision_set_is_refused() {
    let root = temp("corpus-append-drops-decisions");
    let mut pack = pack_with("storybook", &["bedivere"]);
    pack.documents[0].decls.insert(
        0,
        Decl::Ratify(dli_ast::decl::RatifyDecl {
            decision: id("d-1"),
            body: dli_ast::language::RatifyBody::default(),
        }),
    );
    pack.documents[0].decls.push(Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: id("keeps"),
            args: vec![Term::Sym(id("bedivere"))],
        },
        triple: false,
        provenance: Some(Annotation {
            marker: ProvenanceMarker::Decided,
            citations: vec![dli_ast::annot::CitationRef { id: id("d-1") }],
            condition: None,
            tags: Vec::new(),
            span: Span::synthetic(),
        }),
    }));

    let dir = root.join("store");
    fs::create_dir_all(&dir).unwrap();
    let store = Lorestore::open(dir.clone()).expect("open store");
    let head = store.ensure_pack_imported(&pack).expect("seed the store");
    let interner = store.interner_at(&head).expect("read interner");
    assert_eq!(
        store.decisions_at(&head).expect("read decisions"),
        vec!["d-1".to_owned()],
        "the seeded tip knows the decision"
    );
    let before = store.reconstruct_at(&head).expect("reconstruct");

    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a store-backed project loads");
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));
    let mut author = dli_serve::author::store::Store::default();

    // An empty append that carries the interner but omits `decisions`.
    let response = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.import",
                            "params":{"document":"storybook","mode":"append",
                                      "objects":[],"interner":interner}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        response["error"]["code"], "schema-violation",
        "an append that drops the decision set refuses: {response}"
    );

    // And the store is untouched: the refusal happened before any commit.
    let after_head = store
        .branch_head(dli_lorestore::MAIN_REF)
        .expect("read head")
        .expect("head present");
    assert_eq!(after_head, head, "a refused import commits nothing");
    assert_eq!(
        store.reconstruct_at(&after_head).expect("reconstruct"),
        before,
        "the committed lore keeps its standing"
    );
}

/// A `.dli` append that does not extend the store's name table is a caller's
/// mistake and must read as one.
///
/// The `objects` arm catches this by hand; this arm reaches it through the
/// store, which raises `InternerRegression` as an `internal` error — a
/// server-defect code for a caller-supplied payload. DL-0056 ruling 3 says a
/// caller mistake reads as a caller-facing refusal on both arms, so the
/// mapping is asserted here rather than left to the store's phrasing.
#[cfg(feature = "syntax")]
#[test]
fn a_dli_append_that_regresses_the_name_table_is_a_schema_violation() {
    let root = temp("corpus-dli-append-regression");
    seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a store-backed project loads");
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));
    let mut author = dli_serve::author::store::Store::default();

    // Only the new document, not the whole corpus: its compiled name table
    // starts from empty and so cannot extend the store's.
    let response = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.import",
            "params":{"document":"storybook","mode":"append","interner":[],
                      "dli_documents":[{"id":"extra",
                          "text":"scope storybook\nratify d-2\nperson perin ratified d-2\n"}]}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        response["error"]["code"], "schema-violation",
        "a regressing `.dli` append is a caller-facing refusal, not `internal`: {response}"
    );
    let message = response["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("whole corpus"),
        "the refusal names the way forward, got: {message}"
    );
}

/// The `.dli` twin of the decision-drop refusal.
///
/// `import_pack` derives the tip's decision set from the pack's own `ratify`
/// declarations, so re-sending the corpus minus one `ratify` line commits a
/// shrunken set — and every fact resting on that decision demotes. The interner
/// guard does not fire, because the dropped decision is still referenced by the
/// fact and so still in the name table. Same corruption, different arm: the
/// guard belongs at the commit choke point both arms funnel through.
#[cfg(feature = "syntax")]
#[test]
fn a_dli_append_that_drops_a_ratify_is_refused() {
    let root = temp("corpus-dli-append-drops-ratify");
    let dir = root.join("store");
    fs::create_dir_all(&dir).unwrap();
    let store = Lorestore::open(dir.clone()).expect("open store");

    let seed = "scope corpus\nratify d-1\nperson wren ratified d-1\n";
    let (seed_doc, diagnostics) = dli_syntax::parse_document(seed);
    assert!(diagnostics.iter().all(|d| !d.class.is_error()));
    let head = store
        .import_pack(&Pack {
            name: "corpus".to_owned(),
            documents: vec![seed_doc],
        })
        .expect("seed the store");
    let before = store.reconstruct_at(&head).expect("reconstruct");
    assert!(
        dli_syntax::format_document(&before.documents[0]).contains("ratified d-1"),
        "the seeded fact is decided"
    );

    write_manifest(&root, "[documents.corpus]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a store-backed project loads");
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));
    let mut author = dli_serve::author::store::Store::default();

    // The whole corpus again, minus the `ratify` line.
    let response = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.import",
            "params":{"document":"corpus","mode":"append",
                      "dli_documents":[{"id":"corpus",
                          "text":"scope corpus\nperson wren ratified d-1\n"}]}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        response["error"]["code"], "schema-violation",
        "dropping a ratify on append refuses: {response}"
    );

    let after_head = store
        .branch_head(dli_lorestore::MAIN_REF)
        .expect("read head")
        .expect("head present");
    assert_eq!(after_head, head, "a refused import commits nothing");
    assert_eq!(
        store.reconstruct_at(&after_head).expect("reconstruct"),
        before,
        "the committed lore keeps its standing"
    );
}

/// A document id is a wire identifier, deliberately decoupled from lore scope —
/// a store committed as `storybook` may legitimately be served as document `beta`.
/// So an append under a differing document id must PRESERVE the committed
/// scope, not rewrite the chain to match the wire.
///
/// Rewriting it would not change standing or the compiled world, but
/// `reconstruct` takes the scope and the positional-proposal bundle prefix
/// `h.<pack_name>#N` off the head commit, so every committed fact's *rendered*
/// provenance would come back under a name it was never written in — visible in
/// a rendered export and in structured-diff review.
#[test]
fn an_append_under_a_different_document_id_preserves_the_committed_scope() {
    let root = temp("corpus-append-preserves-scope");
    let dir = seed_store(&root, "storybook", "bedivere");
    let store = Lorestore::open(dir.clone()).expect("open store");
    let head = store
        .branch_head(dli_lorestore::MAIN_REF)
        .expect("read head")
        .expect("seeded head");
    let before = store.reconstruct_at(&head).expect("reconstruct");

    // The manifest serves that store under a *different* document id.
    write_manifest(&root, "[documents.beta]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a renamed document loads");
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));
    let mut author = dli_serve::author::store::Store::default();

    let exported = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.export",
                            "params":{"document":"beta"}}),
        &mut sessions,
        &mut author,
    );
    let imported = send(
        &serde_json::json!({"dli":"2.0.0","id":"2","op":"dli.corpus.import",
                            "params":{"document":"beta","mode":"append",
                                      "objects":exported["result"]["objects"],
                                      "interner":exported["result"]["interner"],
                                      "decisions":exported["result"]["decisions"]}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        imported["ok"], true,
        "a differing document id is a supported configuration, not an error: {imported}"
    );

    let after = store
        .branch_head(dli_lorestore::MAIN_REF)
        .expect("read head")
        .expect("head present");
    assert_ne!(after, head, "the append did commit");
    assert_eq!(
        store.head_pack_name().expect("read pack name"),
        Some("storybook".to_owned()),
        "the append kept the committed scope rather than taking the wire id"
    );
    assert_eq!(
        store.reconstruct_at(&after).expect("reconstruct"),
        before,
        "every committed fact renders exactly as it did before the append"
    );
}

/// The `.dli` arm's half of the same invariant. A differing *document id* is
/// supported, but a document declaring a different `scope` is a caller error
/// worth naming — silently overriding it would hide a real mistake.
#[cfg(feature = "syntax")]
#[test]
fn a_dli_append_declaring_a_different_scope_is_refused() {
    let root = temp("corpus-dli-append-foreign-scope");
    seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a store-backed project loads");
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));
    let mut author = dli_serve::author::store::Store::default();

    let response = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.import",
            "params":{"document":"storybook","mode":"append",
                      "dli_documents":[{"id":"other","text":"scope beta\nperson perin\n"}]}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        response["error"]["code"], "schema-violation",
        "a foreign scope on append refuses: {response}"
    );
    let message = response["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("storybook") && message.contains("beta"),
        "the refusal names both scopes, got: {message}"
    );
}

/// The authoring analog of the corpus test above: `author.materialize` on a
/// store-backed document served under a different id must succeed and preserve
/// the committed scope.
///
/// The workspace's base name is the document id the session opened under, and a
/// document id is decoupled from lore scope. Committing under the session's
/// name would rename the chain; refusing would make the one taught authoring
/// interface unusable for a configuration `Project::load` accepts. So it
/// preserves, exactly as `dli.corpus.import` does.
#[test]
fn materialize_under_a_different_document_id_preserves_the_committed_scope() {
    let root = temp("materialize-preserves-scope");
    let dir = seed_store(&root, "storybook", "bedivere");
    let store = Lorestore::open(dir.clone()).expect("open store");
    let head = store
        .branch_head(dli_lorestore::MAIN_REF)
        .expect("read head")
        .expect("seeded head");

    // Served under `beta`, committed as `storybook`.
    write_manifest(&root, "[documents.beta]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a renamed document loads");

    let mut author = author_store_for(&project);
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));

    let opened = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"author.open",
                            "params":{"scope":"beta"}}),
        &mut sessions,
        &mut author,
    );
    let workspace = opened["result"]["workspace"]
        .as_str()
        .unwrap_or_else(|| panic!("author.open answered with a workspace: {opened}"))
        .to_owned();

    let staged = send(
        &serde_json::json!({"dli":"2.0.0","id":"2","op":"author.add_entity",
            "params":{"workspace":workspace,"kind":"person","id":"perin",
                      "name":"Perin","provenance":{"kind":"proposed","origin":"session-scope-test"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(staged["ok"], true, "the entity stages: {staged}");

    let materialized = send(
        &serde_json::json!({"dli":"2.0.0","id":"3","op":"author.materialize",
                            "params":{"workspace":workspace}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        materialized["ok"], true,
        "materialize under a differing document id succeeds: {materialized}"
    );

    let staged_again = send(
        &serde_json::json!({"dli":"2.0.0","id":"4","op":"author.add_entity",
            "params":{"workspace":workspace,"kind":"person","id":"galahad",
                      "name":"Galahad","provenance":{"kind":"proposed","origin":"session-scope-test"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        staged_again["ok"], true,
        "the same workspace stages a second delta: {staged_again}"
    );
    let materialized_again = send(
        &serde_json::json!({"dli":"2.0.0","id":"5","op":"author.materialize",
                            "params":{"workspace":workspace}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        materialized_again["ok"], true,
        "a workspace advances its own ref fence after materialize: {materialized_again}"
    );

    let after = store
        .branch_head(dli_lorestore::MAIN_REF)
        .expect("read head")
        .expect("head present");
    assert_ne!(after, head, "materialize advanced the store it loaded from");
    assert_eq!(
        store.head_pack_name().expect("read pack name"),
        Some("storybook".to_owned()),
        "the commit kept the committed scope rather than the session's document id"
    );
    let world = store.reconstruct_at(&after).expect("reconstruct");
    assert_eq!(
        world.name, "storybook",
        "the reconstructed pack carries the committed scope"
    );
}

/// A branch ref is a single authority snapshot, not a commit pointer plus a
/// best-effort later valuation read. This drives the reported failure all the
/// way through the public tools: author a proposed fact, rule its persisted
/// ref externally, restart, and query through both read surfaces.
#[test]
#[allow(clippy::too_many_lines)]
fn reloaded_store_backed_queries_apply_the_selected_ref_valuation() {
    use std::sync::Arc;

    use dli_core::atom::Atom;
    use dli_lorestore::{Ref, branch_ref_name};

    let root = temp("store-ref-valuation-reload");
    let dir = seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");

    // The first worker materializes one proposal under a named authority gate.
    let initial = Project::load(&root).expect("initial project loads");
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::new(initial));
    let mut author = author_store_for(
        sessions
            .project_handle()
            .as_deref()
            .expect("project-backed sessions"),
    );
    let opened = send(
        &serde_json::json!({"dli":"2.0.0","id":"open","op":"author.open",
                            "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let workspace = opened["result"]["workspace"]
        .as_str()
        .expect("workspace id")
        .to_owned();
    let staged = send(
        &serde_json::json!({"dli":"2.0.0","id":"add","op":"author.add_fact",
                            "params":{"workspace":workspace,"entity":"bedivere",
                                      "key":"color","value":"red",
                                      "provenance":{"kind":"proposed","origin":"source-b"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(staged["ok"], true, "the proposed fact stages: {staged}");
    let materialized = send(
        &serde_json::json!({"dli":"2.0.0","id":"commit","op":"author.materialize",
                            "params":{"workspace":workspace}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        materialized["ok"], true,
        "the proposed fact materializes: {materialized}"
    );

    // An authority process writes both kinds of ruling. The false is deliberately
    // retained in the response: absence and false have different semantics.
    let lorestore = Lorestore::open(dir.clone()).expect("open authority store");
    let current = lorestore
        .branch_ref(dli_lorestore::MAIN_REF)
        .expect("read main ref")
        .expect("main ref exists");
    let commit: dli_lorestore::Commit = lorestore
        .read_object(&current.commit)
        .expect("read commit")
        .expect("main commit exists");
    let source_b = Atom(
        u32::try_from(
            commit
                .interner
                .iter()
                .position(|name| name == "h.source-b")
                .expect("materialized tag gate is interned"),
        )
        .expect("interner index fits an Atom"),
    );
    let superseded = Atom(
        u32::try_from(
            commit
                .interner
                .iter()
                .position(|name| name == "h.storybook#0")
                .expect("seeded proposal gate is interned"),
        )
        .expect("interner index fits an Atom"),
    );
    let mut valuation = current.valuation.clone();
    valuation.set(source_b, true);
    valuation.set(superseded, false);
    let expected = current.digest().expect("ref digest").0;
    let next = Ref::with_valuation(
        branch_ref_name(dli_lorestore::MAIN_REF),
        current.commit,
        valuation,
    );
    assert!(
        lorestore
            .cas_branch_ref(dli_lorestore::MAIN_REF, Some(&expected), &next)
            .expect("authority CAS"),
        "authority ruling commits"
    );

    // A new worker loads the ref once. Advance the live branch afterwards to
    // prove opening a workspace cannot splice that later head into the snapshot.
    let reloaded = Project::load(&root).expect("reloaded project loads");
    let selected_commit = reloaded
        .document("storybook")
        .unwrap()
        .source_digest
        .clone();
    lorestore
        .commit(
            commit.interner.clone(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            commit.decisions.clone(),
            commit.pack_name.clone(),
        )
        .expect("later external write advances the live branch");
    assert_ne!(
        head_of(&dir),
        selected_commit,
        "the live ref has moved after load"
    );

    let project = Arc::new(reloaded);
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
    let mut author = author_store_for(&project);
    let opened = send(
        &serde_json::json!({"dli":"2.0.0","id":"restart-open","op":"author.open",
                            "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let workspace = opened["result"]["workspace"]
        .as_str()
        .expect("restarted workspace id")
        .to_owned();
    let status = send(
        &serde_json::json!({"dli":"2.0.0","id":"status","op":"author.status",
                            "params":{"workspace":workspace}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        status["result"]["root_hash"], selected_commit,
        "the workspace pin is the project-selected commit, not the later head: {status}"
    );

    let goal = serde_json::json!({"pred":"color","args":[{"Sym":"bedivere"},{"Sym":"red"}]});
    let author_query = send(
        &serde_json::json!({"dli":"2.0.0","id":"author-query","op":"author.query",
                            "params":{"workspace":workspace,"goal_atom":goal,"layer":"all"}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        author_query["result"]["rows"][0]["status"], "certain",
        "author.query applies the persisted true ruling: {author_query}"
    );
    assert!(
        author_query["result"]["valuation"]
            .as_array()
            .expect("valuation array")
            .iter()
            .any(|entry| entry["atom"] == "h.source-b" && entry["value"] == true)
            && author_query["result"]["valuation"]
                .as_array()
                .expect("valuation array")
                .iter()
                .any(|entry| entry["atom"] == "h.storybook#0" && entry["value"] == false),
        "the complete persisted overlay, including false, is returned: {author_query}"
    );

    let stale_stage = send(
        &serde_json::json!({"dli":"2.0.0","id":"stale-stage","op":"author.add_fact",
                            "params":{"workspace":workspace,"entity":"bedivere",
                                      "key":"weight","value":"heavy",
                                      "provenance":{"kind":"proposed","origin":"source-c"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        stale_stage["ok"], true,
        "stale workspace may still stage: {stale_stage}"
    );
    let stale_materialize = send(
        &serde_json::json!({"dli":"2.0.0","id":"stale-commit","op":"author.materialize",
                            "params":{"workspace":workspace}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        stale_materialize["error"]["code"], "stale-fence",
        "materialize must not silently parent staged work on the later ref: {stale_materialize}"
    );

    let batch_query = send(
        &serde_json::json!({"dli":"2.0.0","id":"batch-query","op":"dli.query",
                            "params":{"document":"storybook","goal_atom":goal,"layer":"all"}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        batch_query["result"]["rows"][0]["status"], "certain",
        "dli.query shares the store-backed valuation path: {batch_query}"
    );
    assert_eq!(
        batch_query["result"]["valuation"], author_query["result"]["valuation"],
        "both query surfaces expose the effective stored valuation"
    );
}

/// The DM-4 exit scenario (contract 1.3.0, DL-0058): stage a dialect, the
/// morpheme it composes from, the name composed out of them, and a ban —
/// through the tools, on the **default build** — materialize, reload the
/// project from the store, and confirm the *language model* sees all four.
///
/// This is the property the whole milestone is for, and it is asserted here
/// rather than in the syntax lane on purpose. The consumer's authoring server
/// links no `.dli` front end: it cannot write the text form, cannot parse it
/// back, and has no formatter to render what it staged. If the language lane
/// only worked when a parser was present, the tools would be an interface the
/// consumer cannot use. So the round trip under test is tools → fact objects →
/// store → reload → compiled `LanguageModel`, with `.dli` text absent from
/// every step.
// A single end-to-end walk, asserted step by step: splitting it into helpers
// would hide the sequence that is the thing under test.
#[allow(clippy::too_many_lines)]
#[test]
fn the_expert_tools_reach_the_language_model_through_the_store() {
    let root = temp("expert-tools-language-model");
    let dir = seed_store(&root, "storybook", "bedivere");
    let before = head_of(&dir);
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a store-backed project loads");

    let mut author =
        dli_serve::author::store::Store::with_packs(project.documents().map(|(id, document)| {
            (
                id.to_owned(),
                document.pack.clone(),
                document.killed.clone(),
            )
        }))
        .expect("the project compiles");
    for (id, document) in project.documents() {
        if let Some(origin) = document.store_origin.as_ref() {
            author
                .register_store_origin(
                    id.to_owned(),
                    origin.clone(),
                    document.store_pinned,
                    document.store_commit.clone().expect("store commit"),
                    document.store_valuation.clone(),
                    document.store_unassigned.clone(),
                    document.store_ref_digest.clone(),
                )
                .expect("store valuation maps by name");
        }
    }
    author
        .attach_lorestore(root.join(".dli-lorestore"))
        .expect("attach");
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));

    let opened = send(
        &serde_json::json!({"dli":"2.0.0","id":"0","op":"author.open",
                            "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let ws = opened["result"]["workspace"]
        .as_str()
        .unwrap_or_else(|| panic!("author.open answered with a workspace: {opened}"))
        .to_owned();
    let provenance = serde_json::json!({"kind":"proposed","origin":"session-dm4"});

    for (id, frame) in [
        (
            "1",
            serde_json::json!({"workspace":ws,"id":"elder-tongue-fringe","label":"Elder-tongue fringe",
                "body":[{"Vowels":["a","e","y"]},{"Endings":["wyn"]},
                        {"NameKind":{"kind":"person.given","patterns":[["given-proto"]]}}],
                "provenance":provenance}),
        ),
        (
            "2",
            serde_json::json!({"workspace":ws,"id":"wyn","gloss":"fair",
                               "head":["role","given-proto"],"provenance":provenance}),
        ),
        (
            "3",
            serde_json::json!({"workspace":ws,"id":"gwenwyn","label":"Gwenwyn",
                "head":["person.given","in","elder-tongue-fringe"],
                "composition":[{"role":"given-proto","morpheme":"wyn"}],
                "provenance":provenance}),
        ),
        (
            "4",
            serde_json::json!({"workspace":ws,"word":"sword",
                               "reason":"no bladed weapons here","provenance":provenance}),
        ),
    ] {
        let op = match id {
            "1" => "author.add_dialect",
            "2" => "author.add_name_part",
            "3" => "author.add_name",
            _ => "author.add_banned",
        };
        let staged = send(
            &serde_json::json!({"dli":"2.0.0","id":id,"op":op,"params":frame}),
            &mut sessions,
            &mut author,
        );
        assert_eq!(
            staged["ok"], true,
            "{op} stages on the default build: {staged}"
        );
        // The default build renders nothing, so the reply's evidence that the
        // declaration was understood is its findings and its moved digest —
        // never a rendered line, which is exactly the point.
        if !SYNTAX {
            assert_eq!(
                staged["result"]["rendered"],
                serde_json::json!([]),
                "{op}: a build with no formatter renders no lines: {staged}"
            );
        }
    }

    let materialized = send(
        &serde_json::json!({"dli":"2.0.0","id":"5","op":"author.materialize",
                            "params":{"workspace":ws}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        materialized["ok"], true,
        "materialize commits the expert declarations: {materialized}"
    );
    let after = head_of(&dir);
    assert_ne!(after, before, "materialize advanced the store");

    // Reload from disk — a new process would see exactly this — and compile.
    let reloaded = Project::load(&root).expect("the advanced store reloads");
    let document = reloaded
        .documents()
        .find(|(id, _)| *id == "storybook")
        .expect("the document is still served")
        .1;
    let world = dli_world::compile(&document.pack).expect("the reloaded pack compiles");
    let language = &world.model.language;

    let register = language.register("elder-tongue-fringe").unwrap_or_else(|| {
        panic!(
            "the register survived the round trip: {:?}",
            language.registers
        )
    });
    assert!(
        register.phonotactics.endings.iter().any(|e| e == "wyn"),
        "the register kept its typed interior: {register:?}"
    );
    assert!(
        register
            .name_kinds
            .iter()
            .any(|kind| kind.kind == "person.given"),
        "the register kept its name-kind grammar: {register:?}"
    );
    assert!(
        language
            .resolve_morpheme("elder-tongue-fringe", "wyn", "given-proto")
            .is_some(),
        "the morpheme resolves for the register: {:?}",
        language.morphemes
    );
    assert!(
        language.names.iter().any(|n| n.name == "gwenwyn"),
        "the composed name compiled: {:?}",
        language.names
    );
    assert!(
        language.bans.iter().any(|b| b.word == "sword"),
        "the ban compiled: {:?}",
        language.bans
    );
}

// -- the scope-less corpus import (FW-6) -------------------------------------

/// A store-backed document whose lorestore is **freshly opened** — a genesis
/// commit and nothing else, which is the state a consumer seeding a new corpus
/// actually starts from.
#[cfg(feature = "syntax")]
fn empty_corpus_server(
    name: &str,
) -> (
    PathBuf,
    dli_serve::session::Sessions,
    dli_serve::author::store::Store,
) {
    use std::sync::Arc;
    let root = temp(name);
    // `Lorestore::open` writes the genesis commit; nothing is imported.
    dli_lorestore::Lorestore::open(root.join("store")).expect("open an empty store");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a genesis-only store-backed project loads");
    (
        root,
        dli_serve::session::Sessions::from_project(Arc::new(project)),
        dli_serve::author::store::Store::default(),
    )
}

/// **The FW-6 blocker.** `dli.corpus.import { dli_documents }` refused *every*
/// non-empty scope-less corpus with `LoweringDiverged` at index 0.
///
/// The lowering walk minted a proposal's bundle variable as `h.<pack>#0` while
/// the compiled model minted `h.#0`, because the walk fell back to the pack name
/// for a document with no `scope` header where the loader falls back to the
/// empty name. The store's faithfulness guard caught the disagreement and
/// refused the import — correctly; the guard was right and the walk was wrong.
///
/// It survived DM-3 because both committed corpus fixtures write a `scope`
/// header, where the two agree, and because the only scope-less case anyone had
/// exercised was the **empty** document, which mints no bundle at all and so
/// cannot diverge.
#[cfg(feature = "syntax")]
#[test]
fn a_scope_less_dli_corpus_imports() {
    let (_root, mut sessions, mut store) = empty_corpus_server("corpus-scope-less");
    let imported = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.import","params":{
            "document":"storybook","mode":"append",
            "dli_documents":[{"id":"storybook","text":"person joren \"Joren\" proposed\n"}]}}),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        imported["ok"], true,
        "a scope-less corpus must import: {imported}"
    );
    assert!(
        imported["result"]["imported_count"]
            .as_u64()
            .is_some_and(|n| n > 0),
        "the import commits the declaration: {imported}"
    );
}

/// The same import, round-tripped: `import { dli_documents }` → `export` must
/// return the objects, interner and decisions, and the **standing** must
/// survive. A `proposed` claim rests on an open bundle variable, so it reads
/// `possible`; if the bundle name round-tripped as a *tag* instead of a
/// positional bundle, the provenance would come back subtly different, which is
/// the failure mode the interner divergence was hiding.
#[cfg(feature = "syntax")]
#[test]
fn a_scope_less_corpus_round_trips_through_export() {
    let (root, mut sessions, mut store) = empty_corpus_server("corpus-scope-less-roundtrip");
    let text = "person joren \"Joren\" proposed\nperson nara \"Nara\" proposed\n";
    let imported = send(
        &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.import","params":{
            "document":"storybook","mode":"append",
            "dli_documents":[{"id":"storybook","text":text}]}}),
        &mut sessions,
        &mut store,
    );
    assert_eq!(imported["ok"], true, "import: {imported}");

    let exported = send(
        &serde_json::json!({"dli":"2.0.0","id":"2","op":"dli.corpus.export",
                            "params":{"document":"storybook"}}),
        &mut sessions,
        &mut store,
    );
    assert_eq!(exported["ok"], true, "export: {exported}");
    let result = &exported["result"];
    assert!(
        !result["objects"].as_array().unwrap().is_empty(),
        "the export carries the facts: {exported}"
    );
    // Both proposals minted a positional bundle, and they are distinct — one
    // shared name would collapse two independent proposals onto one gate.
    let interner: Vec<String> = result["interner"]
        .as_array()
        .expect("the export carries the interner")
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(
        interner.iter().any(|n| n == "h.#0") && interner.iter().any(|n| n == "h.#1"),
        "the scope-less bundles keep the loader's own naming: {interner:?}"
    );

    // The standing survives: recompile the served store and read it back.
    let reloaded = Project::load(&root).expect("the imported store reloads");
    let document = reloaded
        .documents()
        .find(|(id, _)| *id == "storybook")
        .expect("served")
        .1;
    let world = dli_world::compile(&document.pack).expect("the reconstructed pack compiles");
    let people = world
        .model
        .entities()
        .iter()
        .filter(|e| e.sort.as_deref() == Some("person"))
        .count();
    assert_eq!(people, 2, "both people survive the round trip");
    // A proposal rests on an open bundle, so it is possible-not-certain. If the
    // bundle had round-tripped as a tag or been dropped, this would read
    // `certain` and the corpus would have silently gained authority.
    let statuses: Vec<(String, dli_core::ann::Status)> = world
        .model
        .atoms()
        .iter()
        .map(|a| {
            (
                a.atom.pred.text.clone(),
                a.ann
                    .status(&world.model.world.valuation, &world.model.world.theory),
            )
        })
        .collect();
    assert!(
        !statuses.is_empty(),
        "the reconstructed world holds atoms to judge"
    );
    assert!(
        statuses
            .iter()
            .all(|(_, s)| *s != dli_core::ann::Status::Certain),
        "a proposed corpus must not reconstruct as certain: {statuses:?}"
    );
}

/// `mode: init` was unreachable over the wire, and the fix took two attempts —
/// both recorded, because the one that failed is the more instructive.
///
/// Opening a lorestore writes a genesis commit, so `branch_head` is `Some` for
/// every store the server can serve, and the original guard tested exactly that
/// — leaving `append` as the only way to seed a corpus, which for a non-Rust
/// consumer is no way at all.
///
/// The first replacement was the committed **pack name**, on the reasoning that
/// genesis is unnamed so a name means a corpus. That predicate was wrong, and
/// dangerously so: once a seeding commit records the document's *scope*
/// (DL-0059 ruling 2), a scope-less corpus records the **empty** name, which
/// reads as unnamed — so a populated store would answer "not seeded" and `init`
/// would import straight over it. This test caught that when the ruling-2 change
/// landed.
///
/// The predicate is therefore `all_fact_hashes().is_empty()`: a store is empty
/// when it holds no facts, which is the question `init` is actually asking, and
/// the only one of the three that no naming or commit-shape change can falsify.
#[cfg(feature = "syntax")]
#[test]
fn init_seeds_a_genesis_only_store_and_still_refuses_a_seeded_one() {
    let (_root, mut sessions, mut store) = empty_corpus_server("corpus-init-genesis");
    let frame = |id: &str| {
        serde_json::json!({"dli":"2.0.0","id":id,"op":"dli.corpus.import","params":{
            "document":"storybook","mode":"init",
            "dli_documents":[{"id":"storybook","text":"person joren \"Joren\" proposed\n"}]}})
    };

    let seeded = send(&frame("1"), &mut sessions, &mut store);
    assert_eq!(
        seeded["ok"], true,
        "init must seed a store that holds no corpus yet: {seeded}"
    );

    // And the guard still bites once a corpus is actually there.
    let again = send(&frame("2"), &mut sessions, &mut store);
    assert_eq!(
        again["ok"], false,
        "init into a seeded store refuses: {again}"
    );
    assert_eq!(again["error"]["code"], "schema-violation");
    assert!(
        again["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("already holds a corpus")),
        "the refusal names the reason: {again}"
    );
}

/// **The store's round-trip invariant: a corpus renders back as the document
/// that was authored** — no clause invented, no clause dropped, no scope header
/// the author did not write.
///
/// The comparison is against the *canonical formatting* of the authored text
/// rather than its literal bytes, because the store round-trips through the one
/// formatter by design (koan 7) and that formatter aligns provenance into a
/// column: `keeps(the-king, camelot)   proposed` is the canonical spelling of
/// `keeps(the-king, camelot) proposed`. Comparing to raw input would fail on
/// whitespace the engine is entitled to normalize and say nothing about
/// provenance, which is what is actually under test. This is table-driven over the shapes that differ *structurally*
/// inside the store, because the bug this pins was invisible to every shape but
/// one.
///
/// What it caught, and why the earlier suite did not: `reconstruct_pack` derives
/// both the proposal-bundle prefix (`h.<name>#N`) and the scope header it
/// re-emits from the commit's recorded **pack name**, while the lowering walk and
/// the compiled model mint that bundle under the document's **scope**. Those two
/// agree only when the committed name happens to equal the scope — which is
/// exactly the one shape every committed fixture uses. Everywhere else the
/// prefix missed, the bundle fell through to the *tag* branch, and `proposed`
/// came back as `proposed #0` under an invented scope: fabricated provenance on
/// the served world and on the consumer-facing rendered export.
///
/// Two blind spots stacked. A *carried* declaration (one with a label, so
/// `carriage_kind` is `Some`) replays from its stored carriage JSON and never
/// reaches the polynomial-to-annotation path at all, so it cannot show the
/// defect — and a carried entity was what the first round-trip test used. Hence
/// the bare Fact/Attr/Entity rows below: they are the ones that go through the
/// reconstruction that was wrong.
#[cfg(feature = "syntax")]
#[test]
fn an_imported_dli_corpus_renders_back_as_the_document_that_was_authored() {
    // (case, document id, authored text). The expectation is the authored text
    // put through the canonical formatter — see the note above.
    let cases: &[(&str, &str, &str)] = &[
        // Bare declarations: `carriage_kind` is `None`, so these reconstruct
        // from the support polynomial — the path that fabricated the tag.
        (
            "bare fact",
            "storybook",
            "keeps(the-lake, excalibur) proposed\n",
        ),
        (
            "bare attr",
            "storybook",
            "home(joren, riverside) proposed\n",
        ),
        // A carried declaration, kept as the control: it replays from carriage
        // and so must be unaffected either way.
        (
            "carried entity",
            "storybook",
            "person joren \"Joren\" proposed\n",
        ),
        // Scoped, and scoped under a document id that differs from the scope —
        // a configuration DM-3 explicitly supports, and the shape that proved
        // the defect was never really about scope-less-ness.
        (
            "scoped, id == scope",
            "storybook",
            "scope storybook\n\nkeeps(the-lake, excalibur) proposed\n",
        ),
        (
            "scoped, id != scope",
            "beta",
            "scope storybook\n\nkeeps(the-lake, excalibur) proposed\n",
        ),
        // Several proposals: one bundle would not catch a counter that failed
        // to advance, or two bundles collapsing onto one gate.
        (
            "two bare proposals",
            "storybook",
            "keeps(the-lake, excalibur) proposed\nkeeps(the-king, camelot) proposed\n",
        ),
        // A named origin tag is a real tag — it must survive as one, which is
        // the negative control for the invented-tag bug.
        (
            "explicit origin tag",
            "storybook",
            "keeps(the-lake, excalibur) proposed session-7\n",
        ),
    ];

    for (case, document, authored) in cases {
        let root = temp(&format!(
            "roundtrip-{}",
            case.replace(|c: char| !c.is_alphanumeric(), "-")
        ));
        dli_lorestore::Lorestore::open(root.join("store")).expect("open an empty store");
        write_manifest(
            &root,
            &format!("[documents.{document}]\nstore = \"store\"\n"),
        );

        let load = |root: &Path| {
            let project = Project::load(root).expect("the store-backed project loads");
            (
                dli_serve::session::Sessions::from_project(std::sync::Arc::new(project)),
                dli_serve::author::store::Store::default(),
            )
        };

        let (mut sessions, mut store) = load(&root);
        let imported = send(
            &serde_json::json!({"dli":"2.0.0","id":"1","op":"dli.corpus.import","params":{
                "document": document, "mode":"append",
                "dli_documents":[{"id": document, "text": authored}]}}),
            &mut sessions,
            &mut store,
        );
        assert_eq!(imported["ok"], true, "{case}: import: {imported}");

        // Reload from disk — a fresh process sees exactly this — then render.
        let (mut sessions, mut store) = load(&root);
        let exported = send(
            &serde_json::json!({"dli":"2.0.0","id":"2","op":"dli.corpus.export","params":{
                "document": document, "format":"rendered"}}),
            &mut sessions,
            &mut store,
        );
        assert_eq!(exported["ok"], true, "{case}: export: {exported}");
        let rendered = exported["result"]["rendered"][0]
            .as_str()
            .unwrap_or_else(|| panic!("{case}: rendered export: {exported}"));
        let (parsed, diagnostics) = dli_syntax::parse_document(authored);
        assert!(
            diagnostics.iter().all(|d| !d.class.is_error()),
            "{case}: the authored text parses: {diagnostics:?}"
        );
        let canonical = dli_syntax::format_document(&parsed);
        assert_eq!(
            rendered, canonical,
            "{case}: the corpus did not render back as it was authored"
        );
    }
}

#[test]
fn new_workspace_reads_materialized_main_without_restarting_server() {
    use std::sync::Arc;

    let root = temp("fresh-workspace-after-materialize");
    seed_store(&root, "camelot", "bedivere");
    write_manifest(&root, "[documents.camelot]\nstore = \"store\"\n");
    let project = Arc::new(Project::load(&root).unwrap());
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
    let mut author = author_store_for(&project);
    let open = |id: &str, sessions: &mut _, author: &mut _| {
        let result = send(
            &serde_json::json!({"dli":"2.0.0","id":id,"op":"author.open",
                                "params":{"scope":"camelot"}}),
            sessions,
            author,
        );
        (
            result["result"]["workspace"].as_str().unwrap().to_owned(),
            result["result"]["workspace_digest"]
                .as_str()
                .unwrap()
                .to_owned(),
        )
    };
    let (writer, _) = open("writer", &mut sessions, &mut author);
    let (old, old_digest) = open("old", &mut sessions, &mut author);
    let staged = send(
        &serde_json::json!({"dli":"2.0.0","id":"add","op":"author.add_fact",
            "params":{"workspace":writer,"entity":"bedivere","key":"color","value":"red",
                      "provenance":{"kind":"proposed","origin":"fresh-workspace"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(staged["ok"], true, "{staged}");
    let committed = send(
        &serde_json::json!({"dli":"2.0.0","id":"commit","op":"author.materialize",
                            "params":{"workspace":writer}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(committed["ok"], true, "{committed}");
    let status = send(
        &serde_json::json!({"dli":"2.0.0","id":"status","op":"author.status",
                            "params":{"workspace":writer}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        committed["result"]["workspace_digest"],
        status["result"]["workspace_digest"]
    );
    let (fresh, fresh_digest) = open("fresh", &mut sessions, &mut author);
    assert_ne!(
        old_digest, fresh_digest,
        "the base commit must affect an empty workspace fence"
    );
    let stale = send(
        &serde_json::json!({"dli":"2.0.0","id":"stale","op":"author.add_fact",
            "expected_digest":old_digest,
            "params":{"workspace":fresh,"entity":"bedivere","key":"color","value":"blue",
                      "provenance":{"kind":"proposed","origin":"stale-check"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(stale["error"]["code"], "stale-fence", "{stale}");
    let query = |workspace: &str, sessions: &mut _, author: &mut _| {
        send(
            &serde_json::json!({"dli":"2.0.0","id":"query","op":"author.query",
                "params":{"workspace":workspace,"goal_atom":{"pred":"color",
                          "args":[{"Sym":"bedivere"},{"Sym":"red"}]},"layer":"all"}}),
            sessions,
            author,
        )
    };
    let result = query(&fresh, &mut sessions, &mut author);
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(
        result["result"]["rows"].as_array().unwrap().len(),
        1,
        "{result}"
    );
    let committed_id = committed["result"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| fact["atom"]["pred"] == "color")
        .unwrap()["claim_id"]
        .as_str()
        .unwrap();
    assert_eq!(
        result["result"]["rows"][0]["claim_ids"],
        serde_json::json!([committed_id])
    );
    let old_result = query(&old, &mut sessions, &mut author);
    assert_eq!(old_result["ok"], true, "{old_result}");
    assert!(
        old_result["result"]["rows"].as_array().unwrap().is_empty(),
        "{old_result}"
    );
}

#[test]
fn naming_repair_uses_workspace_dialect_before_and_after_materialize() {
    use std::sync::Arc;
    let root = temp("workspace-dialect-repair");
    seed_store(&root, "storybook", "nara");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Arc::new(Project::load(&root).unwrap());
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
    let mut author = author_store_for(&project);
    let open = |sessions: &mut _, author: &mut _| {
        let result = send(
            &serde_json::json!({"dli":"2.0.0","id":"open","op":"author.open",
            "params":{"scope":"storybook"}}),
            sessions,
            author,
        );
        result["result"]["workspace"].as_str().unwrap().to_owned()
    };
    let first = open(&mut sessions, &mut author);
    let dialect = send(
        &serde_json::json!({"dli":"2.0.0","id":"dialect","op":"author.add_dialect",
        "params":{"workspace":first,"id":"river-speech","provenance":{"kind":"proposed","origin":"test"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(dialect["ok"], true, "{dialect}");
    let check_repair = |ws: &str, name: &str, sessions: &mut _, author: &mut _| {
        let entity = send(
            &serde_json::json!({"dli":"2.0.0","id":"entity","op":"author.add_entity",
            "params":{"workspace":ws,"id":name,"name":name,"kind":"person",
                "provenance":{"kind":"proposed","origin":"test"}}}),
            sessions,
            author,
        );
        let warning = entity["result"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["code"] == "name.unvetted")
            .unwrap();
        assert_eq!(warning["repair"]["params"]["workspace"], ws);
        assert_eq!(warning["repair"]["params"]["dialect"], "river-speech");
        let checked = send(
            &serde_json::json!({"dli":"2.0.0","id":"vet","op":"dli.name.check",
            "params":warning["repair"]["params"]}),
            sessions,
            author,
        );
        assert_eq!(checked["ok"], true, "{checked}");
        assert!(
            !checked
                .to_string()
                .contains("the named register is not declared"),
            "{checked}"
        );
    };
    check_repair(&first, "marin", &mut sessions, &mut author);
    let committed = send(
        &serde_json::json!({"dli":"2.0.0","id":"commit","op":"author.materialize",
        "params":{"workspace":first}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(committed["ok"], true, "{committed}");
    let fresh = open(&mut sessions, &mut author);
    check_repair(&fresh, "sable", &mut sessions, &mut author);
}

#[test]
fn query_claim_ids_deduplicate_committed_sources_and_omit_staged_only() {
    use std::sync::Arc;
    let root = temp("query-committed-claim-ids");
    seed_store(&root, "storybook", "nara");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Arc::new(Project::load(&root).unwrap());
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
    let mut author = author_store_for(&project);
    let opened = send(
        &serde_json::json!({"dli":"2.0.0","id":"open","op":"author.open",
        "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let ws = opened["result"]["workspace"].as_str().unwrap();
    for origin in ["account-one", "account-two"] {
        let added = send(
            &serde_json::json!({"dli":"2.0.0","id":"add","op":"author.add_fact",
            "params":{"workspace":ws,"entity":"nara","key":"color","value":"red",
                "provenance":{"kind":"proposed","origin":origin}}}),
            &mut sessions,
            &mut author,
        );
        assert_eq!(added["ok"], true, "{added}");
    }
    let query = |sessions: &mut _, author: &mut _| {
        send(
            &serde_json::json!({"dli":"2.0.0","id":"query","op":"author.query",
        "params":{"workspace":ws,"goal_atom":{"pred":"color","args":[{"Sym":"nara"},{"Sym":"red"}]}}}),
            sessions,
            author,
        )
    };
    let staged = query(&mut sessions, &mut author);
    assert!(
        staged["result"]["rows"][0].get("claim_ids").is_none(),
        "{staged}"
    );
    let committed = send(
        &serde_json::json!({"dli":"2.0.0","id":"commit","op":"author.materialize",
        "params":{"workspace":ws}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(committed["ok"], true, "{committed}");
    let mut expected: Vec<_> = committed["result"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fact| fact["atom"]["pred"] == "color")
        .map(|fact| fact["claim_id"].as_str().unwrap().to_owned())
        .collect();
    expected.sort();
    expected.dedup();
    assert_eq!(expected.len(), 1, "the shared claim ID is deduplicated");
    let read = query(&mut sessions, &mut author);
    assert_eq!(
        read["result"]["rows"][0]["claim_ids"],
        serde_json::json!(expected)
    );
    assert_eq!(
        query(&mut sessions, &mut author)["result"]["rows"],
        read["result"]["rows"]
    );
}

#[test]
fn conflicting_proposals_warn_and_survive_materialization_and_restart() {
    use std::sync::Arc;
    let root = temp("pending-conflict-persistence");
    seed_store(&root, "storybook", "nara");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Arc::new(Project::load(&root).unwrap());
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
    let mut author = author_store_for(&project);
    let opened = send(
        &serde_json::json!({"dli":"2.0.0", "id":"open", "op":"author.open",
        "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let ws = opened["result"]["workspace"].as_str().unwrap();
    let declared = send(
        &serde_json::json!({"dli":"2.0.0", "id":"functional", "op":"author.declare_functional",
        "params":{"workspace":ws,"pred":"rank","provenance":{"kind":"proposed","origin":"test"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(declared["ok"], true, "{declared}");
    for value in ["captain", "admiral"] {
        let staged = send(
            &serde_json::json!({"dli":"2.0.0", "id":"add", "op":"author.add_fact",
            "params":{"workspace":ws,"entity":"nara","key":"rank","value":value,
            "provenance":{"kind":"proposed","origin":"test"}}}),
            &mut sessions,
            &mut author,
        );
        assert_eq!(staged["ok"], true, "{staged}");
    }
    let checked = send(
        &serde_json::json!({"dli":"2.0.0", "id":"check", "op":"author.check",
        "params":{"workspace":ws}}),
        &mut sessions,
        &mut author,
    );
    assert!(
        checked["result"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["code"] == "continuity.pending-conflict" && f["severity"] == "warn"),
        "{checked}"
    );
    let committed = send(
        &serde_json::json!({"dli":"2.0.0", "id":"commit", "op":"author.materialize",
        "params":{"workspace":ws}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(committed["ok"], true, "{committed}");
    drop(author);
    drop(sessions);
    drop(project);
    let reopened = Project::load(&root).unwrap();
    let doc = reopened.document("storybook").unwrap();
    let world = dli_world::compile(&doc.pack).unwrap();
    assert!(
        dli_world::check::run(&world)
            .iter()
            .any(|f| f.code == "continuity.pending-conflict")
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn defeat_selects_the_ruled_support_when_a_proposal_shares_its_claim_id() {
    use std::sync::Arc;
    let root = temp("claim-with-multiple-supports");
    seed_store(&root, "storybook", "nara");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Arc::new(Project::load(&root).unwrap());
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
    let mut author = author_store_for(&project);
    let opened = send(
        &serde_json::json!({"dli":"2.0.0","id":"open","op":"author.open",
        "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let ws = opened["result"]["workspace"].as_str().unwrap();
    let proposed = send(
        &serde_json::json!({"dli":"2.0.0","id":"propose","op":"author.add_fact",
        "params":{"workspace":ws,"entity":"nara","key":"rank","value":"captain",
            "provenance":{"kind":"proposed","origin":"witness"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(proposed["ok"], true, "{proposed}");
    let first = send(
        &serde_json::json!({"dli":"2.0.0","id":"first","op":"author.materialize",
        "params":{"workspace":ws}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(first["ok"], true, "{first}");
    let recorded = send(
        &serde_json::json!({"dli":"2.0.0","id":"record","op":"author.record_decision",
        "params":{"workspace":ws,"decision":"d-0001"}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(recorded["ok"], true, "{recorded}");
    let ruled = send(
        &serde_json::json!({"dli":"2.0.0","id":"rule","op":"author.add_fact",
        "params":{"workspace":ws,"entity":"nara","key":"rank","value":"captain",
            "provenance":{"kind":"cited","decisions":["d-0001"],"state":"provisional"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(ruled["ok"], true, "{ruled}");
    let second = send(
        &serde_json::json!({"dli":"2.0.0","id":"second","op":"author.materialize",
        "params":{"workspace":ws}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(second["ok"], true, "{second}");
    let claim = second["result"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["atom"]["pred"] == "rank")
        .unwrap()["claim_id"]
        .as_str()
        .unwrap();
    let promoted = send(
        &serde_json::json!({"dli":"2.0.0","id":"promote","op":"author.set_standing",
        "params":{"workspace":ws,"claim_id":claim,"decision":"d-0001"}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(promoted["ok"], true, "{promoted}");
    let persisted = Lorestore::open(root.join("store")).unwrap();
    let head = persisted.root_hash().unwrap().unwrap();
    assert!(
        persisted
            .fact_hashes_from(&head)
            .unwrap()
            .iter()
            .any(|hash| {
                let fact: dli_lorestore::FactObject = persisted.read_object(hash).unwrap().unwrap();
                fact.claim_id.0 == claim
                    && fact.provenance.as_ref().and_then(|p| p.marker)
                        == Some(ProvenanceMarker::Ratified)
            }),
        "the matching provisional support was promoted"
    );
    let defeated = send(
        &serde_json::json!({"dli":"2.0.0","id":"defeat","op":"author.defeat",
        "params":{"workspace":ws,"claim_id":claim,"decision":"d-0002","defeated_decision":"d-0001"}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(defeated["ok"], true, "{defeated}");
    let why = send(
        &serde_json::json!({"dli":"2.0.0","id":"why","op":"author.why",
        "params":{"workspace":ws,"goal_atom":{"pred":"rank","args":[{"Sym":"nara"},{"Sym":"captain"}]}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(why["result"]["why_not"]["status"], "possible", "{why}");
    assert!(
        why["result"]["why"]["rendered"]
            .to_string()
            .contains("witness"),
        "{why}"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn promotion_after_defeat_preserves_attack_history_and_reopens() {
    use std::sync::Arc;
    let root = temp("claim-defeat-before-promotion");
    seed_store(&root, "storybook", "nara");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Arc::new(Project::load(&root).unwrap());
    let mut sessions = dli_serve::session::Sessions::from_project(Arc::clone(&project));
    let mut author = author_store_for(&project);
    let opened = send(
        &serde_json::json!({"dli":"2.0.0","id":"open","op":"author.open",
        "params":{"scope":"storybook"}}),
        &mut sessions,
        &mut author,
    );
    let ws = opened["result"]["workspace"].as_str().unwrap();
    let proposed = send(
        &serde_json::json!({"dli":"2.0.0","id":"propose","op":"author.add_fact",
        "params":{"workspace":ws,"entity":"nara","key":"rank","value":"captain",
            "provenance":{"kind":"proposed","origin":"witness"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(proposed["ok"], true, "{proposed}");
    let first = send(
        &serde_json::json!({"dli":"2.0.0","id":"first","op":"author.materialize",
        "params":{"workspace":ws}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(first["ok"], true, "{first}");
    let recorded = send(
        &serde_json::json!({"dli":"2.0.0","id":"record","op":"author.record_decision",
        "params":{"workspace":ws,"decision":"d-0001"}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(recorded["ok"], true, "{recorded}");
    let ruled = send(
        &serde_json::json!({"dli":"2.0.0","id":"rule","op":"author.add_fact",
        "params":{"workspace":ws,"entity":"nara","key":"rank","value":"captain",
            "provenance":{"kind":"cited","decisions":["d-0001"],"state":"provisional"}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(ruled["ok"], true, "{ruled}");
    let second = send(
        &serde_json::json!({"dli":"2.0.0","id":"second","op":"author.materialize",
        "params":{"workspace":ws}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(second["ok"], true, "{second}");
    let claim = second["result"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["atom"]["pred"] == "rank")
        .unwrap()["claim_id"]
        .as_str()
        .unwrap();
    let defeated = send(
        &serde_json::json!({"dli":"2.0.0","id":"defeat","op":"author.defeat",
        "params":{"workspace":ws,"claim_id":claim,"decision":"d-0002","defeated_decision":"d-0001"}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(defeated["ok"], true, "{defeated}");
    let promoted = send(
        &serde_json::json!({"dli":"2.0.0","id":"promote","op":"author.set_standing",
        "params":{"workspace":ws,"claim_id":claim,"decision":"d-0001"}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(promoted["ok"], true, "{promoted}");
    let persisted = Lorestore::open(root.join("store")).unwrap();
    let head = persisted.root_hash().unwrap().unwrap();
    assert!(
        persisted
            .fact_hashes_from(&head)
            .unwrap()
            .iter()
            .any(|hash| {
                let fact: dli_lorestore::FactObject = persisted.read_object(hash).unwrap().unwrap();
                fact.claim_id.0 == claim
                    && fact.provenance.as_ref().and_then(|p| p.marker)
                        == Some(ProvenanceMarker::Ratified)
            }),
        "the matching provisional support was promoted"
    );

    let why = send(
        &serde_json::json!({"dli":"2.0.0","id":"why","op":"author.why",
        "params":{"workspace":ws,"goal_atom":{"pred":"rank","args":[{"Sym":"nara"},{"Sym":"captain"}]}}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(why["result"]["why_not"]["status"], "possible", "{why}");
    assert!(
        why["result"]["why"]["rendered"]
            .to_string()
            .contains("witness"),
        "{why}"
    );
    let exported = send(
        &serde_json::json!({"dli":"2.0.0","id":"export","op":"dli.corpus.export",
        "params":{"document":"storybook","format":"objects","at":promoted["result"]["root_hash"]}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(exported["ok"], true, "{exported}");
    let imported_dir = tempfile::tempdir().unwrap();
    let imported = Lorestore::open(imported_dir.path().to_path_buf()).unwrap();
    let object_hashes: Vec<_> = exported["result"]["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            let object: dli_lorestore::FactObject = serde_json::from_value(value.clone()).unwrap();
            imported.write_object(&object).unwrap()
        })
        .collect();
    let attack_hashes: Vec<_> = exported["result"]["attacks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            let attack: dli_lorestore::DecisionAttack =
                serde_json::from_value(value.clone()).unwrap();
            imported.write_object(&attack).unwrap()
        })
        .collect();
    let names: Vec<String> =
        serde_json::from_value(exported["result"]["interner"].clone()).unwrap();
    let decisions: Vec<String> =
        serde_json::from_value(exported["result"]["decisions"].clone()).unwrap();
    let imported_head = imported
        .commit_with_attacks(
            names,
            object_hashes,
            Vec::new(),
            Vec::new(),
            attack_hashes,
            decisions,
            "storybook".to_owned(),
        )
        .unwrap();
    assert_eq!(
        imported
            .decision_attacks_from(&imported_head)
            .unwrap()
            .len(),
        1
    );
    drop(author);
    drop(sessions);
    drop(project);
    let reopened = Project::load(&root).expect("attack history survives reopening");
    assert!(reopened.document("storybook").is_some());
}

#[test]
fn question_options_and_their_support_survive_materialize_and_reopen() {
    use serde_json::json;
    let root = temp("durable-question-options");
    seed_store(&root, "storybook", "bedivere");
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Project::load(&root).unwrap();
    let mut author = author_store_for(&project);
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));
    let mut call = |op: &str, params: serde_json::Value| {
        send(
            &json!({"dli":"2.0.0","id":"test","op":op,"params":params}),
            &mut sessions,
            &mut author,
        )
    };
    let opened = call("author.open", json!({"scope":"storybook"}));
    let ws = opened["result"]["workspace"].as_str().unwrap();
    assert_eq!(
        call(
            "author.open_question",
            json!({"workspace":ws,"id":"mystery","prompt":"What happened?"})
        )["ok"],
        true
    );
    for (id, value) in [("witness-one", "drowned"), ("witness-two", "poisoned")] {
        let r = call(
            "author.add_option",
            json!({"workspace":ws,"question":"mystery","id":id,"label":id,
            "implies":{"pred":"death","args":[{"Sym":"bedivere"},{"Sym":value}]}}),
        );
        assert_eq!(r["ok"], true, "{r}");
    }
    let other = call(
        "author.open_question",
        json!({"workspace":ws,"id":"other","prompt":"Another question"}),
    );
    assert_eq!(other["ok"], true);
    let collision = call(
        "author.add_option",
        json!({"workspace":ws,"question":"other","id":"witness-one","label":"other"}),
    );
    assert_eq!(collision["ok"], false, "{collision}");
    let bad = call(
        "author.add_option",
        json!({"workspace":ws,"question":"mystery","id":"bad","label":"bad",
        "implies":{"pred":"death","args":[{"Var":"x"}]}}),
    );
    assert_eq!(bad["ok"], false);
    let committed = call("author.materialize", json!({"workspace":ws}));
    assert_eq!(committed["ok"], true, "{committed}");
    let added = call(
        "author.add_option",
        json!({"workspace":ws,"question":"mystery","id":"witness-three","label":"witness-three",
        "implies":{"pred":"death","args":[{"Sym":"bedivere"},{"Sym":"illness"}]}}),
    );
    assert_eq!(added["ok"], false, "{added}");
    let reopened = call("author.open", json!({"scope":"storybook"}));
    let ws2 = reopened["result"]["workspace"].as_str().unwrap();
    for (id, value) in [("witness-one", "drowned"), ("witness-two", "poisoned")] {
        let atom = json!({"pred":"death","args":[{"Sym":"bedivere"},{"Sym":value}]});
        let result = call(
            "author.query",
            json!({"workspace":ws2,"goal_atom":atom,"layer":"all"}),
        );
        assert_eq!(
            result["result"]["rows"][0]["status"], "possible",
            "{result}"
        );
        let why = call("author.why", json!({"workspace":ws2,"goal_atom":atom}));
        assert!(
            why["result"]["why"]["rendered"].to_string().contains(id),
            "{why}"
        );
    }
}

#[test]
fn fork_hypothesis_cannot_reuse_a_committed_option_name() {
    use serde_json::json;
    let root = temp("option-hypothesis-collision");
    let dir = root.join("store");
    fs::create_dir_all(&dir).unwrap();
    let mut pack = pack_with("storybook", &["bedivere"]);
    pack.documents[0]
        .decls
        .push(Decl::Fork(dli_ast::decl::ForkDecl {
            question: id("fork1"),
            options: vec![],
            body: vec![],
            provenance: None,
        }));
    Lorestore::open(dir)
        .unwrap()
        .ensure_pack_imported(&pack)
        .unwrap();
    write_manifest(&root, "[documents.storybook]\nstore = \"store\"\n");
    let project = Project::load(&root).unwrap();
    let mut author = author_store_for(&project);
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));
    let mut call = |op: &str, params: serde_json::Value| {
        send(
            &json!({"dli":"2.0.0","id":"test","op":op,"params":params}),
            &mut sessions,
            &mut author,
        )
    };
    let opened = call("author.open", json!({"scope":"storybook"}));
    let ws = opened["result"]["workspace"].as_str().unwrap();
    assert_eq!(
        call(
            "author.open_question",
            json!({"workspace":ws,"id":"q1","prompt":"Q"})
        )["ok"],
        true
    );
    assert_eq!(
        call(
            "author.add_option",
            json!({"workspace":ws,"question":"q1","id":"shared","label":"O",
        "implies":{"pred":"death","args":[{"Sym":"bedivere"}]}})
        )["ok"],
        true
    );
    assert_eq!(
        call("author.materialize", json!({"workspace":ws}))["ok"],
        true
    );
    let fork = call("author.open", json!({"fork":"fork1"}));
    let fws = fork["result"]["workspace"].as_str().unwrap();
    let collision = call(
        "author.add_hypothesis",
        json!({"workspace":fws,"id":"shared","statement":"unrelated"}),
    );
    assert_eq!(collision["ok"], false, "{collision}");
}
