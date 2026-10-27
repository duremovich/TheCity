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

### Round 3 from Shadow V11 (`SHADOW_V11.md`, opened 2026-10-27)

Dylan's calls (2026-10-27): round 3 before any map work (the "re-import" is really a ground-site shortage: 1,415 of
8,000 Lots at 100k, which only a footprint redesign or `OPEN-187`'s tenancies fix); school-age children seeded as
their own phase (`OPEN-224`); 100k at ~25 ticks/s is accepted (`OPEN-226`); the walk's pace is left alone (R2's ~20x
slow walk waits for the view). Wave 1 is S1, S3, S5, S6 at once; wave 2 (S2, S4, S7) starts once S1 has merged,
since a plan that fails after an 8-hour walk and a clinic that sees no patient may both be distance. Iterate at 20k
(`--population 20000 --map assets/maps/isle_m_k2.txt`, `run` also `--scaled --ambient-flats 50`); V11's repro picks
are 100k seed 42 days 21-27 (`target/shadow_v11/`), read there only when 20k cannot show the case.

#### S1, distance in the choice (`OPEN-221`'s sim half)

Goals that pick a place pick it by reach: the day labourer's site, the Deal spot, the muster point and the raid
target, the hire (a job taken across the island), the Camp answer, the JoinGang Hideout. A distance (or travel-time)
term enters each pick, and a body does not start a walk it cannot finish inside the plan limit: it picks the next
nearer option or a different goal, and says so. The map's shortage is not this phase. Shadow: day_labourer, forger,
night_shift, miserable, dealer, camp_parent. Moves a number: legs cut at the plan limit (and walk legs over 120 min),
20k seed 42, before and after.

#### S2, plans that fail at the end (`OPEN-222`), wave 2

Collect and Court end in a one-minute `PreconditionLost` after an 8-11 h walk (`gang_leader_96308 d21 08:43`,
`d26 19:24`): find which precondition goes stale on the walk and either re-check it before setting out, hold it on
the way, or have the body do the next sensible thing at the door. Hunt flickers "could not plan or plan failed twice"
every two hours (`buyer_82590 d27`; `vendor_19534` 57 Hunt goals before one plan): a goal that cannot plan backs off
or drops, it does not re-roll. A raider who sat the muster is not marked "missed" (`raider_39544 d22 00:07`).
Shadow: gang_leader, buyer, vendor, raider, gun.

#### S3, justice that reads right (`OPEN-223`)

A victim is not booked for the fight they lost: a sleeper attacked three times and arrested for Assault
(`miserable_29154 d21 01:27-04:26`) is the attribution being wrong; the attackers are named in the victim's diary.
Jail keeps the body alive: meals and sleep in the cell (`jailed_75054` a week at energy 0.00, starving from d26), and
the cell is not a meeting hall (`arms_dealer_65421` 1,171 Acquaintance lines in a jailed day). The fare ladder's
beating has a cool-down per dodger (`dodger_29048` beaten daily). A guard on patrol who witnesses a crime acts on it
(132 witnessed, 0 arrests: `guard_8360`); jail duty is legs or rounds, not one 9-hour action. Shadow: miserable,
jailed, arms_dealer, dodger, guard.

#### S4, care at 100k (`OPEN-216`), wave 2

R3's chain read right at 20k and starves at 100k: no `Treated:` in two ripperdocs' and two orderlies' six work days.
Diagnose the 100k case first (who gets sick, where they ask, why no patient reaches a clinic; distance after S1 is a
suspect), then fix the cause. The sick ask more than once and ask their own (spouse, Friends) before the street
(`sick_91688 d26 00:00`: one refused ask, then nothing). A super with no Block in arrears does a round (repairs,
complaints, the stairs), not one 500-minute ClerkWork (`super_104162 d21 09:02`). Shadow: sick, ripperdoc, orderly,
black_doc, super; read at 100k for the clinic case.

#### S5, the trades still thin (`OPEN-217`)

Bouncers refuse at the door and eject the rough (0 in 12 nights). A day labourer hired works the site, not Scavenge
(`day_labourer_75212 d27 06:00`; the site's distance is S1's). The cabbie's `Paid` with no coin is a bug: find where
the fare goes (`cabbie_82150` 0c all week); resting 9.8 h at the Garage is not work. Reporters print a Story at 100k
(R6b's beat story fires at 20k: find what 100k starves). A hit on a CEO is heard of: the target or the CEO's security
learns, and the diary says so (`ceo_88522 d24 18:00`). Shadow: bouncer, day_labourer, cabbie, reporter, ceo, gun.

#### S6, school-age children (`OPEN-224`)

The seeded city has no school-age children: every School's roster is 0, the teachers' days are empty. Seed children
in the households at world generation (an age spread the config sets, households built with them: rent, food and
care counted), gate School by age in `ages`, and the teacher's day has pupils. The daily "pupil's parent" line with
no pupil goes. Changes household make-up citywide, so print employment, starvation, homeless and the housing split,
20k seed 42 x 30, before and after. Shadow: teacher, camp_parent, a school child, a parent who works, elder.

**The rule (Dylan 2026-10-27, hard, not a switch): no child under 18 is ever killed, hurt or harmed; at 18 all bets
are off.** One predicate (`ages::is_protected`) skips them at every victim pick and harm site; no vice, no job, no
bet under 18; hunger and sickness floor before any damage (an unfed child is taken by the service, never starved);
the camps work children 10+ for Parts, feed them, and the warden's beating is deleted; a child caught stealing or
begging is taken by the service or let go, never arrested with force.

#### S7, the diary's noise (`OPEN-225`), wave 2

A raid minute writes one Raid line ("Raid on X: won/lost", the toll) and folds its Witness, Assault and relationship
lines into counts (`purist_56353 d22 00:06`: 260 Witness, 10 Assaults, 256 more changes, no result line). A witness
line does not repeat at the same minute daily (`gang_leader_18246`, the fare). No "promoted to X" when already X.
The rest day is marked in the hour lines, not only the summary. Shadow: purist, raider, gang_leader, reporter, a
worker on day 27.

Then Shadow V12 in V11's window (100k, seed 42, days 21-27, two per archetype), Sonnet critics, the synthesis.

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

### R4: the friendship flip and the diary's noise (`OPEN-218`, `OPEN-219`; `[noise]`)

- **The flip's cause** (`gun_28759 d21 14:46`, 100k seed 42): `gang::claim`, every blow of a gang claim on a Block
  in another gang's territory (an Extort, or the Squat order's `Occupy` via `squat_claim`), ran
  `make_enemy(m, actor, -0.7)` for **every member** of the holding gang, wherever they were. Edges are one per pair
  and `make_enemy` caps affinity at the Enemy threshold, so a +1.00 Friend in the rival gang read "Enemy, -0.60": her
  Occupy at Block#725 ended 14:46 and 51 of her friends flipped that minute, with no line. The Fight goal then scored
  "enemy near" on the rival members at the Block, hence the seven Attacks there. Now `gang::contested`: only the
  holders who **see** the blow turn (a body in the Block, or within `[crime] sight_day_crime`/`sight_night_crime` of
  the actor, as a crime's witnesses), and the edges made are told: `Crew` `[actor, holder gang, home]`, "X struck at
  H's hold on Block#N for G: 4 of H saw it and count X an enemy now (A, B, C and 1 more)" (`[noise] contest_names`
  3). The other bulk `make_enemy` paths already carry a line (`Betrayal`, `Split`, the succession war, 30 pairs at
  most). The flip did not recur at 20k (no blow on a rival hold in 10 gang and gun picks over the window).
- **The swapped gossip** (`super_101327 d27 10:04`): `memory::deed_of` read every `Lost` as "Assaulted by the
  winner on the holder", so an attacker who lost told people the defender had assaulted him. `law::fight_inner`
  writes the attacker's `Lost` with `object` = himself (`World::remember_entry`: `remember`'s body, the entry built
  by the caller) and `deed_of` reads that as no deed (no gossip, no grudge from it). The memory itself (the care and
  revenge readers, by kind) is unchanged.
