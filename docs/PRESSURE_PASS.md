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
