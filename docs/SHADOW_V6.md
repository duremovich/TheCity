# Shadow pass V6: the pre-view read

2026-10-20, main `a25fdc5` (Lives at work closed) read with the V6 tool (`f81289e`, merged `5d77afa`). The brief is `docs/plans/shadow_v6.md`. Shadow V6 is the assessment before the view milestone and M18, at the game's size (20k, Dylan 2026-10-19: 2k is a test fixture). It reads three things: lives, what a camera following one person would see, and the first week. There are 298 diaries in five windows: 20k on days 0, 30 and 90 (seeds 42, 43 and 44), the 2k control, and an unpinned 20k window. There is also a 30-day run of 20k on the imported island. The runs, the camera columns and the island numbers are in `docs/plans/shadow_v6_runs.md`. Seven Sonnet critics wrote one critique per group in `docs/shadow_v6/<group>.md` (street, gangs, crime, corp, labour, vice, care). Every event is a seeded game abstraction.

## Verdict

**The people are alive; their trades mostly are not, and a camera would show it within an hour.** Every pick has a home, a walk, meals, an unwind and a shift, and the walking and hanging out read as a person (labour § 1). Some picks read as a life worth watching: the madam who joins a gang, is robbed at home, fights back and dies (vice, `s43/madam_24700` d34-35). Others read as trades on screen: the vendor, the street worker with a pimp, the dancer, the companion, the runner and the beggar who sleeps in a capsule bed.

The trade itself is mostly a single long action. Every Trade role works the generic `ClerkWork` (the Fab Tech its `FabWork`) as one block of 530-540 minutes on one tile. That covers 55 % of labour pick-days, and the sweeper stands for 530 minutes on one street tile. Where a trade should have a counterpart, the counterpart is a ledger line, not a body. 76 % of the people named in a settlement are Statistical. The super "calls on" 26 of 68 labour picks at a fixed minute while they are at work or asleep. Venue customers settle in one batch at 21:00. The arms sale books at midnight against a dealer who is asleep or jailed, in front of 21 witnesses.

V5's not-alive list is still not alive: the cab (0 fares in 7 of 7), the courier (0 parcels handed over in all 36 rounds), the bouncer (no door in 7 of 7), the joytoy (0 clients in 5 of 5), the leader (Collect "0 to 0 members" in 4 of 4) and the scav (no harvest in 8 of 8). The crime trades lost their on-screen beats: 0 of 5 guns hunt, 0 of 4 buyers enter a Fixer, and 1 of 4 pimps runs a round.

**The island does not hold 20k** (`isle_m_k4`, 30 days). Homelessness is 7,823 at d0 against the generated city's 800, and 8,032 at d29. Employment runs 0.22 → 0.29. 231 people are starving by d29 and 5,186 are sick at d30. The import has 93 Blocks per 1,000 residents against the generator's 200, which `IMPORT_GRAPHICS.md` already read as too coarse for 20,000.

**The first week is a cold city.** At day 0, 19 of 48 archetypes have no candidate: no gang of any kind, and no dealer, pimp, street worker, vendor, day labourer or office worker. The city starts with 800 homeless and 5,214 elders out of 20,000. Gangs form during the week. The 2k run books 390 marriages, 941 housings and 64 foundings in week 1. Night workers are cooled for 18 hours on d0 (`Earn unplannable: Unreachable`).

## The devices

- `shadow --assert`: **133 PASS, 0 FAIL**.
- `shadow --lives --population 20000` against Lives at work's closing 20k row: idle 3.59 → 3.95 h/day, spouse named 1.05 → 1.01 h/week, empty HangOuts 0.39 → 0.42, met 0.44 → 0.42.
- The camera, pooled over the four pinned windows (1,994 pick-days):
  - 10.2 plans and 1.7 aborts a day. 85 % of aborts are Interrupted.
  - 44 jumps: 35 are sentencing (`Full -> Coarse, sentenced`, 16-608 tiles), 4 are escorts, and 4 are CEO flyer arrivals at about 1.1× the allowance.
  - Labour picks show 0 jumps of their own: a labourer never teleports on his own feet.
- Unpinned picks: 8 of 12 were Statistical for all 3 days and stood on one snapshot tile for 72 hours.

## Before the camera: bugs ranked by what a followed character sees first

Ranked by how visible the bug is on screen × how many lives it distorts. Rows 1-7 are the proposed pre-view fix pass. Each fix needs a failing test first.

