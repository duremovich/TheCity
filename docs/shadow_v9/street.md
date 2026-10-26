# Shadow V9, street critic (homeless, sick, elder, vendor, street_worker, sweeper, camp_warden)

Window: main `4108184`, 100k island, seed 42, d21-27, two picks per archetype (14 diaries). Cites are `file dDD HH:MM`, files under
`target/shadow_v9/isle100k_s42_d21/`. Read: every camera.tsv row of the group, all DAY SUMMARY lines, every GoTo leg of 100+ min,
and full days for `sweeper_23741` d22, `vendor_8378` d23, `vendor_16990` d22, `elder_79050` d22, `homeless_10129` d23,
`homeless_57802` d26-27, `street_worker_16640` d21, `street_worker_15964` d24-26, `camp_warden_23439` d23.

## 1. Verdict

The street group reads as people living lives only while it stays on foot inside one district: the pitch, the squat, the Vat Farm shift.
Any plan whose target is past a certain distance (even in the same district) sends the body onto the El for 4-9 h with nothing happening (V8 bug 1, still here, now hitting vendors, sick and street workers too).
Elders and the jobless young idle at home for days, and every body watches dozens of crimes and reacts to none.
The best material is thin but real: a street worker who pitched, was assaulted, bought a Legs implant and was jailed for Solicitation (`street_worker_15964` d24-d26).

## 2. Per archetype (alive / thin / dead)

- **Homeless: alive, mechanically flat.** `homeless_10129` (33 y) is on the street 16-19 h a day: 76 Wander, 51 Scavenge, 12 Sleep at a squat, net 0c all week. `d23 05:51` "PLAN Earn [Wander > Scavenge]", three Scavenges, `d23 09:26` "goal Earn cooled ... could not plan or plan failed twice", then 3.5 h of 30-min Wanders. `homeless_57802` is the better story: hired at the Vat Farm (work 10 h d22-d26, +5c/+6c a day), then `d27 06:00` StealVehicle, `d27 08:59` Fence at The Drowned Choir for 12c, `d27 11:22` first REAL meal.
- **Sick: thin, care still missing.** `sick_82185` has diagnosed cancer 0.38 -> 0.48 over the week, 0 Ripperdoc or clinic lines, and "OFF SICK TODAY" on d22/d26/d27. Real gain: `d22 00:10` "Quit: ... quit as Vat Tech (far commute)" and a same-minute rehire at Vat Farm#1702 in her own district. `sick_85595` (72 y, silent cancer 0.07) works 9.7 h for 6c; nothing to find.
- **Elder: split.** `elder_58829` (73 y) works Vat Tech 9.7 h at 6c/day: fine, but a worker, not an elder. `elder_79050` (61 y, sociability 0.97) is in Block#549 24.0 h on d21-d24: "PLAN Idle [Rest]" 90-min blocks, d22 summary idle 18.6 h, 2 UNWIND picks in 4 days, both the home HangOut. d25 he rides 13.8 h; d26-27 normal.
- **Vendor: one alive, one dead.** `vendor_8378` (75 y) is the best vendor so far: 66 `sold to`, 7 Vend hours a day (`d23 09:35` to `17:59`), +10c. But every buyer is a Statistical stranger "[off]" and his seven days have identical hours (home 14.6, street 9.4 on every row). `vendor_16990` makes 0 sales: its Pitch spot is two districts away (Bug 2). Vendor-asleep settles are 0 (OPEN-170 gone in this sample).
- **Street worker: alive in one, jailed in the other.** `street_worker_15964`: `d24 00:01` pitch, `d24 03:00` Assault at the pitch (0 coins), `d24 04:00` "+5c (Leisure)" client, `d25 12:25` bought a Legs implant for 60c, `d26 03:42` Arrest, `d26 07:38` "sentenced to 1 days for Solicitation", `d26 22:49` rearrested for Fare Evasion. `street_worker_16640` (18 y, greed 0.90, wealth 0.08) sat at home 21.2 h d21 and 13.4 h d22 on "PLAN Idle [Rest]", with a stealing Earn plan aborted after 6 min (`d21 18:44`-`18:50`); arrested `d23 15:11`, then 4 days of 24 h jail.
- **Sweeper: thin.** Both picks are on the El most of the week (Bug 1). When they reach the Recycler, one Sweep action lasts 207-320 min (`sweeper_18995` d25 12:40 "Sweep @ Recycler#4486, 320 min") with a Fare Evasion witness line every few minutes and nothing else.
- **Camp warden: alive.** `camp_warden_23439` d23: four ClerkWork blocks of 65 min with `GoTo(Station)` between, a Break, and 12 "counted X (the Camp's headcount)" memories (`d21 09:00`). Both wardens work 9 h five days a week. The 2k zero-headcount regression is gone at 100k.

