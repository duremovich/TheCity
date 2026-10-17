# Shadow V4: the corp tower and the law

This critique covers the CEO, exec, office worker, guard and reporter: six diaries per archetype, two per run.

The runs:
- `k2_s43_d26_full`: the 2k city, days 26-33.
- `k2_s44_d90_full`: the 2k city, days 90-97.
- `k20_s42_d20`: the 20k city, days 20-27.

The two 2k runs were regenerated after the first pass, with every pin at Full LOD. They pick the same people, but the weeks diverged from the first draft: different crimes, different deaths. Citations use the form `run/file dDD HH:MM`. Every event is a seeded game abstraction.

## CEO (the richest corps' execs)

**Picks**

| Who | Age | Corp (treasury) | Home | Run |
|---|---|---|---|---|
| **Zara Hawthorne** | 39 | Vatra (26k c) | Spire Block#63, Flyer T1 | 2k d26 |
| **Rafe Quarry** | 51 | Nutrix (19.7k c) | Spire Block#79 | 2k d26 |
| **Liesl Dunmore** | 61 | Vatra | Spire | 2k d90 |
| **Nico Novak** | 37 | Militech (3.4k c) | Spire | 2k d90 |
| **Ilse Tanaka** | 32 | Nutrix 9 (36.5k c) | Block#5185 in **Mid 8** | 20k |
| **Dex Stroud** | 51 | Arasaka (54k c) | **Sump 4**, moved to Sump 3 | 20k |

**The week**

- Every workday is one Meeting of 450-570 minutes, alone.
- The Meeting is never held at the corp's HQ:
  - Rafe meets at Vat Farm#474 (`k2_s43_d26_full/ceo_1826 d26 08:32`).
  - Zara (Vatra) meets at Vat Farm#406 (`k2_s43_d26_full/ceo_1529 d26 08:33`), while Vatra's clerks sit at Corp HQ#452 in the Spire (`k2_s43_d26_full/office_worker_1045 d26 09:02`).
  - Liesl meets at Vat Farm#472 (`k2_s44_d90_full/ceo_1713 d91 08:09`).
  - Nico (Militech) meets at Security Office#425 (`k2_s44_d90_full/ceo_2428 d91 08:39`); Militech's HQ#440 is in the Spire.
  - On 20k, Ilse meets at Vat Farm#4099 and Dex at Security Office#4215 (`k20_s42_d20/ceo_12826 d21 08:43`).
- Evenings go to a bar, the Spire Lounge, a braindance or a club.
- Idle time runs 27-63 h a week; Liesl idles 62.7 h.
- ExecWage is 280 c a week at the big corps, 70-164 c at the d90 ones.

**Events in the week**

- **Rafe** is beaten under a contract Vatra paid for. He buys a Weapon T1 at a bar (`ceo_1826 d27 20:22`).
- On `d30 00:26` Rafe sets out to Donate at a Chapel in Sump Central. The Chapel is full and he re-routes: a 265-minute walk through the night. At 05:56 he heads for his Meeting. He is abducted in Sump Central (`d30 08:05`), two implants are ripped out, and he dies (`d30 10:22`). He was a CEO with 475 c in his purse and an owned hit against him, walking the Sump alone at night.
- **Zara** is charged with Conspiracy in Rafe's beating (`ceo_1529 d27 00:00`). She pays a 60 c bribe, is arrested at 04:36 on d28 and gets 23 days. That is 114 h of the week in jail; she has 517 c, and Vatra has no head for that time.
- **Nico** is jailed one day for Unlicensed Arms (`ceo_2428 d92 10:26`). On d96 02:28 he attacks Sebastian Penhallow, the man laid to robbing him 24 days earlier.
- Stories run that name **Liesl** as a contract target ("Ninefold hired Liesl Dunmore (paid by Nutrix)", `ceo_1713 d93 00:00`).

**Verdict: thin.** Real things happen around them: a hit, a murder, a conspiracy charge, a grudge settled. Their own day is still nine hours alone at a farm, then a bar.

**V2 gaps, rechecked**

