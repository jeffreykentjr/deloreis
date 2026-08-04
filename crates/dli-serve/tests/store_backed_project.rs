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
    seed_store(&root, "tpth", "bedivere");
    write_manifest(&root, "[documents.tpth]\nstore = \"store\"\n");
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
        &serde_json::json!({"dli":"1.2.0","id":"1","op":"dli.corpus.export",
                            "params":{"document":"tpth","format":"rendered"}}),
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
        &serde_json::json!({"dli":"1.2.0","id":"2","op":"dli.corpus.export",
                            "params":{"document":"tpth"}}),
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
        &serde_json::json!({"dli":"1.2.0","id":"1","op":"dli.corpus.export",
                            "params":{"document":"tpth"}}),
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
        &serde_json::json!({"dli":"1.2.0","id":"1","op":"dli.corpus.import",
                            "params":{"document":"tpth","mode":"append",
                                      "objects":result["objects"],
                                      "interner":result["interner"],
                                      "decisions":result["decisions"]}}),
        &mut sessions2,
        &mut store2,
    );
    assert_eq!(imported["ok"], true, "the import answers: {imported}");

    let reexported = send(
        &serde_json::json!({"dli":"1.2.0","id":"2","op":"dli.corpus.export",
                            "params":{"document":"tpth"}}),
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
        &serde_json::json!({"dli":"1.2.0","id":"1","op":"dli.corpus.export",
                            "params":{"document":"tpth"}}),
        &mut sessions,
        &mut store,
    );
    let refused = send(
        &serde_json::json!({"dli":"1.2.0","id":"2","op":"dli.corpus.import",
                            "params":{"document":"tpth","mode":"append",
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
    let mut pack = pack_with("tpth", &["bedivere"]);
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
        name: id("hedgeway"),
        label: None,
        body: DialectBody::default(),
        provenance: Some(proposed()),
    }));
    // One fact grounded in that decision: standing is what the decision set
    // decides, so dropping it changes what this fact means.
    decls.push(Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: id("keeps"),
            args: vec![Term::Sym(id("bedivere")), Term::Sym(id("hedgeway"))],
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
            "tpth".to_owned(),
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
            "tpth".to_owned(),
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
    let mut pack = pack_with("tpth", &["bedivere"]);
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

    write_manifest(&root, "[documents.tpth]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a store-backed project loads");
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));
    let mut author = dli_serve::author::store::Store::default();

    // An empty append that carries the interner but omits `decisions`.
    let response = send(
        &serde_json::json!({"dli":"1.2.0","id":"1","op":"dli.corpus.import",
                            "params":{"document":"tpth","mode":"append",
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
    seed_store(&root, "tpth", "bedivere");
    write_manifest(&root, "[documents.tpth]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a store-backed project loads");
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));
    let mut author = dli_serve::author::store::Store::default();

    // Only the new document, not the whole corpus: its compiled name table
    // starts from empty and so cannot extend the store's.
    let response = send(
        &serde_json::json!({"dli":"1.2.0","id":"1","op":"dli.corpus.import",
            "params":{"document":"tpth","mode":"append","interner":[],
                      "dli_documents":[{"id":"extra",
                          "text":"scope tpth\nratify d-2\nperson kes decided d-2\n"}]}}),
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

    let seed = "scope corpus\nratify d-1\nperson wren decided d-1\n";
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
        dli_syntax::format_document(&before.documents[0]).contains("decided d-1"),
        "the seeded fact is decided"
    );

    write_manifest(&root, "[documents.corpus]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a store-backed project loads");
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));
    let mut author = dli_serve::author::store::Store::default();

    // The whole corpus again, minus the `ratify` line.
    let response = send(
        &serde_json::json!({"dli":"1.2.0","id":"1","op":"dli.corpus.import",
            "params":{"document":"corpus","mode":"append",
                      "dli_documents":[{"id":"corpus",
                          "text":"scope corpus\nperson wren decided d-1\n"}]}}),
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
/// a store committed as `tpth` may legitimately be served as document `beta`.
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
    let dir = seed_store(&root, "tpth", "bedivere");
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
        &serde_json::json!({"dli":"1.2.0","id":"1","op":"dli.corpus.export",
                            "params":{"document":"beta"}}),
        &mut sessions,
        &mut author,
    );
    let imported = send(
        &serde_json::json!({"dli":"1.2.0","id":"2","op":"dli.corpus.import",
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
        Some("tpth".to_owned()),
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
    seed_store(&root, "tpth", "bedivere");
    write_manifest(&root, "[documents.tpth]\nstore = \"store\"\n");
    let project = Project::load(&root).expect("a store-backed project loads");
    let mut sessions = dli_serve::session::Sessions::from_project(std::sync::Arc::new(project));
    let mut author = dli_serve::author::store::Store::default();

    let response = send(
        &serde_json::json!({"dli":"1.2.0","id":"1","op":"dli.corpus.import",
            "params":{"document":"tpth","mode":"append",
                      "dli_documents":[{"id":"other","text":"scope beta\nperson kes\n"}]}}),
        &mut sessions,
        &mut author,
    );
    assert_eq!(
        response["error"]["code"], "schema-violation",
        "a foreign scope on append refuses: {response}"
    );
    let message = response["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("tpth") && message.contains("beta"),
        "the refusal names both scopes, got: {message}"
    );
}
