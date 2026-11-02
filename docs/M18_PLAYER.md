# M18: the player slice, a nobody in the Sump for 14 days

Rewritten 2026-11-02 from `BIG_PICTURE_2026-11-02.md` (Dylan took every recommendation; decisions 1-7 in its table).
The old spec (494 lines, written 2026-10-05..09 at 2,000 residents against the macroquad debug app, last changed
`c9fea4b`) is in git: `git show 2c2e0fd:docs/M18_PLAYER.md`. Where the two disagree, this one wins.

**The slice.** A human takes over one adult in a pre-run city and lives 14 days in the Sump. The city is the 20k
island (`assets/maps/isle_m_k2.txt`, 160-280 ticks/s), loaded from a day-21 save; 100k stays the shipped city and is
read at closes (decision 2). Make the save once: `citysim-cli run --days 21 --seed 42 --population 20000 --scaled
--ambient-flats 50 --map assets/maps/isle_m_k2.txt --save-at 30240` (1,440 ticks a day). The pre-run save meets
`OPEN-150` (no day-0 start) for the slice; the starting-condition seed stays for the shipped game. The client is
cityview (`view/cityview`, the orbit follow-cam and the cutaway); the player is the camera subject, so the V5 ring
(`systems/ring.rs`) makes them and everyone near them Full for free, which also fills the empty room around them.

**What it tests** (the reasons to build it before more sim): whether the board has takeable jobs near a poor player;
whether a nobody survives the Sump (the city's harshness was tuned for NPCs who don't mind dying); whether anything
ever happens *to* the player in 14 days (if nothing approaches them, that is the #1 sim gap, with a target); whether
menu verbs plus a log make dialogue unnecessary for v1.

**How it is proven.** Each phase's proof is a play session (the coder plays; a short log and screenshots from the
bench's shots), or a diary read when the phase changes NPC behaviour (decision 4: such a phase still reads its
archetypes). The slice's exit is **Dylan playing 30 minutes cold** (P5). No gate beyond the change tier and the view
tier (`python tools/test.py view`); no test unless the brief names one. Play sessions write the OPEN rows.

## Kept from the old spec

- **Directed plans through the same exec.** The player's input writes `Brain.plan` (steps or a goal handed to
  `plan::plan_for`); exec runs it like an NPC's and re-checks each step's preconditions at its start
  (`FailReason::PreconditionLost`), so a step is legal exactly when the planner could have placed it. No system
  assumes an agent is an NPC; the player gets no door the city lacks and no rule of their own.
- **The will-gate, per verb.** Preconditions that read Personality as *willingness* (lawfulness, greed, courage on
  `PlanCtx`, `goap/actions.rs:503`) are waived for a player's plan, because the human supplies the will. Preconditions
  of *ability or circumstance* (skills, kit, coins, place, role, cooldowns, capacity) stay. Personality still counts
  where it is how others meet you (resisting, first meetings). Audited lazily: each verb the slice offers lists its
  waived terms in code, not a global audit up front.
- **The city answers, not a nerf.** A will-free human can be more ruthless than any NPC; the reply is dread, heat,
  grudges, hunts and hits, never a player-only multiplier.
- **No child is ever a valid target** (CLAUDE.md, 2026-10-27): no verb in the menu targets anyone under 18.

## P1 the seam (riskiest first)

