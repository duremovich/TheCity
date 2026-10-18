# Shadow pass V2: does the living city live?

2026-10-08, seed 42, main at a737870 (Life pass L2 merged: jobs at seven leisure kinds and a Parts Fab, the fun need and Unwind, HangOut toward known contacts, venues with money and door policy, bouts, off-screen leisure, gang fronts with Collect/Call/Preach, held prisoners, gang quotas, off-screen faction violence, gang desistance, the Reserve release). The re-shadow V1 asked for, in two windows: days 19-26 (a Friday start, so `worker_friday` starts on its Friday) for 17 archetypes x 3, and days 90-97 for the gang archetypes, when the gangs are full. Every event is a seeded game abstraction.

```
citysim-cli shadow --seed 42 --start-day 19 --days 7 --count 3 --pick gang_member,gang_leader,ripperdoc,homeless,ceo,guard,worker,runner,purist,dealer,reporter,cook,club_staff,fighter,fabber,sweeper,worker_friday --out shadow_v2/pinned
citysim-cli shadow --seed 42 --start-day 19 --days 7 --count 2 --no-pin --pick gang_member,homeless,worker,cook --out shadow_v2/nopin
citysim-cli shadow --seed 42 --start-day 90 --days 7 --count 3 --pick gang_member,gang_leader,dealer,runner,purist,homeless --out shadow_v2/d90
```

41 pinned diaries, 8 no-pin diaries and 18 day-90 diaries; the diaries are regenerable with the commands above. Six critics read one diary per archetype (the one closest to its archetype's averages, never a jailed one). The critiques are in `docs/shadow_v2/`: `cook_and_club_staff.md`, `fighter_and_worker.md` (the plain worker took the dealer's slot: day 19 has no dealer), `guard_and_sweeper.md`, `street.md` (homeless, Friday worker, the Statistical stand-ins), `trades_and_top.md` (ripperdoc, Fab tech, reporter, runner, CEO) and `gang.md` (day 90: member, leader, dealer, runner, Purist). Day 19 has no dealer candidate, one runner, one reporter, and its only Purist is a gang leader (Dorian Dunmore, The Unplugged, a gang of one).

**Verdict.** The skeleton V1 asked for now holds: people sleep in blocks, are paid where they work, keep a weekly rest day, meet named contacts on a street corner, and gangs pay their members. One archetype reads as its type: the **Fab tech** ("a factory shift and an evening"). Four are recognisable in their role but not alive: the **guard** ("about half right"), the **Friday worker** ("a Street-class croupier, not yet a person"), the **worker** ("a worker who is being broken") and the **cook** ("partly"). The **homeless**, the **gang member** ("deals on the side of a life spent walking"), the **gang leader** ("collects but does not lead"), the **dealer** ("not yet a business"), the **Purist** ("a gang-aligned street fighter"), the **ripperdoc** ("a clerk with a medical job title") and the **CEO** ("a salaried office worker who walks") are recognisable in name and numbers only. Five are blocked: the **reporter** (two bugs keep her out of her own office), the **fighter** (the bout happens without her), the **club staff** ("a 58-year-old man who walks"), the **sweeper** ("the role does not hold") and the **runner** (dead on day 19 in the first jack-in; on day 90 "a kid with a deck and no reason to use it"). The **child** is still unpinnable. Two causes do most of the damage: distance (home, work, market, Hideout and hang-out spot sit in different districts) and a run of commit bugs (a commute, a shift, a sleep, a purchase or a Lead that is dropped half-way and paid for twice).

## What L2 fixed (against V1 and L1b)

- **Walking.** 3.3-9.8 h a day across the day-19 rows (L1b 2.8-12.1; V1 diaries: gang member 17.4, guard 16.9, worker 10.4). Day-90 gang rows are 9.3-11.1, still the worst.
- **Sleep blocks.** 4.4-6.4 h a day with a longest block of 6.0-8.0 h (L1b 1.5-6.8 and 4.8-8). Nobody sleeps in one-minute fragments at energy 0.
- **Energy-0 hours.** 0-16 per triple (L1b 0-18, L1 off 0-76). The worker is at 10 (V1 diary 36, L1 off 60), the guard at 0 (L1 off 54).
- **Wages at the workplace.** Every trade is paid at its desk at shift end, with no Hall walk. Shifts / paid: workers 13/9, Friday workers 13/8, fabbers 18/17, sweepers 12/11, cooks 15/12, club staff 18/11, guards 11/18 (patrol days are paid by the clock). V1's worker worked 3 days in 6 and was paid once. The ripperdoc's quit/rehire cycle and the off-shift wage chase are gone. The exec is 33, collects an ExecWage every midnight (27 payments, 367 c over three execs), keeps office hours and idles 5.9 h a day (V1: 18).
- **HangOuts with known contacts.** 9-61 HangOuts per triple with 11-59 known contacts met (V1: workers saw nobody). The first street friendship (Vale Hallow and Omar Duarte, Acquaintance to Friend in four days), dice with a spouse, rumours passed at the croupier's table.
- **Collect.** The Hollow paid "1263 to 60 members (the week's tribute and take 2603)", 21 c each; The Unplugged 169 c to 50. V1: extortion took 0 c and the leader's treasury was 4 c for 33 members.
- **Bouts.** A nightly pairing at a Fight Pit (Lena Kimura: 7 bouts, 5 wins), with a rival who becomes a friend through the work.
- **Deals.** Four dealers ran 24 Deal shifts at gang fronts; witness lines fell from 58 in four hours (V1) to 14 in a week. The economics are wrong (cause 14).
- **Held prisoners.** A Statistical prisoner's week in a cell is quiet: one −1 c rent tick a midnight and nothing else (V1: ~70 affinity gains and ~40 MetInJail memories a day).
- **Arrests.** 4 arrests in 4 Arrest plans across three guards (V1 3/42, L1 off 12/105, L1b 12/25). No `PreconditionLost` on arrival, no corpse targets.
- **A fallback for the poor.** Scavenge runs and fails like scavenging (`scavenge_p` 0.15, about 1 c in 19 hours); the dole lands in place at 09:00; a homeless adult can use the Hotel.

## Cross-cutting causes, ranked by how many archetypes each distorts

`[bug]`: a mechanism doing the wrong thing, fix now. `[feature]`: a mechanism that does not exist yet, a later pass.

