# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-10-29, the twenty-sixth session (continued): pressure pass round 6 from Shadow V14 (six phases V1-V6 in two
waves, one fix round), gang turf (`docs/TURF.md`), Shadow V15 read against V14, the 100k counters.**

## Main

Main holds **pressure pass round 6** (`docs/PRESSURE_PASS.md` § "Round 6 from Shadow V14", every phase's entry under
"Implemented: deviations"). The change tier is green at `bf63dee` (2m54s); tests still 1,470 (none added; fourteen
probes whose premises the phases moved were re-pinned in place with "Round 6" reasons, one more by the fix round
`bc0d0ff`). `SAVE_VERSION` 102.

| phase | what it added |
|---|---|
| V1 the ladder at 100k (`systems/shelf.rs`, `Household.pantry_stake`, `kin`) | a Block's shelf split into the households' stakes (the hang was one shelf feeding ten families); one warning per household; a parent's pocket feeds his child; a slipped child fetched back; a street child to a free parent with a Home or taken |
| V2 turf you hold (`systems/turf.rs`, `[turf]`, `Shock::LostTurf`, `EventKind::Turf`) | cells held by presence, the corner and holding it, holder changes told, `rival_of` the neighbour, control from cells; tribute, protection and dealing by cell, the pimp named where the members live (replaces U6's trip bound), recruits from the turf, after-dark avoidance, families out of contested cells; the under-18 gate on `gang::contested` and the squat eviction |
| V3 violence with consequence (`demography::find_bodies`, `law::hue_and_cry`, `tell_the_beat`, `[crime.case]`) | a body found opens a case (the killer named on a lead); the till the nearest stall not emptied in 24 h; the victim cries out to a guard or tells one on the beat; the downed lie still and sleep, no wage; a busted Fixer closes before the night's posts |
| V4 the night out and the trip (`leisure::steps_for`, `Flow::CabTake`, `[life] commute_long_ticks`) | gamble before drink at a kept door, the bar read "through"; a retiree's move to kin skips a spouse's long commute; the rider pays the driver, the commute cab reads the wage, a ride under way holds |
| V5 the turf war (`[turf.war]`) | provocation heat seen by the holders, the leader's answer musters to the corner, hold or flip, the retake, a Hideout raid moves cells; exhaustion, a truce between spent leaders, the law's crackdown; the off-screen roll |
| V6 empty rooms (`[care] orderly_opens`, `clinic_spill`, `on_call_spread`, `[customers] door_broke`, `door_stat_drink`, `[leisure] bout_card_per`) | a Clinic open at night for a patch-up, patients to a free doctor, the back room's doc spread; the door turns away the broke and the off-screen drunk; the pit's bout book and the undercard |

**Shadow V15** (`docs/SHADOW_V15.md`, `docs/shadow_v15/`, V14's window): better than V14 where round 6 aimed (gangs
on the street, pimps with a stable, the door and the pit, cabs, a body found, no child on the street), and **V1
overshot**: the ladder now fires at 100k and takes children from families who could feed them. Much of V3/V5/V6 shows
in the counters, not the diaries (the war is the leader's only). The under-18 rule held in all 114 diaries;
`harm_blocked` 0 over 30 days.

FINDINGs (100k seed 42 x 30, `target/r6_100k/` against `target/r5_100k/`): **ChildTaken 624 -> 4,103** (500 a day at
d19-21), ch_camp mean 119 -> 1,188, ch_street 249 -> 69; starvation 7 -> 18 (adults), school days 18.0 -> 15.2 %;
violence deaths 441 -> 482, Murder 236 -> 284, CorpseFound 481, Arrests 9.5k -> 12.1k, jailed mean 1,052 -> 1,332;
Extortion 2,516 -> 1,116, Raid 155 -> 216; turf: 377 cells held by 101 gangs, 173 wars, 137 flips, 16 truces, 14
crackdowns; pimps 0.2 -> 1.1 a day, street visits 150 -> 466; door refusals 4,461 -> 7,113, bouts 472 -> 1,677;
fares to a driver 5,456 of 7,642; ticks/s 24 -> 16 (shared with two shadow runs: a print, re-read at the next close).

## In flight

- No agents are running; no worktrees.
- `target/shadow_v15/`, `target/r6_100k/` hold the runs.

## Next, in order

1. **Pressure pass round 7 from V15** (`docs/SHADOW_V15.md` § "Round 7's input", not yet planned): first the
   household buys for its children (`OPEN-243`: the fix is the shopping, not the ladder); the war and the case into
   the members' and victims' diaries (`OPEN-241`, `OPEN-238`: the muster still walks off); the guard's arrest
   (`OPEN-244`); the wrong person in the role at the midnight hire (`OPEN-245`); contradictions. Same loop: one Opus
   coder per phase, then Shadow V16 in this window.
2. The 100k map re-import (`OPEN-204`, `OPEN-182`; home-to-post walking, `OPEN-221`; the far posts in `OPEN-240`).
3. The vertical city (`OPEN-187`, `OPEN-191`), the view milestone, M18.

Carried, still open: `OPEN-220`, `OPEN-212`, `OPEN-150`, `OPEN-179`, `OPEN-229` (Unwind next); standing `OPEN-062`,
`OPEN-039`, `OPEN-057`, `OPEN-059`, `OPEN-077`, `OPEN-069`, `OPEN-070`. Closed this round: `OPEN-239` (V4),
`OPEN-242` (V1, read at 100k).

## Process notes from this round

- Each merge's change tier ran on main: green after V3 (main had not moved), after V2+V3, red after all four of wave
  1 (one probe: V1's shelf stakes the probe did not zero; a Sonnet fix round, $1), green after V5 and V6.
- Wave 2 spawned on the red sha with the known red named in the brief; both coders reported it as main's and nothing
  else. That saved the wait for the fix round.
- Four phases bumped `SAVE_VERSION` to 97; renumbered at each merge (97-102). Resolving `OPEN.md` row conflicts by
  keeping the longer of each row works when only one side noted a row.
- The one-100k-run rule held: no phase over $72 (round 5: two at $80). V2 and V4 still ran ~400-480 turns.
- The shadow and the 30-day counters ran at once on the box (~45 min); critics read in ~15 min.

## Cost (API-rate $ equivalent, `tools/session_cost.py --session 13cbd2db`, round 6 part)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| V1 the ladder and care | Opus | 407 | 231k | 63 |
| V2 turf you hold (one 100k x 30) | Opus | 394 | 296k | **72** |
| V3 violence with consequence (one 100k x 30) | Opus | 356 | 210k | 33 |
| V4 the night out and the trip | Opus | 479 | 229k | 64 |
| V5 the turf war | Opus | 242 | 260k | 23 |
| V6 empty rooms (one 100k shadow) | Opus | 340 | 232k | 31 |
| fix round (the grocery probe) | Sonnet | 22 | 59k | 1 |
| Shadow V15 critics (7) | Sonnet | 273 | 62-135k | 9 |
| the turf codemap (Explore) | Opus | 93 | 133k | 4 |
| orchestrator (turf spec, briefs, merges, runs, synthesis) | Opus | ~225 | 161k | ~16 |

Round 6 ≈ $316 for six phases and a shadow read (round 5: ≈ $369). The session as a whole (rounds 5 and 6) ≈ $687.

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers in `docs/TESTING.md`.
