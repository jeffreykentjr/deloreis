#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Caller-project loading and isolation through the real worker loop.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use dli_serve::project::Project;
use dli_serve::serve::serve_project_io;
use serde_json::Value;

fn norse() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/norse")
}

fn temp(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    fs::remove_dir_all(&root).ok();
    fs::create_dir_all(&root).unwrap();
    root
}

fn write_manifest(root: &Path, body: &str) {
    fs::write(
        root.join("deloreis.toml"),
        format!("version = 1\nname = \"test-project\"\n{body}"),
    )
    .unwrap();
}

#[test]
fn loads_the_second_world_as_one_logical_document() {
    let project = Project::load(norse()).expect("Norse project loads");
    assert_eq!(project.name(), "nine-realms");
    let document = project.document("norse").expect("logical document");
    assert_eq!(document.pack.documents.len(), 3);
    assert!(project.document("camelot").is_none());
}

#[test]
fn project_worker_resolves_declared_ids_and_never_demo_falls_back() {
    let project = Project::load(norse()).expect("Norse project loads");
    let input = b"\
{\"dli\":\"0.1.0\",\"id\":\"1\",\"op\":\"dli.query\",\"params\":{\"document\":\"norse\",\"goal\":\"adjacent(asgard, midgard)\",\"layer\":\"certain\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"2\",\"op\":\"dli.check\",\"params\":{\"document\":\"camelot\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"3\",\"op\":\"author.open\",\"params\":{\"scope\":\"norse\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"4\",\"op\":\"author.query\",\"params\":{\"workspace\":\"ws-1\",\"goal\":\"adjacent(asgard, midgard)\",\"layer\":\"certain\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"5\",\"op\":\"author.open\",\"params\":{\"scope\":\"camelot\"}}\n";
    let mut output = Vec::new();
    serve_project_io(&mut Cursor::new(input), &mut output, project).expect("worker completes");
    let responses: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    assert_eq!(responses.len(), 5);
    assert_eq!(responses[0]["ok"], true);
    assert_ne!(responses[0]["result"]["support"], false);
    assert_eq!(responses[1]["error"]["code"], "unknown-doc");
    assert_eq!(responses[2]["result"]["workspace"], "ws-1");
    assert_eq!(
        responses[3]["result"]["rows"][0]["atom"],
        "adjacent(asgard, midgard)"
    );
    assert_eq!(responses[4]["error"]["code"], "unknown-doc");
}

#[test]
fn project_door_t_rejects_invented_forks() {
    let project = Project::load(norse()).expect("Norse project loads");
    let input = b"\
{\"dli\":\"0.1.0\",\"id\":\"1\",\"op\":\"author.open\",\"params\":{\"fork\":\"what-does-wisdom-cost\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"2\",\"op\":\"author.open\",\"params\":{\"fork\":\"invented-fork\"}}\n";
    let mut output = Vec::new();
    serve_project_io(&mut Cursor::new(input), &mut output, project).expect("worker completes");
    let responses: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    assert_eq!(responses[0]["ok"], true);
    assert_eq!(responses[1]["error"]["code"], "unknown-doc");
}