- **The super's notices** fold into one `Notice` a round, at its end (`service::notices_folded`, from
  `trades::round_plan` and `lod`'s Statistical round; a round cut short is told at midnight): "W served notice on 4
  tenants in arrears on the round of 4 Blocks (A, B, C and 1 more)" (`[noise] notice_names` 3), every tenant noticed
  an actor so his diary keeps it. `World.notice_fold` holds the open rounds; `analyze_run.py`'s Notice count is now
  per round. **Zero-valence calls:** a `Tenant` encounter at the row's 0.0 valence writes no `Served` memory either
  side (the notice's own -0.3 memory on the tenant stays).
- **"X saw Y watching"** (phase 5a's culprit-learns roll) has no line: the soured edge (-0.1) and `wr_learned` carry it.
- **The rest day** is marked in the diary's day summary: "DAY SUMMARY d27 (rest day: no shift)" for a job with no
  shift that day (weekday 6, a guard's staggered day), "(the city's rest day)" for the jobless on weekday 6.
- **The Camp answer.** The week's answer takes the single visit slot before a Hurt or Elder visit (it comes once a
  week on a hashed day and was lost for the week). Its hour moves off the parent's shift (`household::camp_hour`):
  the hashed hour stands when free, else the first free hour after it up to `[noise] camp_visit_last_hour` 22, else
  the first free one from `visit_hour_from`. **Deviation:** a Punish visit could not overwrite a Camp one (midnight's
  `visits_due` sets the Camp slot and `protection::enforcer` already skips anyone holding a visit); nothing changed
  there. The shadow tool gains a `camp_parent` archetype (a parent of a child in a standing Camp). At 20k both picks
  are jobless and set out at 12:08 and 14:24 on a 1,150-tile, 600-min walk to the one Camp that Sleep cuts at 22:00:
  the answer still fails there, on the walk (R2's pace, `OPEN-215`), not on the slot or the shift.
- No test (doctrine 2). `SAVE_VERSION` 73 (72 at the branch; R1 took 72 first).

### R3, the wounded and the sick (`OPEN-216`; `systems/care.rs`, `[care]`)

Numbers: 20k `isle_m_k2` seed 42 x 30, one run each, this build with `[care]` out (before) and in (after). The runs
are deterministic on this box (two before runs matched), so the deltas are the phase's, butterflies included.

- **The fee reaches the doctor** (`care::pay_fee`): the doctor on the table takes `doctor_cut` (0.25) of a Clinic's
  fee (a patch, the Clinic's treatment) into his wallet, the rest to the owner's till as before; the event says so
  ("patched Gideon Crane up at Ripperdoc#4864 for 12c (3c of it to the doctor)"). Doctors' cut 0 -> 88c a month.
- **The short have choices** (`customers::care_choice`, `care::settle`): a diagnosed patient short of the Clinic's
  treatment (240c) settles it at the nearest open Clinic (any distance) with the pool, then a pawn (an owned deck,
  weapon or vehicle, not chrome, financed or stolen, handed to the doctor for `pawn_mult` of its value), then the
  doctor's slate for the rest (`slate_down` down, marked up `slate_rate`, while owing the Clinic's doctors under
  `slate_cap`); the slate needs a standing post's wage unless the pawn covers it. The slate is an edge debt to the
  Clinic's Ripperdoc (an orderly patches, the Ripperdoc carries the book): repaid from the wallet as any debt
  (`social` repay) and, past `[hitmarket] debt_min`, a lawless doctor's Debt Beat. A hurt worker short of a patch
  goes on the slate the same way. Without a wage the patient begs the warmest Friend (affinity >= `ask_affinity`,
  holding over `ask_keep`) at midnight once in `ask_days`: he lends on `affinity x (1 - greed/2)` (an edge debt) or
  turns him down; kin already pay through the pool. Then the back room's cut rate (phase 2). Slates 0 -> 1 (199c),
  asks 0 -> 90 (62 lent, 944c; 28 refused), pawns 0 (one in the shadow), Cured 0 -> 1. **Deviation:** the slate is
  the doctor's book, not the owner's: the owner is paid the down and loses the rest (no new ledger; the edge debt
  and the Debt hit market were there). Begging is a Friend's loan; the street bowl stays Earn's.
- **Back rooms see patients** (`exec` `LocationKey::Clinic`): a care trip or a Rescue bound to a BlackClinic
  resolved its GoTo to the nearest Clinic and the Patch there failed `PreconditionLost` (phase 2's "no back-room
  patient in 30 days"; R1's fare-beaten hit the same). The bound back room is walked to now (ungated: a bug fix).
  The Treat goal's "nearest Clinic past reach" skip no longer cuts a care trip (`care_choice` reads its own reach:
  the back room's 300 minutes). A back room's doc is on call when no server is on duty. Back-room patients 0 -> 21,
  patched 26 -> 61, Treated events 31 -> 64.
- **The downed act and are acted on** (`wounded.rs`, `care::call_for_help`, `wake_bystanders`, `bystander`): every
  `call_every_min` while nobody has come the downed calls the warmest tied body (kin, Friend, gang-mate, colleague)
  who can reach it and a doctor in time; the one called comes from any distance (an event: "Kenji Bartlett,
  bleeding, called Cato Marlow for help"). A city guard on shift within `rescue_radius` (the beat) and a stranger
  within `help_radius` (term `stranger_term x (sociability + lawfulness)/2`, not its attacker, no grudge) carry
  anyone down. The street hears it: the `help_wake` nearest off-screen adults within `help_radius` get a body and
  keep one while it bleeds (`lod::class_with`; at 20k the downed lay among Statistical agents, 0 bodies within 12
  tiles in 4 of 5 cases). The first helper binds the wound as he lifts it (`aid_min` once: the nearest doctor was
  150-193 tiles off against a 180-minute bleed). The nearest staffed doctor is a Clinic or a back room
  (`back_stabilise_price`). Calls 31, woken 136, bound 21, carried in time 0 -> 6, bled out 54 -> 44.
- **The sick worker's post** (`care::covers`, `hourly`): at midnight each sick worker's standing post (any tier,
  not his own business) is covered by the nearest jobless lawful adult of the employer's district, hired for the
  day on the sick worker's shifts (`Job.until`, ended hourly), when the owner holds `cover_purse_days` of the wage;
  else the absence is written ("off sick from X (n sick days in 14); nobody covers the Guard's post"). The sick
  day's count and firing stay health's (`sick_fire_days`). **Deviation:** no co-worker's extra shift (one shift a
  day per key in the routine; a second would need the shift model changed). Covered 441, uncovered 41.
- **The founder** (`founding::can_found`, `register`, incorporation): a diagnosed founder's coins count less the
  Clinic's treatment (`care::founder_reserve`), and the incorporation keeps it out of the capital.
- **Reads wrong / open:** a carrier who bound and lifted a body can drop it (`Marek Abernathy`: bound 21851, called
  again 21957, bled out 22047): the Rescue plan is interrupted and nobody picks it up in time; carries are slow
  (R2's pace). Strangers carried 0 at Stabilise (the bound were the tied, woken or called). A slate repayment is a
  bare wallet move (the diary's money line shows "-10c (Wage)"), no flow or event. The jobless old with no Friend
  (`sick_17778`, 71 y, 8c, diagnosed 0.38) have nothing to try: no slate, no back room, no ask; they Earn and climb.
  Starvation (an island FINDING, both runs) 1,415 -> 1,448; violence 112 -> 105.
- **The Statistical mirror:** the ask and the covers are world-side (any tier); the slate, the pawn and the call are
  a body's; an off-screen diagnosed patient treats on `p_stat_treat` as before (with the doctor's cut when a body
  is on duty). No test (doctrine 2). `SAVE_VERSION` 74 (72 at the branch; R1 and R4 took 72 and 73). Re-pinned with `[care]` off and the reason in place:
  `wounded::test_bleed_out_is_a_murder` ("left alone": the woken street binds and carries), and
  `scavs::test_scav_prey_ranked_past_cover` (the covers move the seed-42 city off the test's ground; a butterfly).

### R2, walking pace (`OPEN-215`; `[exec] move_ticks_full`, `stations::deal_beats`, `law::district_route`, `vehicles::road_ticks`)

**Diagnosis: the pace, not the path or the executor.** 20k island seed 42 x 30 (`run --scaled --ambient-flats 50 --legs
--legs-min 30`): walk legs ran at 2.06 min a Manhattan tile (Full 2.24, Coarse 1.99), i.e. `[exec] move_ticks_full` 2 (v1's toy-map pace) plus a
road detour of ~10 % for a Full walker; no stalls, no re-plans booked as walking (a third of the long legs end
`aborted`: interrupts, not waits). On the island's 4 m tiles (`isle_m_k2`, `s = 2 m x k`) that is 2 m a minute. The
fix: `move_ticks_full` 2 -> 1, the fastest the executor walks without sub-tick steps (a Full walker steps at most once
a tick). Every tick-denominated cap (`[distance]`, `[distance.reach]`, `[commute_pressure]`) keeps its time and now
admits twice the tiles; the tile-denominated reaches (`haul_reach`, `round_reach_tiles`, `leg_replan_tiles`) keep their
tiles and halve in time (their comments say so); the vehicles' `mult` scales with the pace, the El's absolute
`ride_ticks_per_tile` does not (the El is 4x the walk, was 8x: fewer rides). Deviation: the El was not rescaled, and
the walk is still ~20x slower than a person on 4 m tiles (faster needs sub-tick steps and an El and vehicle rescale:
Dylan's call).
- **The escort** (`guard_26314`'s 935 min): the van is one timed vehicle leg, timed over the road's length at the
  Car's Coarse blend (`road_share` 0.7: 30 % off-road at walking speed), which a road-measured way does not have.
  `vehicles::road_ticks` times the escort van and the 04:00 transfer at the road column. Deviation: not split into
  legs. A segment shows no new diary line (a segmented GoTo is one ACTION line) and the van would re-spawn per
  segment; the long escorts are Station cells full (`stations_full` 612 of 2,626 escort legs at 20k) sending the guard
  to the Precinct across the island: capacity, the next pass's. The van drives at a Full Car's road pace (`step_q`'s
  quarters, at most `max_steps_per_tick` tiles a tick: the shadow's and the stations tests' no-jump bound).
- **Guard beats near home** (F1's leftover): `deal_beats` deals both rounds by the guard's Home (a district's
  nearness: its nearest Market door, else Home door, else centroid), pairs nearest first, the rest only within
  `[distance] job_cap_ticks` walked of the Home; a guard left over takes his nearest district (a far district's slots
  stay empty; `law_brain::allocate` counts the beats dealt). `district_route`: a district whose Markets all lie past
  the cap walks its Homes nearest the guard (the Spire's one Market drew Mid-Levels guards 340-550 tiles); new
  `[distance.reach] patrol_ticks` (60) caps the loop: the Market within it of the guard, the Bar and the Hall within it
  of the Market, the Homes within it of the anchor (the Bar nearest a Mid-Levels Market stood 117 tiles off).
- **F3's "took a post nearer home"** is not a bug: it fires (20k scaled: 244 -> 200 switches in 30 days). A temporary
  probe of `commute::nearer_job` over 30 days (an unscaled 20k run): 5,079 in-cap candidates rejected for the margin (commutes of 81-120 against
  posts 86-120 away), 517 the own employer, 13 blocked; every far loader, builder, driver and courier search found
  0-1 open posts of its role within `job_cap_ticks` of home (the switch is same-role only). The 100k loader's 997-min
  commute is that: no loader post within reach of his Home. Execs (no `Job`; their Hq) build pressure and drop out of
  the seek: correct. Next pass: a far commuter takes any post he qualifies for (`OPEN-215`).
- **Numbers**, 20k seed 42 x 30 (`--scaled --ambient-flats 50`), one run each, before -> after: min a tile walked
  2.06 -> 1.06 (Full 2.24 -> 1.16, Coarse 1.99 -> 1.02); walk legs of 120 min+ 22,968 -> 20,817 (of 103,983 -> 127,469
  walk legs of 30 min+: fewer ride the El), of 240 min+ 7,529 -> 5,454; Full walk legs of 120 min+ 8,072 -> 4,893; El
  legs of 120 min+ 27,844 -> 15,821; escort legs of 120 min+ 1,670 -> 193 (mean 189 -> 72 min); Patrol legs of 120 min+
  (all modes) 12,183 -> 5,609. Printed: employed d30 11,971 -> 11,591; jailed d30 218 -> 450; arrests 2,243 -> 2,712;
  starvation 0 -> 0; cab fares 20,544 -> 32,591 (`fares_el_missed`: the El shortens fewer commutes against the halved
  walk, so more take the cab); Full El rides 25,264 -> 13,129; Home moves 2,199 -> 2,169. The long walks left are
  placement (Sleep/Home 4,137, Work 1,924: far Homes and posts), JoinGang's Hideout walks (1,850, R5's) and Patrol
  (3,329).
- Re-pinned (the pace moved their premise; reasons in place): `person::el_commuter` sets `[jobs2] p_cab` 0 (the pick
  rolled a cab before the El was asked); the far-Farmer, trade, Home-growth and married-pick probes pin
  `move_ticks_full` 2 (their city's far workers are near or married at 1); the laid-off loader probe makes the loader's
  the Yard's one open post. Shadow tool: the van's jump allowance reads the road column. No test. `SAVE_VERSION`
  unchanged (no saved state moved).

### S5, the trades still thin (`OPEN-217`; `[verbs]` S5 keys, `streetwork.rs`, `customers.rs`, `news.rs`, `contracts.rs`)

Numbers: 20k `isle_m_k2` seed 42 x 30 (`--scaled --ambient-flats 50`), one run each, main `7bbf9b8` -> this branch;
100k (`isle_m_k2_100k`, `--scaled`) seed 42 x 8 for the reporter. No number is claimed; these are prints.

- **The cabbie's "Paid" with no coin (the bug).** The fare went to the Garage's owner (`pay_ride`), and a body
  driver's wage was R6a's `fare_scale`: the day's wage x fares / `cab_fares_full`, so a fare-less shift owed 0c,
  `collect_wage_scaled` paid 0c and wrote `Paid` anyway (`cabbie_82150`: Paid daily, 0c all week); a shift cut while
  he drove a fare (`d24`) never reached `end_shift` and the one fare paid nothing. Now `collect_wage_scaled` writes
  `Paid` only when something was owed; with `[verbs] cab_fare_cut` (0.5) the driver takes his cut of each fare in hand
  at the fare (`streetwork::driver_cut`, `Flow::CabFare` from the Garage's owner: "+3c (CabFare) ... from corp
  Zetatech 8"), the cut is his pay (`fare_scale` gives `Some(0.0)`), and the shift's end tells the shift ("drove 2
  fares for Garage#4791 this shift, 4c his cut" 308 times, "stood by all shift ... no fare, no pay" 213;
  `Jobs2State.fare_cuts`). **The rank:** standing by he no longer rests inside the Garage: `fare_plan` sends him to a
  rank (`rank_tile`: a door of `cab_rank_kinds` within `cab_rank_tiles` 60 of the Garage, hashed on the driver and the
  `cab_rank_ticks` 120 window), a `Rest` on the street there (`exec::start_step` admits it: `at_rank`), driven at the
  cab's speed (`fare_leg`), and when the window turns he cruises to the next (`rank_wait_over`). Cab rides a body
  drove 853 -> 805 (not moved: the riders are not near the ranks; deviation: the rank is not demand-led). Cuts paid
  3,565c.
- **The hired day labourer works the site.** `day_labourer_75212 d27` was hired on the rest day (the post's shift is a
  workday's, `workday_of`), so he held a post with no shift and wandered; on `d25` he scavenged 06:00-08:00 until his
  shift. Now `dawn_pick` holds no shape-up on the rest day, and a one-day post (`Job.until`: the dawn's hire, a sick
  worker's cover) is `shift_pending` from the hire: he walks to the yard and waits there. Posts offered 2,377 ->
  2,086, filled 438 -> 374 (the rest days). The site's distance is S1's.
- **The bouncer.** The door already refused at 20k (960 in 30 days, nearly all "drunk", most at a few Clubs); V11's
  100k bouncers kept quiet doors, and the ejection path (an Assault inside) fired once in 30 days. Now (the customer
  ledger, `customers::rough_at_door` from `service::serve`): a client a tipped worker (a dancer) serves at a door a
  bouncer keeps (not the Brothel: the madam's) turns rough on `[verbs] door_rough_p` (0.03) x (1 - lawfulness): the
  worker hurt, an Assault, the bouncer throws him out and bars him `door_ban_days` (14) (`vice::bar`), grudges both
  ways; a barred client is turned away at that door every time (`customers::cause` "barred", past `door_p`). The
  bouncer's night is one line at his shift's end (`vice::door_told`, bodies only): "Clemence Quarry kept the door of
  Gambling Den#4757 tonight: 9 let in, 4 turned away (drunk 4)" (`ViceState.door_nights`; every Visit arrival counts,
  `service::arrive`). Refusals 960 -> 1,299 (barred 147), ejections 1 -> 81, door lines 507.
- **The reporter at 100k.** Diagnosed with a debug print at 100k (removed): two causes. (1) `run_stories` ran the
  Feeds in id order, each its beat and then its ranked picks, so the lower Feeds' ranked picks of an interviewed deed
  filled `outlets_per_deed` (2) before the walking reporter's Feed came up (every 100k beat deed past day 2 dropped
  on "outlets 2"). Now two passes: every Feed's beat stories, then every Feed's picks and plants. (2) The beat had no
  stops from day 4: `[gossip] pool_cap` is 64 entries a district at any population, and at 100k the first-hand
  entries (hops 0, the beat's only source) fell from 6,068 to 475 in five days as hearsay displaced them. Now the
  crime desk (`World.beat_desk`): `gossip::post_deed_at` keeps each first telling of a deed with a `deed_sev` for a
  day (pruned at midnight), and `beat_stops` reads it beside the pools. And a Feed's reporters split the scenes
  (`ensure_beat` rotates the k-th reporter's stops by k: all three walked the same nearest scene). Beat stories at
  20k 101 -> 420; 100k x 8: before, 16 runnable over days 0-6 and no stop from day 4; after, 17-19 a day on days 2-6.
  Deviation: `pool_cap` is not scaled per capita (that would move all gossip).
- **A hit on a CEO is heard of** (`contracts::heard_of`, from `accept`): a Hit, Beat or Abduct taken on an agent
  reaches a corp's exec through his security (`missions::exec_guards`) at once, anyone else on `hit_heard_p` (0.5, a
  hash of the record); the target's safety falls `hit_heard_safety` (0.3), a `Threatened` memory of the taker, and
  the event sits in his and his guards' diaries ("Jasper Ostrowski, Wren Hawthorne, Cato Carrow, Imani Ambrose's
  security, heard The Unplugged took the beat on Imani Ambrose and warned the boss"; "... got word on the street:
  ..."). 34 in 30 days. Deviation: nothing more follows (no guard bought, no hotel moved); the client is not named.
- **Reads wrong** (the next pass): the cab rank cruises but the riders stay far (one driver 0 fares in six shifts,
  the other 1 a shift); a cabbie on a gang's Guard at a Street Market for 481 min is still dispatched a fare
  (`standing_by` reads the shift, not where he is: `cabbie_11530 d22`); the warned CEO's safety is back to 0.94 four
  hours later and his night is unchanged; an exec is warned again each day a lapsed beat is re-taken; a Club's door
  counted 0 let in on the night its bouncer turned a drunk away; the hired hand rests 3 h at the yard before an
  08:00 shift.
- FINDING (printed, not tuned): Assault 1,030 -> 1,128, Murder 54 -> 71 (the rough clients' Assaults, the bouncers'
  fights). No test (doctrine 2). `SAVE_VERSION` 77 (`Jobs2State.fare_cuts`, `ViceState.door_nights`,
  `World.beat_desk`, the `[verbs]` S5 keys, the `Jobs2Cols` `cab_cuts`, `door_rough`, `door_barred`, `hit_heard`). Premises the moved city broke, re-pinned with the reason in place:
  `street::test_full_commuters_cab_is_timed_and_a_driver_drives_it` (the Garage stand-by: `cab_rank_tiles` 0),
  `scavs::test_drag_holds_gangwork` (the member a broke greedy one: coins and wealth set),
  `wounded::test_abandon_gang_leaves_its_own` (the mate placed beside the downed after the fall).

### R6b, the trades' verbs II (`OPEN-217`, `OPEN-207`; `[verbs]`, `[lod] contract_quota`)

Numbers: 20k `isle_m_k2` seed 42 x 30 (`--scaled --ambient-flats 50`), one run each, base `9755275` -> this build (the
last change, the beat's outlet filter, came after the run).

- **The gun hunts.** Cause: `[lod] contract_quota` (24 bodies, the live contract parties' class) was the one contract
  cap `Config::scaled_to` did not scale: `max_missions` read 120 at 20k but 24 bodies held ~8 runs, so a taken hit,
  beat, locate or tag queued to its deadline (base: 238 of 261 live takes `QUEUED`; V10's 100k guns held a Taken,
  Live hit and never walked). `contract_quota` and `fixer_quota` are now in `config.toml` and per capita in
  `scaled_to`. Second cause, once the runs started: `hunt::within_reach` cooled the goal when the habit's pick (a
  modal district's busiest Bar, the post by day) stood past `stakeout_ticks` (517 cools in 21 days); `contracts::
  reachable_door` now falls back to the target's Home, then its post, within the reach (the doors F1's take gate
  admitted the take on) before the goal cools (231 left: no door in reach from where the taker stands). Takes LIVE
  23 -> 287, QUEUED 238 -> 0; contracts fulfilled 78 -> 139, expired 140 -> 124; Hits done 4 -> 2; HuntStarted 121 ->
  193. FINDING (printed, not tuned): Assault 1,132 -> 1,391, Murder 46 -> 56. Left: a target in a cell (`target_ok`
  false: 1,034 cools by day 22) keeps the run waiting to its deadline, a cool every two hours.
- **The CEO's day is the Hq.** Cause: `industry::grow_hq` built one Hq a day city-wide, so at 20k (90 corps) and 100k
  the food corps waited weeks, and `life::hq_of` ranked the `Market` (a street market's stalls) above every venue:
  their execs held office at a stall with no Job (V10's `ceo_91004`). `[verbs] hq_builds_per_day` (1, per capita in
  `scaled_to`: 10 at 20k) and the Market leaves the rank (a packing plant's or a freight yard's office ranks after
  the Feed). Hqs built in 30 days 29 -> 42; Hq offices at day 21 60 (Market offices 0, from 9 at day 14).
- **A landlord corp's exec has an office** (`OPEN-207`): `life::office_of` = `hq_of`, else under `[verbs]
  landlord_desk` a lettings desk in the corp's standing Block nearest the exec's Home; read by `exec_day_pending`,
  the table (`service::meeting_hour`, `decided`'s district), the exec's ride (`streetwork`) and the diary's "no
  Hq". `hq_of` itself still gives no office in a Home (`test_review7_no_office_in_a_home_or_a_lot` stands).
  Deviation: no round over the blocks; the desk is a Meeting with nobody at it (the corp's supers walk their rounds),
  and a landlord corp past `hq_min_buildings` builds a real Hq anyway (Habitat's exec at Corp HQ#4532 by day 21).
- **The reporter files from his beat.** Cause: the beat (Lives at work phase 5) was walked and its interviews went
  nowhere: the night's ranked picks were other deeds, the byline went to the sharpest reporter, and the interviewed
  deed was often gone from the busy pool by midnight (201 of 367 by day 16) or already run by two outlets. Now
  `run_stories` runs `[verbs] beat_stories` (1) of each reporter's interviewed deeds under his byline first (inside
  `stories_per_day`; the pool's copy, else the deed as he took it down), the Story event names him ("... by Jocasta
  Jarrow (from the beat)"), and `beat_stops` skips a scene whose deed the other Feeds already ran
  (`outlets_per_deed`). Beat stories 0 -> 90 in 30 days (before the outlet filter).
- **The gangs' gun trade** (`weapons::gang_arms_trade`, its own function, the purses through `ownership::pay`): on
  its hashed trade day (`trade_days` 3, no saved state) a gang under its armoury target buys up to `trade_lot` (4)
  from the gang with the largest surplus at `trade_price_share` (0.75) of the dealer's T1 price (68c), keeping
  `trade_reserve_days` (1) of its stipends; the seller's registered dealer (else its leader) deals with the buyer's
  leader, the weapons move armoury to armoury. Deals 0 -> 11 (gang_arms_buys 5 -> 14 with the quartermasters).
  Deviation: a midnight ledger line, no meeting walked; most gang purses (0-200c) afford one weapon at most.
- **The teacher's pupils: diagnosed, not built.** The seeded city has no children (`seed_kin_p` gives grown kin
  only); the only children are the run's newborns (20k: 32 at day 14, 51 at day 21, all under 21 days old), and
  `ages::children` has no school age, so a paid infant would be a "pupil". All 20 Schools' rosters are 0 at days 14
  and 21. A design call for Dylan: seed school-age children at world generation (`OPEN-150`'s starting-condition
  seed) and gate the School routine by age.
- No test (doctrine 2). `SAVE_VERSION` unchanged: no World state; the save's config carries `[verbs]` as an optional
  section (absent: `None`).

### R5, gang verbs that stall (`OPEN-214`; `systems/crew.rs`, `gang.rs` the try-out, `[crew]` R5 keys)

Numbers: 20k `isle_m_k2` seed 42 x 30 (`--scaled --ambient-flats 50`), one run each, base `9755275`. Raid outcomes
swing run to run (three after-runs of near-final trees: 17, 15, 12 results), so read them as a range.

- **The muster held** (`crew::holds`, read by `fixes::committed`): a Raid plan holds against anything but danger or
  starving (as a commute) while the expedition is pending, unless the member chose the shift at midnight. The posted
  muster is a duty: `muster_flat` (0.5) over the Raid goal's considerations for a member holding today's Muster
  (V10: Raid ~0.3 lost to Eat, Unwind and Sleep all evening). A raid ordered after midnight's posting calls its muster
  then (`crew::raid_ordered`, from `faction::apply_order`; the shadow's raiders had no muster on a 06:58 order).
  **Found on the way:** the muster point is inside the Hideout, and a full Hideout failed the walk `BuildingFull` at
  the door (two of four raider picks); courts, rest nights and welcomes there failed the same. `crew::capacity_exempt`
  (in `World::capacity_exempt`): a member into his own Hideout or today's duty place, a recruit into his try-out.
  **The target** (`faction::corp_prize`): a corp raid's prize lies within `raid_target_reach_ticks` (240) of the
  Hideout; every corp raid had marched to one Street Market across the island (5b's reads-wrong). A marcher whose
  Brawl fails its start after the raid is over now reads 5b's late line. Raid results 8 -> 12 (6 won -> 2: three
  raids on Bar#4448 in the Precinct's district met 19-68 answering defenders; the a5 run won 15 of 17), called off at
  the door 8 -> 4, fizzled 2 -> 1; raiders at a fight 3-9 -> 3-24; Raid walk legs to the muster of 120 min+ done 34 ->
  140, aborted 55 -> 20.
- **The try-out** (`gang::tryout_plan`, `tryout_place`, `turned_away`; with `[crew]`): JoinGang is scripted (the
  planner's walk to a Hideout across the island is gone): `[GoTo place, JoinGang place]`, the place the recruit's gang's
  Hideout within `tryout_reach_ticks` (180) of where he stands, else its nearest front within it; neither: JoinGang is
  not scored (`already_satisfied`). The walk and the half hour hold (`crew::holds`; the shift alone breaks it).
  Arriving to no place in the roster (the treasury fell under the stipend line, the roster filled, the tie cooled):
  "X tried out for G at P and was turned away: the treasury could not feed another mouth", a `Crew` line, and no
  second try for `tryout_cool_days` (3). Joins 1,157 -> 1,143; JoinGang walk legs of 120 min+ 3,154 (2,377 aborted)
  -> 1,044 (240 aborted), of 240 min+ 983 -> 118; turned away 376 (265 treasury, 108 full up, 3 no tie). Beaten
  out of the colours is now a leaver's JoinGang cooldown (`[living] desist_cooldown_days`; a pick beaten out at
  11:54 joined the same gang at 17:45).
- **Scavenge** (`exec`): the scav's round is GangWork's Scavenge, which the start check read through the planner's
  `poor` symbol (every scav with a meal's coins failed `PreconditionLost` on the first tick, V10's 52). Every
  Scavenge now checks its own start (`actions::can_start`: outside, or inside a building that is not his Home, which
  the planner reads as the street after an Extort or in a back room; `on_start` steps out). Scav rounds 253 -> 350,
  finds 37 -> 47; Scavenge aborts (all goals) 2,755 -> 0.
- **The broke gang** (`crew::paid_slots`): a gang posts only the watches or guards its treasury over `pay_reserve`
  pays; none and it stands the duty down, told once to the gang and its leader ("The Drowned Choir 8 stood its watch
  down: the treasury (0c) cannot pay a 3c duty") and again when it pays ("can pay a watch again: 1 posted";
  `crew.stood_down`). "Unpaid: the gang is broke" 163 -> 20 (a watch posted at midnight whose purse fell by evening);
  stand-downs 45. Deviation: no new money (R6b's gun trade touches the treasuries).
- **5b's attrition, judged after the stalls** (colours lost in 30 days; behind each, the three misses that cost it):
  base 100 (court summons did not come 94, watch at work 63, court summons at work 61, watch did not come 42, muster
  did not come 23, guard 13, muster at work 4); the stall fixes alone (a5) 50 (court did not come 56, court at work
  42, watch did not come 15, muster did not come 15, watch at work 12, muster at work 8). The daily court rota was two
  thirds of the misses behind a lost colour, and a court at 20:00 posted to a cook whose set-out falls in a shift ending
  at 18:00 read as "chose the shift" three times and a beating. The rule changed: `court_work_excused` (true), a court
  summons missed at work takes the standing ("... (at work: excused)") but not the colours; a watch, a guard or a
  muster missed at work still counts. After (the final tree): 33 (court did not come 57, watch at work 20, muster did
  not come 12, muster at work 5, watch did not come 5; the a6 run 26). Gang leaves 220 -> 207 (walked away 72 -> 75,
  folded and absorbed members 27 -> 73).
- **Shadow** (20k seed 42 d16-d22, picks raider:6, gang_member:3, gang_leader:3, scav:3, jailed:2, homeless:3; the
  a6 build, the final adds the Earn Scavenge's step-out and the late line): "Viktor Volkov ... GOAL Raid (was Work):
  score 0.906 ... muster called", Muster 97 min, Brawl at 22:43, "Raid on Nutrix 2's Street Market#4427: The Ash Rats
  won (15 raiders, 1 defenders); the till smashed, 500c and 351 food taken; wounded: Joss Nakamura, Moth Haas";
  "PLAN JoinGang on Hideout#4464 ... ACTION JoinGang @ Hideout#4464, 31 min ... Tristan Marlow joined Ninefold
  (miserable, mood -0.28)" (`jailed_12466`; sh4: a 163-min walk, the try-out, "Marek Fairweather joined The Red
  Lanterns"); "Dahlia Volkov picked over the bins behind Street Market#4418 on the scav round and sold a find for 3c";
  "Wire Pembroke stood watch at Hideout#4474 for The Salt Widows with Juno Ostrowski, Bex Novak, Edric Novak, Hiro
  Yates (paid 3c)"; "Dorian Ravensworth missed The Drowned Choir 8's court summons at Hideout#4800 (at work: excused)".
  Reads wrong (next pass): a muster at the Hideout marches 461 tiles on an El-estimated reach (227 min) and two picks
  reached the door minutes after the brawl; a corp raid next to the Precinct meets the district's answering guards
  (the brain scores the posted ones only); a gang at the cap or the stipend line turns away a stream of recruits (The
  Unplugged 2, The Drowned Choir 8); "court summons did not come" is now the main loss (a duty Visit lost to another
  visit in the single slot, a far member, a night shift); musters still sit 3-5 h (the 8-h gather window).
- `person::probe_pimp_purse_grows_on_a_pitch_night` goes red here (green on main's last change run): the pick reaches
  his pitch at 23:36, the Collect is cut at midnight after 36 min, and he is jailed for a Deal at 16:03 the next day,
  so no second night; no R5 path touches him (his duty that day a rest-day crew visit). A butterfly on the probe's
  premise; left for the fix round. `probe_hungry_worker_eats_from_the_wage_before_rent` is red as on main.
- No test (doctrine 2). `SAVE_VERSION` 75 (`crew::State.stood_down`, the `[crew]` R5 keys, `Jobs2Cols`
  `crew_stood_down`, `crew_tryout_refused`; `analyze_run.py` prints them in the crew totals).

### R6a, the trades' verbs I (`OPEN-217`; `[trade_verbs]`, `streetwork.rs`, `vice.rs`, `leisure.rs`, `service.rs`)

Each trade's emptiness on a followed body had a cause; the fix is the cause's. 20k island seed 42 x 30 (`--scaled
--ambient-flats 50`), main `9755275` (before) and this branch (after: the build before the last two pimp changes, the
cab, pit, madam and Dollhouse code as merged), one run each. Shadow: 20k, days 14-17, cabbie x4, pimp, fighter x2,
madam x2, joytoy x3, street_worker x2 and six customers (`--agent`).

- **The cabbie.** A Garage keeps one driver a shift (`staff_per_floor` 2, `cab_shifts` alternating), and the
  commute's roll took the Garage nearest the rider's Home: when its one driver was not standing by within the
  rider's wait the off-screen cab took the fare and, past `rides_per_driver`, filled the Garage's day cap by the
  morning rush, so its driver's body stood by all shift with nothing booked (a debug pass: 240 of ~700 bookings
  with no driver on shift, most of the rest with the one driver an hour or more away; a followed D3 driver's
  Garage shut by the cap at 09:00). And every rider came at a shift's start (the evening driver, 16-24, met no
  rush), and after a drop-off the driver walked back (`cabbie_10678 d15`: 224 min for 227 tiles). Now
  (`garage_for`): the ride goes to the free driver's body within the wait with the fewest fares this shift, then
  the soonest, from any open Garage, the cap only the off-screen cab's; an off-screen rider waits
  `stat_fare_wait_ticks` (120; nobody sees his door); a shift's last hour rides home on the commute's own rule
  (`cab_roll_home`: `stat_cab`'s last hour, a body's GoTo home from the workplace once a day, the set
  `exec_home_rolled` reused, an exec holding no Job); a driver on shift heading back to his Garage drives
  (`cab_home`, the `fare_leg` timing). Rides a body drove 396 -> 817 (unmanned 3,637 -> 3,945), cab trips 3,949 ->
  4,685, cab fares 33.7k -> 38.9k. **Deviation:** a driver late past his shift's start drives in from home too
  (`cab_home` reads the GoTo, not where the cab is).
- **The pimp.** He collected (V10's 100k "never" is gone at 20k), but his body went to the nearest pitch due every
  hour, so one worker all night (12 collections from her in four nights, the cut after the first nothing, none from
  the other worker of his district), and a sitting pimp who took a day job kept the post (a Statistical pimp is
  credited present, so absence never dropped him; a Recycler Tech slept through every pitch night). Now the round
  (`round_pitch`) takes a pitch within `pimp_round_tiles` first, then one not collected at within
  `pimp_revisit_ticks`, one owed a take before one not, the nearest (`ViceState.pimp_visits`, pruned at midnight);
  `name_pimps` names the jobless member living nearest the district's street pitches and replaces a sitting pimp
  holding a Job or living `pimp_round_tiles` farther than him; an off-screen worker due at the hour's start is
  met by the pimp's body that stood the hour at her pitch (the hour's `p_pitch` draw met nobody at the end of a
  three-hour walk). Pimp collections 51 -> 32 (one district's pimp in the city; street pitches 3.8 -> 2.8 a day).
- **The fighter.** Bouts were already nightly (513 in 30 days), every one "bets paid 0": the bettors were the
  Full tier's, who plan a pit before the window opens (the step was gated on the window at the plan), and the
  off-screen pit visitor played a table game. Now an off-screen visitor of a pit in the bet window with
  `gamble_rung_courage` stakes on tonight's bout (`bout_bet`, `pit_bettors_max` a pit; both the learned and the
  choice paths), and a body's bet step reads the window at its arrival (`bet_window_at`). Bouts with bets 0 of 513 ->
  239 of 509; the bettors are the Fighter's Attendees ("fought before X (a bettor)").
- **The madam.** She had her workers and her cut, but no client in her diary. Now every Brothel visit served while
  she is on duty is her booking (`vice::booked` from `service::serve`), told in one line at her shift's end
  (`bookings_told`: the clients, the coins through the house, her busiest `madam_names` workers); on `rough_p` x
  (1 - lawfulness) a booked client roughs the worker up (an Assault, her `Lost`, the madam's grudge) and is thrown
  out and barred from the house for `rough_ban_days` (`ViceState.barred`, `leisure::door_ok`). Booking lines 0 -> 249,
  rough clients 35 in 30 days.
- **The joytoy.** `stat_table.toml`'s `p_venue` has no Dollhouse row (learned before the chip had clients), so the
  off-screen evening never visited one, and the high rung was the execs' and the top tenth's alone. Now the
  Dollhouse's off-screen rate is `dollhouse_p_venue` (0.002) where the table has none, at most a visit a night an
  eligible adult, and the lonely with `[customers] client_coins` reach it below the high rung (`lonely_client`).
  Dollhouse visits 25 -> 507. **Not done:** the stat table is not re-learned (Dylan's call).
- **Reads wrong / open** (the next pass's input): a body driver drives about one fare a shift (rides are ~140 a
  day for 35 Garages' 70 drivers) and is paid 2c of 6 (`[customers] cab_fares_full` 3); an off-screen pickup can
  sit 270 tiles off (`cabbie_10678 d14`: 79 + 95 min of legs for one fare). The pimp's district pitches lie up to
  340 tiles apart (`pimp_18110 d17` walked six hours to the far one when no near one was due); one collection in
  his four nights. A fighter's bout is an event while his body does ClerkWork (no fight action). A followed joytoy
  is hired 741 tiles from her Dollhouse (`joytoy_12426`: 330-min commute, in at 23:36); another pick is a corp exec
  holding a Contract Host post outside any chip contract (`joytoy_9225` works the Hq's office hours): the exec
  pick and the post, upstream. A Full lonely client's ladder never picks the Dollhouse (only the off-screen
  evening visits it). Pit#5578's bouts drew no bettor in four nights.
- No test (doctrine 2). `SAVE_VERSION` 76 (75 at the branch; R5 took 75) (`ViceState.{pimp_visits, bookings, barred}`). Premises the moved city
  broke, re-pinned with `trade_verbs = None` and the reason in place: `captives::test_weak_gang_under_retaliate_takes_a_hostage`,
  `captives::test_corp_refused_twice_takes_a_hostage_for_the_sale`, `leaders::test_staff_on_shift_pay_the_week_when_the_owner_is_away`,
  `arms::test_seized_crate_lands_in_the_precinct`, `arms::test_landed_crate_fills_the_armoury_and_the_muster_draws_it`
  (the bettors' stakes moved the gangs' purses), and `tower::test_off_city_writes_nothing_in_three_days` (the pit's
  bettors are the Fighter's Attendees, not the tower's).

### S3, justice that reads right (`OPEN-223`; `law.rs`, `fares.rs`, `[justice]`, `[fares] beat_cooldown_days`)

- **The "victim booked" was a resisted arrest.** `miserable_29154`'s three "Assault: Cutter Crane fought off X" lines
  were `law::arrest`'s scuffle text: three guards came for an open Assault warrant (filed before the diary's window)
  while he slept, a courage-0.95 suspect fought two off, and the third (Emeric Yardley, the "attacker") cuffed him.
  The attribution is right: at 20k seed 42 x 30, of 1,088 witnessed Assault/Murder lines 879 name the actor first and
  the 180 others are all blackmail victims who "went for" the blackmailer; every Assault sentence checked by hand has
  its swing (a drunk at a door, a Confronted owner, a brawl). What was wrong is the line. Now the scuffle reads
  "X resisted arrest and fought off the guard Y (wanted for Assault on Z)", the arrest "Y arrested X for Assault on
  Z", the sentence "X sentenced to 10 days for Assault on Z" (`law::charge_label`: the newest report's witness's
  `SawCrime` object; Murder "of", thefts and the shakedown "from"; no witness memory: the crime alone). No crime is
  added for resisting. Deviation: the brief's "name the attackers" is the guards, named as guards, and the warrant's
  victim. Not confirmed at 100k (the warrant predates d21); the 20k reading stands in.
- **The cells keep the body.** `jail_upkeep`'s meal came from the Market nearest the Jail alone; at 100k it ran dry
  and `jailed_75054` starved from d26. Now the first of the Jail's nearest `kitchen_markets` (4) with food; a body on
  screen that goes without gets a `Cells` line ("went without: the cells' kitchen had no food"), and a starving body
  is fed hourly as the held were (`held_hourly`). Sleep: `jail_upkeep` set energy to 1.0 at midnight, so a body on
  screen ran 1.0 -> 0.00 every day and never slept; now a sentenced body inside sleeps at lights-out
  (`lights_out_hour` 22 to `wake_hour` 6, `law::asleep_in_cell`, read by `needs::run` and the shadow tool's
  activity), and the midnight 1.0 is the off-screen hold's alone.
- **The cell is not a meeting hall.** The 1,171 Acquaintance lines a day were the gang pitch: `social::colocation`'s
  four-hourly jail drift (and `held_daily`'s) called `adjust` for a member with every inmate, which made an edge each
  time; `[social] acquaintance_cap` pruned them at midnight and the drift made them again. The drift now warms only a
  pair with an edge (the arrival's capped `jail_meet_max` meetings). No key: a bug.
- **The fare's cool-down.** `[fares] beat_cooldown_days` 3: a dodger beaten inside it (banned, or broke) is turned off
  the platform instead ("turned him off the platform (banned, the ban stands; beaten for it on d25)"), a lighter
  `Lost`, his plan aborted (he walks), and a broken ban stands rather than restarting. `fares::Record.beaten`;
  `fare_turned_off` 56 in 30 days; the most-beaten dodger 6 -> 5 (a cool-down, not an end: a ban-breaker under
  `ban_break_lawfulness` still rides and is beaten every fourth day at most).
- **The guard acts on what he saw.** `[justice] collar_hours` 2, `collar_flat` 0.5: the suspect of a report past
  petty that the guard himself filed in the last two hours, located and chaseable by him, is his collar
  (`law::collar_of`): the Arrest goal's target first (`plan::bind_target`), open off shift (`[life] arrest_off_shift`
  0 shut it), with a flat while he has it or chases it, so it beats the commute or the desk (a guard who saw a murder
  on his commute walked on to the desk, Work 0.99). The collar is his before another guard's claim (`chaseable` but
  for the claim: in `law::test_theft_witnessed_by_guard_leads_to_jail_within_1_day` a guard across the district
  claimed the thief first while the witness walked to his desk); `law::arrest` refuses a suspect another guard has
  cuffed (two after one). `law_collars` 196 in 30 days; arrests by the witness within 6 h of a guard's sighting
  28 -> 218. A guard on the take still files nothing and so has no collar.
- **Jail duty in blocks.** `[shift.roles] Guard = "stations"` with a gate in `shift::plan` (a guard's shape is the jail
  day's only; a patrol day is Patrol's): the 538-min `GuardJail` is now 40-75 min blocks across the Station or the
  Precinct with the mid-shift break, each block's end a think (`guard_24289 d23 09:06-17:17`: nine blocks).
- **Numbers** (20k island seed 42 x 30, one run each, base `7bbf9b8`): arrests 1,131 -> 1,451; jailed mean 150.1 ->
  181.9 (max 292 -> 392); Assault+Murder events a day 36.1 -> 51.0 (resisted arrests 217 -> 317, "attacked" 666 ->
  964, fights at the Precinct 19 -> 61: more arrests, more grudges on the released); violent deaths 85 -> 83; cell
  fights 55 -> 101; ticks/s 196 -> 130 (three runs on the box at once; a print). The arrests by part, one ablation
  run each on the build before the collar's claim fix (1,452 arrests): without the collar 1,294, without the desk
  blocks 1,384. The harsher law is the mechanism working, not a knob (doctrine 1, 5): a FINDING.
- **Shadow** (20k seed 42 d21-27; miserable, jailed, dodger, guard x2, plus `--agent 24289`, `7825`; no arms_dealer
  candidate at 20k, the jail edge flood read on `jailed_9922`): jail days read "sleep 8.0h, jail 16.0h" (was "jail
  24.0h"), energy 0.92 at 08:00 down to 0.42 at 20:00 and back by 06:00 (was 0.00 every midnight); new edges in a
  jailed week 898 -> 17-31; "Mirabel Draper arrested Quinn Penhallow for Grand Theft from Katrin Adeyemi", "Xia Sato
  resisted arrest and fought off the guard Mirabel Draper (wanted for Assault on Orrin Adeyemi)" three times, then
  "arrested Xia Sato for Assault on Orrin Adeyemi"; "Jasper Crane arrested Orrin Okafor for Murder of Varek Achebe";
  the collar: `guard_7825 d24 13:42` "Juno Adeyemi saw Dante Kaneda commit Shakedown", 13:55 "GOAL Arrest (was
  Patrol): score 1.292", 14:05 "arrested Dante Kaneda for Shakedown".
  Reads wrong (next pass): a patrol day still ends at ~14:00 "not in shift" with 1.8 h of work (corp V11 bug 2,
  untouched); an off-shift guard who saw a Grand Theft (`guard_24289 d21 13:49`) took no collar (the car thief out of
  a chaseable sighting); a sentence names the worst crime's victim even when the arrest was for a later one; a
  report with no witness (off screen) reads "for Theft" with no victim; the harsher law fills the cells (392 at peak) and the street's attacks rise with it.
- No test (doctrine 2). `SAVE_VERSION` 77 (`fares::Record.beaten`, `[justice]`, `[fares] beat_cooldown_days`, the
  `fare_turned_off`, `law_collars` columns). Re-pinned with the reason in place:
  `law::test_theft_witnessed_by_guard_leads_to_jail_within_1_day` to the one-action desk day (`[shift.roles] Guard`
  removed: the desk in blocks moved the scene and the witness lost the walking thief at the chase's last hop) and
  `scavs::test_drag_holds_gangwork`'s starving member holds a meal's coins (the moved city's pick was broke).

### S6, school-age children and the child-safety rule (`OPEN-224`; `[ages]` S6 keys, `ages.rs`, `camp.rs`, `routine.rs`)

**The rule (Dylan 2026-10-27, hard, not a switch): no child under 18 is ever killed, hurt or harmed; at 18 all bets
are off.** `ages::is_protected` (a `Child`, or under `[ages] protected_under_years` = 18, read whatever `enabled` says).

- **Seeded children** (`ages::seed_children`, the last pass of `World::new`): every couple sharing a Home has
  `seed_child_slots` (3) chances at a child on `seed_child_p` (0.6, a pure hash), aged uniformly under 18 and kept
  when the younger parent was `seed_child_gap_years` (18-45) at the birth; spawned as a migrant's child is
  (`demography::spawn_child_aged`, shared now: keyed stream, Parent and sibling edges, no world draw). A
  materialised Ambient couple brings its own (`ambient::materialise`: "... entered the simulation with 2
  children"), so the Statistical tier's households have them too. Day 0: 20k island 2,575 children, gen:5000 620,
  `isle_m_k2_100k` 12,555. **Deviation:** the households are the existing chunks (a couple and roommates); the
  children are dependants in the couple's Home over its capacity, as a birth is, not a re-dealt family Home.
- **School by age** (`school_from_years` 6): a younger child plays at home (an infant on a paid fee was V11's
  "pupil"). A School child is out of the Home 09:00-15:00 (`school_hours`, `household::home_term`).
- **The class** (`service::civic_hour`): one event a teacher ("Solene Hawthorne taught 2 pupils at School#5123",
  "(n more on the roll, no room)"); a pupil writes no teacher memory (V11's "a pupil's parent" was the pupil, an
  infant). The shadow names the parent with the pupil ("met Ines Ashcombe at the door (the parent of Cordelia
  Nakamura, a pupil)"), reads the parent's side ("my child's teacher", was "served-pair ... kind unknown"), writes a
  daily `[family]` line in a parent's diary (each child's place, the pantry), and picks `school_parent`.
- **Food counted** (`routine::grocery_plan`, `lod::stat_grocery`): the seeding alone starved 374 children in days
  6-11 (the jobless never stocked a pantry: the grocery run was a worker's errand, so the service took 1,100 and the
  camps could not feed them). A jobless parent now shops for the children's food by day, and every run's level
  holds `child_pantry_days` (2) of it (`ages::children_food_need`). Taken 1,100 -> 573, starved 374 -> 0.
- **The rule's sites** (audited every victim pick and harm site; the Brain-only ones already shut children out:
  Attack, arrest, brawls, raids and crossfire, riots, `fviolence` and `lod::stat_rolls`, contracts and hunts,
  harvest and abduction, captives, the fare ladder, vehicles, tags, consequence, the god commands, every hire and
  vice gate). Fixed where a child slipped in: the extortion's occupants, the intimidation's target, the territory
  tribute (`gang.rs`), the Home theft's named victim and the beggar's passers (`exec/actions.rs`), the pickpocket
  (`weapons.rs`), a child-owned vehicle (`vehicles.rs`), the organ take (`gray.rs`: a living child fails `living`
  too). Backstops that refuse: `World::kill_by` (counted `harm_blocked`: 0 in the 20k run), `wounded::down`,
  `demography::hire`, `gang::enlist`; the seed never deals an adult under 18. Hunger and sickness floor: no child
  dies unfed (home or camp: the Reserve covers a camp's shortfall, else an unfed day and the scandal), a child's
  malnutrition stops under `mal_onset`. The warden's beating is deleted (`p_camp_beating`, `camp_beating_*`, the
  `camp_beatings` column). A child's street theft and begging are ledger moves with no arrest; the service takes a
  street child at `street_take_days`. The camps still work children of `work_age` (10) for Parts.
- **Numbers** (20k island seed 42 x 30, `--scaled --ambient-flats 50`, before = main 7bbf9b8, one run each):
  people d0 20,027 -> 22,602; employed mean 11,309 -> 11,434 (d29 11,526 -> 11,802); starvation deaths 0 -> 0
  (children 0); homeless mean 366 -> 418 (d29 394 -> 500: the children count in a Home's residents, so the
  relocation and the move-ins find less room; not chased); company beds 4,206 -> 4,428, hotel nights 94 -> 93,
  cots 6.7 -> 6.8, squatters 255 -> 260; camp children d29 14 -> 478, street children 0 -> 81; violence deaths
  85 -> 91 (no child); School days 8 -> 1,868, but ~60 a day after week 1.
- **Reads wrong / open:** most school-age children never reach a School: 195 of 1,520 have one within
  `school_reach` (60 tiles; 20 Schools, Mid and up only, so the Sump has none), and the heads who can spare the fee
  fell 1,449 -> 687 by day 9; teachers teach 0-5 a day (`teacher_12629`: none in 7 days). A School per district or
  a reach by travel (S1's distance) is the next pass; no knob turned. A parent's diary shows the children only in
  the `[family]` line (no school run walked: a child has no body). A camp parent with a full pantry and three
  children in the Camp does not buy them out (`camp_parent_18435`: pantry 17, the fee 60). Re-pinned with the
  seeded children off and the reason in place (a butterfly or a room rule): `ages`
  (`test_fired_elder_moves_in_with_his_daughter`), `civic`, `household`, `housing`, `law`, `leaders`, `leisure`,
  `person` (two probes), `camp` (its fixture; the no-feed test now asserts the child lives), `world`; `scale`,
  `ambient`, `cli` count the adults or open the account on day 0's row; `utility` reads Brain holders. The camp
  beating's test half is deleted. `SAVE_VERSION` 77.

### S1, distance in the choice (`OPEN-221`'s sim half; `systems/distance.rs` `trip`, `[distance.choice]`)

Numbers: 20k `isle_m_k2` seed 42 x 30 (`--scaled --ambient-flats 50 --legs --legs-min 60`), one run each, base
`7bbf9b8`. The pick sites found, and what each now reads:

- **The body's own trip** (`distance::trip`, `trip_ahead`): the reach checks read `distance::travel`, the El's
  structural estimate, for bodies who then walked (no fare and no nerve to jump it, a ban, or after the last train:
  `noroute` 00:00-04:00). 20k base: 491 JoinGang legs over 120 min under the 180-tick try-out reach, 109 over 240.
  `trip` is the El's estimate only when he would board (`transit::pays_or_dodges`) and it runs now and at the
  estimate's end (`in_service`), else the walk; `trip_ahead` drops the hours for a trip planned at midnight. Read by
  `out_of_reach` (`[distance.reach]`), `crew::in_reach` and `set_out` (duty places), `gang::tryout_place`, and every
  pick below. The night's danger test is not read. The hire keeps the structural `travel` (the broke candidate who
  walks today rides on his first wage).
- **The Deal spot** (`stims::site_today` -> `member_site`): the gang's spot (`deal_site_in`: its venue, else its
  corner, else the deal Bar) in each district it holds and its deal district, the nearest by his trip from where he
  sets out (via the Hideout when he must pick up), within `deal_ticks` (120); none: no Deal now, his GangWork falls to
  the order's work. One Crew line a midnight for a stocked dealer with a body off every spot (`distance::daily`: "X
  stays off G's trade today: the nearest spot, P, is 3 h 38 from home").
- **The order's work** (`gang::gang_work_target`, a new `gang_work_ticks` 120): the Home to shake down or claim and the
  derelict to squat (picked near the Hideout and him) past the reach of where he stands is no GangWork (the first S1
  run: dealers off the spot walked 300-580 min to such a Home). The pimp's round pitch (`vice::round_pitch`) and the
  preacher's spot (`leisure::busiest_spot`) on the same reach.
- **The muster** (`crew::post_gang`, `raid_ordered`; `raid::muster_trip`, `muster_in_reach`): a member (and the
  leader) whose Home is past `[crew] duty_reach_ticks` (180) of today's muster point is not posted, and says so ("X
  sat out G's raid: the muster at a door in D1 The Sump is 8 h 40 from home"); the Raid goal's "muster called" gate
  reads the reach from where he stands (not re-read once his Raid plan runs: the march carries him off). **The raid's
  rival** (`faction`): `rival_exists` is false when the rival's Hideout lies past `[crew] raid_target_reach_ticks`
  (240) of ours (R5 bounded the corp prize so; a rival raid or a retaliation marched across the island).
- **The hire** (`demography::job_search_round`'s second round, the corner's dawn pick): the nearest left past
  `hire_max_ticks` (240) is not hired; the vacancy waits. `[distance] job_cap_ticks` stays the soft cap. World
  creation's staffing is the same job search, so the seeded far posts thin too.
- **The site day** (`shift::far_site`): a Construction Yard's nearest open site past `site_ticks` (120) of the Yard
  is no site day; the shift is worked at the Yard (V11's day labourer's 263-min walk to a Parts Fab).
- **The visit** (`household::considerations`, `visits_due`): a Visit's place past `visit_ticks` (240) of where the
  visitor sets out is not walked (a kin, the Camp, an errand: papers, a collection, a dun; a gang duty is `[crew]`'s);
  the week's Camp answer past it from his Home is dropped at midnight, both tiers, with a Visited line ("X could not
  make the trip to Y at Work Camp#4835 this week: 9 h 28 from home").
- **JoinGang** (`gang::recruit_gang`): with `[crew]` on, a gang with no try-out place in the recruit's reach is no
  pick; the best tie among the gangs he can walk to.
- Counters (`Jobs2Cols`): `choice_muster_out` 257, `choice_deal_off` 3,122 (lines), `choice_visit_far` 35,
  `choice_hire_far` 6,411 (refusals; a vacancy is re-read daily).

Before -> after: legs over 120 min cut short (`aborted`, every mode) 6,478 -> 5,173; walk legs over 120 min 16,216 ->
16,319 (flat: what is left is the guards' patrols 3,541 -> 3,954, Sleep Home ~3,000, Unwind Home ~1,450, Eat Market
~1,400 and commutes, none a pick of this phase), over 240 3,941 -> 3,694, over 360 1,029 -> 1,010. By pick (walk legs
over 120 min / cut / max min): GangWork Spot 297/156/630 -> 18/3/182; GangWork TargetHome 197/77/374 -> 67/4/192;
GangWork Seller (the pick-up) 115/68/630 -> 3/0/137; Raid MusterPoint 52/14/687 -> 52/0/386 (over 240: 22 -> 1);
JoinGang 491/88/457 -> 446/43/372 (over 240: 109 -> 2); Visit Victim 129/76/805 -> 67/14/364; Work TargetHome (the
site day) 59/3/311 -> 36/1/203; Work Workplace 1,563/57/1,092 -> 1,457/55/885 (over 240: 325 -> 224). Employed mean
11,309 -> 11,265, homeless mean 366 -> 330, starvation 0 -> 0. FINDING (not tuned): Extortion 532 -> 1,066, Assault
1,030 -> 1,548, Arrest 1,164 -> 1,539, GangJoin 1,030 -> 1,245, Raid events 67 -> 56: a dealer off his spot does the
order's work near him (the shakedowns), and recruits reach a gang they can walk to. Ticks/s 199 -> ~150 (a print:
other phases' builds and sims shared the box).

- **Deviations.** The spec's "picks the next nearer option or a different goal, and says so": a line for the muster,
  the dealer's day and the Camp answer; a kin visit, an errand, a JoinGang, a pimp's or a preacher's reach drops the
  goal silently (a line a think would be noise). The raid's target reuses `raid_target_reach_ticks` and the muster
  `duty_reach_ticks` rather than new keys. The map's shortage (Lots) is not this phase.
- **Reads wrong** (the next pass's input): commutes held from before the cap, or where the El estimate holds and the
  body walks (`gang_leader_6422`'s 438-min walk to his Bar post; a Utility Plant post 608 tiles off); a Contract's
  Guard post 290 min away (`miserable_6527 d22 04:13`, the hit market's pick); a back-room patch walk cut at 217-236
  min (`[care] back_reach_ticks` 300, R3); a Capsule Hotel 371 min off (`hotel_cap_ticks` on the El's estimate); the
  guards' patrol legs (S3's); the Unwind's stay-in fallback walks Home from anywhere. A dealer whose gang holds only
  far districts is off the trade for days ("15 h 08 from home" daily). A Crew duty's place is posted from the Home, so
  a member out at the hour still walks 400-550 min to it (Visit Seller over 240: 87 -> 75).
- Re-pinned premises (the reason in place): `person::probe_far_worker_with_one_rent_moves_near_work` and
  `probe_trade_respects_rent_and_the_evictor` (the hire's hard cap leaves no far Farmer at the v1 pace;
  `[distance.choice]` off), `fviolence::test_short_riot_rolls_its_hours_not_a_day` (the fix-on and fix-off worlds
  part before the riot; off). No test (doctrine 2). `SAVE_VERSION` 77 (`[distance.choice]`, the `choice_*` columns).
