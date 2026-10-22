# Shadow pass V7: the pre-view pass gate

Read on 2026-10-22: main `92c4a37` (the pre-view pass phases 0-6) with the V6 tool plus V7's label fixes (`800922d`).
The brief is `docs/plans/shadow_v7.md`; the runs and measurements are in `docs/plans/shadow_v7_runs.md`.
Seven Sonnet critics wrote `docs/shadow_v7/<group>.md` (street, gangs, crime, corp, labour, vice, care).
The base is `4fb10b9`, the pre-pass main. V7 differs from V6 in one way: the 20k windows run on the island
(`isle_m_k2`, Track B), and V6's generated 20k city (gen) comes back as a control. V6's diaries are gone, so every
V6 figure comes from `SHADOW_V6.md` and `shadow_v6_runs.md`.

## Verdict

**The pass did what it set out to do, on the city it was built on. The island breaks most of it.**
On gen and at 2k:
- the shift reads as the trade;
- the idle freeze is gone;
- settlement waits for both bodies;
- housing no longer churns;
- the implant loop is fixed;
- leaders hold court and give orders in person.

Six of ten labour archetypes, and the forger, the black doc, the doctor, the orderly, the teacher, the warden and the
reporter, now work in 25-90 min blocks over 2-5 tiles (labour § 1, care § 1, crime § 1). The longest work action
fell from a 530-540 min median to 89 (runs).

The island is the city the camera will show, and there the destinations are chosen without regard to distance:
- the job;
- the bed (the Hideout bed included);
- the hangout spot;
- the hotel.

Timed rides also park the body on its boarding tile. So:
- A quarter of island commute legs run 4 h or more, against 0.5 % on gen and none at 2k (labour § 2).
- A third of island labour pick-days are mostly travel, and 36 of 117 at day 90 hold no work at all.
- 89 island pick-days hold one still stretch of 3-11 h with the body on a platform tile (labour bug 2).
- A gang member travels 23.6 h in one day and does not sleep (gangs bug 1).
- A madam walks 7 h to a hangout and 7 h to work, then works 85 min (vice bug 1).
- By day 90, 2,188 islanders are sick. They are hunger-sick: carers among them, on shift and unfed, with no clinic
  treating it (care).

Row 6, the dead trades, is mostly still dead. On the island:
- the cab drives 0 fares;
- the courier hands over 0 parcels, though 10 on gen;
- the bouncer refuses no one;
- the joytoy has no clients;
- no arms dealer exists;
- the scav is never seen harvesting.

**Gate: not passed.** Rows 2, 3, 5 and 7 read as fixed, and row 1 as fixed for most trades. Row 4 is worse on the island,
row 6 is unchanged, and the island's distances are a new row-0 failure above them all.

## The devices (runs)

- **Composed off identity:** all eight pass switches off, against `4fb10b9` on 42-44 × 30, is **identical**.
- **`shadow --assert`:** 132 PASS, 0 FAIL. V6 had 133; phase 5 demoted one sweeper bullet, with its reason.
- **Milestone tier: RED on LOD parity.**
  - Hunger-days, Full 0.578 vs Statistical 1.163, comes from phase 0's idle fix.
  - Ride share, 24 vs 35, comes from phase 2's stations.
  - Paid venue visits, 12.3 vs 14.2, is a sum of five phases.

  A fix round is on it. `core_20k` and `god_20k` pass.
- **`--lives` at 20k:** gen idle 3.46 h/day, met 0.29; island idle 2.93, met 0.22, travel 50.6 h/week.
- **Ticks/s on the island:** 260 → 255 (A/B, print only).

## Before the camera, rows 1-7 as read

