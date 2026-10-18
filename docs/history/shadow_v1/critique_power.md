# Critique: the CEO and the guard (seed 42, d20-d26)

## Ursula Zhou, exec of Zetatech (treasury 8,547c): a day in the life
- 05:37 wakes (slept 5.6h on d20 only), walks 14 min to Bar#415 for a 2c drink, chats with a stranger.
- 10:12 pays 60c for therapy at a Sump ripperdoc, leaving her with 6c.
- 11:37 queues at Civic Hall for 3-4c of dole, then goes home.
- 12:38 to 23:59 "Rest" in 90-minute blocks. Idle is 14-22h of every day.
- d21 to d26: dole at about 00:30, a 9c food run, a stim buy (10c) at Bar#417, then 8-10h parked in that bar watching dealers deal.
- d22 19:21 reports a dealer at the Hall after five witnessed deals. She spends d23-d26 afraid, dosing, and at 41% addiction.
- Net -46c over seven days. Never sleeps after d20, never visits the Spire or an office, never meets her spouse, and sees no bodyguard, rival, meeting or order.

## Nyx Osgood, city guard (41, wage 8/day): a day in the life
- Home is Block#76 in the Spire. Work is the Precinct in Civic, 87 minutes away on foot.
- 06:30 wakes, or is dragged out of bed by the Arrest goal.
- 09:00 to 18:00 shift: pick a warrant, walk 1-4 hours to the suspect's last seen tile, arrive, fail. Repeat.
- Roughly once a day she does a 25-min GuardJail or a Patrol leg. On d26 she "rests" in the Precinct for 68 minutes.
- Walks 87+ minutes to Civic Hall to collect a 5-6c wage, in person, nearly every day.
- 18:00 onward she tries to walk home, is pulled away by Arrest, and sleeps 2-6h.
- Travel is 13-21h a day. Over seven days she made 3 arrests in about 30 attempts. Net +1c.

## Findings