**Goal.** A human drives one body in cityview; the city carries on around it.
**Check first (the riskiest assumption):** that `think` and the plan queue can skip one agent with no side effects.
The skip lands in `systems/think.rs:68` `turn()` (it already skips Statistical, sentenced, cuffed, emigrating and
downed bodies) and in `systems/plan.rs` (`run` :18 must never dequeue the player, `enqueue` :69 refuses them unless
autopilot). Also check that cityview draws a 20k-map save (the island geometry is TheCityGraphics'; the sim's
buildings match lots by rect, `lots.rs`; the 20k and 100k maps are re-imported together, `425e6c2`).
**Anchors.** `view/cityview/src/simthread.rs`: `Shared` holds slots and atomics only (`hold`, `reload_req`), so the
render thread has no way to send the sim a command; the R key's `reload_req`, applied between ticks, is the precedent.
Add a command queue (mpsc or a `Mutex<Vec<_>>`) drained between ticks. `citysim/src/levers.rs` `PlayerCommand` (the
Mayor's levers plus `Pin`, `SetView`) is where the new commands go. `Brain.pinned` (`components.rs` `Brain`; the
sim thread pins the subject, `simthread.rs:223`). `plan::plan_for` (`systems/plan.rs:204`).
**Build.** `PlayerControlled { autopilot: bool }` on one adult; `think` skipped unless autopilot. Commands: `Steps`
(a plan of `ActionInstance`s), `Goal(GoalKind)` (via `plan_for`), `Stop`, `SetAutopilot`. egui via `bevy_egui`
(cityview has no UI today; `citysim-app` already uses `egui-macroquad`): a HUD with the clock, needs, coins, job and
the current plan with its last failure. Click the ground or a door to go there (a ray against the lot meshes).
Keys: autopilot, pause, speed.
**After it the player can** possess a body in the 20k save, walk anywhere, eat and sleep through `Goal`, and hand
the body back to its brain.
**Proof.** A play session: possess a broke Sump worker, walk to a shop, buy, go home, sleep; autopilot on and the
body lives its NPC day. Screenshots of the HUD.
**Out of scope.** Picking bodies, verb menus, the log, a start screen.

## P2 verbs

**Goal.** Click a building or a person and get a menu of what you can do there, and only that.
**Anchors.** Picking: bodies (`view/cityview/src/people.rs`, no pick today) and buildings (`lots.rs`). The menu is
built from precondition checks on a `PlanCtx` (`goap/actions.rs:503`) for the target, so only legal actions list.
Verbs map to existing `ActionKind`s: Eat, Sleep, work and apply (`Goal(Work)` / `Goal(Earn)`), Shop, `StealFood`,
`Attack`, `Chat`, `Extort`, `Register` (P6). The will-waivers go here, per verb: a human may steal where the NPC's
lawfulness or greed gate refuses.
**After it the player can** get a bed, take a job, steal, start a fight, and be arrested for it.
**Proof.** A play session (with its log): a broke character gets a bed and a job, then is arrested for a theft.
**Out of scope.** The board, social moves beyond Chat, the consequence feed.

## W work beats (review 1 option A)

**Goal.** A shift reads as a day at work, not `ClerkWork` x10. Once P2 lets the player take a job, day one is ten
identical blocks in an empty room (`BIG_PICTURE_2026-11-02.md` § 1): fix it for the player and every desk NPC at once.
**Anchors.** `ActionKind::work_for` (`goap/actions.rs:320`) maps most trades to `ClerkWork`; the shift's block seam
is `systems/shift.rs:861` `block_done`; the exec's one-block meeting is `service.rs` `meeting_hour`; promotion into a
building is `lod::set_lod_at` (`lod.rs:570`); effects reuse `social::interacted` (`social.rs:179`),
`World::remember`, `gossip::post_deed`.
**Build.** A config deck `[[work_beats]]` (trade or niche, hour band, weight, counterpart rule, effect, one-line
text), drawn at station-block boundaries, with a real coworker as the counterpart (promoted for the beat, or a
Statistical one named in the line): a task from a named superior, a colleague stopping by, a mistake, a shared lunch
picked by edge, an overheard rumour, the boss's walk-through. Effects only through `social`, memories and gossip
(a skill tick, a standing nudge). Every number in config.
**After it the player** meets named coworkers on shift, and a lunch partner recurs across the week.
**Proof.** A diary read (this phase changes NPC behaviour): the desk archetypes (`office_worker`, a trade worker,
`exec`) at 20k show 3-6 named beat lines a shift and a recurring face in a week; quote them, and report what reads
wrong. Plus one player shift.
**Out of scope.** Per-trade verbs beyond the deck's lines; the gang's own day (review 1 option B), which comes only
if P3-P4 show gang life matters to the player.

## P3 the board and talk

