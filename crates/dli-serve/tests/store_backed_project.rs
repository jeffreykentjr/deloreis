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
    seed_store(&root, "tpth", "bedivere");
    write_manifest(&root, "[documents.tpth]\nstore = \"store\"\n");

    let project = Project::load(&root).expect("a store-backed project loads");
    assert_eq!(people(&project, "tpth"), vec!["bedivere".to_owned()]);
}

#[test]
fn the_fingerprint_is_the_store_head_not_a_workspace_digest() {
    let root = temp("store-backed-fingerprint");
    let dir = seed_store(&root, "tpth", "bedivere");
    write_manifest(&root, "[documents.tpth]\nstore = \"store\"\n");

    let project = Project::load(&root).expect("loads");
    let document = project.documents().find(|(id, _)| *id == "tpth").unwrap().1;

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
    let dir = seed_store(&root, "tpth", "bedivere");
    let pinned = head_of(&dir);

    // Advance the branch past the pin.
    let store = Lorestore::open(dir.clone()).expect("open store");
    store
        .import_pack(&pack_with("tpth", &["bedivere", "kes"]))
        .expect("second import advances the branch");
    assert_ne!(head_of(&dir), pinned, "the branch moved");

    write_manifest(
        &root,
        &format!("[documents.tpth]\nstore = \"store\"\nhead = \"{pinned}\"\n"),
    );
    let project = Project::load(&root).expect("a pinned project loads");

    let document = project.documents().find(|(id, _)| *id == "tpth").unwrap().1;
    assert_eq!(
        document.source_digest, pinned,
        "a pinned document serves the commit the manifest names, not the tip"
    );
    assert!(document.store_pinned, "the document reports itself pinned");
    assert_eq!(
        people(&project, "tpth"),
        vec!["bedivere".to_owned()],
        "the world is the pinned commit's, without the later import"
    );
}

#[test]
fn branch_mode_follows_the_tip_across_a_reload() {
    let root = temp("store-backed-branch-tip");
    let dir = seed_store(&root, "tpth", "bedivere");
    write_manifest(&root, "[documents.tpth]\nstore = \"store\"\n");

    let before = Project::load(&root).expect("loads");
    assert_eq!(people(&before, "tpth"), vec!["bedivere".to_owned()]);
    let first_head = before
        .documents()
        .find(|(id, _)| *id == "tpth")
        .unwrap()
        .1
        .source_digest
        .clone();

    // A write lands on the branch — the shape a materialize leaves behind.
    let store = Lorestore::open(dir.clone()).expect("open store");
    store
        .import_pack(&pack_with("tpth", &["bedivere", "kes"]))
        .expect("second import advances the branch");
    drop(store);

    // Restart: a fresh load of the same manifest sees the new head.
    let after = Project::load(&root).expect("reloads");
    let second_head = after
        .documents()
        .find(|(id, _)| *id == "tpth")
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
        people(&after, "tpth").contains(&"kes".to_owned()),
        "the reloaded world contains the newly committed work"
    );
}

