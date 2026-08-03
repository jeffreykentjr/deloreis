//! The serve loop and request dispatch (`SERVE-1`).
//!
//! `deloreis serve` speaks newline-delimited JSON over stdio: one request frame
//! per line in, one response frame per line out (`docs/spec/10-serving.md` §1).
//! This module owns the framing and the routing; [`crate::handlers`] owns the
//! `dli.*` semantics and [`crate::author`] the `author.*` seam.
//!
//! Framing discipline (`integration/fantasy-contract.md` §2):
//!
//! - A blank line, or any line that is not a JSON object, is treated as stray
//!   log noise and **skipped** — never answered — so a worker sharing its stdout
//!   with a diagnostic line does not desync the frame stream.
//! - A line that opens as a JSON object but does not decode as a request envelope
//!   — truncated JSON, a duplicate key, a missing or mistyped envelope field — is
//!   a `bad-frame` error.
//! - A syntactically valid frame naming an operation the contract does not define
//!   is `unknown-op`; a **killed form** (`dli.suppose`, the renamed
//!   `dli.check-ledger`) is refused the same way, its message naming the
//!   successor.
//! - Responses carry no timestamps: a response is a pure function of the corpus,
//!   the contract version, and the arguments.

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use serde_json::Value;

use crate::CONTRACT_VERSION;
use crate::author;
use crate::envelope::{ErrorBody, ErrorCode, Request, Response};
use crate::handlers;
use crate::ops::Op;
use crate::project::Project;
use crate::session::Sessions;

/// The killed wire forms the server refuses, each paired with its successor. A
/// killed form is never silently accepted or quietly removed — it is refused with
/// `unknown-op` naming what replaced it (`fantasy-contract.md` §3.1).
const KILLED_OPS: &[(&str, &str)] = &[
    ("dli.suppose", "dli.fork.suppose"),
    ("dli.check-ledger", "dli.check-proof"),
];

/// The contract version at which the retired *field* forms in [`KILLED_FIELDS`]
/// start being refused. No version in
/// [`ACCEPTED_CONTRACT_VERSIONS`](crate::ACCEPTED_CONTRACT_VERSIONS) reaches it,
/// so the field-level registry is **dormant**: every string form is still
/// accepted on every wire this build speaks.
const FIELD_RETIREMENT_MAJOR: u32 = 1;

/// The killed wire forms at *field* granularity: `(op, retired field, successor
/// field)`. Where [`KILLED_OPS`] retires a whole operation, this retires one
/// text-carrying parameter in favour of its typed successor — the LS-7 surface
/// retirement (`docs/design-notes/lorestore-roadmap.md` §LS-7). The path is
/// dotted from `params`, with `[]` naming "every element of this array".
///
/// Registered, not yet enforced. A killed form is registered the moment its
/// successor exists so the retirement is a matter of record before it is a
/// matter of refusal; enforcement is gated on [`FIELD_RETIREMENT_MAJOR`], which
/// no accepted version reaches. Under `0.5.1` and every `0.6.x` a request
/// carrying the string form is accepted unchanged.
const KILLED_FIELDS: &[(&str, &str, &str)] = &[
    ("dli.query", "goal", "goal_atom"),
    ("author.query", "goal", "goal_atom"),
    ("dli.fork.add", "declarations", "additions"),
    ("dli.payload-submit", "payload.claims[].atom", "atom_expr"),
];

/// Whether field-level killed forms are enforced on a negotiated wire version.
/// True only from contract [`FIELD_RETIREMENT_MAJOR`].0 up — so false for every
/// version this build accepts, which is what makes [`KILLED_FIELDS`] dormant.
fn field_retirement_active(dli: &str) -> bool {
    dli.split('.')
        .next()
        .and_then(|major| major.parse::<u32>().ok())
        .is_some_and(|major| major >= FIELD_RETIREMENT_MAJOR)
}

/// Whether a dotted `params` path is carried by a request's params. A `[]`
/// segment matches any element of an array, so `payload.claims[].atom` is
/// present when *some* claim carries `atom`.
fn field_present(params: &Value, path: &str) -> bool {
    let Some((head, rest)) = path.split_once('.') else {
        return segment_present(params, path);
    };
    match descend(params, head) {
        Some(inner) => inner.iter().any(|value| field_present(value, rest)),
        None => false,
    }
}

/// The leaf case of [`field_present`]: a single (possibly `[]`-suffixed) segment.
fn segment_present(value: &Value, segment: &str) -> bool {
    descend(value, segment).is_some_and(|found| !found.is_empty())
}