| row | V6 | V7 on gen / 2k | V7 on the island |
| --- | --- | --- | --- |
| 1 statue shift | 530-540 min on 55 % of labour days | fixed for most trades; still one block for the fabber (`FabWork` 537), the super's `ClerkWork` (383), the Court and Deal blocks (40-227, up to 4 h), and Fab Tech, Vat Tech, Engineer and Sanitation (not in `[shift.roles]`) | same, but swamped by travel |
| 2 idle freeze | 14 of 33 street diaries | fixed (45 still days of 12 h or more: 35 in jail, 2 corpses, a few one-off cases) | fixed |
| 3 presence | 8 % settled apart | 1.0 % apart, ~1 % asleep; leaks left: the teacher's door, the orderlies' 00:00 batch, the 21:00 venue counters, a vendor selling asleep, the pimp collecting from a Statistical worker | same |
| 4 long legs | 300-901 min GoTo legs | partly fixed; 2k is not clean (an office worker commutes 3.5-4.7 h) | **worse**: 4 h+ legs in 19-25 % of labour and gang legs; 13 legs of 600 min+ in crime; same-district rides of 10 h |
| 5 housing churn | double `Housed`, execs away | fixed; no double `Housed` | 6 of 18 corp picks sleep away 3 or more nights (the runs' "1 of 24" counted only 0.0 h weeks) |
| 6 dead trades | cab, courier, bouncer, joytoy, leader, scav | leader alive in part; courier alive on gen; the rest unchanged | all dead but the leader and the loader; island foremen hire nobody |
| 7 implant loop | 14 h over 3 days | fixed (7 installs, 0 loops) | fixed |

## New and remaining bugs, ranked by what a followed character sees first

1. **Destinations without distance (island).**
   - Job, bed, Hideout bed (`plan.rs` `bed_of`), hangout spot and Hotel are all picked with no distance term or cap.
   - A segmented leg ends in the wrong district: a CEO bound for a Spire Hq rides 901 min to the Sump and sleeps on
     the street there (corp bug 1, `exec/legs.rs` `segment_end`).
   - A leader ping-pongs between the Hideout and home: 1.8 h asleep, 12.7 h travelling (gangs bug 3).
   - Every critic but care names it.
2. **The body parked for a timed ride.** El rides and `exec_ride` leave the body on its boarding tile for 3-11 h, then
   land it hundreds of tiles away. This is by design, and it is the same freeze-then-jump that row 4 was meant to end
   (labour bug 2, corp bug 3, street bug 1).
3. **Jumps at arrest and transfer.**
   - Prisoners move at 04:00 with no walk, 286-596 tiles (`stations.rs:347`, crime bug 1).
   - The arrest hand-off writes the body to the Station.
   - The escort is a 200-250 min statue (corp bug 2, street bug 3).
4. **Hunger on the island at day 90.** Malnourished carers work their shift, and no clinic treats malnutrition (care
   bug 1). It probably follows from 1 and 2, since travel eats the working day; re-read it after they are fixed.
5. **The dead trades (row 6).**
   - The courier and builder `Unload` at their own post (labour).
   - No cab driver is a body at 20k (`OPEN-149`).
   - The bouncer never refuses and the joytoy gets no client.
   - The scav harvest is seen only from the victim's side.
   - No arms dealer exists.
   - Island foremen make 0 hires, and there is no island day labourer.
6. **The remaining one-block shifts.** The fabber; four roles missing from `[shift.roles]`; the super after their round;
   supers with no round, 3 of 9 (likely `round_reach_tiles` 20 on the island's large Blocks); 12 of 16 sweepers
   stand 5 h or more instead of walking the round; Court and Deal.
7. **The presence leaks** listed in row 3.
8. **Smaller ones.**
   - The foreman hires off a trade: 6 of 70 vice diaries (`OPEN-143`, grown).
   - "Hotel" still resolves to a Soup Kitchen or a Church 33 times (street). The vice critic saw a real Hotel, so
     `OPEN-140` item 5 is only partly fixed.
   - The 2k warden counts nobody.
   - The fare-evasion loop.
   - A gun dies "of Violence" with no attacker line.
   - A doctor sits at the exec's table hourly on shift (`service.rs:954`).
   - A 2,250c LedgerHack.
   - A corp CEO who leads a gang and works as a Vat Tech.

## The pass's FINDINGs on the island at 20k (42-44 × 30 against `4fb10b9`)

- Murders 14 → 16.
- Employment 0.578 → 0.585.
- Arrests +34 %.
- Wallets −17 %.
- Parcels −9 %.
- `served_tenant` −77 % (`stat_home_p` is still the 0.5 placeholder).
- Contract encounters −53 % (`OPEN-157`).
- Eat→Earn aborts +54 %.
- Protection ~2 a seed (`OPEN-162`).
- 40 % of orders go by word (`OPEN-161`).
- 36 % of sweeper shifts are stands.
- At 2k (42-47 × 120): Murders 70 → 79, employment flat.

None is tuned; the wallets and the hunger read together with bug 1.

## What it means for the order of work

The camera goes on the island, so the island must read before the view milestone. The proposed next step is a short
second pass, specced with Dylan:
- distance in every destination choice;
- the body carried along a timed ride;
- walked transfers;
- then the dead trades of row 6;
- then a V8 read on the island.

Gen and 2k need no more work for the camera.
