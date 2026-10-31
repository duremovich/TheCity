# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-10-30, the twenty-eighth session: pressure pass round 8 from Shadow V16 (six phases X1-X6 in two waves,
planned and built in one session), Shadow V17 read against V16, the 100k counters.**

## Main

Main holds **pressure pass round 8** (`docs/PRESSURE_PASS.md` § "Round 8 from Shadow V16", every phase's entry under
"Implemented: deviations"). The change tier is green over the whole round (3m12s on the X5 merge; one line in
`events::harm_line` added after it, built, not re-tiered); tests still 1,470 (none added; probes whose premises moved
re-pinned in place with "Round 8" reasons, one by the orchestrator at the wave-1 merge). `SAVE_VERSION` 113.

| phase | what it added |
|---|---|
| X1 the promise to tell the law (`law::keep_word`, `word_plan`, `cell_case`, `held_apart`, `Corpse.defended`, `[crime.case]` X1 keys) | a promise is a goal walked to the nearest open desk within a day or dropped with a reason line; the held do not witness the street or get hit across the cell wall; no asleep blackmail; a crime in the cells is a case; a death resisting arrest names no killer |
| X2 the target and the bereaved (`systems/grief.rs`, `[grief]`, `contracts::react`, `stalled_runs`, `[search] need_*`) | a death in the house mourned 2-5 days (leave, a funeral, the children's line, a hot hunt); a hit's target hears and takes a guard, a counter-grudge or lies low; a stalled hit let go with a line; the needy sacked search; shadow picks `bereaved`, `marked` |
| X3 the wrong person in the role II (`careers::record_ok`, `Misery.record_at`, `reads_record`, `[misery] employed_steal_misery`) | the law's posts and the books read a record; the employed steal a vehicle only broke or miserable; no Fighter foreman |
| X4 gangs (`turf::rouse`, `raid::pick_muster`, `War.last`, `[gangs] leader_keep_margin`, `Deed::TookOver`) | the lie-low names the right enemy; every leader change a Succession line; the muster where most members reach; the call wakes a sleeper off shift |
| X5 the pit, the ages, marriages (`[leisure] bout_beating`, `Flow::Purse`, `social::suitor`, `World.courting`, `[social] court_*`) | a loser hurt only on a beating, the purse 14c from the pit's owner; bout, arrest, witness and cell lines print ages; no kin or minor courts, a marriage needs a courtship |
| X6 the cell's day, the walk-in, the picks (`law::serve_meals`, `consequence` yard, `household` cell visits, `care::walk_in_clinic`, `shadow.rs` doer ranks) | the meal, the yard, kin visits, the release line; walk-ins to the least busy open Clinic; the pickers rank doers, an empty pick is an error |

**Shadow V17** (`docs/SHADOW_V17.md`, `docs/shadow_v17/`, V16's window): better than V16 where round 8 aimed (the
law keeps its word, the cells have a day, the bereaved mourn, the marked react, marriages flat by week, Succession
lines), corp and the desk unmoved, and **the city hungrier**: starvation 13 -> 45 at 100k, the diaries put it in
Eat losing to Earn, a Pitch or Idle within the half hour (`OPEN-249`). **The under-18 rule held** in 117 diaries;
`harm_blocked` 0 over 30 days.

FINDINGs (100k seed 42 x 30, `target/r8_100k/` against `target/r7_100k/`): starvation 13 -> 45, Starving 7.3k ->
11.4k, ChildTaken 3,703 -> 4,229, school days 15.2 -> 12.3 %, births 274 -> 158 (fewer marriages); Marriage 23,267 ->
8,494 (flat by week); promises 282, kept 197, dropped with a line 82; Arrests 11.2k -> 13.9k; losers hurt 1,724 ->
509; Clinic-days with no patient 853 -> 560; vehicle thefts 2,354 -> 729; JobSearch 84k -> 223k; `sold_dealer` 8 ->
0; ticks/s 18.

## Decided

- **The pressure pass ends at round 9** (Dylan 2026-10-30, roadmap addendum 22): round 9 is broken systems only
  (Y1 hunger in the brain, Y2 contradictions that are state, Y3 one Sonnet narration batch;
  `docs/PRESSURE_PASS.md` § Round 9), and Shadow V18 is the exit read: VISION's five stories quoted from the
  diaries, nobody starving while Eat loses to a non-survival goal, no frozen loop, no minor harmed.
- **Then the map re-import with the vertical city, then graphics** (Dylan 2026-10-30: "Lets move towards
  integrating graphics"; "vertical city is important, so might as well bring it in"): one milestone, planned
  against TheCityGraphics; the view milestone after it, then M18.
- **The neglect ladder's size at 100k stays as built** (Dylan 2026-10-30, `OPEN-231`). Round 8 did not touch it;
  ChildTaken rose with the hunger, the ladder's input.

## In flight

- No agents are running; no worktrees.
- `target/shadow_v17/`, `target/r8_100k/` hold the runs.

## Next, in order

1. **Pressure pass round 9** (`docs/PRESSURE_PASS.md` § Round 9, planned 2026-10-30): Y1 hunger in the brain
   (`OPEN-249`), Y2 contradictions that are state (`OPEN-250`, `OPEN-251`), Y3 the narration batch (Sonnet).
2. **Shadow V18, the pressure pass's exit** (the five stories; one targeted phase per missing story, then close).
3. **The city milestone: the 100k map re-import with the vertical city** (`OPEN-204`, `OPEN-182`, `OPEN-221`,
   `OPEN-240`, `OPEN-241`'s far musters, `OPEN-187`, `OPEN-191`), planned against TheCityGraphics.
4. The view milestone (the Bevy client joins the workspace), then M18.

Carried, still open: `OPEN-217`, `OPEN-220`, `OPEN-212`, `OPEN-150`, `OPEN-179`, `OPEN-229`, `OPEN-231`; standing
`OPEN-062`, `OPEN-039`, `OPEN-057`, `OPEN-059`, `OPEN-077`, `OPEN-069`, `OPEN-070`. Closed this round: `OPEN-246`
(X1), `OPEN-247` (X2, X3), `OPEN-248` (X5). Opened: `OPEN-249` to `OPEN-251`.

## Process notes from this round

- **Merging wave 1 as one set, one tier** worked: the four branches merged in arrival order (each conflict was
  `SAVE_VERSION`, the appended `Jobs2Cols` columns and the docs), then one change tier over the set instead of one per
  merge. It caught one red (a seeded premise), cleared without a fix round.
- **Re-pin a butterflied probe at the killer, not the latest round**: the farm worker probe had been re-pinned three
  times by turning off whichever section moved the city; this time it turns off the organ gangs that kill its pick.
- **Ages on lines are matched by text** (`events::harm_line`): a phase that renames a harm line ("in the cells of" ->
  "in the yard of") must update the matcher; caught at the X5 merge.
- Coders ran 3-5 min tiers on a box shared by four agents; X1 and X4 breached the budget under load only.
- The shadow and the 30-day counters ran at once (39 and 41 min); seven Sonnet critics read in ~15 min.

## Cost (API-rate $ equivalent, `tools/session_cost.py --session e89ef89d`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| X1 the promise to tell the law | Opus | 444 | 241k | 59 |
| X2 the target and the bereaved | Opus | 389 | 280k | **76** |
| X3 the wrong person in the role II | Opus | 228 | 155k | 16 |
| X4 gangs | Opus | 314 | 234k | 32 |
| X5 the pit, the ages, marriages | Opus | 335 | 198k | 32 |
| X6 the cell's day, the walk-in, the picks | Opus | 233 | 236k | 21 |
| Shadow V17 critics (7) | Sonnet | 322 | 77-105k | 9 |
| orchestrator (plan, briefs, merges, runs, synthesis) | Opus | ~200 | 160k | ~13 |

Round 8 ≈ $258 for six phases and a shadow read (round 7: ≈ $232, round 6: ≈ $316). X1 and X2 blew the 300-turn
brief (444, 389): both built a new system end to end (the promise's desk walk, grief).

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers in `docs/TESTING.md`.
