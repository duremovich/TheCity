# M18: The player — one more agent, with a human brain

Companion to `SPEC.md`, `M8_FACTIONS.md`, `M9_LAW.md`, `M10_SCALE.md`, `M11_OWNERSHIP.md`, `M12_DISTRICTS.md`, `M13_ASSETS.md`, `M14_VIRT.md`, `M15_WORD_AND_BLOOD.md`, `M16_CONTRACTS.md`, `M17_OUTSIDE.md`, `ROADMAP_POST_M14.md` and `VISION.md`. M10–M17 built a city that runs without anyone watching. M18 **puts a person in it**: a nobody with twenty coins wakes on a Sump West pavement, gets moved on by a guard at three in the morning, steals a ration, books a capsule bed with the change, overhears two Hollow runners at the Stack Bar arguing over who knifed a clerk, networks with the Fixer behind the bar, takes a Beat for sixty coins, and three weeks later gets a text from the clerk's friend: *"I know who did it. Can we talk?"* Nothing in that paragraph is new mechanics. Every step goes through an entry point an NPC already uses: Vagrancy, Hotel, `Register`, the board, `social::resolve` and the kin channel. Where this document and the earlier ones disagree, this one wins for M18.

The roadmap row (`ROADMAP_POST_M14.md`): *the player as one more pinned agent, founding through the NPC entry points, the dialogue layer as a renderer over social state, story LOD following the player's circle.* M18 also takes what the addenda and `VISION.md` assign to it: permadeath by default with options per world, and combat as rolls and targeting (addendum 4); the PC loop as the NPC ladder, with god tests that time the climb (addendum 5); missions played in person as the third render, and the player as a board challenger (governance addendum, M16 § 6); approaches as action chains the planner proposes, and escape plans (addendum 7); the player on M17's `Abroad` missions and meeting promoted NPCs; and the standing rule of `thecity-player-character-later`. That rule says exec is author-agnostic and runs only `Brain.plan`, a pinned agent is always Full, and no system assumes that an agent is an NPC. First-person, generated text and maps stay out (§ 12).

Scale: 2,000 residents on the 256 × 192 map, with M10–M17 landed. At the time of writing the tree holds M12 phase 2. M13–M17 are specified but not built, and M18 reads them as specified. This is the integration milestone, so it is the shortest spec.

Decisions taken by the drafting agent (overturnable, listed so they are cheap to overturn):

| Question | Decision |
| --- | --- |
| What the player is | A **`PlayerControlled` component on one living adult**, at most one per world. It is pinned (`Brain.pinned = true`), and the LOD pass forces it Full ahead of the ranking. Every other component, rule, table and roll is the NPC's: needs, rent, Vagrancy, arrest, the binder, reputation, grudges, Hunt, hit squads, Wounded. |
| How input reaches exec | **The input system writes `Brain.plan`.** `PlayerCommand::Steps` writes a `Plan { goal: GoalKind::Directed, .. }` of `ActionInstance`s. `PlayerCommand::Goal` calls `plan::plan_for` for a goal the player picks. `think::run` skips the agent unless autopilot is on, and `plan::run` never dequeues it. **Exec is unchanged.** It re-checks every step's preconditions at step start (`FailReason::PreconditionLost`), so a step is legal exactly when the planner could have placed it there. |
| Will vs ability | Preconditions that read Personality as **willingness** are waived for a `Directed` plan, because the human supplies the will. These are lawfulness, greed and courage thresholds such as `gang_eligible`'s desperation gate. Preconditions that read **ability or circumstance** stay: skills, Kit, coins, location, role, tier, cooldowns and capacity. Personality still counts wherever it describes how others meet the character: courage in `resist_t`, sociability in first meetings, and every involuntary choice under `Hold`. Phase 1 audits the table and lists every waived term in code. |
| Involuntary choices | A **`CombatStance`** (Press, Hold, Flee, Yield) chooses among outcomes NPCs already have: fight back, join a brawl, flee home, or submit to arrest or robbery. **It never modifies a roll.** Hold is the NPC rule that reads the character's Personality. Combat is rolls and targeting: the player picks whom to `Attack` and what stance to hold, and `law::resolve_fight` rolls. |
| How a character starts | A **`StartTemplate`** from `[player.templates]`. The default is `sump_nobody`: a homeless adult spawned through `demography::spawn_immigrant` in Sump West, carrying 20 coins and drawing Skills and Body from the seeded distributions. Alternatively, **`TakeOver(agent)`** attaches the component to a living adult. God worlds also allow `custom`. |
| Modes | `WorldOptions.mode`: **Mayor** (v1: levers and no character; the default for `run` and for old saves), **Character** (no city levers), or **Both**. God commands need `WorldOptions.god`, which is set at creation and shown in the title bar. |
| Death | **Permadeath by default.** The character's `Death` is an ordinary death with inheritance, holes, grudges and gossip. It ends the character and the world continues: the app offers a new `sump_nobody` in the same world, or watching as Mayor. With `permadeath = false`, the respawn is **`Heir`**: take over the adult heir with the highest affinity, else a `Newcomer`. The heir takeover is refused under permadeath, so permadeath means losing the life, the edges and the estate. |
| What the player knows | **Knowledge-bound UI.** Panels about other agents and factions read the character's memories, Sightings, edges and `visible` contract set. They do not read `Life`, holes or the event ring. Two exceptions: the map in view is drawn as seen, and the character's own Story tab is complete. The player learns of a hunter the way an NPC would: through a Sighting, a `Threatened` memory or a message. God mode lifts the filter. |
| Quests | **The board plus asking.** `quest::list` is `contracts::visible(player)` plus **asks**. An ask is a personal job (Hunt, Court, Bury) that a known NPC would post under M16 § 1 and that the player has heard about. The player can offer to do it, which is a Persuade move with the M16 price as the stake. On success the NPC posts a direct contract that the player takes at once, with the same payout. |
| Dialogue | **A renderer over legal moves.** `dialogue::options(player, npc)` lists `DialogueAct`s whose preconditions hold, each with its odds from `social::odds`. A `DialogueWriter` trait words them, and the template writer ships. The chosen act becomes a `PlayerCommand::Talk`, and the command log replays it with no writer at all. No writer output is ever read back by the sim. |
| Overhearing | **Every Full agent can eavesdrop, the player included.** A gossip exchange indoors passes its deed to each Full co-occupant with `p = overhear_p`, at `hops + 1`. Exchanges within `hear_tiles` of the player are also pushed to an unsaved ring that the app renders as lines. |
| Story LOD | A **`Circle`** of at most `circle_cap` (24) agents with a salient interaction in `circle_days` (30). Members rank as LOD class 4, below pinned and above guards, so they are never Statistical, and they are Full when in view. A daily **message** pass sends a call, text or email from a member whose brain chooses to share a Life event or a deed that concerns the player. |
| Missions in person | **`Render::Played`** whenever the player is a taker or crew member. The contract is never ledger and never demoted. The crew runs on its own plans, and the player walks their own. Brawl pairing honours the player's `Target`. Watching a mission they are not on is M14's `MissionView`. |
| Approaches | **The planner proposes, the player picks.** `goap::approaches` returns up to `approaches_max` distinct plans for a goal and target. It bans each found plan's signature action and searches again. New approaches are new GOAP actions with preconditions, never scripts. |
| Abroad in person | M17's `Abroad` stays map-less. When the player is in the crew, the single draw becomes **three rounds**, and before each round the player picks a choice against the cast. Neutral choices reproduce the ledger `p` exactly. Talking to a cast member or sparing one moves their disposition, and so their promotion. |
| Founding | `Register { kind, lot }` lets the player choose the kind and the Lot that M11's rule would otherwise choose. **A gang** is founded through the existing bootstrap (an emptied gang and a `WasArrested` memory), through M12's split, or through a new `Register(Hideout)`. The Hideout path is open to NPCs too through the Found goal, behind `found_gang_lawfulness`, so the player gets no door the city lacks. |
| The view | The camera **follows** the player's agent, with free look on a key. The inspector opens on the character. Every view change is still a logged `SetView`, so LOD replays. First-person is out of scope (§ 12). |

