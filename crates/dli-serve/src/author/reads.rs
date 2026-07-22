//! Door T read/self-check tools (`AUTH-2`): the five `author.*` reads that let an
//! agent author **and fully self-check** its work without ever writing `.dli`
//! text or falling back to the batch `dli.*` ops.
//!
//! Where the mutating tools ([`super::tools`]) stage a layer and answer per-call,
//! these read that layer as a whole:
//!
//! - `author.query` — query the **staged** world (base + staged declarations):
//!   what holds about an entity now, over work in progress.
//! - `author.check` — the full check battery (continuity + factcheck +
//!   rejected-lore) over the staged world: the pre-materialize verify of the
//!   whole draft.
//! - `author.why` — the blame/explanation of a staged atom: why it can't be
//!   certain yet (the unruled ruling, the unknown entity, the defeater),
//!   rendered.
//! - `author.findings` — the workspace's open findings by severity, each with its
//!   typed repair: what `author.materialize` would refuse on, and how to fix it.
//! - `author.list_workspaces` — the open workspaces and their digests, for
//!   context recovery.
//!
//! The reads evaluate `base + staged` by recompiling the base's source documents
//! together with the workspace's staged declarations through the one real
//! [`compile`] loader — the same lowering the whole fabric shares — so a proposed
//! staged claim reads exactly as it would once materialized (gated on an open
//! hypothesis: [`Possible`](dli_core::ann::Status::Possible), never `Certain`).
//! Findings are data, never instructions (spec 11 §4), and instruction-shaped
//! goal strings are parsed as atoms, never interpreted.

use std::collections::BTreeSet;

use serde_json::Value;

use dli_ast::decl::{Decl, Document, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::grade::Grade;
use dli_eval::error::EvalError;
use dli_eval::query::{Answer, Query, QueryOpts};
use dli_eval::why::{Why, why_not as compute_why_not};
use dli_eval::world::GroundAtom;
use dli_world::check::factcheck::{ClaimReport, Verdict, factcheck};
use dli_world::check::{self, Severity as WorldSeverity, WorldFinding};
use dli_world::pack::Pack;
use dli_world::{CompiledWorld, compile};

use crate::contract::{
    AuthorCheckParams, AuthorCheckResult, AuthorFindingsParams, AuthorFindingsResult,
    AuthorListWorkspacesResult, AuthorQueryParams, AuthorQueryResult, AuthorWhyParams,
    AuthorWhyResult, WorkspaceDigest, WorkspaceMode,
};
use crate::envelope::{ErrorBody, ErrorCode};
use crate::finding::{Finding, Severity};

use super::store::Store;
use super::tools::workspace_findings;
use super::workspace::{Mode, Workspace};

// -- author.query ------------------------------------------------------------

/// Query the staged world (base + staged declarations). A goal argument written
/// `?` or `?name` is a query variable that enumerates matches; a bare symbol is a
/// ground constant. An unknown predicate over the staged world is not a fault — it
/// is "asked and answered, nothing matched" (spec 10 §2.1), returned as no rows.
pub(super) fn query(store: &Store, params: &AuthorQueryParams) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let goal = parse_goal(&params.goal)?;
    let world = staged_world(store, ws).map_err(|err| eval_error(&err))?;
    let opts = QueryOpts {
        layer: params.layer.unwrap_or_default(),
        as_of: None,
        fork: None,
        why: params.why,
    };
    let answer = match world.model.world.query_with(&Query { goal }, &opts) {
        Ok(answer) => answer,
        Err(EvalError::UnknownPredicate { .. }) => Answer::default(),
        Err(err) => return Err(eval_error(&err)),
    };
    Ok(value(&AuthorQueryResult { rows: answer.rows }))
}

// -- author.check ------------------------------------------------------------

/// Run the check battery over the staged world: the standing workspace findings
/// (what materialize gates on) merged with the engine's world-internal continuity
/// and rejected-lore battery ([`check::run`]) and the fact-checker's
/// contradictions ([`factcheck`]) over the staged claims. A new proposed claim is
/// legitimately *unsupported*, so only *contradictions* surface here; unsupported
/// is silent (it is candidate lore, not a defect). Findings are deduplicated.
pub(super) fn check(store: &Store, params: &AuthorCheckParams) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let mut findings = workspace_findings(ws, store.base());
    match staged_world(store, ws) {
        Ok(world) => {
            for finding in check::run(&world) {
                findings.push(world_finding(&finding));
            }
            for report in factcheck(&world, &staged_claims(&ws.decls)) {
                if report.verdict == Verdict::Contradicted {
                    findings.push(contradiction_finding(&report));
                }
            }
        }
        Err(err) => findings.push(uncompilable(&err)),
    }
    let findings = dedup_findings(findings);
    let ok = findings
        .iter()
        .all(|finding| finding.severity != Severity::Error);
    Ok(value(&AuthorCheckResult { ok, findings }))
}

