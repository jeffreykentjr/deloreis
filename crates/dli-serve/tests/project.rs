#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Caller-project loading and isolation through the real worker loop.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use dli_serve::project::Project;
use dli_serve::serve::serve_project_io;
use dli_world::payload::source_hash;
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

fn enable_craft_packs(root: &Path, packs: &[&str]) {
    fs::create_dir_all(root.join("craft")).unwrap();
    let values = packs
        .iter()
        .map(|pack| format!("\"{pack}\""))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        root.join("craft/enabled-packs.toml"),
        format!("packs = [{values}]\n"),
    )
    .unwrap();
}

fn serve_request(project: Project, request: &Value) -> Value {
    let mut output = Vec::new();
    serve_project_io(
        &mut Cursor::new(format!("{request}\n")),
        &mut output,
        project,
    )
    .unwrap();
    serde_json::from_slice(&output).unwrap()
}

fn claim_payload_request(
    source: &str,
    episode: &str,
    source_file: &str,
    atom: &str,
    quote: &str,
    line: u32,
    prompt_digit: char,
) -> Value {
    serde_json::json!({
        "dli": "0.5.1",
        "id": format!("submit-{episode}"),
        "op": "dli.payload-submit",
        "params": {
            "document": "world",
            "source": source,
            "payload": {
                "meta": {
                    "episode": episode,
                    "source": {
                        "file": source_file,
                        "sha256": source_hash(source),
                        "words": source.split_whitespace().count()
                    },
                    "extracted": "2026-07-26",
                    "extractors": {
                        "schema": "payload/v1",
                        "fields": {
                            "claims": {
                                "model": "fixture-extractor",
                                "prompt": format!("sha256:{}", prompt_digit.to_string().repeat(64))
                            }
                        }
                    }
                },
                "claims": [{
                    "atom": atom,
                    "quote": quote,
                    "line": line
                }]
            }
        }
    })
}

fn assert_payload_finding(
    findings: &[Value],
    source_file: &str,
    payload_digest: &Value,
    episode: &str,
    line: u32,
    quote: &str,
) {
    let finding = findings
        .iter()
        .find(|finding| {
            finding["code"] == "factcheck.unsupported" && finding["source_file"] == source_file
        })
        .unwrap_or_else(|| panic!("changed world did not invalidate {source_file}"));
    assert_eq!(finding["domain"], "payload", "{finding}");
    assert_eq!(finding["payload_digest"], *payload_digest, "{finding}");
    assert_eq!(finding["episode"], episode, "{finding}");
    assert_eq!(finding["source_file"], source_file, "{finding}");
    assert_eq!(finding["line"], line, "{finding}");
    assert_eq!(finding["quote"], quote, "{finding}");
}

#[test]
fn loads_the_second_world_as_one_logical_document() {
    let project = Project::load(norse()).expect("Norse project loads");
    assert_eq!(project.name(), "nine-realms");
    let document = project.document("norse").expect("logical document");
    assert_eq!(document.pack.documents.len(), 3);
    assert!(document.source_digest.starts_with("content-sha256:"));
    assert!(document.profiles.resolve("tabular/nine-realms@1").is_some());
    assert!(project.document("camelot").is_none());
    assert!(project.evidence_dir().is_dir());
    assert!(project.cache_dir().is_dir());
}

#[test]
fn document_source_anchor_changes_with_source_bytes() {
    let root = temp("project-source-anchor");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    write_manifest(&root, "[documents.world]\nsources = [\"world.dli\"]\n");

    let before = Project::load(&root)
        .unwrap()
        .document("world")
        .unwrap()
        .source_digest
        .clone();
    fs::write(root.join("world.dli"), "scope world\n\n").unwrap();
    let after = Project::load(&root)
        .unwrap()
        .document("world")
        .unwrap()
        .source_digest
        .clone();

    assert_ne!(before, after);
}

