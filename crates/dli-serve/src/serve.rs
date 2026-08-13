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

/// Retired stored marker spellings and their typed successor. The AST keeps
/// `Decided` readable for one compatibility cycle, while contract 1.5 refuses
/// a newly imported occurrence through the same killed-form vocabulary used by
/// retired operations and fields.
const KILLED_MARKERS: &[(&str, &str)] = &[("decided", "provisional")];

pub(crate) fn killed_marker_successor(dli: &str, marker: &str) -> Option<&'static str> {
    if !crate::has_standing_tools(dli) {
        return None;
    }
    KILLED_MARKERS
        .iter()
        .find(|(retired, _)| *retired == marker)
        .map(|(_, successor)| *successor)
}

/// The contract version at which the retired *field* forms in [`KILLED_FIELDS`]
/// start being refused. `1.0.0` is the one accepted version that reaches it
/// (LS-8), so the registry is live on exactly that wire and dormant on the
/// whole `0.x` prefix.
const FIELD_RETIREMENT_MAJOR: u32 = 1;

/// One retired wire *field* and what replaced it.
struct KilledField {
    /// The wire op the retirement applies to. A retirement is keyed on the
    /// `(op, field)` pair, never on the field name alone: `author.why` also
    /// takes an `atom`, and it is not this one.
    op: &'static str,
    /// The retired field, as a dotted path from `params` in the same shape
    /// [`handlers::first_unknown_field`] reports — an array is traversed
    /// silently, so `payload.claims.atom` names the `atom` of a claim.
    field: &'static str,
    /// The typed field that replaces it.
    successor: &'static str,
    /// The **required** fields serde would have named before this one, had the
    /// retired field still been declared required — its own struct's earlier
    /// required siblings, as dotted paths from `params`.
    ///
    /// This is what keeps [`required_retired_field`] from stealing serde's
    /// first word below 1.0. `dli.query` with empty params must still report
    /// `document`, because `document` is declared before `goal`; a `Claim`
    /// missing both `atom` and `quote` must still report `atom`, because
    /// `atom` is declared first. The list is empty for a **nested**
    /// registration even when the outer struct has earlier required fields: a
    /// nested decode fails while the outer map is still being visited, so it
    /// beats the outer struct's end-of-map missing-field check outright.
    preceded_by: &'static [&'static str],
    /// The fields whose missing-field error the retired field **out-ranks** —
    /// everything serde would only have reached *after* naming the retired one.
    ///
    /// `preceded_by` alone is not enough, because "the decode failed with some
    /// missing field" does not say which struct that field belongs to. A
    /// malformed typed successor fails **deeper** — `goal_atom: {"args": []}` is
    /// a missing `pred` on `AtomExpr` — and that error was raised while this
    /// struct's map was still being visited, so it beat the retired field to the
    /// answer when the field was required and must still beat it now. Matching
    /// the named field against this list tells "what I out-rank" from "what
    /// out-ranks me": `pred` and `args` are on no list here, so they always win.
    ///
    /// Two kinds of name belong on the list, and a **nested** registration
    /// carries both:
    ///
    /// - The retired field's own later siblings — a `Claim`'s `quote` and
    ///   `line`, which serde reaches only once `atom` is satisfied.
    /// - For a nested field, every field of the **enclosing** struct. A nested
    ///   decode runs while the outer map is still being visited, so it fires
    ///   before the outer struct's end-of-map check: a submission missing
    ///   `document` AND a claim's `atom` named `atom`, not `document`.
    followed_by: &'static [&'static str],
}

