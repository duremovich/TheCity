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

### Phase 6: a business under water (F3's hand-back)

A person-owned venue whose till cannot meet its wages today pays short until the staff walk out (152 unpaid
walkouts at bars in 30 days at 20k). Make failure a story with choices: the owner cuts hours or lets staff go,
borrows (a loan shark, a gang, a corp), sells (to a corp, or to a gang as a front that launders and recruits), or
closes and the building goes vacant for the next founder. A walked-out worker can come back to confront the owner
over the owed wages (a scene, a fight past a grudge threshold, or a report).

Shadow: an owner under water (cook, worker_friday as owner, a bar owner), their staff, a gang leader buying.

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

### Round 2 from Shadow V10 (`SHADOW_V10.md`, opened 2026-10-26)

Dylan's calls (2026-10-26): this round before the map re-import (`OPEN-220` waits on `OPEN-204`); fares get a
ladder, never a cell; gang attrition is judged after the stalls are fixed, not softened first; coders run in waves.
Wave 1 is R1-R4 at once; wave 2 (R5, R6a, R6b) starts once R2 has merged, since a stalled walk may be the pace bug.
Iterate at 20k (`--population 20000 --map assets/maps/isle_m_k2.txt`); the V10 repro picks are 100k seed 42 days
21-27 (`target/shadow_v10/`), read there only when 20k cannot show the case.

#### R1, petty law: the fare (`OPEN-213`)

Fare evasion is never a cell. A dodger caught on the El pays a fine on the spot if the wallet holds it; if not, the
El's guard (an on-duty guard or transit body at the stop; with none, skip the rung and say so) beats them: a wound,
a grudge, a memory. A repeat dodger inside a window is banned from the El and walks; a ban broken is caught again
and beaten. Jail is for what makes story (assault, theft past a threshold, the hit, the gang). Release puts the body
on the street by the cell, not a far one. The Statistical tier takes the same rule, coarse. Numbers in a new
`[fares]` section (or `[law]`'s neighbours). Shadow: scav, raider, pimp, jailed, guard. Moves a number: jailed mean
and arrests for fares, 20k seed 42 x 30, before and after.

#### R2, walking pace (`OPEN-215`)

Same-district walks run well under a tile a minute (`joytoy_103306 d23` 384 min for 149 tiles, `office_worker_14234`
383 for 176). Diagnose before fixing: path cost (detours, blocked or costed tiles), pace (the step's speed), or the
executor (stalls, re-plans, waits booked as walking). Fix the cause. Then: a guard's escort runs as legs, not one
935-min action (`guard_26314`); guard beats are dealt near the guard's home (F1's leftover); find why F3's "took a
post nearer home" never fires for a 997-min commute. The execs' far Hqs are the map (`OPEN-182`), not this phase.
Shadow: joytoy, office_worker, loader, guard, courier. Moves a number: minutes per tile walked (or the walk legs over
120 min), 20k seed 42, before and after.

#### R3, the wounded and the sick (`OPEN-216`)

A wounded body acts and is acted on: the bleeding seek a patch (clinic, black doc, a friend), and the people at hand
(a co-worker, a friend, a guard on the beat) help or carry them; nobody bleeds three hours with no goal
(`guard_8327 d26`). A diagnosed patient short of the fee has choices: care on credit (the clinic books a debt), a
pawn, the black doc's cheaper back room, begging for it (`sick_70955`, `sick_61720`). The treatment fee reaches the
doctor's or the clinic's till. A sick worker's post is covered (a co-worker's extra shift, a shape-up hire) or the
employer counts the absence as phase 1's do. A diagnosed founder keeps the treatment money out of a founding.
Sump back rooms see patients. Shadow: sick, guard, black_doc, orderly, ripperdoc, bouncer.

#### R4, the friendship flip and the diary's noise (`OPEN-218`, `OPEN-219`)

Find and fix why 51 friendships flip to enemy in one minute with no event line, followed by seven assaults on
strangers in two hours (`gun_28759 d21 14:46`, 100k seed 42), and the gossip line with winner and loser swapped.
Noise: the super's notices fold into one line a round with a count; zero-valence "called on" memories are not
written; phase 5a's "X saw Y watching" is dropped or folded; the shiftless rest day (weekday 6) is marked in the
diary; a Camp child's parent on a shift over the 10-20 window can answer, and a Punish or Hurt visit no longer
overwrites a Camp one in the single slot. Shadow: gun, super, a camp child's parent, and a 5a witness.

#### R5, gang verbs that stall (`OPEN-214`), wave 2

Raids that muster lose to Sleep (0 brawls in V10's gang diaries); JoinGang walks of 300+ min never arrive
(`jailed_93738` 11 goals, 0 joins); a try-out ends with no accepted or refused line; Scavenge loops a 1-min step
failing `PreconditionLost` (52, 0 finds); "unpaid: the gang is broke" every duty night. Then judge 5b's attrition
(91 colours lost in 30 days at 20k, mostly missed court summons): print it before and after with the reasons; the
duty rule changes only if it is still silly once the walks arrive. Shadow: raider, gang_member, gang_leader, scav,
jailed, homeless.

#### R6a, the trades' verbs I (`OPEN-217`), wave 2

On a followed body: the cabbie drives fares; the pimp collects; the fighter has bouts (and bettors); the madam and
the joytoy have bookings. Shadow: cabbie, pimp, fighter, madam, joytoy, street_worker, and a customer of each.

#### R6b, the trades' verbs II (`OPEN-217`, `OPEN-207`), wave 2

The gun hunts (a contract taken and walked); the teacher has pupils; the reporter writes a story from what happened
on the beat; the CEO's day is the Hq, not a market stall, and a landlord corp's exec has an office or a round
(`OPEN-207`); gangs buy guns from each other when their treasuries allow. Shadow: gun, teacher, reporter, ceo, exec,
arms_dealer, gang_leader.

Then Shadow V11 in V10's window (100k, seed 42, days 21-27, two per archetype), Sonnet critics, the synthesis.

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
### F2, the missing trades and the no-op verbs (2026-10-26)

Each at its cause. Numbers: 20k `isle_m_k2` seed 42 x 30 `--scaled`, main `28aba7b` then F2, one run each.

- **The trades at 100k.** `Config::fit_to_map` deals the roster `k` = 50 copies from the island's employers (600
  Farms, 150 Markets, 100 Offices), and every opening pass that builds on a Lot dealt 50 copies in a fixed order on
  the 1,415 Lots (8.8 x the reference 160): the Lots ran out at the Noodle Bars, and the Hq, Packing Plant, School,
  StripClub, Brothel, Boutique, Dollhouse, BdStudio and Gunsmith stood nowhere (no Office Worker, Teacher, Madam or
  Contract Host). The Lot-built passes now deal `World::lot_copies` (`ScaleCfg::lot_copies`, set by `fit_to_map`:
  the map's Lots over the reference's, at most `k`; 9 at 100k, 10 at 20k, 1 at 2k, so 2k and 20k are unchanged),
  and the per-capita venue, Hotel and Mission counts follow it: the reference city's mix, ~40 % of the Lots seeded
  and the rest left to grow on (100k: 965 of 1,415 Lots vacant after the seeding, was 0). **Deviation:** the 100k
  map has no Sump Lot at all (1,328 Mid, 87 Spire), so the FightPit (Sump only) never stood and no Fighter was
  hired: a seeded venue with no Lot of its tier refits a derelict Block of the tier, unpaid, as the run's refits
  do (`jobs::seed_derelict`). The Sump's Capsule Hotels still find no Lot at 100k (`OPEN-204`).
