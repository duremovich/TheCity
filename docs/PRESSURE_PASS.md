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
- the scav's harvest is seen from the scav's side, and the scav has a day (bins, a body, a find);
- (V9) the cab gets fares from bodies; the jobless get a dawn shape-up at a foreman's yard; the dealer and the
  vendor get on-screen customers;
- (V9) care-seeking: a diagnosed or hurt person plans a clinic trip, or chooses the black doc, begging or pawning
  when broke; patients walk into a clinic queue; the super walks a round with tenant calls.

Shadow: bouncer, joytoy, pimp, arms_dealer, black_doc, scav, cabbie, day_labourer, sick, ripperdoc, super, and a
customer of each.

### Phase 3: protection bites (`OPEN-160`, `OPEN-162`, `OPEN-157`)

Protection is keyed to held ground, not the gang's weekday order, so it runs at 20k and 100k. An owner who refuses
or misses rounds is punished in person (the till smashed, the owner beaten, the stall kicked over), and the
punishment leaves grudges and a choice: pay, report, or go to a rival. The Fixer's parties come in person in open
hours.

Shadow: gang_leader, gang_member, vendor, cook, fixer, and a shop owner under protection.

### Phase 4: reach and the social graph (`OPEN-194`, `OPEN-210`)

A reach table per purpose (clinic, report a crime, register a company, stakeout, sanitation beat) in config; past
it the goal drops unless none is left. Weak acquaintances prune sooner and each person's graph has a cap.

### Phase 5: consequence (Shadow V9's dull spots)

What happens next, so a started story is picked up:
- witnesses react (report, flee, gossip, or blackmail the culprit), and the city-wide Witness noise falls with it;
- a raid writes a result (wounded, a smashed till, stolen stock, a grudge between crews, a rumour at the venue);
- an expired hit is reposted higher, or the buyer goes himself;
- jail has cellmates and contacts; release returns a member to the gang (welcome, or a beating for talking); jail
  absences cost the job;
- a gang order puts a duty shift on the member's schedule with pay, competing with the day job;
- the faith on screen: a creed's service at a Chapel at a set hour, a congregation, a tithe, conversion or walking
  out, the leaver hunted as a heretic.

Shadow: gang_member, gang_leader, cultist, purist, buyer, gun, guard, and a jailed pick.

### Fix phases from Shadow V9 (`SHADOW_V9.md`)

- **F1, distance at 100k** (after phase 4): legs of 4-11 h, same-district El rides of 4-9 h, the builder's site
  day, a vendor's pitch or a sweeper's post districts away, food 5 h from a rich CEO. The island pass's distance
  held at 20k; find why 100k breaks it and fix the cause.
- **F2, the missing trades and the no-op verbs**: Office Worker, Teacher, Madam, Back-Room Doc, Contract Host and
  Fighter are never hired at 100k; GangWork's Preach never executes; 0-coin extortion logged as success;
  "Collected 0 to 0" (`OPEN-118`); a guard arrested for the fare evasion he logs, 13 of 16 Arrest plans
  `PreconditionLost`; Bury's Cemetery resolving to a workplace.
- **F3, quits and wages** (after phase 1): "quit unpaid" with money in hand, owners quitting their own venue,
  partial wages, same-minute Quit and Hire.

## Implemented: deviations

(Per phase, as they land.)

**Phase 4: reach and the social graph (`OPEN-194`, `OPEN-210`).** `[distance.reach]` (`systems/distance.rs`
`reach`, `out_of_reach`): the reach is measured in travel ticks (`distance::travel`, the walk or the El) from where
the body sets out, as `[distance]`. ReportCrime carries a "hall within reach" gate; Found and Treat are skipped
(`already_satisfied`) with the Hall or the nearest Clinic past reach (a diagnosed cancer is walked to at any
distance: the 2k probe `probe_diagnosed_patient_with_coins_seeks_the_clinic`); a Hunt's or a hit Contract's first
`GoTo(Intel)` (`hunt::within_reach`, read by `contracts::plan`): an ask venue past reach becomes the hunter's own
district's busiest Bar, a stakeout past it is not planned and the goal cools; a sweeper's beat
(`districts::sanitation`) is dealt within reach of the post (home district first, then the nearest district with a
slot), else the post's own district, and `District.sweepers` counts the dealt beats. Deviation: "unless none is
left" reads per purpose: a purpose with alternatives takes the nearest within reach; a discretionary trip with none
within reach is dropped, not walked (the one Hall, the nearest Clinic). `[social]` `weak_affinity`,
`weak_prune_days` (7, the base's 30), `acquaintance_cap` (40): the daily pass drops a person's weakest, then
stalest Acquaintances past the cap; a debt's edge is neither pruned nor counted (the base pruned a weak one with a
debt). 20k island seed 42 x 30 (before = this build with the reach table out and the base prune): edges at d30
663k -> 426k, Friend edges 133k -> 132k, foundings 55 -> 17, contracts fulfilled 71 -> 45, Reports 3,475 -> 3,489,
Treated 43 -> 54 (that run gated the cancer too), ticks/s days 25-30 182 -> 187 (both runs and a shadow on the box
at once; a print). `sickness::test_g3_only_the_broke_steal_food` pools seeds 408-410 (seed 408 alone held one broke
theft in 20 days; the new prune moved it to none).

