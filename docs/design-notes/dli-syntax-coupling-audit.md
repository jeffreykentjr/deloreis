---
status: live
verdict: "Code audit of how deeply dli_syntax is coupled into the workspace, run against the lorestore roadmap's claims. Finding: the AST's expert tier carries eleven declaration bodies as raw clause strings that downstream crates re-parse at semantic-analysis time, so retiring the parser requires structuring those interiors first — recorded in the roadmap as LS-9. Evidence for the roadmap, not authority."
---

# dli_syntax coupling audit

An audit of the actual parser coupling in the workspace, run to falsify the
claims in `lorestore-roadmap.md` before they hardened. Three of the roadmap's
four architectural claims held; one was materially wrong. The roadmap has been
amended (LS-9); this note preserves the evidence.

## The claim that failed

The roadmap originally claimed the kernel operates on the AST post-parse, so
demoting `dli_syntax` to a pretty-printer would mainly affect the loader front
end, formatter, docs-oracle, and `fmt`. That is false as stated.

**The AST is only half structured.** `crates/dli-ast/src/clause.rs` defines
`Clause { text: String, span }` — whitespace-normalized raw source text — and
eleven declaration types carry `body: Vec<Clause>` rather than typed
interiors: `RuleDecl`, `ForkDecl`, `PracticeDecl`, `DialectDecl`, `DraftDecl`,
`ExportDecl`, `NameDecl`, `VariantDecl`, `RatifyDecl`, `PrincipleDecl`,
`ProofDecl`. The crate calls this the "expert tier": downstream lanes
structure their own interiors — by re-entering the parser at
semantic-analysis time, well past the load boundary:

- `dli-world` — `src/loader.rs:474,488` (`parse_rule_clause` on rule bodies);
  `src/payload.rs:923,1159,1543` (`parse_clause_atom`,
  `parse_proof_close_parts`); `src/language/dialect.rs:42` and
  `src/language/name.rs:30` (raw `lex`/`Token`/`TokenKind`).
- `dli-ledger` — `src/practice.rs:956`; `src/ledger_check.rs:1271,1298,1565`.
- `dli-time` — `src/order_rules.rs:538` (`over order` rule guards).
- `dli-serve` — `src/query_goal.rs:19` (wire query goals parsed as clause
  atoms).
- `dli_syntax::validate_proof_decl` — direct AST callers validate proofs by
  round-tripping through the formatter and parser, deliberately, so no second
  proof grammar is maintained. A semantic check implemented as syntax.

Consequence: JSON storage without a schema deepening just embeds `.dli`
micro-syntax strings inside JSON, and the parser stays a hard runtime
dependency of `dli-world`, `dli-ledger`, and `dli-time` regardless of storage
format. **Only `dli-core` and `dli-eval` are genuinely syntax-free** —
neither mentions `dli_syntax` anywhere, tests included. This is why the
roadmap's LS-9 (expert-tier structuring) exists and why LS-7 needs it.

## The claims that held, with sharpenings

- **Crate map.** All fourteen crates exist with the roles the roadmap
  assumes; the seminaive fixpoint is `crates/dli-eval/src/world.rs` and
  `ForkStore` is `fork.rs`. Sharpening: `dli-core` has zero internal
  dependencies (not even `dli-ast`) — the support lattice and grade order
  stand entirely alone. `dli-serve`'s `mcp.rs` is a thin JSON-RPC-over-stdio
  transport adapter over the same `serve::respond` path as the native worker;
  MCP is a transport, not the server.
- **Evidence store.** `crates/dli-serve/src/evidence.rs` matches the roadmap
  exactly: immutable JCS documents (`serde_json_canonicalizer`) addressed by
  SHA-256, project-wide advisory lock (`fs2` exclusive), atomic rename, a
  single linear attestation-batch chain verified on read. Evidence content
  addresses therefore carry over to lorestore unchanged.
- **Door T is already JSON-first on input.** All 20 `author.*` tools take
  typed JSON param structs with `deny_unknown_fields`
  (`crates/dli-serve/src/contract.rs`); no tool accepts `.dli` text. The
  workspace stages `Vec<dli_ast::decl::Decl>` in memory; the digest is a
  streaming SHA-256 over a canonical JSON string. The `.dli` coupling is
  output-side only: `author.materialize` (`author/tools.rs:140`, refuses on
  open error-severity findings) and the rendered diff (`author/render.rs:28`)
  — which is deliberately forgery-proofed as "the human's only supervision
  surface," so LS-5's structured-diff surface is a *replacement design*, not
  a deletion. Two text-on-wire remnants outside Door T: `handlers.rs:465`
  (`fork.add` takes `declarations: [String]` parsed as `.dli`) and
  `query_goal.rs`.

## Corrections to ambient beliefs

- **`.dli-cache` is a reserved path, not a live cache.** Nothing in the
  workspace writes it — `project.rs:302,342` only validates and creates the
  directory. It is also not gitignored (nor is `.dli-evidence/`); both are
  simply untracked. "Replace the cache" is a no-op.
- **The corpus is already mostly structured data.** 16 `.dli` files
  repo-wide, against 361 JSON, 51 TOML, 33 YAML. The consuming world tree is
  predominantly YAML/TOML/JSON; the `.dli` remainder is five language packs,
  one chronology file, one practice file, and the three worked-scenario
  fixtures.

## Sizing

- `dli-syntax`: 4,106 src + 2,055 test lines (`parser.rs` 2,471, `format.rs`
  613, `ladder.rs` 377, `lexer.rs` 269, `diag.rs` 233, `probe.rs` 99) —
  roughly 7.5% of the ~79,500-line workspace.
- A straightforward drop (crate + `cmd/fmt.rs` + docs-oracle and
  `fmt --check` lanes + the docgen keyword page) is ~7,000–7,500 lines, and
  invalidates 67 fenced ```` ```dli ```` blocks across 21 doc files.
- The work that actually frees the drop is LS-9, and it lands in `dli-world`
  (17,228 src), `dli-ledger` (3,640), and `dli-time` (2,229). The `loc` gate
  lane (`xtask/src/loc.rs`) enforces a 25,000 soft / 40,000 hard cap per
  crate `src/`; a schema deepening adds lines before it removes any, so watch
  `dli-world` against the cap.
