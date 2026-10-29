# Shadow V15: pressure pass round 6 at 100k (2026-10-29)

Round 6's read, in V14's window so the two compare one for one: main `bf63dee` (`target/shadow_v15/sha.txt`), the
100k island, seed 42, days 21-27, two picks per archetype (V14's list; pimps now stand on day 21), plus a pimp run
from day 22 (`target/shadow_v15/isle100k_s42_d22_pimp/`). Seven Sonnet critics (`docs/plans/shadow_v15.md`) wrote
`docs/shadow_v15/<group>.md`. Runs in `target/shadow_v15/`; the round's 30-day 100k counters in
`target/r6_100k/analyze.txt` (round 5's in `target/r5_100k/`).

## Verdict

**Better than V14 where round 6 aimed, and one phase overshot.** The street has gangs on it (held corners, the
wall-spray, one real war with a muster and a corner beaten back), pimps run a stable again, the door turns people
away and the pit has recurring rivals, cabs are paid to the driver and land where paid, a death is found and opens a
case, no child is on the street (0 of ~130 `[family]` lines against 13). **V1 overshot**: splitting the shelf by
household made the ladder fire at 100k, and the service now takes children from families who could feed them
(ChildTaken 624 -> 4,103, a wave of 500 a day at d19-21). Much of V3, V5 and V6 shows in the counters but not in the
diaries: the war lives in the leader's diary only, no member hears it; no cry-out reached a sleeper; no night Clinic
patient was in a pick. **The under-18 rule held**: no minor in any harm, vice, corner-fight or war line in 114
diaries; `harm_blocked` 0 over the 30-day run (no pick sent a minor to the kill backstop); the 18 starvation deaths
are adults.

## The 30-day counters (100k seed 42 x 30, round 5 -> round 6; FINDINGs, nothing tuned)

| | r5 | r6 |
|---|---|---|
| ChildTaken / ch_camp daily mean / ch_street | 624 / 119 / 249 | **4,103 / 1,188** / 69 |
| ChildTaken a day, d15 / d20 / d29 | 0 / 35 / 66 | 73 / **545** / 124 |
| warnings standing past 14 days (daily mean) | - | 72 |
| fetched back / off the street / taken off the street / pocket to shelf | - | 140 / 90 / 392 / 4,988 |
| starvation deaths / Starving events | 7 / 6,403 | 18 / 7,416 |
| school days / child-days | 18.0 % | 15.2 % (114 Schools) |
| deaths by violence / Murder | 441 / 236 | 482 / 284 |
| bodies found (CorpseFound) / Reports / Arrests | - / 13.0k / 9.5k | 481 / 16.3k / 12.1k |
| thefts / jailed mean | 17.5k / 1,052 | 19.9k / 1,332 |
| Assault / Extortion / Raid / TerritoryFlipped | 4,848 / 2,516 / 155 / 181 | 5,753 / 1,116 / 216 / 82 |
| turf: cells held (last day) / gangs holding / contested | - | 377 / 101 / 41 |
| turf war: wars / fights / flips / retaken / raid moves / truces / crackdowns / war deaths / gang deaths | - | 173 / 207 / 137 / 10 / 57 / 16 / 14 / 10 / 114 |
| pimps daily mean / street visits | 0.2 / 150 | **1.1** / 466 |
| door refusals / drunk at the door / bouts | 4,461 / - / 472 | 7,113 / 165 / 1,677 |
| cab trips / fares to a driver / to another payee | 15.2k / - / - | 8.6k / 5,456 / 2,186 |
| patched / chrome installs / fake papers / licences | 11.6k / 3,947 / 12 / 158 | 14.9k / 5,796 / 21 / 152 |
| homeless max / Materialised | 13.3k / 612 | 14.3k / 530 |
| harm_blocked (a minor reached the kill backstop) | 0 | 0 |
| ticks/s mean (shared with two shadow runs: a print) | 24 | 16 |

## What reads right (quoted in the group reports)

- gangs: held corners and the spray-can; `gang_leader_68028` with 14 men at war on two fronts, calling them off the
  corners at d24 00:05; a gang-paid patch-up; the body found in a gang death.
- crime: two of four pimps run a stable and take a cut on a worker's pitch; `dodger_71194 d22 08:38` a homeless gang
  leader's death carries the whole V3 chain (the body found, the case, the contracts cancelled).
- vice: the door refuses the broke by name; `fighter_63572` has a three-bout rivalry with Casimir Wexford.
- corp: `office_worker_89863`, a 70-y foreman who is a Red Widows enforcer, extorts a Block at 04:00, fights the
  payer and gets 10 days; `ceo_61159` cabs to a Work Camp and buys her son out; the door-refusal loop nearly gone.
- labour: `day_labourer_11346 d22-26` passed over at the gate, hired, then a better hire; `builder_86331` a brawl,
  a report and "learned who put him inside".
- street: `vendor_9611` defrauded, reports the scammer, loses a fight to her, is jailed; turf lines ("kept off the
  blocks after dark").
- care: a teacher with 7-9 pupils (V14: 1-2); a sick man reaches a soup bowl.

## Round 7's input, ranked

1. **The household does not buy for its children** (`OPEN-243`, new; V1's own reads-wrong): a hungry worker's Eat
   buys for himself and the grocery run stands in only for a fed worker's Eat (`grocery_due`), so a family with coins
   lets its children go unfed until the warning, and the second spell takes them: 4,103 taken at 100k, a tenth of the
   children in Camps by day 20; `ceo_20314`'s household of 500c with a 17-y son "2 days unfed" (corp); a 0-y child
   on a 30-day clock (`black_doc_62012 d22 00:10`, crime). The same reads as hanging where the household cannot buy
   (`camp_parent_106052`'s 4-y on the warning since d18 all week, street). School days fell 18.0 -> 15.2 % with the
   Camps' headcount. The fix is the household's shopping (the parent buys the children's share, the warning moves
   the purse first), not the ladder.
2. **The war and the case reach only the counters** (`OPEN-241`, `OPEN-238`): the muster still walks off (`cultist_62792
   d21 18:32-d22 00:09`, 5.5 h walking, asleep at energy 0.03, no Brawl: V14's bug unchanged); members never hear
   their gang's war; a sleeper robbed four times in three days tells nobody (`courier_50890 d21-23`); a witness to a
   murder finds the body and the case reads "no suspect" with no line that he kept quiet (`gang_member_6731 d24
   02:54`); tribute lines read 0c (street).
3. **The guard cannot make the arrest** (`OPEN-244`, new): one arrest a guard a week, `Arrest unplannable:
   Unreachable` and "no guard on shift near; the warrant stays open" (`guard_27438`); `guard_8327 d27 14:12-14:23`
   beats 18 fare evaders in 11 minutes, some 61 and 67 (no cap, no fine); a bleeding 61-y calls for help three times,
   no Rescued (`camp_warden_8656 d22 13:55`).
4. **The wrong person in the role** (`OPEN-245`, new): the midnight hire names a forger (`forger_7536 d21 00:11`,
   who never works all week) and an orderly (`orderly_21547 d23 00:11`) as the foreman; the orderly is credited with
   36 chrome installs while the Ripperdoc post is vacant and paid nothing for them (`orderly_13875` the same).
5. **Far posts** (`OPEN-240`, `OPEN-221`): a builder sent 190-220 tiles to a different D1 shop each day from a yard 4
   tiles from home (`builder_86331`, the build-site picker); `cook_23744` 10-13 h travel a day (a far hire);
   `fabber_34558` 6.5 h. Part of the map re-import's work.
6. **Contradictions**: a father jailed 20 days for "Abduction" for breaking his own 14-y out of the Camp, the
   witness lines swapping actor and witness (`camp_parent_51134 d22 19:18`); a barred man back in the room the same
   night (`dancer_74279 d21 20:32`); a CEO shaken down asleep and robbed again, never reacting (`ceo_20314 d23`); the
   BountyPaid fold missed Dealing (five Witness and money lines a stand, `dealer_101293 d22 19:01`); the "fee beyond
   the purse" with 10c in it (`school_parent_48165 d22 09:30`, V14's, unchanged); a marriage while the groom is in a
   cell; a told job lead dropped (`jobless_6217 d24`).
7. **Dull**: the door's only reason is "broke" off screen (drink and coins are all the Statistical tier carries);
   bouts have no stakes for the fighter (no purse, no hurt, no loser's line); two of four pimps are gang men who run
   no round; the clinic with a doctor on shift and no patient (`ripperdoc_9880`, V14's, unchanged; V6's spill never
   binds in a pick); reporters, execs and the desks still thin; GOSSIP ABOUT ME lines 60 -> 259 in the vice diaries.
