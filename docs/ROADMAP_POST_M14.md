# Roadmap after M14: word, contracts, the outside, the player

Companion to `VISION.md`, `M10_SCALE.md` and `M11_OWNERSHIP.md`. Drafted 2026-10-05 as one-page sketches, not specs: each milestone below gets its own `M1x_*.md` with the full register (inputs, curves, config, phases) when it is next in line. The data models extend what exists in `components.rs` and the specs before this one; every number is a placeholder for calibration. Decisions are taken by the drafting agent and tabled so they are cheap to overturn.

## The whole roadmap

| Milestone | Scope |
| --- | --- |
| M10 Scale | 2,000 residents, tier-indexed lists, a Statistical tier that produces lives, holes bound lazily, Trace and Life. `M10_SCALE.md`. |
| M11 Ownership | Owners and purses, rent and eviction, eight corps with a brain, founding and incorporation, classes. `M11_OWNERSHIP.md`. |
| M12 Districts | District aggregates, per-district law and private security, litter, bystanders, riots and strikes. |
| M13 Assets | Vehicles, chrome, stims, the security robot: owned assets with upkeep and repossession. |
| M14 Data and Virt | A second plane, decks, ICE, Data as a faction resource, the tech tree. |
| **M15 Word and blood** | Gossip along edges, reputation from what is known, grudges, the Hunt goal, revenge chains, social stats and the social move, the psychological LOD table. |
| **M16 Contracts** | The contract entity, Fixers and hired guns, hits, leverage moves (hostage, extortion, blackmail, threats, fraud), bounties, the law's accessory rule; the open contracts are the quest board. |
| **M17 The outside world** | Parents outside the city at ledger LOD: megacorps, syndicates, the state; remittance, reinforcement, collapse; Virt and economic warfare as the only weapons that reach a parent. |
| **M18 The player** | The player as one more pinned agent, founding through the NPC entry points, the dialogue layer as a renderer over social state, story LOD following the player's circle. |

### Dependencies

**Reputation precedes contracts** because every contract is priced and accepted on it: a hitman takes the job of a buyer whose `honour` says they pay, a blackmail works only while the secret is not yet public, a threat is credible in proportion to the threatener's `dread`, and the law names the buyer of a hit only when word of the contract gets around. Without gossip there is nothing for a contract to be known by. Grudges also give contracts their first and commonest buyer: the avenger too weak to hunt alone. **The outside world needs corps, districts and Virt** (M11, M12, M14): a parent has nothing to fund without a branch, nothing to retake without territory, and nothing that can wound it without the one weapon that reaches a ledger. It sits after M16 rather than before it because a corp's campaign against a rival parent is a string of contracts (a data heist, a strike-breaking crew, sabotage of a Vat Farm), so it reuses the contract board instead of growing bespoke orders; it is otherwise independent of M15 and could run in parallel with it. **The player is last** because the player adds no mechanics of their own: they found through `Register` and the gang bootstrap (M11), they speak through the social move (M15), they take and post jobs on the board (M16), and they can break a megacorp only through M14 and M17. Building the player first would mean building those systems twice, once for the player and once for the city.

---

## M15 Word and blood

**Purpose.** *Friends* and *Word gets around*: your friend is killed by a gang member, you hear who did it, and you go hunting; the killer's brother hears of that in turn. This milestone makes what is known a first-class quantity separate from what happened (M10 already separates them for holes), derives every agent's and faction's reputation from it, and gives agents the stats to act on it: Persuasion, Intimidation, Knowledge and Deception, weighed by relative strength and reputation. NPCs use all of it on each other before the player exists.

### Data model

