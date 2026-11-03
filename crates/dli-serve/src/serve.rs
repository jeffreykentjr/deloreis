//! The serve loop and request dispatch (`SERVE-1`).
//!
//! `deloreis serve` speaks newline-delimited JSON over stdio: one request frame
//! per line in, one response frame per line out (`docs/spec/10-serving.md` §1).
//! This module owns the framing and the routing; [`crate::handlers`] owns the
//! `dli.*` semantics and [`crate::author`] the `author.*` seam.
//!
//! Framing discipline:
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
/// `unknown-op` naming what replaced it.
const KILLED_OPS: &[(&str, &str)] = &[
    ("dli.suppose", "dli.fork.suppose"),
    ("dli.check-ledger", "dli.check-proof"),
    ("dli.check-draft", "contract 2.0.0"),
    ("dli.simulate.start", "contract 2.0.0"),
    ("dli.simulate.status", "contract 2.0.0"),
    ("dli.simulate.cancel", "contract 2.0.0"),
    ("dli.attestation-sweep", "contract 2.0.0"),
    ("dli.payload-submit", "contract 2.0.0"),
];

/// Retired marker spellings and their typed successor. The AST still reads
/// `Decided` in historical data; the current wire refuses new imports of it.
const KILLED_MARKERS: &[(&str, &str)] = &[("decided", "provisional")];

pub(crate) fn killed_marker_successor(marker: &str) -> Option<&'static str> {
    KILLED_MARKERS
        .iter()
        .find(|(retired, _)| *retired == marker)
        .map(|(_, successor)| *successor)
}

/// A retired wire field and its successor, keyed by operation.
struct KilledField {
    op: &'static str,
    field: &'static str,
    successor: &'static str,
}

const KILLED_FIELDS: &[KilledField] = &[
    KilledField {
        op: "dli.query",
        field: "goal",
        successor: "goal_atom",
    },
    KilledField {
        op: "author.query",
        field: "goal",
        successor: "goal_atom",
    },
    KilledField {
        op: "dli.fork.add",
        field: "declarations",
        successor: "additions",
    },
];

/// Whether a dotted path is carried by a params value. Arrays are traversed
/// element-wise without appearing in the path, matching
/// [`handlers::first_unknown_field`]'s convention: `payload.claims.atom` is
/// present when *some* claim carries `atom`.
fn field_present(value: &Value, path: &str) -> bool {
    match value {
        Value::Array(items) => items.iter().any(|item| field_present(item, path)),
        Value::Object(map) => match path.split_once('.') {
            Some((head, rest)) => map
                .get(head)
                .is_some_and(|inner| field_present(inner, rest)),
            None => map.contains_key(path),
        },
        _ => false,
    }
}

/// The successor for a retired field present in a request.
pub(crate) fn killed_field_successor(
    op: Op,
    params: &Value,
) -> Option<(&'static str, &'static str)> {
    let op = handlers::wire_name(op);
    KILLED_FIELDS
        .iter()
        .find(|killed| killed.op == op && field_present(params, killed.field))
        .map(|killed| (killed.field, killed.successor))
}

/// The `killed-form` message for a retired field, shaped like [`refuse_op`]'s
/// and naming the contract that retired it — a caller who sends the string
/// form is reading this to learn both what replaced it and which wire it still
/// works on.
pub(crate) fn killed_field_message(dli: &str, field: &str, successor: &str) -> String {
    format!("field '{field}' is a killed form at contract {dli}; its successor is '{successor}'")
}

/// Echo the current contract on pre-dispatch refusals. A frame that names a
/// retired or unknown version receives the current version in its response.
fn spoken_version(dli: &str) -> String {
    if crate::ACCEPTED_CONTRACT_VERSIONS.contains(&dli) {
        dli.to_owned()
    } else {
        CONTRACT_VERSION.to_owned()
    }
}

/// The outcome of decoding one input line.
enum Frame {
    /// A blank or non-frame (log noise) line: skipped, never answered.
    Skip,
    /// A well-formed request naming a known op.
    Request(Box<Request>),
    /// A structurally broken frame; carries the best-effort id echo.
    Bad(String),
    /// A retired contract version on a frame whose operation is also unknown.
    RetiredVersion { id: String, version: String },
    /// A syntactically valid frame naming an op the contract does not define,
    /// with the successor when the op is a registered killed form.
    Unknown {
        /// The negotiated version to echo.
        dli: String,
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
        /// The negotiated version to echo.
        dli: String,
        /// The correlation id to echo.
        id: String,
        /// The offending field path.
        field: String,
    },
}

/// Run the newline-delimited JSON serve loop over stdio: read one frame per line,
/// dispatch, write one response frame per line. Skipped noise lines produce no
/// output. The session cache and the authoring seam's store live for the life of the loop.
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
        Frame::RetiredVersion { id, version } => Some(refuse_version(&id, &version)),
        Frame::Unknown {
            dli,
            id,
            op,
            successor,
        } => Some(refuse_op(&dli, &id, &op, successor)),
        Frame::SchemaViolation { dli, id, field } => Some(schema_violation(&dli, &id, &field)),
    }
}