#[test]
fn project_worker_uses_the_document_profile_catalog() {
    let project = Project::load(norse()).expect("Norse project loads");
    let input = b"{\"dli\":\"0.5.1\",\"id\":\"csv\",\"op\":\"dli.export.run\",\"params\":{\"document\":\"norse\",\"profile\":\"csv\",\"profile_name\":\"tabular/nine-realms@1\"}}\n";
    let mut output = Vec::new();
    serve_project_io(&mut Cursor::new(input), &mut output, project).expect("worker completes");
    let response: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["result"]["emission"]["profile"], "csv");
    assert_eq!(
        response["result"]["emission"]["artifact"]["profile"],
        "tabular/nine-realms@1"
    );
    let csv = response["result"]["emission"]["artifact"]["content"]
        .as_str()
        .unwrap();
    assert!(csv.contains("asgard-to-midgard"), "{csv}");
    assert!(!csv.contains("asgard-to-alfheim"), "{csv}");
}

#[test]
fn project_worker_resolves_declared_ids_and_never_demo_falls_back() {
    let project = Project::load(norse()).expect("Norse project loads");
    let input = b"\
{\"dli\":\"0.5.1\",\"id\":\"1\",\"op\":\"dli.query\",\"params\":{\"document\":\"norse\",\"goal\":\"adjacent(asgard, midgard)\",\"layer\":\"certain\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"2\",\"op\":\"dli.check\",\"params\":{\"document\":\"camelot\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"3\",\"op\":\"author.open\",\"params\":{\"scope\":\"norse\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"4\",\"op\":\"author.query\",\"params\":{\"workspace\":\"ws-1\",\"goal\":\"adjacent(asgard, midgard)\",\"layer\":\"certain\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"5\",\"op\":\"author.open\",\"params\":{\"scope\":\"camelot\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"6\",\"op\":\"dli.export.run\",\"params\":{\"document\":\"french-deck\",\"profile\":\"cards\"}}\n";
    let mut output = Vec::new();
    serve_project_io(&mut Cursor::new(input), &mut output, project).expect("worker completes");
    let responses: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    assert_eq!(responses.len(), 6);
    assert_eq!(responses[0]["ok"], true);
    assert_eq!(
        responses[0]["result"]["rows"][0]["atom"],
        "adjacent(asgard, midgard)"
    );
    assert_eq!(responses[0]["result"]["rows"][0]["status"], "certain");
    assert_eq!(responses[1]["error"]["code"], "unknown-doc");
    assert_eq!(responses[2]["result"]["workspace"], "ws-1");
    assert_eq!(
        responses[3]["result"]["rows"][0]["atom"],
        "adjacent(asgard, midgard)"
    );
    assert_eq!(responses[4]["error"]["code"], "unknown-doc");
    assert_eq!(responses[5]["error"]["code"], "unknown-doc");
}