```rust
/// Extends MemoryEntry: who did what to whom, and how far the story has travelled.
pub struct MemoryEntry {
    /* kind, subject, tick, salience, valence, second_hand, crime: as today */
    #[serde(default)] pub object: Option<EntityId>,  // the victim when `subject` is the actor ("X killed Y")
    #[serde(default)] pub hops: u8,                  // 0 seen, 1 heard from a witness, 2+ rumour
}
pub enum MemoryKind { /* … */ Killed, Betrayed, Avenged, Paid /* existing */, Threatened, Persuaded }

/// Survives the 24-entry memory cap. Cap 4 per agent, lowest weight evicted.
pub struct Grudges { pub list: SmallVec<[Grudge; 4]> }
pub struct Grudge {
    pub target: EntityId,          // an agent, or a gang/corp entity when the culprit is a faction
    pub cause: GrudgeCause,        // KilledKin { victim }, KilledFriend { victim }, Robbed, Evicted, Betrayed, Jailed
    pub weight: f32,               // 0..=1, from the cause and the edge to the victim; decays 0.01/day
    pub since: Tick,
    pub settled: bool,             // target dead or avenged; kept 30 days for the biography
}

/// Recomputed daily from every living agent's memories. Never written by events.
pub struct Reputation {
    pub dread: f32,      // known violence: Killed, Assaulted, Fought/Won as subject
    pub standing: f32,   // known wealth and position: owner, exec, rank, purse band, Founded
    pub honour: f32,     // kept word: paid debts and contracts vs Betrayed, fraud, reneged contracts
    pub heat: f32,       // known to the law: reports, warrants, arrests
    pub known_by: u16,   // living agents holding at least one memory with this subject
}
/// Same four axes on Gang, Corp and Law, from members' reputations and faction acts (raids, evictions, crackdowns).

/// Extends Skills (0..=1, drift on use as `fighting` does).
pub struct Skills { pub stealth: f32, pub fighting: f32, pub farming: f32,
                    pub persuasion: f32, pub intimidation: f32, pub knowledge: f32, pub deception: f32 }

pub enum MoveKind { Persuade, Intimidate, Deceive, Charm }
pub struct SocialMove { pub actor: EntityId, pub target: EntityId, pub kind: MoveKind, pub stake: Stake }
pub enum Stake { Coins(i64), Info { about: EntityId }, Access(EntityId), Join(EntityId), Yield }
pub enum GoalKind { /* … */ Hunt }
```

**Resolution.** `social::resolve(world, &SocialMove) -> MoveOutcome { success, backlash }` with `p = logistic(k · (skill_a − resist_t + w_m · might_gap + w_r · rep_gap))`. `resist_t` is the target's courage for Intimidate, the target's greed inverted for Persuade, knowledge for Deceive, the edge affinity for Charm. `might(a) = fighting + chrome and weapon bonus (M13) + 0.1 × allies within 8 tiles (gang members, Friends), capped at 0.5`. `rep_gap` uses the axis the move leans on: dread for Intimidate, standing for Persuade, honour for Deceive and Charm. A seeded RNG keyed by `(tick, actor, target)` keeps it deterministic.

### Systems

| System | Cadence | What it does |
| --- | --- | --- |
| Gossip at Socialise | event (Full, Coarse) | The existing `social::gossip` generalised: any deed memory with `salience ≥ 0.4` and a subject travels, `hops + 1`, salience × 0.6, filtered by trust on the edge (a low-trust speaker is believed at half salience). |
| Gossip pass | daily | Per Statistical agent, one deed passed along the strongest edge; per Full or Coarse agent who did not Socialise today, one along a random Friend or Family edge. O(agents). |
| Reputation | daily, after gossip | Rebuild `Reputation` from the memory stores (≈ 48k entries at 2,000 agents) and the faction aggregates. |
| Grudge formation | event | On learning (seeing, hearing, or a bind in M10) that X killed, robbed or betrayed someone the holder has an edge with, push or raise a grudge. Kin and Spouse 0.9, Friend 0.6, self 0.5 × the memory's salience. |
| Hunt brain | daily rescore | Adopt `GoalKind::Hunt` per grudge (below); an agent with an active Hunt is never demoted below Coarse, as gang members are never Statistical. |
| Social moves | event | Extortion, robbery, bribery, recruitment and courtship call `resolve` instead of their fixed rolls. |