### Phase 1: misery acts (`OPEN-178`, `systems/misery.rs`, `[misery]`)

- **Misery** is `clamp((mood_line − mood) / mood_span, 0, 1)` (-0.15, 0.5): the city's mean mood is about +0.05 and
  7-13 % of adults sit under the line on 20k island seed 42 day 21 (the lowest about -0.85), so a span to -1 left
  misery near 0 for everyone and nothing fired; a misery of 1 is mood -0.65.
- **Absenteeism** is a one-person strike (`classes::walk_out`), decided once at the LivingEarly midnight (after the
  no-shows) for the next shift of a holder not on shift: the shift is marked worked and struck, so neither tier works
  or is paid, and the day is off hours. The skipper's day is what the brain makes of it (bed, the street, a gang, a
  theft); the spec's "sleeps in, drinks, wanders, stays with kin" is not forced. Personality has no conscientiousness:
  loyalty stands in beside lawfulness. The owner of the workplace skips but is never fired.
- **Firing** at `fire_absences` (3) in `absence_window_days` (14): a `Fire` event, a `Sacked` memory (new
  `MemoryKind`), `life::note_quit`'s rehire block. The base no-show rule (`fixes::daily`) is untouched (a struck shift
  is no missed workday).
- **Quitting**: consecutive low midnights (`Misery.low_days`, a new component), `quit_days` less the commute, pay and
  company-bed terms, then `quit_p` a day; `economy::quit_job` with the reason in the event. The company bed goes with
  the job through the existing `company::evictions` (noted, evicted after the grace).
- **Petty crime**: the steal costs (food and the vehicle) fall by `steal_cost_cut × misery` and the vehicle theft's
  lawfulness gate widens; off screen `p_steal × (1 + stat_steal_mult × misery)`. No new shoplifting path (the items'
  pickpocket rides the food theft, `weapons::market_petty`). A theft by a miserable thief names it in the event.
- **Brawls** are not the Fight goal (it needs a wronging enemy): an hourly pass over bodies (Full, Coarse) past
  `brawl_min`, in a Bar or a leisure venue (drinking there) or on the street after a drink or a dose, resolved at once by
  `law::resolve_fight_mods` at `brawl_kill_mult`, an Assault (or a Murder) raised, the bouncer's ejection, and grudges
  (`grudges::add`, Assaulted) that make Enemy edges for Fight and Hunt. Off screen: a daily roll against
  `lod::stranger_in_zone`. A drink is a `Socialised` memory naming nobody at the drink's weight (the gang's welcome
  is not one).
- **Recruitment**: JoinGang `+ join_flat × misery`; the member edge needs `join_affinity_cut × misery` less affinity;
  the miserable and lawless are taken off the street as the desperate are; off screen there was no gang join at all,
  so `stat_join_p × misery` a day is new. The faith's Preach draws and converts the miserable. Joins and conversions
  past `join_note_min` name misery in the event.