#[test]
fn project_door_t_rejects_invented_forks() {
    let project = Project::load(norse()).expect("Norse project loads");
    let input = b"\
{\"dli\":\"0.5.1\",\"id\":\"1\",\"op\":\"author.open\",\"params\":{\"fork\":\"what-does-wisdom-cost\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"2\",\"op\":\"author.open\",\"params\":{\"fork\":\"invented-fork\"}}\n";
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
{\"dli\":\"0.5.1\",\"id\":\"1\",\"op\":\"dli.fork.new\",\"params\":{\"document\":\"one\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"2\",\"op\":\"dli.query\",\"params\":{\"document\":\"two\",\"goal\":\"marked(beta)\",\"fork\":\"fork-1\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"3\",\"op\":\"dli.fork.new\",\"params\":{\"document\":\"two\",\"base\":\"fork-1\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"4\",\"op\":\"dli.fork.list\",\"params\":{\"document\":\"missing\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"5\",\"op\":\"author.open\",\"params\":{\"scope\":\"one\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"6\",\"op\":\"author.open\",\"params\":{\"scope\":\"two\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"7\",\"op\":\"author.query\",\"params\":{\"workspace\":\"ws-1\",\"goal\":\"marked(?)\",\"layer\":\"certain\"}}\n\
{\"dli\":\"0.5.1\",\"id\":\"8\",\"op\":\"author.query\",\"params\":{\"workspace\":\"ws-2\",\"goal\":\"marked(?)\",\"layer\":\"certain\"}}\n";
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
fn cyclic_timeline_reports_blame_without_cursor_order_claims() {
    let root = temp("project-cyclic-timeline");
    fs::write(
        root.join("world.dli"),
        "scope world\n\
         use chronology\n\
         ratify d-1\n\
         era age decided d-1\n\
         event first during age decided d-1\n\
         event second during age decided d-1\n\
         before first -> second decided d-1\n\
         before second -> first decided d-1\n",
    )
    .unwrap();
    write_manifest(&root, "[documents.world]\nsources = [\"world.dli\"]\n");
    let project = Project::load(&root).expect("cyclic chronology remains diagnosable");
    let input = b"{\"dli\":\"0.5.1\",\"id\":\"1\",\"op\":\"dli.timeline.show\",\"params\":{\"document\":\"world\",\"at\":\"first\"}}\n";
    let mut output = Vec::new();
    serve_project_io(&mut Cursor::new(input), &mut output, project).expect("worker completes");
    let response: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(response["ok"], true);
    assert!(
        response["result"]["cycle"]
            .as_array()
            .is_some_and(|cycle| !cycle.is_empty())
    );
    assert!(
        response["result"].get("cursor").is_none(),
        "reachability is undefined while cycle blame exists"
    );
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
        "killed:\n  - {token: old-name, match: word, decision: d-0099}\n\
         rejected_claims:\n\
           - {predicate: role, subject: hero, value: auditor, match: word, decision: d-0009}\n",
    )
    .unwrap();
    fs::copy(
        repo.join("worlds/tpth/episodes/saltmarket-episodes.yaml"),
        root.join("episodes.yaml"),
    )
    .unwrap();
    fs::write(
        root.join("outlines.yaml"),
        "id: season-1\ntitle: Season One\nepisodes:\n  - {id: e01, title: First}\n",
    )
    .unwrap();
    fs::write(root.join("craft-ledger.yaml"), "{}\n").unwrap();
    enable_craft_packs(&root, &["try-fail-cycles"]);
    fs::write(
        root.join("craft/thresholds.toml"),
        "[season]\n\
         major_prior_failures = 2\n\
         climax_prior_failures = 3\n\
         competence_floor = 5\n\
         reveal_lead_episodes = 1\n\
         mystery_starved_gap = 3\n\
         cold_open_cast_ceiling = 2\n\
         cast_introduction_band = [1, 3]\n\
         ten_position_band = [0.55, 0.8]\n\
         audio_high_run = 3\n\
         calibrated = true\n",
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
    fs::copy(
        repo.join("examples/norse/export-profiles.json"),
        root.join("document-profiles.json"),
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\"]\n\
         episodes = \"episodes.yaml\"\n\
         outlines = \"outlines.yaml\"\n\
         craft_ledger = \"craft-ledger.yaml\"\n\
         craft_config = \"craft\"\n\
         killed_tokens = \"killed.yaml\"\n\
         profiles = \"document-profiles.json\"\n\
         [games.rune-wheel]\nrules = \"rules.json\"\nprofiles = \"game-profiles.json\"\n",
    );

    let project = Project::load(&root).expect("all optional assets load");
    let document = project.document("world").unwrap();
    assert!(!document.episodes.as_ref().unwrap().is_empty());
    assert_eq!(document.outlines.as_ref().unwrap().episodes[0].id, "e01");
    assert!(document.craft_ledger.as_ref().unwrap().episodes.is_empty());
    assert!(
        document
            .craft_principles
            .as_ref()
            .is_some_and(|principles| !principles.is_empty())
    );
    assert_eq!(
        document
            .craft_thresholds
            .expect("configured thresholds")
            .major_prior_failures,
        2
    );
    assert!(document.craft_policy.is_some());
    assert!(document.killed.hit("the Old Name returns").is_some());
    assert_eq!(
        document
            .killed
            .rejected_hits(&("role".to_owned(), "hero".to_owned()), "auditor")
            .map(|claim| claim.decision.as_str())
            .collect::<Vec<_>>(),
        vec!["d-0009"]
    );
    assert!(document.profiles.resolve("wiki/nine-realms").is_some());
    assert!(
        project
            .game("rune-wheel")
            .unwrap()
            .profiles
            .resolve("cards/rune-wheel")
            .is_some()
    );
}

#[test]
fn project_timeline_knows_lists_every_declared_knower_source() {
    let root = temp("project-knower-universe");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    fs::write(
        root.join("episodes.yaml"),
        "- id: e01\n\
         \x20 production_order: 1\n\
         \x20 story_anchor: {after: [], before: [], phases: []}\n\
         \x20 scenes:\n\
         \x20   - id: s1\n\
         \x20     cast: [cast-only]\n\
         \x20     presence: [!OffMic presence-only]\n\
         \x20     location: null\n\
         \x20     scene_anchor: null\n\
         \x20     knowledge_deltas:\n\
         \x20       - {knower: !Cast delta-only, fact: learned-later}\n\
         \x20     fact_deltas: []\n\
         \x20     opens: []\n\
         \x20     closes: []\n",
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\"]\nepisodes = \"episodes.yaml\"\n",
    );
    let request = serde_json::json!({
        "dli": "0.5.1",
        "id": "knowers",
        "op": "dli.timeline.knows",
        "params": {"document": "world", "at": "0"}
    });
    let response = serve_request(Project::load(&root).unwrap(), &request);
    let rows = response["result"]["knowers"].as_array().unwrap();
    for name in ["cast-only", "presence-only", "delta-only"] {
        let row = rows
            .iter()
            .find(|row| row["knower"]["Cast"] == name)
            .unwrap_or_else(|| panic!("missing {name}: {response}"));
        assert_eq!(row["facts"], serde_json::json!([]), "{row}");
    }
}

#[test]
fn project_check_proof_selects_one_named_proof_and_refuses_ambiguity() {
    let root = temp("project-proof-selection");
    fs::write(
        root.join("world.dli"),
        "scope world\n\
         practice p proposed {\n\
         \x20 rule permit\n\
         }\n\
         proof first speaker witness in p {\n\
         \x20 close absent expect blocked\n\
         }\n\
         proof second speaker witness in p {\n\
         \x20 assume p1 present\n\
         \x20 close present expect holds\n\
         }\n",
    )
    .unwrap();
    write_manifest(&root, "[documents.world]\nsources = [\"world.dli\"]\n");

    let ambiguous = serve_request(
        Project::load(&root).unwrap(),
        &serde_json::json!({
            "dli": "0.5.1",
            "id": "ambiguous",
            "op": "dli.check-proof",
            "params": {"document": "world"}
        }),
    );
    assert_eq!(ambiguous["ok"], false, "{ambiguous}");
    assert_eq!(ambiguous["error"]["code"], "schema-violation");
    assert!(
        ambiguous["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("multiple proofs")),
        "{ambiguous}"
    );

    let selected = serve_request(
        Project::load(&root).unwrap(),
        &serde_json::json!({
            "dli": "0.5.1",
            "id": "selected",
            "op": "dli.check-proof",
            "params": {"document": "world", "proof": "second"}
        }),
    );
    assert_eq!(selected["ok"], true, "{selected}");
    assert_eq!(selected["result"]["report"]["proof"]["text"], "second");
    assert_eq!(selected["result"]["report"]["checks"], true);

    let missing = serve_request(
        Project::load(&root).unwrap(),
        &serde_json::json!({
            "dli": "0.5.1",
            "id": "missing",
            "op": "dli.check-proof",
            "params": {"document": "world", "proof": "missing"}
        }),
    );
    assert_eq!(missing["ok"], false, "{missing}");
    assert_eq!(missing["error"]["code"], "schema-violation");
}

#[test]
fn rejects_an_ambiguous_document_profile_catalog() {
    let root = temp("project-invalid-document-profiles");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    fs::write(
        root.join("profiles.json"),
        r#"{
          "profiles": [
            {
              "target": "csv",
              "name": "tabular/duplicate",
              "version": 1,
              "selects": {"sorts": [], "relations": []},
              "renders": {},
              "carries_provenance": "dev-metadata"
            },
            {
              "target": "csv",
              "name": "tabular/duplicate",
              "version": 1,
              "selects": {"sorts": [], "relations": []},
              "renders": {},
              "carries_provenance": "stripped"
            }
          ]
        }"#,
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\"]\nprofiles = \"profiles.json\"\n",
    );
    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("registered more than once"), "{error}");
    assert!(!root.join(".dli-evidence").exists());
    assert!(!root.join(".dli-cache").exists());
}

#[test]
fn rejects_a_game_profile_that_the_ruleset_cannot_render() {
    let root = temp("project-invalid-game-profile");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/games/french-deck/rules.json"),
        root.join("rules.json"),
    )
    .unwrap();
    fs::write(
        root.join("profiles.json"),
        r#"{
          "profiles": [{
            "target": "cards",
            "name": "cards/invalid",
            "version": 1,
            "selects": {"sorts": ["card"], "relations": ["invented"]},
            "renders": {
              "card": {"id": "{missing}", "keep": ["invented"]}
            },
            "carries_provenance": "dev-metadata"
          }]
        }"#,
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\"]\n\
         [games.deck]\nrules = \"rules.json\"\nprofiles = \"profiles.json\"\n",
    );

    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("selected relation `invented`"), "{error}");
    assert!(error.contains("profiles.json"), "{error}");
    assert!(!error.contains("rules.json is malformed"), "{error}");
    assert!(!root.join(".dli-evidence").exists());
    assert!(!root.join(".dli-cache").exists());
}