/// Resolve one path segment to the values it selects. A bare `name` selects the
/// object member; `name[]` selects every element of that member's array.
fn descend<'a>(value: &'a Value, segment: &str) -> Option<Vec<&'a Value>> {
    let (key, spread) = match segment.strip_suffix("[]") {
        Some(key) => (key, true),
        None => (segment, false),
    };
    let member = value.get(key)?;
    if spread {
        Some(member.as_array()?.iter().collect())
    } else {
        Some(vec![member])
    }
}

/// Refuse a request that carries a retired *field* form, naming the successor
/// field. `None` — always, on every version this build speaks — while the
/// retirement is dormant, or when the request carries no registered killed field.
fn refuse_killed_field(request: &Request) -> Option<Response> {
    if !field_retirement_active(&request.dli) {
        return None;
    }
    let op = handlers::wire_name(request.op);
    KILLED_FIELDS
        .iter()
        .find(|(killed_op, field, _)| *killed_op == op && field_present(&request.params, field))
        .map(|(_, field, successor)| {
            typed_error(
                &request.dli,
                &request.id,
                ErrorCode::KilledForm,
                format!("'{op}' field '{field}' is a killed form; its successor is '{successor}'"),
            )
        })
}

/// The outcome of decoding one input line.
enum Frame {
    /// A blank or non-frame (log noise) line: skipped, never answered.
    Skip,
    /// A well-formed request naming a known op.
    Request(Box<Request>),
    /// A structurally broken frame; carries the best-effort id echo.
    Bad(String),
    /// A syntactically valid frame naming an op the contract does not define,
    /// with the successor when the op is a registered killed form.
    Unknown {
        /// The correlation id to echo.
        id: String,
        /// The offending op string.
        op: String,
        /// The successor op, when this is a killed form.
        successor: Option<&'static str>,
    },
    /// A well-formed frame carrying an unknown envelope field — a schema error
    /// (contract §2, unknown fields are errors), so a stray or mistyped field is
    /// never silently dropped.
    SchemaViolation {
        /// The correlation id to echo.
        id: String,
        /// The offending field path.
        field: String,
    },
}

/// Run the newline-delimited JSON serve loop over stdio: read one frame per line,
/// dispatch, write one response frame per line. Skipped noise lines produce no
/// output. The session cache and the Door T store live for the life of the loop.
pub fn serve_demo() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve_io(&mut stdin.lock(), &mut stdout.lock())
}

/// Run the newline-delimited worker over an explicitly loaded caller project.
pub fn serve_project(project: Project) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve_project_io(&mut stdin.lock(), &mut stdout.lock(), project)
}

/// The serve loop over an arbitrary reader/writer — the testable core of
/// [`serve_demo`], and the seam demo fixtures drive directly.
///
/// Lines are read as **raw bytes** (`read_until`) and lossy-decoded, never as
/// `str`: a single hostile non-UTF-8 byte must not abort the loop (a `lines()`
/// iterator would error out, killing the process and dropping every later
/// frame — a denial of service). A line that lossy-decodes to non-JSON is
/// skipped as noise; the stream never desyncs. Only a genuine I/O failure on the
/// underlying stream ends the loop.
pub fn serve_io<R: BufRead, W: Write>(reader: &mut R, writer: &mut W) -> io::Result<()> {
    let mut sessions = Sessions::new();
    let mut author_store = demo_author_store()?;
    serve_with_stores(reader, writer, &mut sessions, &mut author_store)
}

/// The serve loop over a caller project and arbitrary I/O.
pub fn serve_project_io<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    project: Project,
) -> io::Result<()> {
    let project = Arc::new(project);
    let mut sessions = Sessions::from_project(Arc::clone(&project));
    let mut author_store = project_author_store(&project)?;
    serve_with_stores(reader, writer, &mut sessions, &mut author_store)
}

fn serve_with_stores<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    sessions: &mut Sessions,
    author_store: &mut author::store::Store,
) -> io::Result<()> {
    let mut buf = Vec::new();

    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break; // EOF
        }
        let line = String::from_utf8_lossy(&buf);
        if let Some(response) = respond(&line, sessions, author_store) {
            let encoded = serde_json::to_string(&response).unwrap_or_else(|_| internal_line());
            writeln!(writer, "{encoded}")?;
            writer.flush()?;
            sessions.release_simulation_workers();
        }
    }
    Ok(())
}

