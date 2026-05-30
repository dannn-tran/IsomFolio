# Dev Docs — Philosophy & Conventions

This file documents how IsomFolio's dev docs are written and maintained.

---

## The test for whether something belongs in a doc

> A dev doc should answer a question that a developer couldn't resolve by reading the code in 5 minutes.

If the answer is obvious from well-named code, skip it. Document: **why** decisions were made, **what** invariants must hold, and **how** subsystem boundaries work when the interface is non-obvious.

---

## Three types of docs

### 1. Normative specs

*What must stay true.* `design-system.md` is the canonical example.

- Language is prescriptive: "must", "never", "always"
- These go stale only when the rule itself changes — which is intentional
- Referenced from `CLAUDE.md` so they are consulted before any relevant change
- Detail level: high. Every rule must be specific enough to be testable in code review.

### 2. Architecture docs

*How subsystems interface; why the design is the way it is.* `architecture.md` is the example.

- Describe invariants and decisions, not implementations
- No line-number references — they rot immediately
- A good test: does this paragraph survive a refactor that preserves the same behaviour? If yes, it's architecture. If no, it's an implementation detail and doesn't belong here.
- Detail level: medium. Enough to orient a contributor without narrating the code.

### 3. Temporal docs (plans, TODOs, audits)

*What we intend to do / what we found at a point in time.*

These have a natural expiry: after implementation, they become noise. **Don't commit them.**

- Plans belong in PRs and commit messages, not in source-controlled files
- Open work belongs in GitHub Issues
- Audit findings: the fix goes in a commit; the rationale in the commit message
- Roadmaps go stale as priorities shift; keep them in a project board, not a .md file

---

## Axes for slicing IsomFolio docs

Slice by **stable concern**, not by "what I was working on when I wrote it."

| Doc | Axis | Why it's stable |
|---|---|---|
| `design-system.md` | Visual + interaction system | Rules change only when design intent changes, not when code is refactored |
| `architecture.md` | Crate structure, data flow, subsystem contracts | Invariants survive refactors; describes WHY not WHAT |
| `DEVDOCS.md` | How to write docs | Meta — changes rarely |

Three docs. Not more. Each answers a distinct question.

---

## Keeping docs honest

The failure mode is docs describing intent while code diverges silently. Two mechanisms that work:

1. **`CLAUDE.md` references** — force doc consultation before changes in that area. Already in place for `design-system.md`.
2. **Prescriptive language over descriptive** — "The extension protocol uses stdout for protocol and stderr for diagnostics — never mix" stays true across rewrites. "The `MessageWriter` class writes to `TextWriter output`" becomes false after a rename.

The mechanism that doesn't work: "we'll just remember to update the docs." Docs need to be either enforced (via CLAUDE.md) or so small that updating them is trivial.

---

## What not to document

- Things obvious from well-named code or types
- Completed plans, shipped roadmaps — they belong in commit history
- Bug lists — they belong in GitHub Issues
- DB schema that mirrors the `MIGRATIONS` constant exactly — the constant is the source of truth
- Implementation details that change with every refactor