/// Route a decoded request to its handler and build the response. `author.*` ops
/// go through the frozen authoring seam; every `dli.*` op through the batch handlers.
/// A frame declaring the wrong contract version is refused here.
///
/// Retired fields are refused before routing, for both author and batch operations.
#[must_use]
pub fn dispatch(
    request: &Request,
    sessions: &mut Sessions,
    author_store: &mut author::store::Store,
) -> Response {
    if !crate::ACCEPTED_CONTRACT_VERSIONS.contains(&request.dli.as_str()) {
        return version_mismatch(request);
    }
    if let Some((field, successor)) = killed_field_successor(request.op, &request.params) {
        return refuse_field(request, field, successor);
    }
    // Fences are accepted on post-open authoring mutations.
    let fences_ok = request.op.accepts_author_fences();
    if !fences_ok {
        if request.expected_digest.is_some() {
            return fence_schema_violation(request, "expected_digest");
        }
        if request.idempotency_key.is_some() {
            return fence_schema_violation(request, "idempotency_key");
        }
    }
    if is_author_op(request.op) {
        return author::dispatch(request, author_store);
    }
    handlers::handle_with_store(request, sessions, Some(author_store))
}

/// Construct the explicit demo authoring-seam store, so the author read
/// tools (`author.query`/`check`/`why`) evaluate `base + staged` — the base world
/// the agent is authoring into, not the staged declarations alone (spec 11 §1). It
/// resolves the pack through the same [`crate::session::builtin_pack`] the read
/// surface uses, so the store base and the query scope never diverge.
pub(crate) fn demo_author_store() -> io::Result<author::store::Store> {
    let pack = crate::session::builtin_pack("camelot")
        .expect("the built-in camelot corpus is always present");
    author::store::Store::with_pack(pack).map_err(io::Error::other)
}