#[test]
fn project_stateful_handles_and_workspaces_cannot_cross_documents() {
    let root = temp("project-two-document-isolation");
    fs::write(
        root.join("one.dli"),
        "scope one\nratify d-1\nplace alpha \"Alpha\" decided d-1\nmarked(alpha) decided d-1\n",
    )
    .unwrap();
    fs::write(
        root.join("two.dli"),
        "scope two\nratify d-2\nplace beta \"Beta\" decided d-2\nmarked(beta) decided d-2\n",
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.one]\nsources = [\"one.dli\"]\n\
         [documents.two]\nsources = [\"two.dli\"]\n",
    );
    let project = Project::load(&root).expect("two-document project loads");
    let input = b"\
{\"dli\":\"0.1.0\",\"id\":\"1\",\"op\":\"dli.fork.new\",\"params\":{\"document\":\"one\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"2\",\"op\":\"dli.query\",\"params\":{\"document\":\"two\",\"goal\":\"marked(beta)\",\"fork\":\"fork-1\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"3\",\"op\":\"dli.fork.new\",\"params\":{\"document\":\"two\",\"base\":\"fork-1\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"4\",\"op\":\"dli.fork.list\",\"params\":{\"document\":\"missing\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"5\",\"op\":\"author.open\",\"params\":{\"scope\":\"one\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"6\",\"op\":\"author.open\",\"params\":{\"scope\":\"two\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"7\",\"op\":\"author.query\",\"params\":{\"workspace\":\"ws-1\",\"goal\":\"marked(?)\",\"layer\":\"certain\"}}\n\
{\"dli\":\"0.1.0\",\"id\":\"8\",\"op\":\"author.query\",\"params\":{\"workspace\":\"ws-2\",\"goal\":\"marked(?)\",\"layer\":\"certain\"}}\n";
    let mut output = Vec::new();
    serve_project_io(&mut Cursor::new(input), &mut output, project).expect("worker completes");
    let responses: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    assert_eq!(responses[0]["ok"], true);
    assert_eq!(responses[1]["error"]["code"], "schema-violation");
    assert_eq!(responses[2]["error"]["code"], "schema-violation");
    assert_eq!(responses[3]["error"]["code"], "unknown-doc");
    assert_eq!(responses[6]["result"]["rows"][0]["atom"], "marked(alpha)");
    assert_eq!(responses[7]["result"]["rows"][0]["atom"], "marked(beta)");
}

#[test]
fn rejects_unknown_manifest_fields_and_parent_traversal() {
    let root = temp("project-invalid-manifest");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    fs::write(
        root.join("deloreis.toml"),
        "version = 1\nname = \"bad\"\nsurprise = true\n\
         [documents.world]\nsources = [\"world.dli\"]\n",
    )
    .unwrap();
    assert!(Project::load(&root).is_err(), "unknown fields fail closed");

    fs::write(
        root.join("deloreis.toml"),
        "version = 1\nname = \"bad\"\n\
         [documents.world]\nsources = [\"../outside.dli\"]\n",
    )
    .unwrap();
    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("contain no `..`"), "{error}");
}

#[test]
fn rejects_ambiguous_ids_duplicate_sources_and_duplicate_forks() {
    let root = temp("project-ambiguous");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();

    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\", \"world.dli\"]\n",
    );
    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("same source more than once"), "{error}");

    write_manifest(
        &root,
        "[documents.one]\nsources = [\"world.dli\"]\n\
         [documents.two]\nsources = [\"world.dli\"]\n",
    );
    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("owned by both `one` and `two`"), "{error}");

    fs::write(
        root.join("one.dli"),
        "scope one\nfork shared-question\n  option first\n",
    )
    .unwrap();
    fs::write(
        root.join("two.dli"),
        "scope two\nfork shared-question\n  option second\n",
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.one]\nsources = [\"one.dli\"]\n\
         [documents.two]\nsources = [\"two.dli\"]\n",
    );
    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("fork `shared-question`"), "{error}");

    fs::write(
        root.join("one.dli"),
        "scope one\ndialect common \"Common\" proposed {\n  vowels a\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("two.dli"),
        "scope two\ndialect common \"Common\" proposed {\n  vowels e\n}\n",
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.one]\nsources = [\"one.dli\"]\n\
         [documents.two]\nsources = [\"two.dli\"]\n",
    );
    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("dialect `common`"), "{error}");
    assert!(error.contains("project-wide"), "{error}");

    fs::write(root.join("rules.json"), "{}").unwrap();
    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\"]\n\
         [games.world]\nrules = \"rules.json\"\n",
    );
    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("both a document and a game"), "{error}");
}

#[test]
fn rejects_unsupported_versions_bad_killed_modes_and_overlapping_storage() {
    let root = temp("project-invalid-assets");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    fs::write(
        root.join("deloreis.toml"),
        "version = 2\nname = \"future\"\n\
         [documents.world]\nsources = [\"world.dli\"]\n",
    )
    .unwrap();
    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("version 2 is unsupported"), "{error}");

    fs::write(
        root.join("killed.yaml"),
        "killed:\n  - {token: old-name, match: regex, decision: d-1}\n",
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\"]\n\
         killed_tokens = \"killed.yaml\"\n",
    );
    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("unknown variant `regex`"), "{error}");

    write_manifest(
        &root,
        "[paths]\nevidence = \".state\"\ncache = \".state/cache\"\n\
         [documents.world]\nsources = [\"world.dli\"]\n",
    );
    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("non-overlapping"), "{error}");
}