1. **Distance: hiring, housing, beats and markets ignore it** (~17 archetypes) `[feature]`, with two `[bug]` parts. The club hand commutes 147 min each way (60 h of travel in a week against 35.5 h of work); the reporter is hired from Sump West to a Civic desk 400 min away (`demography::pick_candidate` sorts a Feed's applicants by knowledge only); two guards share a Spire block 75 min from the Precinct; the gang member's home, Hideout and market are three districts apart (67 h walking), the leader lives 5.5 h from her Hideout (90 h, 53 % of her week). The Spire, Sump West and Sump Central have no Market: 150-203 min food walks for the ripperdoc, the Fab tech and the CEO, a 10-hour round trip every third day for the homeless. *Fix:* a commute cap of ~60 min in hiring and housing, gang members housed near their Hideout, a Market or meal point per district (a Sump soup kitchen, a Spire grocer), a Hideout kitchen. `[bug]` parts: the sweeper's beat is reallocated nightly (`jobs::sweep_beats`: Spire on d19, Civic from d21, 130-265 min each way, never a full shift) so pin the beat to the home district; and sleep at the nearest usable bed still loses to Home or the Hideout for the dealer (D3: a 160-min Sleep walk cut by Eat).
2. **The HangOut spot is far, empty or enemy-held** (~14) `[bug]`. 11 of Vale's 15 HangOuts and 7 of Lena's 16 say "nobody there", mostly 08:00-14:00; the club hand walks 195 min at midnight to an empty corner, the Friday worker 179 min, the Parlour attendant 139 min; the runner sits through five empty hours on d19 and, on day 90, returns to a street where 3-5 known Enemies stand. *Cause:* `leisure::choice` / `best_spot` weighs contacts' habitual places, not the hour or the walk, gives a lone hour full fun, and does not subtract Enemies present. *Fix:* cap the free spot's walk at ~45 min (else the nearest spot), weight spots by expected company at that hour, scale the free rung's gain by company, subtract a strong constant per Enemy present.
3. **Spouses and friends at affinity 1.00 are never seen** (~10) `[feature]`. Club hand, fighter (her wife lives one block over), worker, Friday worker, sweeper (a spouse and nine friends), ripperdoc, Fab tech, CEO, runner, guard. *Cause:* no kin term in Socialise or the spot pick; the spot is chosen by co-presence, not by love; spouses live in separate blocks. *Fix:* a weekly "see someone I love" goal (V1's worker fix, still open), a spouse "at home" in the evening, the spot pick weighted by loved contacts' habits.
4. **The commute gate flaps** (9) `[bug]`. The cook, club hand, fighter, guard, Friday worker, ripperdoc, Fab tech and CEO set out, turn back after one leg and set out again; the sweeper leaves late and arrives 1-3 h into the shift. The club hand loses ~2 h a day, the CEO aborts up to four times a morning. *Cause:* `exec::routine::must_leave_for_work` is `until <= travel_estimate + 60`, re-evaluated per leg; `travel_estimate` is Manhattan x 1.5, about double the real walk, and shrinks 1.5x per tile while the clock shrinks 1x, so the gate goes false after a leg and Idle/Unwind takes her home, where it goes true again. *Fix:* latch the commute once a leg starts (the Work plan owns the walk until arrival or shift end, as `raid_plan` owns its chain), estimate from path length at the real speed, and open the exec's "office hours" gate near the Meeting.
5. **The leisure roles' shift is `ClerkWork` with no customers** (8) `[feature]`, with one `[bug]`. Cook (537 min, no bowl), club hand (358 min, no patron), fighter, Parlour attendant (his +1.00 friend walks past the counter twice), croupier, ripperdoc (no patient all week). *Cause:* `goap/actions.rs:272` maps Cook, Host, Attendant, Fighter, Croupier and Concierge to `ClerkWork`, the generic counter. `[bug]`: `assets::seller_open` keys on any staff member's shift clock, so Ripperdoc#443 sold the CEO three Therapy sessions (174 c) while its doctor wandered Civic on her rest day; open only when a staff member is at the counter. The Fab's Parts (`jobs::accrue_fab_work`) and the sweeper's swept litter are real but invisible (tool notes). *Fix:* staff actions driven by arrivals (the cook stocks `stock_food` and serves each `EatOut`, the club hand greets each `Enjoy`, the doctor's shift turns into Treat-the-customer), a `Served` memory and Client edge, a tip or a fee cut, a staff meal from the venue's stock.
6. **A whole day's wage is forfeited when the shift ends early** (7) `[bug]`. Cook paid 4 of 6 shifts (Eat at 16:18, 100 min early); fighter 3 of 6 (Sleep at 23:19); worker 4 of 6; Friday worker 3 of 5 (15 min early, then 2 h 45 min); club hand 5 of 6 (20 min); sweeper (Earn at 15:44); ripperdoc (walks out for food on d25). The pinned worker is poorer than his own Statistical stand-in (paid 6 of 7 by the clock). *Cause:* `exec::actions::end_shift` pays only when ClerkWork completes; Eat, Sleep and Earn outbid an in-shift Work minutes before the end, and their scores do not know the wage is paid at the end. *Fix:* a shift commitment (only starvation below hunger 0.1 or danger interrupts), pro-rata pay above ~50 % worked by the shift clock as `law::credit_guard_shifts` does, Eat and Sleep after the end.
7. **Sleep is still cut** (7) `[bug]`. The cook sleeps 47 min before a nine-hour shift; the club hand 9-11 min before Eat wakes him; the worker sleeps in 25-minute pieces six times in a night while Earn holds 0.899; Eat cuts the Friday worker's sleep at 06:15 three days running; the guard logs ten Sleeps of 1-6 min at energy 0.97 (an Idle plan picking Sleep); the sweeper's blocks are exactly 240-241 min. *Cause:* V1 cause 2, smaller: no committed minimum once a sleep is cut, and Unwind/Eat/Earn beat Sleep at energy 0.4-0.5. *Fix:* a committed four-hour minimum that only starvation or danger breaks, an energy term in Unwind/Eat/Earn under 0.5, Idle never plans Sleep above a high energy; check whether hunger caps a Sleep at 240 min.
8. **Idle and the day off have no menu** (7) `[feature]`. The cook's rest day is five 90-min Rests and a scavenge for 1 c; the club hand's and fighter's are ten 30-min Wanders with "Earn unplannable"; the worker scavenges 11.9 h on his; the dealer wanders 12.9 h a day; Vale's 44.5 h are Wander. *Cause:* V1 cause 4 half-fixed: Idle is a flat 0.05 with Wander as its only plan. *Fix:* a personality-weighted idle and rest-day menu (visit, busk, market, a venue, the Hideout after three Idles running).
9. **Wages that do not buy the week** (6) `[feature]` for the late calibration milestone, with one `[bug]`. A Zetatech Parlour attendant (treasury 13,074 c) starves on d21; the ripperdoc nets −23 c; the Fab tech +3 c; the cook +2 c a day; the leader's cut equals a rookie's (22 c against 21 c). `[bug]`: seeded sweepers keep `wage 5` while `works_wage = 7` applies to new hires only, below the dole after tax by the config's own comment; the sweeper works 40 h and nets +1 c. *Fix:* seed the works wage; later, wages scaled by the employer's treasury and class, a canteen meal on shift, a leader's cut off the top.
10. **A purchase is refunded when a goal swap interrupts it** (6) `[bug]`. Cook: bought at 04:42, refunded at 04:48, bought at 04:49; worker: buy-refund-buy every evening (12 food flows for 6 meals); Friday worker: 12 c of food abandoned for Work; fighter's Braindance entry refunded after 5 min; sweeper 8 c refunded. *Cause:* `World::pending_purchase` refunds on abort (`systems/leisure.rs`), and the 30-min re-think or a failed Earn plan lands inside the 3-11 min BuyFood. *Fix:* a started purchase is atomic (the swap waits for the action to end) and commits the units with the coins; do not start a purchase within one think of a latched commute.
11. **The Flee/Unwind loop** (3) `[bug]`. The day-90 runner flees and returns 18 times in three days to a street where six people beat him; the Purist and the dealer loop the same way. V1 finding 14 unchanged. *Cause:* FleeToHome is chunked and replanned, the spot score adds `affinity x presence` without excluding an Enemy-held spot, no avoid memory. *Fix:* Flee ends only after the hostile has been out of sight 30 min, writes an avoid-spot memory, and cause 2's Enemy penalty.
12. **Nobody mourns** (3) `[feature]`. The runner is stripped in 32 min and buried by a stranger with four +1.00 friends unmoved; the gang member logs two grudges for a killed friend and does nothing; a Hollow dealer overdoses at the front while her crew rests at the Hideout. *Fix:* Death memories and a mood hit for Friend-or-above edges, a Mourn/Bury goal (a gang spend for its own), a grudge that sends a gang's fighters after a named target (M16a's delegated violence).
13. **Gossip told to people who already know** (3) `[bug]`, volume `[feature]` for calibration. 307 tellings about `homeless_825`, 195 of them (64 %) "already known / dropped", the same teller to the same listener four times; 31 tellings of a fighter's sporting result. *Fix:* a per-(teller, listener, deed) told memory with a cooldown of days; count new carriers apart from repeats.
14. **Dealing loses money and sells nothing before dawn** (2: member, dealer) `[bug]`, margins `[feature]`. Four dealers made 10 c in a week; Nikolai sold 9 doses for 9 c and bought 4 back from his own gang for 20 c; four pre-dawn shifts (00:57-05:37) sold 0; Delia used all 9 doses of her stock herself; each sale makes the buyer, often a Friend, a Dealing witness (1.00 to 0.80). *Cause:* `stims::start_deal` only registers the dealer, and nobody shops at 03:00; customers are recorded as witnesses. `[bug]`: a buyer is not a witness, and a Deal shift starts only in the venue's busy hours. `[feature]`: consigned stock with a ledger, a non-hooked dealer doses from stock at most once a day, margins in calibration.
15. **The leader's Lead and Call do nothing** (2: leader, Purist) `[bug]`, retinue `[feature]`. Freya walks 330 min to the Hideout and drops the Lead on the doorstep (`leisure::call_due` needs Evening, she arrives at night); her one Call adds `call_w` to one spot score and nobody comes; the Purist walks 291 min to a Muster and misses it by 14 min. *Fix* `[bug]`: a started Lead holds until its Collect runs or fails, `call_due` stays true for the plan's life, a Muster window scales with the farthest member's walk, Extort skips gang-mates' homes (she squeezed her own dealer into an Enemy). `[feature]`: Call as a Muster-lite with a Briefing, the pay-out handed over in person, a retinue and vehicle.
16. **The LOD re-ranking flaps** (2 stand-ins) `[bug]`. The unpinned `homeless_825` cycles Coarse/Statistical every ~3 h on d22: each Coarse hour re-plans a 48-min walk to Braindance Parlour#481, pays 2 c, and ~12 min into Enjoy the demotion aborts the plan, refunds the entry and snaps her to her Mid East door; six times a day, 7.8-9.4 h of walking, no fun. `cook_498` shows three such refunds. *Cause:* `lod::assign` re-ranks every adult hourly, the hysteresis band covers only an incumbent against an equal-priority newcomer, and `set_lod` calls `abort_plan`. *Fix:* a minimum tier dwell (3 h or the plan's end), demote at a step boundary, never start a paid step with a demotion due inside it. It matters for the player's view.
17. **The Statistical hungry worker never sleeps** (1 archetype, 2 stand-ins) `[bug]`. `worker_1686` reads energy 0.00 for 122 h and `worker_1340` for 92 h. *Cause:* `lod::run_statistical` resets energy only on a Sleep outcome drawn 00:00-06:00, and `hunger < 0.4` forces `Outcome::Eat` whether or not `stat_eat` succeeds (lod.rs:624-656); a 4-6 c wage buys one 4 c meal, which lifts hunger only to ~0.40-0.46. *Fix:* a failed or futile Eat falls through to Sleep at night.
18. **The bout is an assault the fighter may not attend** (1, three diaries) `[bug]`, injury and purse `[feature]`. The loser gets a `Lost` memory and `memory::deed_of` reads it as Assaulted: rumours, two news Stories ("Lena Kimura assaulted Tristan Kowalczyk"), dread 0.22 to 0.82, three grudges and a best friend turned Enemy at −1.00. Lena wins a bout at 23:00 while wandering Mid East on her rest day: `leisure::bouts` checks `on_shift(tod)`, not `is_workday` or presence. *Fix:* a bout memory kind (or a sanctioned flag `deed_of` skips), both fighters inside the pit, cancelled on a rest day. `[feature]` (M16b, the wounded): an injury scaled by the margin, a winner's purse, a rotating card, spectators.
19. **Patrol misses its shape** (1) `[bug]`. The guard's patrol starts at her front door and walks 3.7-3.9 h to the same four Mid East stops every patrol day; she patrols on her rest day unpaid (`Patrol` `on_duty` ignores `is_workday`; `law::credit_guard_shifts` reads it); the duty flips from Patrol to desk at 13:13 and she walks 100 min back (`law::jail_duty` recomputed per call). *Fix:* gate Patrol and Arrest on the workday, start the patrol at the Precinct with a near-first rotated route, fix the duty at shift start. `[feature]` (M16a): witness-first intervention, guard corruption. Also `[bug]`: the guard's desk days still raise jail affinity (Hale Kettering to +1.00, the jailed Ninefold leader a Friend by teatime); cap guard-prisoner affinity.
20. **The reporter cannot reach her desk and is never fired** (1) `[bug]`. Every Work plan from d21 is "unplannable: Unreachable". *Cause (likely):* the Civic Wire is city-owned (`news::seed_feeds`, owner None), so `World::wage_desk` returns the Hall and the `Workplace` entry in `goap/actions.rs:661-666` is skipped. She is never paid, not dole-eligible (the seeded Job has `paid_once`), and never dismissed (`days_unpaid` grows only in `end_shift`). *Fix:* `Workplace` is always the employer's door (Feed, Lab, Fab); dole eligibility after two unworked days; a no-show dismissal after a week. Check Lab#430 and other unowned employers. `[feature]`: a Cover action.
21. **The exec is in the labour pool and the Meeting is empty** (1) `[bug]` + `[feature]`. Juno Ito, Zetatech's exec, is hired as a Militech Fab Tech on d20 and laid off on d21 (`pick_candidate` skips Job holders only). Her Meeting is 9 h at Lab#442 with no attendee, no order read; she owns a Flyer and walks everything. *Fix* `[bug]`: exclude execs and board members from `pick_candidate`. `[feature]`: a Meeting with the corp's managers and a Brief event, the owned vehicle, an escort.
22. **The runner has no client and no fear of ICE** (1, two windows) `[feature]` (M16a), one `[bug]`. Roxy (courage 0.18, deck T1, fresh from jail for a DataTheft) jacks in on a hardened Lab and flatlines in 24 min; Kieran's one run is his own impulse on a Treasury ledger; nobody sells Data all week. *Cause:* the Hack score's fear term `1 - p_fry x (1-courage)` has no kill odds or deck-versus-ICE gap and floors at 40 %. `[bug]`: a gap guard (never route a node at `gap >= flatline_gap` unless wealth is critical). `[feature]`: the Fixer's RunOrders and a sale.
23. **Beg is unreachable from the street** (1, three diaries) `[bug]`. Zero Beg actions; `goap/actions.rs:1208` allows Beg only at a Market or a Bar. *Fix:* Beg at a HangOut spot with company.

## Per archetype

V1's "what would make it real" items are marked *done* where L2 did them.

| Archetype | What the V2 diary shows | What would make it real |
| --- | --- | --- |
| Gang member (d90, Nikolai) | Deals at a Hollow Pit, 9 doses for 9 c, buys 4 back for 20 c; 21 c pay-out asleep; 67 h walking; 14.9 h resting silent at the Hideout among five members; no sleep d93-d96 on stims; two grudges, no action | V1: tribute that pays (*done*), Hideout as base (*half*: used, no talk), dealer margins (*open*), reactions to heat and grudges (*open*). Now: housing near the Hideout, chat in place with co-present members, busy-hour Deal shifts, consignment, buyers not witnesses, a stim crash |
| Gang leader (d90, Freya) | Collect round with real sums (best line in the set); a 330-min walk to Lead, dropped on the doorstep; one Call nobody answers; Extorts like a thug (5 c, 0 c, refused, her own dealer made an Enemy); 90 h walking; paid 22 c against 21 c | V1: Collect (*done*), dues into the treasury (*done*), Muster (*done*, by members), retinue (*open*), delegated violence (*open*, M16a), Parley (*open*, M16b), a readable BreakOut (*half*: the Muster runs, the Brawl has no outcome). Now: a committed Lead, a Call with an audience, a leader's cut, a home beside the Hideout |
| Dealer (d90, Delia) | Five Deal shifts at a Spire club, three pre-dawn with no sale, at most two sales; uses all her own stock; sleep 2.6 h a day (the d90 row); a 184-min Muster and a Precinct brawl with no result | Busy-hour shifts, ambient customers, consigned stock, sleep at the nearest bed, a Brawl that reports who came out |
| Runner (d19 Roxy, d90 Kieran) | Roxy flatlines in Lab#430's ICE 8 h into the week; Kieran: one impulsive run fried by Treasury ICE, extorts for a living, 18 Flee/Unwind loops, six beatings on one street | V1: a runner archetype (*done*: a deck and a run), fixer-fed RunOrders and a sell/upgrade loop (*open*, M16a). Now: a death-risk term and a gap guard, Fried with a cost, Flee that persists |
| Purist (d90, Liesl) | Five Preach (two successes, a generic Persuade won on greed), one Squat with a fight at the door, a Muster missed by 14 min after 291 min of walking, her leader an Enemy | V1: creed goals (*half*: Preach exists, as a gate), gang-mate socialising (*open*). Now: Preach scored on creed that reads the listener's chrome, a Purist goal toward a clinic (M16b Patrol), preaching where the crowd is |
| Ripperdoc (Mags) | 9-h ClerkWork, no patient; the Clinic sells Therapy without her; 150-203-min food walks (no Spire Market); −23 c for the week; spouse never seen | V1: a Clinic counter with customers (*open*), no off-shift wage chase (*done*), no quit/rehire (*done*). Now: open only with the doctor present, Treat as a two-party action with a fee cut |
| Fab tech (Sebastian) | Six full shifts, six wages, 19 HangOuts with nine known people, side bets; Parts invisible; 155-min supper walks; spouse never seen | A "FabWork yielded N Parts" line, a Spire Market, varied spots, a spouse evening |
| Reporter (Vivienne) | A homeless Dreg hired 400 min from the Civic Wire; arrives at the Hall, Work unreachable all week; no wage, no dole, two Starving events | Distance-aware hiring, the Feed as Workplace, absentee dole and dismissal; then a Cover action |
| CEO (Juno) | 33, ExecWage nightly, 9-h empty Meeting at a Lab, hired and laid off as a rival's Fab Tech, walks with a Flyer, three Therapy sessions (174 c) at her own Clinic | V1: exec by age and rank (*done*), salary scaled to the corp (*done*), Spire housing (*done*), office hours (*done*), carry the corp's order (*open*), bodyguards (*open*). Now: out of the labour pool, a Meeting with content, the vehicle, an escort |
| Guard (Mina) | Desk days at the Precinct, paid at 18:00; patrol days 3.7 h walk from her door to the same four stops, unpaid on the rest day; 1 arrest in 1 plan with no booking or trace; befriends prisoners; no bribe offered | V1: Arrest fixes (*done*: 4 in 4), housing near the Precinct (*open*), corruption (*open*, M16a), witness-first intervention (*open*). Now: patrol from the Precinct, workday gate, duty fixed per shift, an arrest memory and booking line, jail affinity capped |
| Sweeper (Dex) | 130-265-min walks to a beat that moves nightly; sweeps 4.6-8.9 h; wage 5 below the dole; starving on d22-d25, net +1 c for 40 h; sleeps on the pavement; owns a bar he never visits | Beat in the home district, the works wage, pro-rata pay, a sweep log line |
| Worker (Emeric) | A 35-min commute, 9-h Parlour shifts with no customer; starves on d21 while employed by the richest corp; loses two wages to Eat and Earn; rest day of rubbish-picking | V1: commute and sleep fixes (*done*), a "see someone I love" goal (*open*), weekly templates (*half*: a rest day). Now: shift commitment and pro-rata pay, a canteen or wage scaled to the employer, an Attendant action |
| Worker on a Friday (Katrin) | A real croupier night shift with rumours at the table; 61.5 h travel; two wages lost (15 min and 2 h 45 min early); 0 c leisure spend with 17 c in purse; no Friday night | Shift lock, commute latch, a walk cap on the free spot, a log of rejected paid rungs, a staff hour at her own venue |
| Cook (Rafe) | On time, 9-h ClerkWork at a noodle bar, no bowl, no customer; buys lunch at a rival market and loses two wages for it; evenings on a corner with his wife and dice | Staff counter serving `EatOut`, a staff meal, shift commitment |
| Club staff (Ulric) | 147-min commute each way, 10-12 h travel days, 4 h sleep, a 6-h shift with no patron, a 195-min walk to an empty corner at 03:00; spouse never seen | Distance-aware hiring, commute latch, greeting patrons, an Unwind brake at midnight |
| Fighter (Lena) | ClerkWork at the pit; a nightly bout she may not attend, with no injury and no purse, logged as an assault: news Stories, dread 0.82, best friend an Enemy; two wages lost to Sleep at 23:19 | A bout memory kind, presence and workday checks, a Fight action; injury and purse (M16b wounded) |
| Homeless (Vale) | Squat in Sump West, dole in place, 1 c from 19 h of scavenging, 10-h food runs every third day, 11 of 15 HangOuts empty, no Beg, nothing happens to her | V1: Earn fallback (*done*: scavenge), save for a bed (*open*), idle menu (*open*). Now: a Sump meal point, Beg at the corner, company-scaled fun, "rung rejected: needs N c" lines, a small street exposure |
| Child | Unpinnable (12 candidates on d19, 57 on d90) | Child routines (V1, *open*) |

## The V2 numbers

Method as SHADOW_V1's L1 table. Hours are per free day (total minus jail, over 24), averaged over the included agents; agents jailed over 84 h or with under 3 free days are dropped. Coin and count columns are sums over the included agents. "Earned" is every inflow by flow kind except gambling and **excludes the dole** (L1's 84 for three homeless is exactly 3 x 7 x 4 c, so L1 counted it; see the tool notes). "Shifts" are days with 3 h or more of work; "paid" counts Wage and ExecWage flows. Energy-0 is 2 h per 2-hourly snapshot at 0.00.