1. **The shift is one statue-still action** (labour bug 1, street bug 2; 61 of 68 labour diaries, 5 of 6 sweepers; **OPEN-137**). `goap/actions.rs`: `Role::Trade(_) => ActionKind::ClerkWork`, and `Sweep` is one stand (`exec/actions.rs:929`). The fix is a visible shift per trade (legs, rounds, crates, fares) built from existing actions where it can be. This is a feature, and the largest row.
2. **The silent idle freeze** (street bug 1: 14 of 33 street diaries, 29 days of 12 h or more, worst 40 h; **OPEN-138**). A goal (`Earn`) is chosen, no plan follows, no cooldown line is written, and then nothing happens for a day. Look first at the "could not plan" path in `goap/`/`plan.rs`.
3. **Settlement by link, not presence** (every group; OPEN-119 and OPEN-120, widened). The super's stop serves tenants who are elsewhere or asleep (`service.rs::super_stop`). The day cook's counter never opens (**OPEN-145**). Venue customers settle in a 21:00 batch. The arms sale books at midnight to the dealer's own gang with the witnesses chosen at the settle (`weapons.rs:699`). A Locate sighting is sold by a gang member asleep beside the target. The rule: an encounter posts when, and only when, both bodies are present, or else it is narrated as off screen.
4. **Long legs that freeze, then jump** (corp bug 1, crime bug 4, gangs bug 1; **OPEN-139**). These are single GoTo legs of 300-901 min at 20k: a CEO stands at home for 12 h on a 724-min "walk", and a leader walks 15 h through three districts and sleeps on a Spire pavement. Escorts cover 88-200 tiles in about 72 min. This is V5 bug 11 grown. 20k distances need re-planning legs and transport, not one timed step.
5. **Housing churn** (corp bugs 2-3, crime bug 3, care bug 1, gangs bug 6; **OPEN-140**). Two `Housed` lines in the same midnight undo each other (`ages.rs:478` and `life.rs:1359` against `life.rs:959`). "Moved nearer work" fires for the jobless and does not land near work. 5 of 14 20k execs, CEOs and guards never sleep at home. A day-0 retiree's `move_in` never sends the body home. "Hotel" resolves to a Soup Kitchen, or to `NoSuchPlace` 12-16 times.
6. **The dead trades** (V5's list; OPEN-115 cab, OPEN-118 leader, OPEN-121 bouncer). New rows: the courier's parcels (**OPEN-141**: `courier_stop` hands over none), the joytoy's clients, and guns and buyers holding contracts they never work. The last two are partly the pick tool's definition (**OPEN-148**).
7. **The implant purchase loop** (vice bug 2, 3 diaries; **OPEN-142**). `BuyAsset` waits 140 min, then fails `PreconditionLost` and retries at once. A dancer spends 14 h over three days in a Ripperdoc. Validate the sale before the wait.

After those, in order:
- the foreman hires a madam off her house (**OPEN-143**, related to OPEN-126);
- a pimp's desist roll ends his round (**OPEN-144**);
- the night guard's shift is `Unreachable` (**OPEN-147**);
- a released member attacks within the minute, and one fight fans out to 30 Witness lines (**OPEN-146**);
- bouts (OPEN-122), triple tips, the stationary super lobby wait, and identical days (OPEN-109's family).

## What the view must handle (requirements for its spec, not sim fixes)

- **Statistical bodies on screen** (**OPEN-149**). An unpinned agent at Statistical LOD has a snapshot position and stands on it for days, and 76 % of the counterparts named in settlements are Statistical. The view needs a rule for drawing them: Ambient puppets driven by the hour's schedule (addendum 13), or promote whatever is in view.
- **The arrest.** Sentencing writes the Precinct or Station position directly, 16-608 tiles away. The options are a "taken away" beat, a fade, or an escort walk in the sim.
- **The weekly rest day.** Every venue's staff is off on `day % 7 == 6` (52 of 52 vice diaries), so on that day the clubs stand empty.
- **The cold start** (**OPEN-150**, decision). A player on day 0 lands in the backlog week with no gangs. The options are to pre-roll the world (start the player at day N of a run), seed gangs and trades at creation, or both.
- **The island's density** (OPEN-136 widened). The 3D map cannot be the game city until it houses and employs 20k: a finer import (k < 4) or more flats a lot.

## V5's bugs, as read

- **Fixed:**
  - the Visit goal (OPEN-116: 0 lines, was 312);
  - the madam's cut (OPEN-123);
  - the street worker's client (1 of 3);
  - the shadow tool's labels (V5 bug 20), apart from the stims sale and dealer rows (vice bug 7).
- **Unchanged:**
  - the cab (OPEN-115);
  - leaders (OPEN-118, OPEN-128);
  - settlement (OPEN-119, OPEN-120), now seen in every group;
  - the Hq-less exec (OPEN-104);
  - the bouncer (OPEN-121);
  - bouts (OPEN-122);
  - the super with no round (V5 bug 14);
  - scavs (bug 19);
  - the homeless housed by the hire (OPEN-126).
- **Worse:** the 20k walk (V5 bug 11; see row 4 above) and the super's notice, which now repeats three days running.
- Each critique's § 2 has the evidence.