**Hunt.** Considerations: `grudge.weight` → Linear{0.9,0.1}; `courage` → Linear{0.6,0.4}; `might_gap` → Logistic{6,−0.1}; `1 − lawfulness` → Linear{0.5,0.5}; `1 − own heat` → Linear{0.4,0.6}. GOAP plan across days: `AskAround` (a Persuade or Intimidate move on agents with an edge to the target, success yields the target's last-known Home or Workplace with knowledge as the bonus), `GoTo`, `StakeOut` (Wait, abandon after 3 hours), `Attack` (the existing assault action, lethal when `weight ≥ 0.8`), `Flee`. On success: `Avenged` event, grudge settled. **Revenge chains** are not coded: the victim's kin learn of the killing through the ordinary gossip and grudge rules and the loop restarts. A gang member's grudge against a rival gang member pushes `Shock::MemberKilled` as today, so personal and faction revenge share the trigger.

### Psychological LOD

| Data and behaviour | Full | Coarse | Statistical |
| --- | --- | --- | --- |
| Personality (six traits), Skills incl. the four social stats | yes | yes | yes, frozen (no drift) |
| Memory (24), Grudges (4) | yes | yes | yes |
| Gossip | every Socialise | every Socialise | one deed per day, daily pass |
| Social moves | resolved per encounter | per encounter | the StatTable outcome stands in (`p_robbed`, `p_flirt`); a hole's binder weights candidates by `dread` |
| Hunt | executes | executes | never: adopting a Hunt promotes to Coarse |
| Reputation | read and contributes | same | same: reputation is a world fact, not a tier privilege |

### Player hooks

`PlayerCommand::Rumour { about, deed }` seeds a memory in one agent (god-mode only, logged); the Inspector gains a **Known** tab: who knows what about this agent, at how many hops, and their reputation axes with the top contributing memories. M18 gives the player agent the moves directly.

### Acceptance (seed 42, 120 days)

- Median `known_by` of a murderer 7 days after a bound killing is ≥ 10; at least 25 % of deed memories in the city are second-hand at day 120.
- At least 15 grudges formed, at least 5 Hunts adopted, at least 2 `Avenged` events, and at least one chain where an avenger becomes the target of a later Hunt.
- An agent's `dread` correlates with their known Killed and Assaulted count (Spearman ≥ 0.6); the top-decile `standing` is ≥ 70 % owners, execs or gang leaders.
- Extortion success rate is higher for gang members with dread ≥ 0.5 than for those below it, and higher against targets with no allies within 8 tiles.
- Assaults per day stay ≤ 6.4, the Full-vs-Statistical parity test still passes, throughput stays ≥ 8k ticks/s.

### Decisions

| Question | Decision |
| --- | --- |
| Per-observer opinion or global reputation | Both, cheaply: the global `Reputation` is daily and stored; an observer's opinion is computed on demand as their edge plus their own memories plus the global axes. No N² table. |
| Incremental or daily reputation | Daily rebuild. 48k memory entries is cheaper than keeping counters right through insert, evict, death and save. |
| Where social stats live | `Skills`, beside `fighting`, with the same range and drift. Not a new component. |
| Four axes | `dread`, `standing`, `honour`, `heat`. Enough to price a threat, a meeting, a deal and a risk. |
| What gossip carries | Deeds with an actor (subject) and optionally a victim (object). Not moods, not prices. |
| Unbound holes | Gossip carries "someone killed Y" with `subject: None`; it gains a subject only when the hole binds. Reputation never reads an unbound hole. |

**Out of scope:** lies that spread on purpose (Deceive is used against a target, not broadcast; fabricated rumours wait for M16 fraud), romance beyond the existing Court, family feuds as a faction, dialogue.

---

## M16 Contracts

**Purpose.** *Assassinations are popular* and *Quests are everywhere, and they are real*. A hit is a contract with a buyer, a target, a price and a risk; a Fixer brokers it; a hired gun takes it. The same entity carries every leverage move: extortion generalised from the gang action, blackmail built on what one knows (M15), hostage-taking, threats and fraud, with a threat instead of a payment. The open contracts are the quest board; the player takes the job the city was about to give to someone else.

### Data model

```rust
pub type ContractId = u64;
pub struct Contract {
    pub id: ContractId,
    pub buyer: EntityId,              // an agent, gang, corp or the Law
    pub kind: ContractKind,
    pub terms: Terms,
    pub posted: Tick, pub deadline: Tick,
    pub broker: Option<EntityId>,     // the Fixer; takes `fixer_cut`
    pub status: ContractStatus,
    pub known_by: SmallVec<[EntityId; 8]>,  // who knows the buyer is behind it; feeds the accessory rule
}
pub enum ContractKind {
    Hit { target: EntityId }, Beat { target: EntityId }, Rob { building: EntityId },
    Recover { item: ItemRef, from: EntityId }, Guard { building: EntityId }, Bounty { wanted: EntityId },
    Sabotage { building: EntityId }, Heist { node: EntityId } /* M14 Data */, Bury { corpse: EntityId },
}
pub enum Terms {
    Pay { price: i64, escrow: bool },                 // escrowed coins sit with the Fixer
    Leverage(Leverage),                               // the taker is coerced, not paid
}
pub enum Leverage {
    Hostage { captive: EntityId, held_at: EntityId },
    Secret { memory: MemoryRef },                     // a deed with this agent as subject, low `known_by`
    Threat { credibility: f32 },                      // from dread and might_gap at posting
    Debt { amount: i64 },                             // an Edge.debt called in
}
pub enum Demand { Coins(i64), Do(ContractKind), Access(EntityId), Release(EntityId), Silence }
pub enum ContractStatus { Open, Taken { by: EntityId, since: Tick }, Fulfilled, Failed, Expired, Reneged, Exposed }

/// A leverage move that is not a job: X makes Y do or pay something.
pub struct Coercion { pub holder: EntityId, pub over: EntityId, pub leverage: Leverage, pub demand: Demand, pub until: Tick }

pub enum Role { /* … */ Fixer }
pub enum BuildingKind { /* … */ Den }                // foundable through Register, a Fixer's office
pub enum Crime { /* … */ Conspiracy, Kidnapping, Blackmail, Fraud }
```

`World::contracts: BTreeMap<ContractId, Contract>` and `World::coercions: Vec<Coercion>`, both saved.

### Systems

| System | Cadence | What it does |
| --- | --- | --- |
| Posting | event and daily | A grudge whose Hunt scores low on `might_gap` but whose holder has coins posts a `Hit` or `Beat`; a gang under Retaliate with a strength deficit posts a `Hit` on the rival leader; a corp under Secure posts `Guard`, under Acquire posts `Sabotage` against the weakest rival; the Law posts `Bounty` on an escaped convict; a family that cannot bury posts `Bury`. |
| Brokering | hourly at each Den | The Fixer matches open contracts to agents who drink at their Bar or sit in their Den: fighting or stealth above the kind's bar, lawfulness below it, buyer `honour` ≥ the taker's caution. Unbrokered contracts are posted direct at `fixer_cut = 0` and only to the buyer's edges. |
| Taking | daily rescore | New goal `Contract` scoring price over wage, risk (target's might and dread, district law coverage) and the buyer's honour. Its plan is the existing plan for the kind: a Hit plans as a Hunt without a grudge, a Rob as theft, a Guard as a private guard shift. |
| Settlement | event | Fulfilled pays escrow less the cut; a buyer who refuses an unescrowed payment is `Reneged`, honour falls when known, the taker gains a grudge. |
| Leverage | event | `Coerce` is a SocialMove: success makes the target obey the demand (pay, perform a contract under `Terms::Leverage`, open a door); failure is backlash (a grudge, a report, an attack). Blackmail requires a `Secret` whose `known_by` is small; it lapses when gossip makes the deed public. Hostages are held at a Hideout or Den and need feeding as prisoners do. |
| Fraud | event | A Deceive move posts a fake contract or sells a fake job; when a victim learns of it, `Fraud` is reported and honour drops. |
| The law | event | A fulfilled Hit is a Murder by the taker under the normal report and arrest rules. On arrest the taker is interrogated (an Intimidate move by the guard): success, or the buyer appearing in `known_by` of any witness, files `Conspiracy` against the buyer, sentence as Murder × `accessory_mult`. |

### Player hooks

`PlayerCommand::PostContract { kind, terms, deadline }` for god-mode (the city as buyer). The **Board** panel lists open and taken contracts with buyer, kind, price, broker and deadline, and is the quest log M18 hands to the player agent. The Inspector lists an agent's contracts and coercions on both sides.

### Acceptance (seed 42, 120 days)

- At least 20 contracts posted across at least 5 kinds; at least 2 `Hit` contracts fulfilled, at least one with a corp or gang as buyer.
- At least one `Conspiracy` arrest naming the buyer of a fulfilled hit.
- At least 3 Blackmail coercions, of which at least one lapses because its secret became public; at least one hostage held and released against a demand.
- At least one Den founded by an NPC; Fixers' income per week is within 0.5–2× a Bar owner's.
- At least one `Reneged` contract followed by a grudge or Hunt against the buyer.
- Murders per 120 days rise by no more than 50 % over the M15 baseline; the law's clearance rate on contract killings is between 20 % and 60 %.

### Decisions

| Question | Decision |
| --- | --- |
| One entity for jobs and leverage | Yes. A coerced job is a `Contract` with `Terms::Leverage`; a coercion without a job is a `Coercion`. Extortion of a Home becomes a `Coercion { demand: Coins }` with the M8 claim counting unchanged on top. |
| Fixer as a business | A Den is a building founded through `Register`; the Fixer is its owner and takes a cut. Unbrokered contracts are legal but only reach the buyer's edges, which is why Fixers earn. |
| Escrow | Optional and visible: escrowed jobs are taken more readily; unescrowed jobs are where `Reneged` and fraud come from. |
| Quests | Not a separate system. A quest is an open contract whose taker could be the player. |
| Accessory | Knowledge-driven: the buyer is named only through interrogation or `known_by`, never by the system knowing the truth. |

**Out of scope:** contracts across the city boundary (M17), negotiation over price (fixed price at posting; haggling is a dialogue-layer render in M18), insurance, courts and trials.

---

## M17 The outside world

**Purpose.** *Break, take over* applied to a megacorp, from `VISION.md` § "The world outside the city". A megacorp is a ledger outside the city with a branch inside it. Burn the branch and the parent sends money, bodies or buyers to retake it; drain the ledger through Virt and economic warfare and the branch is on its own. Gangs have a syndicate in the next city, the Law has the state behind it. Nothing outside is ever simulated per agent; it enters only as resources.

### Data model

```rust
pub struct Parent {
    pub name: String,
    pub kind: ParentKind,              // Megacorp, Syndicate, State
    pub treasury: i64,                 // the outside ledger
    pub income_per_day: i64,           // outside business, before the edge factor
    pub edge: f32,                     // 0.2..=1.5 competitive edge; Data theft and lost IP lower it
    pub branches: Vec<EntityId>,       // corps, gangs or the Law in the city
    pub exposure: f32,                 // share of income that depends on the city branches
    pub stance: ParentStance,          // Fund, Reinforce, Retake, Divest, Abandon
    pub stance_since: Tick,
    pub negative_since: Option<Tick>,
    pub collapsed: bool,
    pub last_reinforce: Option<Tick>,
}
pub enum ParentKind { Megacorp, Syndicate, State }
pub enum ParentStance { Fund, Reinforce, Retake, Divest, Abandon }
// Corp, Gang and Law gain `parent: Option<ParentId>` (serde default None). World::outside: Vec<Parent>.
```

### Systems

One daily pass at `tick_of_day == 0`, after the corp and gang brains, O(parents + branches):

- **Ledger.** `treasury += income_per_day × edge × (1 − exposure × branch_loss) + remittance − upkeep`. Branches remit `remit_share` of positive daily cashflow. A seeded outside market stream (`rng.outside(day)`) moves `income_per_day` by a few percent a week so ledgers drift.
- **Stance** (the brain template, five stances, hysteresis): `Fund` transfers coins to a branch whose treasury is low; `Reinforce` brings `n` agents through the immigration path at the map edge as employees, guards or members of the branch (never spawned inside); `Retake` funds the branch's `Acquire` or `Contest` and posts M16 contracts against whoever took its buildings; `Divest` sells branch buildings to the highest purse; `Abandon` cuts the branch loose (`parent = None`).
- **Weapons that reach the ledger.** A Virt `Heist` on a branch node (M14) moves Data and cuts `edge` by `edge_per_data`; an uplink node per parent can be breached directly for a larger cut behind heavier ICE. Economic warfare counts through `branch_loss`: strikes, an `Undercut` that halves the branch's share, a supply cut (no Farm hauls for 7 days), a broken-up monopoly. Each is an existing M11–M14 event read by the ledger, not a new action.
- **Collapse.** `treasury < 0` for `collapse_days` → `collapsed`, `Collapse` event, no more Fund or Reinforce; the branch keeps running on its own purse and dies by the M11 bankruptcy rule. A collapsed Syndicate stops sending members; a State never collapses but can `Divest` guards when the city's tax base falls (the guard budget shrinks).

### Player hooks

`PlayerCommand::Embargo(ParentId)` and `Tariff(f32)` cut remittances (city policy); everything else the player does to a parent goes through Virt runs and contracts. The City panel gains an **Outside** section: each parent's treasury, edge, stance and branches, with a 30-day sparkline.

### Acceptance (seed 42, 120 days, plus targeted tests)

- Every parent's treasury and stance move at least once; at least one `Reinforce` brings agents through the immigration path.
- A test that razes a megacorp's city holdings shows the parent `Retake` within 7 days and the branch owning a building again within 30.
- A test with 10 successful Heists against one parent drives it to `Collapse` within 60 days; the same parent survives the loss of all its city buildings without the Heists.
- No agent ever exists outside the map; the pass costs < 0.1 ms per day; throughput stays ≥ 8k ticks/s.

### Decisions

| Question | Decision |
| --- | --- |
| What a parent is | A ledger plus a stance, nothing more. No outside map, no outside agents, no outside buildings. |
| How the outside enters | Only as coins, immigrants and Data. A reinforcement is immigration with a destination. |
| Who has a parent | Megacorps always; at most one gang (a Syndicate) and the Law (the State) at seed; local corps founded by incorporation never do. |
| How a megacorp dies | Its ledger runs dry. Losing its city buildings hurts through `exposure`, but never kills it alone. |

**Out of scope:** other cities as places, travel, outside wars, parent-to-parent mergers, the player leaving the city.

---

## M18 The player

**Purpose.** *Break, take over, or simply exist.* The player is one more agent with a human brain. This is the integration milestone: it adds input, a camera and the dialogue renderer, and no mechanics the city does not already use.

### Data model

```rust
pub struct PlayerAgent;                              // marker; implies pinned, always Full
pub struct Circle { pub members: BTreeMap<EntityId, Tick> } // last salient interaction with the player
pub struct Message { pub from: EntityId, pub to: EntityId, pub kind: MsgKind, pub about: MemoryRef, pub tick: Tick }
pub enum MsgKind { Call, Text, Email }
pub enum PlayerCommand { /* … */ Step(PlanStep), Move(SocialMove), Coerce(Coercion),
                         TakeContract(ContractId), PostContract { .. }, Register(BuildingKind), FoundGang }
```

### Systems

- **Input** writes `Brain.plan` steps (`Goto`, `Use`, `Wait`) through `PlayerCommand::Step`; exec runs them exactly as for an NPC (`thecity-player-character-later`). Founding is `Register` and the `JoinGang` bootstrap; jobs are `TakeContract`.
- **Dialogue layer.** A renderer, never a source of truth. When the player is in conversation range of an NPC, the app builds a context from sim state (identity, Personality, the edge, the NPC's memories of the player and their circle, grudges, open contracts they hold, reputations) and asks a language model for a tree whose every leaf is a legal sim action: a `SocialMove`, a gossip exchange (`MemoryRef` passed), a contract offer, a coercion. The chosen leaf goes through `PlayerCommand`; the command log replays without the model. Overhearing: an NPC–NPC gossip or social move within hearing range of the player is rendered as lines from the same context.
- **Story LOD.** Agents in the player's `Circle` (a salient interaction in the last 30 days, cap 24) are pinned at least Coarse, Full within view; a new memory of salience ≥ 0.6 in a circle member about the player or a shared edge emits a `Message` to the player, daily batched.
- **First person** is the eventual view; this milestone ships the third-person app with the player agent and leaves the camera change to its own work.

### Acceptance

- A scripted player run on seed 42 founds a Bar, takes a Hit from the Board, completes it, is reported, gains `dread`, and is hunted by the victim's kin within 30 days, all through `PlayerCommand`.
- Replaying the command log reproduces the run byte-identical with the language model disabled.
- No dialogue leaf can change state except through a `PlayerCommand`; the circle never exceeds its cap; throughput with the player pinned stays ≥ 8k ticks/s.

### Decisions

| Question | Decision |
| --- | --- |
| Is the player special in the sim | No. Same components, same goals table available as input, same reputation, same law. Only `PlayerAgent` and `Circle` are new. |
| Where the model runs | In the app, outside the deterministic sim, producing only choices among legal actions. |
| What the model may invent | Wording and tone. Never facts: names, deeds, prices and places come from the context. |

**Out of scope:** first-person rendering, voice, multiplayer, a scripted main story.

## Addendum (2026-10-05, Dylan): governance, missions, combat maps

Folded into the sketches above rather than a milestone of their own:

- **Governance** (M11 hook, M16/M18 behaviour): every faction carries `Governance::Dictator(agent) | Board { members, vote }`. A board's brain reads the aggregate of its members' `Personality`; a leadership change is a vote that leverage moves (M16) can swing one member at a time. A player voted in as a megacorp's leader is the hardest version of "take over".
- **Missions at three LODs** (M16/M18): a contract can be resolved off screen as a probability (the brawl resolver and binder already do this), watched live through Virt when the agents carry the gear (M14), or played by the player character. One contract, three renderers.
- **The outside as combat maps** (M17/M18): a mission on an outside asset is a small map with a transient cast; an NPC who becomes a friend or a specific enemy of the player is promoted to persistence and may later arrive in the city as an immigrant with an edge to the player.

## Addendum 2 (2026-10-05, Dylan): economy, tech, skills, prisons, verticality

- **Goods and production chains** (grows out of M13's second good): many foods and goods with per-good supply, demand and price; vertical farms and fish farms as legitimate producers; a Soylent-style factory and scav chop-shops whose input is abducted people (abduction = a leverage move, M16). Likely its own milestone between M14 and M15 or folded into M13 if the second good is designed as "goods" from the start.
- **Tech decay** (M14): research upkeep; a tech is lost when upkeep lapses or its Data is stolen and deleted.
- **Skills as competence** (M15 stat model, M11/M16 consumers): a rarity distribution over skills; `Corp` and law effectiveness terms read the skills of exec and staff; killing or poaching the skilled cripples a group; poaching by pay, threat or extortion is a contract.
- **Private prisons and hostages** (M16): a corp `Prison` building; hostages as leverage contracts; freeing them as a mission type.
- **Reputation matrix** (M15): faction × faction and player × faction, derived from what is known.
- **Verticality** (a map milestone after M14): layers with a `z` on tile positions and portals between them; the Virt plane is the first second layer and should be built as one.

## Addendum 3 (2026-10-05, Dylan): location knowledge, surveillance, hit squads

- **Sightings as knowledge** (M15, with gossip): `Sighting { who, where, tick, confidence }` in agent memory and in a faction database; relayed along membership edges; decaying. `law::sightings` is the seed. Nobody knows where anyone is except through sightings.
- **Cameras and stealth** (M14 Data, M13 chrome): static sensors on faction-owned tiles feeding the faction database with an ID probability; stealth tech lowers it, sensor quality raises it.
- **Bounties and tags** (M16 contracts): a contract whose deliverable is a sighting stream; tracking tags as sensors on a person; scanners detect tags.
- **Hit squads** (M16, M12 territory): a raid on a person, target tile from the freshest sighting, gated on who controls the territory, relative strength and the political cost with that faction; or the territory's faction is hired to deliver the target.

