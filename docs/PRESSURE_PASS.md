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

### Round 4 from Shadow V12 (`SHADOW_V12.md`, opened 2026-10-27)

Dylan's calls (2026-10-27): round 4 before the map; neglect warns first and takes on a repeat (`OPEN-231`); School
reach by travel time, not a School per district (`OPEN-232`); ticks/s 22.4 at 100k and 218 at 20k accepted again,
profiled once at the next close (`OPEN-229`). The under-18 rule holds in every phase (CLAUDE.md): a brief that
touches a victim pick, a fight or a punishment says how `ages::is_protected` gates it. Trades go two to a brief (S5
and R6a ran past the turn budget with four). Wave 1 is T1, T3, T4, T6 at once; wave 2 (T2, T5, T7) after wave 1
merges (T2 shares `ages.rs` with T1, T7 folds lines T3-T6 add). Iterate at 20k (`--population 20000 --map
assets/maps/isle_m_k2.txt`, `run` also `--scaled --ambient-flats 50`); V12's repro picks are 100k seed 42 days 21-27
(`target/shadow_v12/`), read there only when 20k cannot show the case.

#### T1, neglect warns first and the Camp is a place (`OPEN-231`)

The take is a ladder, not a roll: a neglected child first brings the service to the door (a warning the parent hears,
a few days' grace read from config), and only neglect again inside the window takes the child. The camp from the
inside: a warden's day among the children (headcount, rations, an absconder brought back, never a blow), the parent's
visit by reach (S1's trip; "could not make the trip" stays when it is true), the buy-out and the break-out read from
the parent's side, a child let home when the parent has fed the household long enough. Shadow: camp_parent,
camp_warden, school_parent, homeless. Moves a number: ChildTaken and the daily ch_camp, 20k seed 42, before and after.

#### T2, School by travel time (`OPEN-232`), wave 2

A school-age child goes to the nearest School inside a travel limit (walk, or the El with the fare a parent pays); a
full School turns the next child away; a child out of reach stays home and the `[family]` line says why ("no School
within 1 h"). The teacher's day follows the roll: no class, no ClerkWork at an empty School. Shadow: school_parent,
teacher, and the archetypes whose `[family]` lines show children (gang_member, forger, buyer). Moves a number: school
days / child-days (4.0 % at 100k), 20k and 100k, before and after.

#### T3, plans that still die silently (`OPEN-222`)

A held contract that never plans (`gun_36512`, "backed off, 7 in a row", then `ContractExpired`): find why the plan
fails and either plan it or let the taker drop it with a reason. An arrest chase that ends in `Unwind` says so and
hands the case on (`guard_8440 d23 11:10`, 1 of 4 completed). A Guard's Work plan that times out on a 0-tile GoTo
(`runner_90375`, 25 times in 6 days). A mustered raider marked "missed" (`raider_70326 d22 00:07`, V11's). A contract
brawl that hits three people who are not the target (`gun_37892 d26 00:09`). Shadow: gun, guard, runner, raider.

#### T4, the trades' inside: the door and the yard (`OPEN-217`)

Bouncers, dancers and club staff at 100k are Wage plus ClerkWork for 28 job-nights; S5's door fired at 20k only. Find
why the followed body never meets a client at 100k (the LOD tier, as S4's care was, is the first suspect) and give the
shift its inside: the door's refusals and ejections, the dancer's set and tips. The foreman's 174 Hire lines become a
morning at the yard (who was picked, who was sent home). Shadow: bouncer, dancer, club_staff, foreman.

#### T5, the jobless look for work (`OPEN-217`, `OPEN-228`'s follow-up), wave 2

A laid-off worker searches (asks Friends, walks to the yards and the hiring boards, takes worse work) instead of four
idle days (`night_shift_23119 d21`); a hired day labourer works the day he was hired for (`day_labourer_100168 d23`);
the hire by distance prefers the city's own jobless near the post over the Ambient pool (S1 stranded natives: homeless
max 11.3k -> 11.9k). Shadow: night_shift, day_labourer, worker, homeless. Moves a number: Materialised and employed
mean, 20k seed 42, before and after.

#### T6, money and labels that disagree

A treatment billed 80c that moves 1c to another payee (`miserable_64146 d22 05:10`); a con that names two sellers
(`vendor_11367 d24 00:09`); the foreman named who just lost the post (`ripperdoc_61226 d27 00:11`, V11's); a CEO's
decision `Served` five times in 70 min at home (`ceo_88522 d24 00:07`); an exec hired as an Engineer while still an
exec (`exec_78363 d26`); a 600c ledger theft four times a week with no heat (`fixer_100695`). Each line says what the
money did. Shadow: miserable, vendor, ripperdoc, ceo, exec, fixer.

#### T7, noise after round 3 (`OPEN-233`), wave 2

The 00:07 standing-fact line ("stays off the trade", "sat out the raid", "chose the shift") logs on the first day and
on the change; "N in a row" back-off lines fold; gossip runs of 20-40 lines fold; a guard's Fare Evasion witness lines
from inside the Precinct; the hourly "met at the table" memories (`ceo_88522`). The victim's age on Assault and
beating lines, so a shadow can check the under-18 rule. The shadow's `sick` pick carries an illness (V12's had none).
Shadow: every group's busiest diary (gang_member, madam, guard, ceo, sick).

### Round 5 from Shadow V13 (`SHADOW_V13.md`, opened 2026-10-28)

Dylan's calls (2026-10-28): Schools seeded by child count **and** founded on demand (`OPEN-232`); the 2c fee stays
(poor children out of School is the city); the neglect ladder tightens: a second warning inside 30 days takes, and a
warning with no free parent at home takes at once (`OPEN-231`); the starving broke get a Mission meal that reaches
them, charity that runs short, not a dole (`OPEN-235`); a small perf phase for `goal::GangWork` (`OPEN-229`); the
round also takes witnessing indoors (`OPEN-236`), the label trades (`OPEN-217`, two to a brief) and the
contradictions (`OPEN-237`, `OPEN-222`'s leftovers). Left out on purpose: the search's other edges and the crew
day-off / BinContest noise (the next noise pass). The under-18 rule holds in every phase (CLAUDE.md): a brief that
touches a victim pick, a fight or a punishment says how `ages::is_protected` gates it. Wave 1 is U1, U3, U4, U5;
wave 2 (U2, U6, U7, U8) after wave 1 merges (U2 shares the Camp parent with U1's Schools, U7 the witness and memory
paths U4 changes, U8 profiles the code the round leaves). Iterate at 20k (`--population 20000 --map
assets/maps/isle_m_k2.txt`, `run` also `--scaled --ambient-flats 50`); V13's repro picks are 100k seed 42 days 21-27
(`target/shadow_v13/`), read there only when 20k cannot show the case.

#### U1, Schools for the children there are (`OPEN-232`)

Seed Schools from the city's school-age count (`[ages]` children a School, placed by T2's reach rule), so 100k stops
undercounting (`schools` counts per Lot copy: 18 for ~11k children, every one full); then found one on demand: a
district whose children are turned away full for N days gets a School on a vacant Lot (or a refit Block, as
`jobs::seed_derelict` does), staffed by the hire. The fee stays. Shadow: school_parent, teacher, and a `[family]` line
with children. Moves a number: school days / child-days and the turned-away-full count, 20k and 100k x 8, before and
after.

#### U2, the neglect ladder tightens (`OPEN-231`), wave 2

A second warning inside `warn_repeat_days` (30) takes; a warning with no free parent at home (every parent jailed,
hospitalised or gone) takes at once. The bugs at its edges: the warning's household is not the child's own
(`reporter_57837 d27 00:10`, Rin Duarte at Block#3363, her Home Block#205); the `[family]` line still "at home" hours
after a take (`camp_parent_23527 d25 09:30`); a jailed parent's warning hanging open all week (`dancer_99218`); a
jobless parent's search crowding out the week's Camp visit (T5's re-pinned buy-out probe: a visit due outranks a
search day). Shadow: camp_parent, camp_warden, school_parent, dancer. Moves a number: ChildTaken and ch_camp, 20k seed
42, before and after.

#### U3, the starving broke reach a meal (`OPEN-235`)

Charity, not a dole: a Mission or soup line serves the starving within the week (servings in config, a queue, a meal
that runs out), a body at hunger 0 walks there by reach, the off-screen starving get the same at the Mission's hour;
the evicted and the just-released are the cases (V13's dead: ~17 evicted, ~7 out of the cells, 7 housed with
`RentShort 0 of N`). Shadow: homeless, miserable, jailed (the released), sick. Moves a number: starvation deaths and
Starving events at 100k x 30, before and after (20k shows none).

#### U4, who is indoors sees indoors (`OPEN-236`)

A witness inside a building sees what happens in that building; a body outside sees the street within sight. The
cells do not see the El gate (`gang_member_6738`, 31 Fare Evasion lines), the Precinct's desk does not see the
street's assaults it cannot reach (`guard_27360`, 41 crimes, the one Arrest `Unreachable`). Witness 190k -> 269k at
100k is the number to read. Shadow: guard, jailed, gang_member, reporter.

#### U5, the forger and the fixer at work (`OPEN-217`)

Both do ClerkWork in V13 (crime). First find why the followed body never meets a client at 100k (T4 found the LOD
tier took the door's customers; S4 the same for care): then the forger's commission (the papers, the walk-in, the
cut) and the fixer's day (posts brokered, the crew picked, the job's outcome back to him; corp's Fixer posts "never
come back with an outcome"). Shadow: forger, fixer, buyer.

#### U6, the pimp and the ripperdoc at work (`OPEN-217`), wave 2

The pimp runs gang Extort errands, no round and no booked client (crime); ripperdocs see 0 and 1 patients a week, 8.8
h of ClerkWork or Rest, and rest at the post five hours before the shift (`ripperdoc_11503 d22 04:13`); the orderly's
34 installs land at 00:00 (care). The same cause first (the tier), then the round and the clinic's day. A jobless pimp
does not search like a labourer (`pimp_84453`). Shadow: pimp, ripperdoc, orderly, street_worker.

#### U7, contradictions (`OPEN-237`, `OPEN-222`'s leftovers), wave 2

Each at its cause: the thief gets the "was robbed" memory and grudge (`homeless_71553 d21 16:26`); a raider "sat out
the raid" while it marches (`raider_84195 d21 00:07`); a contract posts a guard on its own taker (`runner_94196`,
`gun_41438 d23 00:09`); a seven-day stakeout with 12 aborts and no result (`gun_41438`); CabFare names one Friend as
every rider's payer (`cabbie_73680`); a shakedown's victim witnesses his own robbery, its money line `other`
(`jobless_6204`, `courier_50838 d23 03:28`); a sick worker foreman of her own cover hire (`sick_47483 d21`); a CEO's
585c corpse strip unwitnessed (`ceo_20314 d21 08:20`); a jailed exec paid his ExecWage. Shadow: homeless, raider,
gun, runner, cabbie, courier, ceo, exec.

#### U8, the GangWork hotspot (`OPEN-229`), wave 2

`goal::GangWork` scoring is 21.7 % of the 100k profiled time (45.8 s in days 25-27, 3.66M calls, ~12.5 µs each,
`target/r4_perf/prof100k.err`), ahead of `goal::Unwind` (11.3 %). Find the scan behind it and cache or index it; the
scores need not be byte-identical (doctrine: no identity gates), but the choices should read the same in a gang
diary. Numbers: the site's ms a game day and ticks/s, 100k days 25-27 under `--features prof`, before and after, one
run each; a gang_member and gang_leader shadow at 20k.

### Round 6 from Shadow V14 (`SHADOW_V14.md`, opened 2026-10-28)

Dylan's calls (2026-10-28): gangs get **turf** (`docs/TURF.md`: cells held by presence, the corner, turf that pays,
the pimp's pitch where the members live, a war that is provoked, answered, held or lost, and ends); the order is
round 6 before the 100k map re-import (Dylan deferred it); the soup lines stand as U3 built them; the School fee
stays and an empty School is not yet a feature. The under-18 rule holds in every phase (CLAUDE.md): a brief that
touches a victim pick, a fight or a punishment says how `ages::is_protected` gates it. **Cost rule from round 5** (U3
and U6 ran $80 each): a phase's 100k work is at most one run (a counter run or one shadow), named in its brief. Wave 1
is V1, V2, V3, V4 (disjoint: the Camp and household, the gang and vice, the law and the wounded, leisure and the
trip); wave 2 is V5 (on V2's cells) and V6 (the vice and care rooms V2 and U6 touched), each started as soon as its
dependency merges. Iterate at 20k (`--population 20000 --map assets/maps/isle_m_k2.txt`, `run` also `--scaled
--ambient-flats 50`); V14's repro picks are 100k seed 42 days 21-27 (`target/shadow_v14/`).

#### V1, the ladder at 100k and children out of care (`OPEN-231`, `OPEN-242`)

U2's ladder hangs at 100k: warnings standing a week and more with empty pantries and no second warning or take
(`camp_parent_102044` since d5, `scav_46032` since d14, `owner_broke_6426` past his jailing on d26,
`jailed_71789`'s children evicted onto the street d26 untaken); one warning names 18 children of ten families at one
door (`owner_broke_6426 d21 00:10`: a warning is per household, not per Block). Find why the second unfed spell is not
seen at 100k (the tier, the household's pantry read, the warning's household) and fix it there. Children out of care
are found and returned: a child slipped out of the Camp is fetched by the warden or the service within a day; a child
on the street with no free parent is taken (`camp_jobless_61284` d24-27, `miserable_12664` d21-23, two-year-olds).
Shadow: neglect_parent, camp_parent, camp_jobless, camp_warden, miserable (20k; one 100k shadow of neglect_parent and
camp_jobless if 20k cannot show it). Moves a number: ChildTaken, ch_camp, ch_street and warnings standing past 14 days,
20k seed 42 x 30 before and after.

#### V2, turf you hold (`docs/TURF.md` § 1-2; `OPEN-241`, `OPEN-217`'s street)

Turf cells held by presence, the corner, `rival_of` the neighbour, a holder change told; tribute and protection by
cell, the deal spot on the corner, the pimp's pitch in a held cell where members live and the pimp named among them
(replacing U6's `pimp_trip_ticks`, which left 0.2 pimps a day at 100k), recruits from the turf, residents who read the
colours. Both tiers. Fix the two age gaps the codemap found (`gang::contested`, the squat-eviction fight). Also V14's
gang contradictions at their cause: a member dying mid-Extort still collects (`gang_member_6572 d22 14:24`), one
door paying three gangs in 15 hours (`Block#478`; the cell's holder protects his own), `BountyPaid` printed three or
four times a sighting. Shadow: gang_member, gang_leader, dealer, pimp (20k). Moves a number: cells held per gang,
pimps a day and street visits, 20k seed 42 x 30 before and after; one 100k x 30 for pimps a day.

#### V3, violence with consequence (`OPEN-238`)

A death is found: a corpse in the street is seen, reported, and the law opens a case (`courier_48760 d25 05:25`: a
murder and a strip, then work at 08:10 with no Report or corpse line); a looted till is reported by its owner and the
thieves are known (`vendor_9648 d23`: ten strangers in six hours, no report); the wounded on Flee are treated or
carried, not left 72 h at energy 0.01 (`ceo_91004`, `guard_8429`), and a man laid up draws no wage; a Fixer "closed by
the law" is closed (`fixer_40483 d26`). Shadow: courier, vendor, ceo, guard, fixer (20k). Moves a number: Reports and
arrests per Murder, corpses found, the longest Flee, 20k seed 42 x 30 before and after.

#### V4, the night out and the trip (`OPEN-239`, `OPEN-240`)

The Unwind plan does not drink before the door: the bouncer's refusal of the drunk is answered (gamble first, or
another venue), a "barred till" is honoured (`office_worker_56309 d23 21:23`, nightly). A move-in with family reads
the commute (`night_shift_27693 d23 00:08`: 475 tiles, 503 min each way); a cab leg is paid to the cab, not a
stranger, and lands where it was paid for (`fabber_84414`, `exec_73953 d24 01:16`). Shadow: office_worker,
night_shift, fabber, exec, cabbie (20k). Moves a number: door refusals of the drunk, commutes over 3 h, cab fares by
payee kind, 20k seed 42 x 30 before and after.

#### V5, the turf war (`docs/TURF.md` § 3), wave 2 after V2

Provocation on a held cell, the local answer at the corner, hold or flip, the loser's counter-attack, and peace:
exhaustion, a truce between leaders, the law's crackdown on a contested cell; a raid on a Hideout moves the cells
around it; off screen a daily roll with the same events. Shadow: gang_member, gang_leader, raider (20k). Moves a
number: wars started, cells flipped, truces, gang deaths, 20k seed 42 x 30 before and after.

#### V6, empty rooms beside full ones (`OPEN-232`, `OPEN-217`), wave 2 after V2

Patients spread to the clinics that are open, a doc's post not dark all week (two of four clinic posts with no
patient, a ripperdoc struck off unexplained: `ripperdoc_11838 d26`); the back rooms share the trade (574 patients at
one, none at the other); the door turns someone away (bouncers "nobody turned away" every night at 100k) and the
fighter has a bout (no Bout line in a week, the pit door 18-59 a night). Shadow: ripperdoc, orderly, black_doc,
bouncer, fighter (20k). Moves a number: patients per clinic (median and max), door refusals, bouts, 20k seed 42 x 30
before and after.

### Round 7 from Shadow V15 (`SHADOW_V15.md` § "Round 7's input", opened 2026-10-30)

No calls pending for Dylan; the order is V15's ranking. The far posts (`OPEN-240`, `OPEN-221`, V15's item 5) wait for
the 100k map re-import. The under-18 rule holds in every phase (CLAUDE.md): a brief that touches a victim pick, a
fight, a fine or a punishment says how `ages::is_protected` gates it. **Cost rule** (rounds 5 and 6): a phase's 100k
work is at most one run, named in its brief. Wave 1 is W1, W2, W3, W4 (the household's purse; the gang, the war and
the case; the guard's arrest, the fare and the rescue; the hire and the post's credit). W2 and W3 both touch
`law.rs` (W2 the victim's and the witness's side of a case, W3 the arrest and the beat): each brief names the other.
Wave 2 is W5 (contradictions, across everything) and W6 (the dull rooms: vice, leisure, care), started when wave 1
has merged. Iterate at 20k (`--population 20000 --map assets/maps/isle_m_k2.txt`, `run` also `--scaled
--ambient-flats 50`); V15's repro picks are 100k seed 42 days 21-27 (`target/shadow_v15/isle100k_s42_d21/`).

#### W1, the household buys for its children (`OPEN-243`, `OPEN-231`)

V1 split the shelf by household, so the ladder now fires at 100k, and it takes children from families who could feed
them: a hungry worker's Eat buys for himself, and the grocery run stands in only for a fed worker's Eat
(`grocery_due`), so a household with coins lets its children go unfed until the warning and the second spell takes
them (100k ChildTaken 624 -> 4,103, 500 a day at d19-21; `ceo_20314`'s 500c household with a 17-y "2 days unfed";
20k: 37 of 63 second-warning takes covered by the purse). The fix is the shopping, not the ladder: a parent's
grocery run buys the children's share whenever the household's claim is short of it, hungry or fed, Full and
Statistical; a warning moves the household's purse first (the run that day, ahead of the adults' own wants); a
household that truly cannot buy still meets the ladder as U2 built it (`camp_parent_106052`'s 4-y on the warning
since d18 all week is that case: say why it hangs). Shadow: neglect_parent (a pick with coins), camp_parent,
school_parent, ceo, camp_jobless (20k). Moves a number: ChildTaken, ch_camp mean, warnings standing past 14 days,
starvation deaths, school days, 20k seed 42 x 30 before and after; one 100k x 30 counter run after (r6's are
`target/r6_100k/`).

#### W2, the war and the case in the diaries (`OPEN-241`, `OPEN-238`)

The war and the case reach only the counters. The muster walks off (`cultist_62792 d21 18:32-d22 00:09`: 5.5 h
walking, asleep at energy 0.03, no Brawl; V14's bug unchanged): find why the walk does not end at the corner, and
fix it there. A member hears his gang's war: the call to the corner, the cell held or lost, the truce, the
crackdown on his block, each a line in a member's diary (today the leader's only, `gang_leader_68028 d24 00:05`).
The victim's and the witness's side of a case: a sleeper robbed four times in three days finds it when he wakes and
tells a guard or files a report, or says why not (`courier_50890 d21-23`); a witness who finds the body of a murder he
saw keeps quiet for a reason the diary says (his gang, fear of the killer) and the case's "no suspect" reads from
that (`gang_member_6731 d24 02:54`); tribute lines that read 0c pay something or are not printed (street). The
under-18 rule: no muster, corner fight or war pick names a minor. Shadow: gang_member, gang_leader, cultist, courier,
raider (20k). Moves a number: musters that end in a corner fight, members with a war line, 20k seed 42 x 30 before
and after.

#### W3, the guard makes the arrest (`OPEN-244`)

One arrest a guard a week: `Arrest unplannable: Unreachable` and "no guard on shift near; the warrant stays open"
(`guard_27438`). Find why the arrest is unreachable (the target's place, the guard's district route, a held or
indoor target) and fix it there, so a warrant a guard holds is served or handed on. The fare: `guard_8327 d27
14:12-14:23` beats 18 fare evaders in 11 minutes, some 61 and 67: the beat takes a fine first (a fare evader with
coins pays, the broke one is beaten or booked), a guard handles one evader at a time at a pace a person keeps, and the
old and frail are booked, not beaten; under 18 is never beaten (`ages::is_protected`, S3's rule). The rescue: a
bleeding 61-y calls for help three times and no one comes (`camp_warden_8656 d22 13:55`): a call for help reaches a
guard, an orderly or a passer-by within reach and someone answers (Rescued) or the diary says why not. Shadow: guard,
camp_warden, cabbie, sick (20k). Moves a number: arrests a guard a week, warrants served and still open at day 30,
fare beatings and fines, Rescued, 20k seed 42 x 30 before and after.

#### W4, the right person in the role (`OPEN-245`)

The midnight hire names the wrong foreman: a forger (`forger_7536 d21 00:11`, who never works all week) and an
orderly (`orderly_21547 d23 00:11`). The foreman of a hire is someone who works at that post's workplace in a
foreman-able role (round 5 U7 put him at work and not sick; now the role). A post's work is credited to the person
in the post: the orderly is credited with 36 chrome installs while the Ripperdoc post is vacant and paid nothing for
them (`orderly_13875` the same); a chrome install needs a ripperdoc (or a black doc) at work, and the work done pays
the one who did it. Shadow: orderly, forger, ripperdoc, foreman (20k). Moves a number: installs by the doer's role,
hires by the foreman's role, 20k seed 42 x 30 before and after.

#### W5, contradictions (V15 item 6; `OPEN-237`'s leftovers), wave 2

At their cause, each: a father jailed 20 days for "Abduction" for breaking his own 14-y out of the Camp, the witness
lines swapping actor and witness (`camp_parent_51134 d22 19:18`): a parent taking his own child back is a Camp
escape, not an abduction; a barred man back in the room the same night (`dancer_74279 d21 20:32`); a CEO shaken down
asleep and robbed again, never reacting (`ceo_20314 d23`); the `BountyPaid` fold missed Dealing (five Witness and
money lines a stand, `dealer_101293 d22 19:01`); the "fee beyond the purse" with 10c in it (`school_parent_48165 d22
09:30`, V14's); a marriage while the groom is in a cell; a told job lead dropped (`jobless_6217 d24`). Shadow: the
picks' archetypes (20k). No number moves; one 20k shadow.

#### W6, the dull rooms (V15 item 7), wave 2

The door's only off-screen reason is "broke" (the Statistical tier carries drink and coins only): give the off-screen
visitor the reasons the door reads on screen (barred, trouble, the gang's colours on a held cell). The bout has stakes
for the fighter: a purse to the winner, a hurt (`wounded`) and a loser's line; fighters are adults (the pit's picker
gates on `ages::is_protected`). Two of four pimps are gang men who run no round: a pimp named runs his round, or is
not named. The clinic with a doctor on shift and no patient (`ripperdoc_9880`, V14's): V6's spill never binds in a
pick; find why. GOSSIP ABOUT ME lines 60 -> 259 in the vice diaries: fold or rate them (`[noise]`). Shadow: bouncer,
fighter, pimp, ripperdoc, dancer (20k). Moves a number: off-screen door refusals by reason, bout purses, pimps running
a round, patients per clinic (median, min), 20k seed 42 x 30 before and after.

### Round 8 from Shadow V16 (`SHADOW_V16.md` § "Round 8's input", opened 2026-10-30)

Dylan's calls carried: the neglect ladder's size at 100k is left as built (`OPEN-231`; no phase touches it), and far
travel (V16 item 2: `OPEN-240`, `OPEN-221`) waits for the 100k map re-import. The under-18 rule holds in every phase
(CLAUDE.md): a brief that touches a victim pick, a fight, a fine or a punishment says how `ages::is_protected` gates
it. **Cost rule** (rounds 5-7): a phase's 100k work is at most one run, named in its brief. Wave 1 is X1, X2, X3, X4
(the law's promise and the held; the target and the bereaved; the hire and the earner's crime; the gang). X1 and X3
both touch who acts while held or employed, X2 and X3 both touch `search.rs` and `careers.rs`: each brief names the
other. Wave 2 is X5 (the pit, the lines' ages, marriages) and X6 (the cell's day, the clinics' walk-ins, the shadow's
pickers), X6 started when X1 has merged (the cell). Iterate at 20k (`--population 20000 --map
assets/maps/isle_m_k2.txt`, `run` also `--scaled --ambient-flats 50`); V16's repro picks are 100k seed 42 days 21-27
(`target/shadow_v16/isle100k_s42_d21/`).

#### X1, the promise to tell the law, and the held and the asleep who act (`OPEN-246`)

W2's sleeper wakes to the short purse and "means to tell the law", and 5 of 7 never do (`foreman_21236 d23, d27`,
`courier_45596 d24-25`, `gun_44357` three times, then attacks instead; `cook_25080`, `dancer_104874`; 100k: 455 woke,
76 reported). A promise in a line is a goal the person carries: the report is made (a Hall, a guard on the beat, the
Precinct) within a day, or the diary says why it was dropped (the thief found and dealt with himself, fear, the
reach). The held and the asleep do not act: a man in a cell witnesses 14 crimes after his sentence
(`ripperdoc_10662`); a victim beats the thief while the thief is held in the Precinct (`street_worker_35520 d22
18:16`); an orderly blackmails while asleep (`orderly_20962 d26 12:50`); a fight in the cells and a Murder there
print no place, parties or case (`pimp_14518 d26 06:18`, `black_doc_79637 d27 07:16`). Find each at its cause (the
witness roll, the attack's target pick, the blackmail's actor, the cell's crime lines) and fix it there; a crime in
the cells is a crime with a place, its parties and a case. A murder after a pit brawl names the bystander who
reported it as the killer (`miserable_103693 d26 20:15`): the suspect is a party to the crime. Shadow: foreman,
courier, gun, ripperdoc, street_worker, orderly, jailed (20k). Moves a number: woke-and-meant / reported, witnesses
who are held or asleep (should be 0), 20k seed 42 x 30 before and after.

#### X2, the target and the bereaved react (`OPEN-247`, the first half)

A 1,453c hit taken on a sleeping CEO who idles on (`ceo_20314 d27 18:00`, V15's, unchanged in kind): find what
becomes of a taken hit (the hitter's plan, its failure, its timeout) and make it land, fail with a line, or reach the
target as word he can act on (guards, lying low, leaving town, a counter-hit), each with a line. A widow's hunt cools
in 16 minutes and she is at work the next morning, no funeral, no missed shift, her children no line
(`school_parent_87579 d25 21:00`): a death in the household is grief that takes days (a funeral, missed shifts with
the employer's leave or a sacking, the children's line, a hunt that holds while the grudge is hot). A fired cabbie
and a day labourer never search all week; "Quit (no need of the wage)" then four job searches the same day
(`gun_44357 d27`, 255c): the jobless who need the wage search, the one who quit for want of need does not. Shadow:
ceo, school_parent, cabbie, day_labourer, gun (20k). Moves a number: hits taken / landed / failed / dropped, days off
after a death in the household, searches by the sacked, 20k seed 42 x 30 before and after.

#### X3, the wrong person in the role, II (`OPEN-247`, the second half; `OPEN-245`'s follow-up)

A man three days out of a theft sentence hired as a Guard (`camp_jobless_66381 d27 11:30`): the law's posts (and
the doors and the vaults, where the trade says so in `[[trades]]`) read a record. A 61-y ripperdoc and a teacher
steal motorcycles to earn (`ripperdoc_10662 d24 04:34`, `teacher_14781 d25`): the Earn crime plan wins at 0.05 for
someone with a wage; find why it wins (no legal Earn option scored, a fallback) and fix it there, so the employed earn
by their post and crime comes from need, misery and low lawfulness (phase 1's rule). A Fighter named foreman of a
Fighter hire (the `no_foreman` list misses Fighter, or the pit's hire reads another list). Shadow: camp_jobless,
ripperdoc, teacher, fighter, guard, foreman (20k). Moves a number: Earn crimes by the employed, hires with a record
by trade, 20k seed 42 x 30 before and after.

#### X4, gangs (`OPEN-241`)

The lie-low line names a different enemy than the war (`cultist_80865 d26 00:05`); leaders change with no Succession
line (`raider_29990`, `scav_99001`): a new leader is told to the members and the city's rumour. No raid lands in 16
diaries: the muster point ignores who is near and members sit out "3 h 08 from home"; corner turnout 1,586 of 3,297
at 100k (the call wakes nobody, W2's left-open): the muster takes the members within reach of it, the call rouses a
sleeping member off shift, and a raid that musters marches to its door. Under 18 never mustered, called or picked.
Shadow: gang_member, gang_leader, cultist, raider, scav (20k). Moves a number: raids that reach the door, corner
turnout, Succession lines, 20k seed 42 x 30 before and after; one 100k x 30 for raids landing if 20k cannot show it.

#### X5, the pit, the lines' ages and marriages (`OPEN-217`, `OPEN-248`), wave 2

Every bout hurts its loser (1,724 of 1,724 at 100k) and the patch-up (6-12c) costs more than the purse (5c): a loser
is hurt on a beating (the margin), not on every loss, and the purse pays more than a patch-up; the purse is the
house's money (a `Flow` from the venue's till), not `GambleWin` from the City. Bout, arrest, cell-fight and witness
lines print no ages, so the minor rule cannot be read on them: print the ages the other harm lines print
(`events::with_ages`). Marriages: 23,267 in 30 days at 100k, 16.7k in days 0-6 (r6 the same): find the seeding burst
(the founding's couples, `lod::stat_flirt`'s rate on a fresh city) and fix it at the cause, so a marriage is a
courtship's end and the first week reads like the fourth. Shadow: fighter, bouncer, madam, guard, worker (20k).
Moves a number: bouts, losers hurt, purse against patch-up, Marriage by week, 20k seed 42 x 30 before and after.

#### X6, the cell's day, the clinic's walk-in and the shadow's picks (`OPEN-217`, `OPEN-232`), wave 2 after X1

Jail days are empty: a held person's day in the cells has its routine and its lines (the meal, the yard, a visit from
kin, a fight that X1 gives a place and a case), and his release says what waits outside (the job gone, the Home, the
gang). Walk-in patch-ups still go to the nearest clinic (853 of 1,861 doctor-days with no patient at 100k): the
walk-in reads the staffed clinics near him as the bookings do (W6's `book_clinic`). The shadow's archetype pickers:
both `gun` picks are corp techs with no weapon, the `runner` picks are not gang members, no `arms_dealer` diary for
the fifth round (`citysim-cli/src/shadow.rs`): a pick is someone who does the archetype's thing in the window (a tool
asserts its preconditions: an empty pick is an error line, not a silent miss). Shadow: jailed, ripperdoc, orderly,
gun, runner, arms_dealer (20k). Moves a number: doctor-days with no patient, jail-days with a line, 20k seed 42 x 30
before and after.

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
- **V12 fix: the non-lethal backstop** (Shadow V12's critics; `kill_by` guards only deaths). `law::resolve_fight_mods`
  refuses any fight with a protected party (no rolls, no memories, no wounds; returns `(a, b, false)`; counted
  `harm_blocked`, so above 0 is a pick site that missed). The brawl pickers filter first: `missions::defenders`,
  `raid::gathered_for` (raiders, marchers, riots), `raid::jail_defenders`, the gang-raid defenders, and
  `raid::fight_out_until` drops a minor from either side as the last gate. The fare ladder: `fares::caught` turns a
  protected dodger off the platform (the existing turned-off rung: no beating, no fine, no ban record) and `beat()`
  refuses as the backstop (counted). Also fixed: the gang bin shakedown (`bins.rs`) skips a minor. Audited and
  already shut: tags' chrome mugging (`free_adult` pick), `wounded` (reached only through the fight), camp (the
  beating is gone).

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

### S2, plans that fail at the end (`OPEN-222`; `leaders.rs`, `gang::recompute_leader`, `crew::sat_muster`, `[brain] cool_backoff*`)

Diagnosed at 20k (seed 42 x 30, a debug print on every leader's `Collect`/`Court` that failed its start; 75 in 30
days): **every** `Collect` failure (18) was a body who **no longer led**: `gang::recompute_leader` re-ranks the
gang on a join, a release or the daily pass, and the post moved while he walked (V11 `gang_leader_96308` was a
lieutenant by d26). Of 57 `Court` failures, 42 were the leader at a lieutenant who was out (an off-screen one missed
on the Home draw, a body moved on) or asleep; 8 a court that had closed hours before he arrived (the El estimate,
capped at `court_walk_max`, set a leader 8 h off walking at 15:00 for a 19:00-21:00 court); 7 a lieutenant walking
to court who had lost his rank. Built:

- **A Lead walk for a post lost turns back** (`leaders::posts_changed`, called by `recompute_leader` with `[leaders]`
  on): a member walking a Lead plan for a post he no longer holds (the leader's round, Collect or court; a
  lieutenant's walk to court while he is not one) drops it where he is, with a Crew line ("Tomas Singh turned back
  on the way to Hideout#5034: Hesper Vance leads The Unplugged 2 now").
- **The order left at the door** (`can_court`, `court_start`, `Given::Door`): at a lieutenant's Home the Court step
  starts met or not; out, or asleep beside him, the order is left at his door (counted `orders_word`; "Adela Adeyemi
  left the order at Brenna Yardley's door (Brenna Yardley was asleep): Raid").
- **Court by the trip** (`sets_out`, with `[distance.choice]`): the body's own trip (`distance::trip` from where he
  stands) and a trip past `court_walk_max` is no court today (silent: the summoned still come, and are kept).
- **The door's reason**: a leader's `Collect`/`Court` abort names its check in the PlanAborted line
  (`leaders::door_why`: `not the leader`, `court over`, `no post`, `no longer a lieutenant`, `not beside him`).
- **A muster sat is kept** (`crew::sat_muster`, from `raid::depart`): a member who sat the muster to its departure
  kept the duty, whether or not the march reached the door (only the brawl's present had counted; V11
  `raider_39544` mustered 120 min, his march was cut, "missed" at midnight).
- **A want that cannot plan backs off** (`World::cool_ticks`; `[brain] cool_backoff` 2.0, `cool_backoff_max_ticks`
  720, `cool_backoff_reset_ticks` 1440; `Brain.cool_streak`): the nth cool of a goal in a row (no plan of it
  finished between) lasts `goal_cooldown_ticks x 2^(n-1)`, at most 12 h; a finished plan clears the streak, a day
  with no cool lapses it. General, not Hunt-only: `GoalKind::backs_off` keeps the flat 2 h for the needs and duties
  (Eat, Sleep, Work, Earn, Flee, Fight, Idle, Treat, Rescue, Patrol, Arrest, Squat, Pitch) and Visit (one goal for
  many errands, dropped when it cools). The diary's cool line says "backed off, N in a row" from the second.
- Device (measurement only, `run --legs`): `#failafter` rows (steps failed within 3 ticks of a done leg of
  `--legs-min` or more, by goal, action, reason) and `#goalplan` rows (plans installed and cools by goal).

Numbers: 20k `isle_m_k2` seed 42 x 30 (`--scaled --ambient-flats 50 --legs --legs-min 120`), one run each, base
`9dce338` plus the device (`386ef7c`). **`PreconditionLost` within 3 min of a leg over 120 min: 679 -> 338**; by
step, Lead Court 27 -> 12, Lead Collect 10 -> 0 (Lead aborts anywhere 77 -> 27, every one "not beside him": a body
lieutenant who moved on), Treat Patch 324 -> 6 (not this phase's mechanism: Treat keeps the flat cool; S4's area,
read as the run's divergence), the rest flat (Patrol 74 -> 72, Shop BuyAsset 64 -> 57, Sleep 40 -> 43, Raid Brawl
21 -> 35 with the raids up). **Hunt goals per Hunt plan** ((plans + cools) / plans) 2.29 -> 1.80 (531 plans and
683 cools -> 443 and 355); the other wants' cools: Bury 4,442 -> 1,679, Shop 3,145 -> 1,560, Contract 2,992 ->
1,278, Unwind 6,126 -> 5,764; Work 409 -> 453, Eat 86 -> 99, Sleep 146 -> 86 (flat cools). Courts held 307 -> 245,
orders by word or door 7 -> 26, Lead rounds 128 -> 195. FINDING (not tuned): Raid events 50 -> 82 (orders in force
sooner), muster misses 88 -> 115 (1.7 -> 1.4 a raid), Murders 47 -> 62, Assaults 1,244 -> 1,408, thefts 989 ->
1,216, homeless max 562 -> 734, employed mean 11.28k -> 11.37k, starvation 0 -> 0. The after-run had Visit backing
off (exempted after it; Visit cools 7 -> 15).

- **Shadow** (20k, d21-27, gang_leader, buyer, vendor, raider, gun, gang_member x2, and seven picked from the run's
  long Lead, Hunt and Raid legs). Working: a leader's day of rounds and court (`agent_11007 d22 16:27` GoTo 187 min
  to the Hideout, `19:34` Court 86 min, `19:35` "gave Gwendolyn Thorne the order at court: Expand", `21:00` "held
  court at Hideout#5510: Mags Ambrose, Nadia Garland, Hale Pembroke came"); `agent_27793 d21 20:56` Muster 64 min,
  `d22 02:03` Brawl after the raid, and no "missed" at `d22 00:07`; a hunter whose target is out of reach backs off
  (`agent_11007 d27 21:19` "cooled until d27 23:19", `23:27` "cooled until d28 03:27 ... backed off, 2 in a row");
  `gun_18205` "goal Contract cooled until d22 06:06 (... backed off, 5 in a row)" (was every 2 h). The V11 buyer and
  vendor flicker did not come up in the 20k picks (no Hunt in either buyer or vendor).
- **Deviations.** Collect: "re-check before setting out" cannot see a post that moves on the way, so the walk turns
  back at the change (the brief's "hold it"); the walk's length to a far Hideout is unchanged (no reach on the Lead's
  Collect or round; `OPEN-222` keeps it). Hunt: the general back-off rather than a per-target cooldown (the streak
  is per goal; a heavier grudge waits out the cool). Court with the choice off keeps the capped estimate.
- **Reads wrong** (the next pass's input): leaders still walk 4-8 h to a round's first stop (`agent_11007 d21 10:47`
  407 min to Gambling Den#5725, `d24 09:26` 278 min): the round has no reach. The street chase of a body lieutenant
  ("not beside him", 26 a month). The leader ranking churns (a join or the daily pass moves the post) with no line
  of its own; `turned back` is the first the diary hears of it. Raid Brawl failures after the march up 21 -> 35.
- No test (doctrine 2). `SAVE_VERSION` 81 (`Brain.cool_streak`, `[brain] cool_backoff*`).

### S7, the diary's noise (`OPEN-225`; `citysim-cli/src/shadow.rs`, `crew::raid_result`, `[noise]` S7 keys)

- **The raid result.** `purist_56353`'s raid was a breakout (`raid::breach`, the Precinct), and only `brawl` and
  `corp_brawl` wrote `crew::raid_result`; breach's own line named the gang, the jail and the head. Breach now writes
  the result for every raider and guard ("Raid on the Precinct: the Precinct held (4 raiders, 66 defenders); nobody
  freed; 6 hurt: Smoke Thorne, Ulric Voss, Rhoswen Marlow, Odile Jessop and 2 more; The Unplugged 5 next:
  BreakOut"), with `RaidLeft.freed` (the convicts freed, "2 freed: A, B"; the guards it beat hold no crew grudge,
  `law::on_breakout` answers a breakout as before). Every result line counts its toll, "N dead: ...", "N hurt:
  ...", `[noise] raid_names` 4 named and "and N more". **Deviation:** no "arrested" or "turf" in the toll: a raid
  arrests nobody in its minute (the reports come after; `purist_56353` was taken 6 h later) and no raid moves turf
  (`settle` takes coin, food and goods). "the The Static Rats Hideout" reads "the Static Rats Hideout".
- **The minute's folds (the shadow).** Crowd events of one shape in one minute fold into the first line, the count
  and three names (`fold_shape`, `FOLD_NAMES` 3): "Witness: 58 saw Yuki Ellery commit Assault (Kasimir Inchbald,
  Marek Jessop, Emeric Fletcher and 55 more)", "Assault: Yuki Ellery attacked 2 at the Precinct (Ulric Voss, Piers
  Ellery)" (a pairing that killed stands alone), `Report` ("N reported X for C") and `WitnessReacted` ("2 told 4 of
  their people what X did (the Grand Theft)", the told summed; "N saw X's C and ran"). The money line's `events:`
  and the notable list carry the folded line. The head's older summary of the same brawl ("G raided X: Won (...)",
  "G stormed the Precinct: ...") is not repeated when the minute has the result line (it stays in the event log;
  `analyze_run.py` parses it). Memories and edges past four in a minute show three and count the rest by kind
  ("... and 55 more relationship changes in the same minute: 53 new Acquaintance, 2 new Enemy"; four show as four).
- **The hour's folds (the shadow).** Lines that carry little alone (a new Acquaintance, an Acquaintance's affinity
  drifting, a `MetInJail` memory, a retelling of my deed its hearer knew) wait for the hour (`Quiet`,
  `flush_hourly`): four or fewer print at their minutes, more fold one line a kind at the first ("75 new
  Acquaintances this hour (A, B, C and 72 more)", "met 4 in the cells this hour (...)", "GOSSIP ABOUT ME: N
  retellings this hour, every hearer knew already (told by ...)"). Kind changes and every Friend, Rival or Enemy
  move print at their minute. V11's 100k cells wrote ~1,200 lines a day this way, one or two a minute, past any
  per-minute fold. A heard `Sighting` refreshed hourly (a colleague at his bench, 40 a week in `worker_6654`)
  prints once a day per subject and place.
- **The repeated witness line** (`gang_leader_18246` 18:43, five days) is a re-raised crime, not a re-read record:
  the dodger jumps the same gate on his evening commute and the witness passes it at the same minute, every day; the
  6-h `[life] witness_dedupe_ticks` let each day record afresh (a memory and -0.2 affinity: a stranger sank to Enemy
  -1.00 over a fare). A civilian holding a sighting of the same actor's same petty crime (`law::is_petty`) from the
  last `[noise] petty_rewitness_days` 7 takes no new record (`law::raise_crime_on`, `life::saw_within`): no line,
  memory or affinity hit. The law's eye always acts (the fare ladder and S3's `beat_cooldown_days` are untouched).
  20k seed 42 x 28: Fare Evasion witness lines of a witness and dodger already paired 817 -> 517 (the rest are guards
  on duty, and memories evicted past `memory_cap`). **Found on the way:** `memory::insert` merged a `SawCrime` of a
  second crime into the first (same kind and subject inside 60 min, the first crime kept), so `saw_recently` never
  found the second: an arms dealer's ten witnesses were recorded afresh every minute of her Dealing after one Arms
  Dealing (100k `arms_dealer_40733 d24 00:00-00:08`). The merge now needs the same crime.
- **"Promoted to X" when already X.** `classpay::title` read "Senior" at grade 1 whatever the role's base, so a
  Reporter (`role_grade` 1) raised to grade 2 by his bylines read "promoted to Reporter (was Senior Reporter)". The
  title is by the grade above the base (`grade_of`): the label, "Senior", "Lead". `note_promoted` takes what the
  body held (`classpay::held`): "promoted to Senior Reporter at X (was Reporter)", "promoted to Manager at X (was
  Vat Tech)"; a rise under an unchanged title reads "rose from skilled to professional grade as T at X (8c a day)",
  a graded move at the same title and grade "moved up to a T post at X".
- **The rest day in the hour lines.** The two-hourly needs line ends "| rest day: no shift" (or "| the city's rest
  day") on such a day, except while the body is at Work (a night shift's tail runs into the rest day's morning,
  `worker_6651 d27 00:00-06:00`), and the day's heading reads "## Day 27 (rest day: no shift)".
- **Diary size** (lines a diary-day, the `## Day` sections; 20k seed 42, purist, raider, gang_leader, reporter,
  worker x2, d21-27; the same picks and lives, the two sim changes off): median 100 -> 91, max 154 -> 154, total
  6,383 -> 6,131 (-4 %). These 20k picks hold no raid and no crowd; the folds bite where V11's 100k floods were. 100k
  seed 42 (purist, jailed, arms_dealer, raider x2; the city has moved since V11 and S3 emptied the cells' meeting
  hall): median 77, max 137 a diary-day, against V11's medians of 1,204 (purist), 1,192 (arms_dealer), 1,014
  (raider); the hour's fold took 529 Acquaintance lines into 25 there.
- **Reads wrong still (next pass):** a big workplace's crowd is "new Acquaintances" again every few days (edges decay
  and re-form: `Thea Draper#6275` new on d22, d25, d26 in one worker's diary, 56-75 a shift start); a guard counts
  as a witness of the raider's assault on him (`Kieran Draper saw Rin Garland commit Assault`, d24 07:41); a 20k
  cell still makes Friends at +1.00 inside 12 h (`raider_19825 d21 04:00-12:00`, S3's area).
- No test (doctrine 2). `SAVE_VERSION` 81 (the `[noise]` S7 keys).

### S4, care at 100k (`OPEN-216`; `customers::stat_care`, `care::asks`, `service::super_leg`, `[care]` and `[presence]` S4 keys)

- **The diagnosis** (one 100k seed-42 run of 10 days with temporary prints, removed). Not distance and not a
  per-district cap. (1) **The patients are off screen**: the 100k city is 2,500 Full, 7,500 Coarse and ~90,000
  Statistical, and `care_choice` (the Treat goal's pick) answered `None` for a Statistical agent. At noon 50-100 of the
  wounded and 15-30 of the diagnosed were Statistical against 0-14 and 1-2 with a body; the off-screen wounded had no
  care path at all, and the off-screen diagnosed treated only on the midnight `stat_treat` roll, at the nearest
  Clinic *open or not* (a shut one, or one still being built), with nobody at the table. 20k has the same tier split
  (10 % bodies), so R3's 20k reading was a lucky pick, not a scale effect. (2) **Founded Clinics stand shut for
  days**: the founders' 30-odd Clinics of days 0-3 are construction sites (`industry::start_site` withdraws the
  vacancies until the Yard's crew is done, `crew_days` x up to `crew_slow_max`), so 22 of 38-58 Clinics were staffed
  until day 4, 47-60 after; open Clinics close at 18:00 (a day shift) and the wounded of the night go to the back rooms
  (on call). (3) Of ~60 staffed Clinics only ~10 have a doctor with a body: a Clinic's patient is almost always an
  off-screen doctor's. Distance after S1 is not it: the bodies' care trips that planned reached the door.
- **Off-screen patients are seen where a body would go** (`customers::stat_care`, hourly from `care::hourly`): a
  Statistical adult with a fresh wound (`hurt`), or a diagnosed cancer the pool, a pawn or the slate can settle, takes
  `care_choice`'s place for his tier (`care_choice_any`: the Clinic for the patch or the slate treatment, the back room
  for the gang-hurt, the wanted and the broke, the back room's cut rate), and a diagnosed cancer the pool pays for the
  nearest open Clinic. Seen there when a doctor is on duty with room in his hour (`[[serves]] per_hour`; a back room's
  doc on call counts his own walk-ins, `service::take_room_on_call`): paid, seen to and written as a body's `Patch`
  is (`see_to`, split out of `patch`), so the doctor's diary has the patient. A cancer is seen on the day's
  `[sickness] p_stat_treat` roll, once a day (`Misery.patched_at`). With `[care]` the midnight `stat_treat` is off.
  **Deviation:** the patient is not given a body (the counter's off-screen visit is the precedent): a doctor on
  camera treats a patient who is never on screen.
- **The doctor named is the one who serves** (`service::server_for`, `customers::doctor_for`): the `Treated` event's
  doctor (and the fee's cut) is the on-duty server `service::arrive` hands the patient to, not the lowest id on duty
  (V11 `orderly_18123`: three "treated X (a patient)" memories and no `Treated` line). The install, the Therapy and the
  Detox events name him too and make him an actor ("had Legs implant T1 installed at Ripperdoc#5037 by X").
- **The sick ask their own, more than once** (`care::asks`; `[care] ask_tries` 3, `ask_refused_days` 7): the night's
  ask is a ladder down the Friends at or above `ask_affinity`, warmest first, until the shortfall is covered; one with
  nothing above `ask_keep` says so, one who refuses is not asked again for a week; the kin are named as the pool they
  already are; with no Friend left the line says so (the street, Earn's begging, is what is left). One line a night
  of asking: "Benedikt Wozniak, 175c short of a vat-cancer treatment after the 65c they and theirs could put in (24c of
  their own, Rue Quarry 7c, Rhys Mercer 34c), begged their friends: Leopold Ashcombe lent 2c; Elowen Tanaka turned them
  down: still 173c short". **Deviation:** the spouse is not asked for coins (his purse is in the pool already); he is
  named in the line.
- **The super's day** (`[presence] round_post_stop`, `super_legs`, `super_repairs`, `super_complaints`,
  `super_leg_days`; the super's trade row `shape = "stations"`): a round with no other Block of the landlord's within
  `round_reach_tiles` is the post's own Block (its tenants served: notices, door rent; "walked Block#230 door to door,
  the only Militech 3 Block near"), and the rest of the shift past the round is worked in stations blocks at the post,
  each block a leg drawn from `super_legs`: a repair in a tenant's flat, a tenant at home stopping him in the hall
  about one of `super_complaints`, the stairs walked. A tenant seen to is not the next leg's for `super_leg_days`
  (a Served memory both sides). V11's cause was not arrears: `super_104162`'s post had no other Block in 20 tiles,
  so the round was empty and the shift one `ClerkWork`.
- **The rule** (S6's): a cancer under 18 never kills (`health::daily`: the stage holds at its end; at 18 it runs on).
  A child (`Child`) carries no cancer, so "a sick child goes to care" has no case yet; an adult under 18 goes as anyone.
- **Numbers** (100k `isle_m_k2_100k` seed 42 x 10, `--scaled --ambient-flats 50`, one run each, base `9dce338`):
  `Treated` events 115 -> 1,463 (at a Clinic 57 -> 287, of them patches 23 -> 257; back rooms 58 -> 1,176), `Cured`
  12 -> 36 (the slate's cures at a Clinic), `Loan` lines 145 -> 392, `SuperRound` 80 -> 85 (4 a post's own); violent
  deaths 87 -> 75, arrests 832 -> 880; ticks/s 24.8 -> 21.7 (shadows and builds on the box at once: a print).
- **Shadow** (seed 42 d21-27; 20k `isle_m_k2` sick, ripperdoc, orderly, black_doc, super x2; 100k ripperdoc,
  orderly, sick, super x2). The Clinic at 100k: `ripperdoc_104417 d21 10:52` "Treated: Thea Carrow patched Idris
  Moreau up at Ripperdoc#5039 for 12c (3c of it to the doctor)", four more on d24-25; `ripperdoc_60222 d25 13:42`
  one (V11: none in four clinic workers' six work days). The orderly's line at 20k: `orderly_12191 d26 13:34` "Yuki
  Ellery patched Ysolde Kaneda up at Ripperdoc#4978 (3c of it to the doctor)", with his cut and his memory. The back
  room: `black_doc_8530` 62 `Treated` in the week ("patched Dagger Penhallow up in the back room of Back Room#3095
  for 6c, no questions asked", every hour or two through the night). The ask: `sick_10650 d21, d24, d27 00:00` "Nico
  Park, 223c short of a vat-cancer treatment after the 17c they and theirs could put in (6c of their own, Hale Haas 6c,
  Jasper Bartlett 5c), has no friend left to ask"; at 100k (events) the ladder: "Hana Tanaka ...: Nico Ellery turned
  them down; Idris Penhallow turned them down; Odile Jarrow lent 6c: still 202c short". The super: `super_105346 d21
  11:42` the round and its Notice, then "Sebastian Yardley stopped Benedikt Bellamy in the hall of Block#3526 about the
  smell in the stairwell" (12:47), "fixed the wiring in Rhys Ellery's flat" (`d22 13:47`), 30-33 legs a week in blocks
  of 30-90 min with the break (was one 538-min `ClerkWork`); "Kestrel Lockridge walked Block#518 door to door, the
  only Habitat 5 Block near" (20k events).
  **Reads wrong / next pass:** both 100k orderlies still show "treated X (a patient)" memories with no line in the
  shadow (that build named no doctor on an install; the install, Therapy and Detox lines name him now: "had Legs
  implant T1 installed at Ripperdoc#4978 by Yuki Ellery", 20k events, not re-shadowed). Neither 100k `sick` pick is
  diagnosed (the archetype takes the malnourished), so the cancer ladder is read at 20k and in the events. The off-
  screen patient never appears on camera. Clinics are quiet against the back rooms (the poor, the gang-hurt and the
  night's wounded go to the back room: 1,176 of 1,463 at 100k); a ripperdoc sees ~1 a day, a back-room doc ~10. Most
  legs are the stairs (the tenants are at work by day and one seen to waits a week), and the leg's line comes at the
  block's end. Founded Clinics stand shut days 0-3 at 100k (construction, untouched).
- Re-pinned with the reason in place: `sickness::test_stat_patient_treats_on_p_stat_treat` (the midnight roll, `[care]`
  off: with `[care]` the off-screen patient treats at an open Clinic's hour) and
  `person::probe_laid_off_farm_worker_finds_work_within_14_days` (`[care]` off: the moved seed-42 city abducted and
  killed the pick on day 26, a butterfly) and `succession::test_brothers_after_a_gutting_sack` (the whole gang is the
  clone before the trim: the trimmed Hollow lost its two kept members in the moved city). A walk-in seen off duty
  writes no service room with `[service]` off (`service::test_off_city_writes_nothing`).
- No test (doctrine 2). `SAVE_VERSION` 81 (`care::State.refused`, the `[care]` and `[presence]` keys).

### T3, plans that still die silently (`OPEN-222`; `contracts.rs`, `law::chase_dropped`, `exec::try_enter`, `crew::sat_muster`, `missions::brawl`, `EventKind::Pursuit`)

Diagnosed at 20k (seed 42 x 30, temporary prints, removed) and from the V12 100k diaries. Each case at its cause:

- **A held contract that never plans.** Two causes. (1) `gun_36512`'s record was a **Steal on a Lab**: a ledger
  kind (`render_for` sends Steal, Persuade, Deliver, Spin, Vote, Retake and an unshown Tag to the ledger), but the
  shadow's pin made the taker a body and `on_promoted` (and `resolve_ledger`'s re-queue of an organic record with a
  body in it) turned it live; the Contract plan's chase has no walk to a building (`hunt::target_ok` on a Lab), so
  the goal cooled "7 in a row" to the deadline. Built: `ledger_kind` (`render_for`'s first test) and neither path
  turns one live; a run that no walk works (a ledger kind, a building for a mark) goes back to the ledger with a
  `due` and a `Pursuit` line ("X works the steal on Lab#4915 off the street: no walk plans it (due dN hh:mm)").
  (2) At 20k the cools were **Locates and Tags on a jailed mark** (955 cools in 30 days, 0 with this build): the
  chase gate refuses a mark in the cells. `mark_away` / `mark_out_of_reach`: a mark out of the cells before the
  deadline is waited out (the Contract goal cooled to the release, one line: "Sable Dunmore holds the locate on
  Hana Ironside and waits: Hana Ironside is in the cells till d26 17:06"); one out after it, dead, leaving or abroad
  is let go (`fail_attempt`, the reason in the line: "locate on Imani Ferreira: failed (Solene Lindqvist let it go:
  Imani Ferreira is in the cells till d31 10:53)"). `World::cool_goal` keeps a longer cool the plan set itself.
- **An arrest chase that ends silent** (`law::chase_dropped`, from `think`'s abort of an Arrest plan and from a
  failed Arrest step): one `Pursuit` line says why (already in custody, the warrant closed, the trail went cold, another guard
  has the chase, out of reach, the shift is over, another want first) and, while the suspect is wanted, located and
  unclaimed, hands the case to the nearest free guard at work on shift who can chase him (`handoff_guard`: the same
  force, his pursuit radius, the sighting fresh to him, not on the take) as his collar for `[justice] collar_hours`
  (`World.handoffs`, read by `collar_of`), else "no guard on shift near; the warrant stays open". A failed step with
  the chase still on says nothing (the guard re-plans it). V12 `guard_8440 d23 11:10`: his own collar (S3) lapsed at
  2 h while he chased off shift, and the in-shift factor went to 0.
- **A Guard's 0-tile GoTo timeout** (`exec::try_enter`): the Precinct's door queue (`door_capacity_per_tick` 2) at a
  shift start. 20k: 66 guards entering for Work from 08:10 held the door at 2 a tick to 08:44 and the 67th timed out
  at `door_queue_max_ticks` 30 (2 in 30 days at 20k; `runner_90375` 25 in 6 days at 100k, five times the guards).
  With `[exec] staff_door` on, a building's own staff go in by the staff door, past the public queue (the key's
  comment extended; no new key).
- **A mustered raider "missed"** (`crew::sat_muster`): V12 `raider_70326`'s d21 miss was a **court summons**, not
  the muster: the raid was ordered after midnight (`raid_ordered` posts no muster over a member's day duty), he
  mustered 353 min at the very Hideout he was summoned to and marched, the court was never held, and midnight
  booked the summons missed. A member who sits his gang's muster now has the day's other duty of that gang turned
  into the muster, kept (the court's pending Visit dropped). His `d24` "missed ... guard" was a duty owed by a member
  who led the gang by midnight (the leader is posted none): no longer booked.
- **A contract brawl that hits three not the target** (`missions::brawl`): the pairings ran by strength, so the
  crew's strongest beat Anselm Dunmore's people one by one and never reached him. The crew now goes for its mark
  first (the mark moved to the front of `defenders`), and a Beat is done once the mark lost his pairing ("beat him,
  then lost the door" when the rest held). **Under-18:** the list is `defenders`', which drops every
  `ages::is_protected` body (`fit`), and `fight_out_until` drops them again before the pairings: the order changes,
  never who may be hit. `raid::crossfire` had **no** age gate on its bystander pool (bodies near the door and a riot's
  Statistical draw): both now drop `ages::is_protected` (20k `harm_blocked` 0 -> 0). The crew hears how its job
  ended: `ContractFailed`/`Fulfilled`/`Expired` carry the crew as actors ("hit on Mako Moreau: attempt 1 failed (The
  Drowned Choir 8's crew hit Mako Moreau at Block#248: won (2 vs 1, 0 dead); Mako Moreau survived)" in
  `raider_11724`'s diary).

Numbers (20k `isle_m_k2` seed 42 x 30, `--scaled --ambient-flats 50`, base `7a56d67`, one run each; prints, nothing
claimed but the silent cools): Contract cools on a run that could not plan 955 -> 0 (each now a wait or a let-go
line: ~85 waits, ~90 let go); Precinct door timeouts 2 -> 0; `Pursuit` 1,216 a month (the arrest ends: the trail
went cold 681, out of reach 171, already in custody 113, the shift is over 100, another guard has the chase 73; 49
handed on, 16 of the handed collars cuffed); court summons missed 389 -> 297; 23 day duties turned into a sat
muster (17 a court summons; the run before Welcome and Crew were excluded). FINDING (not tuned, the runs diverge):
Arrests 1,508 -> 1,438, Assaults 1,451 -> 1,229, Murders 65 -> 67, Raids 83 -> 60, ContractExpired 115 -> 68,
ContractFailed 142 -> 204, homeless max 779 -> 686, starvation deaths 0 -> 1, `harm_blocked` 0 -> 0.

- **Shadow** (20k, d21-27, `--pick guard:14,raider:10,gun:8,runner:3`). Working: `gun_20043 d26 05:46`
  "ContractFailed: locate on Imani Ferreira: failed (Solene Lindqvist let it go: Imani Ferreira is in the cells till
  d31 10:53)"; `gun_21749 d26 13:29` "Sable Dunmore holds the locate on Hana Ironside and waits: Hana Ironside is in
  the cells till d26 17:06"; `raider_19389 d23 05:50` the same for a tag; `guard_7793 d26 18:23` "Zelda Draper gave
  up the chase of Ilse Chrome (the shift is over): no guard on shift near; the warrant stays open" (V12's
  `guard_8440` case, said); `guard_25123 d23 15:29` "(he is out of reach)" (the shadows ran before the reasons lost
  their pronoun: a suspect is often a woman); the run's hand-ons ("Zara Voss gave up the
  chase of Tobin Crane (out of reach): handed to Lux Mercer on shift, 2 tiles off"); `raider_11724 d24 22:28` hears
  the crew's job end ("hit on Mako Moreau: attempt 1 failed (The Drowned Choir 8's crew hit Mako Moreau at
  Block#248: won (2 vs 1, 0 dead); Mako Moreau survived)") though he slept through it; no GoTo(Jail) Timeout in 16
  guard and runner diaries (V12: 25 in one).
- **Deviations.** The contract: "plan it" for a Steal is the ledger's roll (a body taker has no heist walk without
  a deck), said. The chase "hands the case on" to a named on-shift guard as his collar, not to the Precinct's board
  (that is the warrant, which stays open either way). The raider: the miss was the court summons, so the fix is the
  duty, not the muster. The brawl: the mark first is an order, not a new target pick.
- **Reads wrong** (the next pass's input): most chases go cold (681 a month): at walking pace a guard reaches the
  last-seen tile after the suspect left it and `arrest` needs him beside the guard (an intercept or a radio
  sighting, not a line); nine guards chased `gun_21749`'s Sable Dunmore over four days, each "the trail went cold".
  A mark in the cells is still taken (the take gate does not read the cells: `gun_20043` took the locate on Imani
  Ferreira at d25 18:00 and let it go at 05:46), and a corp's Tags and Locates stack a dozen holds on one jailed mark
  (Ada Bellamy). The Steal under a pin and the brawl's "beat him, then lost the door" did not come up at 20k (5
  mission brawls in 30 days); 100k was not re-run (V12's 21+7 days at 22 ticks/s is past the foreground cap). The
  `Pursuit` lines are T7's to fold.
- No test (doctrine 2). `SAVE_VERSION` 84 (`World.handoffs`, `EventKind::Pursuit`).

### T1, neglect warns first and the Camp is a place (`OPEN-231`; `camp.rs`, `household.rs`, `service::civic_hour`, `[camp]` T1 keys)

**The ladder (Dylan 2026-10-27: neglect warns first, a repeat inside the window takes).** `camp::take_daily`: a
child at home at `take_days` (2) unfed brings the service to the door: one `Custody` event a Home ("the child
protective service came to the door of Block#3641: Kieran Lindqvist (11 y), Fenn ... went 2 days unfed; Fiora
Oyelaran and Ulric Lindqvist heard it: 3 days' grace, and unfed again before d36 the service takes them"),
`Child.warned_day` set, the unfed count started again. Unfed again to `take_days` inside `warn_window_days` (14) and
past `warn_grace_days` (3) is the take; past the window a neglect is a fresh warning. A street child (no door) is
taken as before; `warn_window_days = 0` is the base (the camp's own probes run so). **The parent's side of it:**
the first free parent short of the grace's food money (`warn_grace_days` of the children's food at his meal price)
asks up to `warn_ask_tries` Friends at `warn_ask_affinity`, each lending above `warn_ask_keep` on `affinity x (1 -
greed / 2)` (an edge debt; the line names each). **The household keeps the children's share** (`camp::adults_share`,
read by `EatAtHome`/`PackMeal`, the GOAP pantry and the Statistical meal): on a warning the adults eat only what is
above `ages::children_food_need`, and the grocery run (`routine::run_level`, `lod::stat_grocery`) counts the pantry
alone, not food carried for a shift. **Deviation:** the spec's ladder alone moved little (ChildTaken 654 -> 389): of
318 warnings, 163 were Homes whose parents held the food money (Block#239: four parents, 225c, taken three days
later); a Block's adults ate the pantry bare before the children's midnight meal. The share is what moved it.
- **Let home:** `let_home_daily` (midnight, before the ladder): a Camp child's count of midnights a parent's Home
  (standing, the parent free and adult) held `home_pantry_days` (3) of the children's food, his other Camp children
  counted in; at `let_home_days` (7) the parent's children go home together, on a warning ("the service let Wire
  Ostrowski, Moth Ostrowski, Hale Ostrowski go home from Work Camp#4835 to Bertram Ostrowski at Block#927: the
  pantry has held their food 7 nights running (home on a warning: ...)").
- **Placement by reach:** a take prefers (after "can feed tonight") a Camp within `[distance.choice] visit_ticks`
  of the child's Home by the first parent's `distance::trip_ahead`, then corp before city, then the nearest. "Could
  not make the trip" stays when true.
- **The warden's day:** the headcount at a shift's first hour is one `Custody` line naming the comings and goings
  since the last count (`CampState.since_count`: brought in, slipped out, caught, let home, bought out, broken out,
  came of age) and last night's meals (`reserve_today`, the unfed); a child's Inmate encounter is no warden memory
  (as a pupil's). A day try at the fence (`p_abscond` 0.004 at a hashed hour in `abscond_hours` 9-17; the night's
  `[service] p_camp_escape` stands): a warden on duty catches it on `p_abscond_caught` (0.75) and walks it back in
  (no fight, no blow: nothing calls `law`; the under-18 rule), else it slips out "past <warden> in the day". The
  visit names the warden who let the parent in (an actor: his diary has it); a failed break-out is a `Custody` line
  naming the wardens who held the door (the fight is adults only; the child is never a party).
- **The parent's side:** each weekly answer says what was weighed (`household::weighed`): "the 60c buy-out beyond
  the 9c in hand", the pantry short of a week of the child's food, the break-out's crew or nerve; a buy-out says the
  days in and the purse.
- **Numbers** (20k island seed 42 x 30, `--scaled --ambient-flats 50`, before = main 7a56d67, one run each):
  ChildTaken 654 -> 105 (52 of them street takes, 32 before); daily ch_camp mean 304.5 -> 24.0 (d29 591 -> 82);
  warnings 616 (children), let home 5, absconders caught 2, escapes 30 -> 4, visits 61 -> 16, buy-outs 0 -> 0;
  ch_street mean 59.5 -> 89.6 (d29 85 -> 159); starvation deaths 0 -> 0, mean hunger 0.636 -> 0.636, harm_blocked 0.
  Without the share (the ladder and the ask alone): ChildTaken 389, ch_camp 129.
- **Reads wrong / open:** street children rose (a family kept home through the warning is later evicted; its
  children are on the street until `street_take_days`: `homeless_11532`'s three all week); the ladder does not reach
  a homeless family. "Nobody in or out since the last count" is a daily warden line in a quiet week (T7's fold).
  Buy-outs still 0: no camp parent holds 60c (`buyout_fee`). A camp parent's diary rarely shows the warning itself
  (the pick is at day 21; the door came earlier): the `[family]` line carries "on the service's warning since d19".
  The camp tests' fixture takes at `take_days` (`warn_window_days = 0`, the reason in place);
  `test_unfed_child_taken_at_take_days_before_death` walks the ladder. No new test. `SAVE_VERSION` 84.

### T6, money and labels that disagree (round 4; `customers.rs`, `leverage.rs`, `industry.rs`, `corp_brain.rs`, `service.rs`, `care.rs`, `demography.rs`, `virt.rs`)

Six lines whose money or names disagreed, each fixed at its cause:

1. **The back room's bill** (`miserable_64146 d22 05:10`, "for 80c" with 0c in the purse and a 1c flow to Cipher
   Santoro). Not a pay bug: the back room's fee is drawn from the pool (`health::payers`: the patient, kin, the
   gang's treasury), and the patient had joined The Hollow, whose treasury put up the rest; the flow went to the
   room's owner, not the doctor. The line now says so: `customers::money_note` appends who put the coins up when the
   patient did not pay them all and whose till took them when it is not the doctor's ("for 80c (38c of their own,
   Omar Kaneda put up 5c, Cato Delacroix put up 37c)", "for 6c, to Ronin Santoro's till, no questions asked").
2. **The con with two sellers** (`vendor_11367 d24 00:09`). Two real cons on one tick: Omar Oakes's fake job of d17
   came due (`fake_due`: the Defrauded line and the grudge on Omar, right) and the same midnight's `fake_pass` sold
   the just-burned mark Kenji Blackwood's (the open-fake-job filter had just been cleared). `[leverage]
   fake_wary_days` (14): a mark holding an unsettled `Betrayed` grudge that young buys no fake job (`leverage::wary`).
3. **The foreman who had just lost the shop** (`ripperdoc_61226 d27 00:11`). `seize` hands the business to the gang
   and the owner-operator stays on as staff, the longest-serving, so `foreman` named him. `industry::picker`: in a
   gang's house (the owner a Gang: a front, a seizure, a fire sale) the gang's own man picks, its member on the staff
   (lowest id), else its leader, and the line says for whom ("picked by Rook Marlow, for The Salt Widows");
   `careers::foreman_actor` reads the same. Elsewhere the foreman as before.
4. **The CEO's call five times in 70 minutes** (`ceo_88522 d24 00:07`). Each shock rescore re-made the exec's
   "keep" call (the same hash of his traits all day) and logged it. `Corp.kept_call`: a keep call is told once per
   standing order, cleared when the order changes. The line also says where it was made: "at the table: ..." only
   inside the office hours with him at the Hq (`exec_present`); else "called from Block#198" (a body) or "off the
   table" (off screen), with "the table last sat: ..." when one had.
5. **The exec hired as an Engineer** (`exec_78363 d26 00:00`). The sick-cover pick (`care::cover_pick`) took any
   jobless adult, and an exec holds no Job. It leaves the execs out, and `demography::hire` refuses an exec as it
   refuses a child (the backstop for every pick beside the labour pool, which already left execs out).
6. **The ledger as free money** (`fixer_100695`, 600c four times a week at heat 0). A ledger run that won every
   contest left no trace at all. `virt::audit_ledger`: a ledger's books (a corp's, the city's) catch each run as a
   shortfall in the same line ("Nutrix's books came up 750c short, the hand unknown") and keep `(tick, runner)` in
   `World.ledger_books` for `[hack] ledger_audit_days` (14); the runner's `ledger_audit_runs`-th (2) run on the same
   books inside the window is matched (the same deck on each: the books, not a roll) and he is named as a naming
   trace names him (`named_by_books`: the Sighting, `DataTheft` filed so he is wanted and his heat rises,
   `last_seen` and `last_trace` at the chair). One run is free; a habit on one ledger is not. 20k seed 42: Rue
   Eastwick's second 750c off the city's ledger in a week was matched (`d12 22:01`), reported, arrested and
   sentenced to 5 days inside the day; a Story ran. 100k (V12's six ids re-shadowed, d21-27): `fixer_100695` is
   arrested for Data Theft `d21 11:32` (matched before the window), 5 days, heat 0.40 through d27, posts a guard on
   himself, and his one run after release is "Nutrix 2's books came up 600c short, the hand unknown".

Read at 100k on V12's ids: `miserable_64146 d22 05:32` "for 80c (The Hollow put up 80c), to Mbeki Holdings's till
(no cure)"; `vendor_11367` one Defrauded (d25), no second sale that night; `ceo_88522` no decision line in the week
(the order stood at Grow; V12 had 15 "keep Grow in Tech"); `exec_78363` no Hire, ExecWage only; `ripperdoc_61226` was
not repossessed this run (he picks as the owner's foreman, right), the front's picker read at 20k ("picked by Rook
Marlow, for The Salt Widows").

Deviations: case 2's critic read ("the grudge and the report land on the wrong man") was wrong; Omar Oakes did sell
the first fake job. Case 6's "a ledger that runs short" is the books' shortfall line and the existing ICE-after-loss
(`ice_target`), not a smaller take. The 20k runners spread over many corps (Rue Eastwick hit six ledgers once each)
and repeat on the city's, so the city's books are kept too. `SAVE_VERSION` 84 (`Corp.kept_call`,
`World.ledger_books`). No test (doctrine 2).

### T4, the trades' inside: the door and the yard (`OPEN-217`; `[verbs]` T4 keys, `service.rs`, `customers.rs`, `vice.rs`, `leisure.rs`, `streetwork.rs`)

- **The 100k diagnosis** (from V12's diaries, then two 100k shadows). The LOD tier, as with S4's care. (1) **The
  door's ledger went to an off-screen colleague**: `customers::door_visit` credited every arrival to the *lowest id*
  of `trades::door_staff`, which counts a Statistical bouncer present on any day. V12's `dancer_79123` and
  `bouncer_95519` kept the same Strip Club#5671 on the same nights: she served 18 visitors there, and he had no door
  line in seven nights. Refusals and rough clients went by a hash over all the present door staff, so the body took a
  share of a share. (2) **A Club whose only body was its Host took its evening in one 21:00 batch**:
  `leisure::queue_at_counter` drew the off-screen visits into a *counter* body's shift only (the bouncer is a counter
  body; the Host and the dancer are not), so `club_staff_36728`'s "served" memories are all at 21:00. (3) **The
  floor's visits went to the colleagues**: a 100k Club or Strip Club has a dozen or more Hosts and dancers present by
  the clock, so the pair hash gave the body about one visit in twenty ("a room of 22, nobody to serve", the first
  100k read). (4) **The foreman picked from his bed**: `dawn_pick` ran at 06:00 wherever he was (V12 `foreman_21236`
  at Rest in Block#540), and he was an actor of each Hire (174 lines).
- **The door** (`[verbs] door_body_first`, `service::door_man`): the let-in, the refusal and the rough client's
  throw-out go to the present door staff, a body first (the hash of the visitor and the hour among them), else the
  hash among all. `maybe_eject` keeps the strongest.
- **The floor** (`service::pick_server`, `[verbs] night_rows` = dancer, Host): a visit goes first to a body of
  `night_rows` on duty with room, and such a body also draws the venue's off-screen visits into its shift as a
  counter body does (`leisure::counter_bodies`). **Deviation:** the body's off-screen colleagues lose those customers
  (and their tips) to her. The coins move between the same purses; none are made.
- **The dancer's set and tips** (`vice::set_done` from the stations block's end, `[verbs] set_rows`; the `[shift]` blocks of
  30-90 min, since a 25-45 override on the dancer row put "station" into the off city's config,
  `stations::test_off_cities_write_nothing`; the shadows above ran with it): each block on the stage is a set, with one line for the room (the visits that
  arrived while she was on), the tips and the tippers. A set to an empty room is counted in the night line, not given
  a line. **The night** (`vice::night_told` from `service::shift_end`, bodies only; `ViceState.serve_nights`; the room
  from `service::arrive`, the served and the regulars from `encounter_row`, the tips where they are paid): the
  dancer's sets and tips, and the club hand's floor (the room, whom he served, the regulars).
- **The yard** (`[verbs] shape_up_gate`, `shape_up_lead_min` 20): a Yard's foreman with a body goes to its gate in the
  dawn window unless the owner cannot pay a hand (the Pitch goal, `streetwork::foreman_gate`/`foreman_due`). He sets
  out when the travel from where he is, plus the lead, reaches `corner_hour`; never when he cannot make it; and the
  goal holds once he is on his way. A committed Sleep ends for it as it does for a shift. The shape-up is one
  `DawnPick` line a Yard (`shape_up_told`): "picked N at the gate" (a body there, or a foreman off screen) or "took N
  off the list, the foreman X not at the gate", with the picked and the corner's sent home named (`[verbs]
  night_names`, "and N more"). A foreman at the gate of a Yard that took nobody says so. Each body sent home has its
  own line and a memory of the foreman. The picks' Hire is the hand's (`demography::hire_with`: the foreman is no
  longer an actor, but the text still names him).
- **The under-18 rule** (`ages::is_protected`): a kept door turns the under-age away every time ("under age" in
  `customers::cause`, past `door_p`; no scene and no grudge). No vice house (StripClub, Brothel, Dollhouse) admits one,
  with a bouncer on or not (`leisure::door_ok`). `rough_at_door` skips a protected client or worker, `scene` never
  fights one, and `maybe_eject` never throws one out by force. `harm_blocked` was 0 in the 20k x 30 run.
- **Shadow** (20k `isle_m_k2` and 100k `isle_m_k2_100k`, seed 42, d21-27; the 100k twice, the second after the floor
  and foreman fixes). The door at 100k: `bouncer_59955 d23 00:00` "Bronwen Crane kept the door of Club#5138 tonight:
  135 let in, 3 turned away (carrying 3)"; `d25 23:30` "Axel Kettering got rough with Rafe Quarry at Club#5138; Bronwen
  Crane threw Axel Kettering out, barred from the door"; `d26 20:45` "turned Axel Kettering away ... (barred)". Ghost
  Chrome was turned away "carrying" nine times in four nights, then `d26 00:02` "HuntStarted: Ghost Chrome went
  looking for Bronwen Crane". The dancer: `dancer_104644 d23 21:28` "Ignatius Fletcher danced a set at Strip
  Club#5357 (43 min) to a room of 5, 5 tips, 5c (Casimir Underhill, Rhoswen Oyelaran, Ulric Quarry and 2 more)";
  `d22 00:00` "danced 9 sets at Strip Club#5357 tonight to a room of 24: 20 tips, 20c; regulars Casimir Underhill";
  `d22 22:00` a rough client thrown out. The club hand: `club_staff_35574 d26 00:00` "Sprocket Stroud worked the
  floor of Club#4902 tonight: a room of 19, served 19 (Xia Blackwood, Rhys Osgood, Imogen Crane and 16 more);
  regulars Rhys Osgood" (the first 100k read: "a room of 22, nobody to serve"). The yard: `foreman_21236 d21 05:26`
  GOAL Pitch, `05:36` Vend at the gate, `06:00` "Silas Ostrowski picked 24 at the gate of Construction Yard#5328 at
  dawn: Xia Ashcombe, Jinx Quarry, Zane Stroud and 21 more; 657 sent home (...)", six lines in the week (174 before).
  At 20k, `foreman_13714 d25 06:00` "Construction Yard#5251 took 1 at dawn off the list, the foreman Jinx Ironside not
  at the gate".
- **Reads wrong** (the next pass): a 100k Fight Pit's door lets in 1-14 a night and turns nobody away
  (`bouncer_94173`), and a Strip Club's sets play to an empty room most of the night (`dancer_75115`: 8 of 9 sets one
  night). That is the venues' traffic, not the door. A door turning the same armed man away nine times is a loop, not
  a scene (the refused passes the door over only for `[exec] refused_ticks`). A 100k corner's shape-up queue is 600-700
  a dawn (`p_corner` over the Statistical broke), and the passed-over have no sequel. A foreman whose walk runs long
  arrives at 06:08, after the pick, and the far foreman (224 min away) is "not at the gate" every day. The first 100k
  read made the foreman an actor of 174 "sent home" lines (fixed: the line is the hand's).
- Numbers (prints, not claimed), 20k seed 42 x 30 (`--scaled --ambient-flats 50`): refusals 1,212, ejections 75,
  door rough 72, barred 113, sets danced 7,836, serve nights 3,512, under age 0, shape-up sent home (bodies) 10,
  harm_blocked 0, tps mean 128.8 (the box shared with three wave-mates). No test (doctrine 2). `SAVE_VERSION` 84
  (`ViceState.serve_nights`, the `[verbs]` T4 keys, and the `Jobs2Cols` `sets_danced`, `serve_nights`,
  `door_under_age`, `shape_up_home`, `foreman_gate`). One premise re-pinned, with the reason in place:
  `leverage::test_protection_pays_weekly_and_stops_when_district_lost` (with 10,000 coins the owner was the richest
  purse at a corp's bankruptcy, bought its Noodle Bar and incorporated, so the Hotel changed hands; she now holds 600).

### T7, noise after round 3 (`OPEN-233`; `citysim-cli/src/shadow.rs`, `events::with_ages`, `service::refused`, `vice::turned_again`, `[verbs]` T7 keys)

Folding is the diary's (the shadow tool): the events and counters underneath are unchanged, except the door's (below).

- **Standing facts** (`Refold::Standing`): the 00:07 Crew lines ("stays off G's trade today", "sat out G's raid",
  "chose the shift as R over G's duty", the percentage aside) and T1's quiet headcount ("nobody in or out since the
  last count", the wardens' names aside) are told on the first day and on the change; a repeat folds into the first
  line, which lists the days: "Crew: Roxy Whitlock stays off The Drowned Choir 10's trade today: the nearest spot,
  Club#5035, is 3 h 12 from home (the same on 2 more days: d22, d25)". A new spot or a new duty is a new line.
- **The day's folds** (`Refold::Day`, the first line rewritten in place): T3's `Pursuit` per guard and suspect in
  the guard's diary ("Garrick Oakes gave up the chase of Agathe Ambrose 2 times today (the trail went cold x2)"), per
  suspect in anyone else's ("2 chases of Moth Duarte given up today (by Wire Lockridge, Odile Greaves; the trail went
  cold x2)"); T4's refusals per face and door ("turned Bex Quarry away ... (drunk) (turned away 2 times today: drunk
  x2; barred till d23)"); a goal's cools per goal and day, streak or not ("goal Earn cooled 7 times today, the last
  until d26 01:50 (...)", "backed off, 2 to 7 in a row"; a jobless pick's Earn cooled every two hours); the word
  about me per rumour and day ("GOSSIP ABOUT ME (as the object): Assaulted by Ignatius Ostrowski on Flux Vance, told
  34 times today (27 to new ears, 7 heard already): A to B, C to D, E to F and 24 more"), which replaces S7's hourly
  fold of the known retellings.
- **The hour's folds**: a petty crime the pick witnessed (`law::is_petty`) waits for the hour (S7's `Quiet`, "Witness:
  saw N commit Fare Evasion this hour (...)"), and its `SawCrime` memory, the same fact, is not printed.
- **Encounters** (`Served` memories): the same pair in the same role again the same day is not news (the CEO's
  hourly table, the madam's hourly staff); a minute's new ones of one shape fold ("met 6 at the table (an attendee)
  (Seraphine Tanaka, Rook Thorne, Hesper Bellamy and 3 more; 2 regulars; 6 off screen)"); the `[off]` camera line is
  the memory line's "off screen" (`cam_settle_with`; SETTLED APART lines and the camera's counts unchanged).
- **The victim's age** (`events::harm_line`, `with_ages`, in `World::push_event`): every person named in an Assault,
  Murder, Assaulted, Brawl, Crossfire, Punished or Wounded line, and a Fare beating, carries " (N y)" after the first
  whole-word mention of the name ("Zelda Draper (35 y) beat Cerys Ramirez (30 y) at the El ..."). One place for every
  site; no text parser reads these kinds (`analyze_run.py` counts them by kind). 20k seed 42 x 30: 3,652 harm lines,
  3,608 with an age (the rest name gangs only), **0 under 18**; `harm_blocked` 0.
- **The `sick` pick** carries an illness: a cancer past stage 0 or a sick day in the last 14 (the malnourished only
  when nobody is ill), a diagnosed cancer first. V12's 100k picks were jobless and malnourished, "no cancer | sick
  days 0/14" (the sick roll is the employed's). Now both 20k picks hold a diagnosed cancer: `sick_14797` "Loan:
  Cordelia Chrome, 234c short of a vat-cancer treatment after the 6c they and theirs could put in ..., has no friend
  left to ask" (d21, d24, d27); in a run before the door's bar, `sick_15869` "Loan: Priya Nakamura, 228c short ...
  begged their friends: Tristan Moreau lent 10c; Amara Blackwood had nothing to spare ..." (S4's ladder), then
  "Treated: Hesper Ellery cut back Priya Nakamura's vat-cancer in the back room ...".
- **The door's answer changes** (a sim change; T4's 100k loop). `[exec] refused_ticks` holds one door a face
  (`Brain.refused`), so a drunk refused at two doors in turn came back to each, and the barred came back nightly to be
  refused "barred". `vice::turned_again` keeps each face's refusals at kept doors for `[verbs] door_return_ticks`
  (2880, `ViceState.turned`); a face back inside it is a return: barred from that house `door_return_ban_days` (2,
  today counted, `vice::shut_out`, a bar a door beside R6a's one a face), the line "turned X away again at the door of
  D (drunk): barred till dN", and the scene he makes swings on `door_swing_p x door_return_swing_mult` (2.0). The
  patron's pick passes over a house that bars him (`leisure::door_seen` reads `vice::barred`, which reads both).
  **Under-18:** the under-age is turned away first ("under age"); a return bars him (no force), and `scene` never
  fights one (`ages::is_protected`, unchanged). 20k `isle_m_k2` seed 42 x 30 (`--scaled --ambient-flats 50`, main
  `0452241` before, one run each): door refusals 1,144 -> 974, refused "barred" 117 -> 4, door scenes 157 -> 210,
  returns 149, faces turned away 3+ times at one door in a 4-day window 33 -> 8, ejections 79 -> 72; FINDING (the
  runs diverge): Brawl 230 -> 294, Assault 1,360 -> 1,418, Murders 48 -> 45, Arrests 1,585 -> 1,434, harm_blocked 0.
- **Diary size** (the same picks and window, 20k seed 42 d21-27, the door off so the world is identical; diary lines):
  `ceo_10496` 1,157 -> 730 (its "at the table" lines 423 -> 43), `ceo_16592` 1,110 -> 855, `madam_23135` 917 -> 770,
  `madam_11894` 874 -> 840, `bouncer_18251` 521 -> 489; the 14 diaries 11,686 -> 10,725 (-8 %).
- **Deviations.** The Fare Evasion lines are folded, not stopped: the guard on jail duty sees the El's gate because
  witnesses are by tile distance whatever the building (`law::raise_crime_mult`: a body inside the Precinct within
  `sight_day_crime` of the gate), a sim matter with counters underneath, left for the next pass. A standing fact
  folds across gaps (a later day's same sentence is "the same" though a day came between). The door's change is a
  bar by day, not "for the night" (`barred` reads days).
- **Reads wrong** (the next pass): the 20k Camp was busy all week, so the quiet count never folded at 20k (V12's 100k
  Camp is the case). A suspect "resisted arrest and fought off the guard X" six times in three days
  (`gang_member_6468`, Moth Duarte): six events, not noise, but the chase never ends. A cancer cut back twice in 43
  minutes at the same back room for 80c each (`sick_15869 d22 05:06`, `05:49`, the run before the bar). Friend affinity moves (25-30 a week
  in a ceo's and a gang member's diary) still print one a line. A fold's first line says "today" but sits at the
  first minute, so a later "again" reads earlier than it happened. 100k was not re-run (V12's 21+7 days at 22
  ticks/s is past the foreground cap); the door loop was read at 20k through the event log.
- No test (doctrine 2). `SAVE_VERSION` 88 (`ViceState.turned`, the `[verbs]` T7 keys).

### T5, the jobless look for work (`OPEN-217`, `OPEN-228`'s follow-up, `OPEN-234`; `systems/search.rs`, `[search]`)

- **Who searches** (`search::searching`): a free jobless adult in need of a wage (`Needs.wealth` under 1, the secure
  quit's line), no exec, not retired, not inside F3's idle days (`Misery.idle_until`), for `search_days` (7) after his
  last post (`Jobs2State.idle_since`; a post lost since the last midnight is day 0), and any jobless body the shape-up
  sent home this morning (`note_sent_home`, T4's passed-over: the sequel). F3's jobless state, the stain and the
  firing employer's refusal stand (`life::quit_blocks` is read at every door).
- **Asking Friends** (a body, the first hour of `hours_from..hours_to`, 07:00-16:00): his `asks` (8) closest Friends
  and kin; one employed where a post he can take is open (`demography::can_take`: `pick_candidate`'s rules for one
  candidate; a graded or skill-ranked post stays the midnight's) within `lead_ticks` (240) gives the word ("Taz
  Coldwater told Kuro Ito of a Packer post at Packing Plant#5313"). "Nobody knew of an opening" is told once a jobless
  spell (`Misery.search_spell`).
- **The doors and the boards** (the Pitch goal's search branch: `streetwork::street_tile`, `considerations`,
  `vend_ticks`; the ask is the `Vend` step's end, `search::on_ask`): the lead's door, else the nearest door with an
  open post he can take within `reach_ticks` (120), else the nearest hiring board (`board_kinds`: the Hall, a
  Market, the yards). At a door with the post open and his he is taken on (`demography::hire`, the vacancy closed
  before the night's Ambient pool); else "the post was gone" or "turned away". A board names the nearest open post
  he can take within `lead_ticks` of it (the next door) or "nothing going". A door `crowd` (2) searchers a post are
  making for is nobody's next; `tries` (3) doors a day, `ask_ticks` (15) at each. The want fades over the search
  days; the score is U(wealth), hunger and energy as the corner's.
- **Worse work** (`worse_after_days` 2): while he searches he stands at the dawn shape-up whatever his purse
  (`streetwork::corner_ok`); and the midnight job search hires a native past `[distance.choice] hire_max_ticks` up
  to `far_ticks` (360) (`search::takes_far` in `demography::job_search_round`), before the Ambient pool takes the
  post. The far hire's move-in is `[commute_pressure]`'s. This is item 3: S1's stranded natives were the posts past
  4 h that the pool filled.
- **The day labourer's day** (`streetwork::dawn_pick`): the picked hand's `Job.shifts` is `[corner_hour +
  day_start_min (30), the day shift's end]`, so he works from 06:30 (V12: Rest 91 + 83 min at the Yard for 09:00).
- **The exec's post** (`OPEN-234`, closed): `search::leave_post_for_chair` lets a held post go with a line at an
  incorporation (`founding::incorporate_owned`), a spinoff's exec (`corps.rs`) and, as the backstop, daily for any
  living exec holding one (`ownership::exec_wages`). `exec_6526`'s path was the incorporation: "Sprocket Singh gave up
  the post of Ripperdoc at Ripperdoc#4936 to take the chair of Singh Holdings" (20k day 12; 15 lines in the run).
- **Statistical mirror**: no walk and no door; an off-screen searcher is the midnight job search's candidate with
  the far reach from `worse_after_days`, and the shape-up's queue on `p_corner` whatever his purse while he
  searches. No child: `is_adult` and `ages::is_protected` in `can_take` (no victim pick, fight or punishment is
  touched); `harm_blocked` 0.
- **Shadow** (20k `isle_m_k2` seed 42 d21-27; a new `jobless` pick, the searching jobless freshest first, as the
  repro of V12's laid-off `night_shift_23119`): `jobless_6381 d21 07:00` "Philippa Whitlock asked Razor Stroud,
  Bertram Kettering, Rafe Whitlock and 5 more about work: nobody knew of an opening"; `09:07` "read the board at
  Construction Yard#5260: a Vat Tech post at Vat Farm#4377"; `13:05` "asked at Vat Farm#4377 about work: the post was
  gone"; `d22` two more boards; `d23 09:38` "asked at Ripperdoc#5658 about work: turned away"; `14:40` "asked at
  Packing Plant#5552 about work and was taken on as Packer". `jobless_6397 d21 08:39` a board, `11:35` taken on as
  Vat Tech. `jobless_6465 d21` two boards "nothing going", then the shape-up d22-25 (worse work), `d22 06:02` Rest 28
  min at Construction Yard#5274, `06:32` the first load, `18:00` +3c. `worker_13107 d27 11:32` out of the cells to
  "the post as Club Staff ... gone", `12:00` asks her Friends, `15:01` a board names a Super's post. An earlier read:
  "Adela Dunmore asked at Block#2227 ... taken on as Super" at 09:04, and Philippa Whitlock at the same door at 09:31,
  "the post was gone".
- **Reads wrong** (the next pass): "the post was gone" is the commonest door line (a board names a post two hours'
  walk away and a nearer searcher or the midnight takes it). A door hire into a post the layoff pass sheds at the
  next midnight ("taken on as Vat Tech" d21 09:06, "laid off: revenue, a shift past full staff" d22 00:07): the post
  was open past the corp's cap. The dawn corner's eat-or-queue tie (Eat 0.240 vs Pitch 0.239 at 05:21) costs a
  searcher the shape-up two mornings. A homeless pick named a board's post and never walked it (`homeless_10221
  d21`). A searcher with no door or board in reach still idles (the rest of the day, once the tries are spent).
- **Numbers** (20k `isle_m_k2` seed 42 x 30, `--scaled --ambient-flats 50`, one run each, base `0452241`):
  Materialised 863 -> 648, Ambient pool hires 821 -> 602, far posts taken by natives 0 -> 300 (191 on day 2),
  employed mean 11,240 -> 11,248 (days 2-6 +180..+200: the natives fill the open posts past the pool's 100 a day;
  vacancies open mean 132 -> 86), population mean 23,641 -> 23,412 (fewer Ambient households come in), employed per
  resident 0.475 -> 0.480. Search days (bodies) 3,412, Friends' words 95, doors and boards asked 4,872, taken on at
  the door 384; day posts filled 394 -> 323. FINDING (not tuned): homeless mean 457 -> 512, max 657 -> 783 (day 20):
  day 14's company-housing buyouts 297 -> 420 evictions (the far-hired natives' corps buy Blocks near work); day 29
  582 -> 589. Ticks/s mean 160 -> 146 (a shadow ran beside it; a print). Not read at 100k (20k shows every case).
- Re-pinned premises (`search = None`, the reason in place; the subject of each is not the search):
  `civic::test_jailer_meal_is_an_inmate_encounter` (a searcher took a jailer's post at the Jail's door by day 2 and
  the prisoner's meal went to him), `outside::test_demand_linear_until_the_cap` (the 2.0-flat city's uncapped visits
  fell to 1), `person::probe_el_commuter_with_coins_rides_and_beats_the_walk` (a different commuter picked, whose ride
  waited), `person::probe_parent_with_the_fee_buys_the_child_out_within_fourteen_days` (a jobless parent searched
  through the week's Camp answer: a reads-wrong for the next pass), `sickness::test_g3_only_the_broke_steal_food` (a
  door hire's wage between the snapshots). The `jobless` shadow pick is new (`citysim-cli/src/shadow.rs`).
- No test (doctrine 2). `SAVE_VERSION` 88 (`Misery.search`, `Misery.search_spell`, `EventKind::JobSearch`, the
  `[search]` keys, the `Jobs2Cols` `search_days`, `search_leads`, `search_asks`, `search_hires`, `far_hires`).

### T2, School by travel time (`OPEN-232`; `ages.rs`, `exec::routine::workday_of`, `[ages]` T2 keys)

**Dylan's call (2026-10-27): reach by travel time, not a School per district.** Built:

- **The pick** (`ages::children` -> `pick_school`, midnight, both tiers): a school-age child on a school day goes to
  the nearest School (`schools_in_reach`, by `ages::school_trip`: the walk at `[exec] move_ticks_full`, or the El's
  structural estimate `distance::travel` when quicker) within `school_trip_ticks` (60) of its Home door, with room
  on its roll, whose fee (and `school_el_fares` (2) `[transit]` fares a day when the School is past the walk) the
  household's payer can spare; the payer is now the parent at home with the most to spare (S6 read the lower id's
  purse alone). The fares go to the Treasury (`Flow::Fare`); a child never rides on a dodge, so no gate, guard or
  beating is reached (the under-18 rule: no new victim pick, no fight, no fare ladder for a child; nothing calls
  `law` or `fares`).
- **The roll** (`roll_cap`): `school_places_per_teacher` (30, the teacher row's `per_hour`) for each teacher on the
  School's staff; full, the next child goes to the next School in reach, else home (`SchoolMiss::Full`); a School
  with no teacher takes no one. `Child.{school, school_miss}` hold today's School (or the one that turned it away
  or was too dear) and why not (`Far`, `Full`, `Purse`); `AgesState.{rolls, roll_day}` the day's rolls.
- **The teacher's day follows the roll** (`ages::teacher_idle`, read by `routine::workday_of`, so the Full Work goal
  and `lod::stat_work` alike): a School with no child on today's roll (a weekend, or none in reach and paid) is no
  workday: no class, no ClerkWork, no shift and no wage. On a school day the School says so ("No pupil on the roll
  at School#5176 today: no class for Delia Underhill, Kuro Singh (no shift, no wage)"). V12's teacher worked
  Saturdays at an empty School; now Saturday and Sunday are off.
- **The `[family]` line** (shadow): "at School#4938 (D4 Mid-Levels) (56 min walk)", "(38 min by the El, the fares
  paid)", "at home (play: no School within 1 h)", "(play: School#X is full, 60 of 60 on the roll)", "(play: the 2c
  fee at School#5279 (D4 Mid-Levels) is beyond the purse)", "(play, under 6 for School)", "(play, no School today)".
- **Deviation: the Schools seeded by reach** (`school_by_reach`, `ages::seed` -> `school_lot`, `HomeGrid`). The
  spec's pick alone moved nothing at 20k: S6's seed put each School on the vacant Lot nearest its district's
  centroid, 20 Schools at five centroids, and 225 of 1,533 school-age children (15 %) had one within an hour (the El
  added 14). Each seeded School now takes the Lot (Mid and up, as before) whose door has the most Home residents not
  yet within the walk of `school_trip_ticks` of a seeded School: 645 (42 %) within an hour, the Sump 3 -> 111 of 462
  (from Mid Lots on its edge), D6 The Vats 0 of 95. Same count, no School per district. `jobs::seed_lots` exposes
  `seed_lot`'s candidates, `vice::seed_venue_on` builds on a chosen Lot.
- **Numbers** (20k island seed 42 x 30, `--scaled --ambient-flats 50`, before = main `0452241`, one run each):
  school days / child-days 2.5 % -> 7.6 % (school days 2,034 -> 6,327; mean ch_school 67.8 -> 210.8); of the
  school-age children's school days, at School 18.5 %, no School within 1 h 20,010, full 488, the purse 7,441, by the
  El 150. Daily school days d0-d4 629 -> 378, then ~210-230 (before 201 -> 134, then ~80): the purse falls in week 1
  as S6 saw. Employed mean 11,240 -> 11,320, homeless max 657 -> 715, violence deaths 81 -> 101, starvation 0 -> 1
  (an adult, `Imani Garland`, evicted d11; the Lot layout moved the city, not chased), harm_blocked 0 -> 0. **100k**
  (`isle_m_k2_100k` seed 42 x 8, same flags): 6.7 % -> 7.2 % (school days 6,029 -> 6,480): the roll binds, 1,080 a
  day flat (18 Schools x 2 teachers x 30) against 10,180 turned away full, 22,440 out of reach, 1,820 unpaid; the
  before fell 1,302 -> 628 by d7 on the purse. The 100k city has 18 Schools for ~11k children (fewer than the 20k's
  20: `schools` is per Lot copy).
- **Reads wrong / open** (the next pass's input): at 100k the Schools are full, not far: two teachers a School and
  18 Schools is the cap (a founding by demand, or Schools by the child count, is Dylan's call; no knob turned). At
  20k the fee is the binder in reach (7,441 unpaid against 6,327 schooled): a jobless parent's child is at School one
  day and home the next ("Ottilie Santoro (7 y) at School#4876 ... (55 min walk)" d21, "the 2c fee ... is beyond the
  purse" d22, `camp_parent_11600`); a teacher's own child is home on the fee (`teacher_11833`: "Dex Greaves (13 y) at
  home (play: the 2c fee at School#5279 ... is beyond the purse)"). A parent on the service's warning still pays the
  fee from what `spendable` leaves (the children's food is not reserved first). Classes are small (1-5 a teacher). The
  El carries few (150 school days at 20k): the stops lie far from most Homes. Mid-Levels children past an hour stay
  home ("Rin Varley (13 y) at home (play: no School within 1 h)", Block#1226 D4).
- Re-pinned on the premise, the reason in place: `civic`'s `schoolchild` fixture puts the staged child on the roll
  (an empty roll is no teacher's workday); `stations`' `station_world` and `person`'s
  `probe_working_couple_moves_near_work`, `probe_worker_eats_before_the_shift_not_during_it` keep S6's centroid
  Schools (`school_by_reach = false`: the Lot layout moved the Stations off guards' reach and the room near work);
  `housing`'s school rule reads `school_trip_ticks`. `ages::place_of` for a hand-set School routine reads the nearest
  School by the trip. No test (doctrine 2). `SAVE_VERSION` 88 (`Child.{school, school_miss}`,
  `AgesState.{rolls, roll_day}`, the `[ages]` T2 keys, the `Jobs2Cols` `school_far`, `school_full`, `school_unpaid`,
  `school_el`).

### U4, who is indoors sees indoors (`OPEN-236`; `law::outdoors`, `law::sees_scene`, `[crime] open_air`)

**Built.** One predicate, `law::sees_scene(p, tile, building, scene_out, reach)`: a body sees a scene when both are
in the same building, or both are `law::outdoors` within `reach` Chebyshev tiles; never through a wall. `outdoors` is
no building, a building's door tile (the threshold sees both ways), or a wall-less kind (`[crime] open_air`: Market,
Cemetery, Lot, FreightYard, ConstructionYard). It gates, and only these (U7 takes the witness and memory paths next):

- **A crime's witnesses** (`law::raise_crime_mult`): an indoor scene is read from the position index's building list
  (`agents_near(tile, 0, building)`, the bodies only), not every body, so it is cheaper than before; a street scene
  is the old body scan with the walls added after the distance check. `witness_cap`, the law's eye first, Eyes'
  `Kit.sight` and the jacked-in skip are as F2 left them.
- **The guards' sightings of the wanted** (`law::sightings_with` and its `#[cfg(test)]` twin `sightings_scan`, kept
  identical): `GuardSight.outside` is read once per guard; the desk inside the Precinct no longer locates a suspect
  walking past on the street, and a guard on the street no longer sees into a flat.
- **SawCorpse at a death** (`demography::on_death`).
- **The off-screen witness** (`bind::bind_in`): the pool skips a body held in the cells that day (`JAILED`, as the
  actor pool already did). The off-screen pool is by district and day, with no tile or building to gate on; the
  meeting draw (`lod::stranger_in_zone`) already skips the held and is no witness roll, so it is unchanged.

**Deviation: what is not gated.** Interaction distances (`law::near`: a loot, a punch, a hand-off), the actor's read of
the law (`guard_within`, `needs::guard_near_fn`: a thief still minds a guard behind a window), the gang's squat-blow
sight (`gang::contested`), a raid's or riot's answerers and the reporter's interview (`news::interview`, who is near
the door, not who saw) keep tile distance. One building is one room: a crime on a tower's floor is seen from all of
it, as before (`Position` has no floor).

**Numbers** (20k island seed 42 x 30, `--scaled --ambient-flats 50`, before = main `8cf7fe1`, one run each): Witness
19,528 -> 12,126 (Fare Evasion 5,844 -> 4,130, Assault 7,023 -> 4,564, Grand Theft 2,963 -> 1,445, Shakedown 2,559
-> 1,382, Theft 702 -> 353), WitnessReacted 4,472 -> 2,633, Report 1,864 -> 1,472, Arrest 1,437 -> 1,054 (arrests a
day 46.7 -> 34.2), jailed mean 174.9 -> 124.2 (max 371 -> 254), thefts 1,094 -> 1,176, assaults+murders a day 53.1
-> 50.4, suspect sightings 7,106 -> 8,306 (more wanted walk free longer), the consequence pass's reports 3,700 ->
2,068. ticks/s mean 138 -> 132 under three wave-mates' builds (a print). The lower arrest count is the city's law
seeing less, not a knob: no band moved.

**Shadow** (`shadow --population 20000 --map assets/maps/isle_m_k2.txt --seed 42 --start-day 21 --days 7`, picks
`guard:2,gang_member:2,reporter:1` in `target/u4_shadow`, the held by `--agent` in `target/u4_shadow_jailed`, the same
held and guards on main's binary in `target/u4_shadow_before`):

- Before, from the cells: "Ysolde Yates saw Wren Ostrowski commit Fare Evasion" (d22 11:14, at Precinct#4460), and the
  desk guard "Elowen Underhill saw Quill Whitlock commit Fare Evasion" (d24 11:32, inside the Precinct). After: no
  Fare Evasion is witnessed from inside any building in 13 diaries.
- The held see the cells: Solene Lindqvist, in the Precinct from before d21 to her release at d27 22:52, has one
  Witness line all week, "saw Axel Kimura commit Assault" (d23 06:18, Precinct#4460); Imogen Ashcombe "saw Juniper
  Bellamy commit Assault" (d23 18:23) and "Anselm Mbeki" (d26 19:15), both in the Precinct.
- The jail guard sees the indoor crime: `guard_24275` at `GuardJail` in Precinct#4460 "saw Rosalind Yardley commit
  Assault" (d26 16:30); `guard_7793` the same at d23 16:00 (Tristan Achebe). Two Witness lines a week each (V13's
  `guard_27360` had 41).
- The street still reports: `guard_7793` on patrol in the Neon Quarter "saw Juniper Sallow commit Grand Theft" (d27
  09:38, on the street) and "arrested Juniper Sallow for Grand Theft" at 10:06; `guard_24275` on a street break "saw
  Vivienne Ostrowski commit Fare Evasion" (d27 14:27). A crew's shakedown inside Block#746 is seen by its partner in
  the building (`gang_member_10186` d22 03:59 / 04:07). The reporter's interviews are unchanged (9 lines).
- **Reads wrong / open** (the next pass's input): `guard_24275` picks Arrest from his own Home (Block#2831, d22 09:05,
  "warrant located") and it dies `Unreachable` at 09:07: a located warrant, not his sighting (the arrest's reach, not
  U4's). A crew's two members each witness the other's shakedown inside the same Home ("Ghost Ironside saw Cato Varley
  commit Shakedown" then the reverse, 8 min apart): accomplices are witnesses (U7's contradictions). With fewer
  arrests the cells hold ~50 fewer; the pressure that cost is the law's, to come back as features if the city reads
  soft (doctrine 5).
- No test (doctrine 2). Re-pinned on its premise, the reason in place: `tests/bind.rs`
  `test_bound_actor_with_witness_is_reported_and_arrested` kept the others out of the candidates by marking them
  jailed for the day, which now also empties the witness pool; they are marked Full that day instead (never bound,
  still witnesses). `SAVE_VERSION` unchanged: no component or save field moved; `[crime] open_air` has a serde
  default.

### U5, the forger and the fixer at work (`OPEN-217`; `gray.rs`, `service.rs` walk-ins and `fixer_hand`, `contracts.rs`, `weapons.rs`, `[gray]` U5 keys)

- **The cause** (V13's diaries, the round 4 100k run, a 20k base shadow). Three, and the tier is only one. (1) **No
  market**: papers are bought only by the wanted, and the Clerk's 60c bribe takes every buyer who can afford a tier-2
  forgery (120c), so the 100k city sold 14 fake papers in 30 days (70 bribed) across ~14 Fixers that ever named one;
  a followed forger saw a client every few weeks. (2) **The counter took the forger's sale**: the walk-in's server was
  the pair hash over the Fixer's present `Client` staff (the hired Fixer and the forger), so half the handovers went
  to the colleague, and the forger the midnight pass named was paid wherever he was (20k base `forger_19041 d21
  13:01` "SETTLED APART: Gray 32c ... I am asleep at Block#3996"); `forger_near` named the first forger on the books,
  not a body. (3) **The fixer's book never reached him**: a brokered post and a take are `Client` encounters only when
  both parties are present (P3), the corps post at 00:06 and the takes come at the match hour, so a body fixer's diary
  held nothing (20k base `fixer_8620`: no post, take or outcome in seven days), and a record's close named nobody at
  the Fixer. The shadow's `fixer` also took absentee owners with a day job elsewhere (V13's `fixer_23959` a Vat Tech,
  the base `fixer_21976` a Street Market clerk).
- **The forger's commission** (`service::walk_in_sale`, `WalkKind`): the forger who made it hands it over when he is
  at the desk with room this hour (`present`, `has_room`); else the counter's server hands over his work ("Hale
  Abernathy handed Casimir Novak a forged weapons licence (Osric Stroud made)"), and a forger in the cells or dead made
  nothing (the server is paid as the maker). An order of today waits for a forger still due at his desk today
  (`forger_due_today`): the counter opened at 09:00 and the commuting forger came in at 09:2x, so the hired Fixer
  handed over his work (100k `fixer_98933 d22 09:00` "(Moth Bellamy made)"); from the next day the counter hands it. Paid at the handover: the forger's share, the owner's `fixer_cut` held by
  the server for the owner (F3's `fixer_owed`, settled on payday or at `collected`); the line says so ("40c paid, 8c of
  it held for the owner"). `forger_near` names a body first, then the lowest id.
- **The forged licence** (`gray::licences_daily`, `[gray] licence_price` 40, `licence_lawful` 0.5,
  `licence_reach_ticks` 120, `licence_pass` 0.6): the second commission. An adult in `[weapons]`' want set, holding no
  licence and no order, under `licence_lawful` (the Clerk's buyers are at or above it: the split is clean), with the
  price, the cheapest weapon and `coin_reserve`, orders one at midnight from the nearest forger whose Fixer is within
  the reach of his Home; the same walk-in as the papers. `GrayState.licences` (holder -> forger) makes him `licensed`:
  the Gunsmith sells to him and he carries. A guard who notices the weapon (`sight_carriers`) looks at it: it passes on
  `licence_pass` (a hash of the holder and the hour), else it is exposed: void, `Forgery` on the holder and the forger,
  and the weapon's `UnlicensedArms` filed; at an arrest, `p_expose` (`gray::on_arrest`). **Deviation:** the spec named
  "the papers"; a forger with only the wanted for clients has no trade at any size, and the unlawful who want a gun had
  nowhere to buy one (`sold_dealer` 0 at 100k, ~240 wants standing a day).
- **The visit's hour** (`service::walk_in_hour_for`): a body buyer's pickup visit is the first hour of the Fixer's
  open hours he is off his own shift; none today, no walk (he comes on his rest day). The counter keeps the day shift:
  a 09:00 visit waited out a 09-18 shift and walked to a shut Fixer at 18:xx, daily.
- **The fixer's day** (`service::fixer_hand`: the holder of the Fixer's `Fixer` post alive and free, a body first, then
  the lowest id; else the owner, a person alive and free). The book is told to him, not met: **posted** ("Vatra 2
  posted a guard on Block#1335 for 33 at Fixer#5271, on Mateo Sato's book"), **taken** from his regulars at the match
  hour with the crew named ("The Drowned Choir 10 took the beat on Roderick Sallow (LIVE, crew 1), put on it with
  Dorian Bellamy by Imogen Duarte at Fixer#4755"), and **the outcome** on every close of a brokered record, with the
  cut (`Contract.cut`, set in `fulfil`) or the escrow's return ("beat on Roderick Sallow: done by The Drowned Choir 10
  (... won (2 vs 2, 0 dead)); back to Imogen Duarte at Fixer#4755: the cut 25c to Imogen Duarte"). **Deviation:** the
  money path stays the owner's (`FixerCut` from escrow): a hired fixer's outcome names the owner's cut; nothing is
  routed through him. The match puts nobody on a job whose mark is away past its deadline (`mark_gone_past_deadline`:
  the cells, abroad, leaving): it stays on the book and expires (20k: three Locates taken at 18:00 on marks "in the
  cells till d36", each let go within the hour). The shadow's `fixer` is the `Fixer` post's holder or an owner with no
  post (an owner with a post elsewhere is not running the office).
- **The under-18 rule** (`ages::is_protected`): a record's mark is never protected (`contracts::target_ok`, now by
  kind: only a Guard or an Escort may name a child), so no Hit, Beat, Abduct, Locate or Persuade is posted on one,
  brokered or direct; and nobody protected takes a job or rides on its crew (`eligible`, read by `crew_ok` for
  `squad_crew` and `job_crew`). `free_adult`'s adult age is also 18; the gate no longer leans on that. The licence's
  buyers are `[weapons]`' adults, and `licences_daily` skips the protected as well.
- **Shadow** (20k `isle_m_k2`, seed 42, d21-27, forger:2, fixer:2, buyer:1; 100k below). The forger: `forger_22831
  d21 09:00` "Rin Pembroke handed Rosalind Blackwood a forged weapons licence at Fixer#5154, 40c paid, 8c of it held
  for the owner"; `d25 12:10` and `d26 17:50` papers to Lux Chrome and Winifred Ellery; `forger_9672` four in the week
  (three licences, `d24 17:00` papers to Kieran Eastwick), and `d21 09:41` "Ines Delacroix arrested Osric Stroud for
  Forgery" (a licence of his exposed), two days in the cells. The fixer: `fixer_8620` all week: two to three posts a
  night "on Mateo Sato's book", the takes at 18:00 "put on it by Mateo Sato at Fixer#5271" (seven), and the closes
  "back to Mateo Sato at Fixer#5271: the escrow back to the buyer" (five, before the mark filter).
- **Shadow at 100k** (`isle_m_k2_100k`, seed 42, d21-27, forger:1, fixer:1; the build before the visit-hour and the
  wait-for-the-forger fixes, 49 min). The forger: `forger_90429 d23 09:00` "Rosalind Achebe handed Alaric Penhallow a
  forged weapons licence at Fixer#5897, 40c paid, 8c of it held for the owner", again `d26 09:00` (V13: no papers
  handed by either forger). The fixer: `fixer_98933` (the Fixer post at Fixer#5120, owned by The Ash Knives) is told
  8-10 posts a night ("Zetatech 7 posted a hit on Quill Oyelaran for 1486 at Fixer#5120 (culprit), on Odile Tanner's
  book"), the takes at 18:00 with the crew ("The Red Lanterns took the hit on Quill Oyelaran (LIVE, crew 4), put on
  it with Santiago Novak, Quill Lockridge, Gwendolyn Ramirez, Bertram Wexford by Odile Tanner at Fixer#5120"), and
  the outcomes ("steal on Lab#5949: done by Elspeth Oakes (49 taken); back to Odile Tanner at Fixer#5120: the cut 35c
  to The Ash Knives"). V13's fixers had two events a week. **Read wrong there:** Odile handed out the forger Moth
  Bellamy's licences at the 09:00 opening (`d22`, `d23` "(Moth Bellamy made)", 21 lines), fixed after the read
  (`forger_due_today`; 20k x 30: 5 -> 1 handed by another server), not re-shadowed at 100k (one 100k shadow a phase).
- **The final 20k shadow** (the same picks' window on the final build): `fixer_8620 d24 00:09` "guard on Braindance
  Parlour#5786: done by Anton Northcott; back to Mateo Sato at Fixer#5271: the cut 9c to Mateo Sato", eight takes "put
  on it by Mateo Sato"; the two forgers drawn this time (Fixer#6013, Fixer#6050 in the Neon Quarter) had no client in
  the week: neither Fixer sold anything in the 30-day run (9 Fixers did). An owner with a hired Fixer on the post
  (`fixer_23495`, Fixer#4755) is not told the book (the post is); she has `d21 13:44` "collected the week's cut at
  Fixer#4755 in person".
- **Reads wrong** (the next pass): the walk-in queue still waits (20k x 30: 223 waited-days and 39 dropped against 33
  served): the wanted are ordered to the nearest forger at any distance, past `[distance.choice] visit_ticks` they
  never walk, and are arrested waiting; a working buyer comes only on his rest day (the counter's day shift, the
  clientele's too). A forger off the desk at a later day's handover is still paid apart (the colleague's handover is
  right, the camera's SETTLED APART line is not). Demand clusters: 9 of the island's Fixers sell; a forger at a quiet
  one has an empty week. A fixer whose office has no posts (`fixer_8354`:
  records none) clerks the counter's ClerkWork, joins a gang and sells a sighting: the week reads as a life, not the
  trade. A corp's Guard converted by a Security corp closes "no cut" (`corp_take` pays the Fixer nothing). Posts reach
  the fixer's diary at 00:06 while he sleeps (the book overnight; not met in person).
- Numbers (prints, not claimed), 20k seed 42 x 30 (`--scaled --ambient-flats 50`, `target/u5_run3.csv`): forger
  commissions handed 33 (forged licences 26, papers 7; 1 by another server) at 9 Fixers; licences exposed 3, passed 4;
  papers bribed 19; fixer outcomes told 91 (fulfilled 59, failed 29, expired 3) to 11 fixers; contracts posted 732.
  No test (doctrine 2). `SAVE_VERSION` 91 (`GrayState.licences`, `WalkIn.kind`, `Contract.cut`, the `[gray]` U5 keys,
  the `Jobs2Cols` `licences_forged`, `licences_passed`, `licences_exposed`, `fixer_outcomes`).

### U1, Schools for the children there are (`OPEN-232`; `ages.rs`, `founding::refit_with`, `[ages]` U1 keys)

**Dylan's call (2026-10-28): Schools by the child count and on demand; the 2c fee stays.** Built:

- **The seed by the child count** (`ages::seed_by_children`, `World::new` after `seed_children`): once the children
  are seeded, the Schools are topped up to one for each `school_age_per_school` (60, a one-floor School's roll of 2
  teachers x 30) school-age children. T2's `schools` per copy stays the floor, placed as T2 placed it. The standing
  Schools cover their rolls of the nearest school-age children first (`HomeGrid` now holds a weight by Home door,
  and a School covers up to its seats, nearest first; T2's residents pass is the uncapped case). Each new School
  stands on the site whose door has the most children left without a seat within the walk of `school_trip_ticks`
  (`school_site`: the vacant Lots of `jobs::seed_lots` and the derelict Blocks of a School's tier compete, a Lot
  first on ties). Its teacher posts open as vacancies (`founding::convert`), filled by the first night's hire, so its
  children are turned away full on day 0 only (20k: 134 that day). A job search run at the seed's end staffed them
  on day 0 but hired into every open post of the seed and moved eight probes' staffing (a Reporter post, the
  Guards, the Supers, a jobless agent); dropped. "The city opened a School in D4 Mid-Levels (seeded for the
  children: 31 school-age without a seat within the walk; Lot 5103)". 20k: 20 -> 26 Schools (1,533 school-age);
  100k: 18 -> 114 (96 built: 74 in D4 Mid-Levels, 22 in D3 Neon Quarter, none on a refit; the median opened with 51
  children without a seat in its walk, the first 179, the last 3).
- **The founding on demand** (`ages::found_on_demand`, a school day's midnight after the pick, both tiers): the
  children turned away full are kept by their Home's district. A district with `found_full_min` (30, a teacher's
  class) or more of them on `found_full_days` (5) school days running (`AgesState.full_run`; a day under the line
  resets it) gets a School on the city's deed, by the same site rule over the turned-away children's doors (a Lot,
  else a refit Block: `founding::refit_with` now takes a School), its posts filled by the day's hire. Not while a
  School in their reach has a roll of 0 (its teachers not yet hired), so one founding is not followed by the next
  before it opens. The fee and the reach rule are T2's, unchanged. Nothing touches a victim pick, a fight, a
  punishment or the fares (the under-18 rule is not reached; a refit's squatters are put out as every refit's).
- **Deviation: unpaid, on the city's deed.** The spec does not say who pays; the seed's Schools are the city's deed,
  unpaid, and a founding draws no coin either (no Treasury gate; no coin made or lost).
- **Deviation: the founding does not fire in the shipped city.** With the seed by the count, the turned-away-full
  line reaches 30 in a district only on day 0, before the hire (20k: 3-27 a day after; 100k: 14-33 a day across the
  city after), and a day under the line resets the run. It is proven on a
  demo root (`target/u1_demo/assets/config.toml`: `school_age_per_school = 0`, `found_full_min = 5`, 20k seed 42 x
  30): day 4, "The city opened a School in D4 Mid-Levels (School#4893 on Lot 4893): 24 of the district's children
  turned away full, 5 school days running (School#4938 full); its teachers to hire"; turned away full 26 -> 0 from
  day 7; at d21 "Tank Park taught 17 pupils at School#4893" (`teacher_22930`), 17-19 a day all week.
- **Deviation: the Sump and the Vats stay far.** A School stands off the Sump (phase 7's tier rule). At 20k the count
  (26) is met while 16,860 school-day child-days are still past an hour's walk: no Mid Lot or Mid derelict Block
  lies within the walk of the Sump's and the Vats' Homes. Left as the city (no knob).
- **Numbers** (seed 42, `--scaled --ambient-flats 50`, before = main `8cf7fe1`, one run each). 20k x 30: school days
  / child-days 7.6 % -> 10.5 % (school days 6,184 -> 8,605; of the school-age school-day child-days 18.2 % -> 25.4
  %); turned away full 579 -> 457 (134 of them on day 0, then 3-27 a day), far 19,620 -> 16,730, unpaid 7,667 ->
  8,119, by the El 147 -> 119; Schools 20 -> 26, none founded. Side numbers (the Lots built on moved the city, not
  chased): violence deaths 94 -> 103, homeless max 683 -> 775, employed mean 11,230 -> 11,330, starvation 0 -> 0,
  harm_blocked 0 -> 0. 100k x
  8: 7.2 % -> 24.2 % (school days 6,471 -> 21,680; of the due 15.8 % -> 53.0 %); turned away full 10,200 -> 4,132
  (4,019 of them on day 0, the top-up's 96 Schools waiting for the first night's hire; then 14-33 a day), far 22,420
  -> 10,210, unpaid 1,833 -> 4,889; Schools 18 -> 114, none founded; daily school days 1,080 on d0 (T2's 18 Schools
  alone), 5,038 on d1 -> 3,063 on d7 (before 1,080 flat: the roll bound; now the purse falls through week 1, as at
  20k in T2); violence deaths 53 -> 56, homeless max 10,760 -> 10,740, employed mean 35,250 -> 35,450, harm_blocked 0
  -> 0. A seed-end job search (dropped, above) had d0 at 5,066 and the run at 28.3 %.
- **Reads wrong / open** (the next pass's input): the fee is now the binder at both sizes, and it flickers: a child
  is at School one day and home the next, even a teacher's own child at her own School, 3 min away (`teacher_7582`
  at School#5171: "Sprocket Stroud (6 y) at home (play: the 2c fee at School#5171 (D4 Mid-Levels) is beyond the
  purse)" d21-d22, "at School#5171 (D4 Mid-Levels) (3 min walk)" d23-d25). The family on the service's warning since
  d24 still pays the fee on d24-d25 (T2's note: the children's food is not reserved first). Classes at the top-up Schools are small (`teacher_12033` Jocasta
  Sato at School#5269: 2-5 pupils a day), so most of a top-up School's roll sits empty on the fee, not on the seats.
  A T2 School the top-up drew its children from stands idle: "No pupil on the roll at School#5176 today: no class
  for Zed Moreau, Cato Carrow" every school day of the shadow (12 days of 22 at 20k, 1 before); nothing closes a
  School nobody attends or moves its teachers. The teacher's "met X at the door (the parent of Y, a pupil)" memory
  repeats every school day with the same parent (`teacher_22930`, five days of Ysolde Hawthorne): noise for the next
  noise pass. The far children in the Sump and the Vats (above).
- Re-pinned on the premise, the reason in place: `ages`' `test_vendors_child_chores_then_school` takes a School with a
  teacher on its roll (the first School by id is now a top-up one waiting for the hire). `care`'s sick cover writes
  its `Served` memory only with the service section on (`service`'s `test_off_city_writes_nothing`: the seed moved a
  cover onto day 1 of the off city, a leak that was there before U1).
- No test (doctrine 2). `SAVE_VERSION` 92 (91 was U5's) (`AgesState.full_run`, the `[ages]` U1 keys, the `Jobs2Cols`
  `schools_founded` column); `analyze_run.py` prints the Schools at day 0 and the last day, and the founded count.

### U2, the neglect ladder tightens (`OPEN-231`; `camp.rs`, `ages::move_in`, `search.rs`, `[camp] warn_repeat_days`)

**Dylan's call (2026-10-28): a second warning inside 30 days takes; a warning with no free parent at home takes at
once.** Built:

- **The second warning takes** (`camp::take_daily`): `[camp] warn_window_days` (14) is now `warn_repeat_days` (30):
  unfed again to `take_days` inside 30 days of the warning, past `warn_grace_days` (3), is the take, and the line
  says so ("the child protective service took Nadia Kettering from Block#324 to Work Camp#4835: unfed 3 days again,
  inside the warning of d7"). The same 30 days hold the household's share (`adults_share`) and the let-home's
  warning. 20k x 30: children warned twice inside 30 days with no take 73 -> 0 (Block#239's five, d3 and d21).
- **No free parent at home takes at once.** A free parent (`camp::free_parent`): living, adult, not sentenced, not
  in cuffs (held or carried), not downed or laid up (`World::wounded`), not leaving; at home is the child's
  `Household.home`. A neglect with none is the take, not a warning; so is a child on an open warning whose parents
  are all not free at midnight (fed or not: `dancer_99218`'s warning hung open while she sat in the cells). The
  line names each parent and where he is ("... from Block#586 to Work Camp#4835: on the service's warning and no
  free parent at home (Winifred Adeyemi laid up, Viktor Abernathy in the cells)").
- **The warning's household is the child's own** (`reporter_57837 d27`): the cause was `ages::move_in` /
  `move_in_with`. An elder past `frail_from_years` taken in by a grown child (or a retiree's spouse) moved and left
  his own young children in the old Block, unfed, with the warning naming him at a door not his. The move now
  brings the children under his roof (`camp::send_home`, dependants: "... moved in with family at Block#21, with
  Cerys Novak"; 295 such moves at 20k x 30). The warning is heard by the free parents living at that door only. A
  catch-all at midnight before the ladder (`camp::follow_parents`): a child whose Home holds no free parent goes to
  live with his first free parent with a standing Home elsewhere (a `Housed` line: "Lorcan Sato (8 y) went to live
  with Dorian Sato at Block#2145: no free parent at home (Jocasta Marlow in the cells, Dorian Sato not living there)
  at Block#545"). Before the move-in fix it fired 191 times (54 on day 1); after, 6 (a mother in the cells, the
  children to their father).
- **The `[family]` line after a take** (`camp_parent_23527 d25 09:30`): the take sets the child's routine to the
  Camp's (the day's was picked at midnight before it), so the 09:30 line reads "in Work Camp#4835" the morning of
  the take; `send_home` sets it back to Play. The take spends the warning (`warned_day` cleared; a let-home sets a
  new one), so a Camp child no longer reads "on the service's warning". The line names the child's Home when it is
  not the parent's ("at home at Block#X").
- **A Camp answer due outranks a search day** (`household::camp_visit_due`, gated in `search::considerations`):
  once the week's answer's hour has come and the child is still in a Camp, the search branch of the Pitch goal
  is off until the answer is made. T5's re-pinned buy-out probe runs with the search on again (the premise
  restored, the reason in place) and passes.
- **Deviation: "hospitalised" is `World::wounded`.** The city has no admission; a parent downed or laid up until he
  rises is not free. A parent in cuffs (an arrest's walk, a carry) counts as held.
- **Deviation: the catch-all's no room check.** A child is a dependant (`send_home`'s rule, T1): it goes to the
  parent's Home whatever its capacity, as a let-home child does.
- **Numbers** (20k island seed 42 x 30, `--scaled --ambient-flats 50`, before = main `85886ba`, one run each):
  ChildTaken 76 -> 73 (street takes 45 -> 41; at home 31 -> 32: 25 on a second warning, 5 with no free parent);
  daily ch_camp mean 13.0 -> 17.3 (d29 58 -> 67); warnings 646 -> 531; Custody 339 -> 291; ch_street mean 75.9 ->
  68.4 (d29 178 -> 133); let home 6 -> 1; followed a parent 6; buy-outs 0 -> 0, break-outs 0 -> 3; starvation
  deaths 1 -> 0; violence deaths 80 -> 88. The `warned_day` clear at the take landed after the run (a Camp child's
  warning read nowhere but the shadow).
- **Shadow** (20k seed 42, `target/u2_shadow*`; `neglect_parent` and `camp_jobless` are new `--pick` archetypes):
  the second warning's take and the line after it (`neglect_parent_19571`, Wisp Adeyemi, d10 00:10 three takes
  "unfed 3 days again, inside the warning of d7", 09:30 "Nadia Kettering (4 y) in Work Camp#4835 (D6 The Vats)");
  the warning at the child's own door, heard by the parents living there and the 30 days (`neglect_parent_17601
  d21 00:10`: "... the door of Block#1696: Gwendolyn Santoro (1 y) went 2 days unfed; Lux Dunmore and Sabine
  Santoro heard it: 3 days' grace, and unfed again before d51 the service takes them"); the warden's count
  (`camp_warden_14571`: 21 -> 30 children d21-d26, every child ate). No-free-parent takes show in the run's events
  (above), not in a pick's diary.
- **Reads wrong / open** (the next pass's input): at 20k the one Camp (Work Camp#4835, the Vats) is 8-11 h from
  every Camp parent picked, so every week's answer is "could not make the trip" (`camp_jobless_11341 d24`, `_11409`,
  `_24312`) and a visit against a search day cannot happen in a shadow: the probe is the proof. The warning
  groups a Block's children of two families into one line with four parents heard (`neglect_parent d10`, Block#3086).
  A parent at 0c with "no friend to ask" has three days to find 3c, and the pantry at 0-2 runs on the
  child-food debt for days before the second neglect lands. Buy-outs still 0.
- Under-18: nothing here touches a victim pick, a fight or a punishment. The service's take is the ladder's only
  teeth; the Camp feeds every child (unchanged), and the moves are a set_home at midnight. No test (doctrine 2).
  `SAVE_VERSION` 93 (`[camp] warn_repeat_days`, the `Jobs2Cols` `camp_followed`, `camp_take_repeat`,
  `camp_take_no_parent` columns); `analyze_run.py` prints the three.

### U3, the starving broke reach a meal (`OPEN-235`; `charity.rs`, `plan.rs`'s Eat, `exec`'s EatAlms, `[charity]` U3 keys)

**Dylan's call (2026-10-28): charity that runs short, not a dole.** The diagnosis came first. At 100k the kitchens
were not short of coins: `mission_purse` stood at 126,493 on d29 (V13) while they served 200-350 meals a day. Every
tithe of a creed lands in its first church (`faith::tithe`), so one kitchen held the city's charity and cooked one pot
a day with it. The other kitchens were broke, and the starving were out of reach (40 tiles, 80 starving, around 19
kitchens on a 1500x938 island). The Starving bursts sat on the mass `LodDemotion` ticks: the dead were off-screen
evicted adults, whom the 12:00 Statistical pass served only within 40 Manhattan tiles of a door. Built, on the
existing Mission path:

- **The starving's reach by trip** (`far_target`, `kitchens_by_trip`): a starving adult (`[life] starving_hunger`)
  reaches a kitchen with a bowl left within `starving_reach_ticks` (240) by `distance::trip` (the walk, or the El for
  a body that would board it) from where he sets out. Eat's order is the near kitchen (`reach_tiles`), then the bin,
  then the far kitchen (`far_alms_plan`, a `SoupLine` line: "set out for the soup line at Soup Kitchen#2930, 1 h 56
  away (starving, no bin nearer)"). G2's `hungry_reach_mult` x the tiles is retired for the kitchen (the bin keeps it).
- **The pot** (`servings_per_day` 150): the nightly restock buys the units that make it (at `synth_stretch`, under
  `stock_cap`) from the nearest Markets with stock, nearest first (the base took the nearest Market whatever its
  shelf, as its comment said it did not).
- **The line** (`queue_until`, `turned_away`; `exec` gives EatAlms its own `Wait`): inside a kitchen whose hour's
  bowls (`meals_per_hour`, doubled by a Volunteer) are out, the body waits for the next hour ("waits in the soup line
  ... the hour's 10 bowls are out, the next at 04:00"). With the pot dry it is turned away ("the pot at Soup
  Kitchen#2559 ran dry before Kestrel Pembroke's turn"), counted in the kitchen's day ("served 150 meals and 6 cots;
  the pot ran dry for 10 more in the line").
- **The off-screen line** (`stat_daily` at `soup_hour` 12): one line for the city. The Statistical hungry poor go
  hungriest first, each to the nearest kitchen in reach with a bowl left (the starving by trip): one bowl a day.
- **Deviation: the kitchens share a purse** (`share_purses`, midnight before the restock; `share_keep_days` 7). A
  kitchen that turned a line away with less than a pot's coins is sent the difference by the kitchens holding more
  than a week of their own pots, the most turned away first. It is a purse-to-purse move, the ledger's `Donate` line,
  and a `Donated` line ("Church of the Open Circuit#2591 sent 150 to Soup Kitchen#5436: its pot ran dry for 1 in the
  line"). Without it the pot and the reach moved nothing: the first run served the same 330 meals a day while 18
  kitchens ran dry beside a 54k purse.
- **Deviation: a soup line where the starving are** (`found_far`, `site_near`; `found_far_min` 20,
  `found_far_per_day` 3). After the off-screen line, take the hungriest starving body out of every kitchen's reach
  who has `found_far_min` others within the walk. The richest kitchen past its keep pays `found_cost.mission` (150,
  `Flow::Found`) to open a Mission on the vacant Lot (else the derelict) nearest him within half the walk, and sends
  it a pot ("Church of the Open Circuit#2591 paid 150 to open Soup Kitchen#2559 in D1 The Sump and sent it 150: 54
  starving out of every kitchen's reach there"). The first cut placed them at the district centroid: six kitchens
  stacked in the Sump and `soup_far` climbed to 1,778 a day.
- **The hall takes its line** (`charity::capacity_exempt`, read by `World::capacity_exempt`). `[buildings.mission]
  capacity` 12 is the night's cots. A packed line failed the walk at the door with `BuildingFull`: in the first
  shadow, `homeless_52040` walked 1-3 h to Soup Kitchen#2930 five times in d26-27 and was locked out each time.
- The kitchen's lines name it by its label (`kitchen_name`): `name_of` read the kind, so one kitchen was "Church of
  the Open Circuit#5432" in a plan and "Soup Kitchen#5432" in the line.
- **The under-18 rule**: the meal paths stay adult-only (`alms_eligible` and `stat_daily` test
  `demography::is_adult`). A child eats through the household and the Camp, and `World::kill_by` refuses a protected
  death (`harm_blocked` 0 in the run). Nothing here picks a victim, a fight or a punishment.
- **Numbers** (100k `isle_m_k2_100k` seed 42 x 30, `--scaled --ambient-flats 50`; before = `target/r4_100k`, main
  `a70800f`; one run each): **starvation deaths 30 -> 1** (before 23 on d29, the rest d26-d29; after one on d29,
  `Sprocket Reyes`, in the cells). **Starving events 13,404 -> 6,296.** Kitchen meals a day d22-29 199-346 ->
  2,924-4,421 (`meals_alms` 3,148 -> 35,949 over the run), with 30,206 starving fed. Turned away with the pot dry: 0-30
  a day to d17, then 316 (d18) to 6,073 (d28). The off-screen starving out of every kitchen's reach: 155 (d17) ->
  0-1 from d22, after 5 soup lines opened (d16, d17, d18, d21 x2). Waits in the line 533. The purse still ends at
  94,250 (before 126,493): the pots, not the coins, run short. Homeless max 13,117 -> 12,969; violence deaths 431 ->
  419; thefts 19,147 -> 16,442 (a FINDING: the fed steal less; the tone comes back through features, not a knob);
  `harm_blocked` 0. 20k (seed 42, the shadow's days 21-27) shows no broke starving body: its one Starving pick
  (`miserable_21501`) went to 0 in a Station cell, and the cells fed her.
- **Shadow** (100k days 21-27, `homeless:4,miserable:2`, `target/u3_shadow100k`). Reads right: `homeless_92259`
  sets out ("1 h 08 away"), waits for the 03:00 bowls, eats at 03:31 (d26). The next night the pot runs dry before
  his turn (d27 04:56) and he sets out for the next kitchen, "3 h 09 away". `homeless_72458` starving walks 2-3 h
  and eats three times in the week (Church of the Open Circuit#5430, Chapel#2600). **Reads wrong / open** (the next
  pass's input): the 3 h 09 walk is cut at 90 min ("cut at street (D4 Mid-Levels)"), and at hunger 0 the body turns
  to Earn's Scavenge and back to its squat (`homeless_92259` d27 12:09), likely the far pot dry on the way. A starving
  body sets out several times a day for the same kitchen, one `SoupLine` line each. The one death left is a body "in
  the cells till d34" (the hold's food, not the line). In the first run's diary a body whose walk the pot outran
  starved 14 h after setting out (`Liesl Ramirez`, d28). A rich kitchen cooks one pot whatever its purse.
- No test (doctrine 2). `SAVE_VERSION` 94 (renumbered at the merge) (`Charity.short_today`, `EventKind::SoupLine`, the `[charity]` U3 keys, the
  `Jobs2Cols` `soup_starving_fed`, `soup_queued`, `soup_short`, `soup_far`).

### U7, contradictions (`OPEN-237`, `OPEN-222`'s leftovers; `law::raise_crime_parties`, `law::accomplice`, `crew::muster_reach`, `contracts::spent_locate`, `Flow::FareCut`, `industry::foreman_on_hand`, `[chrome] strip_heir_salience`)

Each at its cause (20k island seed 42 x 30, `--scaled --ambient-flats 50`, before = main `85886ba`, after = the
branch, one run each; counts from the event log by name; shadows `target/u7_shadow`, `u7_shadow2`, `u7_shadow3`):

1. **The thief "was robbed"** (`exec::steal_food`): a Home theft's victim was the first body in the building, so a
   visiting thief who had just stolen there was the next thief's victim (`homeless_71553`). The victim is now a
   resident of that Home who is present (`World::residents_of`), never a visitor; none present, no victim.
2. **The victim and the accomplices witness the deed** (`law::raise_crime_core`): the witness roll dropped only the
   actor and `except`. It now drops the victim, the deed's object, the named parties (`raise_crime_parties`: a
   shakedown's payers, who had `victim: None`) and the actor's accomplices (`law::accomplice`: a member of the same
   gang on the same goal). The victim and the parties still know who did it: a first-hand `SawCrime` (what
   `ReportCrime` reads; the roll gave them one before) with no Witness line and no witness's reaction, and a guard on
   shift who is the victim files at once. A body shakedown's coins move on the books (`Flow::Coerced`, as the gang's
   tribute), so the payer's money line names it: `courier_15714 d21 23:39` "-5c (Coerced) ... flows: Coerced -5 to
   Kuro Sallow", then "memory WasRobbed of Kuro Sallow", "memory SawCrime of Kuro Sallow (Extortion)", no Witness
   line (V13: "(other)" and "saw ... commit Shakedown"). Counts: an assault's victim "saw" his own assault 372 -> 13
   (the 13 are raid brawls: one raider hits several defenders in the minute, a defender saw the blow on another);
   mutual sightings of one Shakedown within the hour 16 -> 3; Shakedown witness lines 1,397 -> 757; all named
   Witness lines 12,938 -> 11,498, Report 1,545 -> 1,542, Arrest 1,131 -> 1,033.
3. **"Sat out the raid" while marching** (`crew::muster_reach`): the 00:07 duty pass read the muster trip from home
   for a member already on his Raid plan, past the muster (`raider_84195`). A member on his Raid plan is in the raid:
   no line, no muster post. Instrumented once (removed): 125 such members in 30 days (98 in the shadow's week) no
   longer get the line or a second post. All "sat out" lines 121 -> 158 (the rest are members off the march past
   `duty_reach_ticks`, the city moving under the other fixes). No raider of 15 picked marched in the window.
4. **A guard "on" its own buyer; the week-long stakeout** (`contracts::post`, `contracts::spent_locate`). (a) A Guard
   or Escort is posted *for* its ward, a bodyguard for "himself"/"herself": "posted a guard on X" with X the buyer 67
   -> 0, "for himself/herself" 0 -> 56; `exec_24735 d24 00:06` "Kessler 2 posted a guard for Block#2200". (b) The
   stakeout: a brokered Locate escrows its price (30-40c) while `cap_sightings` x `per_sighting` is 150c; after
   three paid sightings every contact was refused ("the sighting went in, unpaid"), the cap never came, and the
   taker re-planned a 2-minute StakeOut every 5-20 min for days (20k `gun_23612`: 49 stake-outs on Florian Upton, no
   sale; V13 `gun_41438`). A live taker's contact that finds the payer spent settles the record as the ledger
   tracker's report does (fulfilled on the sightings it bought; a direct buyer short with none bought reneges; else
   back on the board). Locates fulfilled 50 -> 74, taken 102 -> 72 (spent records no longer cycle takers), 28 closed
   spent (each after 3 paid sightings). A refusal for the 6-h gap is not "spent" and still re-stakes (left).
5. **CabFare from one Friend** (`streetwork::driver_cut`): the 3-4c lines were the driver's cut, paid by the
   Garage's owner (an agent who owns the Garage) under `Flow::CabFare`, so they read as a rider's fare. The cut is
   `Flow::FareCut` (taxed and in `flow_other`, as before: no ledger number moves): `cabbie_14444 d22 09:13` "+7c
   (FareCut) ... flows: FareCut +8 from Ivo Voss" (the owner).
6. **The sick foreman** (`industry::picker`): the Hire's and Layoff's named picker is the foreman (or the gang's
   man) at work today, `careers::on_hand` (not off sick, not held); the midnight foreman flag and his premium are
   unchanged. Hire/LaidOff lines naming a foreman off sick that day 43 -> 8 (6 at the seed's tick 0, 2 at the 00:10
   hire pass before that midnight's sick roll).
7. **The CEO's 585c strip** (`chrome::strip`): the street was empty (nobody within `sight_day_crime` at 08:20; at
   20k half the body strips are witnessed, 51 of 95), and only an heir in sight ever learned. An heir out of sight now
   learns at the settling (strip settles the body at once): an unnamed `Stripped` memory at `[chrome]
   strip_heir_salience` (0.5); no name, so no grudge, report or hunt. The windfall stays the stripper's luck. Strips
   of 100c+ 2 -> 5, unseen 0 -> 4 (1,179c).
8. **The jailed exec's ExecWage** (`ownership` daily exec pay): an exec held in the cells keeps the chair and draws
   no salary that day (a worker in the cells works no shift). Read from the code: no exec was jailed in the 5 exec
   diaries.

**Deviations.** Not taken: the released man who never searches (`loader_74097 d26 16:13` goes straight to
`JoinGang`, a choice his scores explain, not a contradiction); the jobless pimp (U6). The off-screen shakedown
(`gang::stat_extort`) keeps its wallet moves (the payers' money line there is a stand-in); its payers gained the
under-18 filter the body path had.

**Under 18.** No victim pick widened: the Home theft's victim is still `!ages::is_protected` (now also a resident);
the body shakedown's occupants keep their filter, and the off-screen shakedown's payers gain it (S6 had missed it).
The witness changes only remove witnesses; no fight, punishment or target pick changed.

**Reads wrong / open** (the next pass's input): with the shakedown on the books the camera now flags its payer
asleep ("SETTLED APART: Coerced 2c to; Kuro Sallow beside me; I am asleep", `courier_15714 d22 00:35`): an
Intimidate "SUCCESS" against a sleeping household, older than U7, newly visible. A Locate taker still re-stakes every
few minutes through the 6-h gap between paid sightings. Mutual Theft sightings 5 -> 15: two thieves in one Home
now each see the other (no longer each other's "victim"), which reads right.

Re-pinned on its premise, the reason in place: `bodies`' `test_refound_only_after_refound_days` asserts the refound's
brothers arrived (the event's arrivals), not that the gang still holds them days later (the moved city lost them to
the cells and the street before the check). No test (doctrine 2). `SAVE_VERSION` 95, renumbered at the merge (`[chrome] strip_heir_salience`
in the saved config; serde default).

### U6, the pimp and the ripperdoc at work (`OPEN-217`; `vice.rs` the round and `name_pimps`, `care.rs` the bookings and the wash, `service::pick_server`, `exec::routine::shift_pending`, `search.rs`, `demography.rs`, `[trade_verbs]`, `[care]`, `[verbs]` U6 keys)

- **The cause** (a 20k base shadow and run, main `85886ba`; the 100k V13 picks reproduce at 20k). U5's three
  causes, and the tier is one of them. (1) **No market**: the street's off-screen payer is three draws an hour, so 82
  street visits in 30 days at 20k (410c), a pimp's collection 2c or nothing, and a pitch dry `[jobs2] pitch_fail_days`
  closes: a pimp's district empties and the round stops (`pimp_18110 d23`: no pitch hours to d27). A clinic's trade
  was the chrome alone: 25 Clinics, a median of 5 patients a week, half at 00:00. (2) **The colleague and the clock
  took the patient**: the counter's pick (`service::pick_server`) was the pair hash over the present Patient staff,
  the off-screen ones present by the clock, with T4's body-first rule for `Visit` only; and `stat_shop`'s chrome was
  installed in place at midnight, when only a night orderly is on (V13's `orderly_13352`, 5-6 every 00:00; half named
  nobody). (3) **The label**: `name_pimps` fell back to the lowest-id member holding a Job when no jobless one was fit
  (the round 4 search found most of them work): V13's Engineer, the 20k `pimp_21194` a Sanitation hand, asleep through
  every pitch night, GangWork a daytime Extort; and it named the jobless member nearest the pitches at any distance (the
  20k Sump's pimp at 771 tiles, a 347-minute trip, past S1's `gang_work_ticks` every hour: `round_pitch` returned
  nothing, no round ever). A jobless pimp searched like a labourer (V13 `pimp_84453`; T5's search knew no pimp).
- **The pimp's round** (`vice::collection`, `book_client`, `tell_collection`): each collection with a body on either
  side is told in both diaries with the take since the last round, the cut and the gang's share ("came round the pitch
  of X (37 y) and took the cut: 2c of 5c since the last round (1 client), 1c of it up to The Drowned Choir 8"). At a
  dry pitch (nothing since his last collection) the pimp sends her a client (`[trade_verbs] pimp_book_draws` 8): the
  first of eight off-screen district residents with the street's wants and twice its price, never one
  `ages::is_protected`; he pays her `street_price`, the take the cut is taken from ("sent X (73 y) to the pitch of Y (37
  y) (5c) and took the cut ..."). An off-screen pimp credited present books too (the pitches of a pimped district stay
  open). **Deviation:** the spec named "a booked client" for the pimp; the madam's bookings (R6a) are the house's, so
  the pimp's is the tout's: the client he brings.
- **Who holds the street** (`name_pimps`): nobody whose trip from his Home door to the district's nearest pitch is past
  `[trade_verbs] pimp_trip_ticks` (120, as S1's `gang_work_ticks`, which `round_pitch` read before and reads now from
  where he stands) is named (`pimp_round_tiles` in tiles without `[distance.choice]`); the jobless fit member first,
  else the fit member nearest the pitches, who quits a day post for it (`pimp_quits`: "quit as Sanitation (to run The
  Drowned Choir 8's street in D4 Mid-Levels)"); a district with no member that near has no pimp. A named pimp is off
  the job search (`search::jobless`), the midnight hire (`pick_candidate`) and the door hire (`can_take`). A street
  that changes hands is told with its reason ("Yorick Park lost the street of D3 Neon Quarter to Kenji Yates (Rust
  Saints holds the district now)"; in the cells, away from every pitch, holding a day post, living too far, no pitch
  left, no gang holding). **Deviation:** the bound costs pimps: the shadow's 20k city had none on d21 (a 120-tile bound,
  then the 120-minute trip). At 180 minutes it named one, but the 30-day city's collections fell 66 -> 5: the far
  members named were day workers who quit and, off screen, were "away from every pitch 3 days". So 120 shipped.
- **The clinic's day**: a patient goes to a doctor's body on duty with room first (`[care] clinic_body_first`; the
  pair hash among the bodies, then among all); `stat_shop`'s chrome is booked for an hour of `book_hours` (9-17, the
  Ripperdoc's shift) and installed then by the doctor at the counter (`care::book`, `keep_bookings`, hourly from
  `care::hourly`); a shut Clinic's patient goes to the nearest one open, else the next hour, past 17:00 let go (a used
  implant of the booked stock waits for its Clinic). The second trade (`[care] wash_*`, `care::book_washes`, `wash`):
  an off-screen adult carrying `wash_min_tox` 0.4 or more (the synth diet's) with `wash_price` 8 above `wash_keep` 10
  books a blood wash on `p_wash` 0.05 a day at the Clinic nearest his Home, paid as care (`pay_fee`, the doctor's
  `doctor_cut`), half his toxicity out ("had the blood washed at Ripperdoc#N by D (8c, 2c of it to the doctor)").
  **Deviation:** the spec named the patients reaching the body; a trade of chrome alone has no week at a quiet Clinic
  (U5's forger licence, the clinic's wash). At 15c over 20c and 0.01 a day it fired 8 times in 30 days (the toxic are
  the synth-fed poor), so the price was set where it fires, not tuned. Prosthetics, chip installs and detox stay at
  midnight (rare: 11 lines at 00:00 in a week against 254 in the day).
- **No Rest at the post hours before the shift** (`[verbs] wait_at_work_ticks` 120): a body already inside his
  workplace waits for a shift at most two hours off (the routine's 600: V13's ripperdoc came in at 04:00 for a wage and
  rested five hours); a day post's hire keeps the base rule's wait. The 20k doctors' pre-shift Rest is 33-59 min.
- **The under-18 rule** (`ages::is_protected`): the round passes a protected owner's pitch (`round_pitch`), no
  protected member is named pimp (`name_pimps`' `fit`), the pimp's client and the street's own (`client_near`,
  `street_payer`, `book_client`) are never protected, and the street worker is never protected (`book_client`, as well
  as the opener's adult age). The wash takes adults only; a booked install keeps `stat_shop`'s Statistical adults.
- **Shadow** (20k `isle_m_k2`, seed 42, d21-27; the brief's picks; the picker's `ripperdoc` is now a Ripperdoc post's
  holder, an owner only with none: shadow 3's `ripperdoc_7409` was an absentee owner with no Job). The pimp (the build with the booking and the line, before
  the naming bound; its wording since reworded and its gang printed by id, since fixed): `pimp_18110 d21 00:07`
  "Kieran Hawthorne sent Ghost Ashcombe (73 y) to Silvio Wexford (37 y)'s pitch (5c), and took the cut: 2c of 5c since
  the last round, 1c of it up to" the gang; `d21 23:11` "collected 2c of Silvio Wexford (37 y)'s 5c since the last
  round (1 client)"; `d22 01:10` a client sent to Brenna Mbeki (48 y); `d22 03:03`, `d22 23:40` the round at Idris
  Sato's and Brenna's pitches; then no pitch due from d23 (the dry street closing: the booking reached the round's
  pitches only when the pimp's body stood at them, so it was opened to the off-screen pimp too). The final builds'
  pimp picks did not hold the street a night: shadow 4's two "lost the street ... (The Drowned Choir 10 holds the
  district now)", "(Rust Saints holds the district now)" at `d21 00:07`; shadow 8's `pimp_12173` (Lena Kowalczyk, the
  base's orderly, named at d19) "lost the street of D4 Mid-Levels" at `d22 00:07`, her Home moved 313 tiles off (and
  the same night twice, a dropped-district line: fixed), then worked a Club's floor all week ("a room of 15, served
  8"). The quit: "Quill Thorne quit as Sanitation (to run ...'s street in D4 Mid-Levels)" (20k run, d16).
  The clinic: `agent_10309` (Vale Vasquez, the orderly at Ripperdoc#5566, `--agent`): `d21 09:00` "Vale Vasquez patched
  Juniper Pembroke up at Ripperdoc#5566 for 12c (3c of it to the doctor)", `d21 12:37`, `13:00`, `14:00`, `16:00`
  installs "by Vale Vasquez" (V13: every install at 00:00), `d23 13:34` a gang's ("(gang: The Iron Knives)"), 19 lines
  in the week, the `[memory] treated X (a patient)` beside each; `ripperdoc_9562 d23 09:00` "Corwin Brandt patched
  Winifred Ravensworth up at Ripperdoc#5466 for 12c" and a cure; pre-shift Rest 33-59 min.
- **Reads wrong** (the next pass): the clinics' trade is lopsided by catchment and by the shut: a Clinic opens only
  with its Ripperdoc at the counter (`fixes::staff_present`), so one with the post vacant, a doctor walked out unpaid
  (`ripperdoc_9896 d23 14:51`, an under-water owner's cut hours) or on his rest day sends its patients to the few open
  ones: 20k x 30, one Clinic took 118 of the week's 265, the median 4; `ripperdoc_14015` and `ripperdoc_14363` at the
  counter all week with no patient. The night orderly has nobody now (the Clinic is shut at night; the installs moved
  to the day). A district's street changes hands with its gang's splits (D4 six times in 30 days among The Drowned
  Choir 8, 9, 10), and a named pimp quitting his day post is taken on again by a graded post's hire (`d9` "Yorick
  Fletcher quit as Researcher (to run ...)", `d10` "quit as Manager"): the graded and skill-ranked hires are not
  `pick_candidate`'s. A quitter named off screen is often "away from every pitch 3 days" (an off-screen pimp is present
  only while he follows his gang's order). The 20k street is thin by geography: 4-6 pitches, and the gangs holding
  their districts rarely house a member within a commute of them, so a pimp is named on about half the days. No
  shadow of a working round on the final build: the shadow's pimp pick is made at d21 00:00 before the midnight pass
  renames. Not re-shadowed at 100k (the causes show at 20k).
- Numbers (prints, not claimed; 20k seed 42 x 30, `--scaled --ambient-flats 50`; base `target/u6_base_run.csv`, after
  `target/u6_run3.csv`): pimp collections 47 -> 66 (pass-up 19 -> 17), clients the
  pimp sent 11, street visits 82 -> 113, street take 410 -> 565c, pimps' cut 34 -> 32c, pimps 0.73 -> 0.67 a day;
  off-screen chrome booked 2,039, kept 1,329 (installs 1,275 -> 1,124), washes 424, patients a doctor's body served
  757 (a body doctor's week, ~177 across the city), treatments 63 -> 66, patched 1,702 -> 1,537. Clinic events d21-27:
  12 of 271 at 00:00 (base 133 of 238), 252 naming the doctor (base 158), 24 Clinics, the busiest 96, median 3. (The
  first build, before the naming bound, made 89-104 collections a run; the bound costs the named-but-far ones.)
- No test (doctrine 2). `SAVE_VERSION` 96 (renumbered at the merge) (`care::State.booked`, `Booking`, `Visit`; the U6 keys; the
  `Jobs2Cols` `pimp_booked`, `clinic_booked`, `clinic_kept`, `clinic_body`, `clinic_washes`, printed on
  `analyze_run.py`'s street line). Premises the moved city broke, re-pinned with "Round 5 U6" in place:
  `succession::test_brothers_after_a_gutting_sack` (the Hollow's brothers, not any gang's), `camp::
  test_unfed_child_taken_at_take_days_before_death` (the living parents remember: one died in the starving week),
  `fviolence::test_short_riot_rolls_its_hours_not_a_day` (the two worlds part before the riot; a day-long riot kills
  many, no identity across the switch).

### U8, the GangWork hotspot (`OPEN-229`; `gang::gang_work_pick`, `gang::guard_test`, `World::chromed_homes`, `World::guard_grid`, `utility::goals`)

**The scan.** Instrumented once at 20k (seed 42, day 21; the timers removed): 75 % of `goal::GangWork` was the
Purist branch of `gang_work_pick`. A Purist gang shakes down the chromed's Homes first (M15 W31), and a chromed Home
anywhere beats a plain one nearby, so the pick walked **every Home in the city** per call, running `holder_of` (a walk
over every gang's territory) on each Home that survived the cheap rejects (9.2M `holder_of` calls in one 20k day,
~184 us a pick). The pick also ran twice a member think (the scav check, then the walk), and `guard_test` built a
fresh grid of every guard on each pick.

**The index.** (1) `World::chromed_homes`: the Homes a Purist could shake down for their chrome (a resident with
`Kit.visible` over `creed_tolerance`, standing, not derelict, inhabited, held by no gang: the territories read once a
rebuild), by door on a `BuildingGrid`, rebuilt on the first query of an hour. The Purist pick is two grid searches:
the nearest chromed Home on that grid, else the nearest Home on the building grid (the non-Purist search, as before);
the Contest branch reads the rival's list as before. Every gate is re-tested live in the score, so the only thing
the hourly list can do is miss a Home chromed or freed within the hour (it waits for the next hour); a Home that
left the list is never taken. (2) `World::guard_grid`: the guards' tiles on a grid, read on the first query of a
tick, shared by every pick (the Squat branch too); a guard who moved later in the tick is read where he stood.
(3) `utility::goals` GangWork picks the target once a think. No numbers added (the hour is `TICKS_PER_HOUR`, the
guard grid's 16-tile cell the old literal); no state saved (`#[serde(skip)]` memos), `SAVE_VERSION` unchanged (96 after U6).

**Numbers** (100k `isle_m_k2_100k`, `--scaled --ambient-flats 50`, seed 42, a day-25 save made by main `b4fc720`,
days 25-27 under `--features prof`, one run each; `target/u8_perf/before.err`, `after3.err`): `goal::GangWork`
**75.6 s -> 21.6 s** (25.2 -> 7.2 s a game day; 30.1 % -> 6.9 % of the profiled ticks; 3.61M calls each); it is now
below `goal::Unwind` (25.3 -> 29.7 s) and near `goal::Arrest` (14.2 -> 16.1 s). `think::think_with_offer` summed
232.5 -> 206.9 s. The per-tick chromed rebuild was 7.5 s of an intermediate run; hourly it is 0.1 s. Ticks/s
18 / 18 / 16 -> 12 / 12 / 19: **a print on a shared box** (U6 and the fix round building): every site the phase does
not touch ran 10-25 % slower in the after run (`lod::run_statistical` 39.6 -> 48.8 s, `goal::Eat` 14.8 -> 17.9 s,
`tick::think` wall 80 -> 115 s for less summed think time), so the ticks/s are not read.

**Shadow** (20k `isle_m_k2` seed 42 days 21-27, `gang_member:2,gang_leader:1`; `target/u8_shadow_before`,
`u8_shadow_after`). The run diverges (the hour-old list and the tick's guard grid move a few picks), so the members
picked differ; the leader is the same man (Zed Abernathy, The Grey Lanterns). The choices read the same: the same
GangWork lines (`target 1.00->1.00`, `order`, `travel`), the Home shaken down near the member (before
`gang_member_6468 d21 04:15` "PLAN GangWork on Block#1007 ... travel 0.45->0.82", "extorted 5 coins from
Block#1007"; after `gang_leader_14064 d21 05:54` "GOAL GangWork ... travel 2.15->0.48", "PLAN GangWork on
Block#235"), the scav round (`gang_member_26117 d22 03:17` "the scav round 1.00->1.00 ... Scavenge->Street
Market#4418"), the Squat order's Occupy (`Block#2207`, three mornings), and a Purist member of The Unplugged
(`gang_member_6435 d24 18:15`) whose GangWork is the creed's Preach, as the cult member's was before
(`gang_member_10538`, 8 Preach plans). No diary shows a Purist shakedown in the window (the Unplugged member preached),
so the chromed-first pick is read from the code: the same Home as the walk it replaced, but for a Home chromed or
freed within the hour.

**Deviations.** The brief's "cache or index": both (a grid index of the chromed, a per-tick guard grid) plus the
duplicate pick removed. Not taken: `holder_of` stays a walk over the gangs (0.9 s in a 20k day once the Purist
walk was gone); `goal::Unwind` is now the top goal (`OPEN-229`'s next read). No test (doctrine 2).

### V3, violence with consequence (`OPEN-238`; `demography::find_bodies`, `demography::found`, `law::hue_and_cry`, `law::tell_the_beat`, `law::file_report_as`, `weapons::market_petty`, `wounded::laid_up`, `think::turn`, `contracts::daily`, `routine::workday_of`, `[crime.case]`, `[weapons] stall_rob_gap_hours`)

Each at its cause (20k island seed 42 x 30, `--scaled --ambient-flats 50`, before = main `794bc6d`, after = the
branch, one run each; counts from the event log by name; the one 100k run is the branch's 100k x 30 counters, for
the Fixer's bust that no 20k run has):

1. **A death is found** (`courier_48760 d25 05:25`). SawCorpse came only at the death's minute (the bodies in sight
   then: at 05:25 in a Block, nobody awake) and after two days (`corpses`), and nothing turned a body into a case: a
   Murder with no noticing witness left no report at all (and a witness's ReportCrime walks to the one Hall in Civic
   Core, past `[distance.reach] report_ticks` from most of the 100k island). A `Corpse` now keeps its `killer`,
   `tied` (killer and dead had an edge at the death: a motive) and `found`. Every `[crime.case] find_every_ticks` (5)
   each unfound, unburied body of violence is looked for: the first waking, on-screen body that sees it
   (`law::sees_scene`, `[crime] sight`; never through a wall, never the killer), or the Gravedigger at the burial,
   finds it: SawCorpse, a `CorpseFound` line, and the law's case. The killer is named on `lead_p` 0.25, or
   `tie_lead_p` 0.6 when tied, x `competence::law_mult`: a Murder report with no witness and its own line ("the law
   named X for the Murder of Y (who knew the dead; the body found by Z)"); else "no suspect"; a killer already wanted
   for Murder is noted. 20k after: "Beatrix Adeyemi found the body of Quill Draper at the street (D4 Mid-Levels); the
   law opened a case and has a name" (d1 19:30), "the law named Smoke Wozniak for the Murder of Quill Draper", "Rafe
   Crane arrested Smoke Wozniak for Murder" (d4), 30 days. 60 of 91 bodies found inside the hour, the slowest at 25 h.
2. **A looted till is reported and the thieves known** (`vendor_9648 d23`). The till was the district's *lowest
   owner id* with a take: every Market thief in the Sump drained one stall (main 20k: Clemence Nakamura four times,
   twice in 6 h). Now the nearest stall to the thief, not one emptied in the last `[weapons] stall_rob_gap_hours`
   (24), never a minor's. The owner already knew each thief (the victim's first-hand SawCrime, U7); what was missing
   was a way to the law: **the hue and cry** (`law::hue_and_cry`, at a crime's living victim: an on-duty city guard
   outdoors within `[crime.case] hue_reach` 12 takes the report, the suspect last seen there) and **the tell**
   (`law::tell_the_beat`, hourly: a victim, or a Murder's witness, holding an unreported first-hand sighting from
   the last `tell_days` tells an on-duty guard on the street within `hue_reach`; one tell a person a pass). Both read
   ReportCrime's filters (`tell_lawful_min` 0.3, no gang member unless betraying, not keeping it to sell), and the
   teller is awake, adult, on his feet and wanted for nothing. 100k: "Amara Upton emptied the till of Volt Ito's
   stall (4 coins)", "Volt Ito cried out and Clemence Ramirez took the report: Amara Upton for Theft" (d15 10:28),
   "Lux Tanner arrested Amara Upton for Theft from Volt Ito" (16:34), 3 days. 100k: 418 till thefts on 81 stalls, at
   most 12 a stall in 30 days (V14: ten on one in six hours). 20k shadow (`target/v3_shadow2`, `guard_24210`), robbed
   of a Grand Theft off shift at d25 19:44: "Gideon Bellamy told Kasimir Inchbald on the street: Nico Crane, for Grand
   Theft" (d27 01:00).
3. **The wounded** (`ceo_91004`, `guard_8429`). Both *were* carried and stabilised (V14 `d24 06:51 Rescued ... in
   time`); what read wrong: a downed body thought in the tick its carrier's aborted plan let it go (`tick_down`
   re-cuffs after; the CEO picked Flee at 05:53 and the needs line read `Some(Flee)` 72 h); a stabilised patient was
   neither asleep nor thinking, so its energy drained to 0.01 every night; the ExecWage gate read only `Sentence`;
   the licence pass sold him a weapons licence from the Ripperdoc's floor. Now `think::turn` skips the downed, the
   stabilise clears the goal, `wounded::laid_up` (stable, under a roof) counts as asleep in `needs::run`, the exec
   pay and the licence buyers skip the downed. The guard was already unpaid down (no duty goal,
   `credit_guard_shifts`). No laid-up exec in the 20k picks: read from the code.
4. **A Fixer closed is closed** (`fixer_40483 d26 00:09`). `heat_daily` (the bust) ran in `law_and_street_daily`
   after `hire_pass` and the Hit market's posting passes in the same `contracts::daily`; it now runs first, so the
   night's posts find the office shut (`fixer_open`) and go to another book. `routine::workday_of` reads the shut
   office: its staff have no shift (no wage) until it reopens, and the `FixerBusted` line names them. 100k (d17
   00:09): "the D4 Mid-Levels Fixer office was closed by the law for 7 days; its staff of 2 sent home", the same
   minute's posts on Fixer#5120 and #5801, none on Wisp Varley's book until the minute it reopened (d24 00:09).

**Numbers** (20k x 30 before -> after): Murders 50 -> 60, bodies of violence found 0 -> 91 (of 92 violent deaths;
24 named, 64 no suspect, 3 already wanted), Murder reports 7 -> 32 (0.14 -> 0.53 a Murder; 24 of them the case's),
Murder arrests 6 -> 26 (0.12 -> 0.43 a Murder), all Reports 1,393 -> 1,579 (hue and cry 12, tells 29), all Arrests
974 -> 1,106, till thefts 6 -> 4 (four owners). The longest laid-up span (down to `Recovered`) 76.6 h -> 77.8 h
(median 74.1 -> 74.5): `[wounded] recover_days` 3 is the design and was not touched; the patient now sleeps through
it instead of draining. 100k x 30 (branch only): Murders 223, found 409 of 409 violent deaths, Murder reports 0.74
and arrests 0.61 a Murder, hue and cry 45, tells 147, FixerBusted 1. Two earlier 20k runs of the branch (before the
till and tell fixes) gave 0.90/0.73 and 0.75/0.58: the ratios move run to run, the shape holds.

**Deviations.** The till stays the "on the thief's way out" abstraction: a reach cap on the stall (the owner out on
the street within 10 or 40 tiles of the thief) took the 20k thefts 6 -> 0 and 1 (stalls sit far from the Markets; a
Statistical vendor keeps no street tile), so not taken (doctrine 5). Not taken: the member dying mid-Extort (V2's
brief); a laid-up guard posting no cover at the Precinct (V14 corp's mechanism 4); the killer's own week after
(labour's "Haunted", a skipped shift). The found-body finder skips the Statistical (U4's indoor rule); off camera a
body is found at its burial. `CorpseFound` is one new `EventKind`; the case's and the tells' reports are `Report`
lines with their own text (`law::file_report_as`).

**Under 18.** No victim pick widened. The till's owner gains `!ages::is_protected` (S6 had filtered the pickpocket's
victim only). The case never names a protected killer (it says "no suspect"), and neither the hue and cry nor the
tell files on a protected suspect: no warrant, so no arrest force, on a child. A finder may be any age (a find harms
no one). No fight or punishment changed.

**Reads wrong / open** (the next pass's input): 64 of 91 found bodies at 20k stand with no suspect and nothing comes
after (the street's word naming a killer reaches no guard; a case could be named later from it). The 20k shadow's
picks (`target/v3_shadow` .. `v3_shadow5`, 63 diaries) met no found body; item 1's proof is the event log. Two
bodies in the Precinct's cells (`v3_shadow4` d26 00:13 and 00:49, "Killed by gang Ninefold") are the cells' own
question. A tell 29 h after the robbery reads late but right (a guard robbed off shift met an on-duty colleague the
next night).

No test (doctrine 2); the arms till probe holds on its premise (one stall, at the thief's tile). `SAVE_VERSION` 97,
renumbered at the merge (`Corpse.killer`/`tied`/`found`, `ArmsState.till_robbed`, `EventKind::CorpseFound`,
`[crime.case]`, `[weapons] stall_rob_gap_hours`; serde defaults).
### V2, turf you hold (`docs/TURF.md` § 1-2; `OPEN-241`, `OPEN-217`; `systems/turf.rs`, `[turf]`)

- **Cells and the holder** (`turf::daily`, at the gangs' midnight slot before the faction brain): the map in squares
  of `cell_tiles` (24); a gang's day of presence in a cell is `w_held_home` a Home or squat it holds (claim at 3),
  `w_resident` a member living there, `w_hideout`, `w_front`, and its bodies' hours sampled hourly (`turf::hourly`:
  `w_street_hour` on the cell's street or in a venue there, `w_corner_hour` on its corner); a Statistical member's
  day credits `stat_street_hours` to his corner's cell (his Home's without one): the off-screen mirror. Over
  `presence_days` (7): held past `hold_min` (30) and `hold_margin` (0.25) ahead of the next, contested when the next
  is within the margin at `keep_share` of `hold_min`, else open; a holder keeps its cell at `keep_share` unpassed.
  Jailed, dead and under-18 members count for nothing. The first pass seeds silently.
- **The holder change** is one place, `turf::set_status` (V5 flips through it): a `Turf` event naming the gangs,
  up to three members of each living there (else on that corner) and the landmark ("The Unplugged 10 hold the
  blocks around Street Market#4423 now, taken from The Unplugged 5: 1 of their people on the corner most nights";
  "The Drowned Choir 10 are on The Drowned Choir 9's corner: the blocks around Block#631 are contested (43 of
  theirs about the blocks, 40 of The Drowned Choir 9's)"), and for a cell taken from a gang `Shock::LostTurf { by }`
  (a grudge, severity 0.5 as a lost district). "On the corner most nights" is the window's median of distinct
  members on the corner a night; with none, "about the blocks" or "their Hideout's blocks".
- **The corner**: the outside door of the cell's Bar, Market, Club or NoodleBar nearest the mean of its Home doors,
  else the stoop of its fullest Home (**deviation**: the spec's "busiest street tile a member can reach" read as
  the venue door, stable and visible; the busiest by footfall would wander nightly). A member's corner is his Home
  cell's when his gang holds it, else the nearest held cell whose corner is within `corner_reach_tiles` (60) of his
  door. **Hold the corner** is a GangWork arm (`gang::gang_work_target` falls to it when the day's order work is
  done or there is none; `turf::corner_hold`, `corner_plan`: `GoTo(Spot) -> HangOut`), scored as the scav round is
  (`goals.rs`: "the corner" gate, loyalty, energy, hunger), between `corner_from`/`corner_to` (18:00-02:00), at most
  `corner_ticks_day` (240) a night, never on shift, under LieLow, inside or under 18. A Full body holding it is told
  once a night ("Beatrix Draper holds The Unplugged 2's corner by Club#5049 (3 more of theirs on it)").
- **The neighbour** (`World::rival_of` reads `turf::rival`): the gang holding the most cells bordering ours (a
  contest counts its other side), ties by its held cells, then the lower id; the old rule (most territory) for a
  gang holding no cell or bordering nobody. **District control from cells** (`districts::presence`): a standing
  Home counts `held_home` for its cell's holder, not its claimant (squats still by claim).
- **Turf that pays**: `turf::tribute` replaces the per-Home tribute: each Home of a held cell pays its holder
  (`tribute_held` 2 a Home it holds, `tribute_home` 1 any other), a held Home outside held cells pays its claimant 2
  as before, adults only, members of the payee exempt, `Flow::Coerced`, into the week's Collect. Protection by cell
  (`leverage::protection_target`, `turf::leans`, `protection::ground`): a gang leans only on businesses in cells it
  holds or contests; a business already paying one gang is squeezed by the other side only in a contested cell.
  **Deviation**: "or shuts early" not built (the squeeze's punishments already shut a till). The deal spot is the
  corner (`stims::member_site_trip`: with the corner on, the held cells' corners replace the districts' one; a venue
  or the arms Bar as before); the preacher preaches on his gang's busiest corner in reach (`leisure::busiest_spot`).
- **The pimp's pitch** (`streetwork::turf_pitch`, `vice::name_pimps`): a Street pitch opens at the corner or a spot
  of a held cell where the holder's members live, nearest the opener's Home within the job cap (else as before); a
  district's street is the gang's holding the cells its street pitches stand in (then most cells held there), and
  its pimp is named among the members whose Home cell it holds within `pimp_cells` (1) of a pitch's cell: this
  replaces U6's `pimp_trip_ticks` as the naming bound (the key stays for `round_pitch`'s walk). A street lost with
  the pimp's own blocks contested says so.
- **Recruits from the turf** (`gang::recruit_gang_any`): the holder of the recruit's Home cell takes him on a tie
  `recruit_affinity_cut` (0.1) weaker and before any other gang's, and first among the desperate's gangs; the join
  line says "asked on the corner of the gang's blocks, where the recruit lives".
- **The residents read the colours**: after dark a spot or venue in a contested cell is passed over by anyone not
  of its two sides (`leisure::spot_ok`, `nearest_venue`, both tiers' evenings); a Full body is told once a night when
  a contested spot lay within `avoid_tiles` (40) ("Cutter Greaves kept off the blocks around Street Market#4423 after
  dark (The Unplugged 5 and The Unplugged 4 at odds there)"). A family with children in a contested cell moves out
  (`turf::family_moves`, `family_moves_per_1000` 0.2 a day, the nearest Home with room it can pay outside any
  contested cell within `family_move_tiles` 120: "Zara Hawthorne moved the children out of the blocks around
  Block#713 (The Drowned Choir 9 and The Drowned Choir 10 at odds there), into Block#869"), and the commute
  relocation moves no family into one (`life::relocate_up_to_with`). **Deviation**: "a held cell is quiet" is not a
  mechanism of its own; it is the contested cell's avoidance and the holder's exclusive door (below).
- **V14's contradictions at their cause**: (1) a dead extorter collects nothing: `gang::extort` and `stat_extort`
  stop for a body no longer living or without a purse, also after the Intimidate (its backlash can kill); not
  re-shadowed (the V14 repro is a 100k pick). (2) One door, three gangs: Expand's door (`gang::free_door`, both
  tiers) is no other gang's: not in a cell another holds, not under another standing gang's claim (a door one gang
  works is that gang's); Contest still takes the rival's. Extortion 812 -> 588 and TerritoryFlipped 37 -> 13 in
  30 days follow (flips are V5's war now). (3) `BountyPaid` once a sighting: `contracts::on_sighting` pays every
  live Locate the observer sees and tells one line with the sum ("sold a sighting of Cato Volkov for 30 (2
  buyers)"; `pay_sighting_quiet`, `tell_sighting`).
- **The under-18 rule** (`ages::is_protected`): `gang::contested` makes no enemy of or for a protected member or
  actor (the gap the codemap found); the squat-eviction fight neither fights nor evicts a protected squatter, nor
  does a protected claimant fight (`squat_claim_with`, the second gap); no protected member counts for presence,
  holds a corner, or is named on a holder line; the pimp's `fit` (U6's) and the pitch opener's `free_adult` stand;
  `enlist` refuses a minor, so a turf recruit is an adult; tribute is paid by adults only; a family move is the
  children's protection.
- **For V5** (the war): `turf::set_status(world, cell, old, new, tell, now)` is the flip (event, `LostTurf`); the
  provocation hook is `turf::hourly`'s loop (a member on a cell's corner whose holder is another gang: today credited
  as presence only), `gang::claim` / `extort` on a door of another's held cell under Contest, and `turf::State`
  (`cells[k].status`, `.corner`, `.days` per gang, `held`, `rival`); `Shock::LostTurf { by }` is the grudge for
  Retaliate; `turf::neighbours`, `cell_dist`, `centre`, `cell_of` the geometry. The Statistical mirror is
  `daily`'s `stat_street_hours` credit.
- **Shadow** (20k `isle_m_k2`, seed 42, d22-27, `target/v2_shadow/`; picks `gang_member_6371`, `_10106`,
  `gang_leader_15212`, `dealer_7910`, `pimp_21916`): a member on his corner: `gang_member_6371 d22 19:00` above, and
  `gang_member_10106 d27 23:00` "Tank Kaneda holds The Unplugged 2's corner by Block#713 (9 more of theirs on it)";
  a holder change told: `gang_member_6371 d27 00:05` "The Unplugged 2 hold the blocks around Block#713 now, taken
  from The Drowned Choir 10: 2 of their people on the corner most nights"; the pitch in the turf with its pimp:
  `pimp_21916` (Piers Underhill, Drowned Choir 9, Home Block#631 in its held cell) works his round from d22 00:16, 12
  collections in d22-23 ("sent Hale Mbeki (63 y) to the pitch of Tristan Sato (21 y) (5c) and took the cut"), holds
  the corner on his own blocks `d22 19:00` and, his cell contested at `d24 00:05`, the next held one `d24 21:00`
  (Block#652). A resident avoiding a contested cell: no non-member was picked; the 20k run's line is above.
- **Reads wrong** (the next pass): the pimp lost the street at `d24 00:07` when his own cell turned contested (to a
  member on a still-held cell: a contest of a week can strip a street overnight; the reason now says so); gangs are
  big and sprawl (60 members in The Drowned Choir 9; "43 of theirs about the blocks" in one contested cell), so
  turf reads more as where members live than as a block held at a corner; `dealer_7910`'s Purist gang lay low all
  week (no corner, no deal); the leader `gang_leader_15212` (18 y, The Drowned Choir 9's) has no turf line of his
  own; a cell with no building is named by its coordinates ("the blocks at (588, 732)"). The corner deals
  multiplied (9 -> 2,835 in 30 days) and the stims trade with them (21.7 -> 606 coins a day): the corner made the
  dealer reach his buyers, a number moved for a known reason (`FINDING`, not tuned). Turf takes about four days to
  seed and gangs recruit through the first week: 0 cells held to d3, 30 by d5, 152 by d22, 221 by d29.
- Numbers (prints; 20k seed 42 x 30, `--scaled --ambient-flats 50`; before main `794bc6d` `target/v2_base/`, after
  `target/v2_after/`): cells held (last day) 221 by 36 gangs, 6.1 a gang, 25 contested (mean 93 held, 6.7 contested);
  holder changes 320, cells taken 60, contests 72; corner holds 7,038, corner body-hours 23,260; pimps a day 0.10 ->
  0.97 (about 2 from d10), pimp collections 12 -> 115, street visits 84 -> 264, street take 420 -> 1,320c, pimps' cut
  14 -> 150c; turf tribute 15,900c in 30 days (coerced flow 341 -> 727 a day), gang treasury median 116 -> 156, max
  1,238 -> 2,404; protection leans 45 -> 42; recruits from the turf 122; families moved 82; contested cells passed
  over (told) 90; Street pitches in the turf 16; Murders 50 -> 57, Assaults 1,145 -> 1,221, Extortion 812 -> 588,
  DistrictControl changes 10 -> 4; ticks/s 164 -> 122 (the 100k run on the box at once; a print). 100k x 30
  (`target/v2_100k/`): pimps a day 0.2 (round 5, `target/r5_100k/`) -> 1.17, pimp collections 3 -> 104, street visits 150 -> 848, street take 750 -> 4,240c; cells held (last day) 342 by 105 gangs, 3.3 a gang, 48 contested; holder changes 515, taken 140; corner deals 6,031; turf tribute 27,300c; families moved 572; turf recruits 600; Street pitches in the turf 95; Murders 236 -> 237, Extortion 2,516 -> 1,337; ticks/s 17 (the change tier on the box at once; a print).
- No test (doctrine 2). `SAVE_VERSION` 98 (97 on the branch, renumbered at the merge after V3): `World.turf`, `Shock::LostTurf`,
  `EventKind::Turf`, `[turf]`, the `Jobs2Cols` `turf_*` columns (printed on `analyze_run.py`'s turf lines). Probes
  re-pinned with "Round 6 V2" in place: `districts::test_gang_holding_most_homes_controls`,
  `test_control_first_computation_is_silent`, `test_gang_owned_home_weighs_as_a_home` (the claim rule, `[turf]` off),
  `leverage::test_protection_never_on_own_front` (the staged racket holds no cell, `[turf]` off),
  `person::probe_pimp_purse_grows_on_a_pitch_night` (the hand-named pimp, `[turf]` off),
  `factions::test_contest_keeps_working_a_home_after_the_first_blow` (the next blow is the next day's work).
### V4, the night out and the trip (`OPEN-239`, `OPEN-240`; `leisure::steps_for`, `service::refused`, `careers::retire`, `streetwork` the fare, `fixes::committed`, `[life] commute_long_ticks`, `[jobs2] cab_wage_share`, `cab_flush_coins`)

**The door (OPEN-239).** The bouncer's check runs at the start of a venue step inside the venue (`exec::start_step`:
Enjoy, EatOut, Gamble), and a Den's Unwind plan was `GoTo > Drink > Gamble`: the plan's own Drink made its patron
"drunk" (`misery::influence`, a drink within `drink_hours`) and its Gamble was refused, nightly. At a door a bouncer
keeps (`customers::kept_kind`) the Drink now comes after the main act (`GoTo > Gamble > Drink`); an unkept Den is
as before. A drunk who arrives drunk from elsewhere is still turned away, and the pick passes that door over for
`[exec] refused_ticks` (another venue). The bar was honoured all along (`vice::barred` is `day < until`: "barred till
d26" meant d24-d25, and the d26 return was legal); the line now names the last day barred ("barred through d25").

**The move-in (OPEN-240).** `night_shift_27693`'s "moved in with family" is `careers::retire`: a retiree going to a
grown child's Home takes his co-resident spouse along (`ages::move_in_with`, which U2 made bring the children) with
no distance check. The retirement to kin now passes over a child's Home that would put a working spouse's commute
(`life::commute_cost`) past `[life] commute_long_ticks` (180, 3 h) and longer than hers now; a sweeper's beat or no
post reads no commute. The retiree then retires on savings or keeps his post (work till the body quits). The first cap
tried, `commute_move_ticks` (80), passed over a third of the city's retirements (most commutes here run past 80).

**The fare (OPEN-240).** The rider pays the cab, not the Garage's owner: `pay_ride` takes the driver (the body who
drives him, else the Garage's Statistical driver of record: on shift, fewest fares, lowest id), who keeps his cut
(a body: `[verbs] cab_fare_cut`, his pay; an off-screen driver is on the mirror's wage and keeps none) and hands the
house the rest (`Flow::CabTake`, driver -> owner, untaxed since the fare was taxed at him, counted as the owner's
revenue; it replaces U7's `FareCut`, which ran the cut owner -> driver). A body ride is paid at the pick-up, both at
the door (paid at the drop-off, the motion system had already put the rider inside: the settle read apart). A Garage
with no off-screen driver leaves the fare to its owner (the line names the corp). The rider's money line names the
driver's post ("CabFare -10 to Raven Dunmore#20147 (Cab Driver)"). The commute's cab (the roll, the ride home, the
El-missed ride) also reads the wage: a fare past `[jobs2] cab_wage_share` (0.5) of the day's pay walks or rides the
El unless the rider holds `cab_flush_coins` (80): `fabber_84414` paid 7c a leg on a 7c wage.

**The wrong district (OPEN-240).** Two causes. (1) `streetwork::carry` moved a body ride's rider with his driver on
every driver step, whatever the driver's plan: a driver pre-empted mid-fare took his rider wherever he went
(`exec_73953 d24 01:16`: a paid cab to a D4 kitchen, 167 min later on a street in D5). A driver whose plan leaves
the Work goal with the rider aboard now hands him to the off-screen cab from where he stands (`release`;
`fares_dropped`, 3 in 30 days); a rider still waiting at his door waits for a driver finishing another plan. (2) A paid cab was cut by the rider's own next think (`exec_16679 d26 17:02` in the first after-shadow: an
87-min ride cut 3 min short, Shop 0.07 -> Unwind 0.81). `Jobs2State.riding` holds a cab under way (its landing tick,
`Tick::MAX` while a body carries him, aboard) and `fixes::committed` holds it against any goal but an emergency;
the hold reads the cab's own leg (the same landing tick), so an entry a plan reset left behind holds nothing.

**Numbers** (20k `isle_m_k2`, seed 42 x 30, `--scaled --ambient-flats 50`, one run each; before is main `794bc6d`
plus the counters commit, identical in every main column to a run of `794bc6d` itself; `target/v4_before`,
`target/v4_after/run5.csv`): door refusals 1,058 -> 122, **of the drunk 892 -> 25**; door brawls (`Brawl`) 312 -> 86;
**rider fares to a driver 0 -> 1,402, to an owner 4,040 -> 583**; cab trips 4,452 -> 2,378 (the wage share) and cab
hails 428 -> 490; **workers past a 3-h commute** at the midnight read d15 589 -> 497, d30 435 -> 400 (mean 1,213 ->
1,130); retirements 908 -> 623 (`kin_spouse_far`: 1,283 nights a retirement to kin was passed over) and elders taken
in 2,229 -> 1,839. FINDING, not explained within the cap: the after-runs grow their gangs sooner (members d30 934 ->
1,003, and 1,138 in the after-run before the last two narrowings; Extortion 812 -> 999, Assault 1,145 -> 1,382,
GangJoin 1,122 -> 1,213), with employment within 0.5 % and ~150 more elders at work; the gangs' founding tips on day
7 in one run and not the other, and the spread between the two after-runs reads as divergence, not a mechanism.

**Shadow** (20k, seed 42, days 21-27, `office_worker:6,night_shift:2,fabber:3,exec:2,cabbie:2`; `target/v4_shadow`,
before `target/v4_base/shadow` with the brief's picks). The night out gets in: `office_worker_18570 d24 21:55`
"PLAN Unwind on Gambling Den#4757 [GoTo(Seller) > Gamble > Drink]", `22:50` Gamble, `23:21` Drink, back on d27
(06:54 Gamble, 5c to The Neon Wolves); no `PLAN FAILED ... Unwind ... PreconditionLost` and no door line in 15
diaries. The fare to the cab: `exec_16592 d23 17:58` "-10c CabFare to Raven Dunmore#20147 (Cab Driver)", "rode with
Raven Dunmore (as a fare)", `19:06` "now at Fixer#4805": landed where paid; `d27 09:02` "hailed a cab (flush) ...
to Fixer#4805", "-> Fixer#4805 ... 60 min, 127 tiles (done)". The driver's side (first after-shadow):
`cabbie_21507 d21 21:18` "CabFare +6 from Gwendolyn Osgood#14015, CabTake -3 to corp Zetatech 6", the shift line
"drove 1 fare ... 3c his cut". No CabFare `SETTLED APART` in the final shadow (the base had 2). The move-in has no
case at 20k in these picks; it is read from the counter (`kin_spouse_far`) and the code.

**Reads wrong (next pass).** An off-screen ride from a Garage whose drivers are all bodies still pays the owner
(583 of 1,985; "CabFare -22 to corp Zetatech 7"); the fix is a ride that does not happen (no driver, no cab) or the
nearest Garage with an off-screen driver. The cabbie still stands by most of a shift (the commuter cab halved).
`office_worker_18570 d27 07:25` "ACTION Drink @ street ... (ended)": the diary prints where an interrupted step's
body stood when it ended (it left the Den at 07:30). Under-18: nothing here picks a victim or a fight; a minor at a
kept door is turned away "under age" as T4 built, unchanged.

**Deviations.** No new test. Two `tests/street.rs` probes re-pinned on their premise (Round 6 V4): the tier-blind
roll's `pay_ride` takes the driver (`None`), and the body ride is paid at the pick-up, to the driver (`cab_paid_driver`),
the owner taking the house's share (it asserted payment on arrival, to the owner); and
`person.rs`'s `probe_pimp_purse_grows_on_a_pitch_night` reads the meeting as it happens (the pimp's regulars keep 8
faces and his busier gang day on the V4 city pushed the worker off by the window's end). New config: `[life]
commute_long_ticks`, `[jobs2] cab_wage_share`, `cab_flush_coins` (serde defaults). New `Jobs2Cols`: `door_drunk`,
`cab_paid_driver`, `cab_paid_other`, `commute_long`, `kin_spouse_far`, `fares_dropped`; `analyze_run.py` prints
them in "the street". `SAVE_VERSION` 99 (97 on the branch, renumbered at the merge after V3 and V2; the keys, the columns, `Jobs2State.riding`).
### V1, the ladder at 100k and children out of care (`OPEN-231`, `OPEN-242`; `systems/shelf.rs`, `camp.rs`, `demography::children`, `routine::run_level`, `lod::stat_grocery`, `[camp] warn_stale_days`)

**The cause: the household's pantry read.** Not the tier: the ladder fires at 100k (r5 100k x 30: 3,378 children
warned, 540 taken on a second warning). A Home's pantry is one `stock_food`, and a 100k Block holds up to ten
families: the children's meal, the grocery runs (`household_need`: a jobless parent's `children_food_need(home)`, a
worker's shift meals x residents, both the Block's) and the warning read the Block. A broke family's run was never
made while the neighbours' groceries stood on the shelf, its children ate them, and the second unfed spell never
came: of the children warned at a door of ten or more children 0.5 % were taken later, against 27.6 % at a door of
one or two (r5 100k events); when the Block did run dry every family was warned in one line (`owner_broke_6426
d21`, eighteen children). 20k doors hold one to five children (the same read, smaller).

**Built:**
- **The household's shelf** (`shelf.rs`). The households under one roof are the kin living there (union-find over
  Spouse, Parent and Family edges between residents, at midnight before the children's meal: `Household.kin`, the
  lowest id). `Household.pantry_stake` is the units on the shelf that are his: `StoreFood` (Full and
  `lod::stat_store_food`) adds to it, his children's meal takes off it, an adult's pantry meal (`EatAtHome`,
  `PackMeal`, the off-screen meal) comes off, in order, his household's units above its children's food, the
  common units, the neighbours' spare above theirs, his own children's food last; at midnight the stakes are
  scaled down to the stock (spoilage, a theft, a raid); a move leaves the stake behind (`World::set_home`). Units
  nobody holds (the seeded stock, a stake left behind) are common. In a shared Home (two households or more, a
  stake on the shelf) a household's claim is its stake and the common units; a Home of one household, or none
  staked, keeps the common shelf as before. Each household has a Home pantry's `stock_cap` of room (`shelf::room`).
- **What reads the claim:** the children's meal (a household whose claim is short of its children's meal goes
  unfed that night, whatever the neighbours hold), the Full and Statistical grocery runs (`shelf::held_by`,
  `members`, `kids_need`; the pantry alone on its own household's warning, `camp::household_on_warning`), the
  adults' share (`camp::adults_share(world, id, home)`: his household's claim and the neighbours' spare; a
  neighbour's crust is eaten, his child's meal is not), the let-home (`home_holds`) and the warning's food money.
- **The parent's pocket** (`shelf::pocket_to_shelf`, before the children's meal): a free parent living there puts
  the food he carries (not stolen) on the shelf, up to his children's meal tonight (`camp_parent_11848 d24`, first
  cut: a worker on the warning bought three meals, ate one, carried two, and his infant went unfed beside them);
  740 units in 20k x 30.
- **A warning per household** (`take_daily` groups by `(home, shelf::key)`; the food money is that household's
  children's).
- **Children out of care** (`OPEN-242`). A child who slipped out (`escape` sets `Child.slipped`) is fetched back the
  next midnight (`fetch_daily`: into the same Camp if it stands open, else the take's placement; a warden named;
  the headcount's "fetched back"). A street child (no Home, not in a Camp) at midnight (`street_daily`) goes to a
  free parent with a standing Home ("off the street"), else is taken with no free parent ("on the street and no
  free parent (...)") or on an open warning (the street is no door to come back to); a homeless family with a free
  parent and no warning keeps `[ages] street_take_days`. A child moved to a parent's door (`follow_parents`, off
  the street) starts its unfed count again (`camp_jobless_25426 d27 00:10`: the father was warned the night his son
  came, over days unfed at the mother's; landed after the numbers run).
- **The `[family]` line** reads the household's share: "the pantry holds 19, 7 of it ours (18 households under the
  roof)". `ch_warned_old` (daily: children out of a Camp on a warning older than `[camp] warn_stale_days` 14; a
  report only) and the `camp_fetched`, `camp_off_street`, `camp_take_street`, `camp_pocket` columns, printed by
  `analyze_run.py`.

**Deviations.**
- Fixing it at its cause is a model change, not a ladder change: the Block's pantry is shared out by household (no
  per-flat `Building`; the ledger rides on `Household`). The first cut (claims a share of the stock, adults eating
  any household's units first from their own) took 470, then 583 children at 20k x 30, half from households whose
  purse covered the food: the Block's non-shoppers, and the parent himself, ate the food bought for the children.
  The pocket rule and the order of the adult's meal are what made it read right. Cutting adults off the
  neighbours' shelf entirely (tried) cost evictions 1,126 -> 1,480 and thefts 1,161 -> 1,558 and was dropped.
- A broken-out child (a parent's crew) is not fetched: the brief named the slip, and a break-out is the parent's deed.
- The pocket needs no walk home: a free parent living there is enough (the midnight meal is the family's).
- School meals (`[policy]`, off) do not reach a household unfed on its claim (the Reserve pays the Home's debt).
- No 100k run (the cost rule): the 20k shadow shows the shared Block (`camp_parent_11226`, eighteen households
  under one roof) and the per-household warning; the close's 100k counters read it at size.

**Numbers** (20k island seed 42 x 30, `--scaled --ambient-flats 50`, before = main `794bc6d` plus the
`ch_warned_old` column, one run each): ChildTaken 69 -> 100 (on a second warning 28 -> 63, no free parent 2 -> 2,
the street on the new rule 23, `street_take_days` 39 -> 12); daily ch_camp mean 16.7 -> 22.5 (d29 68 -> 96);
ch_street mean 59.0 -> 37.4 (d29 100 -> 63); warnings standing past 14 days mean 62.6 -> 31.4 (d29 292 -> 140);
children warned 540 -> 372; fetched back 4 (escapes 1 -> 4); off the street 43; followed a parent 10 -> 26; let
home 0 -> 0; buy-outs 0 -> 1; starvation deaths 0 -> 0; evictions 1,126 -> 1,154; thefts 1,161 -> 1,031; violence
deaths 86 -> 79; food bought 1.22M -> 1.19M coins.

**Shadow** (20k seed 42 days 21-27, `target/v1_shadow`; the first cut's in the notes above): the slip and the
fetch (`camp_warden_14571 d22 09:00` "1 slipped out (Rex Reyes)", `d23 09:00` "1 fetched back (Rex Reyes)"; the
run's line "Razor Mercer of Work Camp#4835 and the service fetched Rex Reyes (11 y) back to Work Camp#4835 from
Block#3423: slipped out on d22"); the warning per household in a shared Block (`neglect_parent_13080 d21 00:10`,
first cut: "Nova Achebe (11 y) went 2 days unfed; Kenji Achebe heard it", then "the pantry holds 3, 0 of it ours (3
households under the roof)" and 1-3 of his own the rest of the week); the second spell seen (`camp_parent_11848
d25`, first cut: "the pantry holds 8, 0 of it ours", d26 00:10 "took Kasimir Adeyemi ... unfed 2 days again, inside
the warning of d13"; the run: "took Maren Novak from Block#738 ...: unfed 3 days again, inside the warning of
d17"); a street family on a warning taken ("took Delia Adeyemi from the street ...: on the service's warning of
d12 and on the street (Smoke Adeyemi, Vivienne Ambrose with no Home)", `miserable_9202`'s three) and children off
the street to a parent ("Gideon Kaneda (11 y) went to live with Casimir Novak at Block#255: off the street").

**Reads wrong / open** (the next pass): 37 of 63 second-warning takes are households whose purse covered the food
at the warning (13 of 28 before): a hungry worker's Eat errand buys for himself (`grocery_due` is a fed worker's),
the pocket catches some of it. A warned household that feeds its children stands on the warning the full 30 days
(`camp_parent_11226`, since d2, 3-7 units of its own all week): Dylan's 30 days, not a hang, but `ch_warned_old`
counts it. A broken-out 5-year-old sat at his mother's door while she was in the cells (`camp_jobless_25426 d26`)
until the midnight move. Let home still 0; buy-outs 1.

Under-18: nothing here touches a victim pick, a fight or a punishment, and nothing calls `law`. An unfed day counts
toward the service's take and never harms a child (`health::child_day` floors below onset, unchanged); the fetch is
a walk back in; the street take and the moves are the service's protection; the Camp feeds every child. No test
(doctrine 2). `SAVE_VERSION` 100 (97 on the branch, renumbered at the merge after V3, V2, V4).

### V5, the turf war (`docs/TURF.md` § 3; `OPEN-241`; `systems/turf.rs` § the war, `[turf.war]`)

- **Provocation** (`turf::provoke_at`, `war_hourly`, `rolls`): another gang's act on a held cell heats it for the
  holder (`Cell.heat`, by gang), seen by a holder's adult body in the cell (a Statistical act is seen off screen):
  a body an hour on its corner in the corner hours (`heat_corner_hour`; by day a walk past the venue door is no
  trespass), his mark sprayed at the corner once a night (`heat_tag`, the "tag": told first), a deal in the cell
  (`stims`, `heat_deal`), a shakedown of a door there (`gang::extort`, `stat_extort`, `heat_shakedown`), a holder's
  member beaten there by another gang's (`law::resolve_fight_mods` -> `turf::beaten`, `heat_beating`). Off screen a
  gang trespasses on a held cell it borders at `provoke_p_day` a day (x `provoke_order_mult` under Contest, Expand,
  Retaliate), a night's heat (`heat_stat`). Heat keeps `heat_keep_day` a day. Told once a cell a night when a Full
  body did it or saw it ("Agathe Osgood of The Unplugged 3 saw Alaric Voss of The Drowned Choir 9 spraying The
  Drowned Choir 9's mark on the wall at The Unplugged 3's corner by Bar#4432"). A gang lying low provokes nobody.
- **The answer** (`open_fight`, `resolve`): past `answer_heat` the holder's leader calls the members living in the
  cell and within `muster_cells` of it to the corner (his line; `Shock::Trespass { by }` to the gang), the fight
  `answer_window_hours` later (an off-screen provocation's at the evening: `corner_from` + the window). The muster:
  `turf::muster_corner`, read by `corner_hold` (any hour, no cap; not on shift, not lying low, never under 18),
  `gang::gang_work_target` (before the order's work) and the corner arm's `muster_flat`. At the fight each side is
  its adult bodies within `muster_reach_tiles` of the corner, then its Statistical members living about the cell
  (stood at the corner; promoted to Coarse only when the corner is on screen, `lod::on_screen`), sized by its
  presence about the cell (`side_max` x its share of the larger), paired by `raid::fight_out` (each blow an Assault
  or a Murder raised at the corner, witnessed and reported as any: "the law's attention if seen" is the existing
  law). **Deviation**: the spec's "pushes the trespassers off" is the holder as the aggressor; an answer nobody
  of the holder's comes to turns the cell contested; one nobody of the trespassers' is at is told as such.
- **Hold or flip** (`take` through `set_status`): the side left standing holds the cell, locked `hold_lock_days`
  (`Cell.won`, read by `daily`'s judge); a flip gives the loser `Cell.lost` for `retake_days`; held Homes' claims
  are not moved (**deviation**: the cell's holder already takes their tribute and door, V2's `tribute`/`free_door`;
  `Building.claim` stays the claimant's until a shakedown moves it). **A raid on a Hideout** won or sacking moves
  the loser's cells within `raid_cells` of its Hideout to the raider (`raid::settle` -> `turf::raid_moves`).
- **Counter-attack**: a cell's loser comes back for it at `retake_p_day` a day inside `retake_days` (`rolls`, a
  Retake fight at the evening, his leader's line "sent 32 of theirs about the blocks back to the corner by
  Block#756 tonight"). **Deviation**: not a Retaliate order on the cell; the faction brain's Retaliate stays the
  Hideout raid (its 30-day cooldown), fed by `LostTurf` and the answer's `Trespass` as grudges (Raids 55 -> 99).
  **The front**: a contested cell is fought over at `front_p_day` a day.
- **Peace** (`war_daily`): a war (`State.wars`, a pair of gangs) starts at its first corner fight (told, the
  leaders named), goes quiet after `war_quiet_days` without one (told). Its losses are its men's: each side's
  fallen in its corner fights (beaten or dead) and its fighters' later deaths, jailings and leavings
  (`note_loss` from `gang::on_member_killed`, `on_member_arrested`, `leave`; **deviation**: "members dead, jailed
  or desisted in the war" read as the war's own men; counting every arrest of a 60-strong gang spent it in a day).
  Past `exhaust_share` of the side's fit adults at the start (at least `exhaust_min`) over `exhaust_days` the
  leader calls it off the corners: `lie_low` for `lielow_days`, pinned by `faction::rescore` (LieLow, an
  `OrderChanged` "spent in the turf war"); a lying-low holder does not answer. Two spent sides' leaders strike a
  **truce** (`truce_days`): the contested cells between them go to the side ahead (quietly), every cell on the line
  is locked to its holder, their fights are called off, no heat between them. **The law's crackdown**: each
  corner fight adds 1 to the cell's `law_heat` and each death `crackdown_death_w` (kept `crackdown_keep_day` a
  day); past `crackdown_heat` the cell is cracked down on `crackdown_days`: nobody holds its corner, no fight there,
  no heat, and the district's beat walks its Homes (`turf::crackdown_beat` in `law::district_route`;
  **deviation**: not `law::crackdown_turf`, which stays the Crackdown posture's gang route).
- **Off screen**: the same events from the daily `rolls` (the night's trespass, the front, the retake), resolved at
  the corner with the Statistical members placed there; no new tier. 723 of 856 gang members are bodies at 20k
  (`gang_bodies`), so the corner hour is most of it.
- **V2's reads-wrong** taken: the leader has the war's decisions (the answer, the retake, lying low, the truce,
  named at the war's start and on the crackdown); a cell with no building is named by its nearest ("the blocks
  near Block#1172"); a pimp keeps his street while his own blocks are contested by his gang (`vice::name_pimps`:
  `involves`, a cell lost still loses it).
- **The under-18 rule** (`ages::is_protected`): `provoke_at` returns for a protected actor and the hourly loop skips
  protected members (no provoker); `eyes_in` takes an adult seer; `corner_hold` returns for a protected member
  before the muster (never mustered); `can_fight` (living, free, not down, **not protected**) gates `fighters` and
  `muster_pool` (never a defender, an attacker or counted strength), and `raid::fight_out` and its crossfire gate
  again; `note_loss` counts no minor. `harm_blocked` 0 in both runs (no blow reached a minor).
- **Shadow** (20k `isle_m_k2`, seed 42; `target/v5_shadow3/` d14-20, `target/v5_shadow2/` and `target/v5_shadow/`
  d21-27; picks gang_leader x16, gang_member x8, raider x2). A provocation answered: `gang_member_6529 d14 20:00`
  "Agathe Osgood of The Unplugged 3 saw Alaric Voss of The Drowned Choir 9 spraying The Drowned Choir 9's mark on
  the wall at The Unplugged 3's corner by Bar#4432", `gang_leader_7508 d14 20:00` "Ulric Quarry, The Unplugged 3's
  leader, called 42 of theirs about the blocks to the corner by Bar#4432: The Drowned Choir 9 were on their
  turf". Flipped: `gang_member_6529 d14 23:00` "The Drowned Choir 9 beat The Unplugged 3 back on the corner by
  Bar#4432, 6 against 6, and hold it now". Taken back: `gang_member_25702 d15 03:00` "The Unplugged 3 beat The
  Drowned Choir 9 back on the corner by Bar#4432, 2 against 6, and hold it now"; the counter-attack on the next
  cell: `gang_leader_7508 d15 00:05` "Ulric Quarry ... sent 32 of theirs about the blocks back to the corner by
  Block#756 tonight", `gang_leader_19742 d15 21:00` "The Drowned Choir 9 held the corner by Block#756 when The
  Unplugged 3 came back for it, 6 against 6". The crackdown: `gang_leader_19742 d15 03:00` "The law cracked down on
  the blocks around Bar#4432: The Drowned Choir 9 and The Unplugged 3 kept off the corner, the beat walks its
  Homes for 7 days". Spent and the truce: `gang_leader_19742 d16 00:05` "Fenn Novak called The Drowned Choir 9 off
  the corners to lie low: 13 of their 51 dead, jailed or gone since the war with The Unplugged 3 began on day 13"
  (now "beaten, dead, jailed or gone": the fallen in its fights count),
  `gang_leader_7508 d16 00:05` (21 of 56), then "Ulric Quarry of The Unplugged 3 and Fenn Novak of The Drowned Choir
  9 struck a truce: the line runs where it stands (10 blocks and 5, 11 on the border), held 21 days". The war's
  start: `v5_shadow2` "The Ash Wolves and The Iron Knives are at war over the blocks around Street Market#4415 (Hana
  Park leads 10 of The Ash Wolves; Ghost Blackwood leads 43 of The Iron Knives)".
- **Reads wrong** (the next pass): the muster rarely moves a Full member who is at work or on a contract
  (`gang_member_6337`, DC8, worked through Delia Brandt's call); the fights are mostly the corner's neighbours and
  the placed Statistical men; "6 against 1" pushes of a lone tagger are common; the big gangs (50-60 members of the
  Unplugged 3, the Drowned Choirs) fight three wars at once and their leaders roam and spray other gangs' corners
  (`Ilse Greaves`, Iron Knives' leader, on four gangs' corners in a week; Hana Park, a leader, tagging); "The Iron
  Knives had nobody about the blocks to send": cells held by hours where no member lives; a lying-low gang lies low
  twice for two wars (`gang_leader_7508 d16`, `d17`); fights at 03:00-13:00 come from beatings and shakedowns (the
  corner's trespass is evening only); the gang's line "came to the corner ... nobody of X's there" when the
  trespassers left. Pimp collections 93 -> 68 (gangs lying low, corners cracked down: a number moved for a known
  reason, a FINDING).
- Numbers (prints; 20k seed 42 x 30 `--scaled --ambient-flats 50`; before main `4d8ee83`, after
  `target/v5_after/`): wars started 0 -> 34, provocations 1,571, answers and fights scheduled 131 (unopposed 47),
  corner fights 61, cells flipped in the war 44 (13 by a raid), taken back 6, sides spent 18, truces 5,
  crackdowns 3, war deaths 6 (gang deaths 22, a new column); TerritoryFlipped (Homes) 23 -> 18, Raids 55 -> 99,
  Sacked 3 -> 5, Murders 65 -> 67, Assaults 1,224 -> 1,362, Extortion 557 -> 462, violent deaths 91 -> 101; cells
  held last day 210 -> 190 (contested 19 -> 25); ticks/s 161 -> 144 (shadows on the box at once; a print).
- No test (doctrine 2). `SAVE_VERSION` 101 (renumbered at the merge): `turf::State` `wars`, `truces`, `lie_low`,
  `open`, `tagged`, `trespass_told`; `turf::Cell` `heat`, `fight`, `won`, `lost`, `law_heat`, `crackdown_until`;
  `[turf.war]`; the `Jobs2Cols` `turf_provocations` ... `turf_crackdowns`, `gang_deaths` (printed on
  `analyze_run.py`'s "turf war" line). Probes re-pinned with "Round 6 V5" in place (`[turf.war]` off, the subject
  not the war): `leverage::test_protection_pays_weekly_and_stops_when_district_lost`, `person::widow_pick`
  (`probe_widow_commissions_a_hit_at_the_fixer`).
### V6, empty rooms beside full ones (`OPEN-217`, `OPEN-232`; `care::patch_clinic`, `care::open_for_patch`, `customers::on_call`, `customers::back_room`, `customers::cause`, `leisure::stat_visit`, `leisure::bouts`, `vice::last_bout`, `gray::strike_off`, `[care]`, `[customers]`, `[leisure]` V6 keys)

- **The causes** (20k `isle_m_k2` seed 42, main `4d8ee83`; a temporary probe on the door, the pits and the clinics,
  removed). (1) **The Clinic was dark at night**: `assets::seller_open` opens a Clinic only with its Ripperdoc at the
  counter, so at 02:00 every one of 29 Clinics was shut with its orderly on the split shift (21:00-06:00); the night's
  wounded waited for 09:00 and the orderly saw nobody (V14 `orderly_14910`; 20k: 9 Clinic events between 21:00 and
  09:00 in a week). (2) **The back room's doc on call was the first found** (`customers::doctor`'s `find`): the 20k
  Back Room#1971's one doc saw 469 of its 724 patients in 30 days, three colleagues 20-123 (V14's 574 and none is the
  same pick; the two V14 picks are colleagues of one Sump). (3) **The nearest took every patient whatever the queue**:
  `care_choice` and `back_room` named the nearest open, with no read of the doctor's hour (`[[serves]] per_hour`).
  (4) **The door could see nothing off screen**: of 9,264 off-screen arrivals at kept doors in 8 days none had a cause
  (no gang member is off screen, nobody off screen carried, and the off-screen memory keeps no `Socialised`, so a
  Bar's drink never made anyone drunk); a 100k door's arrivals are all off screen, so "nobody turned away" nightly.
  And the broke never reached the door: `stat_visit` let a short purse fail silently before it. (5) **The bout read
  its rotation from the `Bout` memory**, which an off-screen fighter's eight slots can drop, and a pit held one bout
  a night whatever its roster. 20k pits hold four fighters and rotate cleanly (each every other night), so the 100k
  cause is not shown at 20k; both readings are fixed. (6) **Struck off with no why**: the quit said "struck off".
- **Built.** A Clinic is open for a patch-up with any of its patient staff on duty (`[care] orderly_opens`,
  `care::open_for_patch`; a sale, an install and a treatment still wait for the Ripperdoc, `seller_open`). A wound
  walks to the nearest Clinic open for it whose doctor has room this hour, else the nearest open (`care::patch_clinic`,
  `[care] clinic_spill`); the back room the same, within `back_reach_ticks` (`customers::back_room`). The doc on call
  is the pair hash of the patient among the free docs with room (`customers::on_call`, `[care] on_call_spread`). The
  door: a purse short of the venue's price (coins in hand) is a cause, turned away every time, no scene and no grudge
  (`[customers] door_broke`), and the off-screen visitor meets a kept door before his purse (`leisure::stat_visit`); an
  off-screen drink leaves the mark the door reads (`Misery.drank_at`, `[customers] door_stat_drink`, read by
  `misery::influence` within `[misery] drink_hours`; a body's drink is still its memory, V4's ordering untouched). The
  pit keeps its own book of who fought when (`ViceState.bouts`, `vice::last_bout`, the memory only for a fighter not
  in the book) and its card is a bout for every `[leisure] bout_card_per` (4) Fighters on shift, at least one; the
  undercard prints "X beat Y at Z on the undercard", the bets ride on the main event. The strike-off quit says why
  ("struck off the register: the licence board found a lawless practice").
- **Deviations.** The door's list in the brief was the barred, the broke, the armed, the minor and the brawler; off
  screen only the broke and the drink are there to read (the armed and the colours are bodies: gang members are never
  off screen; a minor has no Brain and never reaches a door; the barred know and stay away, `door_seen`). The drink
  is V4's on a body; here only the off-screen mirror (V4's body refusals of the drunk are not moved: 23 -> 68 drunk
  lines a month are the off-screen tier's). The capacity spill rarely binds at 20k (no doctor's hour full); it is
  there for 100k's queue. Patients per clinic stay lopsided by catchment (the Sump's and the Mid-Levels' Clinics); no
  pick sends a patient past a nearer open one with room.
- **Under-18** (`ages::is_protected`): the off-screen patients are adults (`stat_care`'s `is_adult`), a child has no
  Brain to plan a Treat; the bout card filters `is_protected` explicitly (the Fighter's hire is 18 and up); at the door
  "under age" is read before "broke" and is no scene, the broke turned away is no scene and no grudge, the scene's own
  gate is unchanged. Nothing here picks a victim or a fight.
- **Shadow** (20k, seed 42, d21-27, picks ripperdoc:3, orderly:2, black_doc:2, bouncer:2, fighter:2; the after build's
  picks differ from the base's by the moved city). The night orderly (`orderly_9822`, Santiago Ito, Ripperdoc#4998,
  split shift; base orderlies: no line in the week): `d21 21:00` "Santiago Ito patched Orrin Yates up at
  Ripperdoc#4998 for 12c (3c of it to the doctor)", `d25 23:00`, `d26 21:00` the same at night. The back room
  (`black_doc_13078`, Bronwen Pembroke, the base's 469-patient doc): `d21 01:00` "patched Zelda Novak up in the back
  room of Back Room#1971 for 0c, no questions asked", `d21 07:00` "... for 6c (Fiora Fairweather put up 6c), to
  Eastwick Holdings's till"; in 30 days #1971's docs 258 / 188 / 220 (base 112 / 469 / 123). The door
  (`bouncer_20093`, Clemence Quarry, Gambling Den#4757): `d23 22:00` "turned Mirabel Bartlett away at the door of
  Gambling Den#4757 (carrying)", `d24 22:00` "turned Santiago Crane away ... (drunk)", `d24 00:00` "18 let in, 1 turned
  away (carrying 1)"; `bouncer_17697` (a1 build, Gambling Den#5054) `d23 20:45` "turned Mina Sallow away ... (broke)",
  `d24 00:00` "93 let in, 1 turned away (broke 1)". The pit (`fighter_11655`): `d21`, `d23`, `d25 23:00` Bouts as the
  base. The one 100k shadow (why: the door
  and the pit are 100k's, every arrival off screen and the rosters past four; `isle_m_k2_100k`, seed 42, d21-27,
  bouncer:2, fighter:2, black_doc:2, orderly:1; the build before the last fix): `bouncer_61555` (Verity Ito, Club#5702)
  `d21 21:30` "turned Mateo Eastwick away at the door of Club#5702 (drunk)", `d21 22:00` "turned Sabine Jarrow away
  ... (broke)", `d22 00:00` "8 let in, 8 turned away (broke 6, drunk 2)" (V14: "nobody turned away" 7 of 7 nights);
  the broke came back on the evening's queued visits and were barred for the return ("turned away 2 times today:
  broke x2; barred through d22"), so a face turned away at a kept door now drops its queued visits there
  (`stat_visit`'s `door_seen`), the final build. `fighter_64821` (Hesper Ashcombe, Fight Pit#1887): `d23 23:00` "Mags
  Wexford beat Hesper Ashcombe at Fight Pit#1887 (bets paid 0)", `d26 23:00` "Knox Duarte beat Hesper Ashcombe at
  Fight Pit#1887 on the undercard" (V14: no Bout for either fighter in a week; the undercard says the 100k roster is
  past four). The other fighter and that bouncer spent the week jailed. `black_doc_60434` (Tomas Nakamura) 9 patients,
  `black_doc_95016` 14, two rooms of the Sump, neither empty.
- **Reads wrong** (the next pass): a fighter clocks in at a pit shut seven days by a protection fire
  (`fighter_8049`, Fight Pit#5496 burned d16, d18, d20: no bout from d16, she works the dark pit all week); a 20k Club
  door lets in 1 a night (`bouncer_17841`, Club#4621, the Spire): the venue's traffic, not the door; the night
  orderly's chip installs still land at 00:07 (U6's midnight leftovers); the door's off-screen causes stay few (13 of
  1,095 a body door's week), the drink and the purse the only state the off-screen tier carries to it; the clinics'
  catchment (one Clinic 107 of 443 in a week).
- Numbers (prints; 20k seed 42 x 30, `--scaled --ambient-flats 50`, one run each; base main `4d8ee83`, after the
  final build):

  | | base | after |
  |---|---|---|
  | Clinics with a patient (30 d / d21-27) | 29 / 20 | 32 / 27 |
  | patients per Clinic, 30 d: median, max | 30, 398 | 30, 344 |
  | patients per Clinic, d21-27: median, max | 9.5, 137 | 6, 107 |
  | Clinic patients 21:00-09:00 (30 d / week) | 56 / 9 | 234 / 57 |
  | patched (all) | 1,556 | 1,683 |
  | back rooms, 30 d: median, max, min | 145, 724, 89 | 207, 693, 51 |
  | Back Room#1971 by doc (30 d) | 469 / 123 / 112 / 20 | 258 / 220 / 188 / 12 (a1 build) |
  | door refusals (`door_refusals`) | 191 | 208 |
  | refusal lines by cause | colours 39, drunk 23, brawler 9, high 6, dress 5, barred 3, carrying 2, no cause 58 | drunk 67, broke 46, colours 24, high 17, dress 4, brawler 4, carrying 2, no cause 5 |
  | a body bouncer's nights with a refusal | 34 of 571 | 37 of 573 |
  | bouts (30 d) | 511 | 496 (one pit burned shut three times) |
  | Assaults + Murders (a moved city, not claimed) | 1,289 | 1,101 |

  No test (doctrine 2). `SAVE_VERSION` 102 (101 on the branch, renumbered at the merge after V5):
  `ViceState.bouts`, `Misery.drank_at` and the V6 keys (serde defaults).

### W4, the right person in the role (`OPEN-245`; `careers::can_foreman`, `service::install_server`, `service::arrive_by`, `care::pay_install`, `chrome::installed_now`, `[[trades]] no_foreman`, `[care] install_rows`, `install_cut`)

- **The causes** (20k `isle_m_k2` seed 42, main `795943c`). (1) **The foreman was whoever had served longest**:
  `careers::foreman_where` (and `industry::foreman`, careers off) ranks every adult on the staff by tenure and skill,
  any role; U7 added only "on hand" (not sick, not held). A Fixer's office whose senior hand is its forger has the
  forger pick its Fixers; a Clinic whose Ripperdoc post is vacant has the orderly pick its Ripperdoc; a Club its
  dancer, a Market its night stocker. At 00:11 nobody is at a desk: the midnight hire is the post's paperwork, so
  "at work" stays U7's (not sick, not held) and the role is the gate. `forger_7536`'s office was also shut by the
  law all week (`routine::law_shut`: every day a "rest day"), which is why he "never works". (2) **The install's
  doctor was the patient queue's server**: `chrome::installed_now` named `service::server_for(Patient)`, and the
  orderly's `[[serves]]` row is the same Patient queue (the triage); U6's `clinic_body_first` hands a patient to a
  body first, so a Clinic with an off-screen Ripperdoc at the counter (the sale opens only with him,
  `assets::seller_open`) and an on-screen orderly gave the orderly every install (V15's 36). Nothing paid anyone for
  an install: the price went to the owner's till (`Flow::Asset`), and the encounter went to the queue's next server
  again (`service::arrive`).
- **Built.** A trade row can say `no_foreman = true` (`TradeCfg.no_foreman`): its holder is never the foreman who
  picks a hire, names a layoff or takes the midnight `Job.foreman` premium (`careers::can_foreman`, read in
  `foreman_where` and both `industry` fallbacks). Flagged: orderly, forger, organ broker, bouncer, dancer, night
  stocker, night porter, barkeep, brothel worker, companion, BD actor, cab driver, day labourer (the hands under a
  doctor, a fixer, a house, a yard). A building with no foreman-able hand on hand hires with no foreman named ("X
  hired as Ripperdoc"). An install is cut by a `[care] install_rows` server (`Ripperdoc`, `black_doc`): one on duty
  with room this hour first, by the pair's hash, else one on duty, else a cutting row's holder on a working shift by
  the clock (`service::install_server`), never the patient himself. He is named, takes the encounter
  (`service::arrive_by`, his hour's room) and `install_cut` (0.1) of the implant's value from the owner's till
  (`care::pay_install`, `Flow::Treatment`, the place's revenue debited, nothing to an owner-doctor): "... installed
  at Ripperdoc#5839 by Delia Zhou (6c to the doctor)".
- **Deviations.** The spec's "needs a ripperdoc at work" was already the sale's gate (`seller_open`); no new refusal
  was added. The 78 installs with nobody named are sales that end after the doctor left (a body's `BuyAsset` is
  checked at the walk's start and lands past the shift: `orderly_21272 d26 19:03`, her own chrome at her own Clinic).
  A gang's house still picks through the gang's man on the staff whatever his trade (round 4 T6): a forger "for The
  Pale Hounds" is the gang picking, not a foreman (2 such hires by no_foreman trades in 30 days after).
- **Under-18**: nothing here picks a victim, a fight or a fine; the hire's own `is_protected` backstop is untouched.
- **Shadow** (20k, seed 42, d20-26, picks orderly:3, forger:2, ripperdoc:3, foreman:3, the final build). The install
  done and paid by the doctor (`ripperdoc_16969`, Delia Zhou, Ripperdoc#5839): `d21 16:29` "Joss Quarry had Legs
  implant T1 installed at Ripperdoc#5839 by Delia Zhou (6c to the doctor)", her money line "+6c (Treatment) ...
  flows: Treatment +6 from corp Zetatech 5"; 7 installs in the week, `d22 14:47` a gang's arm "(gang: The Static
  Rats)". No orderly installs anything: `orderly_21272` (Slate Oakes) 30 Treated, all her own work, "patched X up at
  Ripperdoc#4936 for 12c (3c of it to the doctor)". Hires named by a real foreman: `foreman_13755` (Wire Greaves, a
  Builder at Construction Yard#5705) `d25 00:11` "Corwin Dunmore hired as Builder (picked by Wire Greaves, the
  foreman)"; `ripperdoc_11516` (Kasimir Ostrowski) `d20 00:11` "Omar Novak hired as Manager (picked by Kasimir
  Ostrowski, the foreman)". The forger's office: `forger_21249` `d25 00:11` "Undine Kaneda hired as Forger" (no
  forger picks; nobody named). An earlier build had a ripperdoc cut her own chrome ("Lavinia Northcott ... installed
  ... by Lavinia Northcott (6c to the doctor)"); the patient is now left out of the pick.
- **Reads wrong** (the next pass): a doctor's own chrome, or a sale that ends after the doctor's shift, is installed
  by nobody named (78 in 30 days); `ripperdoc_16969 d21 14:05` one install with no cut (the owner's till dry at the
  hour); the hire at Fixer#6013 names no foreman though its Fixer (Ivo Stroud) brokers there (not diagnosed: the
  careers age floor or the office's staff list); the forger's quit and search churn (`forger_11197`) is the hire's,
  not the picker's.
- Numbers (prints; 20k seed 42 x 30, `--scaled --ambient-flats 50`, one run each; before = this build with main's
  `assets/config.toml` through `CITYSIM_ASSETS` (empty `install_rows` and no `no_foreman` row is main's pick, line
  for line), after = the final build; the `#role` lines `--report --events` now prints: the doer's or the picker's
  role at the event):

  | | before | after |
  |---|---|---|
  | chrome installs by the doer's role: Ripperdoc / Orderly / none | 1,056 / 578 / 83 | 1,608 / 0 / 78 |
  | chrome installs (all) | 1,717 | 1,686 |
  | coins to the doctor (`care_doc_cut`: patch, treatment, install) | 2,508 | 11,400 |
  | hires named by a no_foreman trade | 409 (orderly 65, BD actor 82, dancer 68, night stocker 49, bouncer 48, brothel worker 40, cab driver 36, barkeep 15, forger 3, night porter 3) | 2 (brothel worker: the gang's man) |
  | hires named by a Ripperdoc / BD Editor | 94 / 73 | 136 / 141 |
  | hires with no foreman named | 1,716 | 1,823 |
  | hires (all) | 13,832 | 13,787 |

  No test (doctrine 2). `SAVE_VERSION` 103 (renumbered at the merge): the `[care] install_rows`, `install_cut` and
  `[[trades]] no_foreman` keys (serde defaults).
### W3, the guard makes the arrest (`OPEN-244`; `plan::arrest_chain`, `exec::actions` the cuffs, `law::serve_at_door`, `law::handed_to_other`, `life::arrive_at_suspect`, `goals` Arrest's chase in hand, `fares::caught`, `care::call_for_help`, `wounded::pick`, `[justice]`, `[fares]`, `[care]` W3 keys)

Each at its cause (20k island seed 42 x 30, `--scaled --ambient-flats 50`, before = main `795943c`, after = the
branch, one run each; counts from the event log, the new columns from the CSV):

1. **The arrest.** Three causes, none of them the suspect's place. (a) *The bind came too late:* the guard's think
   is scored ahead and his plan queued; by the plan the warrant was claimed by another guard or had gone cold, the
   bind found nobody, and the planner's search over every action failed (`Unreachable` 340, `ExpansionCap` 308: all
   648 were an unbound target) and cooled Arrest two hours. The plan is now the chain `[GoTo(SuspectTile)] -> Arrest
   -> Escort` built directly (`plan::arrest_chain`; a prisoner in hand binds first: `Escort` alone); no warrant at
   plan time rests the goal one think ("Arrest dropped: the warrant was taken by another guard or went cold"). (b)
   *The cuffs:* `law::arrest` ran at the end of the 10-minute step, so a suspect beside the guard at its start had
   walked off by its end (1,143 `failed at Arrest: PreconditionLost` against 1,121 arrests); the cuffs go on at the
   step's start (it starts only beside him), the minutes are the cuffing. (c) *A warrant the street never saw:* the
   chase starts only from a guard's sighting, so a suspect who went home stood wanted until the warrant expired.
   `law::serve_at_door` (hourly, `[justice] door_from_hour` 22 .. `door_to_hour` 6): a warrant unseen within
   `suspect_seen_ticks` whose suspect is inside his own Home goes to the nearest free city guard on shift within his
   pursuit radius (his collar by hand-on, the door the sighting; never a minor, never sleeping rough alone), and the
   arrest line says "at home". A hand-on is now that guard's (`handed_to_other` in `chaseable`). Also: a chase begun
   within `[justice] chase_overtime_hours` (2) is finished past the shift's end; a radioed sighting past the guard's
   pursuit radius drops the chase "out of reach" and hands it to a guard near it (a guard walked 259 tiles, 4 h 46,
   on a rest day), and no longer erases that fresh sighting.
2. **The fare.** A guard handles one dodger at a time (`[fares] handle_minutes` 6, `fares::State.busy`; a gate jumped
   meanwhile goes through). The fine already came first; the broke dodger `book_age` (60) or older, sick or wounded is
   *booked*, not beaten (the catch on his record toward the ban, turned off the platform). The minor's rung (turned
   off, untouched) is unchanged and first; `beat`'s `is_protected` backstop stays. Fined and booked lines now carry
   ages (`events::harm_line`).
3. **The rescue.** The callee of the last call who has not come is passed over (`Wounded.tried`, the line names him);
   with no tied body able, the call reaches the nearest who can make it in time: a city guard on shift or the doctor's
   own staff on duty within `[care] call_radius` (40), or an adult passer-by within `help_radius`; an off-screen callee
   gets a body (`lod::set_lod`) and keeps it while called (`care::bystander`); the callee's Rescue carries `call_flat`
   (0.3) and a stranger callee ties on `call_term` (0.8). A rescue in hand holds (`wounded::pick`: his own claim, the
   reach and the clock read when he took it up). Nobody able: "called for help: nobody in reach could get them to a
   doctor in the N min left", once a turn of the calls. A body on someone's back calls nobody.

**Numbers** (20k x 30, before -> after):

| | before | after |
|---|---|---|
| Arrests (events) / a guard a week (400 guards) | 1,121 / 0.65 | 1,325 / 0.77 |
| Reports filed / arrests per report | 1,579 / 0.71 | 1,651 / 0.80 |
| Arrest searches failed (`Unreachable` + `ExpansionCap`) | 340 + 308 | 0 (577 rests of one think) |
| `failed at Arrest: PreconditionLost` | 1,143 | 1,024 (now the suspect gone from the sighting) |
| warrants taken to the door / arrests at home | - | 297 / 463 |
| warrants open at day 30 (`law_open_warrants`; before: reports - arrests) | (458) | 270 |
| chases handed on / "no guard on shift near" / "the shift is over" | 33 / 142 / 73 | 79 / 127 / 34 |
| fare: caught / fined / beaten / booked | 1,632 / 707 / 873 / - | 1,620 / 705 / 637 / 244 |
| fare beatings of a 60+ | 240 | 0 |
| downed / stabilised / bled out | 43 / 28 / 7 | 48 / 36 / 3 |
| calls / to a stranger / after a no-show / nobody able | 51 / - / - / - | 57 / 15 / 17 / 11 |
| lifted / carried in time | 30 / 28 | 37 / 36 |

The arrest a guard a week at 20k is bounded by the reports (0.96 a guard a week after): the reach was the bug, the
rate is the city's crime. The after run predates one guard (a carried body calls nobody), which moves only the call
lines.

**Shadow** (20k seed 42 days 20-27; `target/w3/shadow1` .. `shadow3`, guard, camp_warden, cabbie, sick):
`guard_24230` "d22 22:00 Pursuit: Axel Voss took the warrant on Ottilie Ashcombe for Theft to the door of
Block#2679, 57 tiles off: home tonight, never seen on the street" -> "d22 23:47 Arrest: Axel Voss arrested Ottilie
Ashcombe for Theft at home in Block#2679"; `guard_9691` "d26 09:41 ACTION Arrest @ Block#3621, 11 min (done)" ->
"Arrest: Zelda Sato arrested Rin Ostrowski for Unlicensed Arms at home in Block#3621" -> Escort to the Precinct;
`guard_7839` "d25 16:14 Fare: Zara Voss (32 y) booked Percival Upton (67 y) at the El in D4 Mid-Levels for Fare
Evasion: no fare, no fine, too frail to beat; caught 2 of 3 toward a ban". The run's log: "Lux Mercer (34 y) fined
Emeric Jarrow (74 y) 5 coins at the El"; "Ignatius Carrow (70 y), bleeding, called for help: Kestrel Blackwood (36
y), a passer-by 1 tiles off, heard" -> "Kestrel Blackwood carried Ignatius Carrow to Ripperdoc#4824 in time"; "Vale
Marlow (73 y), bleeding, called for help (Ivo Stroud never came): Casimir Kettering (53 y), a guard on the beat 5
tiles off, heard"; "Liesl Wexford gave up the chase of Ghost Blackwood (out of reach): handed to Katrin Okafor on
shift, 45 tiles off". No camp_warden, cabbie or sick pick met a wound, a fare or an arrest in the week.

**Under 18.** The door pass skips a protected suspect; the fare's minor rung is first and unchanged; the call's
stranger is an adult (a callee carries, no harm); no fight or punishment path widened.

**Reads wrong / open** (the next pass's input): guards idle 5-11 h on some shift days once their patrol legs are
walked (`guard_9691` work 1.8 h, idle 10-14 h d20-22), and a patrol leg can be 222-675 tiles (4-5 h) from where an
escort or a chase left him (`law::district_route` is built per shift, not from where he stands); a door taken by one
guard is sometimes served by a nearer one who planned first; 1,024 chases still end at an empty sighting (the suspect
walked on: the night's door picks them up); 11 calls found nobody able (a sparse street at 20k: no guard on shift
within 40 tiles, nobody within 6). The 100k spree (V15's 18 in 11 minutes) was not re-run: one dodger per 6 minutes
a guard caps it at 10 an hour.

No test (doctrine 2); `probe_widow_commissions_a_hit_at_the_fixer` re-pinned on its premise (the killer the law has not got: the body counts as found, so no case opens; the guards now arrested him 54 min after the find). `SAVE_VERSION` 104 (W4 took 103; renumbered at the merge): `fares::State.busy`,
`Wounded.tried`, the W3 keys and the `law_open_warrants`, `law_door_taken`, `law_door_arrests` columns (serde
defaults).
### W2, the war and the case in the diaries (`OPEN-241`, `OPEN-238`; `raid::wait_for_crew`, `raid::muster_point`, `contracts::mission_muster`, `crew::march_released`, `turf::hearers`, `turf::muster_wait`, `turf::waits_for_called`, `law::tells_the_law`, `law::keeps_from_law`, `consequence::woke`, `demography::found`, `[crew] march_wait_ticks`, `[crime.case] fear_courage_max`)

20k island seed 42 x 30 (`--scaled --ambient-flats 50`), one run each: before = main `795943c` with this phase's
counters only (the run matched the plain main run to the digit), after = the branch. The new `Jobs2Cols` counters
print on `analyze_run.py`'s "the war and the case in the diaries" line.

1. **The muster walks off.** Two causes. (a) *The column split at the door*: the first marcher at the door held the
   Brawl only until three stood there, then went in. The rest of the column, a Coarse body's timed walk or a Full
   body's steps behind, found it over and was released mid-walk with no line. A diagnosis run found the missing
   marchers at the brawl 30-100 tiles out on `GoTo(RaidTarget)`, 31 of 38 of them Coarse. Now the first at the door also
   holds for any marcher on `GoTo(RaidTarget)` due there within `[crew] march_wait_ticks` (60). The arrival is a timed
   walk's `arrive_tick` or a Full body's tiles at the walking pace, and the hold still ends in the march window's last
   hour, as before. (b) *The march across the island* (`cultist_62792 d21`, 100k: 293 tiles to the Hideout, then 220
   toward a Precinct, asleep at energy 0.03 when the raid was over): with no held Home within `muster_near_tiles` of
   the target, the muster was the Hideout, however far. Now it is the held Home nearer the target than the Hideout
   (`raid::muster_point`). A contract mission mustered at the Fixer's office wherever it stood (`gang_member_11411 d25`,
   20k: muster in D3, then 343 tiles toward the Sump for six hours, "the crew never reached the door"). Now
   `contracts::mission_muster` takes the first of its places within `muster_near_tiles` of the door, else the nearest
   of them. A marcher still on the way when the raid is over says so (`crew::march_released`: "X was still on the way
   when G went in at P (...), and turned back" / "... when G's raid was off, and turned back"). One already in the
   brawl from within reach of the door is not counted as missed.
   **The corner muster** (`turf::muster_wait`, `muster_holds`, `waits_for_called`): a called member on his corner
   waits there until the fight. The HangOut lasts until it, and the empty-spot leave and every goal but danger or
   starving give way. A fight that is due waits up to `march_wait_ticks` for the mover's called men on their way. The
   diagnosis says the corner was no walk-off: of the called bodies missing at the fight, about a third were asleep
   (fights at 01:00-05:00 from beatings), a third were on shift and a tenth were still walking. Not changed: a call
   does not wake a sleeper.
2. **A member hears his gang's war** (`turf::hearers`, `war_line`): every war line (the call, the war's start, the
   fight held or flipped, nobody came, lying low, the truce, the crackdown, the war gone quiet) now carries the gang's
   free adults who live in the cell or within `muster_cells` of it. Lying low, the truce and the quiet carry all its
   adults. The shadow's contact list skips a Turf line's actors (the men who hear a line are not people the pick met).
3. **The victim's side.** (a) *The sleeper*: a shakedown's payer who is asleep is no longer the Intimidate's target
   (V15 rolled "Intimidate ... SUCCESS" on a sleeping man) and learns nothing at the minute. On waking
   (`consequence::woke`, every `find_every_ticks`) he finds the purse short: a WasRobbed, the soured edge and a victim's
   first-hand sighting of each extorter. His line says he means to tell the law (an on-duty guard in earshot takes it
   at once, else the hourly tell or ReportCrime) or that he tells nobody, with his reason (`law::keeps_from_law`: his
   gang, the suspect's gang on his block, the sale, his own warrant, not one for the law). (b) *Every payer*: the
   shakedown's parties held a sighting with no object, so the tell (`o == Some(w)`) never read a robbed payer. A
   party's sighting now names himself (victim-like). (c) *Fear* (`[crime.case] fear_courage_max` 0.5, inside
   `tells_the_law`, so it reaches the hue, the tell and the new paths): a victim or witness under that courage does not
   name a man whose gang holds the block he lives on. (d) *The witness who finds the body* (`demography::found`): if
   he holds a first-hand Murder sighting of the killer, he names him ("the law opened a case and has a name: X saw it
   done", a Murder report with him as the witness), or the case line says he kept quiet and why. (e) The summary's
   "tribute received" line prints only when it is not 0c.

**Numbers** (20k x 30, before -> after): raid marchers at the door when it went in 172 of 235 (73 %) -> 204 of 238
(86 %); marchers released on the way 88 -> 90 (both runs still counted a marcher in the brawl from within reach;
the committed count no longer does); raid results 15 -> 18, called off at the door 6 -> 10, fizzled 3 -> 1. Corner
fights: called bodies at the corner 391 of 955 (41 %) -> 359 of 875 (41 %), unmoved for the reasons above. Gang members
(not leaders) on a war line 536 -> 4,202. Sleepers robbed asleep 339 -> 331. Found it on waking 0 -> 285: 128 mean to
tell; 157 tell nobody (their gang 73, not one for the law 29, afraid of the holders 25, wanted 24, the sale 6). A
robbed sleeper's word reaching the law within `tell_days` 7 -> 18. Shakedown tells on the street 0 -> 36; Shakedown
reports 87 -> 100. Found bodies whose finder saw the Murder 6 -> 1 (he kept quiet, with the reason printed). Moved
for known reasons, not claimed: provocations 1,547 -> 1,238, corner fights 64 -> 67, flips 34 -> 40.

**Shadow** (20k, seed 42, d20-27, `target/w2_after/shadow/`, gang_member x8, gang_leader x4, cultist x3, courier x3,
worker x3). A member called while asleep, then on the corner for the fight: `gang_member_10582 d25 00:00` "Roxy
Kowalczyk, The Unplugged 2's leader, called 19 of theirs about the blocks to the corner by Block#335", `03:12` GOAL
GangWork, GoTo(Spot) 20 tiles, `03:20` "HANGOUT ... 7 other(s) there", `04:00` "The Unplugged 2 pushed The Drowned
Choir 8 off the corner by Block#335, 6 against 2". Members' war lines: `gang_member_19576` and `_20109 d22 00:05` "Wire
Novak called The Ash Rats off the corners to lie low: 6 of their 21 beaten, dead, jailed or gone since the war with The
Static Rats began on day 21"; `cultist_9090 d22 00:05` the truce, "Matthias Jarrow of The Drowned Choir 9 and Echo
Wexford of The Drowned Choir 10 struck a truce ..."; `gang_member_14191 d20 23:00` "The Unplugged came to the corner by
Club#4865 (...): nobody of The Neon Wolves' there". The sleeper: `cultist_10709 d22 01:20` "Zoe Chrome woke at
Block#199 to find 4c gone from the purse: Vivienne Haas shook the block down in the night; tells nobody: she runs with
The Drowned Choir 6, and the law is not their way". From the after run's log (no pick met these): "Seraphine Wexford
woke at Block#714 to find 5c gone from the purse: Dmitri Garland shook the block down in the night; she means to tell
the law" (d7 16:30), "Seraphine Wexford told Ulysses Oyelaran on the street: Dmitri Garland, for Shakedown" (d9
01:00), "Dmitri Garland (71 y) resisted arrest and fought off the guard ..." (d10); "Mirabel Abernathy found the body
of Perrin Oyelaran at Parts Fab#4899; the law's case is open on the killer already wanted (Mirabel Abernathy saw it
done and said nothing: she runs with The Red Lanterns, and the law is not their way)" (d15).

**Reads wrong / open** (the next pass): a member on the corner at the fight who is not one of the six reads as absent
(the line gives the count, not him); a called member who is asleep sleeps through the call; 128 sleepers meant to
tell and only 18 did inside two days (the tell needs a guard on the street within 12 tiles, and ReportCrime's Hall is
out of reach); no raid walk-off or released line met a 20k pick, and the 100k cultist case is read from the code only
(the brief named no 100k run); the war's start reaches only the members around its cell, so a member far off hears
only lying low, the truce and the quiet.

**Under 18.** `turf::hearers` takes no protected member. The corner muster and the fight were already gated by
`ages::is_protected` (`corner_hold`, `can_fight`), and the raid's tail check skips a protected marcher. The case never
names a protected killer, and `keeps_from_law` names no protected suspect ("says nothing"). A sleeper's waking line
names the extorters who took his coins (the shakedown's victims were already adults only).

Probes re-pinned in place with "Round 7 W2" (green on main, red on the branch on their premise): `person::probe_widow_commissions_a_hit_at_the_fixer`
(the widow found her husband's body at the killing and the case's scene roll named the killer, jailed that evening: the
probe holds `lead_p`/`tie_lead_p` at 0, a killer at large), `person::probe_pimp_purse_grows_on_a_pitch_night` (a payer of
the pimp's first Extort told the beat and he was jailed before his pitch night: nobody tells the beat in the probe),
`factions::test_muster_point_is_held_home_nearest_target` (the rule moved: a held Home past `muster_near_tiles` nearer the
target than the Hideout is the muster), `scavs::test_scav_prey_ranked_past_cover` (the bare district's law stance read full
cover on the moved city: both districts set to Patrol, the cover the coverage alone).

No test (doctrine 2). `SAVE_VERSION` 104 (renumbered at the merge): `consequence::State` `slept_through`,
`owed_word`, `[crew] march_wait_ticks`, `[crime.case] fear_courage_max`, the `Jobs2Cols` `war_called` ...
`witness_quiet` columns (serde defaults).
### W1, the household buys for its children (`OPEN-243`, `OPEN-231`; `shelf::kids_short`, `shelf::own_held`, `shelf::feeds_household`, `camp::adults_share`, `routine::grocery_plan`, `routine::run_level`, `lod::stat_grocery`, `goals` Eat, `[camp]` W1 keys)

**The causes** (20k `isle_m_k2` seed 42 x 20, main `795943c`; a temporary probe at the children's meal printing each
unfed household-night with its adults' tier, job, coins, carried food and claim, then traces of one Block's runs):
1,008 household-nights unfed in 20 days, 628 with a free parent holding 6c or more and 201 more with the coins in
another adult of the household; at 20k 500 agents are Full, so the off-screen run is most of it. Four causes, in
the order the probe found them:
- **The adults ate the children's food.** A jobless mother bought the children's two units by day (`stat_grocery`)
  and the household's adults ate them by the evening: `camp::adults_share` kept the children's food back from the
  adults only on a warning.
- **The common units fooled every household.** In a shared Home each household's claim is its stake and the common
  units, so every household reads the whole common pool: in Block#1226 (94 households) three common units held
  back every stakeless family's run, the units were eaten by the evening, and at midnight a dozen children went
  unfed on a claim of 0 beside parents holding 10-79c.
- **Food carried counted against the children.** A worker's packed meals filled `meals_held` (Full) and `carried`
  (off screen) toward the run's level; only a warning read the pantry alone.
- **The Full worker never had an hour.** A day worker off shift at 18:00 under the energy line (`[life]
  grocery_energy_floor` 0.4) slept at 18:02 and was up only in the small hours, which `day_worker_night` keeps from
  the run (`agent_15943`, a gunsmith, 47c); his wife's run at 22:53 was dropped at midnight by the same rule and
  she wandered the street till her shift. A hungry worker's Eat went to the planner and bought for himself.

**Built:**
- **The children's share is short** (`shelf::kids_short`): the household's own units (`shelf::own_held`: its stake,
  scaled to the stock when the stakes are above it; the common units left out; the whole stock on a common shelf)
  under its children's food (`[ages] child_pantry_days`). While it is short the run reads the pantry alone (Full
  `run_level`, off screen `stat_grocery`: carried food feeds no child), the "worth the walk" and "one run cannot fill
  it" gates do not apply (a parent walks for his children's meal however short), and the run starts down to `[camp]
  kids_grocery_energy_floor` 0.15, in a day worker's small hours too.
- **The household's purse** (`shelf::feeds_household`): any free adult living with a child of his household (a
  parent, a grandparent, an adult son) shops for its children while their share is short, Full (`shopper_job`) and
  off screen (`stat_grocery`'s jobless branch, by day as before).
- **Hungry or fed** (`routine::eat_is_grocery`): a hungry adult's Eat is the grocery run while the children's share
  is short; the run eats one at the shop (`[GoTo(Market) > BuyFood > EatFromInventory > GoTo(Home) > StoreFood]`,
  `grocery_cap` buys the shortfall and one). A fed adult's Eat scores `[camp] kids_grocery_score` 0.7 ("the
  children's food short", above the errand's 0.5).
- **A warning moves the purse first:** on the service's warning the run scores `[camp] warned_grocery_score` 0.9
  ("the children on the service's warning") with no bedtime cut, and off screen the hour's run comes before the
  hour's leisure (`lod::run_statistical`: the grocery run moved ahead of `leisure::stat_hour`, for everyone).
- **An adult with a meal's coins does not eat the children's share** (`camp::adults_share` keeps it back from an
  adult who `life::can_buy_meal`, warned or not); the broke still eat it, so a household that truly cannot buy
  meets the ladder as U2 built it.
- **The `[family]` line** says whether a warned child ate: "on the service's warning since d20 (fed last night)" or
  "(unfed N nights)". `camp_parent_106052`'s 4-y (V15, 100k) was not a hang: the father, 2-3c in his purse, bought one
  unit a run (`d22 07:11`, `d23 21:33`: a unit is two of the child's nights) and the child ate its half unit each night (the line floors the claim, so "0 of it
  ours" beside a fed child); the warning stands its 30 days (`warn_repeat_days`, Dylan's) while the child is fed,
  and no second spell comes.

**Numbers** (20k island seed 42 x 30, `--scaled --ambient-flats 50`, before = main `795943c`, one run each):

| | before | after |
|---|---|---|
| ChildTaken | 90 | 41 |
| taken on a second warning | 42 | 17 |
| ... of those, the purse covered the food at the warning (the Custody line's "covers" or "covered") | 30 | 7 |
| warnings | 316 | 154 |
| ch_camp daily mean | 22.8 | 11.3 |
| warnings standing past 14 days (`ch_warned_old` mean) | 24.2 | 7.3 |
| street takes | 32 | 8 |
| starvation deaths | 0 | 0 |
| school days / child-days | 9.4 % | 9.1 % |
| pocket units to the shelf | 780 | 396 |
| evictions; thefts | 1,377; 1,248 | 1,369; 1,167 |

The probe's unfed household-nights (20 days): 1,008 -> 617; the nights that bring the warning (a second unfed
night) with a free adult holding the coins 110 -> 25, the broke ones 25 -> 25. School days do not move: at 20k
school is the far, full and unpaid seat (`school_far` 17k child-days), not the Camp's headcount.

**100k x 30** (seed 42, `isle_m_k2_100k`, after only, against round 6's `target/r6_100k/analyze.txt`; 63 min):
ChildTaken 4,103 -> 3,879, taken on a second warning 3,639 -> 3,430, warnings 5,140 -> 4,583, ch_camp mean 1,188 ->
1,085, warnings standing past 14 days mean 72.2 -> 19.0, starvation deaths 18 -> 9, school days 15.2 -> 15.1 %. **The
100k number is not the purse.** Of the second-warning takes, the household's purse covered the grace's food at the
warning in 331 of 3,639 (r6) -> 131 of 3,430 (W1), and one night of it in 29 -> 25; the other 3,279 -> 3,274 are
households that could not pay one night's food at the warning ("Joss Moreau has 14c of the 20c of food and no friend
to ask" for three children; "Nerys Kimura has 0c of the 3c"): a unit costs 1-4c (the meal price the warning reads),
and the families are poorer. V15's reading (a tenth of the children in Camps from families with coins) holds for the
diaries it quoted (`ceo_20314`, `ceo_61159`) and for 20k, not for the 100k count: at 100k U2's ladder takes the
children of the broke, about 3,300 in 30 days, which is the size Dylan's ladder makes of this city's poverty (a call
for Dylan, not this bug: no safety net, `[policy]` school meals off).

**Shadow** (20k seed 42 days 20-27, `target/w1_shadow`: neglect_parent, camp_parent, school_parent, ceo,
camp_jobless x2): the warning moves the purse (`neglect_parent_17209 d20 00:10` "Rook Fletcher (9 y), Hesper
Fletcher (8 y), Taz Fletcher (1 y) went 2 days unfed ... (24c between them covers the 5c of food)", `d20 18:10`
"GOAL Eat (was Unwind): score 0.900 ... because the children on the service's warning", `19:30` "-12c (Food), purse
3c", inventory food 8 -> 11, home at 01:38, and every morning after "on the service's warning since d20 (fed last
night)"); a fed parent shops for the children (`school_parent_20680 d22 04:30` "GOAL Eat (was Sleep): score 0.700
... because the children's food short", BuyFood -6c, purse 1c). No ChildTaken in the ten diaries. The household
that cannot buy meets the ladder (the run's events): "came to the door of Block#535: Moth Inchbald (17 y), Hiro
Inchbald (15 y) went 2 days unfed ... Static Inchbald has 1c of the 3c of food and asked friends for the rest:
Corwin Whitlock had nothing to spare; Ivo Varley had nothing to spare; Hana Upton had nothing to spare: still 2c
short" (d19), then d22 "took Moth Inchbald from Block#535 ...: unfed 3 days again, inside the warning of d19".

**Reads wrong / open** (the next pass): a run started late is cut by Sleep or a Contract on the walk home
(`neglect_parent_17209 d20 22:19` "plan interrupted: Eat -> Contract"; `agent_15943 d3 20:13` "Eat -> Sleep") and
the food reaches the shelf only through the midnight pocket or the next Idle's StoreFood; a far Market (79 and 159
min each way at night from the street) is still the least-detour pick. 7 of 17 second-warning takes still had the
purse at the warning (coins spent before the run, or a Full worker's hours). 330 single unfed nights with a
parent's coins remain (no warning: a child's first night home, a run that ended after midnight). The hungry
grocery run's eat step applies to any hungry adult's idle run, not only the children's.

Under-18: nothing here touches a victim pick, a fight, a fine or a punishment; nothing calls `law`. The children's
food is bought and kept back for them; an unfed day still counts only toward the service's take. No test
(doctrine 2). No save change (the `[camp]` keys are config, serde defaults).

### W5, contradictions (V15 item 6, `OPEN-237`; `household::breakout`, `Crime::CampEscape`, `vice::set_done`, `vice::on_set`, `consequence::paid_awake`, `law::raise_crime_core`, `stims::Stand`, `ages::pick_school`, `social::propose_allowed`, `lod::stat_flirt`, `search::hourly`, `search::on_ask`, `[crime] sentence_days_ext.camp_escape`)

20k island seed 42 x 30 (`--scaled --ambient-flats 50`), one run each: before = main `bdb3d66`, after = the branch;
counts from the event log by name. Shadow `target/w5_shadow` (the branch before the last two fixes) and
`target/w5_shadow2` (the branch): camp_parent x2, dancer x3, ceo x2, dealer x2, school_parent x3, jobless x3,
gang_member x2, d20-27.

1. **The father jailed 20 days for "Abduction"** (`camp_parent_51134 d22 19:18`). The break-out raised and filed
   `Abduction` (`household::breakout`), the term of a chrome harvest. A parent taking his own child back is now
   `Crime::CampEscape` ("Camp Escape", the warden's matter: just above Theft, `[crime] sentence_days_ext.camp_escape`
   2 days), raised at the door and filed by the warden; a failed try is filed before the cuffs, so the arrest names
   the charge (it had read "arrested X" with no report). **The flipped witness lines are no flip**: Sprocket
   Abernathy#102760 was at the Camp at 19:18 (he saw the break-out and ran), was sighted there by `ceo_61159` at
   19:35, and at 20:36 he "committed Abduction" seen by Cipher (stealing at the Street Market seven tiles off) and by
   `ceo_61159` at the same Market: a second parent's break-out at the same Camp the same evening, both labelled
   Abduction. With the label right the two lines read as two escapes. 20k: one break-out a run (before and after),
   now "the city reported Kenji Fletcher for Camp Escape" and no 20-day term.
2. **The barred man back in the room** (`dancer_74279 d21 20:32`). The bar holds (no change to the door or
   `vice::barred`). The set's tally carried him: the 20:32 tip was taken in a set the Flee cut, the cut block never
   reached `vice::set_done`, and its tippers rode into the 23:31 set's line. Two holes, both closed in
   `vice::set_done` / `vice::on_set`: a stations block cut short closes its set (`exec::actions::on_abort` calls
   `set_done`; a block that is no set clears the tally), and a set counts only the room and tips taken while the
   dancer is on the stage (`ClerkWork` in progress; the 20k `dancer_17424 d23 18:00`: the client barred during her
   rest before the set was named a tipper "at" the 18:05 set). Sets naming a client barred earlier that night 12 -> 9;
   all 9 left are sets in progress when the rough happened (he tipped in that set, then was thrown out), none later.
3. **The CEO robbed and never reacting** (`ceo_20314 d23`). W2's waking covers the asleep half, but the sleeper
   still *witnessed* the shakedown: the witness roll took bodies asleep (`ceo_16592 d21 01:03`, 20k: "Static Adeyemi
   saw Hale Tanaka commit Shakedown" asleep in his block, a SawCrime at the minute, then W2's waking line). A sleeper
   sees nothing now (`law::raise_crime_core`). The awake half (`d23 08:26`, `d27 10:59`: robbed at home by one man
   twice, nothing but the money line) had no line at all: the payer's hue needs the street. `consequence::paid_awake`
   gives the awake payer W2's line: he means to tell the law (the word owed, as the woken sleeper's) or tells nobody
   and why (`law::keeps_from_law`), "again" when the same man robbed him inside `tell_days`. 20k: 443 such lines (227
   tell nobody, 23 "again"); woken-sleeper lines 237 -> 327.
4. **The stand's five Witness and money lines** (`dealer_101293 d22 19:01`). Each sale raised Dealing and every
   witness its own line, and each sale paid the dealer's cut. A stand (`stims::Stand`, `World.stands`, the `Deal`
   step at a Bar or on a corner) now folds them: a sale's witness at an open stand goes into its tally (memories,
   reports and reactions unchanged), and at the stand's end (`end_deal`: done, abort, death, arrest) one line ("20
   saw Rhoswen Coldwater deal stims on the corner by Block#759 (12:57-13:33, 5 sales): ...") and one money line, the
   cut paid in one sum. Per-witness Dealing lines 1,054 -> 0, stand lines 361.
5. **"Beyond the purse" with 10c in it** (`school_parent_48165 d22 09:30`). The midnight pick read
   `leisure::spendable` (the coins less the rent's week and the meals); the line at 09:30 read as the morning's purse
   (the night shift's wage in it). `Child.school_purse` keeps the pick's coins and what was kept back, and the line
   says it: "the 2c fee at School#4559 was beyond the purse at midnight (4c in hand, 4c of it kept for the rent and
   meals)". The rule (rent before the fee) is unchanged.
6. **A marriage while the groom is in a cell** (`sick_65180 d26 21:40`). The off-screen flirt (`lod::stat_flirt`)
   proposed from or to a body under a `Sentence`. Nobody held proposes or is proposed to (`social::propose_allowed`,
   both tiers), and nobody courts from a cell. Marriages with a party under a sentence, by name, 55 -> 25; `marry` is
   reached only through `propose_allowed`, so the 25 are name collisions in the 20k log (a Kira Inchbald married
   inside a 10-day term with no release line; names repeat at 20k).
7. **The told lead dropped** (`jobless_6217 d24`). Two causes. The friends were asked eight minutes into a six-hour
   sleep (`search::hourly` took sleepers): a sleeper asks nobody and is given no door until his first hour up. And
   the lead's door was a Market, a board: `on_ask` read the board and never said what came of the lead. A lead's door
   that is a board now says it first: "asked at Street Market#4300 about work (Solene Oyelaran told of it): the post
   was gone; the board there: a Club Staff post at Club#5005". 20k: leads told 54 -> 67; their outcomes 40 hires, 19
   "the post was gone"; no lead's door was a board in the after run (that line is read from the code).

**Numbers moved for known reasons** (FINDING, not claimed): the sleeper is no witness, so Witness lines 12,100 ->
9,957 (with the stand fold), Reports 1,681 -> 1,627, arrests 1,315 -> 1,200, jailed mean 151.9 -> 137.9.

**Reads wrong / open** (the next pass): 5,033 marriages in 30 days at 20k (a quarter of the city; the off-screen
flirt's rate, outside this phase); the stand's line comes at the `Deal` step's end, which can be an hour after its
last sale ("15:38 ... (14:03-14:11, 3 sales)"); a sale's witness learns it at the minute but his diary line comes at
the stand's end; no awake CEO shakedown met a pick in the final shadow (the line is quoted from the first shadow's
payers); "commit Camp Escape" is the generic witness wording; a set's "room" now counts only arrivals during the
set, so more sets read as empty rooms (W6's).

**Under 18.** No pick widened. The break-out was a parent's adult act already (the child is in no fight); the
Camp-escape charge lands on the parent only. `paid_awake` speaks for payers the shakedown already filtered to adults,
and `keeps_from_law` names no protected suspect. The sleeper filter and the stand fold only remove lines.

Re-pinned in place with "Round 7 W5": `civic::test_breakout_is_a_contest_at_the_door_and_files_abduction` (a Camp
escape is filed, not an Abduction) and `contracts::test_conspiracy_ranks_below_murder` (CampEscape moved every rank
above Theft up one), `stims::test_deal_pays_gang_and_dealer` (the cut is paid at the stand's end). No test
added (doctrine 2). `SAVE_VERSION` 106 (renumbered at the merge): `Crime::CampEscape`,
`[crime] sentence_days_ext.camp_escape`, `World.stands`, `Child.school_purse` (serde defaults).

### W6, the dull rooms (`OPEN-217`; `customers::venue_gang`, `customers::cause` "trouble", `leisure::barred_returns`, `leisure::bout_purse`, `leisure::fight_bout`, `customers::hurt`, `demography::pool_member`, `care::book_clinic`, `care::tally_clinics`, `citysim-cli/src/shadow.rs` the gossip fold, `[customers]`, `[leisure]`, `[care]`, `[noise]` W6 keys)

- **The causes** (20k `isle_m_k2` seed 42, main `bed8e38`; a temporary probe on the off-screen door, removed).
  (1) **The door's colours never saw a held cell**: `customers::venue_gang` fell back on `gang::holder_of`, which reads
  a gang's held Homes, and a venue is never a Home, so only a front's door ever read a rival's colours (2 refusals in
  30 days). Off-screen gang members (past `[lod] gang_quota`) do reach the doors (350 of 36,950 off-screen arrivals at
  kept doors, 266 of them at a rival-held cell); the drink and the purse were the only other state the door read off
  screen. A visitor's name (the word's `Reputation.heat`) is carried by both tiers and was never read at a kept door.
  The barred off screen always stayed away (round 4 T7's `door_seen`), so "barred" never came off screen. (2) **The
  bout had no stakes**: `fight_bout` took 0.3 energy and safety from the loser and printed nothing more; no coin moved.
  (3) **The pimp who runs no round**: U6 kept a named pimp out of `pick_candidate`, but the midnight hire is pooled
  (`HirePool`; `pool_member` had no such filter): a pimp quit "to run the street" at 00:07 and the pool hired him back
  at 00:11 (Ottilie Vasquez five nights running, d6-d12; Percival Northcott twice), then lost the street "holding a day
  post" or "away from every pitch 3 days" (V15's `pimp_62797`: "chose the shift as Vat Tech"). (4) **The clinic with a
  doctor and no patient**: every pick of a Clinic is the nearest (the wash and the off-screen chrome booked at the
  Clinic nearest the Home, the patch-up at the nearest open), and V6's spill moves a patient only when the nearest
  doctor's hour is full (`[[serves]] per_hour` 4, an orderly 6), which the city's few patients an hour never fill: a
  Clinic ringed by nearer ones sees nobody (20k `ripperdoc_11516`, Ripperdoc#5892, 0 patients in the base shadow
  week, no wash in 30 days). (5) **GOSSIP ABOUT ME** folded a day per rumour, so a madam with a new rumour about her
  every few hours and the old ones retold to ears that knew them printed a line each (V15 `madam_35331 d27 04:17` "told
  22 times today (22 heard already)").
- **Built.** The venue's gang is the turf holder of the cell its door stands in (V2) before the old Home reading, both
  tiers. A name at or above `[customers] door_trouble_heat` (0.6) is "trouble" at a kept door (on `door_p`, a grudge,
  no swing). An off-screen face barred from a house tries its door anyway on `door_stat_barred_p` (0.3, a hash of him,
  the venue and the day; drunk, always) and is turned away "barred" (`leisure::barred_returns`); the bar holds (W5's
  side). Off-screen refusals are counted by cause (`door_stat_*`). The bout: the winner takes a purse from the house
  (`Flow::GambleWin`), `[leisure] bout_purse` (5) plus `bout_purse_share` (0.25) of the main event's stakes; the loser
  is hurt by `bout_hurt` (0.3) + `bout_hurt_margin` (0.5) x the winner's margin in fighting, a wound to have seen to
  (`Misery.hurt_at`, read by `customers::hurt`, so the patch-up or the back room follows, both tiers; no `Lost` memory,
  the bout stays sanctioned); the line reads "X beat Y at Z (bets paid N); purse 6c; Y hurt, to have it seen to"
  ("badly hurt" at `bout_hurt_bad` 0.45). A named pimp is out of the pooled hire (`pool_member`). A booked visit (a
  wash, a new implant; a used implant stays with its Clinic's stock) goes to the Clinic with a Ripperdoc on staff and
  the fewest bookings today per Ripperdoc among the `[care] book_cands` (6) nearest, within `book_slack_ticks` (30) of
  the nearest (`care::book_clinic`); Clinic-days with a Ripperdoc on staff and those with no patient are counted
  (`care::tally_clinics`). The shadow's GOSSIP ABOUT ME gives at most `[noise] gossip_lines_day` (2) rumours a day a
  role their own line, a rumour opens one only when it reaches new ears, the rest fold into one "N retellings today of
  K other rumours" line a day a role.
- **Deviations.** "The bout's hurt (`wounded`, scaled)" is the care system's wound, not `wounded::down`: a downed body
  bleeds out as a murder by its `by` (`Wounded.murder`), which a sanctioned bout is not; the scale is the hurt's size
  and the line's word. "Trouble" is the name's heat, on screen too (a body with a hot name meets the same door). The
  patch-up keeps the nearest Clinic (a wound walks to the nearest door; the spill stays as V6 built it); only the
  booked trade spreads. The 20k city does not reproduce V15's gossip flood (9 lines in 9 base diaries), so the cap
  never bound here; it is for 100k.
- **Under-18** (`ages::is_protected`): the bout card's gate is V6's (`!is_protected` on the roster) and `fight_bout`
  backstops it (no hurt on a protected loser); the purse goes to the adult winner. The door's new causes come after
  "under age" (a minor is turned away "under age" first, no scene, no grudge); the barred return and "trouble" pick no
  victim. A pimp's pick is U6's (`fit`: never a protected member). The booked patients are adults (`book_washes`'
  filter, the chrome's buyers).
- **Shadow** (20k, seed 42, d20-27, picks bouncer:2, fighter:2, ripperdoc:2, dancer:2; pimp:2 from d25, no pimp being
  named on d20 in the moved city). The door (`bouncer_17958`, Zane Yates, Club#4896): `d22 19:47` "turned Silvio
  Garland away at the door of Club#4896 (trouble)", `d23 00:00` "8 let in, 2 turned away (high 1, trouble 1)", `d24
  18:49` "turned Greta Stroud away ... (a rival's colours) (turned away 2 times today: a rival's colours x2; barred
  through d25)", `d25 00:00` "20 let in, 3 turned away (a rival's colours 3)" (the base bouncers: drunk, broke,
  carrying). The pit (`fighter_11655`, Odessa Okafor, Fight Pit#5578): `d21 23:00` "Raven Dunmore beat Odessa Okafor
  at Fight Pit#5578 (bets paid 0); purse 5c; Odessa Okafor hurt, to have it seen to", `d22 11:56` "Echo Greaves patched
  Odessa Okafor up in the back room of Back Room#1971 for 6c", `d23 23:00` "Odessa Okafor beat Raven Dunmore ...; purse
  5c" with `[money] +5c (GambleWin)`, `d25 23:00` lost again, `d26 11:37` "Lavinia Northcott patched Odessa Okafor up
  at Ripperdoc#5566 for 12c"; `fighter_8049` (Tess Duarte) the same rivalry with Ulric Vasquez, purses 6-9c. The pimp
  (`pimp_22006`, Ivo Voss, The Drowned Choir 9): rounds at `d25 00:51`, `03:08`, `23:41`, `d26 01:29`, `03:54` ("came
  round the pitch of Adela Vasquez (60 y) and took the cut: 2c of 5c since the last round (1 client)"), no day post.
  The clinic: `ripperdoc_10289` (Lavinia Northcott, Ripperdoc#5566) 27 patients in the week (installs, washes,
  patch-ups); the base's empty Ripperdoc#5892 7 patients in the week, 65 in 30 days (base 24).
- **Reads wrong** (the next pass): the loser's patch-up (12c at a Clinic, 6c in a back room) eats more than the 5c
  purse, so a fighter who loses two of three is down on the week (harsh, plausible; the purse is a placeholder);
  `pimp_21304` (Kasimir Jarrow) was named d23 in a district whose pitches were closing and lost the street d25 "living
  off the gang's blocks" with no round, then took a Vat Tech post (named on a dying street); the colours refusals are
  now the door's commonest (369 of 577 lines; Vale Delacroix 11 times in 30 days, the return bar holding him a day at a
  time); off-screen "barred" is 1 in 30 days (few off-screen faces are barred); 291 of 757 Clinic-days with a
  Ripperdoc on staff saw no patient (the booked trade spreads, the walk-in patch-up does not); the dancers' rooms of
  1-2 (`dancer_18200`) are the venue's traffic, not this phase's; Ivo Voss's 14.7-19.5 h of travel a day after he
  takes his gang (d26-27).
- Numbers (prints; 20k seed 42 x 30, `--scaled --ambient-flats 50`, one run each; base main `bed8e38`):

  | | base | after |
  |---|---|---|
  | door refusals (`door_refusals`) | 195 | 641 |
  | refusal lines by cause | drunk 59, broke 52, high 16, carrying 10, brawler 7, dress 4, colours 2 | colours 369, drunk 81, trouble 55, broke 46, brawler 13, high 11, barred 1, dress 1 |
  | off-screen refusals by cause (no base column; the base off screen read the purse and the drink) | | broke 46, trouble 97, colours 97, barred 1, other 1 |
  | bouts / purses paid (coins) / losers hurt | 512 / 0 / 0 | 515 / 515 (3,937c) / 515 (2 badly) |
  | patched (all) | 1,685 | 2,125 |
  | named pimp-days / with a collection (no base column) | | 30 / 19 |
  | a pimp's "quit ... to run the street" / lost the street "holding a day post" | 8 / 2 | 5 / 0 |
  | pimp collections (`pimp_collections`; a moved city) | 59 | 49 |
  | Ottilie Vasquez's street (D4): days, collections | d6-d13, 8 | d6-d17, 25 |
  | patients per Clinic, d21-27: median, min, max, Clinics with none | 4, 0, 100, 8 of 34 | 6.5, 0, 77, 2 of 36 |
  | patients per Clinic, 30 d: median, min, max | 34.5, 1, 398 | 47, 2, 359 |
  | Clinic-days with a Ripperdoc on staff / no patient (no base column) | | 757 / 291 |
  | washes | 551 | 554 |
  | GOSSIP ABOUT ME lines a diary-week (20k shadow; different picks) | 1.0 (9 in 9) | 2.5 (20 in 8; one bouncer assaulted three times) |

  No test (doctrine 2). One probe re-pinned on its premise: `person::probe_sixty_four_with_savings_retires` (on the
  moved city the 600c bought a 500c Legs implant at 17:52, before the midnight roll; the savings now land ten minutes
  before it). `SAVE_VERSION` 106 (the orchestrator renumbers at the merge): `Misery.hurt_at`,
  `ViceState.collected`, `care::State.seen`, the W6 keys and the `Jobs2Cols` `door_stat_broke` ... `clinic_idle_days`
  columns (serde defaults).