#[test]
fn payload_handler_reads_project_owned_outline_context() {
    let root = temp("project-payload-context");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    fs::write(
        root.join("outlines.yaml"),
        "id: season-1\ntitle: Season One\nepisodes:\n  - id: e01\n    title: First\n    tryfail_beats:\n      - {id: promised, attempt: open-door, outcome: no-and}\n",
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\"]\n\
         outlines = \"outlines.yaml\"\n",
    );
    let project = Project::load(&root).unwrap();
    let source = "draft";
    let request = serde_json::json!({
        "dli": "0.5.1",
        "id": "outline-context",
        "op": "dli.check-draft",
        "params": {
            "document": "world",
            "source": source,
            "payload": {
                "meta": {
                    "episode": "e01",
                    "source": {"sha256": source_hash(source)}
                }
            }
        }
    });
    let typo = serde_json::json!({
        "dli": "0.5.1",
        "id": "outline-typo",
        "op": "dli.payload-submit",
        "params": {
            "document": "world",
            "source": source,
            "payload": {
                "meta": {
                    "episode": "typo",
                    "source": {
                        "file": "draft.md",
                        "sha256": source_hash(source),
                        "words": 1
                    },
                    "extracted": "2026-07-26",
                    "extractors": {"schema": "payload/v1", "fields": {}}
                }
            }
        }
    });
    let mut output = Vec::new();
    serve_project_io(
        &mut Cursor::new(format!("{request}\n{typo}\n")),
        &mut output,
        project,
    )
    .unwrap();
    let responses = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let response = &responses[0];
    let codes = response["result"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"outline.beat-unaccounted"), "{response}");
    let typo_response = &responses[1];
    assert_eq!(
        typo_response["result"]["accepted"], false,
        "{typo_response}"
    );
    assert!(
        typo_response["result"]["payload_digest"].is_null(),
        "{typo_response}"
    );
    assert!(
        typo_response["result"]["findings"]
            .as_array()
            .is_some_and(|findings| findings
                .iter()
                .any(|finding| { finding["code"] == "payload.outline-episode-unknown" })),
        "{typo_response}"
    );
}