**Day 19** (Friday start):

| Archetype (incl./picked) | walk | sleep | longest | idle | work | leisure | earned | spent leisure / drink / food / rent | HangOuts / known met | arrests / plans | shifts / paid | energy-0 h | gossip about |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Gang member (2/3) | 6.0 | 4.4 | 6.0 | 4.4 | 3.5 | 2.5 | 47 | 10 / 0 / 36 / 5 | 31 / 58 | – | 6 / 6 | 0 | 1 |
| Gang leader (3/3) | 8.6 | 5.4 | 7.2 | 1.8 | 6.2 | 1.2 | 68 | 20 / 26 / 48 / 7 | 12 / 21 | – | 14 / 10 | 0 | 49 |
| Purist (= a leader) | 7.7 | 5.4 | 7.2 | 1.5 | 7.4 | 1.1 | 18 | (in leader row) | 5 / – | – | 6 / 3 | 0 | 0 |
| Ripperdoc (3/3) | 8.6 | 5.2 | 8.0 | 3.2 | 4.0 | 1.4 | 84 | 27 / 2 / 40 / 11 | 20 / 13 | – | 8 / 13 | 16 | 20 |
| Homeless (3/3) | 6.4 | 6.0 | 7.2 | 4.9 | 0.5 | 2.0 | 38 | 6 / 0 / 76 / 0 | 33 / 24 | – | 1 / 1 | 12 | 209 |
| CEO (3/3) | 3.3 | 5.4 | 6.2 | 5.9 | 2.5 (+4.9 Meeting) | 1.4 | 367 | 118 / 6 / 108 / 16 | 9 / 11 | – | 6 / 27 | 0 | 0 |
| Guard (3/3) | 9.8 | 5.5 | 7.2 | 1.9 | 4.5 | 1.9 | 139 | 9 / 0 / 88 / 16 | 37 / 29 | 4 / 4 | 11 / 18 | 0 | 4 |
| Worker (3/3) | 6.3 | 6.1 | 8.0 | 1.1 | 5.8 | 1.0 | 82 | -3 / 0 / 28 / 10 | 19 / 13 | – | 13 / 9 | 10 | 9 |
| Worker, Friday (3/3) | 8.1 | 5.2 | 6.2 | 2.9 | 4.9 | 1.6 | 73 | 2 / 2 / 72 / 9 | 28 / 22 | – | 13 / 8 | 0 | 16 |
| Runner (0/1) | died d19 08:51 in Lab#430's ICE | | | | | | 0 | | 6 / 2 | | | | |
| Reporter (1/1) | 6.7 | 6.4 | 7.2 | 2.6 | 0.0 (8.2 Scavenge) | 0.0 | 6 | 0 / 0 / 4 / 0 | 0 / 0 | – | 0 / 0 | 2 | 0 |
| Cook (3/3) | 4.6 | 4.8 | 7.2 | 4.8 | 6.2 | 2.2 | 70 | -5 / 2 / 80 / 8 | 45 / 59 | – | 15 / 12 | 0 | 0 |
| Club staff (3/3) | 7.3 | 4.6 | 6.2 | 4.2 | 4.9 | 1.7 | 71 | 3 / 2 / 68 / 10 | 35 / 25 | – | 18 / 11 | 0 | 0 |
| Fighter (3/3) | 8.3 | 5.5 | 8.0 | 2.5 | 2.9 | 1.7 | 75 | 0 / 0 / 72 / 3 | 37 / 17 | – | 10 / 8 | 16 | 144 |
| Fabber (3/3) | 5.6 | 5.5 | 6.6 | 2.1 | 7.4 | 2.9 | 105 | -8 / 4 / 72 / 11 | 61 / 38 | – | 18 / 17 | 0 | 81 |
| Sweeper (2/3) | 5.8 | 4.8 | 6.8 | 2.6 | 6.6 | 1.5 | 93 | 4 / 4 / 48 / 6 | 21 / 32 | – | 12 / 11 | 2 | 0 |