- **The other side** (good mood, item 6) is not done: room spent on the five.
- **Counters** (new `Jobs2Cols` columns, `analyze_run.py` "misery, run totals"): 20k island seed 42 x 30, before ->
  after: skips 0 -> 1,312, misery firings 63, misery quits 42, brawls 119 (57 on screen; 119 grudges formed, 2 Hunts
  and 3 later assaults between a brawl's pair), misery gang joins 368, misery conversions 165; Murder events 58 -> 57,
  Assault 859 -> 1,070, Theft 855 -> 1,052, arrests 2,957 -> 2,989, GangJoin 813 -> 966, gang members day 29 350 ->
  432, followers day 29 4,616 -> 4,671, employed mean 11,226 -> 11,256 (the fired and the quitters are rehired from the
  pool), mean mood 0.043 -> 0.042. FINDING: assaults +25 %, thefts +23 %, gang size +23 %; nothing tuned. (The after
  run predates the drink test's weight filter, the last change: a street brawl after a gang's welcome no longer counts
  as drunk, so on-screen street brawls run a little fewer.)
- No test (doctrine 2).

### F3: quits, wages and the jobless (`economy.rs`, `misery.rs`, `[misery]` F3 keys)

- **Why wages came short.** A person-owned venue pays every wage out of its owner's own purse; a small bar's 2c drinks
  do not cover 4-5c a head, the owner runs dry and every worker (the owner too, paying himself) is short. The forger's
  case was different: a walk-in's 40c paid the forger 32c and left the owner's 8c cut *held by the forger* until the
  owner collected in person (`World::fixer_owed`, `service::collected`), which a Statistical owner never does, so the
  owner could not cover a 5c wage while his forger sat on 348c. Fixed: on payday the server hands the cut he holds to
  the owner (`economy::settle_held_cut`) before the wage is paid. The worker_friday "owner flip" in V9 is the diary's
  `place owner` column (where the pick stands: his Habitat-owned Block vs his Bar), not an ownership change.
- **An owner does not quit his own business.** `economy::household_business` (his or his spouse's): no wage drawn from
  himself (`collect_wage` clears the days owed), a spouse's purse pays what it has with no `Unpaid`, `maybe_quit` and
  the misery quit skip it. Not done: an owner under water cutting hours, selling or closing (the owner still bleeds
  his purse into staff wages and misses rent: an open item for the corp/ownership pass).
- **An unpaid quit is a story**: `Quit` reads "X walked out on <boss> at <place> as <role> (unpaid, owed Nc)" with the
  boss and the place as actors; a boss who is a person owes the wages as an edge debt (`social::repay_debts` takes it
  from his income; the aged debt is called in) and the quitter holds a `Betrayed` grudge (`stiff_grudge` 0.35).
  Deviation: no confrontation scene and no report; at 0.35 the grudge rarely reaches the Fight threshold.
- **The jobless are a state** (`misery::note_left`, `hire_blocked`/`pool_out`/`fired_by`, read by `life::quit_blocks`,
  the `HirePool` walk and members, and the city sweeper and guard reconciles): a misery or unpaid quit is out of every
  hire for `quit_idle_days` (2), a misery or no-show firing for `fired_idle_days` (5), both + up to `idle_jitter_days`
  (3); the firing employer refuses him for `fired_block_days` (120); for `stain_days` (30) a hire pass passes the fired
  over on `stain_fired_p` (0.6) a day, a quitter on `stain_quit_p` (0.3). The commute switch is one move: "quit as X
  (far commute: took a post at Y nearer home)" (still a Quit and a Hire in the same minute; no idle days).
- **The skip is a moment**: rolled at midnight, acted out at the first hour within `skip_lead_ticks` (60) of the shift
  (Statistical: at midnight as before): "skips the shift as Cab Driver: sets off for it, then does not go in" / "wakes
  for it and stays in bed" / "gets up and does not go; heads for Bar#n" / "goes round to a friend's at Block#n"; Unwind
  scores `+ skip_unwind_flat` (0.35) until the skipped shift's end.
- **A recruit sticks**: no walk-away (`gang::desist`) in a member's first `recruit_tenure_days` (14), every recruit.
  `tests/desist.rs` setup advances past it (no new test).
- **Counters**, 20k island seed 42 x 30 (`--scaled --ambient-flats 50`), phase 1's after -> F3: employed mean 11,256 ->
  11,176; misery firings 63 -> 49, misery quits 42 -> 34, skips 1,312 -> 1,208, brawls 119 -> 97; GangLeave 512 -> 115
  (all "walked away"), GangJoin 966 -> 818, gang members day 29 432 -> 681. Quits 683 (unpaid 369, far commute 211,
  misery 34, no need of the wage 33), Fire 483. Rehire gap (next Hire of the same name): misery firing median 6.3 days
  (37 of 49 not rehired by day 30), misery quit 3.0 days (18 of 34 not), unpaid walkout 4.8 days (138 of 369 not), no-show
  firing 1.0 day with 17 of 55 within the minute (that run predates the no-show firing's `note_left`). FINDING: gang
  size +58 % with the leaves gone; nothing tuned.