#[test]
fn check_draft_consumes_caller_owned_runtime_continuity_context() {
    let root = temp("project-runtime-continuity");
    fs::write(
        root.join("world.dli"),
        "scope world\nratify D-0001\nplace anchor ratified D-0001\nfact anchor gender male ratified D-0001\n",
    )
    .unwrap();
    write_manifest(&root, "[documents.world]\nsources = [\"world.dli\"]\n");
    let source = "draft";
    let request = serde_json::json!({
        "dli": "0.5.1",
        "id": "runtime-continuity",
        "op": "dli.check-draft",
        "params": {
            "document": "world",
            "source": source,
            "payload": {
                "meta": {"source": {"sha256": source_hash(source)}}
            },
            "continuity": {
                "established": [
                    {"predicate": "gender", "subject": "anchor", "value": "female"},
                    {"predicate": "mood", "subject": "anchor", "value": "grim"}
                ],
                "references": ["ghost"],
                "prior_facts": [
                    {"predicate": "mood", "subject": "anchor", "value": "weary"}
                ],
                "closed_promises": ["unowed-debt"]
            }
        }
    });

    let response = serve_request(Project::load(&root).unwrap(), &request);
    let findings = response["result"]["findings"].as_array().unwrap();
    let codes = findings
        .iter()
        .map(|finding| finding["code"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert!(
        codes.contains(&"continuity.world-contradiction"),
        "{response}"
    );
    assert!(codes.contains(&"continuity.unknown-entity"), "{response}");
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == "continuity.prior-contradiction")
            .count(),
        2,
        "{response}"
    );
}