No dealer candidate on day 19 or 20. Dropped: `gang_member_1018` (120 h jailed), `sweeper_520` (129 h), `runner_1287` (dead).

**Day 90** (the gang window; day 90 is the Collect weekday):

| Archetype (kept/picked) | walk | sleep | longest | idle | work | leisure | earned | spent leis / drink / food / rent | HangOuts / known met | tribute in | GangWork goals | Muster / Collect / Call | energy-0 h | gossip about |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Gang member (2/3) | 10.0 | 4.7 | 8.0 | 4.1 | 2.0 | 2.3 | 44 | 18 / 0 / 48 / 9 | 22 / 10 | 25 | 20 | 0 / 0 / 0 | 10 | 53 |
| Gang leader (3/3) | 11.1 | 5.6 | 8.0 | 2.3 | 1.8 | 2.5 | 51 | 3 / 0 / 80 / 12 | 45 / 50 | 26 | 17 (+10 Lead) | 0 / 5 (2 paid out) / 1 | 10 | 124 |
| Dealer (3/3) | 9.7 | 2.6 | 7.2 | 5.6 | 3.5 (Deal) | 1.8 | 55 | 65 / 0 / 60 / 5 | 25 / 18 | 45 | 20 | 1 / 0 / 0 | 8 | 38 |
| Runner (2/3) | 10.2 | 6.7 | 8.0 | 3.7 | 0.0 | 2.0 | 21 | 60 / 4 / 36 / 3 | 20 / 25 | 21 | 5 | 0 / 0 / 0 | 6 | 75 |
| Purist (2/3) | 9.3 | 6.2 | 7.2 | 3.8 | 0.0 | 3.6 | 6 | -2 / 0 / 32 / 4 | 40 / 30 | 3 | 14 | 1 / 0 / 0 | 14 | 51 |
| Homeless (3/3) | 5.9 | 6.8 | 6.9 | 4.2 | 0.0 | 1.9 | 31 | 0 / 0 / 76 / 0 | 41 / 20 | 0 | – | – | 0 | 0 |

