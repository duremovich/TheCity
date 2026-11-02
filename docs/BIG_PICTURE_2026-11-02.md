# Big picture, 2026-11-02: step back before M18

The first product-level review since `history/BIG_PICTURE_2026-10-07.md`. Four read-only reviewers (Sonnet), one question
each; their full reports are summarised here, the evidence is theirs, the recommendation is the orchestrator's. Dylan
took every recommendation on 2026-11-02, with decision 4 amended (the "Decisions" table at the end).

## The short version

**The direction is right at the top and drifting underneath.** In a month the project built M15-M17, Virt, chrome, the
vertical city, three LOD tiers at 100k and ten milestones that were never on the roadmap, and a watchable 3D city. The
one thing VISION is for, a person you play in that city, is **0 lines of code** (no `PlayerControlled`, no directed
goal) and has been deferred four times (addenda 16, 20, 21, 22), each time for a locally good reason. Meanwhile the
shadow reads have said "the work and the gangs read thin" since V10, five fix passes ago. Three of the four reviewers
independently land on the same call: **stop the whole-city shadow-and-fix loop and build a thin playable slice next;
let a human in the city generate the backlog.**

## 1. Why work and gangs read thin (review 1)

Not the brain, not the diary. **A shift is one verb with nobody else in the room.**

- **One verb.** `ActionKind::work_for` (`citysim/src/goap/actions.rs:320-348`) maps Ripperdoc, Mechanic, Researcher,
  Reporter, the leisure staff, Fixer, Volunteer and every `Role::Trade(_)` to `ClerkWork`, completed identically
  (`exec/actions.rs:1049`). `office_worker_24590 d22`: ten `GoTo(Station)` + `ClerkWork`, one Break, zero interactions,
  one `[off screen]` Meeting memory in ten hours. The shift's stations are choreography, not content.
- **No counterpart.** The pick's coworkers are Statistical and "bodies never see a Statistical agent in their building"
  (`lod.rs:1682`). At 100k the diary subject is one of ~170 Full bodies; it works in an empty room. Raids land on empty
  Hideouts (`OPEN-278`), Extort finds nobody home, Court fails at Flirt: real mechanisms starved of a second person.
- **A floor of nothing.** `Idle` is a constant 0.05 (`utility/goals.rs:993`) and becomes `Rest` for 4-12 h a day in all
  six diaries read. Belonging and intimacy sit at 0.99 from passive drift, so no social goal ever scores: the office
  worker has nine +1.00 Friends and meets none in a week.
- **Gang life is a second job on top of the first.** The day job scores 0.9, GangWork 0.13-0.44; a member does one
  gang act a day and hears the gang as `[event]` lines.

Every fix pass since V10 fixed a *neighbour* of these (lights, stations, cells, raids, climbs), which is why each
verdict reads "better as a city, thin as a life". Another weights pass will not move it.

## 2. The shortest path to playable (review 2)