## 3. Bugs, ranked by what the player sees first

1. **The El ride across (or inside) one district eats 4-14 h a day and ends nothing.** GoTo legs of 100+ min per diary: `sweeper_18995` 18, `sweeper_23741` 18, `vendor_16990` 12, `street_worker_15964` 9. Examples:
   - `sweeper_23741` `d22 04:17` "GoTo(Beat) ... 384 min" Block#3101 (D1) to Recycler#4486 (D4): D2 07:15, D3 07:54, D1 09:57, back in D1 at 10:41 with work 0.0 and the 09:00 shift missed. `d22 18:02` "GoTo(Home) ... 419 min" D1 to D1 via D3 and D2. d27 travel 20.5 h, sleep 3.1 h.
   - `sweeper_18995` `d25 06:05` "GoTo(Beat) ... 230 min" Block#588 (D4) to a street in D4; `d25 09:59` "GoTo(TargetHome) 161 min" D4 to D4.
   - `vendor_16990` `d21 09:10` "GoTo(Spot) -> cut at street (D5), 540 min", `d21 23:03` "GoTo(Home) 553 min"; `d24 10:24` 466 min then 468 min back. `d27 06:08` "GoTo(Hall) -> Civic Hall#4487 (D2), 713 min".
   - `street_worker_15964` `d24 00:01` "GoTo(Spot) ... 137 min" within D3.
   - `homeless_57802` `d27 11:24` "GoTo(Home) ... 327 min" from Street Market#4298 to Block#1727, both D1.
   - `sick_82185` `d22 18:05` "GoTo(Hall) 370 min" within D1.
   Likely cause is unchanged from V8 bug 1: `transit.rs::route` choosing the structural loop ride (the huge ETA is the signature). Each of these should walk or take a direct hop.
2. **A vendor's Pitch spot is chosen in another district.** `vendor_16990` Pitch at `d21 09:10`, `d22 14:05`, `d23 09:10`, `d24 10:24`, each `GoTo(Spot)` from home in D4 to a street in D5/D6; 0 Vend actions, 0 sales all week; `d25 00:07` "PitchClosed ... (no take)". The spot should be in the vendor's district or near home.
3. **The sweeper's post is in a different district from the home, so the shift is missed.** `sweeper_23741` Block#3101 (D1) to Recycler#4486 (D4); 6 Sweep actions in 7 days (`sweeper_18995` 21). `d22 12:33` "Work unplannable: ExpansionCap" for an employed worker. The post should be in reach (OPEN-194).
4. **Jobless young adults sit at home idle while Earn aborts.** `street_worker_16640` `d21 18:44` plan "Earn ... StealVehicle->Motorcycle T1" interrupted to Idle after 6 minutes, then more 90-min Rests. Same for `street_worker_15964` d21-d23 (home 21.8, 12.8, 21.4 h). Day-0 Idle fallback (OPEN-135) still the default when nothing scores.
5. **Elder idles at home on a 90-min Rest loop for 4 days** (`elder_79050` d21-d24; V8 bug 2 persists). Sociable (0.97) with a Friend (Nadia Moreau +0.87) who is at the same HangOut on some days, so it is not for lack of company.
6. **Diagnosed cancer never reaches a doctor** (`sick_82185`; V8 the same). "OFF SICK TODAY" is the only response.
7. **Witnesses do not react.** `camp_warden_23439` `d23 03:44` "saw Fenn Hallow commit Shakedown", `d23 07:43` "saw Hilde Wexford commit Assault"; ReportCrime scores 0.003. `sweeper_18995` d25 logs 20+ Fare Evasion/Theft witness lines in one day and no report.
8. **Same-minute Quit and Hire** (`sick_82185` `d22 00:10`): no unemployed gap, reads as a pointer shuffle rather than a life event.