**Goal.** The open contracts are the player's quests; a conversation is a menu of social moves.
**Anchors.** `contracts::visible` (`systems/contracts.rs:876`) and `contracts::accept` (`:1477`), the same payout an
NPC gets; fulfilment runs on the existing action chains. `moves::resolve` (`systems/moves.rs:400`, persuade,
threaten, extort) and `social::propose` (`systems/social.rs:620`, befriend, court).
**Build.** An egui board panel near the player (what a Fixer or a known contact would show them); accept; the
contract's steps run as a Directed plan. Talk on a body: Chat, befriend, court, persuade, threaten, ask for work.
**After it the player can** take a Beat or a Locate, finish it and get paid, and work a person through social moves.
**Proof.** A play session: take a contract, finish it, get paid; befriend a bartender; the issuer remembers.
**Out of scope.** Dialogue text beyond a menu row and its odds; posting contracts; missions played in person.

## P4 consequences

**Goal.** The player sees the city happen to them. This is the phase that decides whether the slice is a game.
**Anchors.** The shadow's narration is `citysim-cli/src/shadow.rs` (`DayFold`, `render`), a fold of the event ring
in the CLI crate that cityview does not link: lift the part the log needs into `citysim` or re-render the player's
events in the client. Memories about the player (`World::remember`, `MemoryEntry` subject), gossip (`gossip.rs`),
grudges and hunts (`systems/hunt.rs`, `grudges.rs`), hits (`hitmarket.rs`), `Life` (`components.rs` `Life`) for the end.
**Build.** A live log of the player's own lines; "who knows about you" (memories, gossip and grudges naming the
player, with how they heard); hunts and hits surfaced when they form against the player; a day-end summary (coins,
who now knows you, who is after you, what changed on your block); a death screen from the player's `Life`.
**After it the player** sees a deed travel and come back.
**Proof.** A 14-day play session where one deed visibly returns as a message, a visit or a hunt. If nothing comes
back in 14 days, that is the finding: an OPEN row with the target.
**Out of scope.** The knowledge-bound inspector, the Circle and its calls/texts/emails, overhearing.

**The combat model** (`OPEN-179`) is considered after P4 (decision 5): if the slice's fights read as one dice roll,
it becomes the phase after P4; if not, it waits.

## P5 start and loop

**Goal.** A cold start that a person who did not build it can play.
**Anchors.** `demography::spawn_immigrant` (`demography.rs:1868`) for a new arrival; `save::save_to_file` /
`load_from_file` (`save.rs:583`, `:590`).
**Build.** "A new nobody": pick a broke adult from the day-21 save by archetype, or an immigrant into the Sump with a
few coins. An egui menu (new, load, quit), permadeath (death ends the character; the city goes on; pick another
nobody), save and load of the possessed world, 20k by default. **`citysim-app` (the macroquad debug app) is retired
here** (decision 6): removed from the workspace, its remaining uses moved to cityview or the CLI.
**After it the player can** start, die, start again, save and come back.
**Proof.** **Dylan plays 30 minutes cold.** His notes become OPEN rows owned by the next pass.
**Out of scope.** Modes (Mayor, Both), difficulty, templates beyond one, heirs.

## P6 founding (optional, only if P1-P5 read)

**Goal.** The player opens a bar or a Hideout through the door NPC founders use.
**Anchors.** `founding::register` (`systems/founding.rs:773`), `ActionKind::Register`.
**Proof.** A play session: found, hire, pay rent, lose or keep it.
**Out of scope.** The majordomo and the squad (parked, VISION).

## Dropped (do not reintroduce)

The byte-identical dormancy and replay gates, the fuzz and property tests, the ScriptedPilot ladder asserts, the
tps floors (all against the 2026-10-26 doctrine); Mayor/Character/Both modes, difficulty menus, `StartTemplate`
sets and the heir respawn; the `DialogueWriter`, `assets/dialogue.toml` and overhearing; the Circle and its messages;
missions in person, Abroad rounds and approaches; the majordomo and the squad; § 6's macroquad camera and input.
Any of them comes back only if a play session asks for it.