/// The killed wire forms at *field* granularity. Where [`KILLED_OPS`] retires a
/// whole operation, this retires one text-carrying parameter in favour of its
/// typed successor — the LS-7 surface retirement, activated at contract 1.0 by
/// LS-8 (`docs/design-notes/lorestore-roadmap.md` §LS-7, §LS-8).
///
/// The retirement is **two-sided**, and both sides read this one table:
///
/// - From contract `1.0`, [`killed_field_successor`] matches the field and
///   [`dispatch`] refuses the request with `killed-form`, naming the successor.
/// - Before `1.0`, [`required_retired_field`] requires it — the string form is
///   still the only way to say the thing, exactly as its params struct required
///   it before 1.0 made the field optional. A `0.x` request omitting it is the
///   same schema violation it always was, in the same words.
///
/// So the version gate is not a lock over an inert table; it is the hinge
/// between two live behaviours, and neither leaks onto the other's wire.
///
/// **A registration is per-op, but a params *type* can be shared.** `Claim` is
/// carried by both `dli.payload-submit` and `dli.check-draft`, so both are
/// registered: one entry would retire the field on one wire and silently
/// un-require it on the other, which is precisely the hole
/// `tests/ls8_zero_x_parity.rs` exists to catch.
const KILLED_FIELDS: &[KilledField] = &[
    KilledField {
        op: "dli.query",
        field: "goal",
        successor: "goal_atom",
        preceded_by: &["document"],
        followed_by: &["goal_atom", "layer", "as_of", "fork", "why"],
    },
    KilledField {
        op: "author.query",
        field: "goal",
        successor: "goal_atom",
        preceded_by: &["workspace"],
        followed_by: &["goal_atom", "layer", "why"],
    },
    KilledField {
        op: "dli.fork.add",
        field: "declarations",
        successor: "additions",
        preceded_by: &["fork"],
        followed_by: &["additions"],
    },
    KilledField {
        op: "dli.payload-submit",
        field: "payload.claims.atom",
        successor: "atom_expr",
        preceded_by: &[],
        // `Claim`'s later siblings, then `PayloadSubmitParams`'s own fields:
        // the claim fails while the outer map is still being visited.
        followed_by: &[
            "atom_expr",
            "quote",
            "line",
            "document",
            "payload",
            "source",
        ],
    },
    KilledField {
        op: "dli.check-draft",
        field: "payload.claims.atom",
        successor: "atom_expr",
        preceded_by: &[],
        // As above, over `CheckDraftParams`'s fields.
        followed_by: &[
            "atom_expr",
            "quote",
            "line",
            "document",
            "payload",
            "source",
            "continuity",
        ],
    },
];

/// Whether field-level killed forms are enforced on a negotiated wire version.
/// True only from contract [`FIELD_RETIREMENT_MAJOR`].0 up — so true for
/// `1.0.0` and false for the whole `0.x` prefix, which is the single switch
/// both halves of the retirement turn on.
fn field_retirement_active(dli: &str) -> bool {
    dli.split('.')
        .next()
        .and_then(|major| major.parse::<u32>().ok())
        .is_some_and(|major| major >= FIELD_RETIREMENT_MAJOR)
}

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

/// Whether a dotted path is carried by **every** place that can carry it —
/// the complement [`required_retired_field`] needs, which is not the negation
/// of [`field_present`]: an array where one claim carries `atom` and another
/// does not is neither wholly present nor wholly absent, and only the strict
/// reading keeps the pre-1.0 wire as strict as its params struct was.
///
/// An intermediate the path cannot reach is *not* this check's business — the
/// params decoded before it runs, so the structure above the field is already
/// valid and an absent parent means the field has no place to be.
fn field_carried(value: &Value, path: &str) -> bool {
    match value {
        Value::Array(items) => items.iter().all(|item| field_carried(item, path)),
        Value::Object(map) => match path.split_once('.') {
            Some((head, rest)) => map.get(head).is_none_or(|inner| field_carried(inner, rest)),
            None => map.contains_key(path),
        },
        _ => true,
    }
}

/// The successor field naming a retired form the request still carries, or
/// `None` when the field is not registered or the wire predates the
/// retirement. `Some` only from contract 1.0 up (LS-8) — on the whole `0.x`
/// prefix this is `None` and the string form is answered as it always was.
pub(crate) fn killed_field_successor(
    dli: &str,
    op: Op,
    params: &Value,
) -> Option<(&'static str, &'static str)> {
    if !field_retirement_active(dli) {
        return None;
    }
    let op = handlers::wire_name(op);
    KILLED_FIELDS
        .iter()
        .find(|killed| killed.op == op && field_present(params, killed.field))
        .map(|killed| (killed.field, killed.successor))
}