#[test]
fn payload_handler_uses_project_finding_policy() {
    let root = temp("project-payload-finding-policy");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    enable_craft_packs(&root, &["try-fail-cycles"]);
    fs::write(
        root.join("craft/finding-policy.toml"),
        "[[rules]]\ncode = \"craft.accomplishment-earned\"\ncitations = [\"D-9999\"]\n",
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\"]\ncraft_config = \"craft\"\n",
    );
    let source = "triumph";
    let request = serde_json::json!({
        "dli": "0.5.1",
        "id": "finding-policy",
        "op": "dli.check-draft",
        "params": {
            "document": "world",
            "source": source,
            "payload": {
                "meta": {
                    "episode": "e01",
                    "source": {"sha256": source_hash(source)}
                },
                "scenes": [{"id": "s1"}],
                "beats": [{
                    "id": "triumph",
                    "scene": "s1",
                    "kind": "accomplishment",
                    "thread": "road",
                    "magnitude": "major",
                    "quote": source,
                    "line": 1
                }]
            }
        }
    });
    let response = serve_request(Project::load(&root).unwrap(), &request);
    let finding = response["result"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "craft.accomplishment-earned")
        .expect("craft finding");
    assert_eq!(finding["citations"], serde_json::json!(["D-9999"]));
    assert!(!response.to_string().contains("D-0128"), "{response}");
}

#[test]
fn outline_policy_survives_check_submit_and_restart_replay() {
    let root = temp("project-outline-finding-policy");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    fs::write(
        root.join("outlines.yaml"),
        "id: season-1\ntitle: Season One\nepisodes:\n  - id: e01\n    title: First\n    tryfail_beats:\n      - {id: promised, attempt: open-door, outcome: no-and}\n",
    )
    .unwrap();
    enable_craft_packs(&root, &["try-fail-cycles"]);
    fs::write(
        root.join("craft/finding-policy.toml"),
        "[[rules]]\ncode = \"outline.beat-unaccounted\"\ncitations = [\"D-9999\"]\nseverity = \"warn\"\n",
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\"]\noutlines = \"outlines.yaml\"\ncraft_config = \"craft\"\n",
    );

    let source = "baseline";
    let payload = serde_json::json!({
        "meta": {
            "episode": "e01",
            "source": {
                "file": "draft.md",
                "sha256": source_hash(source),
                "words": 1
            },
            "extracted": "2026-07-26",
            "extractors": {"schema": "payload/v1", "fields": {}}
        }
    });
    let check_draft = serde_json::json!({
        "dli": "0.5.1",
        "id": "outline-policy-check",
        "op": "dli.check-draft",
        "params": {
            "document": "world",
            "source": source,
            "payload": payload.clone()
        }
    });
    let checked = serve_request(Project::load(&root).unwrap(), &check_draft);
    let checked_finding = checked["result"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "outline.beat-unaccounted")
        .expect("outline finding");
    assert_eq!(checked_finding["severity"], "warn", "{checked}");
    assert_eq!(
        checked_finding["citations"],
        serde_json::json!(["D-9999"]),
        "{checked}"
    );

    let submit = serde_json::json!({
        "dli": "0.5.1",
        "id": "outline-policy-submit",
        "op": "dli.payload-submit",
        "params": {
            "document": "world",
            "source": source,
            "payload": payload
        }
    });
    let submitted = serve_request(Project::load(&root).unwrap(), &submit);
    assert_eq!(submitted["result"]["accepted"], true, "{submitted}");
    assert!(
        submitted["result"]["payload_digest"].is_string(),
        "{submitted}"
    );
    assert_eq!(
        submitted["result"]["findings"][0]["severity"], "warn",
        "{submitted}"
    );
    assert_eq!(
        submitted["result"]["findings"][0]["citations"],
        serde_json::json!(["D-9999"]),
        "{submitted}"
    );

    let check = serde_json::json!({
        "dli": "0.5.1",
        "id": "outline-policy-replay",
        "op": "dli.check",
        "params": {"document": "world"}
    });
    let replayed = serve_request(Project::load(&root).unwrap(), &check);
    assert_eq!(replayed["result"]["ok"], true, "{replayed}");
    let replayed_finding = replayed["result"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "outline.beat-unaccounted")
        .expect("replayed outline finding");
    assert_eq!(replayed_finding["domain"], "payload", "{replayed_finding}");
    assert_eq!(replayed_finding["severity"], "warn", "{replayed_finding}");
    assert_eq!(
        replayed_finding["citations"],
        serde_json::json!(["D-9999"]),
        "{replayed_finding}"
    );
}

