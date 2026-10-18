# Shadow pass V4: the NPC shadowing session

2026-10-17, main 9fc0c56 + the shadow-tool fixes (3053c7b `--population`, afb4b4c pins grow `max_full`, bbc1a47 the LOD note). Every pinnable archetype (48), two picks each, 7 days, three windows: the young 2k city, the mature 2k city, and the 20k island. 258 diaries, read by seven critics as lives. Every event is a seeded game abstraction.

```
A=gang_member,gang_leader,ripperdoc,homeless,ceo,exec,guard,worker,runner,purist,reporter,dealer,cook,club_staff,fighter,fabber,sweeper,worker_friday,super,bouncer,dancer,night_shift,orderly,camp_warden,fixer,gun,sick,buyer,joytoy,street_worker,pimp,vendor,day_labourer,cabbie,courier,office_worker,arms_dealer,forger,elder,loader,builder,foreman,madam,companion,black_doc,cultist,teacher,scav
citysim-cli shadow --seed 43 --start-day 26 --days 7 --count 2 --pick $A --out target/shadow_v4/k2_s43_d26_full
citysim-cli shadow --seed 44 --start-day 90 --days 7 --count 2 --pick $A --out target/shadow_v4/k2_s44_d90_full
citysim-cli shadow --population 20000 --seed 42 --start-day 20 --days 7 --count 2 --pick $A --out target/shadow_v4/k20_s42_d20
```

The critiques: `docs/shadow_v4/street.md` (homeless, sick, elder, vendor, street worker, sweeper, camp warden), `gangs.md` (member, leader, Purist, cultist, scav, dealer, runner), `crime.md` (arms dealer, forger, fixer, gun, buyer, pimp, black doc), `corp.md` (CEO, exec, office worker, guard, reporter), `labour.md` (worker, night shift, fabber, loader, builder, foreman, courier, cabbie), `vice.md` (club staff, bouncer, dancer, madam, companion, joytoy, fighter, cook), `care.md` (ripperdoc, orderly, teacher, super).

**Method note.** The first 2k runs pinned ~85 picks against `[lod] max_full = 50`, so the overflow lived at Coarse (44/81 and 62/85 diaries); the tool now grows the cap by the pick count and the 2k runs were regenerated (the `_full` dirs; at most one Coarse row a diary). The critics retracted every finding that had only Coarse evidence: corpse looting by the lawful, the 237-food pocket, the 34-h co-worker marriage, the one-day acting leader, the Chat through a shakedown (one 20k case survives), the same-minute sickness rehire. More Full agents also make a different world, so the 2k stories differ from the first draft.

## Verdict

**Alive** where a gang, a debt, the law or the hit market touches a life: the gun (the best of the pass), the runner at day 90 (a precinct guard selling stolen data to Militech), Nadia the cook (coerces a witness, finds a faith, joins the Choir), the vendors near home, gang members' feuds and hustles, gang leaders as drama, the city guard, the builder at the Yard, CEOs as targets (a contract beating, a conspiracy term, a night abduction). By day 90 the gangs have politics: a parley, bribes to the captain, hits on rival leaders, coerced witnesses.

**Thin** for nearly every trade: the work is a timer (ClerkWork, FabWork) with no counterpart, and the hours after it are Rest, a far corner, or a venue. **Not alive as the trade:** cabbie, foreman, super, madam, bouncer, fixer, pimp, arms dealer, camp warden, street worker, scav (no harvest in the week), dealer (jailed or 0-2 sales). **Absent:** the day labourer (a pick bug) and the black doc (unreachable by construction).

**2k vs 20k.** 20k fixes the commutes of most trades (home and post share a district), but the city-wide things stretch: guards travel 56-88 h a week from the single Precinct, a sweeper walks 901 min to his Hideout bed, the 20k gangs are small numbered clones raiding alone after 8-h marches, and a witnessed crime becomes an arrest at half the 2k rate (35 % of crime-trade hours jailed on 2k vs < 1 % on 20k). **Day 26 vs 90:** the young city jails more (10-day Assault terms for victims who hit their robber); the mature city has bigger, richer, more political gangs.

## V3's gaps