- **The Back-Room Doc** was never hired at 20k either: only a Harvest or typed gang with 700 coins and a vacant Lot
  on held ground opened a BlackClinic, and the typed (scav) gangs are memberless with 50 coins and hold nothing.
  JOBS_AND_LIVES § 7's other founder is added (`gray::doc_clinics`): a struck-off doc with the found cost opens one
  on the Lot nearest his Home. Struck-off docs stay rare (`p_struck_off`, 1 in 30 days at 20k), so the trade stays
  rare; no knob moved.
- **Preach** (`gang_member_18708`: 17 GangWork plans, 0 Preach): `busiest_spot` broke a 0-hanger tie by the lowest
  tile of every held district, a map corner 2-3 h away, and the walk was always cut by another goal. The preacher's
  own district first when held, ties by distance.
- **Extortion of 0 coins** is a `Refused` ("Block#N gave X nothing (nobody was home | nobody had a coin)"), no
  claim, no crime, the task done for the day, on screen and off; the off-screen shakedown now marks only the payers
  robbed (it marked every resident). "extorted 0 coins" 171 -> 0, 237 such `Refused` lines instead.
- **"Collected 0 to 0 members"** (`OPEN-118`, `OPEN-199`): an empty week is no Collect (`collect_pool`); 90 of
  100 Collected lines -> 0 of 16.
- **The law's noise.** A guard off shift files no petty offence (`law::on_duty`, `is_petty`: a fare dodge seen
  from a commuting guard's seat; a murder he still reports); a guard with the fare pays it (`pays_or_dodges`); a
  crime's noting witnesses are capped at `[crime] witness_cap` 4, the law's eye rolled first and always counted.
  Witness 15,304 -> 9,643, Report 3,475 -> 2,994, Arrest 2,957 -> 2,530. **Arrest's `PreconditionLost`** is the suspect gone from a sighting up to
  `suspect_seen_ticks` (240) old after a 15-45 min walk, the chase going on only within `sight + 2`: a guard at
  the last-seen tile now also chases on to another guard's sighting `[life] chase_radio_ticks` (30) fresh. 4,936
  -> 2,128 a month; the rest is the honest lost trail (what the guard does next is phase 5's).
- **Bury** (`worker_friday_93234`): the "Cemetery" is the Recycler (the map's one Cemetery) and also his workplace,
  so that resolution was right. The failures were a digger carrying one corpse into a Bury planned for another
  (BuryCorpse `PreconditionLost` 337 -> 0: an abandoned Bury puts the body down, and a Bury binds the corpse in hand
  first), and a second digger walking after a body in the first one's arms once the 2-h reservation lapsed (the
  Corpse reservation now lasts `plan_timeout_ticks`; CarryCorpse `PreconditionLost` 404 -> 24, Buried 855 -> 947). The 18.9 h Bury day is the distance (F1).
- **Tests** (no test added): three premises the moved city broke, re-pinned with the reason in place:
  `person.rs::probe_working_couple_moves_near_work` skips a venue's floor (the longest couple became two
  Companions at a full Lounge, the spouse laid off before the midnight), `housing.rs::test_company_rent_...` reads
  the owner's share with the tax its accumulator rolls over, `sickness.rs::test_g3_only_the_broke_steal_food` runs
  seed 410 (408 drew one broke theft in 20 days on `28aba7b`, none after; 409-412 draw 2-7).

### Phase 6: a business under water (`systems/underwater.rs`, `[underwater]`)

- **Under water** is the owner's purse under `short_cover_days` (1) of the day's wage bill (the staff outside his
  household), or a hand owed `short_unpaid_days` (2). Every person-owned standing business with staff to pay (not a
  Block, Lot, Hideout, Camp, Mission, Prison or Station) is read at the LivingEarly midnight's end (after the jobs'
  top-up and the visits due, before `demography` hires); after `decide_days` (2) short midnights the owner chooses,
  then no sooner than `decide_cooldown_days` (5). Both tiers decide the same way (the Statistical mirror: the decision,
  the loans, sales and closures are midnight passes; the two scenes are `Visit`s a Statistical visitor is credited at
  its hour).
