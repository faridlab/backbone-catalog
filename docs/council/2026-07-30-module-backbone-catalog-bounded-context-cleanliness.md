<!--
date: 2026-07-30 | repo type: module | unit: backbone-catalog | focus: bounded-context-cleanliness
question: which VINSTEKNIK (Laravel ecommerce) product concepts should catalog adopt? (user named categories + showcases)
roster: chair (subagent), skeptic (subagent), steelman (subagent), yagni-business,
        ddd-bounded-context, contract-seat, domain-expert (invited)
evidence: VINSTEKNIK product-model map (vinsteknik-service) + catalog ADR-001/003/004/005 + schema SSoT
-->

# Council — module:backbone-catalog — focus: bounded-context-cleanliness

## Best call

**Adopt zero new concepts into `backbone-catalog` this quarter. "Categories" (base) is already satisfied by `ItemGroup`; the showcase concept is a `backbone-selling` projection (build there, via ADR-001 §5 logical FK, when a storefront consumer materializes); S1/S2/S4/S5 stay parked per the ADRs' own consumer-gated terms.**

This sides with the Skeptic + YAGNI seats against the Steelman. The steelman's load-bearing premise — "ADR parking lots signal planned scope we should complete" — is false on the record: ADR-004:53 says verbatim "deferred until a consumer needs them," and ADR-001:35 / ADR-003:59 / ADR-005:52 repeat the deferral for the exact items in question (S1). The gate is unmet: zero in-tree consumers of catalog (verified across all workspace Cargo.toml + .rs files). The "showcase" concept (S3) fails the bounded-context test directly — ADR-001 §3 forbids widening Item for a new channel, and "Collection = group X + tag Y" is a saved query, which is selling's job, not identity's. Even S2 — the one concept the skeptic rates merely "erodes" — has no consumer, and the workspace's own rule says wait.

- **Residual negative value**: (1) S2 dimensions stay absent — when a shipping/inventory consumer arrives, ~0.5 day to add 3 nullable decimals + migration; **zero rework** (purely additive). (2) If a storefront lands before `backbone-selling` is staffed, showcases risk being built ad hoc in-situ — ~1–3 days rework; mitigated by making the routing decision (selling, via §5 logical FK) now. (3) S1 deferred — cost of delay ~0 until a consumer; ~1–2 days when a consumer names the constraint shape. Total expected rework of waiting: under one person-week, all additive.
- **Reversibility**: **easy**. Adopting nothing is trivially reversible — every parked item is additive.
- **What would flip this**: a named in-flight consumer this quarter needing one of these. **Cheapest probe (becomes the call if you can't answer)**: name the consumer needing any of these this quarter. If "none," this call stands.

## Disagreement map

- **"Parking lot = planned scope" vs "consumer-gated deferral"** — Steelman vs Skeptic. Crux: the ADRs' own text (ADR-004:53 "deferred until a consumer needs them" + ADR-001:35 + ADR-005:52). Steelman framing collapses on the record. *(Skeptic.)*
- **"Showcase = rule-based taxonomy in catalog" vs "saved query in selling"** — Steelman vs DDD + YAGNI. Crux: VINSTEKNIK etalase = web storefront; "Showcase" imports display semantics; ADR-001 §3 + §5 forbid catalog absorbing a per-channel projection. *(DDD/YAGNI.)*
- **"S2 is cheap + identity-grounded, adopt now" vs "even S2 is consumer-less, wait"** — Steelman vs Skeptic + YAGNI. Crux: `weight_per_unit` precedent (item.model.yaml:110-114) makes S2 the strongest counter — but consistency under the consumer-gate wins. S2 is first to re-open. *(Skeptic/YAGNI.)*

## Adoption verdict (per VINSTEKNIK concept)

| Concept | Adopt into catalog? | Why | Where it belongs |
|---|---|---|---|
| **Categories (base)** — `ItemGroup` | **Already adopted** | Catalog owns ItemGroup (category tree: parent/level/sort/code/name/status). Base case satisfied. | backbone-catalog (done) |
| **S1 — category attribute constraints** (allowed axes per ItemGroup) | **Defer** | Triple-parked (ADR-001:35, ADR-003:59, ADR-005:52); zero consumers; nothing half-built (steelman conflates per-variant integrity with a category mandate). Real retail rule, premature by the ADRs' own terms. | backbone-catalog — re-open when a POS/storefront variant-picker consumer names the constraint |
| **S2 — item dimensions** (length/width/height) | **Defer** (cleanest) | weight_per_unit precedent (item.model.yaml:110-114) → identity-grounded, ~zero coupling; still consumer-less. First to re-open. | backbone-catalog — when a shipping/inventory consumer needs it |
| **S3 / "Showcases" — collections** | **Reject for catalog** | "Collection = group X + tag Y" is a saved query; VINSTEKNIK etalase = web storefront; name imports display; ADR-001 §3+§5 forbid. | **backbone-selling** (storefront projection), referencing catalog.ItemGroup.id + catalog.Item.tags via logical FK |
| **S4 — category item-type constraint** | **Defer** (weak) | item_type already on Item; low-value until a consumer forces it. | backbone-catalog — eventually |
| **S5 — category disambiguation description** | **Defer** (trivial) | Classification metadata, clean, but consumer-less; ~1-field add when a UI needs it. | backbone-catalog — when a UI needs disambiguation |
| **S6 — canonical media reference** | **Reject** | Half-admits media defers to bucket; god-entity smell. | backbone-bucket (media) |
| **Commerce cluster** (reviews, promos, stock, slugs/SEO, wholesale, wishlist, last-seen) | **Reject** | Steelman correctly routed these out. Each is a projection via catalog.Item.id logical FK. | promo/selling, inventory, SEO overlay, engagement/CRM |

## Parking lot

- **S1 — category attribute constraints** — scope: `item_group_allowed_attributes` table + write-service validation. Re-open: first variant-picker consumer.
- **S2 — item dimensions** — scope: 3 nullable decimals on Item (siblings of weight_per_unit). Re-open: first shipping/inventory consumer. Lowest-friction; re-evaluate first.
- **S3 — showcases/collections** — scope: rule-based taxonomy projection. Belongs in **backbone-selling** as a saved-query entity referencing catalog via ADR-001 §5 logical FK. Re-open: a storefront channel project greenlit.
- **Bundle/rental terms** — already parked (ADR-004:53). Re-open: in-tree consumer.
- **Per-item UOM conversions / price lists** — already parked (ADR-001:35). Re-open: in-tree consumer.

**Bottom line: adopt nothing into catalog now; showcases are a backbone-selling projection; re-open S2 first, then S1, the day a named consumer arrives.**