The gangs: The Hollow 60 members, 1,272 c, order VirtRaid; The Unplugged 54 Purists, 151 c, Squat; Ninefold 18, 12 c, Raid, its leader still on a Sanitation day job. Dealers' week: 24 Deal shifts, 10 c of commission across three dealers; one died of an overdose. Runs: four, all self-started, one flatline, two fries, one trace and arrest, no `SellData`. Dropped: `gang_member_1031` (killed d92), `runner_2066` (flatlined d90), `purist_2007` (144 h jailed).

**Beside V1 and L1** (V1 diary / L1 off → on / V2; L1b has ranges only):

| Archetype | walk h/d | sleep h/d | longest sleep | idle h/d | work h/d | earned | energy-0 h |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Gang member | 17.4 / 15.3 → 8.3 / **6.0** | 2.4 → 2.9 / **4.4** | 5.9 → 7.2 / **6.0** | 4.2 → 3.5 / **4.4** | 0 → 0 / **3.5** | 65 → 46 / **47 (2)** | 0 → 54 / **0** |
| Gang leader | – / 15.3 → 7.2 / **8.6** | 4.2 → 2.8 / **5.4** | 7.2 → 5.5 / **7.2** | 2.6 → 12.2 / **1.8** | – / **6.2 (day jobs)** | 56 → 62 / **68** | 26 → 0 / **0** |
| Guard | 16.9 / 13.6 → 9.0 / **9.8** | 5.0 → 4.5 / **5.5** | 7.2 → 8.0 / **7.2** | 2.1 → 7.3 / **1.9** | 2.6 → 2.6 / **4.5** | 102 → 98 / **139** | 54 → 6 / **0** |
| Homeless | – / 9.9 → 4.1 / **6.4** | 5.8 → 5.8 / **6.0** | 8.0 → 7.0 / **7.2** | 6.6 → 8.9 / **4.9** | 0.9 → 1.3 / **0.5** | 84 → 116 / **38 (no dole)** | 76 → 0 / **12** |
| Worker | 10.4 / 7.4 → 7.6 / **6.3** | 4.5 → 5.8 / **6.1** | 7.2 → 8.0 / **8.0** | 5.1 → 1.9 / **1.1** | 6.2 → 7.6 / **5.8** | 70 → 79 / **82** | 60 → 0 / **10** |
| Ripperdoc | – / 5.4 → 3.8 / **8.6** | 4.0 → 4.1 / **5.2** | 6.0 → 5.9 / **8.0** | 7.0 → 7.0 / **3.2** | 6.7 → 7.5 / **4.0** | 132 → 197 / **84** | 0 → 0 / **16** |
| Runner | – / 8.9 → 6.7 (a guard pick) / **died d19** | 4.6 → 5.7 / – | 7.2 → 8.0 / – | 2.6 → 2.5 / – | 7.4 → 7.2 / – | 108 → 80 / **0** | 2 → 0 / – |
| CEO | – / 6.8 → 4.1 / **3.3** | 2.3 → 5.2 / **5.4** | 5.2 → 8.0 / **6.2** | 13.3 → 6.5 / **5.9** (V1 18) | 0 / **2.5 + 4.9 Meeting** | 203 → 231 / **367** | 0 / **0** |
| Purist | – / 11.8 → 2.9 / **7.7** | 4.3 → 1.0 / **5.4** | – / **7.2** | 5.8 → 2.6 / **1.5** | – / **7.4 (Cook job)** | 45 → 36 / **18 (1)** | 8 → 0 / **0** |

Arrests / Arrest plans: V1 3/42, L1 off 12/105, L1 on 6/15, L1b 12/25, **V2 4/4**. Shifts / paid: L1 off 15/13, L1 on 22/15, L1b 16/11, **V2 workers 13/9**. Ranges across all rows, L1b → V2: walk 2.8-12.1 → 3.3-9.8 h/d, sleep 1.5-6.8 → 4.4-6.4 h/d, longest sleep 4.8-8 → 6.0-8.0 h, energy-0 0-18 → 0-16 per triple.