- **The choice** is one weighted draw (a hash of the building and day): hang on `w_hold × (0.25 + pride)`; shorter
  hours, once, `w_hours × (0.5 + loyalty)` (every hand's shift trimmed off its end and wage cut by `hours_cut`); a
  layoff `w_layoff × (1.5 − loyalty)` (`layoff_share`, newest first, `LaidOff` with the owner as an actor and the till
  in the text); borrow `w_borrow × (0.5 + courage) × (1 − misery / 2)`; sell `w_sell × (0.5 + greed)` and close
  `w_close × (0.25 + misery)`, both `× (1 + escalate × earlier decisions)`. A cut sets a hiring freeze (the day's
  vacancies withdrawn) until the purse holds `recover_days` (3) of the bill ("afloat again").
- **Loans**: a lender lends what it can spare up to `loan_days` (14) of the bill, never under `loan_min_days` (3):
  the local gang (`gang_reserve` 20 kept back), the district's richest lawless free adult (a shark, `wealth / 2`), or
  for an owner at least `corp_lawful_min` lawful the richest corp over its reserve; picked on the owner's lawfulness.
  Weekly installments over `loan_weeks` at the lender's rate (`Flow::Finance`); a miss sends the gang's toughest free
  member (or the shark) as a `Visit` (`VisitWhy::Dun`): the arrears paid, or what there is taken and a beating
  (`law::resolve_fight_mods`, an Assault, the owner's grudge and report). At `seize_misses` (3) the gang or shark takes
  the business (`Repossessed`; a gang's is a front), a corp repossesses at `corp_repo_misses` (2) with no collector; a
  borrower who no longer owns it is written off. Nobody is dunned in the cells. Reused: the `Visit` machinery, the
  fight resolver, `corps::move_building`; not the M16b coercion record (its 128 cap and weekly-protection shape).
- **Sales**: a fire sale (`corps::acquire`, `Acquired`) at `sell_price_mult` of the value to the richest corp in the
  kind's niche, or (an owner under `front_lawful_max` lawfulness, or past `front_misery`) to the local gang at
  `front_price_mult` (0.25: muscle, not money). Deviation, "the local gang": the district's controller, then the gangs
  holding Blocks there, then the nearest Hideout that can pay (20k island seed 42: gang turf is 0-4 Blocks a gang by day
  11 and the holders' treasuries are 0c; the controller-only rule found no gang with coins and no gang loan or front
  in 30 days). A **front** (`World.underwater.fronts`; a leisure venue's `front_of` too, so the L2 round and dealers
  read it): its staff join the gang on `front_join_p × (1 − lawfulness)` a day; on its weekday `launder_share` of the
  gang's week's tribute is booked through the till ("washed"); the gang's rival hits it on `rival_hit_p` a week (up to
  `rival_take` from the gang, `Flow::Robbery`, shut `rival_close_days`). Deviation: the rival need not hold turf in the
  district (none did). Laundering is booking only (no clean/dirty coin exists).
- **Close**: every hand let go (wages owed become debts and stiffed workers), the doors shut (`closed_until` ten
  years), the vacancies withdrawn daily; it stands for sale at `shut_price_mult` of its value to a would-be founder
  (a free adult, jobless or on the dole, no gang, no exec, holding the price plus `buyer_reserve_days` of the kind's
  wages), at most `shut_buys_per_day`: `Acquired` and `Founded` "reopened ... bought it shut". Deviation: not a Lot (a
  building does not revert to an open Lot: walls and flow fields), so `founding::register` is untouched (F1's); a shut
  Bar or Hotel unsold `shut_derelict_days` (21) goes derelict (D25's path, refittable).
- **The stiffed worker** (F3's unpaid walkout, or a layoff owed wages: `World.underwater.stiffs`, the coins stay the
  edge debt): from `confront_wait_days` on `confront_p × (0.5 + pride + misery)` a day, at most `confront_max` (3)
  times, a `Visit` (`VisitWhy::Confront`, between `confront_hour_from..to`) to the owner wherever he is. The owner pays
  what he holds on `pay_base + pay_lawful × lawfulness + pay_loyal × loyalty − pay_greed × greed`; refused, the
  Betrayed grudge grows by `confront_grudge`, and at `fight_grudge` (0.5) he swings (an Assault; F3's 0.35 + one
  refusal is 0.48, so the second refusal fights); under it a worker at least `report_lawful` reports the owner to the
  Hall, which garnishes the owed from the owner's purse as it comes in (`Flow::Wage`). Nobody is confronted in jail.
- **Shadow** (`citysim-cli shadow`, new pick `owner_broke`: owners with staff, lowest till over a week's bill first).
- **Counters** (new `Jobs2Cols` `uw_*` columns, `analyze_run.py` "under water, run totals"), 20k island seed 42 x 30,
  one run: owner-days under water 346; decisions: hang on 11, shorter hours 21, layoffs 73 (hands let go), loans 14
  (missed installments 14, collectors' beatings 2, businesses seized 2, none repaid in 30 days), sold to a corp 11,
  to a gang as a front 3 (staff drawn in 12, laundered 1c, rival hits 0), shut 5 (all 5 reopened by a new owner),
  afloat again 17; unpaid walkouts 167 (F3's run: 369 unpaid quits); confrontations 79 (paid 14, fights 20, reports 22,
  garnishments 22). Murder 53, Assault 955, LaidOff 889 (all causes). FINDING: the gangs' treasuries are tens of
  coins, so a gang front is cheap and rare and its laundering is nearly nothing; nothing tuned.
- No test (doctrine 2).

### Phase 5a: consequence, the non-gang half (`systems/consequence.rs`, `[consequence]`)

Raid results, gang duty shifts and release-to-the-gang are phase 5b. Every pass is a mechanism with its numbers in
`[consequence]`; both tiers (the mirror sentence is the module's head: the demands and the services are `Visit`s, a
Statistical visitor credited at the hour; the hit's choice, the cells and the release are tier-blind; the off-screen
crime rolls have no witnesses, as before).

- **Witnesses react.** A civilian who notes a crime that is not petty (`law::raise_crime_mult`, after the Witness
  event; a guard on duty files as before) makes one weighted draw: report (`w_report x lawfulness`: the SawCrime
  memory and the walk to the Hall, unchanged), flee (`w_flee x (1 - courage)`, a violent crime: an Enemy edge and
  safety 0, so the Flee goal takes him), gossip (`w_gossip x sociability`, a crime that is a deed: up to
  `gossip_tell` of his Friend/Family/Spouse/Parent edges hear the rumour at hops 1 through `memory::hear_entry`, so
  grudges and the culprit's name move as any heard deed), or blackmail (`w_blackmail x (1 - lawfulness) x (0.5 +
  greed)`, only the lawless (`bm_lawful_max`) and broke (`bm_broke_coins`)). The culprit sees who saw him on
  `learn_p` (the pair's edge sours; an event in both diaries). Deviation: the city-wide Witness noise does not fall
  (the event is still written; F2's cap of 4 holds); the reaction rides on it. A gang-mate of the culprit does not
  react. A blackmailer's own SawCrime is skipped by ReportCrime (`consequence::keeps_quiet`).
- **Blackmail** (`World.consequence.blackmail`, at most `bm_max_open`): after `bm_wait_days` the witness goes to the
  culprit in person (`VisitWhy::Blackmail`, `bm_hour_from..to`) and names `bm_demand_share` of his purse
  (`bm_demand_min..max`). He pays on `bm_pay_base + bm_pay_fear x (1 - courage) + bm_pay_lawful x lawfulness -
  bm_pay_pride x pride` (`Flow::Bribe`; a Robbed grudge), the demand back in `bm_interval_days`, at most
  `bm_max_demands` ("keeps X's secret now"). Refused: he silences the witness on `bm_silence_w x (1 - lawfulness) x
  (0.5 + courage)` (a fight at `bm_kill_mult`, the Assault or Murder raised, the witness's grudge; a witness who
  wins goes to the law), else the witness reports him (`law::file_report`) and he holds a Betrayed grudge.
- **Hits escalate** (`contracts::set_status`, an organic Hit or Beat a person bought, Expired or Failed): repost
  (`hit_w_repost x (0.5 + grudge)`, at `hit_repost_mult` the old price at the same Fixer or the nearest, at most
  `hit_repost_max` a head, the deposit from his purse), go himself (`hit_w_self x courage x grudge`, the grudge at
  least `hit_self_min`: a Hunt adopted directly, the cooldown lifted, Statistical promoted to Coarse), or let it go
  (`hit_w_giveup x (1.5 - grudge)`, the grudge x `hit_giveup_decay`). `EventKind::HitEscalated` in his diary.
  Deviation: at 20k a person-bought hit that lapses is rare (2 a month; V9's 700 expired at 100k are mostly
  corps' tags, guards and locates, untouched).
- **Jail is a place.** At sentencing (`law::sentence`) the run starts and a civilian reporter is named (a Betrayed
  grudge at `snitch_grudge`; guards are not). Each midnight, per jail: a jailed worker's workday is an absence on
  phase 1's count (`misery::jail_absence`; dismissed at `fire_absences`: a sentence over three days still vacates the
  post at once, as before, so this is the short sentences); and per prisoner at most one of: a fight with a
  cellmate he dislikes (a grudge, a rival's colours, an edge under `cell_fight_affinity`) on `cell_fight_p x (0.5 +
  pride)`; a member cellmate's offer taken on `cell_recruit_p x (1 - lawfulness) x (0.5 + misery)` (`gang::enlist`,
  in the cells, not the stipend rule); a Fixer's name from a cellmate who owns, works at or is a regular of one
  (`cell_contact_p`: a regular there, so the hire pass offers him work); a loan from a cellmate (`cell_loan_*`, an
  edge debt). At most `cell_events_max` a jail a day. Release (`law::release`) writes the changed life: the post
  lost, the debt left, the grudge, the gang, the Fixer's name, the fights.
- **The faith on screen.** Each creed's place holds a service once a week, its weekday and hour hashed on the place
  (`svc_hour_from..to`): a cult's or the Purists' Hideout (the members) and a church's Mission (its creed's
  followers housed in its district; the eldest not called elsewhere preaches). The congregation (at most `svc_cap`,
  living within `svc_reach_ticks` of the place; the preacher always) is a `Visit` to the building `svc_lead_hours`
  before; a congregant brings an unchurched Friend/Family/Spouse on `svc_guest_p`. A body is credited on arrival
  (`consequence::arrivals`, every 10 ticks over the pending visits: the Visit's Rest was cut by Sleep before its
  end). An attendee hears the preacher (belonging `svc_gain`), puts `svc_tithe_share` (at most `svc_tithe_max`) on
  the plate (`Flow::Tithe`: the gang's treasury or the church's purse); a guest converts on `svc_convert_p x (0.5 +
  misery + (1 - belonging))` (a cult's or church's follower; the Purists' gang); a congregant walks out on
  `svc_walkout_p x (1.5 - loyalty) x (0.5 + misery)`: a cult member leaves the gang (the row's `apostasy_w` from the
  leader, `faith::on_leave`), a Purist takes the preacher's `svc_heretic_w`, a church's follower leaves with no
  grudge (its row has none): hunted past `[hunt] hunt_min` as any grudge. The midnight `faith::preach` roll stays
  (the off-screen tier's draw). Deviation: night-shift members (18:00-24:00 posts) never attend (the Visit is not
  taken on shift); many Purist chapels call only the preacher (few members within reach).
- **Shadow** pick `jailed` (an adult under a Sentence on the start day).
- **Counters** (`Jobs2Cols` `wr_*`, `bm_*`, `hit_*`, `snitch_grudges`, `released_changed`, `jail_*`, `cell_*`,
  `svc_*`; `analyze_run.py` "consequence, run totals"), 20k island seed 42 x 30 `--scaled`, one run: witness
  reactions report 1,016, flee 271, gossip 890, blackmail 316, the culprit saw the witness 781; blackmail demands 239
  (paid 87, 717c; silenced 117; reported 35); hits reposted 0, done himself 1, let go 1 (the run before the last
  config change: reposted 2); reporters' grudges 100, changed releases 417, jail absences 512 (93 dismissed), cell
  fights 70, recruits 55, a Fixer's name 187, loans 80; services 78 (attendances 1,194, guests 99, conversions 24,
  walkouts 16, tithe 1,430c), apostates hunted on day 29: 1. Murder 50, Assault 801, Report 2,820, Arrest 2,170,
  Sentence 2,105, Witness 10,451, Fire 569, GangJoin 685 (phase 6's run: Murder 53, Assault 955). FINDING: the jail
  absence dismisses ~90 a month at 20k, 88 of 93 after a 3-day Theft sentence (three workdays in the cells: the
  third absence fires at `fire_absences` 3); nothing tuned.
- No test (doctrine 2).

**F1: distance at 100k.** The premise did not hold: 100k does not break the island pass's distance. Before any change,
seed 42 days 0-8 (`run --legs`, every bodied leg): 4 h+ legs 7.3 % of all legs at 20k, 6.6 % at 100k; structural
commutes of 240 min or more 13.3 % at 20k and 7.0 % at 100k; V8's 20k diaries read 8.1-9.3 % of GoTo legs at 4 h+,
V9's 100k diaries 7.4 %. The critics read the island's tail, which the 20k pass left, at 100k for the first time. Its
causes, each fixed where the pick or the leg is made (no knob moved): (1) **the city's hires ignore distance**:
`reconcile_sanitation` hired the island's poorest for the one Recycler and `reconcile_guards` the island's most
lawful for any post; both now take those within `[distance] job_cap_ticks` (of the Recycler, of a city post) first.
(2) **District labels are space** (I9 again): a patrol's Market was drawn anywhere in
a district up to 1,000 tiles wide (`law::district_route`: within the cap of the guard, the Hall only within it of the
beat); a sweeper's beat stops were its district's dirtiest tiles anywhere (`jobs::sweep_route`: within `[distance.reach]
beat_ticks` of the shift's start first, a soft anchor), and a body sweeper with no stop to walk (the commute before
the round is planned, or a stand) walked to the beat district's dirtiest tile instead of the Recycler's yard
(`jobs::beat_tile`: the post's door). (3) **The pitch** was chosen in the district the
opener stood in at midnight (`streetwork::openings`: the free spot nearest the home in any district under its cap,
none past the job cap). (4) **FleeToHome never rode**: a Sleep's `[FleeToHome > Sleep]` walked 600-1,000 tiles
(`exec::start_step`: it rides the El and walks in segments like a GoTo). (5) **The exec's car**: an exec not housed
at `exec_home_tier` walked (the tier gate is gone), and the car, timed at its Coarse road share (0.88 min a tile
against the El's 0.25), is not taken where the El beats it (`streetwork::exec_ride`, `ride_home_ticks`). Execs still
live 300-900 tiles from a Hq outside the Spire: that is `OPEN-182`'s re-import. Deviation, foundings: the papers are
filed at the nearest of the Hall and the district Stations (`founding::registry`, a scripted `found_plan`; `[distance.
reach] register_ticks` reads that desk). Deviation, contracts: a hunt-run record (an agent target, no Guard, Escort or
Rescue) is offered only to a candidate within `stakeout_ticks` of the target's Home or workplace door
(`contracts::hunt_in_reach`, in `offer`; a gang's crew is not filtered). 20k seed 42 x 30, base (main 53cab2d) -> F1: foundings 17 -> 53,
contracts fulfilled 48 -> 67 (posted 599 -> 642). 100k seed 42 days 0-8 (`run --legs`, one run each): bodied legs of 4 h+ 6.6 % -> 5.3 %, of 2 h+ 17.7 % -> 15.2 %; structural commutes of 240 min+ 7.0 % -> 4.6 %; sweeper Beat legs of 4 h+ 189 -> 57. Guards still hold 78 % of the 4 h+ legs (Patrol 5,178 -> 4,664): a guard's beat is a whole district dealt by his Station's share, walked from his Home at the shift's start (`stations::deal_beats`), and the El refuses many 180-tile trips (`slow`: the island's lines run the long way round). Measurement only: `run --legs` prints `#commute_role` rows; the
diary's header has a Commute line (doors, tiles, travel) and a GoTo line its tiles. No test added.
### Phase 2: the dead trades get customers (`OPEN-176`, `OPEN-190`, `OPEN-120`, `OPEN-017`; `systems/customers.rs`, `[customers]`)

Each trade gets a body that walks to it, through the machinery it already had. Numbers: 20k `isle_m_k2` seed 42 x 30
`--scaled`, main `7823960` (before) then phase 2 (after), one run each.

- **The door** (`customers::door_cause`, read by `leisure::door_ok`): every venue whose door a fighting trade keeps
  (the Bouncer's Club, Den, FightPit, StripClub; `kept_kind` reads the trade rows) turns away, with a bouncer on
  duty, the drunk or high (`misery::influence`), a brawl's starter within `door_brawl_days`, a carried weapon
  (`door_armed`), a rival gang's colours (the venue's gang: its owner, else the holder of its Homes) and, at a Club or
  StripClub, rags (`door_dress_min`), on `door_p`. `door_seen` leaves the call to the door for every kept kind (it was
  the Club's alone). The refusal event names the cause; a drunk, high, armed or brawling refused swings at the
  bouncer on `door_swing_p × (1 − lawfulness)` (`customers::scene`: `law::resolve_fight_mods`, an Assault, "threw X
  out" or "put X down"), and every refused holds a grudge on him. Refusals 111 -> 343 (353 of 374 on an earlier build for drink), scenes 0 -> 83 (one ejection a month).
- **Clients** (`client_mult`, `lonely_need`): a venue with its tipped worker on duty (a companion's Lounge, a host's
  Dollhouse, a dancer's StripClub or Club, a Brothel) scores `× (1 + lonely_pull × lonely + misery_pull × misery)` on
  the Unwind ladder, and the lonely (intimacy under `lonely_intimacy`) with `client_coins` unwind for company with fun
  full (`unwind_satisfied`). The visit is the existing `Enjoy` and its encounter. **Deviation:** OPEN-190 was closed
  by the island F3 (the own-Home Unwind); the joytoy's missing clients were the ladder never weighting the
  Dollhouse, not a missing goal. The chip host remembers nothing under the chip (by design), so her `[[serves]]`
  row now writes the visit as an event (`event_on = ["done"]`). Lonely visits 4,833 (a new counter), companion visits 410 -> 427, Dollhouse visits 38 -> 74, Brothel 3,155 -> 3,393.
- **The pimp** (`OPEN-120`): a pimp's round ran after a cult's sermon (`preach_plan` first: a creed gang's pimp
  preached his pitch hours away), and the hour's pitch was the round's turn across the district (334-360 min legs).
  The round now comes first in GangWork and a pimp's body walks to the nearest pitch due (the run predates the last change: the pitch with a take owed first).
  Collections 41 -> 33 (the street workers are few: 128 street visits a month). The probe `probe_pimp_purse_grows_on_a_pitch_night` is the fix round's.
- **Arms**: the frightened want a gun (`customers::frightened`: safety under `fear_safety`, or a living Enemy edge
  held on them; `weapons::has_reason`); a gang under its armoury target names a quartermaster at midnight
  (`weapons::quartermasters`, a body first) who walks to another gang's dealer through the Shop goal
  (`assets::shop_choice`'s first offer) and buys with the gang's purse into its Hideout; the dealer and the buyer
  each remember the sale. **Deviation:** crates abroad stay (a gang with no other dealer to buy from still ships);
  the gang does not stop "paying itself", it buys from a rival first. Fear's wants 502 want-days, dealer sales 11 -> 12, gang
  buys 0 (the gangs short of stock hold 0-180c; a dealer's T1 is 90c).
- **Care-seeking** (`customers::care_choice`, `care_plan`, `ActionKind::Patch`): a lost fight within `hurt_hours`
  is a wound to have seen to: the Treat goal plans `[GoTo(Clinic) > Patch]` to the nearest open Clinic within reach
  (`patch_price`, the doctor's Patient encounter, "patched X up"), or, for the wanted, a gang member or the short, to
  the back room within `back_reach_ticks` (`back_price` from the patient's pool: wallet, kin, gang; the broke pay
  what they hold). A diagnosed cancer whose pool is short of the Clinic's treatment buys the back room's cut-rate
  remission (`back_onco_price`, `back_onco_mult`, no cure). **Not done:** begging is Earn's as before; pawning has
  no item sale to stand on (next step). Patched 57 (all at Clinics), back-room patients 0 (see below), Treated events 71 -> 124 Treated events.
- **The back room's founder** (`gray::back_rooms`, `OPEN-017`): while fewer stand than one per `back_room_adults`
  adults, a struck-off doc, else the least lawful licensed Ripperdoc under `rogue_lawful` (who quits his Clinic and
  is struck off), opens one: a vacant Lot he can pay for, else the derelict Block nearest his Home refitted unpaid
  (`founding::refit_with` takes a BlackClinic), and is hired as its doc. Back rooms 0 -> 5 (five rogue Ripperdocs, days 0-4, every one a derelict in the Sump).
- **The cab**: a body's GoTo of `hail_min_tiles` or more hails a cab once a day (`streetwork::hail_cab`, after the
  commute roll) when flush (`hail_rich_coins`) or hurried (late for a shift, Treat, Flee, Rescue), from the Garage
  whose driver's body can come soonest within the wait (else the nearest Garage's off-screen cab); the ride is the
  commuter's (`take_ride`: the pick-up and the drop-off legs). A cab driver's body is paid `min(1, fares /
  cab_fares_full)` of the day's wage (`customers::fare_scale`): no fare, no pay for Rest at the Garage (a
  Statistical driver keeps the wage). Hails 915, rides a body drove 44 -> 214 (cab trips 1,035 -> 1,825).
- **The shape-up**: every open Construction and Freight Yard posts `shape_up_posts` day posts at the dawn pick, sites
  or none (the corner's posts dried up with the sites), the broke jobless walk `shape_up_reach` tiles to a corner,
  and the foreman and the pick each remember the meeting. Day posts filled 228 -> 393 (offered 1,028 -> 2,250).
- **The vendor**: a working stall sells, each hour, up to `vendor_per_hour` meals to the hungry bodies on the street
  within `vendor_tiles` (`streetwork::vendor_passers`; the Buyer encounter both sides). Vendor meals to bodies 2,099 (vendor meals 10,918 -> 12,710).
- **The scav's day**: a scav gang's member off hours runs the gang's round of the bins (`customers::scav_plan`:
  GangWork `[GoTo(Bin) > Scavenge]` at the nearest Market's bin within `scav_reach`, the find named in an event), and
  walks `scav_reach` to a fresh body to strip and rip (`chrome::loot_target`). Scav rounds not counted (an event a round).
- **Reads wrong / open** (the next pass's input): no back-room patient in 30 days: every back room stands in a
  Sump derelict and the hurt who avoid a Clinic are elsewhere (300 travel minutes; distance is F1's); back-room docs
  quit unpaid (a room with no patients pays no wage; 10 walkouts). The picked cabbies drove no fare (body rides 214 a
  month city-wide; `fares_no_body` and `fares_too_far` still most of the hails). No gang bought from a rival's
  dealer. The scav's round ran hourly through the night with nothing found (now one round a day, the day's gang
  task; not re-shadowed). The four picked bouncers met no cause in their week though the city's doors turned 343 away.
- **Not done:** the dealer's on-screen buyers (the stims corner is as it was; next step), pawning, the super's
  round with tenant calls.
- **The Statistical mirror**: the door is `door_ok`, which the off-screen visit already meets; the gang's buy runs
  through `assets::stat_shop`'s weapon path; the shape-up's posts are world-side; the rest (clients' pull, care,
  hails, vendor passers, the scav's round) are bodies' choices, the off-screen city keeping its learned rates.
- No test (doctrine 2). `SAVE_VERSION` 69. Premises the moved city broke, re-pinned with the reason in place:
  `gray_faith::test_struck_off_leaves_the_post` (the struck-off doc may open a back room that night: off the
  licensed post, not jobless), `person::probe_cabbie_drives_a_fare_on_his_shift` (`p_hail` 0: the probe's rides are
  the roll's), `captives::test_weak_gang_under_retaliate_takes_a_hostage` (the rival leader's spouse was jailed by
  day 40: released in-test). `person::probe_pimp_purse_grows_on_a_pitch_night` stays red as on main (the fix round's).
### Phase 3: protection bites (`systems/protection.rs`, `[protection]`; `OPEN-160`, `OPEN-162`, `OPEN-157`)

- **Held ground** (`protection::ground`): the districts of a gang's turf Homes and (`hideout_ground`) its Hideout's
  district from day 0, so the pass runs at 2k, 20k and 100k alike. `leverage::protection_pass` (each gang on its weekday,
  as before) leans on up to `lean_per_district` (3) businesses per district of that ground under any order but LieLow
  (M16b: one, under Expand or Contest only, turf districts only). The levy is sized to the business: `levy_share` (0.15)
  of its week's revenue, `levy_min` (10) to `[leverage] protection_weekly` (40); a business under `levy_min` is left
  alone (M16b: 40 flat, nothing under it leaned on). The hold is the gang's (`protection_holds` reads the ground): the
  leaner jailed or dead no longer ends it.
- **Refusal bites** (`OPEN-160`): a lean the owner refuses is recorded standing (`Coercion.refusing`, `Refused` "X
  refused G's protection on B"), and a week missed (a round stop that met nobody, the till short, a refuser's week) is
  a miss (`Coercion.missed`; a short or refused week retried the next day). At `punish_misses` (2; a refusal at once)
  the gang's toughest free member off shift at the hour goes in person (`VisitWhy::Punish` to the business between
  `punish_hour_from..to`; a Statistical enforcer credited at the hour; one enforcer a business): first the till smashed
  or the stall kicked over (a Market or Noodle Bar; `smash_weeks` 2 of the levy from the payer's purse, the enforcer
  keeps `enforcer_cut`, `Flow::Robbery`, shut `smash_close_days`), second the owner beaten if he is there
  (`law::resolve_fight_mods`, an Assault or Murder; out, the till again), from the third a fire (shut
  `fire_close_days` 7). Each raises the crime, leaves the owner's grudge (`Robbed` or `Assaulted`) and a lawful owner's
  report. `EventKind::Punished`.
- **The owner's choice** after, one keyed draw: pay up now `w_pay × (1.5 − courage)` (refusing cleared), report the
  enforcer and the leaner `w_report × lawfulness` (refusing stays: the next week escalates), a rival's cover `w_rival ×
  (0.5 + courage)` (another gang with ground in the district and a free leader: the hold `Broken`, the rival's protection
  recorded at the same levy, the holder a `HomeFlippedAgainst` shock and its leader's grudge on the rival's), or sell or
  shut `w_sell × (0.25 + misery) × punishments` (`underwater::forced_out`: to the gang as a front at
  `front_price_mult` if it can spare it, else phase 6's close). **Deviation:** a rival gang's front leaned on and
  punished makes no choice: a `Trespass { by }` shock on the owning gang and its leader's grudge on the enforcer (the
  turf war's spark is the shock; the Retaliate order reads it).
- **The gang's purse.** The protection paid joins the week's tribute (`leisure::note_tribute`) the leader's Collect
  shares out, so a member's cut shows as a `TRIBUTE received` line at the Collect. Daily (`protection::daily`, after
  `gang::daily_economy`) a gang bails out its jailed (`bail_per_day` 5 a sentence day left, `bail_min_days` 2 to
  `bail_max_days` 20 left, the leader first, keeping `bail_reserve` 100; `Flow::Fine` to the city; `law::release`).
  **Deviation (the stipend):** the daily `[social] gang_stipend` goes only to members with no wage post
  (`stipend_jobless_only`) and only from the treasury above `stipend_reserve` (150): the stipend to every member spent
  every treasury to 0 (first run: median 0 from d18). A gang under its war chest pays nobody daily, and `[living]
  desist_unpaid` takes its unpaid members. Weapons, loans and fronts read the treasury as before.
- **`OPEN-157`, the Fixer's parties in person:** a contract's party the Fixer cannot meet now (no server on duty, the
  body elsewhere) is sent there at the next open hour today (`[protection] broker_visits`, `service::send_broker`, `VisitWhy::Broker`, the staff's
  first shift hour or later; a Statistical party credited at the hour), and met on arrival (`broker_arrived`); the
  encounter is missed only when no visit can be sent. The book credit between two Statistical parties stands.
- **Shadow** (20k island seed 42, d21-d27; picks gang_leader:3, gang_member:3, vendor:2, cook:2, owner_broke:3,
  fixer:2, then four punished owners by `--agent`): a round paid ("Ivo Stroud collected 40 coins of protection at
  Braindance Parlour#5786 from the till, Nico Greaves on shift"); a refusal punished ("Dex Stroud refused The Static
  Rats's protection on Club#4777" d25 00:09, "Perrin Chrome smashed the till at Club#4777 ... took 80c" 17:57) and the
  owner's choice ("Dex Stroud went to the law ... reported Perrin Chrome and Kira Stroud", then Kessler posts a guard
  on the Club and a locate and a tag on Kira Stroud); "Cutter Sato paid The Unplugged 5 40c after the stall kicked
  over at Street Market#4414"; a rival front's answer ("The Hollow will answer The Wire Hands for the smashed till at
  Fight Pit#5580"); bail (a first-build shadow: "The Drowned Choir paid 30c bail for Ivo Stroud (6 days left)", then "released from the Precinct"). Reads wrong: most
  punished businesses are rival gangs' Fight Pits and Dens (a gang leans on a rival's front and smashes 0c); a
  refuser can wait days for the enforcer (`Cutter Sato` refused d26, nobody came by d27); the member's weekly cut is
  1-13c; vendors and cooks work corp venues and see none of it.
- **Counters**, 20k island seed 42 x 30 `--scaled`, `bbb7ae2` then phase 3, one run each (new `Jobs2Cols` `pr_*`,
  `gang_treasury_*`, `broker_walks`; `analyze_run.py` "protection, run totals"): protections made 13 -> 75, weeks paid
  13 -> 110, protection coins 520 -> 4,172, standing at d30 13 -> 115, `Protected` 13 -> 100, Extortion 631 -> 558,
  Refused 745 -> 604 (72 of them refused leans), weeks missed 245, punishments 62 (till 58, beating 1, fire 3), choices
  pay 18, report 6, rival 34 (most a rival front's Trespass), sell 0; bail 67 (1,360c); gang income 18,111 -> 26,651,
  `flow_coerced` 2,230 -> 6,107; gang loans 17 -> 13, gang fronts bought 4 -> 5; members at d30 635 -> 787 (GangLeave
  109 -> 134); Assault 909 -> 1,027, Murder 50 -> 57; `contract_encounters` 78 -> 86 (31 broker walks).
  FINDING: gang treasuries at d30 median 11c, max 297c (start 50): the income rose half again and is spent (the
  Collect, bail, the fronts' payroll and the daily purchases; untraced which dominates), so lending and front buying
  did not move. Nothing tuned.
- No test (doctrine 2). `SAVE_VERSION` 70.

### Phase 5b: consequence, the gang half (`systems/crew.rs`, `[crew]`)

- **Raids write a result** (`crew::raid_result`, from `raid::brawl` and `raid::corp_brawl`): one `Raid` line with every
  raider and defender as an actor (the winner, the coins and food taken or the till smashed, the wounded and the dead
  by name, each gang's next order); the beaten hold a grudge on the other gang (`raid_grudge`), the losing leader on
  the winner (`raid_leader_grudge`); a repelled raid is answered (the defenders' `retaliate_until`/`retaliate_on`, a
  `Raided` shock, a rethink at once). The wounded carry the fight's `Lost` memory, so phase 2's care sends them to a
  Clinic or (a gang member avoids the Clinic) the back room: no new path. A marcher reaching the door after the brawl
  reads it (`late_to_raid`; V9's purist with a 16-min Brawl and no result). The rumour at the venue is the existing
  `gossip::post_raid`. Deviation: no result line for a breach (BreakOut).
- **Duty** (`crew::daily`, the LivingEarly midnight after `household`'s visits): the order posts today's duty: a raid
  order every free member to the muster (kept at the door, `raid::resolve`; a muster that never marched is no miss);
  Contest/Retaliate a guard at the Hideout (`guard_hour`); Expand and the street orders a watch at the nearest of the
  gang's protected businesses, fronts and Hideout (`watch_hour`); LieLow none; `duty_share` of the members, at most
  `duty_max`. A body leader summons `court_summons` members to court by name. Each is a `Visit` (`VisitWhy::Crew`),
  set out by the trip's estimate, none past `duty_reach_ticks` from Home (most members live far from their Hideout on
  the island: the gate keeps V9's 4-h walks out). A kept watch or guard pays `duty_pay` (`Flow::Tribute`, over
  `pay_reserve`, less the bail owed). A member whose shift falls on the duty or the walk chooses (a keyed draw,
  `choose_*` on loyalty, misery, rank, an unpaid gang): the gang strikes the shift (`misery::skip_for`: the absence
  counts toward phase 1's firing), the shift is a missed duty. Misses take standing; `colours_misses` in
  `miss_window_days` send the gang's toughest (`VisitWhy::Reckoning`, a person visit; met at the next midnight if the
  walk never came off) to beat him out (`law::resolve_fight_mods`, `gang::leave`). Deviation: the leader's round takes
  no member along (the round stays the leader's).
- **Standing** is a ladder term (`gang::leader_scores` + `standing_w x standing`): kept duty, missed duty, time served.
- **Court's quorum** (`crew::court_held`, from `leaders::court_done`): fewer than `court_quorum` of the summoned there
  and the court fails with each name's reason (at work, in the cells, did not come); `fold_fails` in
  `fold_window_days` and the gang takes the offer of its rival or the nearest bigger creedless gang (members and
  treasury over, `absorb_offer` to the leader) on `w_absorb x (1.5 - pride)`, or folds (the treasury shared, the
  members walk) on `w_fold x (0.5 + pride)`. Deviation: a creed gang is never absorbed (it folds; `faith::on_leave`
  takes "folded" as no apostasy).
- **Release and return**: `gang::on_member_arrested` notes the sentence (`crew::on_arrested`); daily a jailed member
  with `talk_min_days` or more left talks once on `talk_p x (1 - loyalty)`: a report on a gang-mate (the leader first)
  for Extortion with himself as the witness. On release (`gang::on_member_released`): one who talked (his flag, any
  report he filed on a gang-mate since the sentence began, or phase 5a's hook `crew::note_talked`) gets the
  reckoning; the rest walk to the Hideout (a `Crew` visit `welcome_after_hours` later) and are welcomed by the crew
  there by name, the bail the gang paid (`protection::daily` -> `note_bail`) a debt kept from duty pay,
  `served_standing` for a sentence served (the ladder: "made lieutenant"). A night in the cells is no homecoming.
- **Rest days**: a free member with no shift today goes to the crew on `crew_p` at `crew_hour`, at the first within
  reach of the gang's fronts (Expand and the street orders), the Hideout and the crew's local (the Bar of his home
  district nearest the Hideout); none under LieLow. The line names who else is there (bodies, or the members whose
  own visit there was kept).
- **The treasury's sink, traced** (new `CoinProbe.gang_*`; `run --report` prints `#gang_flow` and `#gang_book` lines
  to stderr, diagnostic): 20k island seed 42 x 30 on main, the gangs took in 16,451c and paid out 15,751c, 5,884c of
  it in gear (an Arms implant for the strongest member every day the treasury touched `[chrome] gang_chrome_floor`
  80, a bike at `gang_buy_floor`, a deck), then the Collect and stipends 2,040, fronts bought 1,647, bribes 1,350,
  front wages 930, loans 692, bail 480. The fix is a budget, not a floor: gear only from what is left over the gang's
  commitments (`crew::commitments`: a week of its duty slots' pay, two days of its fronts' payroll, `[protection]
  bail_reserve`), one piece a week (`crew::gear_spare`, read by `chrome::gang_arms`, `vehicles::gang_bikes`,
  `virt::gang_deck`). After (the same seed, one run): in 26,421c, out 20,408c; gear 948c; the coins go to fronts
  (bought 2,806c, built 750c; front acquisitions, foundings and repossessions 7 -> 12 events), front wages 2,428c,
  the Collect, stipends and duty pay 3,046c, bail 1,770c, loans 1,077c (14 gang loans; 11 on main, 692c). FINDING:
  the median stays low because a third of the gangs earn almost nothing (end books 0-60c: no protection, no turf, a
  Squat or Job order); the earners hold 150-730c.
- **Counters** (new `Jobs2Cols` `crew_*`, `analyze_run.py` "crew, run totals"), 20k island seed 42 x 30 `--scaled`,
  one run: raid results 4 (wounded named 5; raids called off at the door 16); duties posted 1,886, kept 790, missed
  668; the shift chosen 438 times, the gang 291; duty pay 663c; colours lost 91; talked inside 28, talkers beaten out
  13; welcomed back 65, promoted 19; courts with a quorum 93, failed 33; gangs absorbed 0, folded 2 (an earlier
  build's run: absorbed 2, folded 3); rest-day nights with the crew 260; gang treasuries at d30 median 75, max 1,022
  (main: 48, 235).
- **Shadow** (20k island seed 42 d21-d27, picks gang_leader:3, gang_member:4, purist:2, runner:2, dealer:2, raider:4,
  then raider:12; the shadow tool gains a `raider` pick: a member of a gang under a marching order): "Volt Tanaka
  stood guard at Hideout#4463 for The Hollow with Zoe Santoro, Beatrix Tanner, Tank Quarry, Vale Sato (paid 0c, 3c
  kept against his bail)"; "Roderick Reyes welcomed back to The Ash Rats at Hideout#4478 by Casimir Singh, Vale
  Vasquez, Dorian Singh, Quinn Nightingale; made lieutenant"; "Kestrel Wozniak's court at Hideout#4480 failed: nobody
  came (summoned Tobin Sallow (at work); 3 failed in 7 days)", then "The Iron Crows could not muster and took The
  Static Rats's offer: 4 members and 66c went over, 20c to Kestrel Wozniak"; "Taz Northcott talked in the cells: named
  Lavinia Bartlett of The Drowned Choir" (d22), released d26, "is out of The Drowned Choir (talked to the law: thrown
  out, beaten out by Matthias Hallow)"; "Jinx Penhallow chose the shift as Recycler Tech over The Drowned Choir 8's
  court summons" twice, then "is out of The Drowned Choir 8 (lost the colours: 3 duties missed, beaten out by Odile
  Nakamura)"; "Casimir Singh spent a day off with the crew at Hideout#4478: Rue Zhou, Corwin Brandt, Jasper
  Coldwater there". No pick raided in the window (two raid plans, one a 417-min walk to the door); the raid result
  from an earlier build's counter run (the same code but the hash purposes): "Raid on Nutrix 2's Street Market#4427: The Neon Wolves won (5 raiders, 1
  defenders); the till smashed, 500c and 274 food taken; wounded: Rex Carrow, Mags Sallow; The Neon Wolves next:
  Harvest". Reads wrong (next pass): every corp raid in 30 days hits the same Street Market#4427 and takes the 500c
  cap; raids mostly die on the walk (16 called off to 4 fought); the colours go fast (91 in 30 days: three chosen
  shifts in ten days is enough, and the daily court summons is most of the misses); a reckoning sent at midnight
  rarely walks (met at the next midnight, "caught up with"); a summoned member who arrives after court ends reads
  "answered"; members far from the Hideout get no duty at all (the reach gate), so a gang spread over the island is a
  gang on paper.
- `person::probe_evicted_worker_rehouses_on_the_first_rent` goes red with `[crew]` on (green with it off): no crew
  line touches the Farmer; another city around her, she holds 22c at d5 against the 21c deposit, then spends 9c on an
  Unwind and 9c on a Capsule Hotel night and is on 0c at the rehouse day. A probe whose purse premise a butterfly
  breaks (as its Shop cooldown already guards); left for the fix round.
- No test (doctrine 2). `SAVE_VERSION` 71.

### R1, petty law: the fare (`OPEN-213`; `systems/fares.rs`, `[fares]`)

- **The catch** (`law::raise_crime_mult`): the on-duty guard who notices a jumped gate settles it on the spot
  (`fares::caught`), not with a report; `law::file_report` files no Fare Evasion while `[fares]` is set, so no fare is
  a warrant, an arrest or a cell (a guard on the take still lets it go). The El's crews (`TransitState.staff`) are a
  sweeper headcount, not bodies at the stops, so the catcher is always a guard (city or private, on shift, in sight
  of the stop): the beating rung is never skipped on screen, and a stop with no guard on duty catches nobody.
- **The ladder**: the fine (`fine` 5, `Flow::Fine` to the city, a `Paid` memory) if the wallet holds it, and the
  fined dodger rides on; else the guard beats him: down on the platform `beat_down_minutes` (30; `cuffed_by` himself,
  `fares::run` stands him up: "picked himself up off the platform"), a `Lost` memory of the guard (the street hears
  "assaulted by"), a grudge on the guard (`beat_grudge` 0.4: an Enemy edge), `beat_mood`/`beat_safety` off. Deviation:
  the wound is the half hour down, not a `World::wounded` entry (a stable entry's `rise` rolls `[ages] p_maimed` 0.25,
  a maiming for a 1c fare). Caught `ban_after` 3 times in `ban_window_days` 7: banned `ban_days` 14
  (`transit::pays_or_dodges` refuses the ride: he walks; `board` turns a rider banned since his plan off the
  platform). A banned rider under `ban_break_lawfulness` 0.1 boards anyway; caught, he is beaten whatever his wallet
  and banned afresh. Added: `wary_hours` 24, a dodger caught that recently jumps no gate (pays or walks); the first
  build beat one broke woman three times in 100 minutes at one stop on her way to a HangOut and banned her by 17:24.
  The phase-3 deterrence (`dodge_deterred_days`) reads a catch as it read an arrest.
- **Off screen** (`transit::stat_commute`, `stat_errand`): a jumped leg caught at `StatTable.p_dodge_caught` takes
  the same ladder coarse (`fares::stat_caught`: the fine, else the beating's memory and mood, no body and no guard;
  counts toward the ban), and the body no longer comes on screen for an arrest. `calibrate` counts a guard's catch
  (`EventKind::Fare` with the guard as an actor) as the jumped gate caught, so the next table measures the ladder.
- **Release** already puts the body on the street at the cell's door (`release_at_jail_door`, `release`: the
  Precinct's street in D2); V10's "far street" was the Precinct's district, far from a dodger's home, and fares no
  longer go there. No change. Not built: "theft past a threshold" (a Theft report carries no value; the petty food
  theft is now the jail loop: 738 -> 826 Theft sentences).
- **Numbers** (20k island seed 42 x 30, one run each, base `49098c9`): fare arrests 531 (409 sentenced, 122 fined at
  the Precinct) -> 0; arrests 1,448 -> 999; jailed mean 110.2 -> 110.2 (max 210 -> 199; the fare nights were ~10 h
  each, lost in the run's noise: an earlier build without `wary_hours` read 102.9). The ladder: 8,294 caught (1,589
  on screen, 6,705 off), 391 fined, 7,661 beaten, 546 banned, 242 bans broken. FINDING: the fine is rare by the dodge
  rule's construction (a rider with the fare jumps only under `dodge_always` 0.1); `p_dodge_caught` (0.116) was
  calibrated on sentences and wants the next `calibrate`.
- **Shadow** (20k island seed 42 d14-d21; picks scav, raider, pimp, jailed, guard and the tool's new `dodger` pick, a
  repeat dodger at the start day; diaries gain a `Fares` header line): "Hana Oakes fined Knox Yates 5 coins at the El
  in D1 The Sump for Fare Evasion" and two days later "Hana Oakes beat Quinn Abernathy at the El in D1 The Sump for
  Fare Evasion: no fare, no fine, left on the platform 30 min" (the guard's diary and the scav's: Enemy edge, grudge,
  "picked herself up off the platform", then back to work); "Winifred Mbeki beat Gideon Pembroke at the El in D1 The
  Sump ... / Gideon Pembroke banned from the El for 14 days: caught at the gate 3 times in 7 days", then a 621-min
  walk to his Club shift, and d21 "Osric Achebe caught Gideon Pembroke on the El in D1 The Sump inside the ban and
  beat him for Fare Evasion; banned 14 days afresh". Repeat dodgers with coins now pay (10 fares in 5 days, no
  sighting). Jail picks are Assault, Smuggling, Grand Theft; no pick was jailed for a fare. Reads wrong (next pass):
  a banned worker's walk (301 tiles, 621 min: R2's pace) costs the shift, so the ban is a firing; the beaten seek a
  patch and fail `Patch` `PreconditionLost` at a Ripperdoc targeting a Back Room (R3's); the petty-theft jail loop
  (3 days for 2 food) is the fare loop's successor; a private Security Office guard polices the El like the watch.
- No test added (doctrine 2; count 1470). Three `transit` tests of the off-screen catch now read the ladder (a fare
  record, no body for an arrest; `test_caught_commuter_leaves_the_statistical_hour` is `..._keeps_...`: the hour runs
  on), `law::test_bug4_petty_offence_fined_when_the_suspect_can_pay` runs without `[fares]` (the Precinct path), and
  `policy::test_dole_enacted_on_day_10_is_paid_from_the_treasury` lost its pick to a butterfly (every jobless adult a
  tenant who had drawn): today's draw forgotten and the rent from income off for the measured draw. `SAVE_VERSION` 72.