/// Decode one input line and produce its response, or `None` for a skipped noise
/// line. This is the unit seam the dispatch tests drive: feed a raw line, thread
/// the session and store, read back the response.
#[must_use]
pub fn respond(
    line: &str,
    sessions: &mut Sessions,
    author_store: &mut author::store::Store,
) -> Option<Response> {
    match decode_frame(line) {
        Frame::Skip => None,
        Frame::Request(request) => Some(dispatch(&request, sessions, author_store)),
        Frame::Bad(id) => Some(bad_frame(&id)),
        Frame::Unknown { id, op, successor } => Some(refuse_op(&id, &op, successor)),
        Frame::SchemaViolation { id, field } => Some(schema_violation(&id, &field)),
    }
}

/// Route a decoded request to its handler and build the response. `author.*` ops
/// go through the frozen Door T seam; every `dli.*` op through the batch handlers.
/// A frame declaring the wrong contract version is refused here.
#[must_use]
pub fn dispatch(
    request: &Request,
    sessions: &mut Sessions,
    author_store: &mut author::store::Store,
) -> Response {
    if !crate::ACCEPTED_CONTRACT_VERSIONS.contains(&request.dli.as_str()) {
        return version_mismatch(request);
    }
    if !request.op.accepts_author_fences() {
        if request.expected_digest.is_some() {
            return fence_schema_violation(request, "expected_digest");
        }
        if request.idempotency_key.is_some() {
            return fence_schema_violation(request, "idempotency_key");
        }
    }
    // Field-level killed forms (LS-7). Dormant: `field_retirement_active` is
    // false for every accepted version, so this never fires on a wire this build
    // speaks. It sits on the live path so the mechanism is exercised, not
    // shelved, when contract 1.0 turns it on.
    if let Some(refusal) = refuse_killed_field(request) {
        return refusal;
    }
    if is_author_op(request.op) {
        return author::dispatch(request, author_store);
    }
    handlers::handle(request, sessions)
}

/// Construct the explicit demo Door T store, so the author read
/// tools (`author.query`/`check`/`why`) evaluate `base + staged` — the base world
/// the agent is authoring into, not the staged declarations alone (spec 11 §1). It
/// resolves the pack through the same [`crate::session::builtin_pack`] the read
/// surface uses, so the store base and the query scope never diverge.
pub(crate) fn demo_author_store() -> io::Result<author::store::Store> {
    let pack = crate::session::builtin_pack("camelot")
        .expect("the built-in camelot corpus is always present");
    author::store::Store::with_pack(pack).map_err(io::Error::other)
}

/// Construct a strict Door T store over every manifest-declared document.
/// When possible, attaches a lorestore at `<project_root>/.dli-lorestore/` so
/// `author.materialize` can commit staged fact objects durably (DL-0044, LS-4).
#[allow(clippy::print_stderr)] // diagnostics for operator, not a wire response
pub(crate) fn project_author_store(project: &Project) -> io::Result<author::store::Store> {
    let packs = project.documents().map(|(id, document)| {
        (
            id.to_owned(),
            document.pack.clone(),
            document.killed.clone(),
        )
    });
    let mut store = author::store::Store::with_packs(packs).map_err(io::Error::other)?;
    let lorestore_path = project.root().join(".dli-lorestore");
    if let Err(e) = store.attach_lorestore(lorestore_path.clone()) {
        eprintln!(
            "author: lorestore unavailable at {}: {}",
            lorestore_path.display(),
            e
        );
    }
    Ok(store)
}

// -- framing -----------------------------------------------------------------

/// Decode one line into a [`Frame`].
fn decode_frame(line: &str) -> Frame {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.starts_with('{') {
        return Frame::Skip;
    }
    // The strict decode rejects duplicate keys (serde's derived struct fails on a
    // repeated field), missing/mistyped envelope fields, and unknown/killed ops
    // (the `Op` enum admits no unlisted variant).
    match serde_json::from_str::<Request>(trimmed) {
        Ok(request) => match envelope_unknown_field(trimmed, &request) {
            // The Request struct tolerates extra top-level fields, so a stray or
            // typo'd field (e.g. a mistyped `expected_digest` that would defeat the
            // stale-fence) is silently dropped unless caught here — the envelope
            // half of the params round-trip.
            Some(field) => Frame::SchemaViolation {
                id: request.id,
                field,
            },
            None => Frame::Request(Box::new(request)),
        },
        Err(_) => classify_reject(trimmed),
    }
}

