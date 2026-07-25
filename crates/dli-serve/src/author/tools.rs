//! Door T tools (`AUTH-1`): the fourteen `author.*` tools and their immediate
//! graded findings.
//!
//! Each tool lowers an agent intent to typed AST, stages it over the workspace's
//! base, checks the staged layer, and answers with the feedback contract of spec
//! 11 §4: graded findings (data, never instructions), the rendered canonical
//! surface of what it staged, and the new workspace digest. Nothing an agent
//! supplies mints authority — every claim enters `proposed` or is
//! `hypothesis`-scoped (§5) — and instruction-shaped strings in parameters are
//! stored and rendered as data, never interpreted (§5, framing safety).
//!
//! Fail-closed (§4): a malformed call rejects whole (a `schema`/`refused` wire
//! error, staging nothing); an **error-severity finding** stages the claim but
//! bars `author.materialize` until the world is made consistent again.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use dli_ast::annot::{Annotation, CondExpr, ProvenanceMarker};
use dli_ast::decl::{
    AttrDecl, BeforeDecl, Decl, EntityDecl, EntityKind as AstEntityKind, FactDecl, HypothesisDecl,
    OptionClause, QuestionDecl,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::Status;
use dli_core::grade::Grade;
use dli_world::CompiledWorld;

use crate::contract::{
    AuthorAddBeforeParams, AuthorAddEntityParams, AuthorAddEventParams, AuthorAddFactParams,
    AuthorAddHypothesisParams, AuthorAddOptionParams, AuthorAddRelationParams, AuthorDiffParams,
    AuthorDiffResult, AuthorMaterializeParams, AuthorMutationResult, AuthorOpenParams,
    AuthorOpenQuestionParams, AuthorOpenResult, AuthorStatusParams, AuthorStatusResult,
    AuthorSupposeParams, EntityKind as WireEntityKind,
};
use crate::envelope::{ErrorBody, ErrorCode};
use crate::finding::{Finding, Provenance, Severity};

use super::render::{render_decls, render_option, render_scoped};
use super::store::Store;
use super::workspace::{Mode, Workspace};

// -- author.open -------------------------------------------------------------

/// Open a workspace over the store's base, in `scope` or `fork` mode (spec 11
/// §2). The `scope` param stages over an existing scope (a pack, in DL-0016's
/// renamed surface); `fork` opens a fork-scoped workspace.
pub(super) fn open(store: &mut Store, params: &AuthorOpenParams) -> Result<Value, ErrorBody> {
    let (mode, base_name) = match (&params.scope, &params.fork) {
        (Some(scope), None) => (Mode::Scope, scope.clone()),
        (None, Some(fork)) => (Mode::Fork, fork.clone()),
        _ => {
            return Err(schema_error(
                "author.open needs exactly one of `scope` or `fork`",
            ));
        }
    };
    let ws = store
        .open(mode, base_name.clone())
        .ok_or_else(|| ErrorBody {
            code: ErrorCode::UnknownDoc,
            message: format!("no project document named \"{base_name}\" is loaded"),
            data: None,
        })?;
    Ok(value(&AuthorOpenResult {
        workspace: ws.id.clone(),
        workspace_digest: ws.digest.clone(),
    }))
}

// -- author.status / author.diff (read-only) ---------------------------------

/// The workspace's staged canonical surface and current digest.
pub(super) fn status(store: &Store, params: &AuthorStatusParams) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    Ok(value(&AuthorStatusResult {
        rendered: render_decls(&ws.decls),
        workspace_digest: ws.digest.clone(),
    }))
}

/// The rendered canonical block of the whole staged change (spec 11 §2).
pub(super) fn diff(store: &Store, params: &AuthorDiffParams) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    Ok(value(&AuthorDiffResult {
        rendered: render_decls(&ws.decls),
    }))
}

// -- author.discard ----------------------------------------------------------

