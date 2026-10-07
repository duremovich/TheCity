# Shadow pass V1: do residents live like people?

2026-10-07, seed 42, main at 960082d (M15 phases 1-3). Dylan's question: follow residents of different kinds at full simulation detail and judge whether their days read like real people of their type. Method: `citysim-cli shadow --seed 42 --start-day 20 --days 7 --pick gang_member,gang_leader,ripperdoc,homeless,ceo,guard,worker,runner,purist,dealer,child` (pins the picks at Full LOD and writes a diary per agent), then five critics read two diaries each. The critiques are in `docs/shadow_v1/` (one file per pair, each with a day in the life, numbered findings with diary evidence, likely cause, severity and fix). The diaries are regenerable with the command above.

**Verdict.** No archetype reads as its type yet. The skeleton works (needs, goals, plans, money, memories, the theft → report → arrest → sentence chain is good emergence), but a day is dominated by walking, broken sleep and idling, and the role a resident has (gang leader, ripperdoc, runner, CEO, guard, Purist) barely shows in what they do.

## Cross-cutting causes (fix these first: they distort every archetype)

1. **Travel eats the day.** The gang member walks 114 of 168 hours; the guard 13-21 h a day; the worker's commute is 4-6 h; single GoTo steps run 4-7 h. Target and goal choice carry no distance cost (`gang::gang_work_target`, `squat_targets`, `utility/goals.rs`), hiring and housing ignore distance (`demography::hire_candidate`), Sleep walks home instead of to the nearest bed, wages and dole are collected only at the Hall (`CollectWage` at `world.wage_desk`). *Fix:* a travel-time term in target and goal scoring, distance-aware hiring and housing (commute cap ~60 min), sleep at the nearest usable bed, wages paid at the workplace and dole collected in bulk or at a district office.
2. **Sleep loses to everything.** GangWork scores a flat ~0.85 and Arrest fires off shift, with no energy or hunger term; `actions::finishes_early` ends Sleep whenever `routine::must_leave_for_work` holds, which with a long commute is most of the day, so the worker sleeps in one-minute fragments at energy 0.00 and Work/Eat/Socialise flip every 30 minutes. *Fix:* a committed minimum sleep that ignores the commute gate, energy and hunger terms in GangWork and Arrest, Arrest gated on shift, a stronger night gate for the housed.
3. **Stale targets.** Arrest walks to a suspect's last-seen tile within 512 tiles (3 arrests in ~30 attempts, every failure `PreconditionLost` on arrival, possibly a corpse); Chats target people who have moved. *Fix:* re-locate on arrival, expire stale sightings, a per-warrant cooldown, the nearest guard, never a dead target.
4. **No fallback for the poor.** Once the dole is taken `Earn` is satisfied, Beg lives only inside Earn, so broke Street agents Rest or Idle 12-14 h; nobody saves for a Hotel bed; Flee has no plan without a Home. *Fix:* keep Earn alive below a meal with a Beg/Scavenge/gig menu, a "save for a bed" goal, Flee to a squat, Precinct or crowd.
5. **Witness and edge spam.** A multi-minute crime emits one witness event a minute, each −0.2 affinity (a stranger becomes an Enemy in 32 minutes, five Dealing witnesses in 11 minutes); jail at Coarse LOD hands out ~70 affinity gains and ~40 MetInJail memories a day; 17 strangers become Acquaintances in one minute in a bar; an edge can read Enemy at +0.60. *Fix:* one witness record per crime instance, slower edge creation from co-presence, collapse MetInJail, fix the Enemy kind threshold.
6. **Orders flap.** A gang's order flips 9 times in 6 days after its leader's arrest (shock rescoring), so no GangWork completes. *Fix:* a minimum order dwell (e.g. 24 h) and hysteresis on shock rescores, unless the shock is severe.

## Per archetype (what is missing for the role to show)