/// The retired field a **pre-1.0** request must carry but does not — the other
/// side of the same registry. Contract 1.0 makes the string form optional so
/// the typed successor can be sent alone; on every earlier wire the field is
/// still required, so this restores the exact `missing field` verdict the
/// params struct produced before it was. `None` from 1.0 up, where the field is
/// refused rather than required.
///
/// Three guards decide it, and each answers a different question about what
/// serde would have said when the field was still declared required:
///
/// - **Is the field absent?** [`field_carried`], element-wise across arrays.
/// - **Would serde have named an EARLIER field?** [`KilledField::preceded_by`].
///   `dli.query` with empty params must still report `document`.
/// - **Did the decode fail for something this field out-ranks?**
///   [`KilledField::followed_by`]. A decode that succeeded needs no answer here;
///   a decode that failed for a *sibling declared later* (a `Claim`'s `quote`)
///   is the retired field's to pre-empt; a decode that failed for anything else
///   — an unknown key, a type error, or a missing field on a **nested** type
///   like `AtomExpr`'s `pred` — was raised while this struct's map was still
///   being visited, so it beat the retired field to the answer then and keeps
///   it now.
pub(crate) fn required_retired_field(
    dli: &str,
    op: Op,
    params: &Value,
    decode_error: Option<&serde_json::Error>,
) -> Option<&'static str> {
    if field_retirement_active(dli) {
        return None;
    }
    let missing = decode_error.and_then(missing_field_name);
    let op = handlers::wire_name(op);
    KILLED_FIELDS
        .iter()
        .find(|killed| {
            killed.op == op
                && !field_carried(params, killed.field)
                && killed
                    .preceded_by
                    .iter()
                    .all(|earlier| field_carried(params, earlier))
                && match (decode_error, missing.as_deref()) {
                    // The decode stands; nothing raced the retired field.
                    (None, _) => true,
                    // A sibling declared after it — its to answer instead.
                    (Some(_), Some(named)) => killed.followed_by.contains(&named),
                    // Anything else serde raised first, including a missing
                    // field belonging to some nested type.
                    (Some(_), None) => false,
                }
        })
        .map(|killed| killed.field)
}

/// The field named by a serde **missing field** error, or `None` for any other
/// error class.
///
/// Reading the message is the coupling it looks like, and it is the same
/// coupling [`required_retired_field`]'s callers already carry by reproducing
/// serde's wording verbatim. `the_missing_field_wording_is_serdes_own` pins it
/// against the live serde, so a wording change fails loudly rather than
/// silently widening what may be pre-empted — this returning `None` can only
/// ever make the retirement yield.
fn missing_field_name(error: &serde_json::Error) -> Option<String> {
    // `from_value` renders exactly ``missing field `x` ``; `from_str` appends a
    // line and column, so the name is read up to the closing backtick rather
    // than to the end.
    error
        .to_string()
        .strip_prefix("missing field `")?
        .split('`')
        .next()
        .map(str::to_owned)
}

/// The leaf name of a registry path, as serde names a missing field. Serde
/// names the field alone and carries no path, so a nested registration
/// (`payload.claims.atom`) must report `atom` to keep the pre-1.0 message
/// byte-identical.
pub(crate) fn field_leaf(field: &str) -> &str {
    field.rsplit('.').next().unwrap_or(field)
}

/// The `killed-form` message for a retired field, shaped like [`refuse_op`]'s
/// and naming the contract that retired it — a caller who sends the string
/// form is reading this to learn both what replaced it and which wire it still
/// works on.
pub(crate) fn killed_field_message(dli: &str, field: &str, successor: &str) -> String {
    format!("field '{field}' is a killed form at contract {dli}; its successor is '{successor}'")
}

