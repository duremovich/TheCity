# Street critique (V2): homeless_1829 (Vale Hallow), worker_friday_2235 (Katrin Mbeki), and the homeless_825 / homeless_1887 stand-ins

Seed 42, start d19 (a Friday), main at a737870 (Life L2 merged). Code cites are file:function, read-only, checked unless marked "unverified". Diary lines are `dNN HH:MM` in `shadow_v2/pinned/` unless marked `nopin/`.

## Day in the life

### Vale Hallow, 20, homeless Dreg (a typical day, d20 to d24)
- She wakes in a derelict Block squat in Sump West (Block#333), usually after a 4 to 7 h sleep that began anywhere between 20:00 and 04:00.
- 09:00: the 4c dole drops where she stands.
- Morning or afternoon: a Wander, then up to three one-hour Scavenge actions on the street. Typical haul: 0 c. Over the week the scavenging found 1 c in about 19 hours.
- Midday: a one-hour HangOut at a fixed street corner 53 min from her bed. Eleven of fifteen HangOuts say "nobody there". The exception is Omar Duarte, met five times, Acquaintance to Friend by d25.
- Rest of the day: 30-minute Wander blocks (90 of them), a Sleep trip back to the squat, a Wander, a Sleep trip.
- Every third day: the food run. She has no market in Sump West, so she walks 5 h to a Mid West market, spends her whole purse on 3 food (12 c), eats one on the spot, and walks 4 h back to bed. Arrive at the squat 03:35.
- Week: sleep 43.6 h, travel 44.5 h, idle 44.5 h, leisure 15.2 h, scavenging 18.7 h, work 0, social 0. Net +5 c. Events: none.

### Katrin Mbeki, 20, Street-class croupier (Gambling Den, Civic; home in the Spire)
- Fri d19: sleeps 5 h, walks 131 min to a Spire street corner for a morning HangOut, wanders 3.5 h, starts commuting at 12:43 for an 18:00 shift, flaps between Work and Idle four times, reaches the Den at 16:44, works the 18:00 to 24:00 croupier shift. Hears and tells six rumours across the tables. Leaves 15 minutes early to sleep. The wage for the whole day is not paid.
- Sat d20 (rest day): walks 155 min to a Civic market for 12 c of food, then scavenges three hours at 3 c in her purse (1 c found), walks 137 min to the corner, hangs out an hour, walks home 129 min, sleeps 20:00 to 00:30.
- Sun d21 to Tue d23: sleep 4 to 5 h in two blocks, two to four hours of travel to hang out and to eat, 5 to 6 h of ClerkWork per night, paid at midnight (6, 5, 5 c).
- Wed d24: walks home 129 min to eat and finds the pantry empty. Walks to the market (155 min), buys 12 c of food, abandons it for Work (the 12 c is refunded), flips between Eat and Work for two more hours, reaches the Den at 17:04. At 21:15 she leaves the shift with 2h45 to run, walks 139 min for food, eats at 23:45. The day's wage is lost.
- Thu d25: the diary ends mid-shift.
- Week: sleep 38.6 h, work 32.0 h, travel 61.5 h, leisure 10.7 h, social 0, idle 14.0 h. Spent on leisure: 0 c. Net -11 c. Paid for 3 of 5 shifts worked.

### The off-screen homeless (nopin/homeless_1887, 168 h Statistical)
- All week at one tile, "street (Mid West)", doing nothing.
- A day is a dole of +4 c at about 09:27 and one meal of -4 c to Greenline at a drifting hour.
- Energy refills to 1.00 between 00:00 and 02:00 every night. Fun jumps +0.28 about every other evening with 0 c spent.
- No scavenging, no Hotel, no walking, no contacts (three Acquaintances in a week).

## Findings

### Homeless

**H1. The poor's food run is a 10-hour round trip every third day (distorts the day).**
- Evidence:
  - d19 16:59 `GoTo(Market)` 301 min, arrives 22:00, buys 12 c of food, `GoTo(Squat)` 249 min.
  - d22 17:59 `GoTo(Market)` 301 min, arrives 23:00, back 251 min.
  - d25 20:29 `GoTo(Market)` 211 min.
  - Travel is 44.5 h of 168 (26 %), and two evenings (d19, d22) end on a 4-hour walk to bed that arrives at 03:35.
- Cause: Sump West has no Market (the diary names only Street Market#412, Mid West). The Eat plan walks to the market, because `GoTo(Market)` has no nearer option. Nothing ties the squat choice (`street::squat_reach`, `hotel_reach_homeless`) to the food she eats. After eating she walks back to the same squat instead of the nearest bed (Hotel, rough sleep).
- Fix: either put a cheap stall or a soup kitchen in every Sump district (the dole is a meal, so a Sump meal point should exist), or make the squat and Hotel pick weigh the distance to a Market. Do not let a tired, fed agent walk 4 h to a bed when `rough_ok` or a Hotel is nearer.
- Reads as real in spirit: a food desert is exactly the kind of pressure Dylan wants. It is the size (a quarter of her life) that reads as a travel bug, not as hardship.

**H2. Income: 1 c from scavenging. The "38 vs 116" is mostly a method difference, plus the L1b lever (calibration, not a bug).**
- Evidence:
  - Vale: `Sanitation +1` once (d24 08:48) in about 19 scavenge hours.
  - homeless_1887: 6 finds in 51 scavenge actions (12 %).
  - The `[life] scavenge_p` is 0.15 a try and `jobs::scavenge_p` floors the litter scaling at that. 0.15 c an hour is at most 1.2 c a day for an 8-hour grind, against 4 c of dole and 4 c of meal.
- Why 38 vs 116:
  - The V2 "earned" column sums flow kinds. The dole is not a flow kind (Vale's flow table lists only Food and Sanitation), so the 38 is non-dole income (Vale 1 c, 1887 6 c, 825 the rest: a 5 c wage and a 12 c fence sale).
  - The L1 figure 84 (life off) equals 3 agents x 7 days x 4 c exactly, so L1 almost certainly counted the dole (unverified: the L1 method is not written down).
  - Add 3 x 7 x 4 = 84 c of dole (less 825's jailed days) to 38 and V2 lands near 110 against 116, so the gap is mostly the method.   - What did change is the L1b lever (`scavenge_p` 0.5 to 0.15): scavenging fell from about 4 c a day at the cap to under 1.
- Tool bug: the diary labels a coin by the action running when it lands. At 09:00 on d24 the dole landed during a Scavenge and was written "+4c (scavenging)". Vale's "scavenging +5 c over 2 flows" is 1 c plus one mislabeled dole. In pinned 1887, "scavenging +18 c" is 6 c plus three doles.
- Fix: label by flow kind from the ledger, and add a Dole line to the flow table so earned columns can be compared with L1.

**H3. The ladder is not climbable on the dole, and nothing shows the attempt (distorts; by design, but unreadable).**
- Evidence:
  - Vale already holds the squat rung. Her purse peaks at 13 c (d25), is 0 c after each food run, and never has the 21 c deposit (`rent.rehouse_coins_mult` 21 x `base[0]`) plus a night's 3 c that `leisure::spendable` keeps back.
  - Net income is about +0.5 c a day (dole 4 + scavenge 0.4 - food 3.4). A lease is 40 days away at that rate, with no goal that saves.
  - homeless_1887 shows the Hotel rung works: three nights (`d20 22:40` `GoTo(Hotel)` 121 min, `CheckIn`, -3 c) for its last coins.
- Cause: the climb is arithmetic only. Dole = meal, so there is no surplus, and the only surplus source (scavenging at 0.15 an hour) is under 1 c a day.
- Fix:
  - Log "rung rejected: needs N c, has M" so the reader can see why no one climbs.
  - Give a jobless Dreg a visible job route. Sweepers earn 6 to 7 c a day in these diaries, and no hire event appears for a jobless 20-year-old in the week (unverified why: hiring and vacancies were not checked).
  - Optionally let a long-run saver goal hold back a Hotel night.

**H4. Beg is unreachable from the squat (distorts the day).**
- Evidence: zero Beg actions in all three homeless diaries.
- Cause: `goap/actions.rs` allows Beg only `at(Market)` or at a Bar (line 1208). The squat-dweller is never near either.
- Fix: allow Beg at a HangOut spot with 1 or more others present. That is the street, and it gives the corner a second purpose.

**H5. The empty corner (distorts the day).**
- Evidence: 11 of Vale's 15 HangOuts and 4 of 1887's 13 say "nobody there". The spot is a fixed point 53 min from her bed. She walks 53 min, stands an hour alone, and walks away.
- Cause: `leisure::choice` scores the free spot at `hangout_hour x hours + 0.1 x social`, so a lone hour on a corner still earns the full fun gain.
- Fix: scale the free rung's gain by company (alone half, a known contact full plus belonging), and pick a nearer spot when the best one is far.
- Credit: when someone is there the corner works (Omar Duarte, five meetings in four days, affinity +0.12 to +0.42, Friend). That is the first street friendship the project has produced.

**H6. Idle is still a quarter of her life, and the night gate is weak (distorts the day).**
- Evidence:
  - 44.5 h of 30-minute Wander blocks. d21 and d25 are 9.6 and 10.7 h idle.
  - d22 01:59: Sleep for 30 minutes, then Unwind wins at 02:29 and she walks 55 min to the empty corner and 53 min back (the Sleep goal at 01:06 had U(energy) 0.03).
  - Sleep is 43.6 h in many fragments, several in the daytime.
- Cause: `Idle` is a flat 0.05 and `Wander` is its only plan. `Unwind` scores "off hours 1.00" at night with no sleep brake once energy has recovered a little.
- Fix: a personality-weighted idle menu (watch, scavenge elsewhere, visit a known contact, busk), and a night brake on Unwind for anyone with a bed within reach.

**H7. The street never touches her (distorts the tone).**
- Evidence:
  - Vale: "Notable events: none" for 7 days alone, on a street at night, as a 20-year-old.
  - homeless_1887: two Witness events, nothing else.
  - No theft, no assault, no sweep, no kindness, no hostile edge.
- Cause: the street pressure is only arithmetic (the food walk, the 0.15 scavenge). There is no social or violent exposure on the squat or the street for a Dreg at Full LOD, and the sweepers, who clean streets, do not move squatters.
- Fix: a small per-night exposure on the street rung: a mugging roll scaled by what she carries, a gang shakedown on a corner, a sweep that moves a squatter. Dylan's brutality-via-pressure rule says restore the tone with deliberate levers.

### Friday worker

**W1. A whole day's wage is forfeited by leaving the shift early (distorts the day; money).**
- Evidence:
  - d19 23:45: `plan interrupted: Work -> Sleep` (Sleep 0.498 against Work 0.455), 15 minutes before the end of 18:00 to 24:00. No wage arrives at d20 00:00.
  - d24 21:15: Eat 0.639 against Work 0.601, she walks 139 min for food, 2h45 before the end. No wage at d25 00:00.
  - Two of five attempted shifts, 11 c of 27, gone, on a wage of 5 to 6.
- Cause: `exec::actions::end_shift` runs only when the shift completes. It sets `last_shift_day` and owes a day. There is no pro-rata and no commitment against Sleep or Eat.
- Fix: a shift lock (Work cannot be outbid by Sleep or Eat in the last N minutes, or in a whole shift unless critical), and pro-rata pay above 50 % worked.

**W2. The commute gate flaps, and she buys food and gives it back (distorts the day).**
- Evidence:
  - d19 12:43 to 15:17: four `Work -> Idle -> Work` cycles on the way to an 18:00 shift. A Wander sits between each GoTo(Workplace).
  - d24 13:35 BuyFood -12 c, 13:45 `Eat -> Work`, `+12c (food purchase) ... (refund)`. Then 14:45 Work to Eat, 15:15 Eat to Work. She is fed at 23:45, 15 hours later.
- Cause: `routine::must_leave_for_work` is `until <= travel_estimate + DEPARTURE_MARGIN`. As she walks the estimate shrinks, the gate turns off, a 30-minute Wander follows, and the gate turns on again. The refund is the correct accounting of the abandoned purchase, so no coins leak. The cost is the 12 hours of wasted day.
- Fix: latch the commute once begun (until she reaches the workplace), and do not start a purchase when a latched commute is within one think of its gate.

**W3. Travel is 61.5 h a week and the free rung costs 5 hours of walking per hour of fun (distorts the day).**
- Evidence: the HangOut corner is 131 min from home (d19 05:45, d23 06:32) and 137 min from the Den (d22 14:45). After the midnight shift she walks 179 min to it (d22, d23 00:02), hangs out an hour at 03:00, and walks 68 to 131 min home. The Spire home to the Civic market is 155 min. Leisure 10.7 h costs about 40 h of walking.
- Cause: the cross-cutting cause 1 of SHADOW_V1 survives L1 for non-commute trips. The spot is the contacts-weighted best (`best_spot`, `hunt::habit`), not the nearest, and nothing penalises a three-hour approach beyond the `travel` factor.
- Fix: cap the free spot's walk at about 45 minutes (fallback: the nearest spot), and weigh Market and spot choice by walking time.

**W4. No Friday night, and no spending at all (distorts the day; partly by design).**
- Evidence:
  - Friday evening is the croupier shift itself, at the table of a leisure venue she never plays at.
  - Saturday evening (d20) she has 3 c after a 12 c food run and sleeps 20:00 to 00:30.
  - Leisure spend 0 c for the week while her purse held 16 to 17 c on d19 and d24 00:00.
  - On the d24 05:35 morning pick, with 17 c, the pick is still the free corner at score 0.08.
- Cause: `leisure::spendable` keeps back 2 meals (8 c), a week of rent share, owed rent, and `paid_candidates` needs `2 x price <= purse`. With 17 c that leaves about 5, below most rungs. The diary does not say which paid rungs were rejected (unverified which one blocked her on d24).
- Fix: log the rejected paid rungs, and give staff a discount or a free hour at the workplace they run. A Street worker on 6 c a day cannot afford a night out in this economy, which is the tone Dylan asked for, but then the diary should show the want (fun 0.6 to 0.9 on d24 and d25) failing at a price, not a free corner.

**W5. Spouse and Friends at affinity 1.00 are never seen (distorts the day).**
- Evidence: Bex Quarry (Spouse), Cipher Ashcombe and Beatrix Bellamy (Friends) all at +1.00; "Top contacts" lists ten people and none of them is one of the three. Social time 0.0 h. This is V1 finding 9, unfixed.
- Cause: Socialise and Unwind pick targets by co-presence at the corner, not by love. The Socialise goal is 0.06.
- Fix: a weekly "see someone I love" goal, and weigh the HangOut spot choice by the loved contacts' habits.

**W6. The pantry walk (distorts the day).**
- Evidence: d24 08:49 `GoTo(Home)` 129 min, `EatAtHome` PreconditionLost at 10:59, then a 155-min Market trip. d20 06:17 `EatAtHome` 15 min then `StockGone` at 06:32. V1 finding 10, unfixed.
- Cause: the plan commits to the home meal without a check against the household's pantry (unverified exactly how `PlanCtx` reads it; the plan was built "Eat on Block#16 [GoTo(Home) > EatAtHome]").
- Fix: gate EatAtHome on the pantry as it stands at plan time, and plan the Market route first when the pantry is empty.

**W7. Sleep is fragmented and gets cut by the morning meal (cosmetic).**
- Evidence: Eat interrupts Sleep at 06:15 on d21, d22, d23. d23 06:13 `Sleep 2 min`, then Eat. 5.5 h a day, never one block.
- Fix: a stronger night gate for the housed (V1 finding 12), and an Eat threshold lower in the first hour of sleep.

### The Statistical stand-in

**S1. `homeless_825` flips between Coarse and Statistical every hour, and every hour is a trip that cannot finish (distorts the day).**
- Evidence (`nopin/homeless_825.md`, d22 to d25):
  - A Coarse hour: `GoTo(Seller)` 48 min to Braindance Parlour#481, `Enjoy` -2 c, 12 minutes in, then `plan interrupted: LodDemotion`, `LOD Coarse -> Statistical`, `now at street (Mid East)`, `Leisure +2 (refund)`.
  - The next hour is Statistical, then it repeats (d22 03:15 to d25 17:00, about half the hours of each day).
  - Travel is 7.8 to 9.4 h a day. Leisure credited 1.4 to 2.0 h. Fun falls 0.58 to 0.41 from 06:00 to 22:00 on d22, so the aborted visits give none.
  - Net coins per cycle: 0. Net result: a nine-hour walking loop with no event and no fun.
- The refund loop is not a leak. Every `-2` is matched by the next hour's `+2` and the owner (Zetatech) is debited and credited the same. The d19 21:00 and d20 21:00 `+5` club refunds are the same pattern: charged at 20:05, refunded at the demotion.
- Cause: `lod::assign` re-ranks every adult hourly by view priority, then distance to the view centre (the map centre in the shadow tool), then index. She sits near the Coarse cap, so the swap flips her each hour. Hysteresis covers only an incumbent ranked within `HYSTERESIS_BAND` against an equal-priority newcomer. Demotion calls `abort_plan` and snaps her to her phase door (unverified which of those drives the flip each hour).
- Fix: a minimum tier dwell (at least 3 h, or until the plan finishes) and demote only at a step boundary. At minimum, refuse to start a paid step when the next demotion is inside the step's length.
- Severity for the real game: only no-pin and out-of-view agents. It matters for the player's view because the hour-boundary flip also happens to anyone near the Coarse cap.

**S2. Does the off-screen homeless live the same life? Economically yes, in everything else no.**

| | Pinned homeless (1829, 1887) | Statistical stand-in (nopin 1887) |
| --- | --- | --- |
| Dole | +4 c a day, in place | +4 c at about 09:27 |
| Food | 3 per 12 c every 3 days, 10 h round trip | -4 c, once a day, no trip |
| Scavenge | about 0.15 c an hour; 1 c to 6 c a week | none |
| Bed | squat (1829), Hotel 3 nights (1887) | none; energy refilled at 00:00 to 02:00 |
| Walking | 44.5 h and 46.9 h a week | 0 |
| Contacts | 1 friend (1829); 9 distinct met at HangOuts (1887) | 3 new acquaintances |
| Fun | 0.40 to 0.67 (1829) | +0.28 about every other evening for 0 c, 0.45 to 0.87 |
| Net coins over the week | +5 c (1829), -3 c (1887) | -4 c (1887) |

- The two lives agree on money because dole = meal. They disagree on the physical day. The stand-in has no food desert, no scavenging income, no ladder and no walking. If the Statistical tier learns its rows from Full agents, the Dreg's walking, empty corners and scavenge are invisible there.
- The stand-in is richer in fun than the pinned Dreg: 0.28 for free against a 53-minute walk to an empty corner.
- The stand-in's food has no market, no price, no stock: the Greenline debit is booked with no purchase. The pinned Dreg never gets food in Sump West at all.
- Fix: the Statistical homeless should carry the Full tier's two costs, a food-run debit of travel time on the day's phase (no walking is fine, but then no free fun) and a scavenge credit at `scavenge_p`.

**S3. Gossip: 209 (pinned) and 307 (nopin) tellings about homeless_825 is plausible in volume and shape, wrong in memory.**
- The agent is a Corp-class Clerk with 86 edges, known by 138, assaulted twice (Rhys Stroud, the Ninefold gang) and covered by the Civic Wire on d20 00:00. She is not a Dreg. She married on d21 and was laid off on d22.
- Volume: 307 tellings, 253 distinct teller-listener pairs. 112 carry the deed to a new listener, 195 (64 %) are "already known / dropped".
- Shape: 66, 84, 94 on d19 to d21, then 26, 22, 15. Peak on the story day and a fourfold decay in two days. That is the right shape.
- The wrong part: the same teller re-tells the same deed to the same listener inside the hour (Yuki Fletcher to Tess Carrow at 19:36 and 20:39 on d19; Zane Duarte to Dahlia Ito four times), and individuals tell it 11 to 14 times a week (Mei Haas, Cutter Okafor).
- Fix: a per-(teller, listener, deed) told memory with a cooldown of days, so repeats fall to the plausible ones. Also count new carriers separately from repeats in the stats.
- Note for the pick: homeless_825 is Housed in Block#217 at d19 00:00 and has a Clerk job at Street Market#414. It is not a street life. The `homeless` pick should require no Home at the start and no Job after the first hour.

## What reads right (against V1 and L1b)

- **Scavenging exists and fails like scavenging.** V1: never begged or scavenged. V2: 1 c over 19 hours for Vale and 6 c over 51 hours for 1887. It reads as a last resort, which is what the lever said.
- **Hang-outs make a friend.** Vale's Omar Duarte: five meetings, Acquaintance +0.12 to Friend +0.42 in four days. The regulars at the Spire corner (Mei Ravensworth, Fiora Ostrowski, Sebastian Coldwater) show up on several days. Katrin hears and passes rumours at the table while she works.
- **The dole in place.** A coin lands at 09:00 where she stands. V1 walked 2 h to the Hall for it.
- **Workers earn and are paid.** Katrin: 3 wages in 5 shifts against V1 Echo's 1 in 3. The croupier shift is a real shift in a real venue. Zero energy-0 hours (V1: 36 for the worker diary).
- **Sleep is one place.** Vale sleeps in her squat, not a nowhere. Her longest sleep is 7.2 h (V1 homeless 8.0 h, L1 7.0 h).
- **The rest day exists.** d20 is a day off with no shift, and the worker who is broke scavenges on it, which reads as honest desperation.
- **A homeless adult can use the Hotel.** homeless_1887 spends its last 3 c on a bed three nights.
- **Idle is down.** The homeless average 4.9 h a day against 6.6 h (L1 off) and 8.9 h (L1 on). Katrin has 2.0 h.
- **V1's headline bugs did not recur.** Flee never came up, there is no witness spam, and no one sleeps in one-minute fragments at energy 0.

## Verdicts

**Vale Hallow (homeless): recognisable, not yet alive.** V1 blocked the role (no income but the dole, no ladder). V2 distorts it. She scavenges, sleeps in one bed, makes a friend and eats, and the rungs exist. But a quarter of her week is a food walk to a market she cannot live near, a quarter is Wander, and the corner is empty three times out of four. Nothing happens to her, no one threatens or helps her, and no coin leaves her for any reason but food. She is a bookkeeping entry that walks. The ladder is climbable only by the Hotel, and nothing shows she tried.

**Katrin Mbeki (Friday worker): reads as a Street-class croupier, not yet as a person.** The work is a real night shift with gossip and a wage. The week is dominated by travel (61.5 h) and by rules that cost her money (a whole wage lost for 15 minutes, a purchase made and returned). A Friday night does not exist: Friday is the shift, after the shift is a 3-hour walk to an empty corner at 03:00, and the want to spend (fun 0.9 by d25) is never given a price to meet. Her Spouse and two Friends at 1.00 are as absent as Echo Singh's in V1. She is the better of the two in V1 terms: no longer broken, only unaccompanied.

## Top changes, ranked

1. W1 and W2: lock the shift (no early exit, pro-rata pay) and latch the commute.
2. H1 and W3: a nearer Market or meal point in the Sump, nearer beds, a cap on the free spot's walk.
3. S1: a minimum LOD tier dwell, and no paid step when a demotion is within the step's length.
4. H2 and the tool bug: label coins by ledger flow, add Dole to the flow table, then re-compare earnings with L1.
5. H4, H5 and H7: Beg at the corner, company-scaled fun for the free rung, a small street exposure so the tone is more than arithmetic.
6. W5, W6: a "see someone I love" goal and a pantry check.
7. S2, S3: the Statistical homeless carries a scavenge credit and no free fun; gossip needs a told-memory.