**The no-pin set.** `homeless_1887`, `worker_1340`, `worker_1686` and `cook_1564` were Statistical all week: a dole or a clock-credited wage, a meal debit, rent ticks, needs drift. The homeless and cook stand-ins recover energy nightly; the two workers sit at 0.00 for 122 h and 92 h (cause 17). `homeless_825` flaps between tiers (cause 16). `gang_member_1018` spends 118 h in a held cell with one rent tick a midnight. Pinning changes trajectories: the no-pin `homeless_825` is never jailed, the pinned one for 71 h.

## Tool notes for V3

- **Label coins by ledger flow, not by the running action.** The dole landing during a Scavenge reads "+4c (scavenging)", during a Deal "+4c (stims sale)"; scavenged coins are booked as "Sanitation +1 from the City" by the sim itself (a ledger-label fix too).
- **Add a Dole row to the flow table** and an earned column with and without it, so V3 compares with L1 (whose earned column included the dole).
- **The homeless pick must check housing:** no Home at the start and no Job after the first hour. `homeless_825` is a housed Corp-class Clerk with 86 edges.
- **The pinned prisoner's Coarse wobble:** a pinned agent in a cell still flips tier around the Jail; keep the pin's tier in the cell.
- **Gossip-about lines:** split new carriers from repeats, and cap the per-line volume in the diary (307 lines for one agent).
- **A `--start-day` warning for gang windows:** warn when the gangs on the start day are near-empty (day 19 had one real gang of 22) or when the pick list has no dealer candidate, and suggest a later day.
- **Keep logging after a death** for a few days the events where the dead agent is the object (mourning, stripping, burial, gossip).
- **Report what a job produced:** Parts per FabWork, litter swept per Sweep, the outcome of a Brawl, the booking line of an Arrest; and log rejected paid rungs ("needs N c, has M").

## Plan

**"L2 shadow fixes" commit, now** (the `[bug]` list, in priority order):

1. Commute latch and a path-length estimate: `exec::routine::must_leave_for_work`, `travel_estimate`.
2. Shift commitment and pro-rata pay: `exec::actions::end_shift`, the Eat/Sleep/Earn in-shift interrupt.
3. A committed minimum sleep, an energy brake on Unwind/Eat/Earn, no Idle Sleep at high energy: `actions::finishes_early`, `utility/goals.rs`.
4. Atomic purchases: `World::pending_purchase` and `leisure::on_abort`.
5. Unwind spot pick with a walk cap, an hour term, company-scaled fun and an Enemy penalty: `leisure::choice`, `best_spot`.
6. Flee persists with an avoid-spot memory: FleeToHome.
7. LOD tier dwell and no paid step before a demotion: `lod::assign`, `lod::set_lod`.
8. Statistical Sleep when Eat is futile: `lod::run_statistical`.
9. The bout: its own memory kind, both fighters present, the workday: `leisure::bouts`, `memory::deed_of`.
10. Patrol on the workday, from the Precinct, duty fixed per shift; guard-prisoner affinity capped: `utility/goals.rs` Patrol, `law::jail_duty`, `patrol_route`, `MetInJail`.
11. The unowned employer's Workplace, absentee dole and no-show dismissal: `goap/actions.rs` Workplace distance, `World::wage_desk`, `world.rs` `paid_once`.
12. Clinic open only with staff present: `assets::seller_open`.
13. Execs out of the labour pool: `demography::pick_candidate`.
14. The sweeper's beat in the home district, seeded on the works wage: `jobs::sweep_beats`, `wage_sanitation`.
15. A committed Lead, `call_due` held for the plan, a Muster window by walk, Extort skips gang-mates: `leisure::call_due`, `gang::extort` targets.
16. Buyers are not Dealing witnesses; Deal shifts in busy hours: `stims::start_deal`, the witness emission.
17. Gossip told-memory per (teller, listener, deed): `gossip`.
18. The Hack gap guard: `virt.rs` Hack score.
19. Beg at a HangOut spot with company: `goap/actions.rs` Beg precondition.
20. Nearest-bed sleep over Home and Hideout; EatAtHome checks the pantry at plan time (V1 finding 10): the Sleep and Eat planners.
21. The scavenge credit booked as Scavenge, not Sanitation: the ledger flow.

Then re-run the day-19 and day-90 windows on the fix commit and compare against this table before the gate.

**The `[feature]` list, triaged:**

- **M16a:** the runner's client and the Fixer (RunOrders, a Data sale), guard corruption, witness-first intervention (the accessory rule's law side), delegated violence for the leader and grudges that send fighters.
- **M16b:** the bout injury and the wounded, Parley, the Purist Patrol and a creed that reads chrome, the leader's escort as an Escort contract.
- **The leisure/roles pass after M18:** staff actions with customers (cook, club hand, attendant, croupier, fighter's Fight action, the doctor's Treat, the reporter's Cover, the exec's Meeting with content), markets by zone and a Hideout kitchen, distance-aware hiring and housing, retinues and owned vehicles, funerals and mourning, the "see someone I love" goal and kin in spot choice, the idle and rest-day menu, street exposure for the homeless and a saver goal, Call as a Muster-lite with the pay-out in person.
- **The late calibration milestone:** dealing margins and consignment, stim sleep (a crash after a stimmed run), gossip volume, wages scaled by employer and class against food at 4 c, the leader's cut, `scavenge_p`.

The next re-shadow runs at M18 (Dylan's directive), over many NPC kinds, with the V3 tool notes above.

## L2 shadow fixes (what landed)

2026-10-08, on 56f3110. The Plan's 21 `[bug]` items, each a mechanism fix, behind one master: `[life] l2_fixes = true` (`systems::fixes::on`, which also needs `[life] enabled` and `[living] enabled`; `--l2-off` and `--life-off` turn it off, and the off city is 56f3110's byte for byte). Behaviour choices carry a sub-switch in `[life]`; plain bugs ride the master alone. The helpers are in `citysim/src/systems/fixes.rs`; every call site names its item. No item draws a roll of its own on any stream. `CITYSIM_L2FIX_OFF=7,16` leaves single items off (the bisection device used below).