/// The version a pre-dispatch refusal echoes: the frame's own, when the frame
/// named one this worker actually speaks, and the build's otherwise.
///
/// A refusal raised *before* [`dispatch`] — an unknown op, an unknown envelope
/// field — has no negotiated version to echo, so it used to answer with
/// [`CONTRACT_VERSION`]. That made a framing refusal drift with every version
/// bump: the same `0.5.1` unknown-op answered `0.5.1` under one build and
/// `0.6.4` under the next, having nothing to do with the caller's wire. Echoing
/// the frame's own version instead pins the answer to the conversation, and
/// falling back to [`CONTRACT_VERSION`] keeps the invariant that a response
/// never claims a version this worker cannot speak.
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
/// A **killed field** is refused here too, before either seam. It has to be:
/// the retired forms are still declared by their params structs — that is what
/// keeps the `0.x` wire whole — so a `1.0` request carrying one would decode
/// cleanly and be answered. Version-gated (`field_retirement_active`), the
/// check is a no-op on every `0.x` request and covers `author.*` and `dli.*`
/// alike by sitting above the routing fork.
#[must_use]
pub fn dispatch(
    request: &Request,
    sessions: &mut Sessions,
    author_store: &mut author::store::Store,
) -> Response {
    if !crate::ACCEPTED_CONTRACT_VERSIONS.contains(&request.dli.as_str()) {
        return version_mismatch(request);
    }
    if request.op.added_in_1_5_0() && !crate::has_standing_tools(&request.dli) {
        return typed_error(
            &request.dli,
            &request.id,
            ErrorCode::UnknownOp,
            format!(
                "operation {:?} is not available before contract 1.5.0",
                request.op
            ),
        );
    }
    if request.op.added_in_1_6_0() && !crate::has_functional_decls(&request.dli) {
        return typed_error(
            &request.dli,
            &request.id,
            ErrorCode::UnknownOp,
            format!(
                "operation {:?} is not available before contract 1.6.0",
                request.op
            ),
        );
    }
    if request.op.added_in_1_7_0() {
        // Every 1.7.0 op gates on its OWN floor predicate (the campaign's
        // three-floor design: `has_identity_assertions`, `has_rule_authoring`,
        // `has_branch_writes`) rather than one blanket flag, even though every
        // floor is `at_or_above(dli, (1, 7, 0))` today — so a later divergent
        // floor is a one-line change here, never a silent capability leak.
        let available = match request.op {
            Op::AuthorAddRule => crate::has_rule_authoring(&request.dli),
            _ => crate::has_identity_assertions(&request.dli),
        };
        if !available {
            return typed_error(
                &request.dli,
                &request.id,
                ErrorCode::UnknownOp,
                format!(
                    "operation {:?} is not available before contract 1.7.0",
                    request.op
                ),
            );
        }
    }
    if request.op.added_in_1_8_0() && !crate::has_inertia(&request.dli) {
        return typed_error(
            &request.dli,
            &request.id,
            ErrorCode::UnknownOp,
            format!(
                "operation {:?} is not available before contract 1.8.0",
                request.op
            ),
        );
    }
    if request.op.added_in_1_9_0() {
        // Every 1.9.0 op gates on its OWN floor predicate (the same per-op-floor
        // discipline the 1.7.0 block follows): `has_as_of_read` for the point
        // read, `has_decisions` for the baseline recorder. Both are
        // `at_or_above(dli, (1, 9, 0))` today, so a later divergent floor is a
        // one-line change here, never a silent capability leak.
        let available = match request.op {
            Op::AuthorRecordDecision => crate::has_decisions(&request.dli),
            _ => crate::has_as_of_read(&request.dli),
        };
        if !available {
            return typed_error(
                &request.dli,
                &request.id,
                ErrorCode::UnknownOp,
                format!(
                    "operation {:?} is not available before contract 1.9.0",
                    request.op
                ),
            );
        }
    }
    if let Some((field, successor)) =
        killed_field_successor(&request.dli, request.op, &request.params)
    {
        return refuse_field(request, field, successor);
    }
    // `author.branch`/`author.merge` accept the envelope fences ONLY from
    // contract 1.7.0 up (WS-3: they moved from the `read` wrapper to `mutate`,
    // so idempotency-key replay now actually caches). This is a version-gated
    // exception, not a change to `accepts_author_fences` itself — blanket-
    // adding them there would flip a fence-refusal on every wire below 1.7.0,
    // moving released bytes (the same C4 discipline every other capability
    // gate here follows).
    let fences_ok = request.op.accepts_author_fences()
        || (matches!(request.op, Op::AuthorBranch | Op::AuthorMerge)
            && crate::has_branch_writes(&request.dli));
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
    handlers::handle(request, sessions)
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
            store.register_store_origin(
                id.to_owned(),
                origin.clone(),
                document.store_pinned,
                document
                    .store_commit
                    .clone()
                    .expect("store-backed documents carry their selected commit"),
                document.store_valuation.clone(),
                document.store_ref_digest.clone(),
            );
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
            dli: spoken_version(value.get("dli").and_then(Value::as_str).unwrap_or_default()),
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
/// are refused, never silently dropped (`fantasy-contract.md` §3.1, §9).
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

    /// Every registered killed field names a real op and a successor distinct
    /// from the form it retires — the registry cannot drift into naming a
    /// successor that is the retired field itself. Its `preceded_by` fields are
    /// paths under the same params, never the retired field again.
    #[test]
    fn killed_fields_name_real_ops_and_distinct_successors() {
        for killed in KILLED_FIELDS {
            let op = killed.op;
            assert!(
                Op::ALL
                    .iter()
                    .any(|known| handlers::wire_name(*known) == op),
                "killed field registry names an unknown op '{op}'"
            );
            assert_ne!(
                killed.field, killed.successor,
                "'{op}' retires a field to itself"
            );
            for earlier in killed.preceded_by {
                assert_ne!(
                    *earlier, killed.field,
                    "'{op}' lists its own retired field as preceding it"
                );
            }
            // A nested registration is reached through the outer map, so it
            // out-ranks the outer struct's missing fields outright and can
            // never need a predecessor.
            if killed.field.contains('.') {
                assert!(
                    killed.preceded_by.is_empty(),
                    "'{op}': a nested retirement beats the outer struct's end-of-map check"
                );
            }
        }
    }

    #[test]
    fn decided_marker_is_killed_only_on_contract_1_5() {
        assert_eq!(killed_marker_successor("1.4.0", "decided"), None);
        assert_eq!(
            killed_marker_successor("1.5.0", "decided"),
            Some("provisional")
        );
        assert_eq!(killed_marker_successor("1.5.0", "ratified"), None);
    }

    /// The version gate reads the major and nothing else: live on `1.0.0` — the
    /// one accepted version that reaches it — and off across the whole `0.x`
    /// prefix. A minor bump can never wake the registry by accident.
    #[test]
    fn field_retirement_is_keyed_on_the_major_alone() {
        for version in crate::ACCEPTED_CONTRACT_VERSIONS {
            assert_eq!(
                field_retirement_active(version),
                version.starts_with("1."),
                "field retirement at accepted version {version}"
            );
        }
        assert!(field_retirement_active("1.0.0"));
        assert!(!field_retirement_active("0.9.9"));
        assert!(field_retirement_active("2.0.0"));
    }

    /// Every retired field is still DECLARED by its params struct — that is
    /// what keeps the `0.x` wire whole — so it decodes cleanly and never
    /// becomes an "unknown field". The refusal therefore cannot come from the
    /// decode seam; it comes from [`dispatch`], which consults the registry
    /// before routing. Here: the registry matches at `1.0.0` and matches
    /// nowhere below it.
    #[test]
    fn the_registry_matches_at_1_0_and_nowhere_below() {
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
        ];
        for (op, params) in &carriers {
            assert!(
                killed_field_successor("1.0.0", *op, params).is_some(),
                "{} carries its registered retired field",
                handlers::wire_name(*op)
            );
            for version in crate::ACCEPTED_CONTRACT_VERSIONS {
                if version.starts_with("1.") {
                    continue;
                }
                assert!(
                    killed_field_successor(version, *op, params).is_none(),
                    "{} at {version} must not reach the killed-field registry",
                    handlers::wire_name(*op)
                );
            }
        }
    }

    /// The refusal names the successor FIELD, and the wire it was made on.
    #[test]
    fn contract_1_0_names_the_successor_field() {
        let (field, successor) = killed_field_successor(
            "1.0.0",
            Op::Query,
            &json!({ "document": "d", "goal": "serves(mara, ?who)" }),
        )
        .expect("`goal` is registered as retired for dli.query");
        assert_eq!((field, successor), ("goal", "goal_atom"));

        let message = killed_field_message("1.0.0", field, successor);
        assert!(
            message.contains("'goal'") && message.contains("'goal_atom'"),
            "the refusal names the retired field and its successor: {message}"
        );
        assert!(
            message.contains("killed form"),
            "the message is shaped like refuse_op's: {message}"
        );
        assert!(
            message.contains("1.0.0"),
            "the refusal names the wire it was made on: {message}"
        );
    }

    /// The other side of the same table: before 1.0 the retired field is
    /// REQUIRED, and the requirement is spelled the way serde spelled it — a
    /// nested registration reports its leaf, because `serde_json::from_value`
    /// names the missing field alone and carries no path.
    #[test]
    fn the_retired_field_is_required_below_1_0_and_refused_at_it() {
        let missing = json!({ "document": "d", "goal_atom": { "pred": "x", "args": [] } });
        for version in crate::ACCEPTED_CONTRACT_VERSIONS {
            let required = required_retired_field(version, Op::Query, &missing, None);
            if version.starts_with("1.") {
                assert_eq!(required, None, "1.0 does not require a retired field");
            } else {
                assert_eq!(
                    required,
                    Some("goal"),
                    "{version} still requires the string form"
                );
            }
        }
        // Carried: nothing to require, on either side of the hinge.
        let carried = json!({ "document": "d", "goal": "serves(mara)" });
        assert_eq!(
            required_retired_field("0.6.4", Op::Query, &carried, None),
            None
        );
        assert_eq!(field_leaf("payload.claims.atom"), "atom");
        assert_eq!(field_leaf("goal"), "goal");
    }

    /// Every guard is coupled to serde's exact wording — the message the
    /// callers reproduce, and the name [`missing_field_name`] reads back out of
    /// the error to decide what may be pre-empted — so the wording is pinned
    /// against the live serde rather than assumed. A serde that renamed this
    /// error would fail here, loudly, instead of silently changing which field
    /// a malformed request is told about.
    #[test]
    fn the_missing_field_wording_is_serdes_own() {
        #[derive(Debug, serde::Deserialize)]
        #[allow(dead_code)]
        struct Probe {
            first: String,
            second: String,
        }

        let error = serde_json::from_value::<Probe>(json!({ "second": "x" }))
            .expect_err("`first` is missing");
        assert_eq!(
            error.to_string(),
            "missing field `first`",
            "the message `required_retired_field`'s callers reproduce verbatim"
        );
        assert_eq!(missing_field_name(&error).as_deref(), Some("first"));

        // Serde names the FIRST missing field in declaration order — the whole
        // premise of `KilledField::preceded_by`.
        let error =
            serde_json::from_value::<Probe>(json!({})).expect_err("both fields are missing");
        assert_eq!(missing_field_name(&error).as_deref(), Some("first"));

        // An error raised while the map was still being visited is not a
        // missing-field error at all, so nothing may pre-empt it.
        let error = serde_json::from_value::<Probe>(json!({ "first": 7, "second": "x" }))
            .expect_err("`first` is not a string");
        assert_eq!(
            missing_field_name(&error),
            None,
            "a type error is nobody's to out-rank: {error}"
        );

        // `from_str` appends a line and column; the name is still read cleanly.
        let error = serde_json::from_str::<Probe>(r#"{"second":"x"}"#).expect_err("`first`");
        assert!(error.to_string().contains("at line"), "{error}");
        assert_eq!(missing_field_name(&error).as_deref(), Some("first"));
    }

    /// The blocking case the second red team found: a decode that failed
    /// **deeper** is not the retired field's to answer. `goal_atom: {"args":[]}`
    /// is a missing `pred` on `AtomExpr`, raised while `QueryParams`'s own map
    /// was still being visited — so it out-ranked the retired `goal` when
    /// `goal` was required, and it must still out-rank it.
    ///
    /// The distinction is `followed_by`: `pred` and `args` are not siblings of
    /// `goal`, so they are never pre-empted, while a `Claim`'s `quote` is and is.
    #[test]
    fn a_nested_decode_failure_is_never_pre_empted() {
        let malformed = json!({ "document": "d", "goal_atom": { "args": [] } });
        let nested = serde_json::from_value::<crate::contract::QueryParams>(malformed.clone())
            .expect_err("`pred` is missing from the typed successor");
        assert_eq!(missing_field_name(&nested).as_deref(), Some("pred"));
        assert_eq!(
            required_retired_field("0.6.4", Op::Query, &malformed, Some(&nested)),
            None,
            "the nested `pred` answers, not the retired `goal`"
        );

        // A sibling declared after the retired field IS pre-empted: this is the
        // `Claim` case, where serde named `quote` only because `atom` stopped
        // being required.
        let claim = json!({ "payload": { "claims": [{ "line": 1 }] } });
        let sibling = serde_json::from_value::<dli_world::payload::Claim>(json!({ "line": 1 }))
            .expect_err("`quote` is missing");
        assert_eq!(missing_field_name(&sibling).as_deref(), Some("quote"));
        assert_eq!(
            required_retired_field("0.6.4", Op::PayloadSubmit, &claim, Some(&sibling)),
            Some("payload.claims.atom"),
            "`quote` is a sibling declared after `atom`, so `atom` answers"
        );
    }

    /// A claims array is required element-wise: one claim carrying `atom` does
    /// not excuse the next. `field_present` (any) and `field_carried` (all) are
    /// genuinely different questions, and the pre-1.0 wire needs the strict one
    /// or a mixed array would slip through where the params struct refused it.
    #[test]
    fn an_array_requires_the_retired_field_of_every_element() {
        let mixed = json!({ "payload": { "claims": [
            { "atom": "warded(arden)", "quote": "q", "line": 1 },
            { "atom_expr": null, "quote": "q", "line": 2 },
        ] } });
        assert_eq!(
            required_retired_field("0.6.4", Op::PayloadSubmit, &mixed, None),
            Some("payload.claims.atom"),
            "every claim must carry the string form before 1.0"
        );
        let all = json!({ "payload": { "claims": [
            { "atom": "warded(arden)", "quote": "q", "line": 1 },
            { "atom": "warded(brac)", "quote": "q", "line": 2 },
        ] } });
        assert_eq!(
            required_retired_field("0.6.4", Op::PayloadSubmit, &all, None),
            None
        );
        // An empty array requires nothing, and neither does an absent parent:
        // the params already decoded, so there is no place for the field to be.
        for vacuous in [
            json!({ "payload": { "claims": [] } }),
            json!({ "payload": {} }),
            json!({}),
        ] {
            assert_eq!(
                required_retired_field("0.6.4", Op::PayloadSubmit, &vacuous, None),
                None,
                "nothing to require: {vacuous}"
            );
        }
    }

    /// A typed successor is never itself a killed form, and an unregistered op
    /// carrying a same-named field is not caught by another op's entry.
    #[test]
    fn only_the_registered_pair_matches() {
        assert!(
            killed_field_successor(
                "1.0.0",
                Op::Query,
                &json!({ "document": "d", "goal_atom": { "pred": "x", "args": [] } }),
            )
            .is_none(),
            "the typed successor is not a killed form"
        );
        assert!(
            killed_field_successor("1.0.0", Op::Check, &json!({ "goal": "serves(mara)" }))
                .is_none(),
            "`goal` is retired for dli.query, not for every op that might carry it"
        );
    }

    /// Registry paths are dotted exactly as `first_unknown_field` reports them:
    /// arrays are traversed without appearing in the path, so
    /// `payload.claims.atom` matches the `atom` of ANY claim.
    #[test]
    fn array_paths_match_any_element_without_an_index() {
        let params = json!({
            "payload": { "claims": [
                { "atom_expr": null, "quote": "q", "line": 1 },
                { "atom": "warded(arden)", "quote": "q", "line": 2 },
            ] }
        });
        assert_eq!(
            killed_field_successor("1.0.0", Op::PayloadSubmit, &params),
            Some(("payload.claims.atom", "atom_expr"))
        );
        for empty in [
            json!({ "payload": { "claims": [] } }),
            json!({ "payload": {} }),
            json!({}),
        ] {
            assert!(
                killed_field_successor("1.0.0", Op::PayloadSubmit, &empty).is_none(),
                "no claim carries the retired field: {empty}"
            );
        }
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