#[test]
fn a_store_backed_document_reports_its_origin_for_attach() {
    let root = temp("store-backed-origin");
    let dir = seed_store(&root, "tpth", "bedivere");
    write_manifest(&root, "[documents.tpth]\nstore = \"store\"\n");

    let project = Project::load(&root).expect("loads");
    let document = project.documents().find(|(id, _)| *id == "tpth").unwrap().1;

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
    seed_store(&root, "tpth", "bedivere");
    fs::write(root.join("tpth.dli"), "scope tpth\n").unwrap();
    write_manifest(
        &root,
        "[documents.tpth]\nsources = [\"tpth.dli\"]\nstore = \"store\"\n",
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
    write_manifest(&root, "[documents.tpth]\n");

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
    fs::write(root.join("tpth.dli"), "scope tpth\n").unwrap();
    write_manifest(
        &root,
        "[documents.tpth]\nsources = [\"tpth.dli\"]\nhead = \"sha256:00\"\n",
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
    seed_store(&root, "tpth", "bedivere");
    write_manifest(
        &root,
        "[documents.tpth]\nstore = \"store\"\nhead = \"not-a-hash\"\n",
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
    seed_store(&root, "tpth", "bedivere");
    write_manifest(&root, "[documents.tpth]\nstore = \"../elsewhere\"\n");

    let error = Project::load(&root).expect_err("a store path may not escape the project");
    let message = error.to_string();
    assert!(
        message.contains("store") || message.contains("escape") || message.contains("invalid"),
        "the refusal is about the path: {message}"
    );
}

// ── the consumer's five read ops, over a store-backed project ───────────────

/// The five operations the fantasy factory reads lore through. A store-backed
/// project must answer all of them on the **default build**, because that is
/// the whole capability the corpus pivot depends on: serve a corpus that was
/// never `.dli` text, from a build that cannot parse `.dli` text.
///
/// Each op is asserted to *answer* — `ok: true`, no typed error. Their verdicts
/// are pinned by their own contract fixtures; what is under test here is that
/// the store-backed source form reaches the same machinery.
#[test]
fn a_store_backed_project_answers_the_five_consumer_read_ops() {
    use std::sync::Arc;

    use dli_serve::author::store::Store as AuthorStore;
    use dli_serve::envelope::Response;
    use dli_serve::serve::respond;
    use dli_serve::session::Sessions;

    let root = temp("store-five-read-ops");
    seed_store(&root, "tpth", "bedivere");
    write_manifest(&root, "[documents.tpth]\nstore = \"store\"\n");

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
        serde_json::json!({"dli":"1.1.0","id":"1","op":"dli.check",
                           "params":{"document":"tpth"}}),
        &mut sessions,
        &mut store,
    );
    assert!(checked.ok, "dli.check answered: {:?}", checked.error);

    // 2. dli.query — the typed `goal_atom`; the rendered `goal` is a killed
    //    field at major >= 1, so the 1.1.0 caller sends the typed form.
    let queried = send(
        serde_json::json!({"dli":"1.1.0","id":"2","op":"dli.query",
                           "params":{"document":"tpth",
                                     "goal_atom":{"pred":"sworn",
                                                  "args":[{"Sym":"bedivere"}]}}}),
        &mut sessions,
        &mut store,
    );
    assert!(queried.ok, "dli.query answered: {:?}", queried.error);

    // 3. dli.name.check — the kernel half of the naming oracle.
    let vetted = send(
        serde_json::json!({"dli":"1.1.0","id":"3","op":"dli.name.check",
                           "params":{"name":"bedivere","dialect":"common"}}),
        &mut sessions,
        &mut store,
    );
    assert!(vetted.ok, "dli.name.check answered: {:?}", vetted.error);

    // 4. dli.export.run — the context-pack profile off the built-in registry.
    let exported = send(
        serde_json::json!({"dli":"1.1.0","id":"4","op":"dli.export.run",
                           "params":{"document":"tpth","profile":"context-pack"}}),
        &mut sessions,
        &mut store,
    );
    assert!(exported.ok, "dli.export.run answered: {:?}", exported.error);

    // 5. dli.check-draft — the full battery over a minimal `payload/v1` draft.
    let drafted = send(
        serde_json::json!({"dli":"1.1.0","id":"5","op":"dli.check-draft",
                           "params":{"document":"tpth","source":"baseline",
                                     "payload":{
                                       "meta":{"episode":"episode-1",
                                               "source":{"file":"draft.md",
                                                         "sha256":"8ba8496a2525ae171ffd104d632dede6ef418d9b95962a9d88e2fcdbc8d48d24",
                                                         "words":1},
                                               "extracted":"2026-07-25",
                                               "extractors":{"schema":"payload/v1"}},
                                       "claims":[],"deltas":[],"scenes":[],"beats":[],
                                       "promises":{"opened":[],"closed":[]},
                                       "proof_ledgers":[],
                                       "names":{"introduced":[]},
                                       "lexicon":{"banned_hits":[],"register_tokens":0}}}}),
        &mut sessions,
        &mut store,
    );
    assert!(drafted.ok, "dli.check-draft answered: {:?}", drafted.error);
}
