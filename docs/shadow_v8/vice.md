# Shadow V8 critique: vice (club_staff, bouncer, dancer, madam, companion, joytoy, fighter, cook)

**Verdict: distance mostly landed (long legs and all-day-walk days fell by half or more outside s44), but the I1 "stay in" rule turned free time into bed-and-Rest, the 21:00 batch got worse, and two V7 defects survive (the bouncer, the rich pick who stops working).**

Method: 70 vice diaries in V8 (s42d0 12, s43 16, s44 16, gen 16, k2 8, nopin 2) against 70 in V7. Scripts over the `.md` and `camera.tsv` (hours, GoTo legs, event counts, the Time-use and Leisure tables), then full days read: companion_7618 d91-d93, madam_12321 d92-d93, club_staff_8280 d94 (all s44); joytoy_13649 d31, cook_15282 d32 (s43); fighter_13680 d93 (s44); joytoy_15520 d30-d32 (gen). Cites are `window/file dDD HH:MM`; windows `s42d0 s43 s44 gen k2`. Picks differ from V7's, so archetypes are compared in aggregate; same-person reads are gen/madam_23940 and s43/fighter_8413. No quoted line is from a Coarse or Statistical stretch.

## 1. Per archetype

**Club staff: counter busier, life around it thin.** `served` lines 87 -> 212, all `[off] ... Statistical (off screen)`, and 174 of the 214 `served` lines in the group (81 %; V7 57 %) are on the single minute 21:00.
> `s44/club_staff_23710 d92 21:00` eight `served ... (a customer, at the Attendant's counter)` lines in one minute, all Statistical; d93-d96 are the same 12.6 h home / 6.0 work / 5.4 street row.
She has zero leisure in the window (Leisure: 0.0 h, fun 0.00 and mood 0.10 at d94 00:00) and a 147-min walk each way to a D6 job (`d94 15:03`, `d94 00:02`).

**Bouncer: still a statue, as planned.** 0 `door`, `refus` or `eject` hits in 9 bouncers (V7 0 of 9); the roll-up `refusal` column is 0 in every window. Shift is 8-12 blocks of 198-236 min (s43/bouncer_14739 max 220). A reason beyond patron supply: `leisure::door_seen` (systems/leisure.rs:295) takes the refusal path only when the venue is a Club, and 5 of the 9 pinned bouncers guard a Fight Pit or Gambling Den (s42d0/bouncer_22966, s43/bouncer_14739, s44/bouncer_21902, gen/bouncer_15602, k2/bouncer_2244), where no refusal can exist whoever arrives. Worth a line in OPEN-176.

**Dancer: the best-fixed trade.** Tips 49 -> 68 lines, `served` 53 -> 89, all 9 dancers tip (V7 7 of 9). s44 dancers' zero-work days 8 -> 2 and the two "employed on paper" dancers are gone. The 21:00 batch persists (35 of 98 Tip lines at 21:00, V7 18 of 72). s42d0/dancer_10165 walks 619 min to a Club d0-d5 (`d1 10:11`, `d4 10:11`) until `d6 00:00 Housed: moved nearer work`, the only completed move-nearer in the 70.

**Madam: the house is paid, the person can stop.** MadamCut shows in 9 of 9 diaries (s42d0/madam_22577 68 lines). In s44 the two madams have 7 zero-work days of 14 (V7 2): madam_12321 is home 24.0 / 19.1 / 24.0 h on d93 / d95 / d96, wealth 1.00, `GOAL Idle (Work 0.031)` through d92-d96, `d94 00:00 Quit ... (no need of the wage)`, plus 42 one-minute `Rest`s inside her own shift (`d92 21:09-23:51`). That is V7's dancer_9249 (bug 5) and bug 8 in a new person. Same person gen/madam_23940: 122-197-min legs (8) -> none; home hours up 4-10 -> 6-15; HangOuts 15 -> 9.

**Companion: alive, one bad loop.** Tips 15 -> 25 across 7 diaries; repeated tipper in one line 5 -> 8 across dancers and companions (gen/companion_6184 `d36 21:00` +12c). The worst loop in the group is s44/companion_7618 (3.1).