/// Discard a workspace (R3/C3). With no `decl`, drops the whole workspace and
/// answers `{discarded}`. With a `decl` — the rendered canonical line of one
/// staged declaration — drops only that declaration, leaving the workspace open,
/// and answers `{discarded, workspace_digest}`; a `decl` that matches nothing is
/// an `unknown-doc` refusal (it never falls through to nuking the workspace).
pub(super) fn discard(
    store: &mut Store,
    ws_id: &str,
    decl: Option<&str>,
) -> Result<Value, ErrorBody> {
    let Some(decl_id) = decl else {
        return Ok(json!({ "discarded": store.discard(ws_id) }));
    };
    let (_, ws) = parts_mut(store, ws_id)?;
    let index = ws
        .decls
        .iter()
        .position(|staged| decl_key(staged) == decl_id);
    match index {
        Some(i) => {
            ws.remove_decl(i);
            // A structural change invalidates every cached replay: a later retry
            // must re-execute against the new state, not serve a stale result for
            // a declaration that is no longer present (R3/B5).
            ws.idempotency.clear();
            Ok(json!({ "discarded": true, "workspace_digest": ws.digest }))
        }
        None => Err(unknown_decl(decl_id)),
    }
}

// -- author.materialize ------------------------------------------------------

/// Render the workspace to `.dli` file lines **as data** — the kernel writes
/// nothing (spec 11 §4). Refuses with `materialize-blocked` when any
/// error-severity finding is open; the blocking finding codes ride in the error
/// data.
pub(super) fn materialize(
    store: &Store,
    params: &AuthorMaterializeParams,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let base = store.workspace_base(&params.workspace).ok_or_else(gone)?;
    let blocking = dedup(
        workspace_findings(ws, base)
            .into_iter()
            .filter(|f| f.severity == Severity::Error)
            .map(|f| f.code),
    );
    if !blocking.is_empty() {
        return Err(ErrorBody {
            code: ErrorCode::MaterializeBlocked,
            message: format!(
                "{} error-severity finding(s) are unresolved; resolve them or discard before materializing.",
                blocking.len()
            ),
            data: Some(json!({ "blocking": blocking })),
        });
    }
    Ok(value(&AuthorMutationResult {
        findings: Vec::new(),
        rendered: render_scoped(&ws.base_name, &ws.decls),
        workspace_digest: ws.digest.clone(),
    }))
}

// -- the claim-bearing mutations ---------------------------------------------