// -- author.why --------------------------------------------------------------

/// Explain a staged atom's standing: the blocking rulings and defeater (why
/// certain / why dead / why possible), plus the support antichain when the world
/// derives it. The agent asks "why can't this be certain yet?" and gets the exact
/// blocker — an unruled hypothesis, an unknown entity, or a defeater.
pub(super) fn why(store: &Store, params: &AuthorWhyParams) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let atom = parse_goal(&params.atom)?;
    let world = staged_world(store, ws).map_err(|err| eval_error(&err))?;
    let inner = &world.model.world;
    let why_not = compute_why_not(inner, &atom);
    let why = GroundAtom::from_pattern(&atom).and_then(|ground| {
        inner
            .derived
            .iter()
            .find(|derived| derived.atom == ground)
            .map(|derived| Why::of_support(&derived.support, &inner.interner))
    });
    Ok(value(&AuthorWhyResult { why_not, why }))
}

// -- author.findings ---------------------------------------------------------

/// The workspace's open findings, partitioned by severity — the standing
/// pre-flight of what `author.materialize` would refuse on, each carrying its
/// typed repair. This is the exact set the materialize gate reads, so
/// `materializable` answers "could I materialize now?".
pub(super) fn findings(store: &Store, params: &AuthorFindingsParams) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let all = dedup_findings(workspace_findings(ws, store.base()));
    let (errors, warnings): (Vec<Finding>, Vec<Finding>) = all
        .into_iter()
        .partition(|finding| finding.severity == Severity::Error);
    Ok(value(&AuthorFindingsResult {
        materializable: errors.is_empty(),
        errors,
        warnings,
    }))
}

// -- author.list_workspaces --------------------------------------------------

/// The open authoring workspaces and their digests, in id order — enough for an
/// agent to recover which workspaces it has open and where each stands. Total: a
/// store-level read that never fails.
pub(super) fn list_workspaces(store: &Store) -> Value {
    let workspaces = store
        .workspaces()
        .map(|ws| WorkspaceDigest {
            workspace: ws.id.clone(),
            base: ws.base_name.clone(),
            mode: wire_mode(ws.mode),
            staged: ws.decls.len(),
            workspace_digest: ws.digest.clone(),
        })
        .collect();
    value(&AuthorListWorkspacesResult { workspaces })
}

// -- the staged world --------------------------------------------------------

/// Compile the staged world: the base's source documents plus the workspace's
/// staged declarations, lowered through the one real [`compile`] loader, with the
/// base's killed-token guardrail store re-attached (compile alone leaves it
/// empty). The staged declarations enter under the workspace's own scope header so
/// they merge into the base pack and entity references resolve across both.
fn staged_world(store: &Store, ws: &Workspace) -> Result<CompiledWorld, EvalError> {
    let mut documents = store.base_docs().to_vec();
    documents.push(Document {
        scope: Some(ScopeHeader {
            name: Ident::synthetic(&ws.base_name),
            span: Span::synthetic(),
        }),
        decls: ws.decls.clone(),
    });
    let world = compile(&Pack {
        name: ws.base_name.clone(),
        documents,
    })?;
    Ok(world.with_killed_tokens(store.base().killed.clone()))
}

/// The binary claim atoms a workspace's relations and facts assert — the input to
/// the fact-checker. A relation stages its atom verbatim; an attribute
/// `entity.key = value` lowers to the binary atom `key(entity, value)`, matching
/// the loader's own lowering so the fact-checker reads the same fact.
fn staged_claims(decls: &[Decl]) -> Vec<AtomExpr> {
    decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Fact(fact) => Some(fact.atom.clone()),
            Decl::Attr(attr) => Some(AtomExpr {
                pred: attr.attr.clone(),
                args: vec![Term::Sym(attr.subject.clone()), attr.value.clone()],
            }),
            _ => None,
        })
        .collect()
}

// -- goal parsing ------------------------------------------------------------