## Goals and acceptance

The city should read the same with a person in it as without, and the person should feel it. The target spiral is: **a scripted character climbs from the street to a deed through hotel nights, a squat, a lease and a founded Capsule Hotel → takes and fulfils a Beat from the Stack Bar Fixer → is arrested at least once → kills a Hollow runner in a scripted fight, is witnessed, gains `dread`, and the runner's brother hunts them or posts a Hit within 30 days → overhears it all at the Bar → dies on day 100, and the city does not notice beyond its usual grief and gossip.** On seed 42, 2,000 residents, 120 days:

- **dormant**: with `mode = Mayor` and no character, the 120-day CSV is byte-identical to M17 outside the new columns;
- **the ladder** (`ScriptedPilot`, § 9): the first Hotel night by day 3, a squat or lease by day 30, and **a deed by day 90**, all through `PlayerCommand`s an app user could issue (no god command); ≥ 1 arrest of the character; ≥ 1 contract taken from the board **and fulfilled**; ≥ 1 ask offered and accepted;
- **word and blood**: within 30 days of the scripted killing, ≥ 1 Hunt goal or a `Hit`/`Locate` contract targets the character, and ≥ 1 strike (a stake-out `Attack` or a mission door) reaches them; the character's `dread` is above its day-0 value;
- **words**: ≥ 100 overheard lines and ≥ 20 conversations by the character; ≥ 20 messages, with ≥ 1 from each of Call, Text and Email; the circle never exceeds `circle_cap`;
- **death**: on day 100 a god `KillAgent` on the character emits `CharacterEnded`; no agent holds `PlayerControlled` afterwards; `NewCharacter` on day 101 succeeds; the standing bounds hold to day 120;
- **replay**: the command log replays the pilot run byte-identically with the dialogue writer disabled; a property test shows that no `DialogueOption` changes world state except through its `PlayerCommand`;
- **god scenarios** (§ 9) run, and each one shows the world reacting;
- Assault events per day stay ≤ 42.7; Murders per 120 days stay within +10 % of the M17 run on the same seed, excluding the character's own deeds; starvation deaths and population stay within the scaled v1 bounds; the v1, M8–M17 and Full-vs-Statistical parity gates still pass;
- throughput stays **≥ 8,000 ticks/s** with the character pinned and a full circle.

## 1. The character

