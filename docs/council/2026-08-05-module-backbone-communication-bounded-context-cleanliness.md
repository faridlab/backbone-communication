---
date: 2026-08-05
repo_type: module
unit: backbone-communication
focus: bounded-context-cleanliness
roster: chair, skeptic, steelman, yagni-business, ddd-bounded-context, contract-seat, domain-expert(invited)
---

# Council — module:backbone-communication — focus: bounded-context-cleanliness

## Best call
**Wire `CommunicationWriteService` into `CommunicationModule` and expose one validated write route** (field on the struct at `src/lib.rs:57-62`, construct in the builder at `src/lib.rs:139-160`, mount a write router alongside `readonly_routes()`). This installs the front door the regen claimed to add but didn't.

- Residual negative value: ~1-2 days to wire (field + builder + minimal handler/sub-router); introduces a published outward write contract with a ~2-3 week drift window before a sibling consumes it; the RLS-fence parking-lot issue (company_id defaults to all-zeros on pre-existing rows) becomes live the moment the write path is reachable, so residual risk is one RLS-bypass class on legacy rows until that migration is fixed.
- Reversibility: easy on the wiring (a field + a route mount are trivially removable); costly on the contract once a sibling calls `receive_inbound` / `mark_delivered`.
- What would flip this: a clean-slate regen into a temp dir (the skeptic's probe) showing the codegen template already emits a wiring block for the write service in a newer revision — in that case manual wiring creates merge conflicts with the next regen and the call flips to "run the probe first, then wire from the template output."

## Disagreement map

1. **Is the regen "guardrails tightened" or "the front door was removed"?**
   - Crux: the regen narrowed `message_service`/`thread_service` to `pub(crate)` AND deleted `CommunicationQueryService`. Net effect on the outward contract.
   - Steelman / contract-seat-aligned view: tightening direction is correct — raw `GenericCrudService` mutators should never have been the published seam.
   - Contract-seat / DDD-seat view: but the module now offers siblings NO service-level contract (no read contract since QueryService is gone; no write contract since WriteService isn't wired). The ACL is half-built — DTOs/events in `exports/` with no service behind them. The Chair fact decides this for the write side: the write service exists in `application/service` but is not in `module.rs`, so the regen did not install a front door, it removed both.
   - Resolution: contract-seat wins. Tightening was right; leaving nothing in its place is the regression. Best call wires the write side back.

2. **Is `value_objects/` dead code or a latent context violation?**
   - Crux: does it mislead the next reader / risk a silent re-wire on the next regen?
   - Yagni-business + DDD-seat: dead modeling, no live rule — `git rm` it; cost of waiting is near-zero.
   - Skeptic: the primed `value_objects/mod.rs` re-exports are a trap — a future template flip silently re-wires them, AND they paper over a real SSoT break (`Metadata` vs `AuditMetadata`).
   - Resolution: skeptic's concern is real but the active harm is the SSoT break, not the dead files. Removing the dir is cheap and removes one source of confusion; fixing the type rename is the substantive move.

3. **Schema-as-SSoT: is the contract void for `@audit_metadata` fields?**
   - Crux: schema says `Metadata`, code ships `AuditMetadata` — which name wins?
   - Skeptic + domain-expert agree: this is a real ubiquitous-language break; the schema-named `Metadata` is unreachable and the live type is hand-defined in `entity/mod.rs:61`.
   - Steelman: codegen substitution is a known internal mechanism; the entity compiles.
   - Resolution: skeptic + domain-expert win. The CLAUDE.md SSoT claim is void for audit fields until the rename is reconciled.

## Recommendations (ranked by leverage)

| # | Move | Leverage | Residual negative | Reversibility | Evidence to flip |
|---|------|----------|-------------------|---------------|------------------|
| 1 | Wire `CommunicationWriteService` into `CommunicationModule` (field at `src/lib.rs:57-62`, build at `src/lib.rs:139-160`, mount a write router alongside `readonly_routes()` at `src/lib.rs:101-110`). | Installs the missing front door; converts the regen's "tightened" framing from aspirational to true; gives siblings a real outward write contract. | ~1-2 days; ~2-3 week contract-drift window; RLS-bypass risk on legacy rows until the parking-lot migration is fixed. | Wiring easy; contract sticky once consumed. | Clean-slate regen shows the template already wires it — then defer to the template. |
| 2 | Reconcile the `Metadata` vs `AuditMetadata` SSoT break: pick ONE name, update both `schema/models/{message,thread}.model.yaml` and `src/domain/entity/mod.rs:61`. | Restores the schema-as-SSoT invariant for every `@audit_metadata` field; removes the silent-substitution trap. | ~half a day; if you pick `Metadata`, downstream OpenAPI/migration renames cascade. | One-way door on the chosen name; reversible at code level. | Codegen maintainers confirm `AuditMetadata` is the canonical generator output — then the schema YAML is the side to change. |
| 3 | `git rm -r src/domain/value_objects/` (the dir is not in the crate graph — `src/domain/mod.rs` does not declare `pub mod value_objects`). | Removes ~413 lines of misleading dead code; eliminates the primed re-export trap the skeptic flagged. | Near-zero; only that a future template revision expects the dir and you re-create it on regen. | Trivially reversible (`git revert`). | A template version pins `value_objects` as a generated output dir — then keep it. |
| 4 | Run the skeptic's clean-slate probe: regen into a temp dir, `diff -r` against the working tree. | Settles all four questionable outcomes (orphaned VOs, Metadata substitution, QueryService deletion, write-service wiring) in one shot before you spend the wiring effort. | ~1-4 hours; zero code risk; could delay move #1 by half a day. | Fully reversible. | The diff is empty for the four contested files — then moves #1-#3 are unaffected and proceed. |
| 5 | Re-publish an outward read contract for siblings (the role `CommunicationQueryService` filled) via `exports/` — a trait or a thin read-service surfaced alongside the write service. | Restores the read-side promise the regen deleted. | ~half a day; adds a second surface to maintain. | Medium — once siblings depend on it. | No in-workspace sibling currently needs a programmatic read API (HTTP `readonly_routes()` suffices) — then defer (yagni). |

## Parking lot
- RLS-fence migration defaults `company_id` to all-zeros UUID on pre-existing rows, silently passing `app.company_id` policy — raised by skeptic, scope: `migrations/` (not the bounded-context lens; becomes load-bearing the moment move #1 ships).
- Dual write path (generic CRUD `CreateMessageDto` vs `CommunicationWriteService` domain command) should be explicitly demoted to admin-only in docs/route composition once move #1 lands — raised by DDD-seat, scope: `src/lib.rs` route composer docs.
- Orphaned `Metadata`-named `@audit_metadata` schema field affects every entity in the workspace that uses the directive, not just this module — raised by skeptic + domain-expert, scope: root (codegen template).
