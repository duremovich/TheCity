# The pressure pass (opened 2026-10-26)

The first pass under the reset doctrine (CLAUDE.md § The doctrine). Its purpose is mechanism, not balance: a city
where misery, debt, fear and turf push people into the choices that make story (skipping work, stealing, fighting,
quitting, joining a gang, finding God, paying a joytoy, buying a gun), and where the trades that serve those
choices have customers. Dylan 2026-10-26: "move forward with making the simulation more expansive and interesting,
even if it is at the expense of balance. Move fast and break things. We can tune later."

Every phase is proven by a shadow read (the archetypes named per phase), the change tier once, and no new test
unless a phase names one. Numbers that move are printed (before and after, one run each, seed 42), never gated.
Shadow V9 (100k, `target/shadow_v9/`) runs beside phase 1; what it finds joins this list as fix phases.

## Phases

### Phase 1: misery acts (`OPEN-178`)

A miserable person behaves differently from a content one, every day, on screen. Mood (`mood.rs`, -1..1) already
exists; today it moves only Socialise/Court (`utility/goals.rs` `mood_mult`), emigration, riots and class unrest.
Add, each a mechanism with its numbers in a new `[misery]` config section:

1. **Absenteeism.** Below a mood threshold a worker may skip a shift (sleeps in, drinks, wanders, stays with kin);
   the chance grows with misery and falls with conscientiousness/lawfulness. The employer counts absences; past N
   in a window the worker is fired (an event and a memory the diary shows). A skipped shift pays no wage.
2. **Petty crime.** Misery raises the weight of the theft and mugging paths of `Earn` (and shoplifting where it
   exists), on top of poverty and lawfulness: a miserable, broke, low-lawfulness person steals sooner.
3. **Fights.** Misery plus drink or a dose, in a venue or the street, with a rival, a grudge target or a stranger
   who slights them, starts a brawl (`Fight`). A brawl leaves grudges, so it feeds the revenge chains (`Hunt`).
4. **Quitting.** Sustained misery at a job (days below the threshold while employed; long commute, low pay, a
   company bed) makes a worker quit; a company bed goes with the job. The quitter is jobless and in the pick pool.
5. **Recruitment.** Gangs (`JoinGang`) and the faith (conversion, `faith.rs`/`creeds.rs`) recruit the miserable
   more easily: a mood term in the join and conversion odds.
6. **The other side.** A good mood shows too: courtship and generosity (a gift or a round bought for a friend) a
   little more often. Optional if the coder runs out of room.

Statistical tier: the same rules, coarse (one daily roll), so the off-screen city carries the same pressure.

Shadow: the lowest-mood adults (a `miserable` pick in `citysim-cli shadow`, the tool may gain it), worker,
office_worker, loader, fabber, gang_member, cultist, homeless, fighter. The diaries should show a skipped shift, a
firing, a theft by a miserable worker, a bar brawl with a grudge after it, a quit, a recruitment that names misery.

### Phase 2: the dead trades get customers (`OPEN-176`, `OPEN-190`)

Something sends bodies to the trades that exist but serve nobody:
- the bouncer refuses at Pits and Dens too, and refuses for cause (drunk, a brawler from phase 1, a rival's colours);
- the joytoy gets clients: the lonely (low intimacy) with coins, through `Unwind` (`OPEN-190`);
- the pimp collects from bodies on screen;
- the arms dealer sells: gangs buy from dealers instead of paying themselves, the frightened (low safety) buy a gun;
- the black doc treats the wounded who avoid a clinic (the wanted, the gang-hurt, the broke);
- the scav's harvest is seen from the scav's side.

Shadow: bouncer, joytoy, pimp, arms_dealer, black_doc, scav, and a customer of each.

### Phase 3: protection bites (`OPEN-160`, `OPEN-162`, `OPEN-157`)

Protection is keyed to held ground, not the gang's weekday order, so it runs at 20k and 100k. An owner who refuses
or misses rounds is punished in person (the till smashed, the owner beaten, the stall kicked over), and the
punishment leaves grudges and a choice: pay, report, or go to a rival. The Fixer's parties come in person in open
hours.

Shadow: gang_leader, gang_member, vendor, cook, fixer, and a shop owner under protection.

### Phase 4: reach and the social graph (`OPEN-194`, `OPEN-210`)

A reach table per purpose (clinic, report a crime, register a company, stakeout, sanitation beat) in config; past
it the goal drops unless none is left. Weak acquaintances prune sooner and each person's graph has a cap.

### Fix phases from Shadow V9

Added here as the critics report, ranked by what the player sees.

## Implemented: deviations

(Per phase, as they land.)