/// Add an entity of a given kind (person/place/thing/era). Provenance required;
/// the display name is stored verbatim as data.
pub(super) fn add_entity(
    store: &mut Store,
    params: &AuthorAddEntityParams,
) -> Result<Value, ErrorBody> {
    if let Some(refusal) = refuse_cited(store, &params.workspace, &params.provenance) {
        return refusal;
    }
    let decl = Decl::Entity(EntityDecl {
        kind: map_kind(params.kind),
        name: ident(&params.id),
        label: Some(params.name.clone()),
        time: None,
        slots: Vec::new(),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Add a keyed value on one entity — lowers to `key(entity, value)`.
pub(super) fn add_fact(
    store: &mut Store,
    params: &AuthorAddFactParams,
) -> Result<Value, ErrorBody> {
    if let Some(refusal) = refuse_cited(store, &params.workspace, &params.provenance) {
        return refusal;
    }
    let decl = Decl::Attr(AttrDecl {
        subject: ident(&params.entity),
        attr: ident(&params.key),
        value: Term::Sym(ident(&params.value)),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Add a binary relation between two entities — lowers to `relation(from, to)`.
pub(super) fn add_relation(
    store: &mut Store,
    params: &AuthorAddRelationParams,
) -> Result<Value, ErrorBody> {
    if let Some(refusal) = refuse_cited(store, &params.workspace, &params.provenance) {
        return refusal;
    }
    let decl = Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: ident(&params.relation),
            args: vec![Term::Sym(ident(&params.from)), Term::Sym(ident(&params.to))],
        },
        triple: false,
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Add a chronology event (an event is not an entity; §3.4). Provenance required.
pub(super) fn add_event(
    store: &mut Store,
    params: &AuthorAddEventParams,
) -> Result<Value, ErrorBody> {
    if let Some(refusal) = refuse_cited(store, &params.workspace, &params.provenance) {
        return refusal;
    }
    let decl = Decl::Entity(EntityDecl {
        kind: AstEntityKind::Event,
        name: ident(&params.id),
        label: Some(params.name.clone()),
        time: None,
        slots: Vec::new(),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Order one event before another — declaring an order is a ruling, so
/// provenance is required. A declared order that closes a cycle is an error.
pub(super) fn add_before(
    store: &mut Store,
    params: &AuthorAddBeforeParams,
) -> Result<Value, ErrorBody> {
    if let Some(refusal) = refuse_cited(store, &params.workspace, &params.provenance) {
        return refusal;
    }
    let decl = Decl::Before(BeforeDecl {
        earlier: ident(&params.earlier),
        later: ident(&params.later),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Open a question — a tracked debt (§3.6). No claim is asserted by opening one.
pub(super) fn open_question(
    store: &mut Store,
    params: &AuthorOpenQuestionParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Question(QuestionDecl {
        name: ident(&params.id),
        prompt: Some(params.prompt.clone()),
        options: Vec::new(),
        provenance: None,
    });
    stage(store, &params.workspace, decl)
}

/// Add one option to a question already open in the workspace (§3.7).
pub(super) fn add_option(
    store: &mut Store,
    params: &AuthorAddOptionParams,
) -> Result<Value, ErrorBody> {
    let (_, ws) = parts_mut(store, &params.workspace)?;
    let mut findings: Vec<Finding> = [("id", params.id.as_str()), ("label", params.label.as_str())]
        .into_iter()
        .filter_map(|(field, text)| field_control(field, text))
        .collect();
    let index = ws
        .decls
        .iter()
        .position(|decl| matches!(decl, Decl::Question(q) if q.name.text == params.question));
    let Some(i) = index else {
        findings.push(question_unknown(&params.question));
        return Ok(mutation(findings, Vec::new(), &ws.digest));
    };
    // Rebuild the question with the appended option, then swap it in — the
    // immutable borrow ends before the cache-maintaining replace.
    let updated = if let Some(Decl::Question(q)) = ws.decls.get(i) {
        let mut question = q.clone();
        question.options.push(OptionClause {
            name: ident(&params.id),
            implies: None,
        });
        Some(Decl::Question(question))
    } else {
        None
    };
    if let Some(decl) = updated {
        ws.replace_decl(i, decl);
    }
    let rendered = render_option(&params.question, &params.id);
    Ok(mutation(findings, rendered, &ws.digest))
}

/// Mint a hypothesis variable for fork-scoped supposition (§3.8): legal only in
/// a fork workspace and limited to one per proposal bundle.
pub(super) fn add_hypothesis(
    store: &mut Store,
    params: &AuthorAddHypothesisParams,
) -> Result<Value, ErrorBody> {
    let (_, ws) = parts_mut(store, &params.workspace)?;
    let decl = Decl::Hypothesis(HypothesisDecl {
        name: ident(&params.id),
        statement: Some(params.statement.clone()),
    });
    let mut findings = control_char_findings(&decl);
    if ws.mode != Mode::Fork {
        findings.push(hypothesis_not_in_fork());
    }
    let existing = hypothesis_count(ws);
    if existing >= 1 {
        findings.push(hypothesis_bundle_limit(existing + 1));
    }
    let rendered = render_decls(std::slice::from_ref(&decl));
    ws.push_decl(decl);
    Ok(mutation(findings, rendered, &ws.digest))
}

/// Suppose one or more staged hypotheses true within the fork (§3.9): exploration
/// over an unchanged store — it asserts nothing in canon and adds no pack surface.
pub(super) fn suppose(store: &mut Store, params: &AuthorSupposeParams) -> Result<Value, ErrorBody> {
    let (_, ws) = parts_mut(store, &params.workspace)?;
    let staged: BTreeSet<String> = ws
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Hypothesis(h) => Some(h.name.text.clone()),
            _ => None,
        })
        .collect();
    let mut findings: Vec<Finding> = params
        .hypotheses
        .iter()
        .filter_map(|hyp| field_control("hypothesis", hyp))
        .collect();
    let mut supposed = Vec::new();
    for hyp in &params.hypotheses {
        if staged.contains(hyp) {
            supposed.push(hyp.clone());
        } else {
            findings.push(hypothesis_unknown(hyp));
        }
    }
    for hyp in &supposed {
        if !ws.suppositions.contains(hyp) {
            ws.suppositions.push(hyp.clone());
        }
    }
    if !supposed.is_empty() {
        findings.push(consequence_changed(&supposed));
    }
    ws.reseal();
    Ok(mutation(findings, Vec::new(), &ws.digest))
}

// -- staging + the checked layer ---------------------------------------------

/// Stage a world declaration: check the candidate against the current staged
/// state and the base, render it, push it, and reseal the digest. The check runs
/// *before* the push, so a reference resolves against what is already present.
fn stage(store: &mut Store, ws_id: &str, decl: Decl) -> Result<Value, ErrorBody> {
    let (base, ws) = parts_mut(store, ws_id)?;
    let findings = decl_findings(&decl, ws, base);
    let rendered = render_decls(std::slice::from_ref(&decl));
    ws.push_decl(decl);
    Ok(mutation(findings, rendered, &ws.digest))
}

/// The findings a declaration raises against the staged state and the base.
/// Every declaration is first scanned for control characters in its free-text
/// fields (R3/C1); the world declarations then add their resolution findings.
fn decl_findings(decl: &Decl, ws: &Workspace, base: &CompiledWorld) -> Vec<Finding> {
    let mut out = control_char_findings(decl);
    match decl {
        Decl::Entity(e) if e.kind == AstEntityKind::Event => {}
        Decl::Entity(e) => out.push(name_unvetted(&e.name.text)),
        Decl::Attr(a) => out.extend(attr_findings(a, ws, base)),
        Decl::Fact(f) => out.extend(relation_findings(f, ws, base)),
        Decl::Before(b) => out.extend(before_findings(b, ws, base)),
        _ => {}
    }
    out
}

/// `add_fact` findings: an unknown subject (error) and a slot that contradicts a
/// certain base value (error).
fn attr_findings(attr: &AttrDecl, ws: &Workspace, base: &CompiledWorld) -> Vec<Finding> {
    let known = known_entities(base, &ws.decls);
    let mut out = Vec::new();
    if !known.contains(&attr.subject.text) {
        out.push(entity_unknown(&attr.subject.text, &known));
    }
    if let Term::Sym(value) = &attr.value {
        let certain = base_certain_values(base, &attr.attr.text, &attr.subject.text);
        if certain.len() == 1
            && !certain.contains(&value.text)
            && let Some(canon) = certain.iter().next()
        {
            out.push(contradiction(
                &attr.subject.text,
                &attr.attr.text,
                &value.text,
                canon,
            ));
        }
    }
    out
}

/// `add_relation` findings: each argument that names no known entity is an error,
/// carrying candidate ids (§3.3).
fn relation_findings(fact: &FactDecl, ws: &Workspace, base: &CompiledWorld) -> Vec<Finding> {
    let known = known_entities(base, &ws.decls);
    fact.atom
        .args
        .iter()
        .filter_map(|arg| match arg {
            Term::Sym(id) if !known.contains(&id.text) => Some(entity_unknown(&id.text, &known)),
            _ => None,
        })
        .collect()
}

/// `add_before` findings: an unknown endpoint (error) and an order that closes a
/// cycle against existing orderings (error; §3.5).
fn before_findings(before: &BeforeDecl, ws: &Workspace, base: &CompiledWorld) -> Vec<Finding> {
    let known = known_entities(base, &ws.decls);
    let mut out = Vec::new();
    if !known.contains(&before.earlier.text) {
        out.push(entity_unknown(&before.earlier.text, &known));
    }
    if !known.contains(&before.later.text) {
        out.push(entity_unknown(&before.later.text, &known));
    }
    if closes_cycle(&before.earlier.text, &before.later.text, ws, base) {
        out.push(order_cycle(&before.earlier.text, &before.later.text));
    }
    out
}

/// Every finding open over the whole workspace — the materialize gate reads its
/// error-severity subset. Each declaration is re-checked against the *full*
/// staged state, so a later-added target clears an earlier reference's block.
///
/// The read tools (`author.findings`, `author.check`) reuse this so the standing
/// aggregate is exactly what materialize refuses on.
pub(super) fn workspace_findings(ws: &Workspace, base: &CompiledWorld) -> Vec<Finding> {
    let mut out = Vec::new();
    for decl in &ws.decls {
        out.extend(decl_findings(decl, ws, base));
    }
    let hyps = hypothesis_count(ws);
    if hyps > 0 && ws.mode != Mode::Fork {
        out.push(hypothesis_not_in_fork());
    }
    if hyps > 1 {
        out.push(hypothesis_bundle_limit(hyps));
    }
    out
}

// -- entity/order resolution -------------------------------------------------

/// The known-entity set: every entity the base declares or observes, unioned with
/// the entities and events staged in the workspace.
fn known_entities(base: &CompiledWorld, decls: &[Decl]) -> BTreeSet<String> {
    let mut set: BTreeSet<String> = base
        .model
        .entities()
        .iter()
        .map(|entry| entry.name.clone())
        .collect();
    for decl in decls {
        if let Decl::Entity(e) = decl {
            set.insert(e.name.text.clone());
        }
    }
    set
}

/// The distinct certain values the base holds for a `(pred, subject)` slot — the
/// basis a proposed fact could contradict.
fn base_certain_values(base: &CompiledWorld, pred: &str, subject: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for atom in base.model.atoms() {
        if atom.atom.pred.text != pred || atom.atom.args.len() != 2 {
            continue;
        }
        let status = atom
            .ann
            .status(&base.model.world.valuation, &base.model.world.theory);
        if status != Status::Certain {
            continue;
        }
        if let (Term::Sym(subj), value) = (&atom.atom.args[0], &atom.atom.args[1])
            && subj.text == subject
        {
            out.insert(term_text(value));
        }
    }
    out
}

/// Whether adding `earlier -> later` closes a cycle against the base's and the
/// workspace's existing orderings.
fn closes_cycle(earlier: &str, later: &str, ws: &Workspace, base: &CompiledWorld) -> bool {
    if earlier == later {
        return true;
    }
    let edges = before_edges(ws, base);
    reachable(later, earlier, &edges)
}

/// Every `before` edge the base and the workspace already hold.
fn before_edges(ws: &Workspace, base: &CompiledWorld) -> Vec<(String, String)> {
    let mut edges = Vec::new();
    for atom in base.model.atoms() {
        if atom.atom.pred.text == "before"
            && let [Term::Sym(a), Term::Sym(b)] = atom.atom.args.as_slice()
        {
            edges.push((a.text.clone(), b.text.clone()));
        }
    }
    for decl in &ws.decls {
        if let Decl::Before(b) = decl {
            edges.push((b.earlier.text.clone(), b.later.text.clone()));
        }
    }
    edges
}

/// Whether `to` is reachable from `from` following the directed edges.
fn reachable(from: &str, to: &str, edges: &[(String, String)]) -> bool {
    let mut frontier = vec![from.to_owned()];
    let mut seen: BTreeSet<String> = BTreeSet::new();
    while let Some(node) = frontier.pop() {
        if node == to {
            return true;
        }
        if !seen.insert(node.clone()) {
            continue;
        }
        for (a, b) in edges {
            if a == &node {
                frontier.push(b.clone());
            }
        }
    }
    false
}

/// Candidate ids an unknown reference most plausibly meant: known entities within
/// a small edit distance, nearest first.
fn candidates(target: &str, known: &BTreeSet<String>) -> Vec<String> {
    let mut scored: Vec<(usize, &String)> = known
        .iter()
        .map(|name| (levenshtein(target, name), name))
        .filter(|(distance, _)| *distance <= 3)
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    scored
        .into_iter()
        .take(5)
        .map(|(_, name)| name.clone())
        .collect()
}

/// The Levenshtein edit distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

// -- control-character defense (R3/C1) ---------------------------------------

/// The `field.control-char` findings a declaration's free-text fields raise: one
/// per field carrying a control character. A newline or tab smuggled into an
/// origin, value, name, prompt, or option would forge a second declaration line
/// in the review diff, so it enters as an error-severity finding that bars
/// materialization (the display is separately sanitized in `render`).
fn control_char_findings(decl: &Decl) -> Vec<Finding> {
    free_text_fields(decl)
        .iter()
        .filter_map(|(field, text)| field_control(field, text))
        .collect()
}

/// A `field.control-char` finding for one field, if its text carries a control
/// character.
fn field_control(field: &'static str, text: &str) -> Option<Finding> {
    text.chars()
        .any(super::render::is_forbidden_char)
        .then(|| control_char(field))
}

/// The named free-text fields a declaration renders — everything a control
/// character could hide inside.
fn free_text_fields(decl: &Decl) -> Vec<(&'static str, String)> {
    let mut fields = Vec::new();
    match decl {
        Decl::Entity(e) => {
            fields.push(("id", e.name.text.clone()));
            if let Some(label) = &e.label {
                fields.push(("name", label.clone()));
            }
            annotation_fields(&mut fields, e.provenance.as_ref());
        }
        Decl::Attr(a) => {
            fields.push(("entity", a.subject.text.clone()));
            fields.push(("key", a.attr.text.clone()));
            fields.push(("value", term_text(&a.value)));
            annotation_fields(&mut fields, a.provenance.as_ref());
        }
        Decl::Fact(f) => {
            fields.push(("relation", f.atom.pred.text.clone()));
            for arg in &f.atom.args {
                fields.push(("argument", term_text(arg)));
            }
            annotation_fields(&mut fields, f.provenance.as_ref());
        }
        Decl::Before(b) => {
            fields.push(("earlier", b.earlier.text.clone()));
            fields.push(("later", b.later.text.clone()));
            annotation_fields(&mut fields, b.provenance.as_ref());
        }
        Decl::Question(q) => {
            fields.push(("id", q.name.text.clone()));
            if let Some(prompt) = &q.prompt {
                fields.push(("prompt", prompt.clone()));
            }
            for option in &q.options {
                fields.push(("option", option.name.text.clone()));
            }
            annotation_fields(&mut fields, q.provenance.as_ref());
        }
        Decl::Hypothesis(h) => {
            fields.push(("id", h.name.text.clone()));
            if let Some(statement) = &h.statement {
                fields.push(("statement", statement.clone()));
            }
        }
        _ => {}
    }
    fields
}

/// The free-text fields a provenance annotation carries (origin tag, cited
/// decision ids, hypothesis condition).
fn annotation_fields(fields: &mut Vec<(&'static str, String)>, annot: Option<&Annotation>) {
    let Some(annot) = annot else {
        return;
    };
    for tag in &annot.tags {
        fields.push(("origin", tag.clone()));
    }
    for citation in &annot.citations {
        fields.push(("decision", citation.id.text.clone()));
    }
    if let Some(CondExpr::Hyp(hyp)) = &annot.condition {
        fields.push(("hypothesis", hyp.text.clone()));
    }
}

/// The canonical rendered line that names a staged declaration for `author.discard`
/// (R3/C3) — the same sanitized line the agent reads in `author.diff`.
fn decl_key(decl: &Decl) -> String {
    render_decls(std::slice::from_ref(decl))
        .into_iter()
        .next()
        .unwrap_or_default()
}

// -- finding constructors ----------------------------------------------------

fn name_unvetted(id: &str) -> Finding {
    Finding {
        severity: Severity::Warn,
        code: "name.unvetted".to_owned(),
        message: format!(
            "The name \"{id}\" has not passed the naming oracle; it enters proposed and unvetted."
        ),
        grade: Grade::Asserted,
        repair: Some(json!({
            "kind": "run-check",
            "op": "dli.name.check",
            "params": { "name": id }
        })),
        citations: Vec::new(),
    }
}

fn entity_unknown(id: &str, known: &BTreeSet<String>) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "entity.unknown".to_owned(),
        message: format!(
            "No entity \"{id}\" is in scope. The claim is staged but cannot materialize until it exists."
        ),
        grade: Grade::Checked,
        repair: Some(json!({ "kind": "resolve-entity", "candidates": candidates(id, known) })),
        citations: Vec::new(),
    }
}

fn contradiction(subject: &str, key: &str, value: &str, canon: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "fact.contradiction".to_owned(),
        message: format!(
            "claims {subject}.{key} = {value}, but the base asserts {canon} as certain."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
    }
}

fn order_cycle(earlier: &str, later: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "order.cycle".to_owned(),
        message: format!(
            "before({earlier}, {later}) closes a cycle against the existing orderings; the timeline would have no consistent order."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
    }
}

fn consequence_changed(supposed: &[String]) -> Finding {
    Finding {
        severity: Severity::Warn,
        code: "consequence.changed".to_owned(),
        message: format!(
            "Under {}, the derived-consequence set may move. See author.diff for the consequences.",
            supposed.join(", ")
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
    }
}

fn hypothesis_not_in_fork() -> Finding {
    Finding {
        severity: Severity::Error,
        code: "hypothesis.not-in-fork".to_owned(),
        message: "A hypothesis is legal only in a fork workspace; open with {\"fork\": …}."
            .to_owned(),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
    }
}

fn hypothesis_bundle_limit(count: usize) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "hypothesis.bundle-limit".to_owned(),
        message: format!(
            "A proposal bundle carries at most one hypothesis; this workspace now stages {count}. A second supposition is a second fork."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
    }
}

fn hypothesis_unknown(id: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "hypothesis.unknown".to_owned(),
        message: format!("No hypothesis \"{id}\" is staged in this workspace to suppose."),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
    }
}

fn question_unknown(id: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "question.unknown".to_owned(),
        message: format!(
            "No question \"{id}\" is open in this workspace; open it before adding options."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
    }
}

fn control_char(field: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "field.control-char".to_owned(),
        message: format!(
            "The {field} field carries a control or bidi/zero-width character; it could forge or reorder a declaration in the review diff. Remove it before materializing."
        ),
        grade: Grade::Checked,
        repair: Some(json!({ "kind": "remove-control-chars", "field": field })),
        citations: Vec::new(),
    }
}

fn cited_refused() -> Finding {
    Finding {
        severity: Severity::Error,
        code: "provenance.cited-refused".to_owned(),
        message: "Door T authors `proposed` or `hypothesis` only; an agent cannot cite a ruling to mint canon authority. Citing a decision is a human act in review — resubmit with proposed provenance."
            .to_owned(),
        grade: Grade::Checked,
        repair: Some(json!({ "kind": "add-provenance", "required": "proposed" })),
        citations: Vec::new(),
    }
}

// -- lowering + wire helpers -------------------------------------------------

/// Lower a wire [`Provenance`] to an AST [`Annotation`]. `proposed` records an
/// origin tag; `hypothesis` scopes the claim to a fork-local hyp condition.
/// Nothing an agent supplies mints authority (§5; R3/B1): `cited` is **refused**
/// at the tool boundary ([`refuse_cited`]), and this lowering never maps it to a
/// ledger marker — a `cited` that somehow reached here fails closed to a bare
/// `proposed` with no citation, never `decided`/`ratified`/`provisional`.
fn annotation(provenance: &Provenance) -> Annotation {
    match provenance {
        Provenance::Cited { .. } => Annotation {
            marker: ProvenanceMarker::Proposed,
            citations: Vec::new(),
            condition: None,
            tags: Vec::new(),
            span: Span::synthetic(),
        },
        Provenance::Proposed { origin } => Annotation {
            marker: ProvenanceMarker::Proposed,
            citations: Vec::new(),
            condition: None,
            tags: vec![origin.clone()],
            span: Span::synthetic(),
        },
        Provenance::Hypothesis { hyp } => Annotation {
            marker: ProvenanceMarker::Proposed,
            citations: Vec::new(),
            condition: Some(CondExpr::Hyp(Ident::synthetic(hyp))),
            tags: Vec::new(),
            span: Span::synthetic(),
        },
    }
}

/// Map the wire entity kind to the AST entity kind.
fn map_kind(kind: WireEntityKind) -> AstEntityKind {
    match kind {
        WireEntityKind::Person => AstEntityKind::Person,
        WireEntityKind::Place => AstEntityKind::Place,
        WireEntityKind::Thing => AstEntityKind::Thing,
        WireEntityKind::Era => AstEntityKind::Era,
    }
}

/// The ground string form of a term (mirrors the loader's rendering).
fn term_text(term: &Term) -> String {
    match term {
        Term::Var(id) | Term::Sym(id) => id.text.clone(),
        Term::Str(text) => text.clone(),
        Term::Int(value) => value.to_string(),
        Term::Time(dli_ast::term::TimeRef::Era(id)) => format!("era:{}", id.text),
        Term::Time(dli_ast::term::TimeRef::Event(id)) => format!("event:{}", id.text),
    }
}

fn ident(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn hypothesis_count(ws: &Workspace) -> usize {
    ws.decls
        .iter()
        .filter(|decl| matches!(decl, Decl::Hypothesis(_)))
        .count()
}

/// A mutation result value: findings, rendered lines, and the new digest.
fn mutation(findings: Vec<Finding>, rendered: Vec<String>, digest: &str) -> Value {
    value(&AuthorMutationResult {
        findings,
        rendered,
        workspace_digest: digest.to_owned(),
    })
}

/// Serialize a typed result to a wire value.
fn value<T: serde::Serialize>(result: &T) -> Value {
    serde_json::to_value(result).unwrap_or(Value::Null)
}

/// Deduplicate finding codes, preserving first-seen order.
fn dedup(codes: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    codes
        .into_iter()
        .filter(|code| seen.insert(code.clone()))
        .collect()
}

/// The refusal a claim-bearing tool returns when an agent supplies `cited`
/// provenance (R3/B1): Door T authors `proposed` or `hypothesis` only, so a
/// citation is refused as an error-severity finding and **nothing is staged** —
/// no forged decision variable ever reaches the workspace, so the claim can
/// never read certain. `None` when the provenance is agent-authorable.
fn refuse_cited(
    store: &mut Store,
    ws_id: &str,
    provenance: &Provenance,
) -> Option<Result<Value, ErrorBody>> {
    if !matches!(provenance, Provenance::Cited { .. }) {
        return None;
    }
    Some(match parts_mut(store, ws_id) {
        Ok((_, ws)) => Ok(mutation(vec![cited_refused()], Vec::new(), &ws.digest)),
        Err(body) => Err(body),
    })
}

/// The base and a mutable workspace, or `internal` if the workspace vanished
/// (dispatch checks existence before calling a tool, so this is unreachable in
/// practice — it fails closed rather than panicking).
fn parts_mut<'a>(
    store: &'a mut Store,
    id: &str,
) -> Result<(&'a CompiledWorld, &'a mut Workspace), ErrorBody> {
    store.parts_mut(id).ok_or_else(gone)
}

fn schema_error(message: &str) -> ErrorBody {
    ErrorBody {
        code: ErrorCode::SchemaViolation,
        message: message.to_owned(),
        data: None,
    }
}

fn gone() -> ErrorBody {
    ErrorBody {
        code: ErrorCode::Internal,
        message: "the workspace vanished mid-call".to_owned(),
        data: None,
    }
}

/// The refusal for an `author.discard` naming a declaration the workspace does
/// not hold — never a fall-through to discarding the whole workspace (R3/C3).
fn unknown_decl(decl_id: &str) -> ErrorBody {
    ErrorBody {
        code: ErrorCode::UnknownDoc,
        message: format!("no staged declaration matching \"{decl_id}\" in this workspace"),
        data: None,
    }
}
