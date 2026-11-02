---
status: live
title: "Verdicts and evidence"
verdict: "Grades describe how procedures were verified; support records the rulings and assumptions on which conclusions depend."
---

# Verdicts and evidence

The kernel evaluates structured lore deterministically and offline. A caller
may use a model outside the kernel to propose or interpret lore, but the kernel
does not call a model or treat generated prose as a verdict. Attribution,
review, and permission to rule belong to the consuming application.

## The grade lattice

A verdict carries one of four verification grades:

```text
checked > tested > attested > asserted
```

- **checked**: a procedure re-derives exactly from committed inputs.
- **tested**: a procedure has statistical evidence from a reproducible test.
- **attested**: a stored model judgment can be inspected and contested, but
  is not mechanically re-derived.
- **asserted**: a claim is taken as given, without a verifying procedure.

Support and grade are separate. Support records the decisions and assumptions
that make a conclusion hold; grade describes the procedure used to reach the
verdict. A deterministic proof check over authorially ruled premises can be
`checked` even though the rulings themselves are asserted. A required procedure
chain rolls up to its weakest grade; independent alternative procedures roll
up to their strongest available grade. Neither rule turns repeated attestations
into a test.

## Attestations

Projects can contain committed attestation evidence. The project-backed
`dli.check` verifies and replays the evidence against configured questions and
reports current, missing, stale, or negative answers. Records retain the
question, answer, model and prompt identifiers, sampling policy, rationale
hash, fingerprint, and lineage needed to inspect them. A missing or stale
answer is a finding, not a guessed answer.

Contract 2.0.0 does not provide an attestation-sweep write operation. The
kernel reads existing evidence and reports demand; a consuming application
owns any external evidence workflow and review policy. A model judgment is
not proof of a claim and cannot silently create canon.

## Structured claims

The engine checks the claims, decisions, and rules a caller supplies. It can
report a contradiction, explain support and defeaters, or leave an unresolved
claim possible. It cannot establish that a transcription faithfully captures a
manuscript or that a model-generated assertion is true. Applications should
inspect source material and apply their own review rules when that distinction
matters.