**The slice: a nobody in the Sump, 14 days**, on a pre-run day-21 save, **at 20k** (`isle_m_k2`, 160-280 tps; 100k for
reads, the doctrine's own rule for "too slow to iterate"). Every verb already exists as an NPC action:
`plan::plan_for` (`systems/plan.rs:204`), `contracts::visible`/`accept` (`contracts.rs:876`, `:1477`),
`moves::resolve` (`moves.rs:400`), `social::propose` (`social.rs:620`), `founding::register` (`founding.rs:773`).
The player is the camera subject, so the V5 ring makes them and everyone near them Full for free, which also fills
review 1's empty room around the player.

What is missing: a command channel *into* cityview's sim thread (`simthread.rs` only publishes), a possessed agent whose
`think` is skipped unless autopilot is on, picking, any UI at all (egui via `bevy_egui`), and feedback that an action
mattered (a live log, "who knows about you", a day-end summary). The last is the hard part.

| phase | what |
|---|---|
| P1 the seam | command channel; `PlayerControlled { autopilot }`; `think` skipped for it (the riskiest assumption: check first); egui HUD (clock, needs, coins, job, plan); click-to-go |
| P2 verbs | pick buildings and bodies; a menu of actions whose preconditions hold there; per-verb will-waivers (a human may steal where the NPC's greed gate refuses) |
| P3 board and talk | the contract board live; Talk wired to `moves::resolve` and `social::propose` |
| P4 consequences | live diary log, memories and gossip about you surfaced, hunts and hits when they form, day-end summary, death screen from the Life |
| P5 start and loop | "a new nobody" (pick or immigrant), menu, permadeath, save/load; exit: **Dylan plays 30 minutes cold** |
| P6 (only if P1-P5 read) | founding a business or Hideout |

M18_PLAYER.md (494 lines, written at 2k against macroquad) is mostly stale: §6 camera and input, the Mayor/Both modes
and difficulty menus, the byte-identical and replay gates (contradict the 2026-10-26 doctrine), the dialogue writer, missions
in person and the majordomo. Replace it with ~20 lines per phase. `OPEN-150` (no day-0 start) is met for the slice by
the pre-run save.

What the slice tests: whether the board has takeable jobs near a poor player, whether a nobody survives the Sump (the
city's harshness was tuned for NPCs who don't mind dying), whether anything ever **happens to** the player (if
nothing approaches them in 14 days, that is the #1 sim gap, with a target), and whether menu verbs plus a log make
dialogue unnecessary for v1.

## 3. Scope (review 3)

- **v1 as built, stop adding:** M15-M17, Virt, chrome, the economy, the vertical city, the LOD tiers, the survival
  loop, the news. **v1, missing:** the player, the combat model (`OPEN-179`: a player's central verb is a one-roll
  fight today). **After v1:** destruction, power/water, room templates, real traffic (`OPEN-275`), the lighting pass
  (`OPEN-284`), chrome on credit (`OPEN-286`), generated dialogue, attention. **Park/cut:** daemons, the squad and
  majordomo, cataclysms, combat maps, "the dystopia as a contest".
- **Superseded text still live:** "isometric" in VISION (the view section and two others), the roadmap's character-view row, M18_PLAYER;
  M18's 2k/macroquad premise; the roadmap tables omit everything built since M15; addendum 16 ("no review until M18")
  and 17-19 (the dole) overtaken by 20-22 and CLAUDE.md; the fate of `citysim-app` (macroquad) unstated.
- **OPEN.md (168 open/owned):** ~30 are v1 for a player (the slice's own list), 25-40 are stale or moot (V4-era rows,
  2k-only rows, determinism/mirror rows; *ids from truncated output: verify each before closing*), a block is
  calibration the doctrine already freezes, and many rows bundle 4-8 problems under one id so they never close. No
  priority column.

## 4. Code health (review 4)

178k lines in citysim/src, growing 4.7:1 adds to deletes. What will actually slow the next 100k lines:

1. **config.rs (14.5k lines) + config.toml (6k, 2,584 keys)**: the #1 and #2 churn files, the merge-conflict surface
   for parallel phases. No dead keys. Split by domain (2 Sonnet phases).
2. **`World` (4.8k lines, ~60 pub fields) and lateral reach**: `contracts.rs` calls `law` 46x and `hunt` 28x, `goals.rs`
   calls `law` 22x. Fence with thin APIs, not an ECS (3 phases).
3. **God-functions**: `considerations_with` 886 lines, exec `start_step` 349 and `step_agent` 282, shadow `observe`
   1,097. These drive agents' context and turns (V7 387 turns, V8 449). Split per goal/step (2 Opus phases).
4. **The 00:11 hitch is architectural**: global midnight passes, growing with the city's age (2-4.2 s). A due-list
   scheduler flattens it (2-3 phases). Matters at 100k in the view; not at 20k.
5. **The change tier is about to break its budget**: `core_sanity` is 152 s of a 160 s tests step and grew 24 s in
   three days. Split it; `SAVE_VERSION` u8 -> u16 (126 of 255); delete the 953 `serde(default)` shims kept for saves
   nobody loads (1 Sonnet phase, mostly deletion).

Not worth touching: `view/vendor`, the CLI tools, `Vec<Option<T>>` storage, the 117 flat systems files, the docs pile.

## Recommendation

1. **Next milestone: the player slice (P1-P5), at 20k.** Its exit is Dylan playing, not a shadow critic. Whole-city
   shadow-and-fix rounds stop; the slice's play sessions write the OPEN rows.
2. **Work beats ride inside it**, not before it. Once P2 lets a player take a job, the player hits `ClerkWork` x10
   on day one. Insert review 1's option A (a config deck of named beats at station-block boundaries, a coworker as
   counterpart, effects through `social`, memories, gossip) as a phase between P2 and P3, and the gang's own day
   (option B) only if P3-P4 show gang life matters to the player. Review 1's cheap diagnostic (pin coworkers Full) is
   what the ring already does for the player, so skip it.
3. **One hygiene phase first (Sonnet, cheap):** split `core_sanity`, `SAVE_VERSION` to u16, drop the old-save shims.
   It protects the only per-merge gate. The config split, World fence and god-function splits wait: the slice's phases are
   sequential, so the conflict surface does not bite; revisit if a slice coder blows 300 turns in `goals.rs`/`exec`.
   The due-list scheduler waits until the slice runs at 100k.
4. **One docs phase (Sonnet):** M18_PLAYER.md rewritten as the slice (~20 lines a phase); "isometric" and addendum 16
   struck; the roadmap table brought up to date; OPEN.md gets a `v1` tag on the slice's rows and the stale rows closed
   after a per-row check.
5. **Frozen until the slice reads:** the leftovers B and D (`OPEN-278`, `285`, `287`), `OPEN-265`, traffic, lighting,
   chrome credit, calibration. Coder A's three bugs (`OPEN-280`, `281`, `282`) finish and merge: they are needed
   regardless, and `282` makes the child rule checkable.

Rough cost: hygiene + docs ~$15, slice ~6 phases at the V-phase record (~$30-40 each) ≈ $200-250. Cheaper than the view
milestone, and it ends with something to play.

## Decisions (Dylan)

| # | question | recommended |
|---|---|---|
| 1 | The player slice next, ahead of more sim passes | yes |
| 2 | The slice runs at 20k; 100k stays the shipped city, read at closes | yes |
| 3 | M18_PLAYER.md replaced by ~20 lines per phase; its gates and menus dropped | yes |
| 4 | Whole-city shadow-and-fix rounds stop; play sessions drive the backlog | **yes, amended (Dylan):** shadows stay as the check that NPCs live full lives, run at major milestone closes to find weirdness (findings become OPEN rows, not the next round); a phase that changes NPC behaviour still reads its archetypes |
| 5 | The combat model (`OPEN-179`) in the slice or after it | after P4, if fights read as a dice roll |
| 6 | `citysim-app` (the macroquad debug app) retired once P1 lands | yes |
| 7 | The hygiene and docs phases before P1 | yes |