### Data model

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerControlled {                       // component; at most one living holder
    pub since: Tick,
    pub template: String,                           // "sump_nobody", "takeover", "custom"
    pub character: u32,                             // the nth character in this world, from 1
    pub autopilot: Autopilot,
    pub stance: CombatStance,
    pub target: Option<EntityId>,                   // preferred opponent in a brawl
    pub found_kind: Option<BuildingKind>,           // read by Register for this agent only
    pub plans: BTreeMap<String, Vec<ActionInstance>>, // saved plans: the escape plan, the commute
    #[serde(skip)] pub approaches: Vec<Plan>,       // the last list shown; Approach(n) picks
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Autopilot { #[default] Off, Needs, Full }   // Needs: Eat and Sleep only, when idle

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum CombatStance { Press, #[default] Hold, Flee, Yield }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldOptions {                           // World::options; saved
    pub mode: PlayMode, pub god: bool,
    pub permadeath: bool, pub respawn: Respawn,
    pub start: String,                              // default template
    pub difficulty: Difficulty,
    pub characters: u32, pub deaths: u32,
}
pub enum PlayMode { Mayor, Character, Both }
pub enum Respawn { Newcomer, Heir }
pub struct Difficulty { pub coins_mult: f32, pub skill_mult: f32, pub start_friend: bool, pub start_lease: bool }
```

`GoalKind::Directed` is appended. It is never scored and never in `GOAL_ORDER`; it only labels a player-written plan. `LocationKey::Building` (resolves to `ActionInstance.target`) and `LocationKey::Tile` (resolves to `ActionInstance.tile`) are appended so that a step can name any door or pavement. `World::player() -> Option<EntityId>` is a cached lookup that is rebuilt on load.

**Difficulty touches only the start.** It changes the template's coins, skills, a starting Friend and a starting lease. It never adds a multiplier that applies only to the player. A harsher city means world config, such as M16's `wound_share` or the guard count, which applies to everyone.

### The input system (`systems/player.rs`)

- **Commands** (§ 7) are applied in `World::apply_commands` at the top of the tick, like every lever. `Steps(v)` aborts the current plan through `abort_plan` and writes `Plan { goal: Directed, target: v[0].target, steps: v, started_tick }`. `Goal(g, target)` aborts, sets `current_goal` and calls `plan::plan_for` with the target bound. On failure it emits `PlanFailed` to the HUD and does not cool the goal. `Stop` aborts. `RunPlan(name)` writes a saved plan.
- **think and plan**: `think::run` skips the holder unless `autopilot = Full`. With `Needs`, an idle holder with no plan for `needs_idle_ticks` is scored over Eat and Sleep only, through the same `utility::think` with a goal mask. `plan::enqueue` refuses the holder unless autopilot planned. **exec, reservations, door queues, witness and sight are untouched.**
- **Advice**: once an hour, `utility::think` runs on the holder for display only and fills `Brain.last_think`. The HUD shows "what someone in your shoes would do", which is the NPC's top five with their considerations. It costs one think an hour.
- **Will gates**: `PlanCtx` gains `directed: bool`, which is true while the running plan's goal is `Directed`. It is also true for a `Goal` command, because the human chose that goal. Waived terms read `ctx.directed ||`. Phase 1 lists them; currently they are `gang_eligible`'s desperation clause and the lawfulness gates on theft-family and `Attack` preconditions.
- **LOD**: `lod::run` places the holder first in `ranked`, ahead of class 5. A Full slot is reserved for it, so `max_full` effectively shrinks by one.

### Starting, taking over, dying

`NewCharacter { template }` spawns the template through `demography::spawn_immigrant` with an `Arrival` override. The override sets the district and tile, the coins (× `coins_mult`), the skills (`U(0,1)^rarity_exp × skill_mult`, the M15 draw), the Body draw, `dress = 0`, and no Home, so the character starts homeless. The Personality draw is the immigrant's and only matters under `Hold`. `start_friend` adds one Friend edge (affinity 0.4) to a jobless adult in the district. `start_lease` houses the character through M11's re-housing at once. The command is refused while a holder lives, or in Mayor mode. `TakeOver(agent)` attaches the component to a living, free adult who is not a guard. In Character mode it needs `god`, because taking over a captain or a corp exec is a god move.

**Death.** In `demography::kill`, a holder's death runs the ordinary path first: Life, inheritance, holes, grudges and the kin channel. The component then goes with the corpse, `deaths += 1`, and `CharacterEnded` is emitted (red). The `Respawn` option and `permadeath` decide what the app offers:

| `permadeath` | Offered |
| --- | --- |
| true | `NewCharacter` (a stranger with nothing of the dead one), or Mayor view. The app's save slot for this world is overwritten at death; manual saves of a permadeath world are disabled (save on quit). |
| false | `Heir`: `TakeOver` of the living adult Spouse, child or Family member with the highest affinity to the dead character, who has already inherited by M11's rule. If there is none, a `NewCharacter`. Loading an older save is allowed. |

The character cannot be killed or saved by anything an NPC would not suffer or enjoy. M16's `Wounded`, Trauma Team subscriptions and Clinics are how a character survives a fight.

```toml
[player]
mode = "Mayor"                     # new worlds in the app default to "Character"
god = false
permadeath = true
respawn = "Heir"
start = "sump_nobody"
needs_idle_ticks = 60
advise_every_ticks = 60            # one think an hour, display only
hud_plan_steps = 8
[player.difficulty]
coins_mult = 1.0
skill_mult = 1.0
start_friend = false
start_lease = false
[player.templates.sump_nobody]
district = "Sump West"
coins = 20
age_years = 24
[player.templates.mid_clerk]       # an easier start: a lease and a clerk job
district = "Mid West"
coins = 120
age_years = 30
lease = true
job = "Clerk"
```

## 2. The ladder and the loop

The character survives through the NPC ladder (VISION, "The survival loop"). Every rung is one an NPC climbs, and the HUD shows which rung they are on (`pc_rung`):

| Rung | Test (computed daily) | The player's way up |
| --- | --- | --- |
| 0 Street | none of the below | `Steps[GoTo(Market), StealFood(Market)]`; `Beg`; `CollectDole`; Vagrancy applies at 03:00 |
| 1 Hotel | a booked bed last night | `Steps[GoTo(Hotel), CheckIn, Sleep]` (M12) |
| 2 Squat | holds `Squatter` | `Goal(Squat, derelict)`; the gangs' claim machinery can evict |
| 3 Lease | a Home with rent | M11 re-housing once the wait and deposit allow, or `Goal(Earn)` for a job first |
| 4 Deed | owns a building, directly or as exec | `Register { kind, lot }` at the Hall; incorporation at `incorporate_buildings` |

**Founding.** `Register { kind, lot }` writes `Steps[GoTo(Hall), Register]` with `found_kind` and `ActionInstance.target = lot`. `Register` reads `found_kind` and the bound Lot for this agent and falls back to M11's per-capita rule when they are `None`. The cost, the Lot conversion and the vacancies are unchanged.

**A gang** has three doors: the M8 bootstrap (`JoinGang` at an emptied gang's Hideout with a `WasArrested` memory; the will gate is waived but the empty-gang and arrest conditions are not); M12's split, when the character is a lieutenant; or `Register(BuildingKind::Hideout)`. The last is new for everyone. `found_cost.hideout` buys a Lot conversion or a derelict the founder squats; a gang entity is created through M12's split spawn with the founder as leader, needing ≥ `found_gang_friends` Friends who accept a Persuade move with `Stake::Join(gang)`. NPCs reach it through the Found goal when `lawfulness < found_gang_lawfulness` and they have the Friends. The phase-5 target is 0–2 NPC-founded gangs per 120 days.

### Quests

`quest::list(world, player) -> Vec<Quest>` is computed on demand by the Quest panel and never stored:

```rust
pub struct Quest { pub source: QuestSource, pub kind: ContractKind, pub target: Target, pub price: i64,
                   pub deadline: Option<Tick>, pub broker: Option<EntityId>, pub odds: f32 }
pub enum QuestSource { Board(ContractId), Offer(ContractId), Ask { npc: EntityId, origin: Origin } }
```

- **Board** is `contracts::visible(world, player)`, open or standing. `AcceptContract(id)` calls M16's `contracts::accept` with the same eligibility as a Fixer offer: not the target, not kin of the target, free, holding no other contract. **The lawfulness gate is waived** under the will rule. The crew is the character's Friends who are regulars, as M16 picks.
- **Offer**: once the character has `Network`ed at a Fixer, they are an ordinary candidate in its daily matching. An offer to them arrives as an Email with `offer_hours` to answer (`AcceptContract` or `DeclineOffer`). Silence counts as a refusal, and matching moves to the next candidate.
- **Ask**: a known NPC (an edge other than Enemy) whose M16 posting rule for Hunt, Court or Bury holds, apart from the price check, **and** about whom the character holds the triggering memory (`Killed` of their kin or friend, `Rejected`, an unburied kin corpse). `OfferService { to, origin }` is a Persuade move with `Stake::Coins(M16 price)`. On success, `contracts::post` makes a direct contract with the NPC as buyer and `Origin` the ask, taken by the character at once (`ServiceAgreed`). On failure, M15 backlash applies.

`odds` is M16's strike estimate `p_win`, with the character's `fi` as the taker for violent kinds and `social::odds` for talking kinds. The character takes the same jobs, at the same price, that the city was about to give someone else.

```toml
[player.quests]
offer_hours = 24
found_gang_friends = 2
found_gang_lawfulness = 0.2
[corps]                            # addition
found_cost = { hideout = 300 }
```

## 3. Dialogue: a renderer over the social state

### Conversing

```rust
pub enum DialogueAct {                              // every leaf of every tree is one of these
    Chat(EntityId), Flirt(EntityId), Propose(EntityId),
    Move(SocialMove),                               // M15: Persuade, Intimidate, Deceive, Charm with a Stake
    Ask { of: EntityId, about: EntityId },          // AskAround in person
    Tell { to: EntityId, memory: MemoryRef },       // pass a deed the character knows: one gossip exchange
    Coerce { over: EntityId, leverage: Leverage, demand: Demand }, // M16 leverage::coerce
    Offer { to: EntityId, origin: Origin },         // § 2 asks
    Leave,
}
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct MemoryRef { pub kind: MemoryKind, pub subject: Option<EntityId>, pub object: Option<EntityId>, pub tick: Tick }
pub struct DialogueOption { pub act: DialogueAct, pub p: Option<f32>, pub crime: Option<Crime>, pub cost: i64 }
```

`dialogue::options(world, player, npc) -> Vec<DialogueOption>` enumerates acts whose preconditions hold now:

- the NPC is co-located, or within `talk_tiles` on the street;
- the M15 meeting gate holds (a broke nobody gets no audience with an exec);
- Stakes are affordable;
- `Tell` lists only memories the character holds;
- `Coerce` lists only leverage the character holds: a Secret by M16's rule, a Captive they hold, a Debt on the edge, or Threat.

`p` comes from **`social::odds(world, &SocialMove) -> f32`**, the pure half of `social::resolve` factored out; `resolve` is `odds` plus the draw, so no outcome changes. `PlayerCommand::Talk(act)` writes `Steps[Talk]` with the new `ActionKind::Talk` (duration `talk_ticks`, target the NPC, precondition still in range). On completion, Talk calls the same function the NPC path calls: `social::resolve`, `leverage::coerce`, `social::gossip`, `social::propose`, Chat's or Flirt's completion effects, or M15's AskAround body. The NPC's own plan is not interrupted, because talking costs the listener nothing. NPCs **start** conversations with the character only through acts they already make: extortion, AskAround, gossip at Drink, a poach, a blackmail, a guard's arrest. These arrive as overheard lines addressed to the character, so a social act always runs through the code path that already handles it.

### The writer

```rust
pub trait DialogueWriter {                          // app side; never called by the sim
    fn line(&self, ctx: &LineCtx) -> String;        // an overheard or received act and its outcome
    fn label(&self, ctx: &TalkCtx, opt: &DialogueOption) -> String; // a choice in the tree
}
pub struct TalkCtx {                                // built from sim state each time; never saved
    pub npc: EntityId, pub name: String, pub personality: Personality, pub appearance: Appearance,
    pub edge: Option<Edge>, pub rep: Reputation, pub regard: Option<Regard>, // the NPC's view of the character
    pub about_player: SmallVec<[MemoryEntry; 6]>,   // the NPC's own memories with the character as subject or object
    pub grudge: Option<Grudge>,                     // the NPC's grudge on the character, if any: tone may show it
    pub holds: SmallVec<[ContractId; 4]>,           // open contracts they placed or hold
    pub wants: Option<GoalKind>,                    // their current goal
}
```

`TemplateWriter` ships. It reads `assets/dialogue.toml`, which keys lines by act, outcome and a tone (cold, warm, afraid, contemptuous) taken from affinity, fear and `taste`. Names, deeds, prices and places are filled from the context. A **model writer** is the hook only (§ 12): it may write wording and tone and invent no facts, and its labels must map one to one onto `options`. The sim never reads a writer's output. The command log holds the chosen `DialogueAct`, so a replay needs no writer. The tone may reveal what the NPC feels, including a grudge, as a face would; the tree never reveals what the NPC does not know.

### Overhearing

`dialogue::heard(world, speaker, listener, act, outcome)` is called from `social::gossip`, Chat, Flirt and Drink completion, `social::resolve`, `leverage::coerce`, `social::propose`, `gang::extort`, AskAround and an arrest. It does two things:

1. **Eavesdropping is a rule for everyone.** When the act is a gossip exchange inside a building, each Full co-occupant other than the pair learns the deed with `p = overhear_p`, at `hops + 1` and `conf × overhear_conf`, through `memory::insert`. The cost is O(occupants) per exchange. M15's calibration is re-checked in phase 3.
2. **Lines.** When the player is in the same building, or within `hear_tiles` on the street, an `Utterance { tick, speaker, listener, act, outcome, about }` is pushed to `World::overheard` (`#[serde(skip)]`, a ring of `overheard_cap`). The app renders it as a speech line over the speakers and in the Heard log.

```toml
[dialogue]
talk_ticks = 5
talk_tiles = 2
hear_tiles = 6
overhear_p = 0.5
overhear_conf = 0.8
overheard_cap = 64
odds_display = "bands"             # "bands" (unlikely … near certain) or "percent"
templates = "assets/dialogue.toml"
```

## 4. The circle, calls and emails

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Circle { pub members: BTreeMap<EntityId, Tick> }   // on the holder; last salient interaction
pub enum MsgKind { Call, Text, Email }
pub enum MsgAbout { Life(LifeKind, Option<EntityId>), Deed { deed: Deed, actor: Option<EntityId>, object: Option<EntityId> }, Offer(ContractId), Alarm(EntityId) }
pub struct Message { pub from: EntityId, pub kind: MsgKind, pub about: MsgAbout, pub tick: Tick, pub read: bool }
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Inbox { pub list: VecDeque<Message> }               // on the holder; cap inbox_cap
```

**Membership.** The following interactions with the character set `members[x] = now`:

- a Talk act in either direction;
- an overheard act addressed to the character;
- a contract both are parties to;
- a fight;
- an edge change of ≥ `circle_edge_delta` in a day;
- being the character's Spouse, Parent, child or Family, refreshed daily.

A member with no interaction for `circle_days` leaves. Past `circle_cap`, the oldest non-kin member leaves.

**Story LOD.** In `lod::run`, members rank at class 4. With `max_full + max_coarse` slots, 24 members are never Statistical, so their days are decided per agent and not by the table. On screen they rank Full.

**Messages.** At `msg_hour` each day, each member with an unsent item decides whether to send it:

| Decision | Considerations | Effect |
| --- | --- | --- |
| Send | `Can(alive ∧ free ∧ a Life event since the last pass with salience ≥ msg_salience, or a new deed memory whose subject or object is the character or someone with an edge to both)` → GATE; affinity to the character → Linear{0.7,0.3}; `sociability` → Linear{0.5,0.5}; salience → Linear{0.6,0.4}; send at ≥ `msg_min` | a `Message`; Spouse, Parent and Family **Call**; Friends **Text**; employer, landlord, Fixer and contract parties **Email** |

A message about a deed is **M15's kin channel widened to the circle**: one `social::gossip` exchange from the member to the character, so the character learns it at `hops + 1`. Offers (§ 2) are Emails from the Fixer. **Alarm** is the bunker hook: a Sighting written into the character's `FactionDb`, or a building they own, of an agent who holds a grudge on them or who is on a mission against them. It comes from M14 cameras or M16 tags on the character's assets, and it fires an Alarm so the character can run a saved escape plan (`RunPlan`). At most `msg_per_day` messages are sent a day, by salience.

```toml
[circle]
circle_cap = 24
circle_days = 30
circle_edge_delta = 0.1
msg_hour = 20
msg_salience = 0.6
msg_min = 0.4
msg_per_day = 6
inbox_cap = 64
life_cap_player = 400              # the holder's Life cap; NPCs keep 48
```

## 5. Missions in person

**Played.** A contract whose taker or crew includes the holder is `Render::Played`. It is never ledger, never demoted, and outside `max_missions` queueing. The NPC crew plans the M16 chain as usual. The character gets the chain proposed as approaches and is expected at the muster point. The mission waits `player_grace_hours` past `raid_at` for the character and then departs without them. A taker who does not show fails the attempt as M16 rules. Arriving late, the character joins the brawl in progress.

**Combat.** `fight_out` pairs raiders and defenders by strength. When the holder is present, their pairing is their `target` if it stands in the brawl, else the pairing rule. Stance decides only presence and response:

- **Press** joins every brawl the character is adjacent to and always resists arrest;
- **Hold** applies the NPC rule (courage ≥ 0.5 joins; the M9 resist roll reads Personality);
- **Flee** writes `FleeToHome` the tick a brawl or `Attack` involves the character;
- **Yield** submits to arrest and hands a robber the M8 take.

Every roll is `law::resolve_fight`'s, and Wounded is M16's. Outside missions, `Attack(target)` is a `Steps` leaf with the target within 4 tiles. It is an Assault and is witnessed, reported and remembered like any other.

**Approaches.** `goap::approaches(world, id, goal, target, k) -> Vec<Plan>` runs `plan_for` and bans the plan's **signature action**: its most costly action that is not `GoTo`, `Wait` or `Muster`. It repeats until `k` plans exist or `approach_expansions` is spent. Each entry shows:

- steps and cost;
- `p_win` for a strike chain;
- the crimes it commits;
- whether each step's preconditions hold now.

For a Hit the chains include the M15 stake-out at the intel tile, the night ambush at `TargetHome`, M14's door hack and M13's vehicle approach when the character has the kit. A mission-door assault comes as a squad. `Approach(n)` writes the n-th plan as `Steps`. NPCs keep taking the cheapest plan, so the list is the same planner, read wider.

**Abroad in person.** When the holder is in an M17 `Abroad` crew, the draw becomes `abroad_rounds` rounds at a per-round `p'` that solves `P(≥ ⌈rounds/2⌉ wins | p') = p`, so neutral choices reproduce M17's `p`. Before each round the app shows the cast (`CastMember` records) and the player sends `AbroadChoice`:

```rust
pub enum AbroadChoice { Press, Sneak, Talk(u8), Spare(u8), Withdraw }
```

| Choice | Effect on the round | Effect on the cast |
| --- | --- | --- |
| Press | `p' + choice_bias` | none |
| Sneak | `p' + choice_bias × (stealth − 0.5) × 2` | none |
| Talk(i) | `p' + choice_bias × (persuasion − 0.5) × 2` | the member's disposition moves `talk_disp` |
| Spare(i) | `p' − choice_bias` | the member's disposition moves `spare_disp` |
| Withdraw | ends the mission as failed | none |

Each round rolls the death draw at `abroad_death_p ÷ rounds`. M17's promotion then reads the moved dispositions, with `promote_pinned_mult` already applying. A promoted NPC who arrives joins the circle with their edge. Generated combat maps are out of scope (§ 12).

**Watching.** When the character is the buyer and not on the mission, the camera can switch to the mission through M14's `MissionView` while a crew member streams it. Without a streamer the character sees only outcome events.

```toml
[player.missions]
player_grace_hours = 2
approaches_max = 4
approach_expansions = 2000
abroad_rounds = 3
choice_bias = 0.15
talk_disp = 0.3
spare_disp = 0.5
```

## 6. Camera and view

- **Camera** (`citysim-app/src/camera.rs`): `CameraMode::Follow(EntityId) | Free | Mission(ContractId)`. Follow eases `centre` toward the holder's tile at `follow_ease` (0.2) a frame. `F` toggles Free, and panning drops to Free. Every view change still issues `SetView`, so LOD replays.
- **Input** (`input.rs`):
  - a right-click on a tile writes `Steps[GoTo(Tile)]`;
  - a right-click on a building opens the menu of `ActionKind`s whose preconditions would hold on arrival (a `PlanCtx` for the holder with the building bound), plus GoTo;
  - a right-click on an agent offers Talk (the dialogue panel), Attack, Follow (GoTo their tile, re-issued as they move) and Approaches;
  - number keys pick stance, `P` toggles autopilot, `Q` opens Quests, `M` the inbox, `J` the Board filtered to the character.
- **HUD**: needs bars, coins, `pc_rung`, stance, autopilot, the current plan's steps (`hud_plan_steps`), the advice trace and the clock.
- **Inspector** opens on the character. Its tabs are Now, **Story** (M10; the full Life at `life_cap_player`), **Known** (M15: what the city knows of the character; in Character mode only the tallies the character could infer: `dread`, `standing`, `honour` bands and the faction opinions cached for pinned agents), **Kit** (M13 inventory, chrome, assets, vehicle), **Holdings** (buildings, gang or corp, scrip), **Contracts** (theirs, both sides, plus coercions) and **Grudges** (their own; others' grudges on them only as far as their memories show).
- **First person** is out of scope. It needs map layers and interiors (the verticality milestone), a raycast renderer over tile and room geometry, and per-frame agent interpolation. The sim already supplies everything else: the holder's tile, facing from the last step, sight from Kit, and the overheard ring as speech.

## 7. Commands, levers, CLI, events

```rust
pub enum PlayerCommand { /* … M17 */
    // M18 character commands: refused in Mayor mode or with no holder (except NewCharacter, TakeOver)
    NewCharacter { template: String }, TakeOver(EntityId),
    Steps(Vec<ActionInstance>), Goal(GoalKind, Option<EntityId>), Approach(u8), Stop,
    SavePlan { name: String, steps: Vec<ActionInstance> }, RunPlan(String),
    SetAutopilot(Autopilot), SetStance(CombatStance), Target(Option<EntityId>),
    Talk(DialogueAct), AcceptContract(ContractId), DeclineOffer(ContractId),
    PlacePosting(Posting),                          // M16 contracts::post with the character as buyer
    OfferService { to: EntityId, origin: Origin },
    Register { kind: BuildingKind, lot: Option<EntityId> },
    AbroadChoice { contract: ContractId, round: u8, choice: AbroadChoice },
    // M18 god commands (need WorldOptions.god)
    GrantGear { agent: EntityId, kind: AssetKind, tier: u8 }, SetBody { agent: EntityId, strength: f32, reflex: f32 },
    Teleport { agent: EntityId, tile: TilePos }, SetWorldOption(WorldOptionEdit),
}
```

Every command is logged with its tick, and a replay of the log reproduces the run. A refused command logs `CommandRefused` (grey) with its reason (mode, no holder, preconditions, not visible) and has no effect. The existing god commands that apply to a character are M15's `GrantSkill` and `SetReputation`, M11's `GrantCoins`, v1's `KillAgent`, `JailAgent` and `FreeAgent`, M16's `Coerce` and `TakeContract` (forced), and M17's `PostAbroad`.

**CLI** (`citysim-cli run`):

- `--player <template|agent-index>` creates the character on tick 0 and sets `mode = Both`;
- `--pc "day=<D>[:<HH>]:<command>=<args>"` (repeatable) schedules character commands through the lever parser: `goto=hall`, `steps=GoTo(Market),StealFood(Market)`, `goal=Squat`, `register=hotel`, `accept=first_visible:Beat`, `talk=persuade:<agent>:coins:50`, `stance=yield`, `autopilot=needs`;
- `--pilot ladder` runs the test-side `ScriptedPilot` (§ 9) inside the CLI;
- `--permadeath false`, `--god`.

Agents are named by index or by `--select-name` names, as the existing levers do.

**Events** (new `EventKind`s, gold): `CharacterCreated`, `CharacterTookOver`, `CharacterEnded`, `CommandRefused`, `ServiceAgreed`, `OfferReceived`, `OfferLapsed`, `RungReached` (with the rung), `PlayedMission`. Messages and lines are not events; they are counted in the CSV.

**CSV** (`--report`): `pc_alive`, `pc_rung`, `pc_coins`, `pc_dread`, `pc_standing`, `pc_honour`, `pc_heat`, `pc_circle`, `pc_messages`, `pc_overheard`, `pc_talks`, `pc_contracts_done`, `pc_arrests`, `pc_hunters` (agents with a Hunt on the character plus open Hit, Beat, Locate and Tag contracts naming them; measured, never shown in Character mode), `pc_characters`, `eavesdrops`.

## 8. Save compatibility

`World::options` defaults to Mayor mode, so every pre-M18 save loads as v1 play. `PlayerControlled`, `Circle` and `Inbox` are new component stores. `GoalKind::Directed`, `LocationKey::Building` and `Tile`, `ActionKind::Talk` and every command and event above are appended. `World::overheard` is not saved. `NewCharacter` works on any loaded save, so an M17 city can be entered mid-run. **With no holder, every M18 branch is dormant**: think, plan and LOD read `World::player()` as `None`, eavesdropping is the only new rule that runs, and `[dialogue] overhear_p = 0` with no holder gives a 120-day CSV byte-identical to M17. `Config::v1_profile` sets `overhear_p = 0` and `mode = Mayor`, so the v1, M8 and M9 tests run unchanged.

## 9. Testing and calibration

**Unit tests** (`citysim/tests/player.rs`, `dialogue.rs`, `circle.rs`):

- `Steps` writes a `Directed` plan that exec runs exactly as the same `ActionInstance`s from the planner, byte for byte over 200 ticks;
- a step whose precondition does not hold fails with `PreconditionLost` and changes nothing;
- the will gates are waived and the ability gates are not: a lawful character can steal, and a broke one cannot `CheckIn`;
- think and plan never touch the holder under `Autopilot::Off`, and `Needs` plans only Eat and Sleep;
- the holder is Full under any LOD pressure;
- Vagrancy, arrest, rent arrears and the binder treat the holder as any agent;
- `Register` honours `found_kind` and the Lot;
- `Register(Hideout)` makes a gang with the founder as leader only with enough Friends;
- `AcceptContract` refuses an invisible contract and a contract on kin;
- an offer lapses after `offer_hours`;
- `OfferService` posts a direct contract at the M16 price;
- `social::odds` equals the `p` that `resolve` draws against;
- every `dialogue::options` leaf maps to a command, and `options` mutates nothing (a `World` hash before and after);
- an overheard gossip exchange teaches a Full co-occupant at `overhear_p` and a Statistical one never;
- the ring fills only within `hear_tiles`;
- the circle caps at 24 and keeps kin, and members never go Statistical;
- the message considerations send by kind;
- a Played mission waits `player_grace_hours`, and brawl pairing honours `target`;
- stances never change a `resolve_fight` roll for the same RNG stream;
- `approaches` returns distinct signature actions;
- neutral `AbroadChoice`s reproduce M17's `p` over 10,000 draws within 1 %;
- under permadeath, the character's death ends the character and refuses `TakeOver(heir)`, and without permadeath the heir is taken over with the estate;
- a pre-M18 save loads in Mayor mode, and `NewCharacter` on it succeeds;
- the replay of a 30-day scripted log is byte-identical.

**The pilot.** `ScriptedPilot` (`tests/support/pilot.rs`) is a test-side policy, `fn decide(&World, EntityId) -> Vec<PlayerCommand>`, called hourly. It issues only character commands and runs with `autopilot = Needs`. Its rules, in order:

1. Steal food when hungry and broke.
2. Book a Hotel when coins ≥ the night price at 21:00, else squat.
3. `Network` at the nearest Fixer every 7 days and accept the visible Beat, Deliver or Locate with the best odds above 0.5.
4. Take a job through `Goal(Earn)` when one is open.
5. `Register` the cheapest foundable kind as soon as coins allow.
6. On day 30, `Goal(Fight)` against the nearest Hollow member until one dies, the scripted killing.

**Scenario** (`citysim/tests/scenario.rs`, `#[ignore]` `test_m18_player_seed_42`): the bullets under *Goals and acceptance*, with the pilot from tick 0 and a god `KillAgent` on day 100.

**God scenarios** (`tests/god.rs`, the VISION questions asked with a character holding the best stats, unlimited money and every tool). Each runs from `NewCharacter custom` with skills at 1.0, Body at 0.9, `GrantCoins 100000` and `GrantGear` tier 3:

- `god_pc_power`: how fast does the character reach `standing ≥ 0.8`? The script registers buildings, incorporates, hires Security and posts Hits on the three richest rival execs. The gate is reaction: ≥ 1 corp under Acquire or Secure targeting the character's corp, and ≥ 1 Hunt or Hit on the character within 60 days. The days to incorporation and to `standing ≥ 0.8` are recorded.
- `god_pc_board`: take Habitat through its board. The character is hired at Habitat by a Persuade with `Stake::Job` and reaches the board by standing. They then blackmail, threaten or bribe each swing member, with `Coerce` forced only where M16's secret rule finds a secret. The gate is ≥ 3 `VoteSwung` and the character as chair (`Corp.exec`) within 120 days, or the failure recorded with the tally.
- `god_pc_break_corp`: destroy Militech through the board and Virt. The character founds a Lab, posts `Steal { Node(uplink) }` heists and runs uplink wipes with a tier-3 deck, while the character's own Security corp undercuts. The gate is `ParentDied` or Militech's branch bankrupt by day 120. The control is the same money spent on buildings alone, which leaves Militech's parent alive.
- `god_pc_take_city`: the character founds a gang through `Register(Hideout)`, funds it and orders raids. Does its district share pass 50 %? Do the law's stance and the State's mandate (M17) react, and do rival gangs split?

**Calibration** is tuned through `[player]`, `[dialogue]` and `[circle]` only, and never by touching NPC tables to suit the player:

| Target | Band |
| --- | --- |
| Pilot days to deed, seed 42 | 30–90 |
| Pilot deaths before day 30, over 20 seeds, `sump_nobody` | 10–40 % |
| Overheard lines per in-game hour at a busy Bar | 2–10 |
| Messages per day, days 30–120 | 0.5–4 |
| Circle size on day 60 (pilot) | 8–24 |
| Seeds with a hunter on the character within 30 days of the scripted killing | ≥ 50 % of 20 |
| NPC-founded gangs through `Register(Hideout)` per 120 days | 0–2 |
| M15 rumour reach on day 120 with eavesdropping vs without | within ±10 % |

**Throughput.** One pinned agent plus ≤ 24 circle members inside the existing Coarse budget. The input system is O(1) a tick (applying queued commands). Advice is one think an hour. Eavesdropping is O(occupants) per indoor gossip exchange. The ring is O(1) per social act (one distance check). The message pass is O(circle) a day. Approaches, options, odds and the quest list are computed on demand by the app, never per tick. Dialogue rendering is UI. Target ≥ 8,000 ticks/s with the holder and a full circle; the regression budget is ≤ 2 %.

## 10. Phases

1. **The character**: § 1 (`PlayerControlled`, `WorldOptions`, templates, `NewCharacter` and `TakeOver`, the input system, `Directed` plans and the will-gate audit, `LocationKey::Building` and `Tile`, autopilot, advice, the forced Full slot, death and respawn), the `Steps`, `Goal`, `Stop`, `SetStance` and `SetAutopilot` commands, camera Follow, the HUD, CLI `--player` and `--pc`, and the god set. With no holder, a 120-day CSV is byte-identical to M17, and the 30-day replay is byte-identical. Commit.
2. **The ladder and the board**: § 2 (`pc_rung`, `Register { kind, lot }`, `Register(Hideout)` for everyone, the Quest panel, `AcceptContract`, offers by Email, `OfferService`), the knowledge-bound inspector, `ScriptedPilot`, the ladder half of the scenario. Commit. The first `RungReached 4` on seed 42 is the first visible M18.
3. **Words**: § 3 (`social::odds`, `ActionKind::Talk`, `dialogue::options`, `DialogueWriter` and `TemplateWriter` with `assets/dialogue.toml`, eavesdropping for every Full agent and its M15 re-check, the overheard ring and speech lines) and § 4 (`Circle`, class-4 LOD, the message pass, Alarm and `RunPlan`). Commit. Watch it live: lines at a Bar should read as the city's own gossip.
4. **In person**: § 5 (`Render::Played`, the grace wait, target and stance in `fight_out`, `goap::approaches`, Abroad rounds and choices, `MissionView` switching), `PlacePosting`, the remaining panels and CSV columns, and the god scenarios. Commit.
5. **Gate and polish**: the M18 scenario, the four god scenarios, calibration, README and docs (the controls), then `/code-review high <base>..HEAD` and a fix commit, then push.

## 11. Risks

- **Player-written plans reach exec paths that assumed the planner's shapes.** Examples are a `Use` with no `GoTo` before it, a target the planner would have bound differently, or a Directed plan surviving a goal-specific abort hook. The precondition re-check is the guard. Phase 1 adds a fuzz test of 10,000 random `Steps` vectors over 30 days, which must show no panic, no stuck reservation and no plan that runs forever.
- **Replay and determinism leak through the app.** Camera, dialogue, the writer and the quest list are app-side and must never write the world. Only commands do. The `World`-hash test around `options` and `quest::list`, and the byte-identical replay with the writer off, are the gate. Every new app read takes `&World`.
- **The will waiver makes the player the strongest thing in the city.** Free of Personality, a human can be more ruthless than any NPC. The reply is the city's, not a nerf: dread, heat, grudges, hunts, hit squads and the accessory rule already scale with deeds. The death band, the hunter band and the god scenarios check that the city answers. If it does not, the fix goes into the NPC rules that should have answered, never into a player-only multiplier.

## 12. Out of scope (after M18)

First-person rendering and the camera beyond Follow, Free and Mission (§ 6 lists what it needs); real model-generated text and voice (the `DialogueWriter` trait is the hook, and only the template writer ships); generated combat maps for `Abroad` and anything drawn of the outside beyond the cast panel; multiplayer, and more than one holder per world; a scripted main story; haggling beyond the fixed M16 price; daemons beyond M17's Blackwall cap; Power and Water as goods; map layers, verticality, interiors and room-level security; climbing and jumping between buildings; a dedicated bunker goal beyond saved plans and Alarms; inventory management beyond the M13 Kit panel.

## Addendum (2026-10-05, Dylan): the majordomo and the squad

Spec amendment; decisions tabled as overturnable and to be read by the M18 plan and its coders.

**The idea.** When the player owns a faction, they can hire a **majordomo** (a general, a lieutenant) to handle the day-to-day of a bigger faction. The player does not have to: a micromanager issues every order. A player who wants to be a CEO sets policy, hands the operation to the majordomo, and goes rampaging with a smaller **squad** they direct personally.

**How it lands on what exists.** A faction's brain already runs off one agent's `Personality` (the gang leader, the corp exec; M11 `Governance::Dictator`). For a player-owned faction:

- **No majordomo**: the faction's brain is off; the player issues the faction's orders directly through `Character` commands (the same `Order`/`CorpOrder` set the brains choose from, plus price levels, postures, contracts), and the faction's daily mechanics (wages, rent, upkeep, contracts, hiring) still run.
- **A majordomo hired**: an agent with the role `Majordomo` (hired through the vacancy table like any job, at a wage; or promoted from a member) becomes the faction's `exec`/`leader` for the brain, which runs on *their* `Personality` and competence (M15) under the player's **policy**: pinned orders, a price-level band, a posture, a treasury floor, a "no raids into cover" flag, a list of standing contracts to keep. Policy is a small struct the brain reads as gates and flats. The player's own orders override for a day.
- **The squad**: up to `squad_max` (6) members the player directs with the raid machinery (muster at the player, follow, attack a target, hold a door), through the same `Expedition` M16 uses, with the player as the leader body; squad members keep their needs and can refuse by loyalty (a social move).
- **The majordomo is an agent**: they can be poached (M15), bribed, blackmailed, coerced (M16), killed, or **turn**: a majordomo whose loyalty falls below `turn_threshold` under a rival's coercion embezzles (a Flow to themselves), leaks (a deed to a rival's database), or splits the faction (the M12 split rule with themselves as the new leader). The player sees the trace of their decisions in the faction panel and can fire or replace them.

| Decision | Call |
| --- | --- |
| Role | `Role::Majordomo` with `employer` = the faction's seat (Hideout or the corp's first building); wage `[player] majordomo_wage` (20/day); one per faction. |
| Brain switch | `Faction.brain_runs = majordomo.is_some()`; when off, `Order`/`CorpOrder` come only from `Character::FactionOrder`; the orders' effects (`act`) run either way. |
| Policy | `Policy { pinned_order: Option<..>, price_band: (f32, f32), posture: Option<..>, treasury_floor: i64, no_raids_into_cover: bool, keep_contracts: Vec<ContractId> }`, `#[serde(default)]`, read by the brain as gates and flats; the inspector shows which policy line gated a decision. |
| Squad | `Squad { members: Vec<EntityId> }` on the player, cap 6; commands Follow, Hold, Attack(target), Muster, Dismiss; resolved on `Expedition` with the player as leader; refusal by loyalty through the social resolver. |
| Turning | `loyalty < turn_threshold` (0.3) after a coercion or a missed wage: embezzle (`Flow::Embezzle`, 10 % of the treasury), leak (a deed), or split (M12 rule); logged as a `Betrayal` event the player's circle reports. |
| NPCs too | Any faction whose leader or exec is away (jailed, Abroad, a run) names a majordomo by the same rule, so the mechanic is not player-only. |