## 4. Dull spots, ranked (mechanism, not knob)

1. **Jail is a black hole.** `street_worker_16640` d24-d27: "jail 24.0h", 0 plans, 6161 min still, only "MetInJail of X" memories. Mechanism: a jail day plan (cell, yard, work detail, a fight with a cellmate, a visit from kin, a gang recruiting inside, a stash) so a sentence produces story, and a release event with consequences (lost room, lost job, debt).
2. **Witness without consequence.** 83 Witness lines in `street_worker_15964`, dozens in `sweeper_18995`, none acted on. Mechanism: a witness rolls a response by role, trait and tie: a warden or sweeper reports to the nearest precinct, a friend of the victim intervenes, a low-lawfulness bystander pockets the dropped goods, a coward walks away and keeps a memory of it.
3. **Homeless with no yield.** `homeless_10129` net 0c all seven days, Scavenge cools after three tries. Mechanism: what a scavenge finds depends on the place (a market alley gives scraps, a Recycler gives parts, a rich quarter gives a dropped wallet or a beating) and a fence path (a Recycler or gang buys parts); a squat that can be taken or lost ("someone else sleeps in Block#3258").
4. **Vendor with no customers on screen.** `vendor_8378` sells to Statistical strangers and his days are identical. Mechanism: let a Full-LOD walker in range actually stop and buy; let a regular form (repeat-buyer memory to Friend edge); add a bad-day variant (a drunk who will not pay, a spoilt stock) and the protection visit of PRESSURE_PASS phase 3.
5. **Elder as a figure of the street.** `elder_79050` sits in his flat. Mechanism: elders hold a stoop spot outside their block, sit for hours, watch the street (witness gossip), pass rumours (they already carry "Evicted by corp ..." rumours) and know everyone. A reason to be on screen and a gossip node for the player.
6. **Diagnosed sick on a 6c wage.** `sick_82185` cancer 0.48. Mechanism: the diagnosed price a treatment ("300c, you have 5c") and choose: quit work and go home to die, borrow from a gang, steal, sell a kidney. A deadline (stage ~0.9) forces the choice.
7. **Camp warden is a pure clerk.** `camp_warden_23439` 9 h of ClerkWork and Break; headcount memories fire no event. Mechanism: a count that comes up short (a child missing, an inmate absconded) starts an event: search, report, bribe, cover up.
8. **Sweeper rounds are single 320-minute actions.** Mechanism: a beat is a route of 4-6 stops with finds (a body, a stash, a broken terminal) and a short action at each, so the sweeper meets corpses and caches; a body found goes to the Precinct or a Ripperdoc and leaves a SawCorpse chain.
9. **The street worker's pitch has no turf.** The one good story has no pimp, no protector, no rival pitch. Mechanism: pitches belong to turf (a gang or pimp takes a cut or beats an off-turf worker; PRESSURE_PASS phase 2 pimp); an arrest offers a fixer or a bribe; the fine comes out of the implant savings, a real loss.

## 5. Already in PRESSURE_PASS.md, evidence seen

- **Phase 1 misery acts**: `street_worker_16640` mood -0.20, wealth 0.08, greed 0.90, courage 0.84, jobless, three hours of Rest after an aborted Earn: a clean absenteeism/petty-crime target. `sweeper_18995` d25 `GOAL JoinGang 0.362` ranked behind Work and Unwind, so a recruitment mood term has a live candidate.
- **Phase 1 quitting**: `sick_82185` `d22 00:10` "Quit ... (far commute)" is the commute-driven quit already firing, with no downtime.
- **Phase 2 pimp / dead trades**: `street_worker_15964` d24 "+5c Leisure" client and the Solicitation arrest, no pimp present.
- **Phase 3 protection**: `vendor_8378` sells 66 times in 7 days with no extraction line.
- **Phase 4 reach (OPEN-194, OPEN-210)**: Bugs 1-3 are reach failures (sweeper post in D4 from D1, vendor spot two districts away, GoTo(Hall) 370-713 min). A reach cap per purpose would remove them.