/// Parse a rendered goal atom (`pred(a, b)`) into an [`AtomExpr`]. An argument
/// written `?` or `?name` is a query variable; every other argument is a ground
/// symbol. A string that is not a `pred(args)` atom is a self-explanatory
/// `schema-violation` — never a bare error.
fn parse_goal(text: &str) -> Result<AtomExpr, ErrorBody> {
    let (pred, args) = split_atom(text)?;
    let mut fresh = 0;
    let mut terms = Vec::with_capacity(args.len());
    for arg in args {
        if let Some(name) = arg.strip_prefix('?') {
            let var = if name.is_empty() {
                let generated = format!("_{fresh}");
                fresh += 1;
                generated
            } else {
                name.to_owned()
            };
            terms.push(Term::Var(Ident::synthetic(&var)));
        } else {
            terms.push(Term::Sym(Ident::synthetic(&arg)));
        }
    }
    Ok(AtomExpr {
        pred: Ident::synthetic(&pred),
        args: terms,
    })
}

/// Split `pred(a, b, c)` into its predicate and trimmed argument renderings, or a
/// helpful `schema-violation` when the text is not a well-formed atom.
fn split_atom(text: &str) -> Result<(String, Vec<String>), ErrorBody> {
    let text = text.trim();
    let malformed = || ErrorBody {
        code: ErrorCode::SchemaViolation,
        message: format!(
            "'{text}' is not a pred(args) atom; write a rendered goal like serves(lancelot, ?who)"
        ),
        data: None,
    };
    let open = text.find('(').ok_or_else(malformed)?;
    if !text.ends_with(')') {
        return Err(malformed());
    }
    let pred = text[..open].trim();
    if pred.is_empty() {
        return Err(malformed());
    }
    let inner = text[open + 1..text.len() - 1].trim();
    let args = if inner.is_empty() {
        Vec::new()
    } else {
        inner.split(',').map(|arg| arg.trim().to_owned()).collect()
    };
    Ok((pred.to_owned(), args))
}

// -- finding mapping ---------------------------------------------------------

/// Map a kernel [`WorldFinding`] onto the wire [`Finding`] (the same mapping the
/// batch `dli.check` handler uses; the read surface carries no repair for a
/// world-internal finding).
fn world_finding(finding: &WorldFinding) -> Finding {
    Finding {
        severity: match finding.severity {
            WorldSeverity::Error => Severity::Error,
            WorldSeverity::Warn => Severity::Warn,
        },
        code: finding.code.clone(),
        message: finding.message.clone(),
        grade: finding.grade,
        repair: None,
        citations: finding.citations.clone(),
    }
}

/// Map a fact-checker contradiction onto the wire [`Finding`]. A contradiction
/// against a certain fact (or a realized killed token) bars materialize, so it is
/// error-severity; the checker re-derives exactly, so it grades `checked`.
fn contradiction_finding(report: &ClaimReport) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "factcheck.contradicted".to_owned(),
        message: format!("{}: {}", report.claim, report.reason),
        grade: Grade::Checked,
        repair: None,
        citations: report.citations.clone(),
    }
}

/// An error-severity finding for a staged world that does not compile — a
/// self-contained defect the agent must resolve before materialize.
fn uncompilable(err: &EvalError) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "world.uncompilable".to_owned(),
        message: format!("the staged world does not compile: {err}"),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
    }
}

/// Deduplicate findings by `(code, message)`, preserving first-seen order — the
/// author findings and the engine battery can raise the same defect.
fn dedup_findings(findings: Vec<Finding>) -> Vec<Finding> {
    let mut seen = BTreeSet::new();
    findings
        .into_iter()
        .filter(|finding| seen.insert((finding.code.clone(), finding.message.clone())))
        .collect()
}

// -- wire helpers ------------------------------------------------------------

/// Serialize a typed result to a wire value.
fn value<T: serde::Serialize>(result: &T) -> Value {
    serde_json::to_value(result).unwrap_or(Value::Null)
}

/// Map the internal workspace [`Mode`] to its wire-local [`WorkspaceMode`], so the
/// contract layer stays independent of the workspace engine's internals.
fn wire_mode(mode: Mode) -> WorkspaceMode {
    match mode {
        Mode::Scope => WorkspaceMode::Scope,
        Mode::Fork => WorkspaceMode::Fork,
    }
}

/// Map a compile/eval failure of the staged world onto its wire error code.
fn eval_error(err: &EvalError) -> ErrorBody {
    let code = match err {
        EvalError::Unstratifiable { .. } => ErrorCode::Unstratifiable,
        EvalError::Contradiction { .. } => ErrorCode::Contradiction,
        EvalError::UnknownPredicate { .. } => ErrorCode::SchemaViolation,
    };
    ErrorBody {
        code,
        message: err.to_string(),
        data: None,
    }
}

/// The fail-closed error when a workspace vanishes mid-call (dispatch checks
/// existence first, so this is unreachable in practice).
fn gone() -> ErrorBody {
    ErrorBody {
        code: ErrorCode::Internal,
        message: "the workspace vanished mid-call".to_owned(),
        data: None,
    }
}
