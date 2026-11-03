//! Exercise the published quickstart on a blank, store-backed project.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Server {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next_id: u32,
}

impl Server {
    fn start(project: &std::path::Path, all_tools: bool) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_deloreis"));
        command.args(["mcp", "--project"]).arg(project);
        if all_tools {
            command.args(["--tools", "all"]);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            next_id: 0,
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.next_id += 1;
        writeln!(
            self.input,
            "{}",
            json!({"jsonrpc":"2.0", "id":self.next_id,
            "method":"tools/call", "params":{"name":name,"arguments":arguments}})
        )
        .unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        assert!(
            self.output.read_line(&mut line).unwrap() > 0,
            "server exited during {name}"
        );
        let response: Value = serde_json::from_str(&line).unwrap();
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    fn ok(&mut self, name: &str, args: Value) -> Value {
        let response = self.call(name, args);
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

#[test]
fn quickstart_preserves_staged_claims_when_a_decision_is_recorded() {
    for bulk in [false, true] {
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
        let mut server = Server::start(&project, bulk);
        let workspace = server.ok("author.open", json!({"scope":"world"}))["workspace"]
            .as_str()
            .unwrap()
            .to_owned();
        for (kind, id, name) in [
            ("person", "wren", "Wren"),
            ("place", "thornfield", "Thornfield"),
            ("place", "ashby", "Ashby"),
        ] {
            server.ok("author.add_entity", json!({"workspace":workspace,"kind":kind,"id":id,"name":name,"provenance":{"kind":"proposed","origin":"quickstart"}}));
        }
        server.ok("author.declare_functional", json!({"workspace":workspace,"pred":"home","provenance":{"kind":"proposed","origin":"quickstart"}}));
        if bulk {
            server.ok(
                "author.record_decisions",
                json!({"workspace":workspace,"records":[{"decision":"d-0001"}]}),
            );
        } else {
            server.ok(
                "author.record_decision",
                json!({"workspace":workspace,"decision":"d-0001"}),
            );
        }
        server.ok("author.add_fact", json!({"workspace":workspace,"entity":"wren","key":"home","value":"thornfield","provenance":{"kind":"cited","decisions":["d-0001"],"state":"ratified"}}));
        server.ok("author.materialize", json!({"workspace":workspace}));
        server.ok(
            "author.record_decision",
            json!({"workspace":workspace,"decision":"d-0002"}),
        );
        server.ok("author.add_fact", json!({"workspace":workspace,"entity":"wren","key":"home","value":"ashby","provenance":{"kind":"cited","decisions":["d-0002"],"state":"ratified"}}));
        let checked = server.ok("author.check", json!({"workspace":workspace}));
        assert!(
            checked["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["code"] == "continuity.fd-violation"),
            "{checked}"
        );
        let blocked = server.call("author.materialize", json!({"workspace":workspace}));
        assert_eq!(blocked["error"]["code"], "materialize-blocked", "{blocked}");
        drop(server);
        let mut restarted = Server::start(&project, bulk);
        let fresh = restarted.ok("author.open", json!({"scope":"world"}))["workspace"]
            .as_str()
            .unwrap()
            .to_owned();
        let home = restarted.ok("author.query", json!({"workspace":fresh,"goal_atom":{"pred":"home","args":[{"Sym":"wren"},{"Sym":"thornfield"}]}}));
        assert_eq!(home["rows"][0]["status"], "certain", "{home}");
    }
}

#[test]
fn live_document_reads_follow_commits_across_server_processes() {
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
    let mut writer = Server::start(&project, true);
    let mut reader = Server::start(&project, true);
    let workspace = writer.ok("author.open", json!({"scope":"world"}))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let goal = json!({"pred":"person","args":[{"Var":"who"}]});
    let _ = writer.call("dli.query", json!({"document":"world","goal_atom":goal}));
    writer.ok("author.add_entity", json!({"workspace":workspace,"kind":"person","id":"wren","name":"Wren","provenance":{"kind":"proposed","origin":"test"}}));
    writer.ok("author.materialize", json!({"workspace":workspace}));
    let fork = writer.ok("dli.fork.new", json!({"document":"world"}))["fork"]
        .as_str()
        .unwrap()
        .to_owned();
    let fork_initial = writer.ok(
        "dli.query",
        json!({"document":"world","goal_atom":goal,"fork":fork}),
    );
    assert!(
        fork_initial["rows"].to_string().contains("wren"),
        "{fork_initial}"
    );
    writer.ok("author.add_entity", json!({"workspace":workspace,"kind":"person","id":"miller","name":"Miller","provenance":{"kind":"proposed","origin":"test"}}));
    writer.ok("author.add_entity", json!({"workspace":workspace,"kind":"era","id":"spring","name":"Spring","provenance":{"kind":"proposed","origin":"test"}}));
    writer.ok("author.add_event", json!({"workspace":workspace,"id":"first-day","name":"First day","era":"spring","provenance":{"kind":"proposed","origin":"test"}}));
    writer.ok("author.declare_functional", json!({"workspace":workspace,"pred":"home","provenance":{"kind":"proposed","origin":"test"}}));
    for value in ["thornfield", "ashby"] {
        writer.ok("author.add_fact", json!({"workspace":workspace,"entity":"wren","key":"home","value":value,"provenance":{"kind":"proposed","origin":value}}));
    }
    writer.ok("author.materialize", json!({"workspace":workspace}));
    let current = writer.ok("dli.query", json!({"document":"world","goal_atom":goal}));
    assert!(current["rows"].to_string().contains("wren"), "{current}");
    assert!(current["rows"].to_string().contains("miller"), "{current}");
    let fork_after = writer.ok(
        "dli.query",
        json!({"document":"world","goal_atom":goal,"fork":fork}),
    );
    assert!(
        fork_after["rows"].to_string().contains("wren"),
        "{fork_after}"
    );
    assert!(
        !fork_after["rows"].to_string().contains("miller"),
        "{fork_after}"
    );
    let checked = writer.ok("dli.check", json!({"document":"world"}));
    assert!(
        checked["findings"]
            .to_string()
            .contains("continuity.pending-conflict"),
        "{checked}"
    );
    let timeline = writer.ok("dli.timeline.show", json!({"document":"world"}));
    assert!(timeline.to_string().contains("first-day"), "{timeline}");
    let other = reader.ok("author.open", json!({"scope":"world"}))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let result = reader.ok("author.query", json!({"workspace":other,"goal_atom":goal}));
    assert!(result["rows"].to_string().contains("wren"), "{result}");
}

#[test]
fn a_live_read_does_not_reinitialize_a_missing_ref() {
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
    let mut server = Server::start(&project, false);
    server.ok("author.open", json!({"scope":"world"}));
    let main_ref = project.join(".deloreis/store/refs/main");
    std::fs::remove_file(&main_ref).unwrap();
    let response = server.call("dli.check", json!({"document":"world"}));
    assert_eq!(response["ok"], false, "{response}");
    assert!(
        !main_ref.exists(),
        "a read silently recreated the missing ref"
    );
}
