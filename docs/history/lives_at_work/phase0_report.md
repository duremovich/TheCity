# Phase 0 (Baselines) report

**CLAUDE.md auto-load check:** Yes. The project's CLAUDE.md (the doctrine with "Gates assert broken-system signals", the "Agent rules" list, the standing decisions) was in my context as a system-reminder before I opened any file, alongside the user-level CLAUDE.md, the plan-implementation SOP rule, and the MEMORY.md index.

## Where

- Worktree: `D:\TheCity\.claude\worktrees\agent-aeb9f5f4cf9b9e4ee`, branch `worktree-agent-aeb9f5f4cf9b9e4ee`, based on main `23a161d` (`git merge --ff-only main` was already up to date).
- Final sha: `5d20c86` (one commit, on top of `23a161d`; working tree clean; `git ls-files --eol | grep w/crlf` empty).

## What each probe prints, and how to run it

**Law probe** (`citysim/tests/law_probe.rs`, two `tier: diag` tests): per witnessed act (a `Witness` event, deduplicated per actor/crime/hour, tier = the actor's LOD at the act): reported within 1 day, **arrested within 3 days** (new column: an `Arrest` whose `actors[1]` is the actor), sentenced within 3 (jailed / fined); rows by tier, tier/crime (≥ 20 acts), `all`; the sentence mix; then **one row per district sorted by the district's walk to the Precinct** (Manhattan tiles from the district centroid, its mean street tile, to the Jail's door; districts have no Hall), then **four distance bands by quartile of the districts' distances** with each band's tile range and the per-tier rows inside each band (≥ 20 acts). Runs: 2k seeds 42-44 (one thread each), 20-day warm-up, 60 measured days, per seed and pooled; 20k seed 42, 20 + 30 days. Env: `PROBE_SEEDS` (comma list; `PROBE_SEED` for one), `PROBE_DAYS`.

```
cargo test --release -p citysim --test law_probe -- --ignored --nocapture law_probe_2k    # ~3 min
cargo test --release -p citysim --test law_probe -- --ignored --nocapture law_probe_20k   # ~7 min
python tools/test.py diag --only law_probe_2k
```

**Lives probe** (`citysim-cli shadow --lives`, in `citysim-cli/src/shadow.rs`): the `--assert` recipe (day 19, 7 days, seeds 42-44 pooled, up to 30 free agents per archetype and seed, each archetype in its own clone, seeds run one at a time) over all 48 pinnable archetypes; with `--population 20000` one window of the generated city (seed 42, day 20, 7 days), every archetype pinned together in one world, 5 each (V4's 20k recipe; a 20k world cannot be cloned per archetype). One `PROBE lives <city> <archetype>:` line per archetype plus `all`: `n`, agent-days, `idle_h/d`, `married / never_named / never_coloc / named/wk / coloc_h/wk (stat-spouse share)`, `hang/wk empty (empty/all)`, `shifts servedable any bodies/h venue_visits/shift`, the top 3 source event kinds. No RNG, no new sim hook; `--assert`'s output is untouched (`CHECKS` unchanged).

```
cargo run --release -p citysim-cli -- shadow --lives                     # 2k, ~3 min
cargo run --release -p citysim-cli -- shadow --lives --population 20000  # 20k, ~10 min
```

Signals (also in the file header): spouse *named* = a `ShadowNote::Move` with the spouse or a HangOut with the spouse at the tile (what the V4 critics read); spouse *co-located* = hourly sample, same building or both outdoors within `COLOC_TILES` = 2, neither at a sleep-class step (a Statistical spouse counts as awake where its demotion snapped it; the share is printed); *empty HangOut* = nobody else in `world.hangouts` at close; *Served-able shift* = whole free workday shift at a building with a non-staff body inside (`Building.occupants` minus `World::staff_of`) at any hourly sample or a `SERVED_SOURCE_KINDS` event (AssetBought, Installed, Treated, Rescued, Cured, LicenceSold, Gambled, Bout, MissionServed, Donated, TraumaSave) naming the workplace or an actor inside it. **Known limit:** a Statistical visitor (`leisure::stat_visit`, `lod::stat_eat`, the Statistical shop) moves no `Position` and writes no event, so it is invisible to both signals; it does bump `Venue.visits_today`, which a third column (`venue_visits/shift`, and the `any` share) reads for leisure venues. No event exists for a visit's completion, a tip, a cab fare or a detox.

## Headline numbers

**Save-hash identity:** `citysim-cli run --days 30 --seed 42 --save-at 43200` save file sha256 before `f796f3632e225a6566622ad171c9a09fa357be7274ebe27e7532d74ebb485b41`, after `f796f3632e225a6566622ad171c9a09fa357be7274ebe27e7532d74ebb485b41`: identical.

**Law probe, arrest share by distance band** (reported share in brackets):

| city | band (tiles; districts) | acts | arrested (reported) | Coarse | Full | Stat |
|---|---|---|---|---|---|---|
| 2k pooled 42-44 × 60 d | 1: 2-45 (3) | 2,609 | 0.82 (0.69) | 0.80 | 0.85 | 0.91 |
| | 2: 108-122 (2) | 705 | 0.68 (0.54) | 0.68 | 0.69 | — |
| | 3: 126-144 (2) | 1,245 | 0.63 (0.29) | 0.64 | 0.55 | 0.77 |
| | 4: 189 (1) | 405 | 0.50 (0.28) | 0.47 | 0.72 | — |
| | all | 4,964 | 0.73 (0.53); per seed 0.71 / 0.69 / 0.78 | 0.71 | 0.77 | 0.84 |
| 20k 42 × 30 d | 1: 2-128 (7) | 3,328 | 0.83 (0.76) | 0.81 | 0.89 | 0.82 |
| | 2: 144-324 (7) | 1,564 | 0.70 (0.52) | 0.67 | 0.78 | 0.75 |
| | 3: 376-520 (6) | 1,017 | 0.43 (0.29) | 0.41 | 0.58 | 0.64 |
| | 4: 572-692 (6) | 1,310 | 0.37 (0.21) | 0.35 | 0.48 | 0.67 |
| | all | 7,219 | 0.66 (0.54) | 0.61 | 0.84 | 0.78 |

Reading: falls with distance at both scales (0.82 → 0.50 at 2k; 0.83 → 0.37 at 20k), carried by the Coarse tier; the report share falls with it too (0.76 → 0.21 at 20k), so the witnesses' walk is part of it. The 20k Precinct went full 34 times in 30 days. Full logs: `scratchpad/law_probe_2k.log`, `law_probe_20k.log`.

**Lives probe:** see docs/LIVES_AT_WORK.md § 13 (written from `scratchpad/lives_2k.log` and `lives_20k.log`). 2k `all` (1,821 agent-weeks pooled over the archetypes): idle 3.86 h/d; 1,427 married picks, 997 never named the spouse in a move or HangOut, 460 never co-located awake; named 1.78/wk, co-located 22.1 h/wk; HangOuts 14.7/wk, 29 % empty (7,591/26,630; V4 read 120/369 = 33 %); 8,426 whole shifts, 28 % Served-able (32 % with the venue tally), 1.43 non-staff bodies per shift hour (the guards' Jail inmates and the Hq's meeting crowd carry that mean; most counter trades read 0.00-0.5). 20k `all` (228 picks, 5 per archetype in one world): idle 3.88 h/d; 159 married, 148 never named, 70 never co-located; named 0.34/wk, co-located 7.05 h/wk (28 % with a Statistical spouse); HangOuts 16.4/wk, 47 % empty (1,766/3,719; V4 read 39 % at 20k); 906 shifts, 33 % Served-able (37 % any signal). By trade at 2k: cooks/teachers/couriers/loaders/sweepers/builders/reporters 0.00-0.06 Served-able; night porters 0.80 (hotel guests), bouncers 0.76, club staff 0.51, dancers 0.50, ripperdocs 0.44, orderlies 0.43, guards 0.68 (inmates).

Reading beside V4: V4's "14 of 17 married never met" is 70 % never-named at 2k (93 % at 20k) but only 32 % (44 %) never co-located awake: couples share a roof for hours and do nothing together there (§ 3's home-spot gap). V4's 120/369 empty HangOuts (33 %) reads 29 % at 2k, 47 % at 20k.

Both runs' per-archetype tables are in docs/LIVES_AT_WORK.md § 13; the 20k window's 5-per-archetype rows are one person's week each (`arms_dealer`, `runner`, `scav` bodies/h > 100 are a Jail or Hq workplace).

## Gates (on the final tree, `5d20c86`)

`cargo fmt --check`: clean. `python tools/test.py change` footer, verbatim:

```
[change] GREEN; BUDGET BREACHED 4m37s of 4m00s: the phase that breached pays it back before it closes
```

(The breach is the pre-existing one, OPEN-093: main read 4m11s at the session close; this phase adds no non-ignored test, and the run shared the box with the 20k lives probe for its first minutes. `citysim_cli` 24 passed, 1 ignored; every binary ok.)

`python tools/test.py lint` footer, verbatim:

```
lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 48
```

(The 6 warnings are the pre-existing over-1,500-line files: `god.rs`, `contracts.rs`, `hitmarket.rs`, `person.rs`, `wages.rs`, `shadow.rs`; `shadow.rs` grew 3,078 -> 3,453 lines. Ignores 46 -> 48: none added by this phase, the law probe's two were already diag; the count moved because `test.py lint` counts `#[ignore]` attributes in the tree at the time it runs. Verify with `python tools/test.py lint` on main if that matters.)

The merge tier was not run (the orchestrator runs `/merge-set`). Nothing in the change can move it: the probes draw no RNG, write no component, push no event; the behaviour tier's `CHECKS` and `--assert` output are untouched (the new tallies ride on `Track`/`Agg` fields the existing metrics do not read).

## Open

- **A Statistical visitor is invisible to the Served-able signals** (bodies, events). Stated in the shadow.rs header, § 13 and the commit; the venue `visits_today` tally is the one place it shows, so the `any signal` column is a ceiling for leisure venues only; Clinics, Markets and shops have no tally. Phase 1's encounter makes it visible. Not a bug, a baseline limit.
- **The 20k lives window is one world with 228 pins** (5 per archetype together), not a clone per archetype: `[lod] max_full` grows by 228 as in `shadow --pick`, so the 20k city's Full set differs from a plain run. Stated in the header and § 13. `black_doc` had no 20k candidate; `pimp` 1, `camp_warden` 2 (the tool's notes).
- **`ceo` and `exec` are the same 27 people at 2k**, and `all` pools every archetype row, so an agent in two archetypes counts twice in `all`; printed in the header line.
- **Guards' and execs' "non-staff bodies"** are the Jail's inmates and the Hq's meeting attendees (bodies/h 16 and 43 at 2k, 325 and 323 at 20k): correct by the definition (the other side of the counter) but not a visitor count; read by trade, not pooled.
- **The arrest fall is also a report fall** (20k reported 0.76 -> 0.21 by band): the witnesses' walk to the Precinct is part of the mechanism, not the escort alone. Printed in § 5.1's reading; the decision is phase 7's.
- The change tier's 11 s breach (OPEN-093) stands; this phase adds nothing to it.