| Archetype | What the diary shows | What would make it real |
| --- | --- | --- |
| Gang member | Hideout only for sleep; no gang-mate contact; extortion took 0 c; income is the dole; dealing earned 10 c with 58 witness lines | Hideout as home base (socialise, stash, briefings), tribute that pays, dealer margins and discretion, reactions to heat and grudges |
| Gang leader | Runs solo errands like a member; never musters, collects, recruits or has a retinue; treasury 4 c for 33 members; jailed d21 and BreakOut issued twice with no visible result | Rank goals: Muster, Collect, Parley, delegated violence, retinue travel; dues into the treasury; a readable BreakOut |
| Purist | The creed is arithmetic only (recruit filter, tithe, shakedown order); never meets her own gang | Creed goals: Preach, Patrol, Gather; gang-mate socialising |
| Ripperdoc | The Ripperdoc role runs as ClerkWork: a 9 h shift with no customers; the wage chase (Work scores ~0.95 off shift for 4-7 c owed); quits unpaid at 18:00 and is rehired at 00:00 daily (`economy::maybe_quit`) | A Clinic counter job with customers, fees and a wage from revenue; no off-shift wage chase |
| Runner | The pick was a guard who owns a deck: no Virt goals, the deck never used | A runner archetype: fixer-fed RunOrders, no day job, a sell and upgrade loop |
| CEO | Zetatech's exec is 19, jobless, on the dole, 58 c in her purse, renting in Mid East, idle 14-22 h a day; execs are picked at seed as the greediest jobless adult over 18 (`ownership.rs`), paid a flat 10 c a day, with no exec goal | Exec selection by age, skill and rank; an HQ workplace and shift; salary scaled to the corp; Spire housing; actions that carry the corp brain's order; bodyguards |
| Guard | 3 arrests in ~30 attempts; lives in the Spire with an 87-minute commute; walks to the Hall for 5-6 c; sleeps 2-6 h; walks past thefts; lawfulness 0.24 yet takes no bribes | Arrest fixes above; housing near the Precinct; guard-level corruption; witness-first intervention |
| Worker | Sleep/work thrash; worked 3 of 6 days, paid 4 c once; spouse and friends at affinity 1.00 never seen; the theft chain works | Commute and sleep fixes; a weekly "see someone I love" goal; weekly routine templates |
| Homeless | Never begs, scavenges or climbs the ladder to a Hotel at 3 c; Flee unplannable; a corpse lay in the bar all afternoon (Bury unplannable, Loot StockGone) | Earn fallback, save-for-a-bed, a personality-weighted idle menu (busk, watch, scavenge, visit) |
| Child | Children have no Brain, Needs or Memory: a blank 24 h | Child routines (school, play, chores, family) as a later feature |

## Tool notes for V2

The dealer pick fell back to a jailed gang member (5 of 7 days in jail); pinned agents drop to Coarse inside the Jail; money kinds are inferred (no per-agent ledger); gossip told about the agent is not captured; children cannot be pinned. V2 should pick agents free on the start day, keep the pin in jail, and add a `--runner` pick that requires Virt activity.

## Plan

