# Shadow V11: pressure pass round 2 at 100k (2026-10-27)

Round 2's read, in V10's window so the two compare one for one: main `f0b7e44` (`target/shadow_v11/sha.txt`), the
100k island, seed 42, days 21-27, two picks per archetype plus round 2's new picks (`dodger`, `camp_parent`): 107
diaries. Seven Sonnet critics (`docs/plans/shadow_v11.md`) wrote `docs/shadow_v11/<group>.md`. Runs in
`target/shadow_v11/`; the round's 30-day 100k counters in `target/r2_100k/analyze.txt`.

## Verdict

**Better than V10 in five groups of seven; care reads worse, and most of what's left is a body that never gets
there.** What round 2 fixed shows at 100k:
- the fare jail loop is gone (the ladder fires: fines, beatings, bans) and the friendship flip and Witness flood with
  it (crime, street);
- CEOs and execs work at their own Hq; office commutes 2-3 h a day, not 9-10; a guard's escort 63-72 min, not 15 h
  (corp);
- a raid lands and has a tail: a 142-min muster, a march, a brawl, gossip all night, an arrest, ten days
  (`purist_56353 d21 23:39`); try-outs end in a line; Scavenge no longer fails on its first tick (gangs, street);
- madams book clients nightly and bar the rough ones; a joytoy serves; a fighter has a bout; a broke owner lives a
  full slide from unpaid wage to repossession (`owner_broke_7581`, vice);
- a cabbie takes 16 fares in a week; a thief goes from report to sentence in one morning (labour); back-room docs
  have patients, guns and buyers act on contracts (crime).

What still reads wrong: stories start more than they get picked up, and the reason is mostly distance and plans that
fail at the end of a long walk. Care at 100k reads worse than V10 (no clinic worker sees a patient, the supers do no
round).

## Next pass, ranked by what the player sees

1. **Walks that never arrive at 100k** (`OPEN-221`). R2 fixed the pace (min a tile 2.06 -> 1.06); what's left is
   where things are and goals that ignore it: same-district legs of 200-800 tiles cut at the plan limit
   (`forger_102600 d25 00:00` 724 tiles; `miserable_22034 d26 12:14`, a 72-year-old walking 801 tiles to deal;
   `night_shift_27704 d21 08:31` 767 tiles; a day labourer's "site" 237 tiles in another district). The map
   re-import (`OPEN-204`, `OPEN-182`) is half of it; a distance term in the choice of site, spot, muster and hire is
   the other half.
2. **Plans that fail at the end** (`OPEN-222`). Collect and Court are one-minute `PreconditionLost` no-ops after
   8-11 h walks (`gang_leader_96308 d21 08:43`, `d26 19:24`; V10's, unchanged); Hunt flickers "could not plan or
   plan failed twice" every two hours (`buyer_82590 d27`; a KilledKin grudge logging 57 Hunt goals before one plan,
   `vendor_19534`); a raider who sat the muster is marked "missed" (`raider_39544 d22 00:07`).
3. **Justice that reads broken** (`OPEN-223`). A sleeper attacked three times in 80 minutes is arrested and sentenced
   to ten days for Assault, the attackers never named (`miserable_29154 d21 01:27-04:26`); jail is a week at energy
   0.00 starving from d26 (`jailed_75054`), with 1,171 new Acquaintance lines in one jailed day
   (`arms_dealer_65421`); a dodger beaten at the El daily (`dodger_29048`); guards on patrol witness 132 crimes and
   arrest from none, and jail duty is one 9-hour action (`guard_8360 d23 21:02`).
4. **Care at 100k** (`OPEN-216`, kept open). No `Treated:` in two ripperdocs' and two orderlies' six work days; the
   sick stop after one refused ask, their spouse and six Friends never asked (`sick_91688 d26 00:00`); the supers do
   one 500-min ClerkWork and no round when no Block is in arrears (`super_104162 d21 09:02`). R3's chain read right at
   20k; something at 100k starves it of patients.
5. **Trades still thin** (`OPEN-217`, kept open). Bouncers never refuse or eject in 12 nights; a day labourer hired
   and then scavenging all day (`day_labourer_75212 d27 06:00`); a cabbie `Paid` daily with no coin, 0c all week
   (`cabbie_82150`, a bug); a cabbie resting 9.8 h at the Garage as work; reporters interview and print no Story at
   100k (R6b's beat story fired at 20k); a hit on a CEO the CEO never hears of (`ceo_88522 d24 18:00`).
6. **The seeded city has no school-age children** (`OPEN-224`, R6b and the care critic): every School's roster is
   0, the teachers' days are empty, a daily "pupil's parent" line has no pupil. Seed them at world generation
   (`OPEN-150`) and gate School by age: Dylan's call.
7. **Noise** (`OPEN-225`). A raid minute writes 260 Witness lines, 10 Assaults and "256 more relationship changes",
   and no "Raid on X: won/lost" line (`purist_56353 d22 00:06`); a fare-evasion witness line repeats at the same
   minute five days running (`gang_leader_18246`); "promoted to Reporter" while already one.

Day 27 is the city's rest day (R4 marks it "rest day: no shift" in the summary); the vice critic read its zero work as
a bug again. The marker is in the day summary only; the hour lines could say it too.

## What round 2 did to the city (100k, seed 42 x 30, `r2_100k` against V10's `pp100k`)

Printed, not gated: employed mean 39.3k -> 39.8k (J24 0.430 -> 0.437), thefts 22.5k -> 17.0k, arrests 16.2k ->
9.3k, Murders 264 -> 242, deaths by violence 601 -> 393, starvation deaths 34 -> 10, jailed mean 890 -> 1,028
(max 2,222 -> 2,852), homeless max 12.8k -> 11.3k. Ticks/s days 25-30 on the same method 23.7 -> 24.8 (worst tick
2.0 s): the round did not move it, but both read under the perf pass's 30 at its own gate (`OPEN-226`). At 20k R2's
pace alone cost 256 -> 222 ticks/s on a quiet box. The 20k city (scaled) holds: 0 starved, employed 0.59.