| V3 gap | Now |
|---|---|
| 1 Idle hours after the shift | unchanged (2k companions 74-94 h idle a week; office clerks 59 h); and a new cause found for the jobless (bug 1 below) |
| 2 The work is a counter | unchanged, for every trade (feature 1) |
| 3 Distance | better at 20k for most trades, unchanged at 2k; city-wide roles worse at 20k |
| 4 Ages vs trades | unchanged (15 of 50 labourers over 60; fighters 61-67; a 19-year-old teacher) |
| 5 Rare trades absent | day labourer: a pick bug; black doc: unreachable; pimp and street worker: titles without the trade |
| 6 A killer stays poor | half: the stipend and tribute are paid, invisibly (bug 14) |
| 7 The anti-chrome cult harvests chrome | worse and more general: four critics (bug 13) |
| 8 The chip's cost invisible | unchanged |
| 9 Faith without worship | unchanged: 0 Preach in 6 cultist diaries (bug 13) |
| 10 Elders without kin | unchanged |
| 11 The robbed do not arm | not seen either way; the robbed now fight instead (bug 3) |

## Bugs, ranked by how many lives they distort

Each names the likely cause the critics found; a fix needs a failing test first.

1. **The jobless poor stop earning once they hold a meal's coins** (street; idle 46-65 h a week). `utility/goals.rs::already_satisfied`, the Earn arm, with the dole at 0. Fix: Earn is satisfied by days of food or a wage, not one meal.
2. **The night isn't the night** (labour, care, crime). Day workers sleep 19:00-23:30 and stand on a corner at 03:00: `leisure.rs::considerations` has no Night gate (Socialise has one). Grocery runs out of bed 00-05h: the grocery term is damped only on work nights. A 68-year-old loops Rest for four days without sleeping: Rest gives +0.1 energy per 90 min (`exec/actions.rs`).
3. **The Fight goal has no proportion** (care, corp, labour, gangs, vice). A flat +0.2 for a robber in reach and no lawfulness, witness, stake, age or relative-strength term: victims of a 1-5 c shakedown assault the extortionist and get 10 days (5 of 24 care picks); old workers wake to fight; a grudge fight finishes a man lying downed (`fresh_hostile_near` doesn't skip `world.wounded`), a Murder. The robber's friends then swear against the victim (retaliation filed as plain Assault).
4. **The law's sentence mix** (care, labour, gangs, crime). Fare evasion is the top sentence (42 vs 33 Assault), arrested at a desk or a bar 22 h later: `law.rs::jail_suspect` fines only when the Precinct is full. A 0-coin shakedown gets 6 days. Fix: a fine whenever the suspect can pay; scale Shakedown by the take. Then measure (see 22).
5. **Execs live street lives** (corp, street). `life.rs::hq_of` has no `Hq` in its ranking and sorts by distance first (CEOs hold Meetings at Vat Farms; Arasaka's 20k CEO lives in the Sump). `streetwork::openings` and `gang::recruit_gang` lack an `is_exec` filter (a 68-year-old exec on a solicitation pitch); the exec's Earn falls through to StealVehicle; `ownership::exec_wages` skips `leave_for_promotion` (an exec keeps her Guard post).
6. **The wrong person in the post** (street, crime). Pitch openings go to any adult without a Job, savings favoured (3 of 4 street workers aged 64-68); the pitch spot is the district's first free one, not the nearest home (47-168 min walks); no skill floor on a hire (a knowledge-0.00 homeless Dreg hired as Teacher); a "no need of the wage" quitter is rehired by another employer the same midnight (`quit_blocks` is per-employer).
7. **Tips** (vice). Every tip goes to the lowest-id worker on duty (`vice.rs::tipped_on_duty`: 56 c vs 0 c on the same shifts); Clubs fall through `_ => return` in `vice.rs::on_visit`, so dancers are never tipped.
8. **Gang leadership by loyalty, not tenure** (care, vice). `gang.rs::leader_scores` counts days/100: a clinic owner leads the Choir 3 days after joining. And `succession.rs::decide` hands the gang to a deputy for a 1-day fare sentence (only Vagrancy is exempt).
9. **Raids and shakedowns** (gangs, care). Lone raids, 1 against 6, after `raid.rs::wait_for_crew` gives up (call it off under `raid_min_members`); 9 of 17 shakedowns take 0 coins and still make Enemies, memories and claims; extortion takes from every occupant of a Home, the extorter's new Friend included.
10. **Social edges ignore what just happened** (labour, corp, care, street, vice). Jail drifts every co-held pair positive, enemies included (−1.00 → −0.10 in 13 h; `social.rs::colocation`); a lost arrest scuffle gossips as "Assaulted by <guard>" (`memory::deed_of`); a Chat completes after the partner robs you (`exec/actions.rs` checks only proximity); an off-screen killing bound to a Full agent asleep across town (`bind.rs::candidates_in`; one Full case, more at Coarse); a woman strips her father-in-law's body before her wife buries him (`chrome.rs::close_to` ignores a spouse's kin).
11. **Corp guards police the city** (corp, street). The Arrest goal has no city-guard gate; `law::private_patrol_route` samples 5 clients city-wide (a 382-min first leg, 88 h of travel at 20k). `gang.rs::holes_up_at` sends a member to the Hideout bed at any distance (901 min).
12. **A courier's round inside a Club loses the afternoon shift** (labour). `goap/world_state.rs::of_building` treats any Club, Noodle Bar or Security Office as the agent's Workplace; ClerkWork then fails PreconditionLost twice and Work cools 2 h.
13. **Creeds don't bind** (gangs, labour, crime, care). `assets.rs::chrome_max_tier` / `shop_choice` sell chrome to Purists and cult members, who are expelled the same minute; `gray.rs::assign_types` types the cult as scavs; `leisure.rs::preacher` needs `Order::Expand`, so cults never preach.
14. **The gang's pay is invisible** (gangs, labour). The stipend and the territory tribute are wallet writes with no `Flow` (`gang.rs::daily_economy`), so Collect reads "0 to 0 members" on takes of 40 and 229.
15. **Contracts** (crime). `contracts.rs::render_for` always renders a Tag as Ledger, even for a pinned taker (261 c paid while resting); `fulfil` refunds a locate's leftover to the buyer when others' sightings fill the cap (3 days of stake-out, 0 c); a man hearing of a hit on himself weighs it 0.25 < `grudge_min` 0.35 and never reacts; a locate's StakeOut runs in 2-minute loops.
16. **The arms trade and the black clinic can't start** (crime). A sale needs an armoury surplus that never forms on 0-180 c treasuries (0 weapon sales in 35 dealer-days); `gray::build_clinics` needs 700 c and its posts hire only struck-off docs.
17. **Bouts** (vice). The same two fittest fighters every night, the rest benched; bets paid 0 in 8 of 8.
18. **Staff leisure at their own workplace**, then a wait of up to 10 h for the shift (vice; `leisure.rs::paid_candidates`, `WAIT_AT_WORK_MAX`).
19. **Work has no hunger term** (street): a sweeper works at hunger 0.11 with 15 c in hand, 26 h after his last meal.
20. **At 20k, low-urgency goals starve in the plan queue** for 1-6 h while the body stands (labour; `plan.rs::enqueue` orders by urgency alone, no aging).
21. **One 20k assault runs in 11 outlets** (gangs): the numbered corp clones each publish it.
22. **20k turns witnessed crimes into arrests at half the 2k rate** (crime; reports 0.18 vs 0.32-0.40 a diary, arrests 0.30 vs ~0.6, witnessed the same). Untraced; the single Precinct (feature 6) is the suspect. With 4 and the Full-vs-Statistical arrest rate, this wants a probe before a fix.
23. **The shadow tool** (street, crime). The day labourer is picked at 00:00, when no day post is held; `sick` takes cancer at stage 0.00; the vendor pick takes a pitch closed that midnight; the pimp pick takes a jailed pimp; the arms-dealer pick tests stock, not surplus; `flow_kind` buckets money by the running action (midnight sales read as a daily pass).

## Features, ranked

1. **The work has a counterpart** (all seven critics). A patient queue at the Clinic, a pupil roster at the School, a fare for the cab, a crate for the loader, a client for the companion and the street worker, a door for the bouncer, a tenant at the super's stop, an inmate for the warden, staff and a cut for the madam and the pimp, a corner and regulars for the dealer, attendees and a decision in the CEO's Meeting, a beat for the reporter, a broker's visit for the fixer.
2. **The evening and the rest day** (vice, corp, street, labour). Beyond one venue and a corner: home, the spouse, the kids, a friend's place; leave an empty spot after an hour (120 of 369 HangOuts empty at 2k, 39 % at 20k).
3. **Households live together** (care, gangs, street, labour). 14 of 17 married picks never meet their spouse in 7 days; a new mother is on a corner two nights after the birth; a household's business isn't shared; a taken child leaves only a grudge.
4. **Ages and professions** (vice, labour, care, corp). Retirement or a frailty exit, age bands per trade, tenure that predates day 0, a Profession that survives a layoff, a foreman by experience.
5. **Leaders lead** (gangs). Lieutenants take the risky orders; the leader's cut off the top; 7 of 8 people running a gang ended the week jailed, downed or dead.
6. **The city's scale at 20k** (corp, care, street). District Precincts and beats near home; the super's round to near blocks or counted as work; Spire homes for 20k CEOs.
7. **Wealth has a body** (corp). The Flyer flown, an escort after a hit, no night walks through the Sump, bail and a lawyer, an acting exec.
8. **Gossip beyond crime** (gangs, labour). Wages, layoffs, weddings, births.
9. **Owners run what they own** (care, crime). An owner-operator path; owner pay from profit.
10. **Sickness and age change behaviour** (street, gangs). Seeking care, sick days, age and illness in a fight's strength; elders taken in or visited.
11. **Aftermath** (crime, care). A witnessed manslaughter reported; a parent's answer to a taken child.
12. **The chip's cost shown** (vice): the debt behind the contract, no re-hire the night of a breakdown.

## The sweep (merged 2026-10-17)

`v4fix` ran the sweep, one solo coder working test-first, from ce5afe6 to 4f5b88e. It merged as 2712b4a.
- **Fixed:** 21 of the 23 bugs. The tests are in `citysim/tests/shadow_v4.rs`.
- **Skipped:** bug 16. Gang treasuries block the arms trade and the black clinic, so the fix is a feature (credit, a back-room pitch), not a cut in costs.
- **Partly fixed:** bug 22, the rest of which goes to LIVES_AT_WORK D11.
- **Found along the way:** a save-determinism bug. A wronged spouse's edge turned Enemy while `World::spouses` kept the pair, so a loaded game diverged 98 ticks later. The test is `test_wronged_spouse_index_survives_a_load`.

**Before and after** (core_sanity events and the CLI, 120 days):

| Measure | ce5afe6 | 4f5b88e |
|---|---|---|
| Murders, seeds 42/43/44 | 287 / 263 / 256 (806) | 113 / 91 / 177 (381) |
| Violent deaths, seeds 42-47 | 1,984 | 770 |
| Population at day 120 | 1,688 / 1,721 / 1,715 | 1,904 / 1,943 / 1,820 |
| Employed at day 120 | 1,526 / 1,595 / 1,545 | 1,745 / 1,709 / 1,701 |
| Starvation | 0 / 0 / 1 | 0 / 0 / 0 |
| Gang-vs-gang feud days, 42-44 | 267 / 317 / 204 | 231 / 106 / 18 (noisy) |

- The fixes alone cut Murders by about 47%: 78e1902, the tree before the stat-table regen, read 148 / 166 / 110.
- The regen adds per-seed noise.
- The city is about half as murderous. Under the brutality-via-pressure rule, the tone comes back through pressure, not by reverting fixes. That is Dylan's call.

**Bullets demoted to printed FINDINGs.** The bugs were feeding their counts, and each is still unit-tested:

| Bullet | ce5afe6, seeds 42-47 | Now |
|---|---|---|
| Parley successes | [1,2,2,0,0,1] (37 attempts) | 42-44 [0,1,0]; 9 attempts after batch 2 |
| Purist expulsions | [21,15,12,24,21,12] | 42-44 [0,0,0] |
| Organs exported | [6,0,9,8,0,0] | 42-44 [0,0,0] |
| Sanctioned | [4,3,4,3,1,4] | 42-44 [0,0,0] |

Parley lost its input, not its mechanism. Gang-days with a stronger rival gang in a vendetta went from 92 to 0, because one-sided fights and empty shakedowns no longer breed feuds.

**Law probe** (`citysim/tests/law_probe.rs`, `#[ignore]`d). A witnessed act counts as sentenced if a Report follows within 1 day and a sentence within 3:

| City | ce5afe6 | after batches 1-4 |
|---|---|---|
| 2k overall | 0.70 / 0.73 / 0.70 | 0.71 / 0.75 / 0.71 |
| 20k overall | 0.46 | 0.64 |
| 20k Coarse | 0.36 (0.24 reported) | 0.56 (0.44 reported) |
| 20k Full / Stat | 0.70 / 0.78 | 0.73 / 0.83 |

The probe ruled out Precinct capacity (no one went unpunished for a full jail) and the plan queue. What is left is guard throughput through one Precinct at 20k.

On 2k seed 43, days 20-40, the sentence mix changed:
- **ce5afe6:** 516 fare nights, 130 Theft, 61 Assault.
- **Now:** 330 fines, 240 fare nights (those who cannot pay), 82 Theft, 53 Assault, 53 Shakedown.

**The re-shadow** (`target/shadow_v4_after/`, the same picks) reads:
- **Day-worker leisure steps, 00:00-06:00:** 638 → 126. The 126 left are picks with no day shift.
- **Small-hours grocery runs:** 87 → 0.
- **Fines:** 6 → 107.
- **CEO meetings at the HQ:** 0 → 30, against 6 elsewhere.
- **Dancer tips:** 0 → 60.
- **Idle hours a week, jobless street picks:** 29.4 → 24.0.

**Left open:**
1. **LOD parity, actor share by lawfulness.**
   - law0: Full 0.915, Stat 0.746. law1: Full 0.068, Stat 0.202.
   - Bug 3's binder mirror is in, but it moves the shares by only about 0.015.
   - The likely fix is to have `calibrate` learn the binder's per-lawfulness weights. `[bind] lawfulness_power` is not retuned.
2. **Bug 16:** credit or a back-room pitch.
3. **Bug 22's rest:** D11, district Stations.
4. **Shakedown terms scaled by the take:** needs a save field.
5. **Gang feuds and Parley need a real feed:** rival-turf shakedowns, raids that land.

**The review round** (`/code-review high` on ce5afe6..2712b4a, 10 findings, all real). The fixes are 15eb256, b68b5b4, 0fb3b6a and 1552009, with tests:
- **Bug 11's beat check was a no-op.** It now lives in `law::chaseable`, on the real pursuit path.
- **The succession gap could go negative.** The runner-up now comes from the heir's own tenure class.
- **The binder judged the wrong day.** It now excludes an agent who was Full on the deed day (`trace_flags::FULL`), not one Full at bind time.
- **The arrest scuffle left grudges.** It is now remembered as WasArrested up front, so no grudge and no Enemy edge.
- **Fines skipped the booking.** A fined arrest still interrogates, settles contracts, spreads the rumour and counts (`law::booked`).
- **Work's hunger term ignored reach.** It now needs a meal in reach.
- **An office could be a Home or a Lot.** It no longer can.
- **The plan-queue aging scan was O(n).** `World::plan_queue_age` now indexes it.
- **Cleanups:** `law::is_petty`, and one exec helper.

`god_flood_guns` now asserts the scenario's mechanism: the god-placed crate is drawn down (7 crate weapons carried, strict > 0). Both earlier forms of the assert depended on the trajectory.

After the round, Murders on 42-44 read 94 / 61 / 112 (267), and core_20k reads 89; the two moved opposite ways, which looks like noise. The merge set is green on main at 1552009. LOD parity law0/law1 is still open: Full 0.930 vs Stat 0.757.

The re-shadow diaries were in the `v4fix` worktree's `target/shadow_v4_after/` and went with the worktree on 2026-10-18 (the process review removed it before copying them out; `OPEN-095`). They are deterministic: regenerate them on `2712b4a` with `citysim-cli shadow` and the same picks if a before/after read is wanted.