**Joytoy: 0 clients as planned, now 2 shut-ins of 7.** No client line (V7 0 of 7). s43/joytoy_13649 never goes out: 0 HangOut, fun 0.95 -> 0.00, mood 0.30 -> 0.11, 84 h Idle in the week (3.2). s44/joytoy_21087 walks 15.5 h a day on d92 and d94 with 0.1 h of work (home D1 Sump, Dollhouse D3). gen/joytoy_15520 (62 y) is laid off from Contract Host `d30 00:00`, hired Attendant by the foreman that minute, laid off again `d31 00:00` ("a shift past full staff"), then home 24.0 h on d32-d34 and d36 with wallet 3c, wealth 0.08, no Earn goal.

**Fighter: calm and empty.** `Bout:` 23:00 every second night, one fixed pair per Pit, `bets paid 0` on every line (s43/fighter_8413 vs Razor Adeyemi `d30`, `d32`, `d35`: V7's pair). The fighter is mid-`ClerkWork` at the bout (s44/fighter_13680 d93 22:33 87 min, bout d94 23:00). Leisure collapsed: fighter idle 4.1 -> 8.3 h/day, HangOuts 11.6 -> 4.1 a diary, fighter_8413 3 -> 0. The V7 499-min commute is gone; s44/fighter_13680 lives 5 min from the Pit and its day is sleep, Rest, one Enjoy (`d93 06:37`), work: a fine life, a dull one.

**Cook: the day cook is still a half-trade.** 5 of 9 pinned cooks hold the 09:00-18:00 shift (s42d0/cook_15953, cook_17328; s43/cook_15282; gen/cook_24811, cook_8109). Their work is 9-12 blocks of 180 min or more (s43/cook_15282 `d32 09:03 ClerkWork 213 min` then `12:38 322 min`, one tile) and 0-1 counter-served lines in 7 days (cook_15953 `17:15`, cook_24811 `15:45`, the other three none). Evening cooks fell too: group counter lines 53 -> 24 (s44/cook_10142 0, s43/cook_8193 4). This matches the day-cook test needing its premise set by hand: no customer reaches a day counter by default.

## 2. V7 items

| V7 item | V8 |
|---|---|
| Free-time legs 300-900 min, late at work | **Fixed on s43 and gen, not s44/s42d0.** Island legs of 200+ min 225 -> 165; s43 78 -> 24, gen 32 -> 5, but s44 65 -> 64 and s42d0 82 -> 77. Pick-days with 10 h+ on foot and no work (non-rest) 22 -> 12 (s43 11 -> 1, gen 9 -> 1, s44 1 -> 8). |
| Hotel 380 min away | Not observable: no vice pick sleeps at a hotel. |
| Foreman takes people off their trade | **Unchanged:** s43/companion_7129 Manager at Parts Fab `d34 00:00`, s43/cook_8193 Broker at Lab `d36 00:00`, s43/madam_23514 Manager at Ripperdoc `d32 00:00` (3 of 4 hires are off-trade; V7 6 of 70). |
| Bouncer a statue | **Same**, plus the Club-only door (above). |
| Dancer who never works | Fixed for dancers; reappears as madam_12321 and gen/joytoy_15520. |
| Customers never reach s44 | Reversed: s44 `served` 2 -> 87 (club_staff_23710 44, dancer_25394 16). |
| 21:00 venue counters (OPEN-170) | **Worse:** served at 21:00 57 % -> 81 %, Tip 25 % -> 36 %. |
| Stims label, triple tips, bout pair, bets paid 0 | Unchanged, not the pass's. |
| Body jumps | 10 `JUMP` lines in 70, all `4 tiles in 1 min, street -> street, exec Idle, GoTo(x)` (s44/club_staff_8280 `d92 15:03`): the lens' kerb allowance, not the sim. |

## 3. The worst moments

**1. A companion who walks 17.5 h a day and works 0.1 (s44/companion_7618 d91-d96).** Home Block#335 and job Spire Lounge#4624 are both D5 Spire. `d91 10:28 GoTo(Workplace) 438 min` goes `street D5 -> D4 Mid-Levels (11:48) -> D5 (16:22) -> lounge 17:46`; she works 80 + 87 + 39 min. `d92 00:02 GoTo(Home) 649 min`, on the street in D5 only at 21:02, a 2-min `Rest` at the lounge `22:28`, then `22:32 GOAL Sleep` and another `GoTo(Home) 649 min`. Camera d92 and d93: home 4.3 / 4.2, work 0.1, street 17.5 / 17.7; d95 and d96 repeat it.
**2. A joytoy given nothing to do (s43/joytoy_13649 d30-d36).** `GOAL Unwind` appears 0 times in 697 lines, not even as a runner-up (goals: Eat 13, Idle 13, Sleep 8, Work 6); `HangOuts: 0`. d31 02:39-15:37 is eight 90-min `Rest`s at home between a meal and the shift; fun 0.95 (d30) -> 0.11 (d33) -> 0.00 (d36). She lives in D3 Neon Quarter, where s43/joytoy_15045 logged 36 HangOuts. Same shape: s43/cook_15282 (fun 0.00 mood 0.01, d31), s44/joytoy_26025 (mood 0.01, d94).
**3. An attendant who arrives 2.5 h late every night (s44/club_staff_8280 d94).** Home D5 Spire, job D6 Vats: `00:02 GoTo(Home) 601 min` (in at 10:03), sleep 4 h, `14:21 GoTo(Workplace) 371 min`, in at 20:32 for an 18:00 shift, works 3.3 h. Same d93-d96.

## 4. Bugs, ranked

1. **Same-district trips of 5-11 h.** 56 of the island's 165 vice legs of 200+ min begin and end in one district (V7 109 of 225): s44/companion_7618 (12), s42d0/madam_22577 (9), s44/dancer_18902 (5), s42d0/dancer_21514 `d6 11:51 GoTo(Home) 319 min` D5 -> D5. The route runs through another district (companion_7618 `d91 11:48`), so a missing road link or a path that avoids the direct way, not a pick. Site: the island's pathing or the El link for walkers (not read). Fix: find why a D5 -> D5 walk goes via D4; check the path cost to the direct segment.
2. **Unwind does not fire, so fun sinks to 0.00.** 9 of 70 vice diaries have no HangOut and no fun hour (V7 0 of 70): joytoy_13649, cook_15282, joytoy_26025, club_staff_23710/8280, dancer_25394, madam_22577, bouncer_14739, club_staff_12144. Group HangOuts away from home 362 -> 178, `Rest` 1,358 h -> 2,454 h. `distance.rs` says the own Home stays as the fallback, and joytoy_13649 shows no Unwind plan at all, so the offer is gated before that fallback (the goal in `goap/`, `leisure::best_spot_kind`; not read). Fix: when no spot is in reach, still offer the own-Block HangOut or a street spot near home (OPEN-109's feature), so fun is not a sink.
3. **The move-nearer pressure does not resolve on the island.** One completed move in 70 (s42d0/dancer_10165 `d6 00:00`); s44/club_staff_8280 and s44/companion_7618 are at day 90 with commutes of 6-11 h. Check the I2 trigger for a day-90 worker on a walk that the pass thinks is short (see bug 1).
4. **A rich or laid-off trade person goes home and quits.** madam_12321 (wealth 1.00, Work 0.031, Quit `d94 00:00`) and gen/joytoy_15520 (3c, no Earn goal for five days). Fix: Work must outrank Idle inside the shift for an employed pick, and an unemployed poor pick needs an Earn goal.
5. **A madam who tries a clinic 4 h away every morning and never arrives.** s44/madam_21371 `GoTo(Clinic)` d91 208 min (ended), d92 225, d94 278, d95 244, d96 278 to Ripperdoc#4514 (D5); 22.4 h home on d93. Likely the same path defect as bug 1.
6. **The foreman places, the corp lays off.** gen/joytoy_15520 `d30 00:00` hire, `d31 00:00` laid off. And off-trade hires (section 2). OPEN-143, unchanged.
7. **Feature (OPEN-176):** the bouncer's door exists only at a Club. Pits and Dens need it too, or a bouncer there never meets a refusal.

## 5. FINDINGs (calibration)

- Group idle 3.5-5.1 -> 5.0-8.3 h/day (joytoy 3.7 -> 8.0, fighter 4.1 -> 8.3); fun hours 1.0-2.6 -> 0.9-1.7 h/day. Home hours are up almost everywhere (fighter s42d0 7.9 -> 15.4, s43 9.8 -> 14.1); work hours are flat (4.4 -> 4.5). The "stay in" rule shows as about 2 more hours at home and about 1 fewer at a spot.
- `served` 148 -> 214, but 81 % on one minute: the counters still run on a clock, not on arrivals.
- Cook counter lines 53 -> 24; day cooks 0-1 a week.
- HangOuts a diary: fighter 11.6 -> 4.1, madam 8.7 -> 4.6, cook 11.3 -> 5.2.
- Foreman: 3 of 4 island hires take the pick off the trade (V7 rate).
- `bets paid 0`, one pair per Pit (OPEN-122).
