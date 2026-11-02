---
status: reference
title: "Scale and distribution: peer, transactor, and bounded evaluation"
verdict: "Parked design for a possible distributed peer and service; not part of the fiction release."
---

# Scale and distribution

This parked note records a possible archive-scale boundary. It does not claim
that a managed service, remote transport, or Magic Set router is implemented
today. Current lorestore remains the embedded, deterministic implementation;
the distribution milestones in `../PLAN.md` are deferred.

## 1. The split

At atelier scale, an embedded process can load and symbolically saturate a
whole corpus. At archive scale, DLI keeps the same fact model and semantics but
separates I/O-bound retrieval from CPU-bound symbolic evaluation:

| Component | Owns | Does not own |
|---|---|---|
| **Peer** | Immutable local object cache, exact symbolic fixpoint over a bounded cluster, valuation overlays, standing, `why`, and local hypothetical branches. | Write serialization, application authorization, or global policy. |
| **Managed service** | Commit/ref serialization, immutable object and index storage, and query-relevant cluster construction. | Final standing, provenance valuation, or consensus policy. |
| **Consuming application** | Authentication, authorization, review, authority policy, agent orchestration, and human escalation. | Reimplementing DLI's semantics. |

The peer is an embeddable Rust component and may also be compiled to WASM. The
first implementation has the service role run in the same process as the peer;
network transport and remote object storage are later substitutions behind the
same semantic boundary.

## 2. Snapshot-pinned provenance clusters

A peer asks for a cluster using a root commit (or ref resolved to one), a typed
goal, and its valuation context. The service answers with a manifest pinned to
that root and the complete immutable inputs needed for that goal: base facts,
rules, relevant defeaters, and the referenced content-addressed objects.

The manifest is not a best-effort retrieval result. It carries enough identity
information for the peer to reject a mismatched root, missing object, or
incomplete response. The defining oracle is exactness:

> For the same root, goal, and valuation, evaluating a valid cluster in the
> peer yields the same bindings, standings, and provenance as evaluating the
> complete corpus.

If the service cannot construct such a finite cluster, it must say so. It may
not return a partial cluster as though it were an answer. This preserves DLI's
replayability law across the network boundary.

## 3. Supplementary Magic Sets

The service uses **supplementary Magic Sets** (with sideways information
passing) to construct clusters. A typed goal seeds bound arguments; the
transformation propagates those bindings through rules and indexed base facts;
supplementary predicates retain useful partial bindings while avoiding wide
intermediate joins. The result preserves bottom-up, set-oriented evaluation
while restricting it to the goal-relevant subgraph.

SLG/tabling is not the archive router's execution model. It would make the
peer depend on a stateful, tuple-at-a-time remote evaluation session and blur
the peer/service boundary. Magic Sets let the service ship a finite,
snapshot-pinned cluster while the peer retains autonomous, local symbolic
evaluation.

The service may cache or materialize routing/index results, but those are
derived readers. They never replace the committed fact/commit/ref model and
must be rebuildable from it.

## 4. Database properties at both halves

The architecture borrows database concepts without changing the epistemic
model:

| Database concern | DLI equivalent |
|---|---|
| Append-only log | Content-addressed commit DAG and immutable fact objects. |
| Snapshot isolation | A root hash plus valuation identifies a reproducible read context. |
| Index segment | A snapshot-pinned provenance cluster, not an authoritative derived answer. |
| Materialized view | A hot-core or peer-local symbolic fixpoint, rebuildable from committed facts. |
| Compaction | Derived packfiles, indexes, or snapshots that preserve all committed objects and query meaning. |
| Tombstone | Not a DLI defeater: a defeater conditionally invalidates a proof path under a valuation; it does not erase history. |

Content addressing removes semantic cache invalidation: an object identified by
its hash never changes. Peers may evict cached objects for local resource
reasons, but no peer may reinterpret an object or mix it with a different root
manifest.

## 5. Consensus boundary

This topology supports a consensus service without claiming one prematurely.
Agents can contribute structured claims and counterclaims; DLI can return their
derived standing, explicit conflicts, defeater paths, and minimal explanations.
That makes a deterministic referee possible. It does **not** yet define a
winner-selection rule, trust weighting, argumentation framework, probabilistic
belief model, or escalation workflow. Those are application policy or future
semantics, and require a later ledger ruling.

## 6. Non-negotiable invariants

- The offline gate and the peer's certifying evaluator call no network and no
  model.
- Facts and commits remain immutable; compaction never rewrites history.
- A valuation overlay changes evaluation, not the committed fact set.
- A benchmark result, model majority, or routed cluster cannot by itself make a
  claim certain; only the stored inputs and supplied valuation determine the
  result.
- The service verifies structure and referential integrity. Authorization and
  approval policy remain outside DLI, as required by DL-0054 and DL-0058.
