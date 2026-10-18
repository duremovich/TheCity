# Lives at work, phase 4a: the cab, the parcel, the lift, the client

Phase 4 (`phase4_brief.md`) is split in two (sixteenth session, 2026-10-18, the token rules): **4a** finishes the cab and builds the parcel, the lift and the street worker's client (the brief's blocks A-D); **4b** builds the pimp, the dealer and the street trades' regulars (E-G). You are a fresh agent; the first coder parked a WIP commit.

## Start

1. `git merge --ff-only main`; `git log --oneline -1` must show the sha the orchestrator named.
2. `git cherry-pick 2c22096` (the WIP: block A and the scaffolding, on top of `1f37f78`; main has moved only by docs and tools since, so it applies clean).
3. Read, in this order and no more up front: `docs/history/lives_at_work/phase4_report.md` § "Status at the session close" (what is built, the failing test, the next three steps); `phase4_brief.md`'s anchor rows **the cab roll, the driver's body, the parcel, the lift, the client, config, counters, parity** (the pimp, dealer and vendor rows are 4b's); `docs/LIVES_AT_WORK.md` §§ 0 (D4, D6), 1.3 (no `ActionKind` interrupted mid-shift), 2.3's cab, courier, loader and street-worker rows, 6's matching rows, 9.1 (Phase 4) and § 14's phase 1-3 blocks for the house style of a deviation.

The switch is **`CITYSIM_LIVES_STREET_OFF=1`** (`[service] street`), not the brief's `CITYSIM_STREET_OFF`, which is Jobs & lives' `[jobs2] street` and turns the whole street economy off. SAVE_VERSION is 38 in the WIP; bump again only if 4a changes a saved shape beyond it (the `Parcel` vec is already on `Jobs2State`).

## Build, in order

- **A, finish the cab**: the failing `tests/street.rs::test_full_commuters_cab_is_timed_and_a_driver_drives_it` at "the fare paid" (the report's step 1: read the printed `Jobs2Cols` tail, find why `pay_ride` moved no coins). Then drop its `eprintln!`.
- **B, the parcel**: `phase4_brief.md`'s parcel row.
- **C, the lift**: the lift row, including the owner's `p_short_report` Theft report after `industry::pilfer`'s hit.
- **D, the client**: the client row (`vice::street_hour`'s body-first pick, `service::encounter` with no place, `street_assault` taking the client as its actor).

Tests (from `phase4_brief.md` § Tests, the 4a ones): the cab roll shared by both tiers (passes), the off city writes nothing (passes), a parcel delivered to a present recipient and returned at shift end otherwise, a pilfered unit files a Theft report on the hashed hit and never at `p_short_report = 0`, the cabbie probe in `tests/person.rs` (a split-shift cabbie drives at least one fare a day with `p_cab` raised in-test; control `p_cab = 0`), and the printed bullets in `tests/core.rs` for a cab fare by a named driver, a parcel to a named recipient and a lift. Each fails on the base first (doctrine 9). The `tests/lod.rs` row for cab rides per commuter-day by tier. During the work run only these by name (`cargo test --release -p citysim --test street <name>`).

**Budget: ~300 turns.** If D will not fit, hand back A-C green and name D in § 14 as 4b's first item; do not rush it.

## Measure once, at the end

```
python tools/phase_measure.py --base <the sha you were briefed on> --off CITYSIM_LIVES_STREET_OFF \
  --arch cabbie,courier,loader,vendor,homeless,day_labourer \
  --cols cab_trips,cab_fares,rickshaw_trips,parcels,pilfered,street_visits,street_assaults,street_take,cab_rides_driven,cab_rides_unmanned,fares_short,parcels_delivered,parcels_returned,lifts,pilfer_reports,street_clients_near \
  --events FareShort,Pilfered,Murder
```

One command: the seed-42 × 30 off identity against main's binary (built from `git archive`, run from its own root, cached), the 42-44 × 120 counters base -> yours, and the `shadow --lives` rows. It writes `measure.md` under `target/measure/`; paste it into the report. If an `--arch` name is wrong the script lists the known ones. The device: courier and loader `met` off their 0.00-0.06 baseline, cabbie `met` above 0, `OFF IDENTITY: IDENTICAL`. Then `python tools/test.py change` once and `python tools/test.py lint`; footers verbatim.

Not this phase (the gate runs them once): 42-47, the 42-44 off identity, alternating A/B, save size, diaries.

Logs stay on disk: `tail`/`grep` them, never `cat`. Cap a test diagnosis at ~30 minutes, then write it as open.

## Hand back

- **One squashed commit** on your branch (the WIP plus your work; `git reset --soft <briefed sha>` then commit), message: the phase, each mechanism's Statistical mirror (§ 6), the deviations, the numbers against the base, the session's attribution lines. Never merge to main.
- **Docs in the commit**: `docs/LIVES_AT_WORK.md` § 14 (a phase 4a block: the WIP's deviations from `phase4_report.md`, the pilfer already built, the crash roll skipped, anything left for 4b), § 13 after-rows from the measure; `docs/ARCHITECTURE.md` § 6 (`streetwork.rs`'s fare plan, `service::encounter`), § 7 (the `[service]` street keys).
- **Report** (≤ 100 lines) at the path the orchestrator names: final sha, what was built, the measure table, the footers, deviations, what is left for 4b.
