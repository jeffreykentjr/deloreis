---
status: exploring
verdict: "Characterizes the design space for evolving deloreis from single-sovereign canon to multi-authority, type-rich, dynamically-inferred knowledge. Maps prior art (Datomic, Prolog derivatives, ASP), development sequencing by lift/value, and key technical challenges. Exploratory, not committed."
---

# Multi-Authority Knowledge Representation: Beyond Single-Sovereign Canon

## Current State: Single Sovereign, Two Doors

The current architecture (DL-0002, DL-0004, DL-0005) makes three intertwined assumptions:

1. **One sovereign authority** — There is one canonical valuation, one set of decisions that are "ratified."
2. **Two doors to knowledge** — Humans write `.dli` text (Door H), agents call MCP tools (Door T).
3. **Static type schema** — Entity kinds have fixed slots; the schema is part of the world definition.

## The Shift: Authority Boundaries, Not Doors

The fundamental reframe: **eliminate the H/T distinction entirely**. All knowledge creation happens through **declarative agent interactions** via MCP tool calls. The real distinction is **authority boundaries**:

```
Authority: Marvel Comics (Earth-616)
Authority: Ultimate Marvel (Earth-1610)
Authority: Fanon Collective (Unspecified Reality)
Authority: Official Handbook Editorial Board
Authority: Movie Adaptation Team (MCU)

# Each authority has its own valuation context
# Facts can be scoped to authorities or cross-authority
```

## The Core Tension: Dynamism vs. Rigor

> We want type refinement and inference capabilities like logic programming, but we must preserve the dynamism of "code is data; data is code" — no recompile steps, possible worlds on the fly.

This tension is resolved by **layering rigor on top of dynamism**, not replacing dynamism with rigor.

### Datomic: Schema as Data
Datomic treats schema **transactions as data** — schema evolution happens through the same transactional interface as fact insertion. No recompile steps.

### Prolog: Types as Logical Constraints
Typed Prolog derivatives treat types as **logical predicates** that constrain acceptable inputs — types are **inferred logically**, not checked syntactically.

### Clingo: Stable Model Semantics
Answer Set Programming allows **multiple stable models** — each is a valid "worldview" — directly mapping to our "possible worlds" concept.

## Design Space Mapping

### Layer 1: Authority Model (Medium Lift, High Value)

Replace single sovereign valuation with **trust-weighted, composable authorities**.

- Each fact carries an `authority` tag
- Authorities have **trust weights** and **scope domains**
- Resolution strategy: weighted combination, conflict detection, provenance tracking

**Prior art:**
- **TNL-Datalog**: Datalog with trust annotations
- **SPKI / KeyNote**: Formal trust composition
- **ASPIC+ with preferences**: Ordered argumentation frameworks

**Dev lift:** Moderate. Requires extending the valuation model and adding trust propagation semantics to the polynomial evaluation. No schema changes needed.

### Layer 2: Constraint Validation (Low Lift, Medium Value)

A **schema-on-read** constraint system that validates facts without requiring recompilation.

```dli
rule person_requires_name(P)
  requires person(P) implies exists N . name(P, N)

rule standing_king_requires_territory(P)
  requires standing(P, king) implies exists T . sovereign_of(P, T)
```

Constraints compile to additional **support polynomials** — a constraint violation is a fact whose polynomial is unsatisfiable.

**Prior art:**
- **SHACL** (RDF): Declarative constraints with logical forms
- **Liquid Haskell**: Refinement types
- **Spec# / Viper**: Verification via injected assertions

**Dev lift:** Low. Adds a constraint compilation phase that generates additional polynomials. The evaluation engine already handles unsatisfiable polynomials.

### Layer 3: Argumentation Framework (High Lift, High Value)

Facts are supported by **arguments**, arguments **attack** other arguments, and **acceptability semantics** determine which arguments are "in" or "out."

```dli
argument a1: "Spider-Man is Peter Parker"
  supported_by: [d-comic-1962-08-25, d-official-handbook-1980]

argument a2: "Spider-Man is Miles Morales"
  supported_by: [d-ultimate-universe-1610, h-spider-verse-fanon]

argument a3: "Only one person can be Spider-Man"
  supported_by: [d-unique-identity-principle]
```

