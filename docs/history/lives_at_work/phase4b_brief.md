# Lives at work, phase 4b: the pimp, the dealer, the street trades' regulars

The second half of phase 4 (`phase4_brief.md`, split 2026-10-18): blocks E-G. Phase 4a (the cab, the parcel, the lift, the client) is merged. You are a fresh agent.

## Start

1. `git merge --ff-only main`; `git log --oneline -1` must show the sha the orchestrator named.
2. Read, in this order and no more up front: `docs/LIVES_AT_WORK.md` § 14's phase 4a block (what 4a built, what it left you: if it left D, the client, D is your first item); `phase4_brief.md`'s anchor rows **the client** (only if left), **the pimp, the dealer, vendor/rickshaw/tinker, config, counters, parity**; `docs/LIVES_AT_WORK.md` §§ 0 (D4, D6), 1.3, 2.3's pimp, dealer and vendor rows, 6's matching rows, 9.1 (Phase 4).

The switch is `CITYSIM_LIVES_STREET_OFF=1` (`[service] street`, built by 4a). Bump SAVE_VERSION once if you add a saved shape (`jobs2.vice.pimp_seen`, `World.corner_dealers` will).

## Build, in order

- **E, the pimp**: the pimp row (presence within `pimp_tiles`; the Statistical pimp credited on `gang::following_order`, a deviation to write; the round in `gang_work_target`; `street_cut` to the pimp with `pimp_pass_up` to the gang; `pimp_seen` and `name_pimps` dropping the absent).
- **F, the dealer**: the dealer row (`stims::deal_corner`, `World.corner_dealers`, the `Buyer` encounter in `buy_stims`/`stat_buy`, the regular bought first, the lookout's `witness_mult`).
- **G, the regulars**: vendor, rickshaw (already in 4a's `full_cab`), tinker → `service::encounter`.

Tests (from `phase4_brief.md` § Tests, the 4b ones): the street assault reads the pimp's presence, not his freedom; a dealer's third buyer is a regular with an edge and `stat_buy` picks the dealer the buyer knows; a lookout halves the witness term; the off city still writes nothing in 3 days; the pimp probe in `tests/person.rs` (a pimp's purse grows on a pitch night); the printed bullets in `tests/core.rs` for a pimp's collection and a dealer's regular; the `tests/lod.rs` row for stims dealt per adult-day by tier. Each fails on the base first. During the work run only these by name.

**Budget: ~300 turns.** If G will not fit, hand back E-F green and name G in § 14.

## Measure once, at the end

```
python tools/phase_measure.py --base <the sha you were briefed on> --off CITYSIM_LIVES_STREET_OFF \
  --arch gang_member,vendor,cabbie,homeless \
  --cols street_visits,street_assaults,street_take,pimp_cut,stims_dealt,dealing_reports,vendor_meals,tinker_repairs,pimp_collections,pimp_pass_up,corner_deals,dealer_regulars,lookout_halved \
  --events Murder
```

The device: `gang_member` `met` up (dealers and pimps), street assaults moving with the pimp's presence, `OFF IDENTITY: IDENTICAL`. Paste `measure.md` into the report. Then `python tools/test.py change` once and `python tools/test.py lint`; footers verbatim. Not this phase (the gate runs them): 42-47, the 42-44 off identity, A/B, save size, diaries. Logs on disk; `tail`/`grep`, never `cat`. Cap a diagnosis at ~30 minutes.

## Hand back

One squashed commit on your branch (message: the phase, each mechanism's Statistical mirror, deviations, numbers against the base, the attribution lines); never merge to main. In the commit: § 14 (a phase 4b block), § 13 after-rows, `docs/ARCHITECTURE.md` § 6/§ 7 (`[stims] deal_corner`, the pimp keys). Report (≤ 100 lines) at the path the orchestrator names: final sha, what was built, the measure table, the footers, deviations, anything left.