**Life pass L1** (before M15 phase 5 calibrates, because these change every trajectory): the six cross-cutting causes above plus the bugs (the ripperdoc's ClerkWork role and quit/rehire cycle, exec selection and pay, the jail LOD pin, the Enemy edge threshold). Then re-run this shadow pass and compare. The role features in the table (leader goals, creed goals, Clinic customers, a runner archetype, household goals, guard corruption, child routines) are triaged into the roadmap as an addendum, with the cheap ones folded into L1 where the plan allows.

## L1 results

Life pass L1 is the `[life]` config section and `systems::life`. `enabled = false` (the CLI's `--life-off`, and any pre-L1 save) reproduces ab79188: the 15-day report and the calibration city match byte for byte. Off-screen agents follow a re-run `calibrate` (`assets/stat_table.toml`). The bulk dole is built but off (`dole_bulk_days = 1`): the daily dole is paid where the agent is at 09:00 (`dole_in_place`).

**Shadow, seed 42, day 20 + 7, `--count 3` per archetype, the same build with life off and on.** Free agents are picked first (a V2 tool note). Hours are per free day: jail is excluded, and agents jailed most of the week are dropped from the averages.

| Archetype | walk h/d off → on | sleep h/d | longest sleep | idle h/d | work h/d | earned (3 agents) | arrests / Arrest plans | shifts / paid | energy-0 hours |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Gang member | 15.3 → 8.3 | 2.4 → 2.9 | 5.9 → 7.2 h | 4.2 → 3.5 | 0 → 0 | 65 → 46 | – | – | 0 → 54 |
| Gang leader | 15.3 → 7.2 | 4.2 → 2.8 | 7.2 → 5.5 h | 2.6 → 12.2 | – | 56 → 62 | – | – | 26 → 0 |
| Guard | 13.6 → 9.0 | 5.0 → 4.5 | 7.2 → 8.0 h | 2.1 → 7.3 | 2.6 → 2.6 | 102 → 98 | 12/105 → 6/15 | 10/10 → 5/14 wages | 54 → 6 |
| Homeless | 9.9 → 4.1 | 5.8 → 5.8 | 8.0 → 7.0 h | 6.6 → 8.9 | 0.9 → 1.3 | 84 → 116 | – | – | 76 → 0 |
| Worker | 7.4 → 7.6 | 4.5 → 5.8 | 7.2 → 8.0 h | 5.1 → 1.9 | 6.2 → 7.6 | 70 → 79 | – | 15/13 → 22/15 | 60 → 0 |
| Ripperdoc | 5.4 → 3.8 | 4.0 → 4.1 | 6.0 → 5.9 h | 7.0 → 7.0 | 6.7 → 7.5 | 132 → 197 | – | 17/10 → 19/11 | 0 → 0 |
| Runner (guard pick) | 8.9 → 6.7 | 4.6 → 5.7 | 7.2 → 8.0 h | 2.6 → 2.5 | 7.4 → 7.2 | 108 → 80 | 2/34 → 0/0 | 21/18 → 17/16 | 2 → 0 |
| CEO (exec) | 6.8 → 4.1 | 2.3 → 5.2 | 5.2 → 8.0 h | 13.3 → 6.5 | 0 (office hours count as "other") | 203 → 231 | – | – | 0 |
| Purist | 11.8 → 2.9 | 4.3 → 1.0 | – | 5.8 → 2.6 | – | 45 → 36 | – | – | 8 → 0 |

V1's single diaries (960082d) for reference: gang member walk 17.4 h/d; guard 16.9 h/d with 3 arrests in 42 plans; worker 10.4 h/d with energy at 0 for 36 h; CEO idle 18 h/d.

Sleep is still short (4-6 h a day of `Sleep` actions). Part of that is Rest at home, which the tool counts as idle. Part is real: a waking goal still beats Sleep above energy 0.6.

The CEO now keeps office hours. `Meeting` runs at the corp building nearest the exec's Home, 09:00-17:00. The exec is the wealthiest jobless adult aged 30 or more, paid 0.2 % of the corp's treasury a day (10 to 40), and moves to a free Spire Block when one has room.

Clinic staff already work their Clinic shift. `ClerkWork` runs at the employer, `seller_open` keys on it, and Therapy and Install already credit the Clinic. What a ripperdoc still lacks is customers.

**City, seeds 42-44, 120 days (ab79188 → L1, bulk off; bulk-on row with its own calibrated table).**

| | thefts | violent deaths | Murders | jailed (mean) | population d120 | starvation deaths | gang income | assaults/day | ticks/s (report col) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| ab79188 | 7940 / 8992 / 8614 | 277 / 251 / 239 | 187 / 170 / 174 | 140 / 141 / 137 | 1909 / 1904 / 1935 | 2 / 15 / 8 | 45.4k / 39.3k / 35.3k | 20.4 / 18.3 / 17.8 | ~5.6k |
| L1, bulk dole off | 2796 / 2195 / 2483 | 148 / 102 / 124 | 60 / 40 / 52 | 87 / 80 / 82 | 2044 / 2082 / 2065 | 0 / 1 / 0 | 33.6k / 32.3k / 31.4k | 18.5 / 19.9 / 19.3 | ~8.8k |
| L1, bulk dole on (7 days) | 1932 / 1289 / 2250 | 121 / 91 / 115 | 55 / 45 / 55 | 78 / 83 / 88 | 2071 / 2106 / 2082 | 1 / 0 / 0 | 35.7k / 28.7k / 29.1k | 18.7 / 14.0 / 17.5 | ~8.9k |

Need-driven crime falls sharply even with the bulk dole off. In the calibration city (500 agents, all Full) thefts went from 1,152 to 129. The rows of hungry agent-hours shrank about 5x: residents who are paid at work and do not walk 4 h for the dole eat on time. The Statistical table learns this from the Full tier, so off-screen theft (×0.17), robbery, assault and killing (×0.1) fall with it.

Restoring the brutality is a tone decision through deliberate levers (dole size, prices, gang pressure), not through broken days. Gangs, membership and Hideout work stay near base. Witness lines fall from ~23k to ~16k, `OrderChanged` from ~240 to ~115, and save size at day 60 (seed 42) from 22.1 to 14.1 MB.

### L1b: the pressure calibration

Dylan's decision (2026-10-07): keep L1 and bring back the dystopian pressure through deliberate levers, not broken days. The levers, each commented in `assets/config.toml`:

- Food `price_base` 3 → 4.
- `[life] scavenge_p` 0.5 → 0.15. Scrap is a last resort, not a living.
- Execs are the greediest of the wealthiest tenth of the jobless aged 30 or more (`life::pick_exec`, `exec_greed`). The corp brain reads its exec's greed: Squeeze scores greed², and Acquire reads greed.
- The dole stays at 4. A cut to 3 starved the corps: truck share 0.2-0.3, Virt nodes 24-27, Data sold 0-100.

The table was recalibrated. Seeds 42-44, compared with ab79188 and with L1 (the bulk dole off in both L1 rows):

- Thefts are 9.2-14.6k (7.9-9.0k; L1 2.2-2.8k).
- Violent deaths are 115-159 (239-277), Murders 33-55 (170-187), and jailed 96-111 (137-141).
- Riots are 4-6 (2-4), and Dregs sit in the 1-5 % band on 118-120 days.
- Starvation deaths are 10-15 (2-15), and population on day 120 is 1957-2001 (1904-1935).
- Gang income is 26-43k (35-45k), and ticks/s is 6.3-7.8k (~5.6k).

Off-screen killings stay at 3-6 (ab79188 ~140). The table learns its violence from the Full tier, and L1 removed the Hall-queue fights and the witness-spam feuds that produced it. Killings reach 20 only with thefts around 30-50k and ticks/s under 4,000.

Gate doctrine (Dylan, 2026-10-07): the gates assert mechanism, sanity and existence. Calibration bands are printed as `FINDING` lines.

Shadow, `--count 3`, life on with L1b. Hours are per free day:

- Walking is 2.8-12.1 h a day; the Purist is the 12.1.
- Sleep is 1.5-6.8 h, and the longest sleep is 4.8-8 h.
- Hours at energy 0 are 0-18 per triple (L1 off: 0-76).
- Guards made 12 arrests in 25 Arrest plans (L1 off: 12 in 105).
- Workers worked 16 shifts and were paid for 11.