1. **The CEO is not a CEO in any visible way.** What: 19 years old, no Job, a 10c/day exec wage, dole queue, a Mid East rental, and zero actions from the corp's `Secure` order. Why it matters: the exec is the player's future target and the corp's face (VISION: execs ride cars with bodyguards, set routines, can be hunted or leveraged). Likely cause: `ownership.rs` seed step 6 picks the greediest jobless adult with `is_adult` (18+) and no check on age, skill, wealth or class. `exec_wages` pays a flat `wage_exec` = 10 into the daily pass. `is_exec` is only read as an exclusion in `founding`, `assets`, `districts` and `street`, never as a goal gate. The corp brain reads only the exec's greed, courage and lawfulness. Severity: missing feature, with the age pick a design gap. Fix: add an Exec role (workplace = the corp's best building, shift hours, `Work` goal with Meeting/Review actions). Pick execs by age (30+), persuasion/knowledge and rank, and let the brain's chosen order spawn visible exec actions (Secure orders a bodyguard hire and a trip to the Security Office).

2. **Exec pay is absurdly small and the money is disconnected.** What: 8,547c in the treasury, and she dole-collects for 3-4c while greed is 0.96. Why: wealth drive pushes an heiress to a handout line. Cause: `wage_exec` = 10 flat, no salary scaled to treasury, no draw or dividend. Severity: shallow. Fix: exec comp as a share of corp income (salary tier plus a bonus on profit days), with the Earn goal ignored or replaced by a Draw action at the corp HQ.

3. **Exec housing and the Spire are inverted.** What: the CEO rents Mid East Block#105 from Habitat, while a 41-year-old guard on 8c/day lives in a Spire Block. Why: the Spire is meant to be the rich district (M11 gives all 60 Spire Blocks to the largest Housing corp). Cause not confirmed in code; I found no class-aware home assignment at seed. Severity: shallow. Fix: assign tier-2 Homes to execs and top earners at seed, and migrate households by wealth. Make Spire rent a visible status filter.

4. **Idle plus Rest eats the whole day.** What: Idle (constant 0.05, runner-up 0.15) loops 90-minute Rest blocks, 14-22h/day, and Rest restores energy so Sleep never scores again after d20. Why: it reads as a coma, and the CEO's belonging, intimacy and safety needs sit unmet with no goal strong enough. Cause: goal scoring leaves Socialise at 0.10-0.20, below Earn and GetHigh spikes, and a flat Idle floor with no "do something with money" goal. Severity: shallow. Fix: add rich-agent goals (Dine, Shop, Visit Spouse, Attend Event) gated on the wallet, give Rest diminishing energy returns, and scale Idle down when belonging/intimacy are above 0.5.

5. **The spouse never appears.** What: both agents have a Spouse at +0.85-0.87 affinity. The CEO meets hers 0 times and the guard's spouse shows up once, as a gossip target. Intimacy sits at 0.9-1.0 (guard) and decays from 0.92 to 0.61 (CEO). Why: marriage is invisible. Cause: no Visit/Dinner-with-spouse goal; the Socialise target is picked at the bar, not from family. Severity: missing feature. Fix: a Household goal pair (EatTogether, SpendTimeWith spouse) using the existing Chat action, plus a belonging boost.

6. **The guard walks hours to chase stale sightings and arrests almost nobody.** What: 3 arrests in about 30 attempts, every failure `PreconditionLost` on arrival (e.g. d20 Dagny Park#1619 chased from 15:58 to d21 00:10 across four districts). Gossip on d22 says that suspect was killed by Philippa Duarte. Why: the law looks broken and the guard burns her life on it. Cause: `law.rs:chaseable` uses a pursuit radius of 512 tiles and a `suspect_seen_ticks` window, and the Arrest step calls `arrest()`, which fails unless the guard is within 1 tile of a living, not-sentenced suspect. The guard walks to the last seen tile, not the suspect's current one, and re-picks the same suspect with a 2-hour cooldown. `located()` uses `is_alive`, so a corpse may still count; this needs checking. Severity: bug (stale target and radius) and shallow (no tracking). Fix: shrink the radius to the guard's district plus a neighbour, drop a suspect once last-seen is older than a few hours, re-target on sightings in transit, and stop chasing a corpse.

7. **The commute is 87+ minutes each way, and wages are collected in person.** What: Spire to Civic 87 min, then another trip to Civic Hall for a 5c wage, almost daily. Why: it crushes the guard's life, with 13-21h/day in travel. Cause: `GoTo(Hall) > CollectWage` plus a home assigned with no regard for job location. Severity: shallow. Fix: pay the wage into the wallet at the shift's end (or let the Precinct pay it), and assign guards to Homes near the Precinct. Give guards a vehicle or a patrol car.

8. **Sleep is endlessly interrupted; energy sits at 0.00 for a day with no effect.** What: on d21 she sleeps 2.2h and is at energy 0.00 from 06:00 to 22:00, yet keeps walking, with only mood sliding to -0.19. Why: no exhaustion consequence; the Arrest goal (0.865) beats Sleep (0.49-0.76) even off shift. Cause: `Arrest` scores at 0.522 with "in shift 0.00->0.50", so it runs 24 hours. Severity: bug (off-shift duty) and shallow (no exhaustion). Fix: gate Arrest on shift or on-call, add a collapse or errors-from-fatigue rule, and let Sleep win under 0.2 energy.

9. **No corruption or bribe use despite lawfulness 0.24 and a 8c wage.** What: one attempt to strip a corpse (d26 14:55, in shift, 40-min walk, failed `PreconditionLost`). No bribes taken, no looking away, no shakedown of dealers she passes. Why: the brief says "corruptible"; M9 has bribes for factions only. Cause: `bribe_price` and the law-brain bribes are faction-level; no guard-level Take-a-bribe decision. Severity: missing feature. Fix: a GuardCorrupt action on a witnessed crime with a lawfulness/greed roll, a payoff from the offender, and a reputation risk.

10. **The CEO is a dealer's best customer and a mortal enemy in the same hour.** What: she buys stims from Bar#417's owner on d21-d23, sits there for hours, then gets five witnessed-Dealing hits in 11 minutes and the dealer goes Acquaintance to Enemy (-0.98), then Flee. Why: it shows no personality or consistent values; duplicate events inflate affinity. Cause: `Witness` fires per tick-pair without dedup, and her "ReportCrime" uses lawfulness 0.68 while GetHigh uses 1-lawfulness. Severity: shallow. Fix: dedupe witnessed crimes per (offender, crime, 30 min), weight Witness by relationship and by her own involvement.

11. **A busy Hall and unrelated travel stalls.** What: d23 `GoTo(Hall)` fails `BuildingFull` at 03:24 and then takes 71 more minutes. A 10-minute home trip takes 148 minutes at 21:21. Why: it is unclear whether this is crowding or pathing. Cause: not checked. Severity: possible bug. Fix: log `blocked_since` and congestion in the diary, and cap Hall queues with a wait action.

12. **No reaction to the world.** What: the guard saw theft (d21, d26) and assault (d24) in shift and kept walking. The CEO is the target of no threat, and her "safety 0.07" after a dealer's hostility produces only a flee. Why: the law does not feel like a presence. Severity: shallow. Fix: a witness-first response for in-shift guards: Intervene or Detain when the crime is within a few tiles, before pursuing old warrants.

## Top 5 changes, ranked
1. Make execs real: age, skill and rank selection, an HQ workplace and shift, a salary scaled to the corp, and visible actions that carry the brain's order (findings 1-3).
2. Fix the guard's chase: smaller radius, stale-sighting expiry, no corpse targets, Arrest gated on shift (6, 8).
3. Fix homes and commutes: assign Spire housing by class, guards near the Precinct, wages paid at shift end (3, 7).
4. Add rich-agent and household goals so a wallet and a marriage produce a day (4, 5).
5. Add guard-level corruption and witness-first intervention, and dedupe witnessed crimes (9, 10, 12).