#[test]
fn document_check_replays_committed_payloads_after_restart() {
    let root = temp("project-payload-replay");
    enable_craft_packs(&root, &["try-fail-cycles"]);
    fs::write(
        root.join("world.dli"),
        "scope world\n\nratify d-1\n\
         place baseline \"Baseline\" decided d-1\n\
         place marker \"Marker\" decided d-1\n\
         place witness \"Witness\" decided d-1\n\
         adjacent(baseline, marker) decided d-1\n\
         adjacent(marker, witness) decided d-1\n",
    )
    .unwrap();
    fs::write(
        root.join("outlines.yaml"),
        "id: season-1\ntitle: Season One\nepisodes:\n  - id: e01\n    title: First\n  - id: e02\n    title: Second\n",
    )
    .unwrap();
    write_manifest(
        &root,
        "[documents.world]\nsources = [\"world.dli\"]\n\
         outlines = \"outlines.yaml\"\n\
         craft_config = \"craft\"\n",
    );
    let source = "baseline marker";
    let submit = claim_payload_request(
        source,
        "e01",
        "draft.md",
        "adjacent(baseline, marker)",
        source,
        1,
        '0',
    );
    let accepted = serve_request(Project::load(&root).unwrap(), &submit);
    assert_eq!(accepted["result"]["accepted"], true, "{accepted}");
    let payload_digest = accepted["result"]["payload_digest"].clone();

    let second_quote = "marker witness";
    let second_source = format!("\n\n\n{second_quote}");
    let second_submit = claim_payload_request(
        &second_source,
        "e02",
        "second.md",
        "adjacent(marker, witness)",
        second_quote,
        4,
        '1',
    );
    let second_accepted = serve_request(Project::load(&root).unwrap(), &second_submit);
    assert_eq!(
        second_accepted["result"]["accepted"], true,
        "{second_accepted}"
    );
    let second_payload_digest = second_accepted["result"]["payload_digest"].clone();
    assert_ne!(payload_digest, second_payload_digest);

    fs::write(
        root.join("outlines.yaml"),
        "id: season-1\ntitle: Season One\nepisodes:\n  - id: e01\n    title: First\n    tryfail_beats:\n      - {id: promised, attempt: open-door, outcome: no-and}\n  - id: e02\n    title: Second\n",
    )
    .unwrap();
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    let check = serde_json::json!({
        "dli": "0.5.1",
        "id": "check",
        "op": "dli.check",
        "params": {"document": "world"}
    });
    let replayed = serve_request(Project::load(&root).unwrap(), &check);
    assert_eq!(replayed["result"]["ok"], false, "{replayed}");
    assert!(
        replayed["result"]["findings"]
            .as_array()
            .is_some_and(|findings| findings
                .iter()
                .any(|finding| finding["code"] == "outline.beat-unaccounted")),
        "{replayed}"
    );
    let findings = replayed["result"]["findings"]
        .as_array()
        .expect("check findings");
    assert_payload_finding(findings, "draft.md", &payload_digest, "e01", 1, source);
    assert_payload_finding(
        findings,
        "second.md",
        &second_payload_digest,
        "e02",
        4,
        second_quote,
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
    assert!(error.contains("must not traverse symlinks"), "{error}");
}

#[cfg(unix)]
#[test]
fn rejects_storage_path_symlinks_even_when_they_resolve_internally() {
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
    assert!(error.contains("must not traverse symlinks"), "{error}");
}

#[cfg(unix)]
#[test]
fn rejects_an_internal_storage_symlink_without_materializing_other_paths() {
    use std::os::unix::fs::symlink;

    let root = temp("project-storage-internal-symlink");
    fs::create_dir_all(root.join("state/evidence")).unwrap();
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    symlink("state/evidence", root.join("evidence")).unwrap();
    write_manifest(
        &root,
        "[paths]\nevidence = \"evidence\"\ncache = \"future/cache\"\n\
         [documents.world]\nsources = [\"world.dli\"]\n",
    );

    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("must not traverse symlinks"), "{error}");
    assert!(!root.join("future").exists());
}

