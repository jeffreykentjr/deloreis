//! The Hollow walkthrough, driven through the actual MCP stdio transport.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Server {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    id: u32,
    calls: Vec<Value>,
}

impl Server {
    fn start(project: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_deloreis"))
            .args(["mcp", "--tools", "all", "--project"])
            .arg(project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            id: 0,
            calls: Vec::new(),
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.id += 1;
        self.calls.push(json!({"tool":name,"arguments":arguments}));
        let frame = json!({"jsonrpc":"2.0", "id":self.id, "method":"tools/call",
            "params":{"name":name,"arguments":arguments}});
        writeln!(self.input, "{frame}").unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        assert!(
            self.output.read_line(&mut line).unwrap() > 0,
            "server exited during {name}"
        );
        let response: Value = serde_json::from_str(&line).unwrap();
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    fn ok(&mut self, name: &str, arguments: Value) -> Value {
        let response = self.call(name, arguments);
        assert_eq!(response["ok"], true, "{name}: {response}");
        response["result"].clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn atom(pred: &str, subject: &str, value: &str) -> Value {
    json!({"pred":pred,"args":[{"Sym":subject},{"Sym":value}]})
}

fn cited(decision: &str) -> Value {
    json!({"kind":"cited","decisions":[decision],"state":"ratified"})
}

fn query(server: &mut Server, workspace: &str, pred: &str, value: &str) -> Value {
    server.ok(
        "author.query",
        json!({"workspace":workspace,
        "goal_atom":atom(pred, "miller", value), "layer":"all"}),
    )
}

fn why(server: &mut Server, workspace: &str, value: &str) -> Value {
    server.ok(
        "author.why",
        json!({"workspace":workspace,
        "goal_atom":atom("death", "miller", value)}),
    )
}

fn record(transcript: &mut String, server: &mut Server, title: &str, value: &Value) {
    use std::fmt::Write as _;
    writeln!(
        transcript,
        "\n## {title}\n\n```json\n{}\n```",
        serde_json::to_string_pretty(
            &json!({"calls":std::mem::take(&mut server.calls),"result":value})
        )
        .unwrap()
    )
    .unwrap();
}

#[test]
fn hollow_walkthrough_is_replayable_over_mcp() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = repo.join("examples/hollow");
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("story");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_deloreis"))
            .arg("init")
            .arg(&project)
            .output()
            .unwrap()
            .status
            .success()
    );
    let mut server = Server::start(&project);
    let mut transcript = String::from(
        "# The Hollow: a walkthrough\n\nThese calls and responses were captured from a walkthrough of The Hollow.\n",
    );

    let workspace = server.ok("author.open", json!({"scope":"world"}))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    server.ok(
        "author.record_decision",
        json!({"workspace":workspace,"decision":"d-0001"}),
    );
    for (kind, id, name) in [
        ("person", "wren", "Wren"),
        ("place", "thornfield", "Thornfield"),
        ("place", "ashby", "Ashby"),
        ("person", "miller", "The Miller"),
        ("era", "before-rain", "Before the Rain"),
        ("era", "after-rain", "After the Rain"),
    ] {
        server.ok("author.add_entity", json!({"workspace":workspace,"kind":kind,"id":id,"name":name,"provenance":cited("d-0001")}));
    }
    server.ok(
        "author.declare_functional",
        json!({"workspace":workspace,"pred":"home","provenance":cited("d-0001")}),
    );
    server.ok("author.add_fact", json!({"workspace":workspace,"entity":"wren","key":"home","value":"thornfield","provenance":cited("d-0001")}));
    server.ok("author.materialize", json!({"workspace":workspace}));
    let home = server.ok(
        "author.query",
        json!({"workspace":workspace,"goal_atom":atom("home","wren","thornfield")}),
    );
    assert_eq!(home["rows"][0]["status"], "certain");
    record(
        &mut transcript,
        &mut server,
        "1. Wren's home is canon",
        &home["rows"],
    );
    server.ok(
        "author.record_decision",
        json!({"workspace":workspace,"decision":"d-0002"}),
    );
    server.ok("author.add_fact", json!({"workspace":workspace,"entity":"wren","key":"home","value":"ashby","provenance":cited("d-0002")}));
    let checked = server.ok("author.check", json!({"workspace":workspace}));
    let conflict = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "continuity.fd-violation")
        .unwrap();
    assert!(conflict["citations"].to_string().contains("d-0001"));
    assert!(conflict["citations"].to_string().contains("d-0002"));
    let blocked = server.call("author.materialize", json!({"workspace":workspace}));
    assert_eq!(blocked["error"]["code"], "materialize-blocked");
    record(
        &mut transcript,
        &mut server,
        "2. Conflicting canon is refused",
        &json!({"finding":conflict,"error":blocked["error"]}),
    );
    server.ok("author.discard", json!({"workspace":workspace}));
    let workspace = server.ok("author.open", json!({"scope":"world"}))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();

    server.ok(
        "author.open_question",
        json!({"workspace":workspace,"id":"miller-death","prompt":"How did the miller die?"}),
    );
    for (value, witness) in [("drowned", "witness-one"), ("poisoned", "witness-two")] {
        server.ok(
            "author.add_option",
            json!({"workspace":workspace,"question":"miller-death",
            "id":witness,"label":witness,"implies":atom("death", "miller", value)}),
        );
    }
    let drowned = query(&mut server, &workspace, "death", "drowned");
    let poisoned = query(&mut server, &workspace, "death", "poisoned");
    assert_eq!(drowned["rows"][0]["status"], "possible");
    assert_eq!(poisoned["rows"][0]["status"], "possible");
    let why_drowned = why(&mut server, &workspace, "drowned");
    let why_poisoned = why(&mut server, &workspace, "poisoned");
    assert!(
        why_drowned["why"]["rendered"]
            .to_string()
            .contains("witness-one")
    );
    assert!(
        why_poisoned["why"]["rendered"]
            .to_string()
            .contains("witness-two")
    );
    record(
        &mut transcript,
        &mut server,
        "3. Two witnesses, two possible accounts",
        &json!({
        "drowned":{"status":drowned["rows"][0]["status"],"why":why_drowned["why"]["rendered"]},
        "poisoned":{"status":poisoned["rows"][0]["status"],"why":why_poisoned["why"]["rendered"]}}),
    );
    server.ok("author.materialize", json!({"workspace":workspace}));
    let reopened = server.ok("author.open", json!({"scope":"world"}))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    for (value, witness) in [("drowned", "witness-one"), ("poisoned", "witness-two")] {
        assert_eq!(
            query(&mut server, &reopened, "death", value)["rows"][0]["status"],
            "possible"
        );
        assert!(
            why(&mut server, &reopened, value)["why"]["rendered"]
                .to_string()
                .contains(witness)
        );
    }
    let workspace = reopened;
    server.ok(
        "author.record_decision",
        json!({"workspace":workspace,"decision":"d-0003"}),
    );
    server.ok("author.add_fact", json!({"workspace":workspace,"entity":"miller","key":"death","value":"drowned","provenance":cited("d-0003")}));
    server.ok("author.materialize", json!({"workspace":workspace}));
    let drowning = query(&mut server, &workspace, "death", "drowned");
    assert_eq!(drowning["rows"][0]["status"], "certain");
    let claim = drowning["rows"][0]["claim_ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let defeat = server.ok("author.defeat", json!({"workspace":workspace,"claim_id":claim,"decision":"d-0004","defeated_decision":"d-0003"}));
    assert!(defeat["root_hash"].is_string());
    let blocked_why = why(&mut server, &workspace, "drowned");
    assert_eq!(blocked_why["why_not"]["status"], "possible");
    server.ok("author.add_fact", json!({"workspace":workspace,"entity":"miller","key":"death","value":"poisoned","provenance":cited("d-0004")}));
    server.ok("author.materialize", json!({"workspace":workspace}));
    let retconned = query(&mut server, &workspace, "death", "poisoned");
    assert_eq!(retconned["rows"][0]["status"], "certain");
    record(
        &mut transcript,
        &mut server,
        "4a. The retcon",
        &json!({"blocked_supports":blocked_why["blocked_supports"], "poisoned":retconned["rows"]}),
    );
    let attacker = retconned["rows"][0]["claim_ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    server.ok("author.defeat", json!({"workspace":workspace,"claim_id":attacker,"decision":"d-0005","defeated_decision":"d-0004"}));
    let restored = query(&mut server, &workspace, "death", "drowned");
    assert_eq!(restored["rows"][0]["status"], "certain");
    let overturned = query(&mut server, &workspace, "death", "poisoned");
    assert_eq!(overturned["rows"][0]["status"], "possible");
    record(
        &mut transcript,
        &mut server,
        "4b. Restoring the earlier ruling",
        &json!({"drowned":restored["rows"], "poisoned":overturned["rows"]}),
    );

    server.ok(
        "author.record_decision",
        json!({"workspace":workspace,"decision":"d-0006"}),
    );
    server.ok("author.add_rule", json!({"workspace":workspace,"name":"away-if-not-stayed",
        "head":atom("away","wren","thornfield"),
        "body":[{"atom":atom("journey","wren","stayed"),"negated":true}],"provenance":cited("d-0006")}));
    server.ok("author.materialize", json!({"workspace":workspace}));
    server.ok(
        "author.branch",
        json!({"workspace":workspace,"action":"create","name":"never-left","from":"main"}),
    );
    let branch = server.ok(
        "author.open",
        json!({"scope":"world","branch":"never-left"}),
    )["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    server.ok(
        "author.record_decision",
        json!({"workspace":branch,"decision":"d-0007"}),
    );
    server.ok(
        "author.add_fact",
        json!({"workspace":branch,"entity":"wren","key":"journey","value":"stayed",
        "provenance":cited("d-0007")}),
    );
    server.ok("author.add_rule", json!({"workspace":branch,"name":"welcome-if-stayed",
        "head":atom("welcome","wren","thornfield"),
        "body":[{"atom":atom("journey","wren","stayed"),"negated":false}],"provenance":cited("d-0007")}));
    let diff = server.ok("author.diff", json!({"workspace":branch}));
    let rows = diff["rows"].as_array().unwrap();
    let away = rows
        .iter()
        .find(|row| row["atom"] == "away(wren, thornfield)")
        .unwrap();
    assert_eq!(away["before"]["status"], "certain");
    assert_eq!(away["after"]["status"], "possible");
    let welcome = rows
        .iter()
        .find(|row| row["atom"] == "welcome(wren, thornfield)")
        .unwrap();
    assert_eq!(welcome["introduced"], true);
    assert_eq!(welcome["after"]["status"], "certain");
    let main = server.ok(
        "author.query",
        json!({"workspace":workspace,"goal_atom":atom("away","wren","thornfield")}),
    );
    assert_eq!(main["rows"][0]["status"], "certain");
    record(
        &mut transcript,
        &mut server,
        "5. What if Wren stayed?",
        &diff["rows"],
    );
    transcript.push_str("\nThe branch removes the certainty of `away`; it does not assert that Wren is not away. The result is `possible`, not a claim of falsehood.\n");
    let before_restart = restored["rows"].clone();
    drop(server);
    let mut server = Server::start(&project);
    let reopened = server.ok("author.open", json!({"scope":"world"}))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let replay = query(&mut server, &reopened, "death", "drowned");
    assert_eq!(
        replay["rows"], before_restart,
        "restarting must replay the same answer"
    );
    record(
        &mut transcript,
        &mut server,
        "6. Replay after restart",
        &replay["rows"],
    );

    let path = example.join("transcript.md");
    if std::env::var_os("DLI_BLESS_HOLLOW").is_some() {
        fs::write(&path, transcript).unwrap();
    } else {
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            transcript,
            "set DLI_BLESS_HOLLOW to update the checked transcript"
        );
    }
}

#[test]
fn exported_hollow_text_imports_to_the_same_first_ruling() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = repo.join("examples/hollow");
    let project = tempfile::tempdir().unwrap();
    let init = Command::new(env!("CARGO_BIN_EXE_deloreis"))
        .arg("init")
        .arg(project.path())
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let import = Command::new(env!("CARGO_BIN_EXE_deloreis"))
        .arg("import")
        .arg(example.join("hollow.dli"))
        .arg("--project")
        .arg(project.path())
        .output()
        .unwrap();
    assert!(
        import.status.success(),
        "{}",
        String::from_utf8_lossy(&import.stderr)
    );
    let mut server = Server::start(project.path());
    let ws = server.ok("author.open", json!({"scope":"world"}))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let home = server.ok(
        "author.query",
        json!({"workspace":ws,"goal_atom":atom("home","wren","thornfield")}),
    );
    assert_eq!(home["rows"][0]["status"], "certain");
}