/// Construct a strict authoring-seam store over every manifest-declared document.
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
    // A store-backed document authors against the store it was loaded from, not
    // a copy of it (DL-0055), so its origin is registered before the attach.
    for (id, document) in project.documents() {
        if let Some(origin) = document.store_origin.as_ref() {
            store
                .register_store_origin(
                    id.to_owned(),
                    origin.clone(),
                    document.store_pinned,
                    document
                        .store_commit
                        .clone()
                        .expect("store-backed documents carry their selected commit"),
                    document.store_valuation.clone(),
                    document.store_unassigned.clone(),
                    document.store_ref_digest.clone(),
                )
                .map_err(io::Error::other)?;
        }
    }
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
        Ok(request) if retired_version(&request.dli) => Frame::RetiredVersion {
            id: request.id,
            version: request.dli,
        },
        Ok(request) => match envelope_unknown_field(trimmed, &request) {
            // The Request struct tolerates extra top-level fields, so a stray or
            // typo'd field (e.g. a mistyped `expected_digest` that would defeat the
            // stale-fence) is silently dropped unless caught here — the envelope
            // half of the params round-trip.
            Some(field) => Frame::SchemaViolation {
                dli: spoken_version(&request.dli),
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
    // Decode the envelope again with an untyped op, retaining serde's checks
    // for missing, mistyped, and duplicate envelope fields. A Value-only parse
    // would silently collapse duplicate keys before the unknown-op check.
    #[derive(serde::Deserialize)]
    struct RawEnvelope {
        dli: String,
        id: String,
        op: String,
        #[serde(default, rename = "params")]
        _params: Value,
        #[serde(default, rename = "expected_digest")]
        _expected_digest: Option<String>,
        #[serde(default, rename = "idempotency_key")]
        _idempotency_key: Option<String>,
    }
    let Ok(raw) = serde_json::from_str::<RawEnvelope>(line) else {
        // Recover only an unambiguous string id for the error response. This
        // lenient projection never decides whether the frame is valid.
        #[derive(serde::Deserialize)]
        struct CorrelationOnly {
            id: Option<String>,
        }
        let id = serde_json::from_str::<CorrelationOnly>(line)
            .ok()
            .and_then(|frame| frame.id)
            .unwrap_or_default();
        return Frame::Bad(id);
    };
    let id = raw.id;
    if !is_known_op(&raw.op) {
        if retired_version(&raw.dli) {
            return Frame::RetiredVersion {
                id,
                version: raw.dli,
            };
        }
        let successor = KILLED_OPS
            .iter()
            .find(|(killed, _)| *killed == raw.op)
            .map(|(_, successor)| *successor);
        return Frame::Unknown {
            dli: spoken_version(&raw.dli),
            id,
            op: raw.op,
            successor,
        };
    }
    Frame::Bad(id)
}

/// Whether an op string names a contract operation.
fn is_known_op(op: &str) -> bool {
    serde_json::from_value::<Op>(Value::String(op.to_owned())).is_ok()
}

/// Whether an op is an `author.*` tool (routed through the authoring seam).
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

fn schema_violation(dli: &str, id: &str, field: &str) -> Response {
    typed_error(
        dli,
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
/// are refused, never silently dropped.
fn refuse_op(dli: &str, id: &str, op: &str, successor: Option<&str>) -> Response {
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
    typed_error(dli, id, code, message)
}

/// Refuse a request carrying a retired *field*, the twin of [`refuse_op`]: a
/// registered killed form is `killed-form` naming its successor, never a silent
/// acceptance and never an anonymous schema violation. The negotiated version
/// is echoed, so the refusal identifies the wire it was made on.
fn refuse_field(request: &Request, field: &str, successor: &str) -> Response {
    typed_error(
        &request.dli,
        &request.id,
        ErrorCode::KilledForm,
        killed_field_message(&request.dli, field, successor),
    )
}

/// The previously served version families are retired forms, not unknown versions.
fn retired_version(version: &str) -> bool {
    let Some((major, minor, _patch)) = version.split_once('.').and_then(|(major, rest)| {
        let (minor, patch) = rest.split_once('.')?;
        Some((
            major.parse::<u32>().ok()?,
            minor.parse::<u32>().ok()?,
            patch.parse::<u32>().ok()?,
        ))
    }) else {
        return false;
    };
    version == "0.5.1" || (major == 0 && minor == 6) || major == 1
}

fn version_mismatch(request: &Request) -> Response {
    if retired_version(&request.dli) {
        return refuse_version(&request.id, &request.dli);
    }
    typed_error(
        CONTRACT_VERSION,
        &request.id,
        ErrorCode::VersionMismatch,
        format!(
            "this worker speaks contract {CONTRACT_VERSION}, not '{}'",
            request.dli
        ),
    )
}

fn refuse_version(id: &str, version: &str) -> Response {
    typed_error(
        CONTRACT_VERSION,
        id,
        ErrorCode::KilledForm,
        format!("contract '{version}' is a killed form; its successor is contract 2.0.0"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn killed_fields_name_real_ops_and_distinct_successors() {
        for killed in KILLED_FIELDS {
            assert!(
                Op::ALL
                    .iter()
                    .any(|op| handlers::wire_name(*op) == killed.op)
            );
            assert_ne!(killed.field, killed.successor);
        }
    }

    #[test]
    fn retired_decided_marker_names_its_successor() {
        assert_eq!(killed_marker_successor("decided"), Some("provisional"));
        assert_eq!(killed_marker_successor("ratified"), None);
    }

    /// The refusal names the successor FIELD, and the wire it was made on.
    #[test]
    fn contract_2_names_the_successor_field() {
        let (field, successor) = killed_field_successor(
            Op::Query,
            &json!({ "document": "d", "goal": "serves(lyra, ?who)" }),
        )
        .expect("`goal` is registered as retired for dli.query");
        assert_eq!((field, successor), ("goal", "goal_atom"));

        let message = killed_field_message("2.0.0", field, successor);
        assert!(
            message.contains("'goal'") && message.contains("'goal_atom'"),
            "the refusal names the retired field and its successor: {message}"
        );
        assert!(
            message.contains("killed form"),
            "the message is shaped like refuse_op's: {message}"
        );
        assert!(
            message.contains("2.0.0"),
            "the refusal names the wire it was made on: {message}"
        );
    }

    /// A typed successor is never itself a killed form, and an unregistered op
    /// carrying a same-named field is not caught by another op's entry.
    #[test]
    fn only_the_registered_pair_matches() {
        assert!(
            killed_field_successor(
                Op::Query,
                &json!({ "document": "d", "goal_atom": { "pred": "x", "args": [] } }),
            )
            .is_none(),
            "the typed successor is not a killed form"
        );
        assert!(
            killed_field_successor(Op::Check, &json!({ "goal": "serves(lyra)" })).is_none(),
            "`goal` is retired for dli.query, not for every op that might carry it"
        );
    }

    /// The path shape the registry uses is the one the decode seam actually
    /// produces — pinned against `first_unknown_field` itself, so the two cannot
    /// drift into disagreeing about how a nested field is named.
    #[test]
    fn registry_paths_match_the_unknown_field_reporter() {
        let given = json!({
            "payload": { "claims": [{ "atom": "warded(arden)", "quote": "q", "line": 1 }] }
        });
        let kept = json!({
            "payload": { "claims": [{ "quote": "q", "line": 1 }] }
        });
        assert_eq!(
            handlers::first_unknown_field(&given, &kept).as_deref(),
            Some("payload.claims.atom"),
            "the registry path is spelled the way the decode seam reports it"
        );
    }
}