/// The first unknown top-level envelope field, if any: the decoded request
/// re-serialized and compared against the raw frame (mirrors the params-level
/// check in [`crate::handlers`]). `params` is an opaque value that round-trips
/// identically, so op-internal unknowns are left to the params check.
fn envelope_unknown_field(line: &str, request: &Request) -> Option<String> {
    let input: Value = serde_json::from_str(line).ok()?;
    let normalized = serde_json::to_value(request).ok()?;
    crate::handlers::first_unknown_field(&input, &normalized)
}

/// A frame that failed the strict decode: tell `unknown-op` (a well-formed op the
/// contract does not define) from `bad-frame` (everything else — broken JSON, a
/// duplicate key, a missing envelope field).
fn classify_reject(line: &str) -> Frame {
    // A lenient value parse still fails on truly broken JSON (a truncated frame),
    // which is a bad frame. A duplicate key parses here (last value wins) but
    // failed the strict decode above, so it lands as a bad frame below.
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Frame::Bad(String::new());
    };
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if let Some(op) = value.get("op").and_then(Value::as_str)
        && !is_known_op(op)
    {
        let successor = KILLED_OPS
            .iter()
            .find(|(killed, _)| *killed == op)
            .map(|(_, successor)| *successor);
        return Frame::Unknown {
            id,
            op: op.to_owned(),
            successor,
        };
    }
    Frame::Bad(id)
}

/// Whether an op string names a contract operation.
fn is_known_op(op: &str) -> bool {
    serde_json::from_value::<Op>(Value::String(op.to_owned())).is_ok()
}

/// Whether an op is an `author.*` tool (routed through the Door T seam).
fn is_author_op(op: Op) -> bool {
    serde_json::to_value(op)
        .ok()
        .and_then(|value| value.as_str().map(|s| s.starts_with("author.")))
        .unwrap_or(false)
}

// -- error responses ---------------------------------------------------------

/// A fallback line for the impossible case that a response cannot be serialized.
fn internal_line() -> String {
    format!(
        r#"{{"dli":"{CONTRACT_VERSION}","id":"","ok":false,"error":{{"code":"internal","message":"response could not be serialized"}}}}"#
    )
}

fn typed_error(dli: &str, id: &str, code: ErrorCode, message: String) -> Response {
    Response {
        dli: dli.to_owned(),
        id: id.to_owned(),
        ok: false,
        result: None,
        error: Some(ErrorBody {
            code,
            message,
            data: None,
        }),
    }
}

fn bad_frame(id: &str) -> Response {
    typed_error(
        CONTRACT_VERSION,
        id,
        ErrorCode::BadFrame,
        "the line is not a well-formed request frame".to_owned(),
    )
}

fn schema_violation(id: &str, field: &str) -> Response {
    typed_error(
        CONTRACT_VERSION,
        id,
        ErrorCode::SchemaViolation,
        format!("unknown envelope field '{field}' is not defined by the contract"),
    )
}

fn fence_schema_violation(request: &Request, field: &str) -> Response {
    typed_error(
        &request.dli,
        &request.id,
        ErrorCode::SchemaViolation,
        format!("envelope field '{field}' is defined only for mutating author.* operations"),
    )
}

/// Refuse an op the contract does not dispatch: a registered killed form is a
/// `killed-form` error naming its successor; anything else is `unknown-op`. Both
/// are refused, never silently dropped (`fantasy-contract.md` §3.1, §9).
fn refuse_op(id: &str, op: &str, successor: Option<&str>) -> Response {
    let (code, message) = match successor {
        Some(successor) => (
            ErrorCode::KilledForm,
            format!("'{op}' is a killed form; its successor is '{successor}'"),
        ),
        None => (
            ErrorCode::UnknownOp,
            format!("no operation named '{op}' is defined by the contract"),
        ),
    };
    typed_error(CONTRACT_VERSION, id, code, message)
}