**Prior art:**
- **ASPIC+**: Standard framework for structured argumentation
- **Defeasible Logic (DL+, Nute)**: Rule-based defeasible reasoning
- **Logic Programming with Negation as Failure**: Related attack semantics

**Dev lift:** High. Requires rethinking the evaluation model from "facts are true/false/dead" to "arguments are in/out/undecided."

### Layer 4: Uncertainty Propagation (Medium-High Lift, Medium-High Value)

Extend the grade lattice from recheckability to **confidence/uncertainty**.

```dli
fact spider_sense_triggers
  confidence: 0.87
  provenance: "observed in 6 issues, 2 movies, 1 game"
  authority: [Marvel_Comics: 0.95, MCU: 0.90]
```

The grade lattice becomes a **product lattice**: `(checked > tested > attested > asserted) × (confidence ∈ [0..1]) × (authority_weight)`.

**Prior art:**
- **ProbLog, TriRL**: Probabilistic Datalog
- **LPFP, FLP**: Fuzzy Logic Programming
- **Credal Networks**: Imprecise probability over logical structures

**Dev lift:** Medium-high. Requires extending the truth value space beyond boolean and modifying the join/sup operations in the fixpoint.

### Layer 5: Fuzzy Entity Resolution (High Lift, Medium Value)

Entities aren't exact atom matches — they're **probabilistically linked** across sources.

```dli
entity_link spiderman_comics ≈ spiderman_mcu
  confidence: 0.95
  evidence: ["same alter_ego", "same core_powers", "different_origin_story"]
```

**Prior art:**
- **Fellegi-Sunter**: Statistical record linkage
- **Entity matching in KGs**: ML-based entity resolution
- **OWL SameAs reasoning**: Logical equivalence relations

**Dev lift:** High. Requires a new reasoning layer over the entity store and integration with the valuation model.

## Development Sequencing by Value/Lift Ratio

| Layer | Lift | Value | Why Now? |
|-------|------|-------|----------|
| **Constraint Validation** | Low | Medium | Immediate safety net for complex schemas. |
| **Multi-Authority Model** | Medium | High | Foundation for everything else. |
| **Uncertainty Propagation** | Medium-High | Medium-High | Makes the system realistic for messy knowledge. |
| **Argumentation Framework** | High | High | The "killer app" for contested knowledge. |
| **Fuzzy Entity Resolution** | High | Medium | Powerful but niche. |

## Why Not Just Use Datomic/Clingo?

### Why Not Datomic?
- **Single consistent timeline** (no contested canon)
- **ACID transactions** (not always available in distributed agent settings)
- **EPL queries** (not polynomial-based provenance reasoning)
- **SQL-like schema** (not logic-program-style inference)

### Why Not Clingo?
- **Static program analysis** — encode entire problem as ASP rules
- **No provenance tracking** — derivation chains are lost
- **No incremental updates** — changing one fact can require full recomputation

### Why Not Typed Prolog Derivatives?
- **Compile-time type checking** — the whole point is to avoid recompiles
- **Static world models** — no "possible worlds on the fly"
- **Single authority** — no trust-weighted conflict resolution

## Technical Challenges

### 1. Product Lattice Explosion
Combining `grade × confidence × authority_weight` creates a **product lattice** that can become unwieldy.

### 2. Negation Semantics for Multiple Authorities
Current negation is "valuation-neutral poison." With multiple authorities, negation becomes **context-dependent**, requiring **credulous vs. skeptical** evaluation modes.

### 3. Performance Degradation from Argumentation
ASPIC+ style argumentation can have **exponential** complexity in the number of arguments.

### 4. Incremental Recompilation of Constraints
Adding a new constraint must **retroactively** check existing facts, requiring a **change propagation** mechanism.

## What Preserves Our DNA

Despite all extensions, three principles must remain:

1. **Declarative interaction** — All knowledge through tool calls/declarations
2. **Polynomial provenance** — Every fact still has a support polynomial
3. **Possible worlds without recompile** — Forks and hypotheticals are lightweight

## Open Questions

1. **Can argumentation frameworks be compiled to polynomials?**
2. **How does the grade lattice extend to continuous confidence?**
3. **What's the minimal authority model extension?**
4. **Can constraint validation be lazy?**