#[test]
fn invalid_overlapping_storage_has_no_filesystem_side_effect() {
    let root = temp("project-storage-overlap-no-write");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    write_manifest(
        &root,
        "[paths]\nevidence = \"state\"\ncache = \"state/cache\"\n\
         [documents.world]\nsources = [\"world.dli\"]\n",
    );

    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("non-overlapping"), "{error}");
    assert!(!root.join("state").exists());
}

#[test]
fn rejects_storage_that_contains_a_manifest_input() {
    let root = temp("project-storage-contains-input");
    fs::create_dir_all(root.join("canon")).unwrap();
    fs::write(root.join("canon/world.dli"), "scope world\n").unwrap();
    write_manifest(
        &root,
        "[paths]\nevidence = \"canon\"\ncache = \".cache\"\n\
         [documents.world]\nsources = [\"canon/world.dli\"]\n",
    );

    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("must not contain project input"), "{error}");
    assert!(error.contains("world.dli"), "{error}");
}

#[cfg(unix)]
#[test]
fn rejects_storage_file_hard_linked_to_a_manifest_input() {
    let root = temp("project-storage-hard-link-input");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    fs::create_dir_all(root.join("evidence")).unwrap();
    fs::hard_link(
        root.join("world.dli"),
        root.join("evidence/source-alias.dli"),
    )
    .unwrap();
    write_manifest(
        &root,
        "[paths]\nevidence = \"evidence\"\ncache = \"cache\"\n\
         [documents.world]\nsources = [\"world.dli\"]\n",
    );

    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("aliases project input"), "{error}");
    assert!(error.contains("source-alias.dli"), "{error}");
}

#[test]
fn materializes_missing_storage_suffixes_before_serving() {
    let root = temp("project-storage-missing-suffix");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    write_manifest(
        &root,
        "[paths]\nevidence = \"state/future/evidence\"\ncache = \"state/future/cache\"\n\
         [documents.world]\nsources = [\"world.dli\"]\n",
    );

    let project = Project::load(&root).expect("nested storage roots are materialized");
    assert!(project.evidence_dir().is_dir());
    assert!(project.cache_dir().is_dir());
    assert!(project.evidence_dir().starts_with(&root));
    assert!(project.cache_dir().starts_with(&root));
}

#[cfg(unix)]
#[test]
fn rejects_special_filesystem_nodes_inside_machine_storage() {
    use std::os::unix::net::UnixListener;

    let root = temp("project-storage-special-node");
    fs::write(root.join("world.dli"), "scope world\n").unwrap();
    fs::create_dir_all(root.join("evidence")).unwrap();
    let _socket = UnixListener::bind(root.join("evidence/worker.sock")).unwrap();
    write_manifest(
        &root,
        "[paths]\nevidence = \"evidence\"\ncache = \"cache\"\n\
         [documents.world]\nsources = [\"world.dli\"]\n",
    );

    let error = Project::load(&root).unwrap_err().to_string();
    assert!(error.contains("special filesystem node"), "{error}");
}