fn version_mismatch(request: &Request) -> Response {
    typed_error(
        &request.dli,
        &request.id,
        ErrorCode::VersionMismatch,
        format!(
            "this worker speaks contracts {:?}, not '{}'; pin the vendored contract",
            crate::ACCEPTED_CONTRACT_VERSIONS,
            request.dli
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    fn request(dli: &str, op: Op, params: serde_json::Value) -> Request {
        Request {
            dli: dli.to_owned(),
            id: "1".to_owned(),
            op,
            params,
            expected_digest: None,
            idempotency_key: None,
        }
    }

    /// Every registered killed field names a real op and a successor distinct
    /// from the form it retires — the registry cannot drift into naming a
    /// successor that is the retired field itself.
    #[test]
    fn killed_fields_name_real_ops_and_distinct_successors() {
        for (op, field, successor) in KILLED_FIELDS {
            assert!(
                Op::ALL
                    .iter()
                    .any(|known| handlers::wire_name(*known) == *op),
                "killed field registry names an unknown op '{op}'"
            );
            assert_ne!(field, successor, "'{op}' retires a field to itself");
        }
    }

    /// The dormancy proof: field retirement is off for every version this build
    /// accepts, so no accepted wire can ever be refused for a killed field.
    #[test]
    fn field_retirement_is_dormant_on_every_accepted_version() {
        for version in crate::ACCEPTED_CONTRACT_VERSIONS {
            assert!(
                !field_retirement_active(version),
                "field retirement must be dormant on accepted version {version}"
            );
        }
    }

    /// (a) Dormant in practice: a `0.6.x` request carrying the retired string
    /// form draws no refusal, for every registered killed field.
    #[test]
    fn string_forms_are_accepted_under_current_versions() {
        let carriers = [
            (
                Op::Query,
                json!({ "document": "d", "goal": "serves(mara, ?who)" }),
            ),
            (
                Op::AuthorQuery,
                json!({ "workspace": "w", "goal": "serves(mara, ?who)" }),
            ),
            (
                Op::ForkAdd,
                json!({ "fork": "fork-1", "declarations": ["note(\"x\") proposed"] }),
            ),
            (
                Op::PayloadSubmit,
                json!({ "payload": { "claims": [{ "atom": "warded(arden)", "quote": "q", "line": 1 }] } }),
            ),
        ];
        for version in crate::ACCEPTED_CONTRACT_VERSIONS {
            for (op, params) in &carriers {
                let refusal = refuse_killed_field(&request(version, *op, params.clone()));
                assert!(
                    refusal.is_none(),
                    "{} at {version} must still accept its string form",
                    handlers::wire_name(*op)
                );
            }
        }
    }

    /// (b) The mechanism is correct, shown against a synthetic `1.0.0` wire no
    /// accepted-version path can reach: the refusal is `killed-form` and its
    /// message names the successor FIELD.
    #[test]
    fn synthetic_1_0_would_refuse_the_retired_field() {
        assert!(field_retirement_active("1.0.0"));
        let refusal = refuse_killed_field(&request(
            "1.0.0",
            Op::Query,
            json!({ "document": "d", "goal": "serves(mara, ?who)" }),
        ))
        .expect("a 1.0.0 request carrying `goal` is refused");
        let error = refusal.error.expect("killed-form error body");
        assert_eq!(error.code, ErrorCode::KilledForm);
        assert!(
            error.message.contains("'goal'") && error.message.contains("'goal_atom'"),
            "the refusal names the retired field and its successor: {}",
            error.message
        );
        assert_eq!(refusal.id, "1", "the refusal echoes the correlation id");
    }

    /// The typed successor is never itself a killed form: a `1.0.0` request that
    /// carries only `goal_atom` passes the field gate.
    #[test]
    fn synthetic_1_0_accepts_the_typed_successor() {
        let refusal = refuse_killed_field(&request(
            "1.0.0",
            Op::Query,
            json!({
                "document": "d",
                "goal_atom": { "pred": { "text": "serves", "span": { "file": 0, "start": 0, "end": 0 } }, "args": [] }
            }),
        ));
        assert!(
            refusal.is_none(),
            "the typed successor is not a killed form"
        );
    }

    /// The `[]` path segment means "some element of this array": a claim list
    /// where only one claim carries the retired `atom` is still a hit, and an
    /// empty claim list is not.
    #[test]
    fn array_paths_match_any_element() {
        let params = json!({
            "payload": { "claims": [
                { "atom_expr": null, "quote": "q", "line": 1 },
                { "atom": "warded(arden)", "quote": "q", "line": 2 },
            ] }
        });
        assert!(field_present(&params, "payload.claims[].atom"));
        assert!(!field_present(
            &json!({ "payload": { "claims": [] } }),
            "payload.claims[].atom"
        ));
        assert!(!field_present(
            &json!({ "payload": {} }),
            "payload.claims[].atom"
        ));
        assert!(!field_present(&json!({}), "goal"));
        assert!(field_present(&json!({ "goal": "serves(mara)" }), "goal"));
    }
}