- Meeting with content: unchanged.
- Meeting at the wrong building: worse, now that HQs exist.
- The commute flap: fixed.
- Vehicle and escort: unchanged, and now lethal (Rafe's walk). Zara's Flyer is never flown.
- Friends at +1.00 who never show up: unchanged. Liesl's top four friends, all at +1.00, appear 1-2 times each in the week.

**2k vs 20k**

- On 2k the CEOs live in the Spire. On 20k they do not.
- Dex, the head of Arasaka, "moved nearer work" into Block#1691 in Sump 3 (`k20_s42_d20/ceo_12826 d20 00:00`). The "work" is the Security Office that `hq_of` picked.
- That night he walks 172 minutes to sleep in a church shelter (`d20 00:16`).

## Exec (the remaining corps' heads)

**Picks**

| Who | Age | Corp (treasury) | Run |
|---|---|---|---|
| **Greta Bartlett** | 38 | Habitat (2.7k c; owns only Blocks) | 2k d26 |
| **Juniper Park** | 50 | Park Holdings (-54 c), homeless | 2k d26 |
| **Agathe Zhou** | 69 | Arasaka | 2k d90 |
| **Tamsin Delacroix** | 60 | Zetatech (213 c), *also a city Guard* | 2k d90 |
| **Fenn Kettering** | 55 | Stackwell 7 (Hunker) | 20k |
| **Mako Upton** | 58 | Habitat 8 (Hunker) | 20k |

All draw 10 c a day.

**The weeks: street lives with a corp title**

- **Greta** opens a Street Vendor pitch in the Spire (`k2_s43_d26_full/exec_2175 d26 00:00`) and works it 51 times. Her 50 h of "other" is vending. Habitat owns no building `hq_of` ranks, so she has no office at all. On d31 a robbery from day 0 is laid to her.
- **Juniper** joins the Ninefold gang (`exec_979 d27 20:43`). Her corp goes bankrupt at midnight. She is abducted in Sump East (`d28 05:01`), harvested and dead by 08:15.
- **Agathe** beats the man laid to her robbery (`k2_s44_d90_full/exec_1589 d90 09:14`). She keeps her Meetings at Security Office#424. Then her Earn goal steals motorcycles to fence (`d91 17:40`, `d93 17:40`). She is arrested at 01:41 on d94 and gets 6 days for Grand Theft.
- **Tamsin** works a full GuardJail shift at the Precinct (`exec_911 d90 09:02`) and is never at Zetatech. A locate contract runs on her. She is abducted in the Spire (`d91 05:21`), has five implants ripped out and dies (`d91 10:01`).
- **Fenn** joins The Iron Knives (`k20_s42_d20/exec_16096 d22 11:30`), deals at a bar for 4 h, opens a pitch, and gets 23 days for Conspiracy in a gang killing (`d24 06:09`).

**Verdict: alive, but not as execs.** These are the most eventful lives in the tower, because a small corp's head is a 10-c-a-day adult whom every street system treats as jobless. Three of the four 2k execs end the week dead or jailed.

**V2 "exec hired as a worker":** fixed in `pick_candidate`, but execs still leak into street work and gangs through other doors (gap 3).

## Office worker

**Picks**

| Who | Age | Corp | Home and HQ | Run |
|---|---|---|---|---|
| **Greta Haas** | 54 | Vatra | Spire Block#50 | 2k d26 |
| **Seraphine Marlow** | 74 | Greenline | home in Mid East, HQ in Spire | 2k d26 |
| **Fiora Oakes** | 31 | Vatra | homeless; evicted by Vatra | 2k d90 |
| **Ursula Coldwater** | 68 | Militech | Spire | 2k d90 |
| **Wisp Reyes** | 36 | Nutrix 4 | home and HQ in Mid 3 | 20k |
| **Zane Tanaka** | 51 | Zetatech 4 | home and HQ in Mid 4 | 20k |

**The week.** Six 9-hour ClerkWork shifts, 8 c a day.

- Greta Haas idles 59 h a week.
- Seraphine, at 74, commutes 45 h and buys a Motorcycle T1 on finance (`k2_s43_d26_full/office_worker_2355 d27 11:58`).
- Fiora sleeps in the Church of the Open Circuit in Sump West (`k2_s44_d90_full/office_worker_1365 d90 18:42`). She works every shift at the HQ that evicted her, and is housed in a Spire Block on d96.
- Ursula is shaken down for 5 c at her Block (`office_worker_2457 d91 21:48`). Ninety minutes later she attacks the extortionist in the street (`d91 23:42`) and is jailed for 10 days (`d94 01:44`).
- On 20k the commutes are short (25-41 h of travel a week), and coworkers become friends at the HQ: Viktor Mbeki goes from Acquaintance to Friend +0.60 within one shift (`k20_s42_d20/office_worker_25059 d21 08:49-17:19`).

**Verdict: thin on 2k, nearly alive on 20k.**

**V3 gaps, rechecked.** The idle evening is unchanged on 2k. Seeded ages are unchanged: clerks of 68 and 74.

## Guard

**Picks**

| Who | Age | Post | Run |
|---|---|---|---|
| **Thea Greaves** | 56 | city guard | 2k d26 |
| **Clemence Ostrowski** | 35 | city guard | 2k d26 |
| **Hollis Eastwick** | 18 | city guard, night desk | 2k d90 |
| **Volt Yardley** | 54 | city guard, night | 20k |
| **Nadia Lindqvist** | 47 | Arasaka security | 2k d90 |
| **Osric Osgood** | 33 | Arasaka 10 security | 20k |

**City guards are alive.**

- Desk days and patrol days alternate.
- The arrest count is real: Thea makes 6 escorts in the week, Clemence 7, Hollis 4.
- Suspects fight back: two fight Clemence off (`k2_s43_d26_full/guard_2261 d28 12:32`, `d30 15:52`). She cuffs Ines Northcott on the second try (`d30 16:03`).
- Clemence arrests Jasper Kettering, one of the reporter picks (`d26 10:35`).
- Witness events fire on patrol, and contact with prisoners stays at Acquaintance.

**Distance dominates their weeks.**

- City guards travel 56-62 h a week on 2k.
- On 20k, Volt (home in Sump 6, the city's one Precinct in Civic) travels 75 h. He sleeps in Capsule Hotels, and his early-morning bed hunts fail with `NoSuchPlace` (`k20_s42_d20/guard_22806 d22 06:06`).

**Corp guards are not corp guards.**

- Nadia (Arasaka, posted to Security Office#424) never stands at her office. She makes **13 escorts to the city Precinct in a week**, twice any city guard (`k2_s44_d90_full/guard_2241 d90 09:57, 13:13, 16:11, d91 10:59 ...`).
- Nadia's week: 83 h of travel, 6 h of "work", and nights in a Sump West church shelter 210 minutes from her beat (`d90 20:51`).
- Osric's first patrol leg on 20k is a 382-minute walk (`k20_s42_d20/guard_7552 d20 00:00`). He arrests a city warrant and escorts the suspect to the Precinct (`d20 21:22-21:32`).

**Verdict.** City guard: alive, but a commuter. Corp guard: a free second police force, not a corp employee.

**V2 guard gaps, rechecked**

- Fixed: G2 (patrol on the day off), G3 (duty flip), G6 (arrests not reading as arrests), G8 (affinity with prisoners).
- Improved: G1 (patrol encounters) and G7 (crime volume).
- G4 (distance): unchanged on 2k, worse on 20k.
- New: arrested suspects gossip that the guard assaulted them. Knox Inchbald, arrested by Thea at `d29 12:40`, spreads "Assaulted by Thea Greaves" (`k2_s43_d26_full/guard_1303 d31 22:23`).

## Reporter

**Picks**

| Who | Age | Feed | Run |
|---|---|---|---|
| **Ilse Park** and **Jasper Kettering** | 52 and 41; spouses in Block#111 | Feed#495 (Nutrix), Mid West | 2k d26 |
| **Ulysses Osgood** | 52 | Feed#495 | 2k d90 |
| **Benedikt Sato** | 74 | Feed#495 | 2k d90 |
| **Undine Vance** | 43 | Mid 1 Feed | 20k |
| **Rook Fairweather** | 66 | Mid 5 Feed | 20k |

**The week**

- All of them live 20-40 minutes from their Feed, so the V2 hire fix landed.
- They work six 9-hour ClerkWork shifts at 5 c a day, the lowest pay in the tower. Evenings are 15-40 hang-outs and street dice.
- On d26 at 06:40, Alaric Okafor shakes the couple down for **1 c** at their Block. Jasper (lawfulness 0.71, courage 0.25) wakes into a Fight goal and attacks him; so does Ilse (`k2_s43_d26_full/reporter_1466 d26 06:43-06:58`). Three neighbours witness it. Both are arrested by 10:36 and get 10 days each, so both diaries are 156 h of jail.
- The d90 and 20k reporters witness crimes on shift, and none of it becomes their story.
- Their only part in the news happens off screen:
  - On 20k, Rook is named the "betrayed" in four stories that run on d20 (`k20_s42_d20/reporter_18930 d20 00:00`).
  - Benedikt is "betrayed" by Agathe Zhou and Nico Novak (`k2_s44_d90_full/reporter_2014 d91, d93, d95`).
  - What happened: `captives::fabricate`'s Deceive move against the Feed's sharpest reporter failed (the reporter saw through the planted lie), and a rival corp paid to run the failure. Neither reporter ever meets the exec who tried.

**Verdict: thin.** A sociable desk clerk; the newsroom's work happens in `news::daily`.

**V2 gaps, rechecked.** Hire distance, the unowned Feed and the absentee hole are fixed. "No beat" is unchanged.

## Interactions

- **Read like people:** grudges after robberies; arrest fights that leave enemies; a CEO who buys a gun after a beating; the chain from a corp hit to its CEO in jail; Nico settling a 24-day-old robbery; coworkers who become friends at a shared desk (20k).
- **Do not read like people:**
  - Victims of a 1-5 c shakedown attack the extortionist within minutes and serve 10 days, while the extortionist walks: the reporters, Ursula.
  - A CEO never meets one of his employees.
  - Friends at +1.00 are absent all week.
  - Arrests are gossiped as assaults.

## Gaps, ranked by how much they distort a life

1. `[bug]` **The exec's office is the nearest farm, or nothing.**
   - Cause: `systems/life.rs::hq_of`. Its RANK list has no `BuildingKind::Hq` (and no Home, so Habitat has no office), and it sorts by distance before rank.
   - Effect: the CEO's 8 h a day lands at a Vat Farm or Security Office. The same pick drives the relocation that put Arasaka's head in the Sump, and leaves a housing corp's exec vending in the street.
   - Fix: rank `Hq` first, sort by `(rank, distance)`, and give a corp with no ranked building a desk at any of its buildings.
2. `[feature]` **The Meeting has no one in it** (V2, unchanged).
   - Hold the Meeting at the HQ with the corp's management and clerks present, giving co-presence edges and Met memories.
   - Read the corp brain's day out as the exec's decision: the order, the contracts posted (`ContractPosted` already names the corp at midnight).
3. `[bug]` **Execs leak into street life.**
   - `systems/streetwork.rs::openings` filters on `has::<Job>` but not `founding::is_exec`. That is how Greta (2k) and Fenn (20k) opened pitches.
   - `systems/gang.rs::recruit_gang`, the JoinGang gate, has no exec filter (Juniper, Fenn).
   - The exec's Earn goal falls through to `StealVehicle` (Agathe), because pay of 10 c a day reads as poverty.
   - `systems/ownership.rs::exec_wages` calls `classpay::leave_for_promotion` only for the `ranked` pick, never for the `replacement_exec` fallback, so a promoted employee keeps their Job. `life::exec_day_pending` then skips any Job holder, which is how Tamsin works as a Guard and never holds office.
   - Fix: add `!is_exec` in openings and recruit_gang; call `leave_for_promotion` for whichever pick wins; and either keep execs out of street Earn or treat them as a separate class.
4. `[bug]` **Victims retaliate out of all proportion.**
   - Cause: `utility/goals.rs` `GoalKind::Fight`. A shakedown flips the edge to Enemy -1.00 whatever the stake, and the goal gates only on "enemy near" plus courage, fighting and safety. There is no lawfulness term, no witness or guard risk, and no weight for the size of the loss. So a lawful (0.71), timid (0.25) reporter attacks over 1 c in front of three neighbours.
   - Fix: add a `(1 - lawfulness)` consideration, a penalty for witnesses or guards nearby, and enmity scaled by the stake against the purse. Let ReportCrime outscore Fight for the lawful.
5. `[bug]` **Corp guards police the city.**
   - Cause: `utility/goals.rs` `GoalKind::Arrest` gates only on `Role::Guard`, with no `law_brain::is_city_guard` check. `systems/law.rs::private_patrol_route` draws 5 random clients city-wide whenever a corp has more than 5 contracts.
   - Fix: private guards act only on crimes against their corp's buildings or clients (or hand the suspect to a city guard), and patrol the 5 clients nearest their office.
6. `[feature]` **Wealth has no body**, and on d30 it cost a CEO his life.
   - The owned Flyer for any trip over about 20 minutes.
   - When a locate, tag or hit targets an exec, the corp auto-posts the existing "guard on <person>" contract.
   - Travel by class at night: a CEO does not walk the Sump for 4 h to a Chapel.
   - Spire housing at 20k scale.
   - Bail or a lawyer priced by standing.
   - An acting exec while the head is jailed (Zara, 23 days).
7. `[feature]` **The law's geography.** One Precinct per city: guards travel 56-62 h on 2k and 75 h on 20k, and sleep in hotels. Build Precincts per district cluster, give each guard a beat in their home district, and house guards near the station.
8. `[feature]` **The reporter has no beat** (V2, unchanged).
   - A Cover action: a deed the reporter witnessed, or heard at confidence of 0.6 or more, runs under their byline the next day.
   - The fabrication contest as a visit: the exec or a fixer meets the reporter at the Feed.
   - Reporter standing that grows with stories.
9. `[bug]` **A lost arrest scuffle becomes "Assaulted by <guard>".**
   - Cause: `systems/memory.rs::deed_of` maps `MemoryKind::Lost` to `Deed::Assaulted`, with the cuffing guard as the actor. That feeds gossip, dread and grudges.
   - Fix: write the suspect's loss in `law::try_arrest` as an Arrested memory, or skip the mapping when the winner was a guard on an Arrest.
10. `[feature]` **Idle evenings and old clerks** (V3 #1 and #4, unchanged on 2k).
    - 59 idle hours a week; clerks of 68 and 74, a reporter of 74, an exec of 69.
    - A retirement age, and a post-shift menu: errands, kin visits, the corp bar, overtime.
11. `[feature]` **Reporter pay.** 5 c a day for knowledge 0.92-1.00 is below a clerk's 8 c. Make the post a graded professional one in class pay.
