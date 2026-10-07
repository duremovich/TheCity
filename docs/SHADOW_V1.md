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