#[test]
fn loads_optional_project_assets_without_demo_resolution() {
    let root = temp("project-optional-assets");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    fs::write(
        root.join("killed.yaml"),
        "killed:\n  - {token: old-name, match: word, decision: d-0099}\n",
    )
    .unwrap();
    fs::copy(
        repo.join("worlds/tpth/episodes/saltmarket-episodes.yaml"),
        root.join("episodes.yaml"),
    )
    .unwrap();
    fs::copy(
        repo.join("examples/norse/games/rune-wheel/rules.json"),
        root.join("rules.json"),
    )
    .unwrap();
    fs::copy(
        repo.join("examples/norse/games/rune-wheel/profiles.json"),
        root.join("game-profiles.json"),
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\"]\n\
         episodes = \"episodes.yaml\"\n\
         killed_tokens = \"killed.yaml\"\n\
         [games.rune-wheel]\nrules = \"rules.json\"\nprofiles = \"game-profiles.json\"\n",
    );

    let project = Project::load(&root).expect("all optional assets load");
    let document = project.document("world").unwrap();
    assert!(!document.episodes.as_ref().unwrap().is_empty());
    assert!(document.killed.hit("the Old Name returns").is_some());
    assert!(
        project
            .game("rune-wheel")
            .unwrap()
            .profiles
            .resolve("cards/rune-wheel")
            .is_some()
    );
}

#[cfg(unix)]
#[test]
fn rejects_a_source_symlink_that_escapes_the_project() {
    use std::os::unix::fs::symlink;

    let root = temp("project-symlink-escape");
    let outside = root.parent().unwrap().join("outside-project.dli");
    fs::write(&outside, "scope outside\n").unwrap();
    symlink(&outside, root.join("escaped.dli")).unwrap();
    fs::write(
        root.join("deloreis.toml"),
        "version = 1\nname = \"bad\"\n\
         [documents.world]\nsources = [\"escaped.dli\"]\n",
    )
    .unwrap();

    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("escapes the project directory"), "{error}");
}

#[cfg(unix)]
#[test]
fn rejects_hard_linked_sources_with_different_logical_owners() {
    let root = temp("project-source-hard-link-alias");
    fs::write(root.join("one.dli"), "scope one\n").unwrap();
    fs::hard_link(root.join("one.dli"), root.join("two.dli")).unwrap();
    write_manifest(
        &root,
        "[documents.one]\nsources = [\"one.dli\"]\n\
         [documents.two]\nsources = [\"two.dli\"]\n",
    );

    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("aliases"), "{error}");
    assert!(error.contains("owned by both `one` and `two`"), "{error}");
}

#[cfg(unix)]
#[test]
fn rejects_a_storage_path_whose_existing_ancestor_escapes() {
    use std::os::unix::fs::symlink;

    let root = temp("project-storage-symlink-escape");
    let outside = root.parent().unwrap().join("outside-storage");
    fs::create_dir_all(&outside).unwrap();
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    symlink(&outside, root.join("escaped")).unwrap();
    write_manifest(
        &root,
        "[paths]\nevidence = \"escaped/evidence\"\ncache = \".cache\"\n\
         [documents.world]\nsources = [\"world.dli\"]\n",
    );

    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("traverses a symlink outside"), "{error}");
}

#[cfg(unix)]
#[test]
fn rejects_storage_paths_that_alias_the_same_directory() {
    use std::os::unix::fs::symlink;

    let root = temp("project-storage-symlink-alias");
    fs::create_dir_all(root.join("shared")).unwrap();
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    symlink("shared", root.join("evidence")).unwrap();
    symlink("shared", root.join("cache")).unwrap();
    write_manifest(
        &root,
        "[paths]\nevidence = \"evidence\"\ncache = \"cache\"\n\
         [documents.world]\nsources = [\"world.dli\"]\n",
    );

    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("non-overlapping"), "{error}");
}
