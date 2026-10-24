# Shadow V8: the gangs (critic report)

Archetypes: gang_member, gang_leader, purist, cultist, scav, dealer, runner, plus the new arms_dealer. Every event is a seeded abstraction; I judge whether it reads as the trade and as a life.

Paths are `<window>/<file>` under `D:/TheCity/target/shadow_v8/` (V7 under `shadow_v7/`). Stamps are the diary's own. Counts come from scripts over the diary lines in both trees (my eight archetypes), never from missing lines. `isle_s42_d0` has no gang candidate in either pass. V8 diaries read: isle_s43 16, isle_s44 18, gen 18, 2k 10, nopin 1 (`gang_member_8447`).

**Verdict: the pass fixed the arrest teleports, the Hideout/Home ping-pong and most of the Harvest dead-end, and put raids on screen. It did not fix the gang member's real problem on the island: every gang has one Hideout, most in The Sump, the members live anywhere, and the bed, the work and the rounds are still 4-17 hour walks. A day-job leader (`isle_s44_d90/gang_leader_9711`) spends 14-20 h a day travelling and works 9.7 h in a week.**

## 1. Fixed (V7 said broken, V8 shows fixed)

- **Arrest and transfer jumps: gone.** Jumps of 20+ tiles in my archetypes: V7 11 (`isle_s43_d30/scav_23612` d30 02:30 390 tiles in 21 min; `purist_23879` d30 00:29 560 tiles; 440-tile cell-to-precinct pops), V8 0. Held rows read as a person in a cell: `isle_s44_d90/dealer_36006` d91-d96 "jail 24.0h, Precinct#4460 1440m".
- **The Hideout/Home ping-pong: gone.** V7 `gen20k_s43_d30/gang_leader_23560` slept 11 min at the Hideout and walked 109 min home, three nights. V8 Sleep at a Hideout under 60 min: island 1 of 46 (V7 1 of 40), gen 0 of 12 (V7 3 of 19). `isle_s44_d90/gang_leader_21251` sleeps 476 min at Hideout#4480 each night d91-d95.
- **Harvest ExpansionCap: mostly gone.** "GangWork unplannable: ExpansionCap": V7 35 on scavs (30 in `scav_7234`), V8 23 in all (7 in `isle_s44_d90/scav_17252`, 4 in `isle_s43_d30/cultist_8817`). No diary repeats it 30 times. The scav still does not harvest (section 2).
- **Raids on screen.** `ACTION Muster` 1 -> 4, `ACTION Brawl` 0 -> 4, `PLAN Raid` 3 -> 11, `ACTION Patrol` 5 -> 22. `isle_s44_d90/arms_dealer_22850` d90 19:32 "PLAN Raid on Street Market#4421 [GoTo(MusterPoint) > Muster > GoTo(RaidTarget) > Brawl]". Orders are given in person: `isle_s44_d90/gang_leader_14477` d90 12:41 (Raid), `gang_leader_21251` d95 06:59 (Raid).
- **Protection rounds.** `gen20k_s43_d30/gang_leader_9939` d32 10:36 and 15:16 "collected 40 coins of protection ... from the till, X on shift"; `k2_s42_d26/cultist_2568` d31 14:54. V7 had one.
- **Deal as a 4 h street statue: mostly gone.** Deal blocks of 100+ min: V7 21, V8 7. Dealers deal from a venue (Bar#4440, Club#5818, Fight Pit#4806). Dealer travel 8.4 -> 3.9 h a day; their legs of 240+ min 12 -> 3.
- **The 2,250c dealer ledger hack: gone.** V8 hacks are runners' (131c, 600c, 200c: `gen20k_s43_d30/runner_7993` d31 18:01 and d34 04:16, `isle_s43_d30/runner_8403` d32 18:09).

## 2. Still broken

- **The Hideout bed and rounds are a cross-island commute.** GoTo(Hideout) legs on the island: V7 n 85, median 103 min, 24 at 240+; V8 n 78, median 144 min, 21 at 240+. 58 of 78 target a Hideout in D1 The Sump; the walkers live in D3-D6. Phase 1 changed which bed a gang member picks, but a gang has one Hideout. Longest: `isle_s44_d90/purist_14764` d93 00:02 1,061 min (Club D5 -> Chapel D1); `isle_s43_d30/cultist_8817` d32 00:27 674 min and d36 00:24 697 min (Vats -> Sump).
- **Legs of 4 h or more, all my archetypes, island:** V7 139 of 605 (23 %), V8 118 of 768 (15 %); gen 29 -> 21. Better, not fixed. Same-district legs of 240+: 41 -> 25.
- **Same-district GangWork walks of 4-13 h.** `isle_s43_d30/gang_member_7638` (Ninefold, 48 members; Home and Hideout both in D1): d35 00:16 "GoTo(Spot) @ Hideout#4464 -> street (D1 The Sump), 808 min (done)"; d34 08:07 GoTo(TargetHome) 351 min; d34 02:59 GoTo(Spot) 245 min. Day summaries: d33 travel 16.2 h, d34 14.7 h, d35 20.1 h with 2.6 h sleep, all in one district.
- **Court is a 60-110 min statue, or a 1-min fail.** `isle_s43_d30/gang_leader_25004` d30 19:14 106 min, d31 19:12 108 min; `isle_s44_d90/gang_leader_21251` d90-d94 19:57 "63 min", five nights, a 3-member gang, nobody named, no order. Fails: `gang_leader_9711` d92 03:54 1 min, `gang_leader_14477` d92 09:38 and 20:46 1 min.
- **"0 to 0 members" (OPEN-118) is not fixed.** 8 of 17 Collected lines: `gen20k_s43_d30/gang_leader_20053` d34 12:16 (a 16-member gang, "take 0"); `isle_s44_d90/gang_leader_7555` d90 23:54; `k2_s42_d26/gang_leader_1739` d27 02:30 ("take 42").
- **Scavs never harvest or abduct.** 0 `ACTION Rip` or Abduct in any scav diary (V7 0). `isle_s43_d30/scav_19092` (Hollow, order Harvest, 49 members) has a job and never plans GangWork. Extorts of 0 coins: `gen20k_s43_d30/scav_21455` d30 02:01, `k2_s42_d26/scav_1970` d26 08:10, `purist_15191` d30 09:57.
- **Cultists preach on a clock and never persuade.** `gen20k_s43_d30/cultist_25229` Preach 31 min at street Sump 6 every day d30-d35. `isle_s43_d30/cultist_7340` Preach d34 11:16, d35 11:36, d36 08:36, each followed by "Persuade failed p=0.30 / 0.45 / 0.48". Calibration, not a bug.

## 3. New (regressions or new shapes)

- **The Spire-job leader who cannot work.** `isle_s44_d90/gang_leader_9711` (Ninefold, 37 members): Home Block#388 D4, job Office Worker at Corp HQ D5 (shift 540-1080), Hideout D1. It sleeps at the Hideout and commutes from there. d91 05:57 "GoTo(Workplace) Hideout -> Corp HQ, 589 min", arrives 15:46, "Rest 2 min", d91 15:50 "GoTo(Seller) -> Hideout 724 min". d91 and d93: sleep 4.0 h, travel 19.9 h, work 0. d90 travel 14.9 h, d92 13.2 h, d94 13.8 h. Work in the week: 9.7 h. The day job loses to the Hideout bed, not to court.
- **Twelve-hour naps on a pavement.** `ACTION Sleep @ street`: leaders V7 12 of 101 sleeps, V8 18 of 119; dealers 1 of 29 -> 7 of 33. `isle_s44_d90/gang_leader_14477` (Vat Tech) d91 18:47 "Sleep @ street (D6 The Vats), 709 min", six minutes after leaving the Vat Farm; `gang_leader_7555` d90 02:55 711 min; `isle_s43_d30/gang_leader_8256` d33 17:50 747 min in D2 Civic Core; `gang_member_8447` d31 06:47 784 min. A boss of 16-49 asleep on a street for 12 h reads as no one's life.
- **Scavs idle at home in 1-minute Rests.** Scav `Rest` actions V7 77, V8 312; idle 4.6 -> 6.2 h a day. `isle_s44_d90/scav_14509` (Static Rats, order Squat): d91 12:47-17:47 "ACTION Rest @ Block#647, 1 min" every 30 min for five hours, then four 61-min HangOuts; d91 summary "idle 8.3h". The Squat order gives it nothing to plan and the Idle goal fills with 1-min Rests.
- **Dealers sell off the diary.** `sold to` memories with the dealer as seller: V7 7, V8 1 (`gen20k_s43_d30/dealer_19226` d30 13:29). `isle_s43_d30/dealer_8852` d30 14:31 "Deal @ Club#5818, 211 min" has no sale line. The trade moved indoors; I cannot tell from the diary whether sales still happen.
- **Arms dealer at a Bar: one deal, then a 6 h walk.** `isle_s44_d90/arms_dealer_22850` d90 00:00 "Deal @ Bar#4440 (D2 Civic Core), 140 min" (plan `GoTo(Hideout) > PickUp > GoTo(Seller) > Deal`), then d90 02:20 "GoTo(Home) ... 371 min", then d90 19:32 GoTo(MusterPoint) 258 min ending in D3 for a D1 target. 2 Deal actions across the four arms-dealer diaries; the island two have no job and live on Scavenge. They are gang members who hold a stock, not a visible trade (the crime critic reads this).

## 4. The sore points, one line each

- **Leader Hideout/Home ping-pong (phases 1-2):** fixed (section 1); the bed that remains is the Spire leader's.
- **Leaders' day jobs vs court and rounds:** a leader with a job beside the Hideout works (`gen20k_s43_d30/gang_leader_20053` ClerkWork 47, Court 5, Collect d34); one with a job a district away loses it to the commute (`gang_leader_9711`).
- **Arms dealer from a Bar (phase 8):** present (`arms_dealer_22850` d90 00:00), not a watchable trade.
- **Scav:** unchanged as an actor.
- **Phase 10 / OPEN-186:** nothing in my diaries shows off-screen gang violence. Murder 5 -> 3, Death 5 -> 3, Assault 28 -> 26, Arrest 45 -> 61. The Violence deaths are on screen (`gen20k_s43_d30/purist_16297` d34 16:24, `cultist_16374` d34 08:23).

## 5. Bugs, ranked

1. **Gang members commute to one Hideout district.** Evidence: section 2 (median 144 min, 21 of 78 at 240+, 58 of 78 in D1) and `gang_leader_9711`'s 19.9 h days. Site: `citysim/src/systems/plan.rs:606` `bed_of` returns the gang's one Hideout for any Sleep plan with `GoTo(Hideout)`; the Sleep goal does not compare it with Home or Work (unverified: not read past `bed_of`). Fix: gate the Hideout bed on the walk, with Home as the bed when the Hideout is farther than a day-job commute; or one Hideout per district for a big gang. No knob.
2. **Same-district GangWork walks of 4-13 h.** `gang_member_7638` d35 00:16 808 min. The target of `Collect->Amara Nightingale#9035` is a person, not a tile; a GoTo(Spot) that re-resolves to a moving person would explain the length (unverified). Fix: resolve the Spot to a tile at plan time and cap the leg.
3. **Sleep on a street after a shift.** `gang_leader_14477` d91 18:47. The Work -> Sleep interrupt sleeps where the body stands; give a person with a Home the bed-walk (the homeless are exempt).
4. **Scav 1-min Rest loop.** `scav_14509` d91. Fold into one Rest block or give Squat a place to go.
5. **"0 to 0 members" in Collected.** The count reads the lieutenant tier only (OPEN-118).
6. **Court as a statue / 1-min fail with no reason.** Print the attendee, end court at the last arrival.
7. **Scavs never Harvest or Abduct** (OPEN-097; the dead-trade plan OPEN-176 stands).

## 6. FINDINGs (calibration, no knob)

- Cultist Persuade failed 3 of 3 at p 0.30-0.48 (`cultist_7340`); preaching at one street spot at one hour daily.
- Dealer sales on camera 7 -> 1; deals are venue stays.
- 6 of 10 scav Extorts pay 0 coins.
- Idle per day: member 3.6 -> 2.5 h, leader 1.7 -> 3.1, purist 2.2 -> 3.8, scav 4.6 -> 6.2, runner 3.1 -> 4.4. This lens shows the 1.5 h idle growth of the verdict.
- Travel per day fell for most (dealer 8.4 -> 3.9 h, purist 9.3 -> 6.9, scav 7.2 -> 4.6) but not for cultists (10.1 -> 9.9) or for leaders with a day job away from the Hideout.
- A 3-member gang leader with no job who vends on the street six hours a day and holds court at 19:57 (`isle_s44_d90/gang_leader_21251`) reads as a person.