1. **Commute latch and the real walk** (`commute_latch`): `routine::must_leave_for_work` holds while the Work plan made for the pending shift walks (`fixes::commuting`), and reads the walk as `exec::timed_walk` (Manhattan x `move_ticks_full` x vehicle x litter: the `commute_tpt_walk` column reads 2.05 ticks a tile; the old Manhattan x 3 shrank faster than the clock); the exec's office walk is latched in `life::exec_day_pending`; think holds a latched commute against any non-emergency winner (`fixes::committed`). The fighter's 12 Work/Unwind flips before an 18:00 shift are gone (she now leaves at 09:34 for a 419-min walk and waits at the pit: the distance is cause 1's `[feature]`).
2. **Shift commitment and pro-rata pay** (`shift_commit`, `shift_pro_rata` 0.5, `starving_hunger` 0.1): an in-shift work step is outbid only by an emergency (Flee, Fight, Arrest, Eat below hunger 0.1); a work step cut after half the shift is a worked shift paid pro rata (`economy::collect_wage_scaled`, arrears in full; guards keep the shift clock). The fighter's Sleep at 23:54 no longer costs the day.
3. **Committed sleep** (`sleep_commit`, `sleep_wake_energy` 0.99, `idle_sleep_energy` 0.9, `energy_brake` 0.5): a Sleep is held from its first tick (it was held only after 60 min) and runs to 0.99 energy (it ended at 0.9 and Idle planned the next minutes later: the cook's 1-6 min Sleeps from 01:22 to 06:00); Eat (above hunger 0.25), Earn and Unwind take `0.5 + energy` below energy 0.5. `idle_sleep_energy` stays 0.9: at 0.7 the nights went back to 90-min Rests (L1's bug). "Sleep -> X" interruptions on 42-44: 14.1k -> 2.7k a run.
4. **Atomic purchases** (master): `BuyFood` takes its units with its coins at the start; an aborted `BuyFood`, `EatOut`, `Enjoy` or `Gamble` keeps its purchase (the meal is eaten, the entry's fun pro rata, the bet stands), and think holds a paid step. Refunds in the diaries 6 -> 0; no-pin `homeless_825` 65 -> 0.
5. **The Unwind spot** (master; `spot_walk_cap_tiles` 22, `spot_lone_fun` 0.4, `spot_enemy_penalty` 0.5): `best_spot` keeps spots within ~45 min (else the nearest alone), adds the hour's company on the corner (strangers 0.1 each, to 0.5), subtracts 0.5 per Enemy body within 3 tiles; the free rung's score and a HangOut's fun scale with company (`spot_lone_fun` alone).
6. **Flee persists** (master; `avoid_spot_ticks` 1440, `flee_clear_ticks` 30): `FleeToHome` writes `Brain.avoid_spot` (the tile fled, for a day; `spot_ok` skips spots within 3 tiles of it) and cools Unwind and Socialise for 30 min on arrival.
7. **LOD tier dwell** (`lod_dwell`, `lod_dwell_ticks` 180): in `lod::assign`, a body within 3 h of its last tier change (to its plan's end, at most 6 h) or mid-purchase keeps its Coarse slot against an equal-priority newcomer, and one demoted within the dwell is not promoted back over an equal-priority incumbent; the counts are conserved (`Brain.lod_since`). No-pin `homeless_825` over days 19-26: tier changes 86 -> 0, LodDemotion aborts 43 -> 0; LodDemotion events on 42-44 -32 %.
8. **Statistical Sleep when Eat is futile** (`stat_sleep_futile_eat`): a hungry night hour whose Eat came to nothing sleeps (`lod::stat_sleep_night`). No-pin `worker_1340` / `worker_1686` energy-0.00 snapshots over days 19-26: 46 / 61 -> 0 / 0.
9. **The bout** (master): the fighters must be in the pit (a body inside, or Statistical) on a working shift; both remember `MemoryKind::Bout` (`deed_of` reads no deed: no rumour, Story, grudge or dread from a bout).
10. **Patrol** (master; `jail_affinity_cap` 0.3): Patrol and Arrest on the guard's workday, with a guard's rest day staggered by its index (`routine::workday_of`): the whole watch shared the city's rest day, so the gate alone left no guard at work one day in seven (arrests on that weekday over 42-53: 8,226 on main, the unpaid rest-day chase; 1,121 with the gate alone; 4,373 staggered, the other weekdays 4,119-4,790). The round starts at the stop nearest the guard (`plan::plan_for` rotates the drawn route; deviation: from where she stands, not from the Precinct). The desk/patrol duty is fixed per shift key as it comes due (`Job.duty_fixed`, `law::credit_guard_shifts`). A guard and a prisoner warm to at most 0.3 in the cells. Rest-day PatrolLegs in the diaries 5 -> 0.
11. **Absentees** (master; `noshow_fire_days` 7): a city-owned Feed's or Lab's staff get the `Workplace` distance (the Civic Wire's reporter was sent to the Hall: "Unreachable" all week); seven missed workdays (`fixes::missed_workdays`: shifts over and not worked since the last worked or the hire) are a no-show dismissal at midnight (`fixes::daily`; the vacancy is posted). The absentee's dole at two missed workdays (`noshow_dole_days`) was removed 2026-10-08 per roadmap addendum 17: a Job holder never draws the dole.
12. **The Clinic's counter** (master): `assets::seller_open` for a Clinic needs a doctor on a working shift at the counter (a body inside, or Statistical).
13. **Execs out of the labour pool** (master): `demography::pick_candidate` skips the exec set.
14. **Sweepers** (master): `districts::sanitation` deals each sweeper its home district's slot first (else yesterday's beat), the allocation unchanged; at midnight every Recycler sweeper is raised to `[budget] works_wage` 7.
15. **The leader** (master; `muster_walk_cap_ticks` 480): a started Lead holds against non-emergencies; `call_due` and `collect_due` stay due for a running Lead's life (past the Evening, past midnight); a raid's muster waits for its farthest member's walk to the Hideout (up to 8 h); Extort skips gang-mates at the door. Called events on 42-44: 102 -> 135.
16. **Dealing** (master; `deal_busy_from` 12:00, `deal_busy_to` 02:00): the buyer is no witness of its sale (`law::raise_crime_except`); a Deal shift starts only in the busy hours, and off them a dealer has no GangWork (the first cut let it fall through to its gang's shakedowns: Murders on 42-47 rose 215 -> 364 with the hours gate alone; with the fall-through closed, 255). Dealing witness lines on 42-44: 18.6k -> 13.4k.
17. **Gossip told-memory** (master; `told_cooldown_days` 3): `gossip::exchange` skips a deed this teller told this listener in the last 3 days (`World.told`, pruned at midnight).
18. **Hack gap guard** (master; `hack_gap_wealth` 0.9): no freelance run when the target's defence beats the attack by `[ice] flatline_gap` unless `U(wealth)` is 0.9 or more.
19. **Beg at a spot** (`beg_at_spot`): Beg's precondition takes the street within 2 tiles of a spot where someone else hangs out, at half cost there (else Scavenge always won). Beg actions in the twelve diaries: 0 -> 16.
20. **Beds and the pantry** (master): the Hideout is a bed whenever nearer than Home (no `bed_margin_tiles`; the Hotel keeps its margin, it costs); EatAtHome's pantry at plan time is the stock less the housemates' reservations.
21. **The Scavenge ledger** (master): `Flow::Scavenge` (in `flow_other`, as `Sanitation` was; the CSV is unchanged).
22. **The burial is the gravedigger's shift** (2026-10-08, with addendum 17): a Gravedigger's completed `BuryCorpse` on a workday whose shift is not yet worked counts as that day's shift and is paid as a shift's end (`fixes::burial_shift`, `economy::collect_wage`). Wages accrued only on the `TendGraves` shift, which a digger out burying missed every 1-5 days; with the dole gone, six of the seven starved on 42-44 were gravediggers.

Also: the shadow tool's "won back" counts `StreetDice` winnings (and street-dice stakes as gambling).

**Identity.** `--l2-off` against 56f3110's CLI (built from `git archive 56f3110`), 120 days, seeds 42 and 43: every report column but ticks/s and every event identical; `calibrate --agents 500` writes the identical table. Every item masked (`CITYSIM_L2FIX_OFF`, the master on) reproduces main's six-seed Murders, starvation and assaults exactly.

### Shadow before/after

`citysim-cli shadow --seed 42 --start-day 19 --days 3 --count 2 --pick guard,worker,cook,fighter,homeless,gang_member` on 56f3110 (before) and on the fix tree (after). The picks are drawn on day 19 of each city, so the after picks are partly other people; the third table re-shadows the before's twelve (`--agent`) on the fix tree. Per archetype (two diaries a row): hours per free day averaged, longest is the longest single Sleep, counts summed. "Work interrupted" counts `plan interrupted: Work ->`; paid counts Wage flows; rest days are the shift key's (staggered for guards after).

Before (56f3110):

| archetype | walk h/d | sleep h/d | longest h | work h/d | shifts / paid | Work interrupted | refunds | Bout/Won/Lost mem | arrests (rest day) | rest-day PatrolLeg | energy-0 h | Beg | scavenge booked Scavenge / Sanitation |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| cook | 4.7 | 4.4 | 5.6 | 4.5 | 3 / 3 | 2 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 0 / 0 |
| fighter | 7.2 | 5.7 | 7.2 | 3.5 | 4 / 0 | 11 | 0 | 0 | 0 (0) | 0 | 2 | 0 | 0 / 1 |
| gang member | 7.2 | 3.9 | 4.6 | 3.0 | 2 / 3 | 3 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 0 / 0 |
| guard | 8.5 | 4.8 | 6.8 | 6.2 | 4 / 4 | 10 | 0 | 0 | 0 (0) | 5 | 0 | 0 | 0 / 0 |
| homeless | 5.7 | 7.2 | 8.0 | 3.0 | 2 / 2 | 1 | 4 | 0 | 0 (0) | 0 | 0 | 0 | 0 / 1 |
| worker | 4.9 | 7.4 | 8.0 | 3.8 | 3 / 2 | 4 | 2 | 0 | 0 (0) | 0 | 2 | 0 | 0 / 8 |

After (the same command on the fix tree):

| archetype | walk h/d | sleep h/d | longest h | work h/d | shifts / paid | Work interrupted | refunds | Bout/Won/Lost mem | arrests (rest day) | rest-day PatrolLeg | energy-0 h | Beg | scavenge booked Scavenge / Sanitation |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| cook | 5.4 | 5.0 | 5.8 | 6.0 | 4 / 4 | 0 | 0 | 0 | 0 (0) | 0 | 0 | 5 | 0 / 0 |
| fighter | 4.3 | 4.7 | 7.8 | 4.0 | 4 / 2 | 0 | 0 | 2 | 0 (0) | 0 | 0 | 0 | 0 / 0 |
| gang member | 8.3 | 5.3 | 6.0 | 0.4 | 0 / 0 | 0 | 0 | 2 | 0 (0) | 0 | 0 | 0 | 1 / 0 |
| guard | 10.4 | 5.5 | 6.5 | 1.8 | 1 / 5 | 0 | 0 | 1 | 4 (0) | 0 | 0 | 0 | 1 / 0 |
| homeless | 9.7 | 4.8 | 6.7 | 3.5 | 2 / 3 | 0 | 0 | 1 | 0 (0) | 0 | 0 | 9 | 1 / 0 |
| worker | 5.7 | 5.5 | 6.0 | 5.9 | 4 / 4 | 0 | 0 | 0 | 0 (0) | 0 | 0 | 2 | 0 / 0 |

After, the before's twelve people (`--agent`):

| archetype | walk h/d | sleep h/d | longest h | work h/d | shifts / paid | Work interrupted | refunds | Bout/Won/Lost mem | arrests (rest day) | rest-day PatrolLeg | energy-0 h | Beg | scavenge booked Scavenge / Sanitation |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| cook | 4.7 | 5.2 | 6.0 | 4.5 | 3 / 3 | 0 | 0 | 1 | 0 (0) | 0 | 0 | 4 | 0 / 0 |
| fighter | 3.8 | 4.9 | 7.0 | 3.1 | 3 / 2 | 0 | 0 | 1 | 0 (0) | 0 | 0 | 2 | 1 / 0 |
| gang member | 7.4 | 4.5 | 7.3 | 2.8 | 2 / 2 | 2 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 3 / 0 |
| guard | 11.0 | 5.9 | 7.0 | 2.0 | 1 / 5 | 0 | 0 | 2 | 4 (0) | 0 | 0 | 0 | 0 / 0 |
| homeless | 6.0 | 5.7 | 7.4 | 3.0 | 2 / 2 | 0 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 1 / 0 |
| worker | 5.6 | 6.0 | 6.6 | 5.6 | 4 / 4 | 2 | 0 | 0 | 0 (0) | 0 | 0 | 8 | 0 / 0 |

Read with care: three days, two diaries a row. What moved by mechanism: Work interruptions 31 -> 0 (the picks) and 4 (the same twelve); refunds 6 -> 0; worked shifts are paid (fighters 4 / 0 -> 4 / 2: the second shift ends after the run); rest-day patrol legs 5 -> 0; Beg 0 -> 16; energy-0 hours 4 -> 0; scavenged coins booked as Scavenge. The worker and homeless rows sleep less per day because the before worker was out of work (1.6 h/d) and slept 8 h a day; the energy model sets a working day's sleep near 5-6 h (16 h awake at 0.042 an hour, refilled at 0.125 an hour). Guards walk more: patrol days are walking days (the round now starts near the guard; distance stays cause 1's `[feature]`). Not fixed here (features): the homeless dealer's 13 h a day between the Hideout and a Mid East Bar (cause 1), staff with no customers (cause 5), the idle menu (cause 8).

### The city (120 days)

56f3110 against the fix tree, seeds 42-44 (`run --days 120 --report --events`; Murders and assaults from the events, assaults include Murders; Coarse is `tier_coarse`, mean / max):

| seed | starved | thefts | Murders | assaults/day | employed d120 (mean) | wages / dole | gang income | Coarse mean / max |
|---|---|---|---|---|---|---|---|---|
| 42 main | 8 | 10,603 | 22 | 13.27 | 555 (515) | 292,377 / 678,612 = 0.43 | 27,886 | 151 / 155 |
| 42 fixes | 1 | 12,070 | 43 | 17.71 | 558 (527) | 304,097 / 670,620 = 0.45 | 45,868 | 151 / 156 |
| 43 main | 14 | 12,319 | 43 | 19.82 | 556 (520) | 293,347 / 659,540 = 0.44 | 44,049 | 151 / 157 |
| 43 fixes | 0 | 11,590 | 30 | 15.31 | 537 (512) | 297,971 / 681,388 = 0.44 | 39,812 | 151 / 165 |
| 44 main | 13 | 13,047 | 54 | 20.02 | 541 (506) | 285,749 / 670,608 = 0.43 | 42,698 | 151 / 157 |
| 44 fixes | 0 | 12,313 | 35 | 16.17 | 553 (521) | 305,401 / 667,152 = 0.46 | 45,990 | 151 / 156 |

Over 42-47: Murders 215 -> 251 (the M15 bound is 394), assaults/day 16.99 -> 16.43, starvation 60 -> 4, gang income 38.3k -> 42.0k a seed. Over twenty seeds (7-14, 42-53): Murders 795 -> 841, starvation 193 -> 15, evictions 191 -> 23 (an evictee joins a gang within 14 days at the same rate), episodes ended by the law 8 of ~41 -> 2 of ~33 (rest-day guards no longer chase; the M13 bullet's existence over 42-49 holds on seed 49). PlanAborted -20 %, `Work ->` interruptions -44 %, `Sleep ->` -81 % (42-44). Over a year (seed 42) assaults/day 24.5 -> 29.4 and the peak 30-day window 34.0 -> 43.3, with 62 more people alive on day 365 (starvation 25 -> 4); seed 43's peak 36.1.

### Gates

Trio green; `--test god` 30/30, `god_corps` 12/12, `god_districts` 11/11; `--test lod` 11/12 with `test_kitted_vs_unkitted_parity` red as on 56f3110 (identical numbers; its city is `calibration_city`, where the fixes are off); the serialized scenario suite 18/18 after four conversions, each with its per-seed data in the comment: M11 "Evicted on every seed of 42-44" -> on some seed of 42-47, and the eviction spiral's joins judged over 42-47 (>= 3: 4); M12 Crossfire on seed 42 -> on some seed of 42-47 (seed 42 read 1 on main); M14 the Research-built Lab -> on some seed of 42-53 (main built one on 47 alone of 42-47); the L2 year runs' assault windows -> the 42.7 band printed, 1.5x asserted. Unit-test touch-ups: `leisure::test_enjoy_charges_price_and_refunds_on_abort` runs the refund with the fixes off and asserts the kept purchase with them on; `jobs::test_l2_off_world_has_no_venues_and_same_first_day` strips `l2_fixes` from the pre-L2 config cut (it sits in `[life]`, above the cut).
