# M16: Contracts — the board, Fixers, hired guns, leverage, the wounded

Companion to `SPEC.md`, `M8_FACTIONS.md`, `M9_LAW.md`, `M10_SCALE.md`, `M11_OWNERSHIP.md`, `M12_DISTRICTS.md`, `M13_ASSETS.md`, `M14_VIRT.md`, `M15_WORD_AND_BLOOD.md`, `ROADMAP_POST_M14.md` and `VISION.md`. M15 gave the city knowledge as talk: who is feared, who is owed, who means to kill whom. M16 gives it **knowledge as a price**: what it costs to have a thing done, and who will do it. A Mid West clerk's friend is too weak to hunt the Hollow runner who killed her, so she takes her savings to the Fixer behind the Stack Bar; the Fixer passes the job to a Sump gun who drinks there; the gun waits outside the runner's Home on the habit the Bar told him, and the runner dies on the second night. Three weeks later the gun is picked up on an old Assault, gives up the Fixer's name under questioning, and the clerk's friend is charged as accessory. Meanwhile Habitat keeps three of Stackwell's tenants' children in a private prison until Stackwell's board sells two Blocks, and Stackwell's chair loses the next vote to the member Habitat has been blackmailing. Where this document and the earlier ones disagree, this one wins for M16.

The roadmap row (`ROADMAP_POST_M14.md`): *the contract entity, Fixers and hired guns, hits, leverage moves (hostage, extortion, blackmail, threats, fraud), bounties, the law's accessory rule; the open contracts are the quest board.* M16 also takes what the addenda and `VISION.md` assign to it: missions at three LODs (governance addendum), the board vote swung by leverage (governance), private prisons, hostages and freeing them as a mission, abduction as a leverage move feeding the scav economy, poaching by threat (addendum 2), hit squads gated on territory, relative power and political cost or the territory's faction hired, bounties for location streams, tracking tags and scanners (addendum 3), propaganda and fabricated stories as contracts (addendum 6; M15 deferred fabrication here), the `Wounded` state, Rescue by faction temperament, Clinics as treatment and Trauma Team as a subscription contract on a person (addendum 8), and `Heist` as a contract kind (M14 deferred it). Daemons, the outside world and the player stay out (§ 14).

Scale: 2,000 residents on the 256 × 192 map; every number assumes M10–M15 have landed. At the time of writing the tree holds M12 phase 2; M13's assets, M14's runs and `FactionDb`, and M15's deeds, reputation, grudges, Hunt and `social::resolve` are specified, not built, and M16 reads them as specified.

Decisions taken by the drafting agent (overturnable, listed so they are cheap to overturn):

| Question | Decision |
| --- | --- |
| What a contract is | A struct in `World::contracts: BTreeMap<ContractId, Contract>`, not an entity: buyer, kind, target, terms, deadline, broker, taker, status, risk, political cost, and `known_by` (who knows the buyer is behind it). Hundreds at most (`max_open` 256 open). The existing `Corp.contracts` (Security guard contracts on buildings) stays as it is; M16 calls those *guard contracts* in prose and does not migrate them. |
| Jobs and leverage | Two structs, as the roadmap sketched: a **`Contract`** is a job someone is paid or coerced to do; a **`Coercion`** is a standing hold one party has over another (protection, blackmail, a hostage) with a demand. A coerced job is a `Contract` with `Terms::Coerced(CoercionId)`. |
| Who may be a buyer | Any M11 owner: an agent, a gang, a corp, or `None` for the city and the law, paying through `World::purse` and `ownership::pay`, so the ledger and the conservation test cover every coin. |
| Escrow | **Brokered means escrowed.** A contract placed with a Fixer moves the price into `Contract.escrow` at posting (`Flow::Escrow`, counted by `ownership::total_coins`); a direct contract (no Fixer) is unescrowed and reaches only the buyer's edges and faction. Reneging and fraud live on the direct side, which is why Fixers earn their cut. |
| The Fixer | `BuildingKind::Fixer` (label "Fixer", letter `X`), foundable through `Register`, worked by its owner or one `Role::Fixer`. It keeps a book, a list of regulars, a cut and its own **heat**. Matching is **daily**, at `match_hour`, never hourly. The roadmap's `Den` is renamed to the kind it is. |
| Who takes jobs | Agents (hired guns, couriers, runners, talkers), gangs as factions (the tenth gang `Order::Job`), Security corps (Guard, Escort, Trauma), Feeds (Spin). The taker of record is one `EntityId`; a crew rides along in the mission. The player takes the same jobs in M18 through the same call. |
| Missions | A **mission is a `raid.rs` expedition with a contract as its source.** The muster, the march, `raiders_at`, `by_strength`, `fight_out`, crossfire and litter are factored to take a `Mission` as well as a `Gang`; gang raids keep their `Gang.raid_at` state unchanged. A hit squad is a raid on a person. |
| Three LODs | One contract, three renderers: **ledger** (both parties Statistical and unwatched: one draw at a hash-picked due tick), **live** (any party Full or Coarse, or the mission in view: executed by exec; streamed through M14's `Overwatch` when a crew member carries a deck), **played** (M18). The ledger draw uses the same strength estimate the live brawl would use. |
| Off-screen fulfilment | **A pre-bound hole.** A ledger Hit kills through `demography::kill` and opens a `HoleKind::Killed` hole with the new field `Hole.contract = Some(id)`; the binder skips the candidate draw and binds `Actor(taker)`, still rolling the witness from the hole's own stream. The 2-bit kind field stays full; the answer never changes. |
| The accessory rule | Knowledge-driven, as M15 requires. A new deed `Hired` (actor buyer, object target) is known to the buyer, the Fixer and the crew, and travels by gossip like any deed. The law files `Crime::Conspiracy` against the buyer when an interrogation of an arrested taker succeeds or when a guard or a reporting witness holds `Hired` about the victim of an open Murder report. The system never names a buyer it has not been told. |
| Hit squads | A Hit whose solo `p_success` is below `squad_below` goes as a squad. The strike is **gated** on the target district's controller (M12), relative strength, and a political cost with that controller; when the cost exceeds the job's value and the controller is a gang that would sell, the job is offered to that gang instead (`Sold out`). |
| Bounties and tags | `Locate` pays per sighting delivered to the buyer's `FactionDb`; `Tag` plants an `AssetKind::Tag` (M13 asset, tier 1–3) on the target that writes one sighting an hour until its battery dies or a `Scanner` finds it. Both reuse M14/M15's `Sighting`. |
| Extortion | Generalised to `leverage::coerce`, the Intimidate move of M15 with a demand. The gang Home shakedown is a one-shot coercion with the M8 claim counting unchanged on top; **protection** is the standing version, weekly, on owner-run businesses in held districts. |
| Blackmail | A secret is a deed the holder knows about the target whose holder count is ≤ `secret_known_max`. It lapses when the deed's pool reach passes `secret_public_reach`. Holder counts are tallied by M15's daily reputation pass for the few keys referenced by secrets, so no extra memory scan. |
| Hostages | A `Captive` component on the held person (as `Sentence` holds a convict), at a Hideout or a corp `BuildingKind::Prison`. Taking one is M13's `Abduct` and its `Crime::Abduction`; the leverage is a `Coercion` over the captive's kin, employer or faction. Freeing one is a `Rescue` contract: a raid on the building. |
| Fraud | Three forms only: a buyer who posts direct with no intent to pay (renege decided at posting from a hash), a fabricated story bought through Spin, and a fake fixer selling a job that does not exist. All go through M15's Deceive move and `Crime::Fraud`. |
| The wounded | `resolve_fight`'s death roll, when it hits, becomes `Wounded` with `p = wound_share` instead of a death; the wounded bleed out after `bleed_hours` unless carried to a Clinic. Deaths can only fall, never rise, from this rule; the Murder baseline is protected. |
| Rescue and temperament | `Temperament { Abandon, Loyal, Recover }` as a trait on gangs (Recover for corps and the law, fixed). Rescue is a Full/Coarse goal for the wounded's faction-mates and kin. Off screen it is in the StatTable, re-calibrated, except Trauma subscribers, who get a multiplier. |
| Trauma Team | `ContractKind::Trauma`: a standing weekly subscription a person buys from a Security corp. A wound writes a beacon sighting (conf 1.0) to the corp's `FactionDb`; the corp sends a crew from its Security Office to the last sighting, carries the client to a Clinic. |
| Governance | `Governance::Board { members }` gains a vote rule and a vote every `vote_days` or on the chair's death. Two seed corps get boards. Members vote by preference; a `Vote` coercion or a `Persuade` contract swings one member at a time. |

## Goals and acceptance

The city should read as a place where anything can be bought: a price on a name that the buyer thinks nobody knows, a Fixer who knows everyone's, a gang that will sell out a stranger in its own street and not its own member, a corp that keeps people in cells for leverage, and a board that votes the way its secrets say. The target spiral: **a Mid West clerk is killed by a Hollow runner → her friend's grudge cannot win a fight, so she posts a Hit with the Stack Bar Fixer → a Sump gun takes it, stakes out the runner's Home on the Bar's intel and kills him → the runner's brother hunts the gun (M15) while the law files a Murder → the gun is arrested on another charge, interrogated, names the Fixer and the buyer → Conspiracy against the friend, the Fixer's heat closes the Stack Bar office for a week → The Hollow posts a Locate bounty on the friend.** On seed 42, 2,000 residents, 120 days:

- contracts: ≥ 60 posted across ≥ 8 kinds; ≥ 25 fulfilled across **≥ 4 kinds**; ≥ 2 Fixers in business on day 120, ≥ 1 founded by an NPC; a Fixer's weekly income within 0.5–2× a Bar owner's; ≥ 1 `Reneged` followed by a grudge against the buyer;
- hits: ≥ 4 Hits fulfilled, ≥ 1 with a gang or corp buyer, ≥ 1 by a squad of ≥ 2, ≥ 1 squad strike declined on political cost, ≥ 1 job sold to the territory's gang; **≥ 1 fulfilled hit later attributed**: a `Conspiracy` filed against its buyer ≥ 1 day after the killing; clearance on contract killings (taker arrested within 30 days) 20–60 %;
- leverage: ≥ 1 hostage taken and **≥ 1 hostage rescued**; ≥ 3 blackmail coercions with ≥ 1 lapsed when its secret went public; ≥ 1 protection coercion paid for ≥ 3 weeks; ≥ 1 poach by threat; ≥ 1 fabricated story and ≥ 1 exposed;
- bounties: **≥ 1 `BountyPaid`**, ≥ 1 tag planted and ≥ 1 found by a scanner;
- the wounded: wounded are 20–50 % of would-be fight deaths; ≥ 1 rescue by a gang member, **≥ 1 `TraumaSave`**, ≥ 1 `BledOut`;
- governance: ≥ 2 board votes held, **≥ 1 `VoteSwung`** (a member whose vote under coercion differs from their own preference);
- Assault events per day stay ≤ 42.7, **Murders per 120 days rise by ≤ 25 % over the M15 run** on the same seed (contract killings included), starvation deaths and population stay within the scaled v1 bounds, and the v1, M8–M15 and Full-vs-Statistical parity gates still pass;
- throughput is reported per gate; the shared floor is 4,000 ticks/s (2026-10-06: relaxed from 8,000 while systems are still being built; optimisation is a later pass).

## 1. The contract entity

### Data model

```rust
pub type ContractId = u64;     // World::next_contract, monotonic, saved
pub type CoercionId = u64;     // World::next_coercion
pub type MissionId = ContractId; // a mission is keyed by its contract

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum ContractKind { Hit, Beat, Abduct, Rescue, Escort, Steal, Deliver, Locate, Tag, Persuade, Vote, Spin, Guard, Trauma }

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Target {
    Agent(EntityId),
    Building(EntityId),
    Node(NodeId),                                  // M14 Data: a Steal is a heist
    Carry { what: Cargo, to: EntityId },           // Deliver: a corpse, a captive, Parts, Stims, Data
    Seat { corp: EntityId, member: EntityId, for_: EntityId }, // Vote
    Story { deed: Deed, actor: EntityId, object: Option<EntityId>, fabricated: bool }, // Spin
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Terms { Pay { price: i64 }, Coerced(CoercionId) }

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ContractStatus { Open, Taken, Standing, Fulfilled, Failed, Expired, Reneged, Cancelled }

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Origin { Hunt, Court, Bury, GangOrder(Order), CorpOrder(CorpOrder), Law, Board, Subscription, God }

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Render { #[default] Ledger, Live, Played }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contract {
    pub id: ContractId,
    pub buyer: Option<EntityId>,                   // an M11 owner; None = the city or the law
    pub agent: Option<EntityId>,                   // the person who placed it (the buyer, an exec, a leader, the captain)
    pub kind: ContractKind,
    pub target: Target,
    pub terms: Terms,
    pub escrow: i64,                               // coins held; 0 when direct
    pub broker: Option<EntityId>,                  // the Fixer building
    pub posted: Tick, pub deadline: Tick,
    pub origin: Origin,
    pub status: ContractStatus,
    pub taker: Option<EntityId>,                   // an agent, a gang, a corp or a Feed
    pub crew: SmallVec<[EntityId; 4]>,
    pub attempts: u8,
    pub risk: f32,                                 // 1 − p_success for the median gun, at posting
    pub political: f32,                            // § 3, at posting, refreshed at the strike
    pub renege: bool,                              // decided at posting, hidden (§ 4 fraud)
    pub due: Option<Tick>,                         // the ledger resolution tick
    pub render: Render,
    pub known_by: SmallVec<[EntityId; 8]>,         // who knows the buyer is behind it
}
```

`World::contracts`, `World::coercions: BTreeMap<CoercionId, Coercion>` (§ 4), `World::missions: BTreeMap<MissionId, Mission>` (§ 3), `World::next_contract`, `World::next_coercion` are saved. Closed contracts stay `closed_keep_days` for the biography and the Board panel's history, then drop; `World::contract_log` keeps a ring of 512 one-line outcomes.

### Price, risk and posting

`contracts::post(world, Posting) -> Result<ContractId, Refusal>` is the one entry point for brains, agents, god commands and, later, the player. The price is `price = price_base[kind] × (1 + risk_w × risk) × (1 + standing_w × standing(target)) × price_level_b`, where `risk` comes from the same estimate the strike uses (§ 3) against a median gun (fighting 0.5, Kit 0), `standing` is M15's, and `price_level_b` is the broker's (1.0 direct). The buyer must hold `price` in its purse; a brokered post moves it to `escrow` (`Flow::Escrow`), a direct post moves nothing. `Hired` is posted as a first-hand deed memory in the placing agent (and in the Fixer on a brokered post), so the knowledge exists from the first tick. `renege = hash(seed, id) < renege_base × (1 − lawfulness) × (1 − honour)` of the placing agent on direct posts only.

### Who posts what

Brains' orders and personal goals become contracts only where this table says; everything else keeps its M8–M15 rule.

| Source | Trigger | Contract |
| --- | --- | --- |
| Hunt (M15) | an eligible grudge whose Hunt scores with `might_gap < hire_gap`, holder's coins ≥ price; Statistical holders in the M15 daily pass | `Hit` when `weight ≥ lethal_min`, else `Beat`, target the grudge target |
| Court | a suitor rejected twice (`Rejected` memories) with coins ≥ price and sociability < 0.4 | `Persuade` (Charm on the courted, stake: affinity toward the buyer) |
| Bury | a kin corpse unburied 24 h, no living adult kin at Full or Coarse, coins ≥ price | `Deliver { Corpse → Recycler }` |
| Gang Raid / Retaliate | `ratio(own, rival) < hire_ratio` and treasury ≥ price | `Hit` on the rival leader (direct to a lieutenant's edges, or brokered when a Fixer is in a held district) |
| Gang BreakOut | the same deficit | `Rescue { the boss at the Precinct }`: the crew joins the breach as raiders |
| Gang Harvest (M13) | fit members < 3 | `Abduct` of the best visible-chrome target, price `abduct_share × chrome_value` |
| Gang vendetta (M15) | an open vendetta with no fresh sighting of the rival leader | `Locate` bounty on the rival leader |
| Corp Secure | a building with a loss in 7 days, no affordable guard contract | `Guard` on the building; `Locate` on the culprit when one is named |
| Corp Lobby | exec lawfulness < `dirty_lobby` | `Beat` (or `Hit` below `dirty_lobby / 2`) on the culprit gang's leader instead of the bribe |
| Corp Acquire | the weakest rival refuses the offer (M15 honour or not bankrupt) | a `Coercion` over its exec (§ 4), demand `Sell(building)` |
| Corp Research (M14) | no own runner, a rival Lab in `focus` with Data | `Steal { Node }`: a heist by a freelance runner, payload to the buyer |
| Corp Spin (M15) | while held | `Spin` (planting or burying), taken by the Feed; `fabricated` per § 4 |
| Law | a wanted suspect unlocated ≥ `law_bounty_days`, or an escaped convict | public `Locate` (buyer `None`, visible to everyone); a captain with lawfulness < `death_squad_lawfulness` posts `Hit` on the highest-heat gang leader |
| Board | § 6 campaign | `Vote` and `Persuade` |
| Any adult | § 5 subscription rule | `Trauma` |

## 2. Fixers, the board, hired guns

### The Fixer

`BuildingKind::Fixer` (appended; label "Fixer", letter `X`), `[buildings] fixer = { capacity = 8, stock_cap = 0, staff = 1 }`, foundable through `Register` (`found_cost.fixer` 400, `value` 500, upkeep 8), hired `Role::Fixer` (label "Fixer") sorted by persuasion + knowledge. An agent founds one through M11's `Found` goal when `lawfulness < fixer_lawfulness` and `persuasion + knowledge ≥ fixer_skill`; two are seeded by `founding::build_on_lot` on Lots in Mid East and Sump Central, owned by the jobless adult with the lowest lawfulness living nearest, as M11 dealt Bars.

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Broker {                            // on the Fixer building
    pub book: Vec<ContractId>,                 // open and taken, sorted
    pub regulars: BTreeMap<EntityId, Tick>,    // last Network visit
    pub cut: f32,                              // fixer_cut at founding
    pub heat: f32,                             // 0..=1
    pub closed_until: Option<Tick>,
    pub income: VecDeque<i64>,                 // per day, last 14
}
```

**Regulars.** `Network` (new `ActionKind`, dur 30, at a Fixer) marks the agent a regular for `regular_days`. Earn gains the option `GoTo(Fixer) → Network` for adults who are jobless or under-paid and have fighting, stealth, hacking, persuasion or intimidation ≥ `gun_skill_min`, with `1 − lawfulness` → Linear{0.6,0.4}; a gang member networks for their gang. Statistical regulars are written by the daily pass for adults whose Trace shows them in the Fixer's district and who pass the same gate, on one hash-picked day in seven.

**Visibility: the board.** `contracts::visible(world, viewer) -> Vec<ContractId>` is what the Board panel, the Contract goal and M18's quest log read. A brokered contract is visible to the regulars of its Fixer and to gangs holding or adjacent to the Fixer's district; a direct contract to the buyer's edges (any `RelKind` but Enemy), the buyer's gang or corp staff; a public contract (buyer `None`) to everyone; `Guard`, `Escort` and `Trauma` to Security corps; `Spin` to Feeds. God mode sees all.

### Matching (daily, at `match_hour`)

Each open Fixer ranks its book by `price × (1 + age_days ÷ deadline_days)` and makes up to `offers_per_day` offers: for each contract, the best eligible taker by `fit = skill[kind] × (1 − risk_for(taker)) × (0.5 + 0.5 × honour(taker))`, where `skill[kind]` is fighting (Hit, Beat, Abduct, Rescue, Escort, Guard), stealth (Steal, Tag, Locate), hacking (Steal on a Node), persuasion or intimidation (Persuade, Vote), and `risk_for` the strike estimate with the taker's own `fi`. Eligibility: a regular, not the target, not the target's Spouse, Family or Friend, `lawfulness ≤ gun_lawfulness[kind]` for an illegal kind, free and not jailed, not already holding a contract. The taker **accepts** when the Contract goal's score (below) ≥ `accept_min`; a refusal moves the offer to the next candidate. A gang (through its leader's brain) and a Security corp are candidates too, scored by their best member. On acceptance: `status = Taken`, `taker`, a crew of up to `crew_max` of the taker's gang members or Friends who are regulars when `risk > squad_below`, `known_by` += taker and crew, `ContractTaken` event. The Fixer's cut is paid at fulfilment, never at acceptance.

### The Contract goal

| Goal | Considerations | Plan |
| --- | --- | --- |
| Contract | `Can(holds a Taken contract ∧ free)` → GATE; `U(price ÷ week_wage)` with `week_wage = max(7 × wage, 50)` → Logistic{8,0.3}; `1 − risk_for(self)` → Linear{0.7,0.3}; buyer honour (direct only) → Linear{0.5,0.5}; `1 − lawfulness` (illegal kinds) → Linear{0.6,0.4}; `courage` (violent kinds) → Linear{0.5,0.5}; urgency `1 − time_left ÷ total` → Linear{0.5,0.5}; flat `contract_flat` | per kind, below |

`GoalKind::Contract` sits after `Hunt` in `GOAL_ORDER`. Plans reuse existing chains:

| Kind | Plan |
| --- | --- |
| Hit, Beat | M15's Hunt chain without a grudge: `[AskAround] → GoTo(Intel) → StakeOut → Attack(target)`, lethal per kind; a squad goes as a mission (§ 3) |
| Abduct | `GoTo(Intel) → StakeOut → Abduct` (M13) `→ GoTo(Holding) → HandOver` |
| Rescue | a mission on the holding building (§ 3); the freed captive follows the crew home |
| Escort, Guard | `GoTo(Client or Building) → Guard(hours)`: stand within 2 tiles, defend in any brawl or assault on the client (the defender list of `fight_out` and `Attack` gains guards on contract) |
| Steal | a building: M13's theft family on the target; a Node: M14's Hack with `patron = buyer` and the payload delivered to the buyer's store |
| Deliver | `GoTo(Cargo) → PickUp / CarryCorpse → GoTo(To) → HandOver` |
| Locate, Tag | `[AskAround] → GoTo(Intel) → StakeOut` writes the sighting; Tag adds `PlantTag` |
| Persuade, Vote | `GoTo(Intel) → Coerce` (§ 4) with the contract's stake |

**Settlement** (`contracts::settle`): Fulfilled pays `escrow × (1 − cut)` to the taker (`Flow::Payout`, split equally with the crew) and `escrow × cut` to the Fixer's owner (`Flow::FixerCut`). A direct contract pays from the buyer's purse unless `renege`, in which case it is `Reneged`: a `Betrayed` deed (actor buyer) posted at `reach0.betrayed`, and the taker gains `Grudge { cause: Betrayed, weight: renege_grudge }`, which M15's Hunt reads. A taker who dies, is jailed, or loses the strike fails the attempt: back to `Open` with `attempts + 1` until `max_attempts`, then `Failed`; at `deadline` `Expired`. Both refund escrow (`Flow::Escrow`, reversed). `ContractPosted`, `ContractTaken`, `ContractFulfilled`, `ContractFailed`, `ContractExpired`, `Reneged` events.

### Hired guns at the Statistical tier

A Statistical taker is offered and accepts exactly as a Full one (the score reads Personality, Skills and purse). If the target or any crew member is Full or Coarse, pinned, or in view, the contract is `Render::Live` and the taker is promoted to Coarse with Contract pinned, bounded by `max_missions` (queued by price). Otherwise it is `Render::Ledger` with `due = posted + hash(seed, id) % (deadline − posted) ÷ 2`. Promotion of either party before `due` turns a ledger contract live.

```toml
[contracts]
enabled = true
max_open = 256
closed_keep_days = 30
price_base = { hit = 400, beat = 120, abduct = 300, rescue = 350, escort = 80, steal = 150, deliver = 40, locate = 20, tag = 150, persuade = 100, vote = 250, spin = 120, guard = 25, trauma = 35 }
deadline_days = { hit = 10, beat = 7, abduct = 10, rescue = 7, escort = 3, steal = 7, deliver = 3, locate = 14, tag = 10, persuade = 7, vote = 14, spin = 5, guard = 7, trauma = 7 }
risk_w = 1.5
standing_w = 1.0
max_attempts = 2
accept_min = 0.35
contract_flat = 0.0
renege_base = 0.3
renege_grudge = 0.6
hire_gap = -0.1
hire_ratio = 0.7
abduct_share = 0.3
dirty_lobby = 0.3
law_bounty_days = 3
death_squad_lawfulness = 0.2
[fixers]
fixer_cut = 0.2
fixer_lawfulness = 0.35
fixer_skill = 0.8
regular_days = 14
gun_skill_min = 0.45
gun_lawfulness = { hit = 0.3, beat = 0.4, abduct = 0.3, steal = 0.4, tag = 0.5, persuade = 1.0, vote = 0.6, rescue = 0.7 }
match_hour = 18
offers_per_day = 4
crew_max = 4
[buildings]                        # addition
fixer = { capacity = 8, stock_cap = 0, staff = 1 }
[corps]                            # additions
found_cost = { fixer = 400 }
value = { fixer = 500 }
upkeep = { fixer = 8 }
```

## 3. Missions, hit squads and the law

### The mission

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mission {
    pub contract: ContractId,
    pub crew: SmallVec<[EntityId; 6]>,      // the taker first
    pub muster: TilePos,                     // the crew's nearest held Home, squat or the Fixer
    pub door: TilePos,                       // the target's intel tile or the building's door
    pub raid_at: Tick,
    pub stream_by: Option<EntityId>,         // M14 Overwatch
    pub defenders_hint: u8,                  // allies expected at the door, for the panel
}
```

`raid.rs` is factored, not forked: `Expedition` (a trait over `&Gang` and `&Mission` with `door`, `raid_at`, `crew`, `is_live`) feeds `raiders_at`, `depart`, and a `fight_out` that already takes two lists. The gang path keeps its code and calibration. A mission's plan is `GoTo(MusterPoint) → Muster → GoTo(MissionDoor) → Brawl`, the same actions with `LocationKey::MissionDoor`. **Defenders** at a person: the target, their Escort and Guard contractors within `raid_gather_radius`, gang members and Friends within it whose courage ≥ 0.5, the building's powered robots and private guards when the door is a building's. A Rescue's defenders are the holding gang's members inside or the Prison's guards and robots. M14's door, robot and camera runs apply to a mission's building door as to a raid. Crossfire (M12) applies.

### The strike decision

At `Muster` the leader of the crew re-reads the freshest intel (M15: own and buyer's `FactionDb` sightings, Locate streams, tags) and decides:

- `p_win` from the brawl estimate: crew `Σ strength` against the expected defenders' `Σ strength` (the same `strength = fi + 0.25 × courage` as `by_strength`), `p_win = logistic(strike_k × (S_crew ÷ max(S_def, 0.1) − 1))`.
- The **controller** `C` of the door's district (M12). `political = 0` when `C` is the buyer, the taker's gang, `City` or `Contested`; else `political = pol_w × control_share × (0.5 + 0.5 × Regard(C, buyer).fear) × (1 + max(0, Regard(C, buyer).value))`, M15's Regard. A target who is a member or employee of `C` doubles it.
- **Strike** when `p_win × price − (1 − p_win) × price × loss_w − political × price ≥ 0`; else **hold** a day (re-read intel) up to `hold_days`, then **sell out** or fail.
- **Sell out.** When `C` is a gang, the target is not its member, and `Regard(C, target's faction).value < sell_regard`, the contract is offered to `C` at `price × sell_premium` (the buyer tops up escrow or the offer lapses); `C`'s leader accepts with `L.greed` → Linear{0.6,0.4}, `1 − L.lawfulness` → Linear{0.5,0.5}, `fear` of the buyer → Linear{0.5,0.5} ≥ `accept_min`, and `C` executes it under `Order::Job`. `SoldOut` event.

A strike in another faction's district pushes `Shock::Trespass { by }` (0.4) on a gang `C` or `CorpShock::Trespass` (0.3) on a corp `C`, and a `Raided` deed (actor the buyer's faction) into the district pool: the political cost is paid in Regard and vendetta weight, through M15.

`Order::Job` (tenth gang `Order`, `is_raid()` true when the job is a mission): `Can(a Taken contract held by the gang)` → GATE; `price ÷ hoard_heat` → Logistic{8,0.3}; `L.greed` → Linear{0.5,0.5}; `1 − target_cover` → Linear{0.8,0.2}; flat `order_flat.job` (0.0). It holds until the contract settles.

### Bounties, tags and scanners

`Locate` is a stream: every `Sighting` of the target written by a viewer of the contract (§ 2 visibility) is copied to the buyer's `FactionDb`, and the writer is paid `per_sighting` from escrow, at most one paid sighting per `min_gap_hours` and `cap_sightings` in all (`BountyPaid` event; `Flow::Payout`). A public bounty by the law is paid by the Treasury and sets `law.last_seen`.

`Tag` plants `AssetKind::Tag` (M13 asset, tier 1–3, sold by Security Offices at `tag_price[tier]`) with `PlantTag` (new `ActionKind`, dur 2, adjacent): `security::contest(stealth tier of the taker, sight tier of the target)` where the target's tier is `1 + Eyes tier`. On success the tag is an `Asset` with `loc = On(target)`, owner the buyer, and writes `Sighting { conf: tag_conf[tier] }` to the buyer's db once an hour for `tag_battery_days`. `AssetKind::Scanner` (tier 1–3, `scanner_price`) is posted at Clinics and Security Offices; `Scan` (new `ActionKind`, dur 10, `scan_price`) at a building with a scanner contests `scanner tier` against `tag tier`, a win destroys every tag on the agent (`TagFound` event, a `Tagged` memory naming the tag's owner when the contest margin is ≥ 1). Adults with `heat ≥ scan_heat` or a hunter on them add Scan to the Shop goal. Tags are capped at `max_tags` city-wide; the hourly write is O(tags).

### The ledger resolution

At `due` a ledger contract draws once from `SimRng::contract(id)` with the strike's `p_win` (defenders from the target's Trace habit: Home at night, else the workplace). Hit: a win kills the target (`demography::kill`, `DeathCause::Violence`) and opens a pre-bound `HoleKind::Killed` hole with `contract = Some(id)`; Beat opens `Assaulted`; Steal on a building opens `Robbed` with `loot` = the take; Locate pays `cap_sightings ÷ 2` sightings; Persuade and Vote resolve the social move at any tier (no body needed, as M15 poaching). A loss is a failed attempt; when the target's might wins by > 0.3 the taker dies (`p_win < ledger_death_p` of the draw), opening a `Killed` hole on the taker pre-bound to the target. Abduct, Rescue, Escort, Guard, Trauma and squads are **never** ledger: they promote.

### The law's view

A fulfilled Hit is a `Murder` by the taker under the normal report, witness and arrest rules; a Beat an `Assault`. Nothing about the contract is visible to a witness.

- **The `Hired` deed** (appended to M15's `Deed`; `reach0 = 0.3`, `deed_sal = 0.7`, `deed_sev = 0.7`, `heat_w = 0.4`, `honour_w = −0.2`) is known first-hand to `known_by`. It travels by gossip; a Fixer speaking at Drink passes `Hired` with `p = fixer_talk × (1 − Fixer heat)` only to other regulars. Kin of the target who learn `Hired` form a grudge on the buyer (`GrudgeCause::Hired`), so revenge can reach the buyer.
- **Interrogation.** When a taker of a fulfilled Hit or Beat within `interrogate_days` is jailed for any crime, the arresting guard makes an Intimidate move (M15 `resolve`, guard `skill_a = intimidation + 0.3 × Law.competence`, `bias = interrogate_bias`) against the taker; success puts the buyer and the Fixer in the guard's memory as `Hired` at `hops = 1, conf = 1`.
- **Conspiracy.** `law::accessory_check`, daily: for every open Murder or Assault report whose victim was the target of a fulfilled contract, if any guard or the report's witness holds `Hired` about that victim with `conf ≥ accessory_conf`, file `Crime::Conspiracy` (label "Conspiracy", severity just below Murder, sentence `accessory_mult × sentence(Murder)`) against the placing agent (`Contract.agent`; a corp's exec, a gang's leader, the captain when the law posted it). `Accessory` event. M15's street silence applies to witnesses, not to guards.
- **The Fixer's heat** rises `heat_per_hit` per fulfilled Hit and `heat_per_named` each time an interrogation names the Fixer, decays `heat_decay` a day. At `warrant_heat` the owner is a Conspiracy suspect and the office closes for `close_days` (`closed_until`, `FixerBusted`), with its book's open contracts refunded. A Fixer may bribe: M9's `offer_bribe` with `Payer::Owner`, scored with heat → Linear{0.8,0.2}, lowers its heat by `bribe_cool` on a take.

```toml
[missions]
max_missions = 12
squad_below = 0.6
strike_k = 3.0
loss_w = 0.5
pol_w = 1.0
hold_days = 2
sell_premium = 1.5
sell_regard = 0.0
ledger_death_p = 0.15
[bounty]
per_sighting = 15
cap_sightings = 10
min_gap_hours = 6
tag_price = [0, 150, 400, 900]
tag_conf = [0.0, 0.7, 0.85, 0.95]
tag_battery_days = 10
max_tags = 32
scanner_price = [0, 300, 800, 2000]
scan_price = 40
scan_heat = 0.4
[law]                              # additions
interrogate_days = 30
interrogate_bias = 0.0
accessory_conf = 0.5
accessory_mult = 0.75
heat_per_hit = 0.15
heat_per_named = 0.3
heat_decay = 0.02
warrant_heat = 0.8
close_days = 7
bribe_cool = 0.3
fixer_talk = 0.3
[gangs.order_flat]                 # addition
job = 0.0
```

## 4. Leverage

### Data model

```rust
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Leverage {
    Threat,                                         // credibility = might_gap and dread, M15
    Secret { deed: Deed, actor: EntityId, object: Option<EntityId>, day: u32 },
    Hostage { captive: EntityId },
    Debt { amount: i64 },                           // an Edge.debt called in
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Demand {
    Coins { amount: i64, weekly: bool },            // a shakedown, protection, hush money, a ransom
    Do(ContractId),                                 // a coerced job (Terms::Coerced)
    Sell(EntityId),                                 // a building to the holder at value
    Join { building: EntityId, wage: i64 },         // poaching by threat
    Vote { corp: EntityId, for_: EntityId },
    Release(EntityId),                              // a captive or a convict (via the captain)
    Silence { report: u64 },                        // withdraw testimony: the report loses its witness
    Access(EntityId),                               // a door: DoorOpen for door_hours (M14's effect)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Coercion {
    pub id: CoercionId,
    pub holder: Option<EntityId>,                   // an M11 owner
    pub agent: EntityId,                            // who made the move
    pub over: EntityId,                             // the coerced agent (an exec or leader for a faction)
    pub leverage: Leverage,
    pub demand: Demand,
    pub since: Tick, pub until: Tick, pub next_due: Tick,
    pub paid: u16,
    pub status: CoercionStatus,                     // Active, Complied, Refused, Broken, Lapsed, Exposed
}
```

### The move

`leverage::coerce(world, agent, over, leverage, demand) -> CoercionOutcome` is M15's `social::resolve` with `MoveKind::Intimidate` and two additions to the logit: `+ lev_bias[leverage]` and `− demand_cost`, where `demand_cost = demand_w × value(demand) ÷ max(purse(over) + 7 × wage(over), 1)` (a ransom a family cannot pay is refused). A Secret adds `secret_w × deed_sev × (1 − reach)`; a Hostage adds `hostage_w × rel_w(over, captive)` (M15's relation weights). Success makes the demand happen (coins move with `Flow::Coerced`; `Do` sets the coerced taker; `Sell` transfers at `value`, `Acquired` event; `Join` is M15's poach; `Vote` pins the member's vote; `Silence` marks the report `withdrawn`, so it no longer counts toward the warrant; `Access` sets `DoorOpen`). Failure is M15's backlash, plus: the over files a report (`Crime::Extortion` for coins and threats, `Crime::Blackmail` for a secret) with `p = backlash_report × lawfulness`, and a holder of a weekly coercion that is refused loses it (`Refused`). `Coerced`, `CoercionBroken` events; the deeds `Extorted` and, new, `Blackmailed` (`reach0 = 0.2`, `deed_sev = 0.4`, `honour_w = −0.4`) go to the pools at the usual reach.

| Move | Who uses it, when |
| --- | --- |
| **Shakedown** (the M8 Home extortion) | `gang::extort` calls `coerce(Threat, Coins { extort_amount, weekly: false })` against the strongest occupant; the claim counting runs on success as today. The M15 Intimidate roll is this roll. |
| **Protection** | a gang under Expand or Contest, once a week per held district, targets the agent- or corp-owned Bar, Market, Hotel, Clinic or Fixer with the highest revenue in it: `Coins { protection_weekly, weekly: true }`. Paid weekly from the owner's purse until refused, the gang loses the district, or the agent is jailed. A corp owner pays only when its Security cover of that building is 0. |
| **Threat** | a coerced job: a gang leader or exec who cannot afford a contract's price makes it `Terms::Coerced` on a weaker regular; a suspect with an open report threatens its witness with `Silence` when `lawfulness < silence_lawfulness`. |
| **Blackmail** | any adult with `lawfulness < blackmail_lawfulness` holding a secret on someone richer (`standing_t > standing_a + 0.2`) makes it once a month: `Coins { blackmail_weekly, weekly: true }`, or `Vote` and `Sell` from a board campaign or Acquire. A secret is a deed with `deed_sev ≥ 0.4` or `honour_w < 0` held first-hand or at `hops ≤ 1` whose holder count (M15's daily pass tallies the keys referenced by active secrets) is ≤ `secret_known_max`. It **lapses** (`Lapsed`, no backlash) when any pool entry of that deed reaches `secret_public_reach`: the secret is out. |
| **Poaching by threat** | M15's poaching step, for a corp with exec lawfulness < `poach_threat_lawfulness`, uses `Threat` or a Secret with `Join` instead of the wage premium. `Poached` deed as M15. |
| **Debt** | a lender (M15 `Edge.debt`) whose debtor has missed `debt_days` calls the debt as `Coins { amount: debt }`; failure is backlash, success settles the edge. |

### Fraud

- **Reneging** (§ 2) is fraud. The street does not go to the law, it hunts: a Reneged taker files `Crime::Fraud` (label "Fraud", severity just above Theft, `sentence_hours.fraud` 48) against the placing agent only when the taker's lawfulness ≥ 0.5, and otherwise keeps the grudge.
- **Fabricated stories.** A Spin contract with `fabricated = true` (posted when the buyer's exec or leader has `lawfulness < fabricate_lawfulness` and no true rival misdeed scores, price × `fabricate_mult`) plants a deed that never happened. The Feed's reporters contest it with a Deceive move by the Fixer or exec against their mean knowledge; a Feed that sees through it refuses and the attempt is a `Betrayed` deed against the buyer. A planted fabrication carries `Story.fabricated` (hidden); each day each holder of it whose own first-hand memories contradict it (they were the object, or a witness of the object elsewhere that day) exposes it with `expose_p`. **Exposed**: `Exposed` event, a `Betrayed` deed against the buyer at `reach0 = 1.0`, `Crime::Fraud` on the placing agent, and the Feed's reach × 0.8 for 14 days.
- **The fake job.** A regular with deception ≥ 0.6 and lawfulness < 0.3 may sell a non-existent contract to another regular for `fake_fee` (a Deceive move; success takes the fee, `Flow::Coerced`; the victim learns it on the deadline: `Betrayed` deed, grudge `Betrayed`).

### Hostages and the private prison

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Captive { pub holder: Option<EntityId>, pub held_at: EntityId, pub since: Tick, pub coercion: Option<CoercionId>, pub until: Tick }
```

`Captive` on the held agent (Full or Coarse, never demoted while held) pins `ServeTime` at the holding building as `Sentence` does, counts against its capacity, and costs the holder `hostage_food` coins a day (`Flow::Upkeep`). Holding buildings: a gang's Hideout, a squat, and the new **`BuildingKind::Prison`** (label "Private Prison", letter `Q`, `[buildings] prison = { capacity = 20, stock_cap = 100, staff = 4 }`, Security niche, `found_cost.prison` 800, `value` 1200, upkeep 20, staffed by `Role::Guard` with the corp's Security Office as employer, never foundable by an agent). Taking a hostage is an `Abduct` (M13: `resolve_fight` with abductors summed, a carry, `Crime::Abduction`, `Abducted` event) by the holder's members or by an Abduct contract; on `HandOver` the `Captive` is added and a `Coercion { leverage: Hostage, over }` is made at once, `over` chosen as the captive's employer's exec, gang leader, or the living kin with the largest purse, in that order, with the holder's demand. The coercion is re-rolled daily until `until = since + hostage_days`; on `Complied` the captive is released at the door (`HostageFreed`); on expiry the holder releases or kills with `p_hostage_kill × (1 − holder lawfulness)` (`HostageKilled`, a Murder by the holder's agent).

**Who takes hostages.** A gang holding Retaliate or Job may take the rival leader's Spouse or child instead of raiding when `ratio < hire_ratio`, demand `Release` of its convicts (the captain complies when `Law.competence < 0.4` and lawfulness < 0.5) or `Coins`. A corp under Acquire whose seller refused twice, exec lawfulness < `dirty_lobby`, with a Prison or a Hideout-holding gang under contract, abducts the seller exec's kin, demand `Sell`. Corps build a Prison under Secure when holding a hostage with nowhere to put it and cash ≥ 0.8.

**Rescue.** The `over` of a hostage coercion, or the captive's gang, posts `Rescue` (or the gang raids directly: M8 Raid with the holding building as `raid_target`, as M12's corp raids). The mission's win frees every captive of that holder inside (`HostageFreed`, `Rescued` deed with the crew as actors, honour +), and the holder's members inside are beaten per `fight_out`. A captive whose `until` passes during a live Rescue's muster waits for it.

**Abduction for the factory.** `Deliver { Captive → building }` to an M13 Hideout under Harvest is the scav economy's input today; the goods milestone's Soylent factory reads the same contract kind and needs no change here.

```toml
[leverage]
max_coercions = 128
lev_bias = { threat = 0.0, secret = 0.3, hostage = 0.5, debt = 0.2 }
demand_w = 1.0
secret_w = 1.0
hostage_w = 1.5
backlash_report = 0.5
protection_weekly = 40
blackmail_weekly = 30
blackmail_lawfulness = 0.35
secret_known_max = 5
secret_public_reach = 0.3
silence_lawfulness = 0.3
poach_threat_lawfulness = 0.3
debt_days = 14
fabricate_lawfulness = 0.35
fabricate_mult = 2.0
expose_p = 0.05
fake_fee = 40
[hostage]
hostage_days = 7
hostage_food = 3
p_hostage_kill = 0.3
[buildings]                        # addition
prison = { capacity = 20, stock_cap = 100, staff = 4 }
[corps]                            # additions
found_cost = { prison = 800 }
value = { prison = 1200 }
upkeep = { prison = 20 }
[law.sentence_hours]               # additions
blackmail = 72
fraud = 48
```

## 5. The wounded, Rescue, Trauma Team

### Wounded

`law::resolve_fight` keeps its formula and M13's chrome terms. When the death roll hits, a second draw makes it a wound with `p = wound_share × (1 − K.armour_loser)` instead; a robot is never wounded. The wounded agent gets

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wounded { pub by: Option<EntityId>, pub since: Tick, pub bleed_until: Tick, pub tile: TilePos, pub stable: bool }
```

pins `ExecState::Down` (no action, cannot be robbed of anything but its Wallet, carried as a corpse is carried: M13's `exec::follow_bearer`), is at least Coarse while down, and writes a `Sighting` of itself at its tile to its gang's and employer's `FactionDb` and, if it holds a Trauma subscription, to the Security corp's (conf 1.0, the beacon). At `bleed_until` an unstabilised wounded dies by `World::kill_by(…, Violence, by)`: the report becomes a Murder, the Murder count is unchanged from the M15 rule, and `BledOut` fires. `Stabilise` (new `ActionKind`, dur 30, at a Clinic with a Ripperdoc inside, `stabilise_price` paid by the carrier's faction, the wounded or the subscription; `Flow::Treatment`) sets `stable`, and the agent rises after `recover_days` with energy and safety at 0.2 (`Recovered` event). The witness, report and memory rules are the M12 Assault rules at the wound and the Murder rules at a bleed-out; a wound is never reported twice.

### Rescue

`Temperament { Abandon, Loyal, Recover }` on `Gang` (`#[serde(default)]` Loyal; The Unplugged and any gang whose leader's loyalty < 0.3 seed Abandon); corps and the law are `Recover`. **Rescue** (Full and Coarse goal): `Can(a Down agent within rescue_radius who is a Spouse, Family, Friend, fellow member, colleague, or a guard's arrestee, unstable ∧ a Clinic reachable before bleed_until)` → GATE; `rel_w` or faction term (Recover 1.0, Loyal `loyalty`, Abandon 0) → Linear{0.8,0.2}; `courage` → Linear{0.4,0.6}; `1 − danger` (an attacker within 4 tiles) → Linear{0.5,0.5}. Plan `GoTo(Wounded) → CarryWounded → GoTo(Clinic) → Stabilise`. A gang member with `loyalty ≥ rescue_loyalty` rescues under Loyal. City guards carry their own and their arrestees. `Rescued` event and deed (honour +0.3 via `honour_w.rescued`).

### Trauma Team

`ContractKind::Trauma` is a standing contract (`Standing`) between a client (buyer, the person) and a Security corp (taker), renewed weekly at `fee_weekly × price_level[Security]` (`Flow::Trauma`, taxed as owner revenue); unpaid, it lapses. Subscribers: every corp exec and board member (paid by the corp), and once a day a Full or Coarse adult (a Statistical one in the daily pass) with coins ≥ `subscribe_coins` and `heat ≥ subscribe_heat`, or an open hunt or contract against them, subscribes with `p_subscribe`. Clients choose the Security corp minimising `price_level × (1.5 − honour)` (M15's rule for guard contracts). **The call**: on a client's wound, the beacon sighting goes to the corp's `FactionDb`; the corp sends `trauma_crew` on-shift guards from its nearest Security Office as a mission whose door is the **last sighting** of the client (not the true tile: a client carried off by an abductor or a rescuer is met where the db says), plan `GoTo(Door) → CarryWounded → GoTo(Clinic) → Stabilise`, defenders fought through `fight_out` if the attacker is still there. A client stabilised before `bleed_until` is a `TraumaSave`; the corp pays `stabilise_price` (no extra fee). Off screen a subscriber's StatTable `p_killed` is multiplied by `1 − trauma_stat_save`; nothing else changes the table.

```toml
[wounded]
wound_share = 0.6
bleed_hours = 3
stabilise_price = 80
recover_days = 3
rescue_radius = 12
rescue_loyalty = 0.5
[trauma]
fee_weekly = 35
trauma_crew = 2
subscribe_coins = 400
subscribe_heat = 0.3
p_subscribe = 0.2
trauma_stat_save = 0.5
[reputation]                       # additions (M15 tables)
honour_w = { rescued = 0.3, hired = -0.2, blackmailed = -0.4 }
heat_w = { hired = 0.4 }
dread_w = { hired = 0.3 }
```

## 6. Governance and the vote

`Governance::Board { members }` (the M11 D49 hook) becomes `Board { members: Vec<EntityId>, chair: EntityId, last_vote: Tick, votes: SmallVec<[(EntityId, EntityId); 8]> }`. `[governance] boards` names the seed corps with boards (Habitat and Nutrix): the exec as chair plus the `board_size − 1` employees with the highest standing; a dead or departed member is replaced by the next. A board's brain reads the **mean Personality** of its members, weighted 2 for the chair, instead of the exec's.

**The vote** runs every `vote_days` and at once on the chair's death. Candidates are members with `greed × standing ≥ ambition_min` plus the chair. Each member votes for the candidate maximising `pref = 0.4 × affinity + 0.3 × standing + 0.3 × (honour − 0.5) × 2` (M15) unless a `Vote` coercion pins them. Ties go to the chair. A new chair becomes `Corp.exec`; `VoteHeld` (with the tally) and, per pinned member whose `pref` choice differs, `VoteSwung`.

**The campaign.** `campaign_days` before a vote, the strongest challenger whose projected tally loses by ≤ 2 targets the swing members (closest `pref` margin) one a day: a `Persuade` move with `Stake::Coins(vote_bribe)` when lawfulness ≥ 0.5, else a Blackmail or Threat coercion with `Demand::Vote`, or posts a `Vote` contract for a regular with persuasion or intimidation. A rival corp under Acquire may back a challenger the same way. M18's player is one more challenger.

```toml
[governance]
boards = ["Habitat", "Nutrix"]
board_size = 5
vote_days = 45
campaign_days = 7
ambition_min = 0.4
vote_bribe = 150
```

## 7. The Statistical tier and LOD

| Data and behaviour | Full | Coarse | Statistical |
| --- | --- | --- | --- |
| Posting (Hunt hires, Court, Bury) | event | event | the M15 daily pass |
| Regular at a Fixer | `Network` | `Network` | the daily pass, one hash-picked day in seven |
| Taking a contract | the Contract goal | same | accepted by score at matching; a live contract promotes to Coarse |
| Executing | exec | exec | the ledger draw at `due`, pre-bound holes |
| Coercions | the move at the encounter | same | resolved at any tier (no body needed); hostages are always Coarse |
| Wounded, Rescue | exec | exec | the StatTable `p_killed`, re-calibrated; Trauma multiplier |
| Tags, bounties | hourly sighting writes | same | tags write regardless of tier (a tag is the target's eyes on them, not theirs) |
| Board votes | any tier | any tier | any tier |

The StatTable is unchanged in shape; `calibrate` is re-run for `Network`, `Stabilise`, `CarryWounded`, `PlantTag`, `Scan` and the Wounded split of `p_killed`.

## 8. Player levers and god commands

| Command | Effect |
| --- | --- |
| `SetFixerLicence(bool)` | Off: Fixers are illegal; every open office has `heat` floored at 0.5 and new ones cannot `Register`. |
| `SetAccessoryMult(f32)` | The Conspiracy sentence multiplier. |
| `PublicBounties(bool)` | The law posts `Locate` on every wanted suspect after one day instead of `law_bounty_days`. |
| `SetPrisonPermit(bool)` | Off: corps cannot build Prisons; existing ones hold no new captives. |

CLI: `--lever day:SetFixerLicence:false`. **God commands**: `PostContract { buyer, kind, target, price, brokered, deadline_days }` (logged; `Origin::God`); `TakeContract { contract, taker }` (forces acceptance; M18's `PlayerCommand::TakeContract` is the same call); `Hostage { captive, holder, at, demand }`; `Wound { agent, by }` (the Wounded state at once, bleed timer running); `Subscribe { agent, corp }`; `TagAgent { agent, owner, tier }`; `Coerce { agent, over, leverage, demand }` (forced success); `CallVote(corp)`; `ExposeSecret(coercion)` (posts the deed at reach 1.0).

## 9. UI, events, CSV

New `EventKind`s (amber): `ContractPosted`, `ContractTaken`, `ContractFulfilled`, `ContractFailed`, `ContractExpired`, `Reneged`, `SoldOut`, `StrikeDeclined`, `BountyPaid`, `Tagged`, `TagFound`, `Accessory`, `FixerBusted`, `Coerced`, `CoercionBroken`, `Lapsed`, `Exposed`, `HostageTaken`, `HostageFreed`, `HostageKilled`, `Wounded`, `Rescued`, `BledOut`, `Recovered`, `TraumaCall`, `TraumaSave`, `VoteHeld`, `VoteSwung`. New `Deed`s `Hired`, `Blackmailed`, `Rescued`; `GrudgeCause::Hired`; `GoalKind`s `Contract`, `Rescue`; `ActionKind`s `Network`, `HandOver`, `Guard`, `PlantTag`, `Scan`, `Coerce`, `CarryWounded`, `Stabilise`; `Order::Job`; `Shock::Trespass`, `CorpShock::Trespass`; `Crime`s `Conspiracy`, `Blackmail`, `Fraud`; `BuildingKind`s `Fixer`, `Prison`; `Role::Fixer`; `AssetKind`s `Tag`, `Scanner`; `ExecState::Down`; `Flow`s `Escrow`, `Payout`, `FixerCut`, `Coerced`, `Trauma`.

- **Board panel** (new, `J`): open, taken and standing contracts with kind, buyer (or "anonymous" unless the viewer is in `known_by`), target, price, broker, deadline, taker, render tag (LEDGER, LIVE, and the M14 stream); filters by viewer (god, a selected agent's `visible` set: the M18 quest log). History from `contract_log`.
- **Mission panel**: the crew, the door, the strike decision's terms (`p_win`, controller, political cost), the live brawl through M14's `MissionView`.
- **Inspector**: an agent's contracts on both sides, coercions held and suffered, Captive and Wounded blocks, Trauma subscription, tags on them (god only).
- **Building panel**: a Fixer's book, regulars, cut, heat, income; a Prison's captives.
- **Corp panel**: board, chair, last tally, campaign.
- **City panel**: a Contracts section (open by kind, fulfilled today, hits, hostages held, wounded down, Fixers and heat) and the levers.
- **Map overlay** (`J`): a glyph on each Fixer sized by book, a crosshair on each live mission door, a pulsing cross on each Down agent, a lock on held captives.
- **CSV** (`--report`): `contracts_open`, `contracts_posted`, `contracts_fulfilled`, `contracts_failed`, `contracts_expired`, `reneged`, per kind `k_{kind}_posted` and `k_{kind}_done`, `hits_done`, `hits_squad`, `strikes_declined_pol`, `sold_out`, `contract_murders`, `contract_cleared`, `accessory`, `bounties_paid`, `tags_live`, `tags_found`, `coercions_active`, `protection_paid`, `blackmail_active`, `secrets_lapsed`, `fabricated`, `exposed`, `hostages_held`, `hostages_freed`, `hostages_killed`, `wounded`, `bled_out`, `rescued`, `trauma_subs`, `trauma_saves`, `votes`, `votes_swung`, per Fixer `f{i}_heat`, `f{i}_income`, ledger columns `flow_escrow`, `flow_payout`, `flow_fixer_cut`, `flow_coerced`, `flow_trauma`.

## 10. Save compatibility

Every new field is `#[serde(default)]`: `Hole.contract`, `Gang.temperament`, the board fields of `Governance::Board`, `Building.broker`; `World::contracts`, `coercions`, `missions`, `contract_log`, `next_contract`, `next_coercion` default empty. `Wounded`, `Captive` and `Broker` are new component stores. Every enum variant above is appended (`Order::Job` tenth, `Crime`s ordered by `severity()` as today, not by declaration). A pre-M16 save gets two seeded Fixers on the first day when Lots exist, no contracts, no boards until the first midnight (built from `[governance]`), and Loyal for every gang. Escrow is counted by `ownership::total_coins`, so the conservation test holds across save and load mid-contract. `Config::v1_profile` sets `[contracts] enabled = false`, which turns off posting, Fixers, missions, coercions beyond the shakedown roll (extortion as M15 left it), the Wounded split (`wound_share = 0`), Trauma and votes, so the v1, M8 and M9 tests run unchanged.

## 11. Testing and calibration

**Unit tests** (`citysim/tests/contracts.rs`, `missions.rs`, `leverage.rs`, `wounded.rs`, `governance.rs`): a brokered post moves the price to escrow and `total_coins` is unchanged; a fulfilled contract pays taker and Fixer at the cut; an expired one refunds; a direct contract with `renege` set posts `Betrayed` and a grudge; `visible` hides a brokered job from a non-regular and shows a direct one to the buyer's Friend; matching never offers a Hit to the target's kin; the Hunt hires when `might_gap < hire_gap`; a ledger Hit opens a pre-bound hole that binds to the taker with the same witness on save, load and re-bind; a mission's `fight_out` matches a gang raid's result for the same lists and seed; a strike in a rival-held district computes the political term and pushes `Trespass`; a gang sells out a non-member and refuses a member; a Locate pays at most once per `min_gap_hours`; a tag writes hourly and a scanner of higher tier removes it; interrogation names the buyer and the next `accessory_check` files Conspiracy; a buyer nobody has heard of is never charged; the Fixer closes at `warrant_heat`; `coerce` is order-independent; a ransom above the purse is refused; protection pays weekly and stops when the district is lost; a secret lapses when its pool reach passes the bound; a fabricated story is exposed by its object; a hostage is fed by the holder and freed by a Rescue win; a wound bleeds out at `bleed_hours` and is a Murder, a stabilised one recovers; deaths with `wound_share > 0` never exceed deaths with it 0 for the same seed; a Trauma beacon routes the crew to the last sighting, not the true tile; a vote under a pinned member records `VoteSwung`; a pre-M16 save loads.

**Scenario** (`citysim/tests/scenario.rs`, `#[ignore]` `test_m16_contracts_seed_42`): the bullets under *Goals and acceptance*, in particular ≥ 4 fulfilled kinds, a Fixer in business, an attributed hit with the buyer as accessory, a hostage and a rescue, a Trauma save, a bounty paid, a board vote swung, and Murders within 25 % of M15. **God scenarios** (`tests/god.rs`): `PostContract Hit` on each corp exec on day 10 at 2,000 coins (who takes it, does the squad strike in the Spire or sell out, does the corp's Lobby turn on the buyer); `PostContract Hit` on The Hollow's leader from Arasaka (does Ninefold, holding the district, sell him out); `Hostage` of the captain's Spouse by Ninefold demanding `Release` (does the law comply, Crush, or post a Rescue); `SetFixerLicence false` (do Fixers bribe, close, or move to the Sump); `Subscribe` every Corp-class adult and run M13's `ChromeEveryone 3` (do Trauma crews keep up); `CallVote Habitat` after `Coerce` on two members (does the chair fall, does the brain's order change with the board's Personality).

**Calibration**, tuned via `[contracts]`, `[fixers]`, `[missions]`, `[bounty]`, `[leverage]`, `[hostage]`, `[wounded]`, `[trauma]` and `[governance]` only:

| Target | Band |
| --- | --- |
| Contracts posted per 120 days | 60–300 |
| Fulfilled share of closed contracts | 35–70 % |
| Hits fulfilled per 120 days | 4–20 |
| Contract killings as a share of Murders | 5–25 % |
| Clearance on contract killings | 20–60 % |
| Fixer weekly income ÷ a Bar owner's | 0.5–2.0 |
| Active coercions on day 120 | 10–60 |
| Hostages held at once, max | 1–6 |
| Wounded ÷ (wounded + fight deaths) | 20–50 % |
| Wounded saved (rescued or Trauma) | 30–70 % |
| Trauma subscribers on day 120 | 2–8 % of adults |
| Board votes with a swung member | ≥ 1 in 120 days, ≤ 50 % of votes |

**Throughput.** Nothing runs per agent per tick. Matching is daily over ≤ 256 contracts × the Fixers' regulars (≤ 2k pairs); visibility is computed on demand for the Board panel and the Contract goal's gate; missions run on the raid machinery for ≤ 12 promoted crews; ledger contracts are one draw at `due`; coercions are ≤ 128 rolled weekly or daily; tags write ≤ 32 sightings an hour; the accessory check reads open Murder reports and the guards' memories (≤ 40 × 24) daily; secret holder counts piggyback on M15's daily reputation pass; votes are O(board²) every 45 days. The new Coarse loads (crews, hostages, the wounded, ≤ 60 at once) fit inside `max_coarse`.

## 12. Phases

1. **The board and money**: § 1 and § 2 (`Contract`, posting, escrow and the flows with conservation, the Fixer building, regulars, visibility, daily matching, the Contract goal for Hit, Beat, Steal, Deliver and Locate on existing chains, settlement, renege), Hunt hiring, the ledger resolution with pre-bound holes, `PostContract` and `TakeContract`, the Board panel and CSV. A 10-day CSV matches M15 outside the new columns. Commit.
2. **Missions and the law**: § 3 (the `Expedition` factoring of `raid.rs` with the gang path byte-identical, squads, the strike decision, political cost, `Trespass`, selling out and `Order::Job`, tags and scanners), the `Hired` deed, interrogation, Conspiracy, Fixer heat and bribery, and the brain postings of § 1. Commit. The first `Accessory` on seed 42 is the first visible M16.
3. **Leverage**: § 4 (`Coercion`, `coerce`, shakedowns through it with gang income within 10 % of M15, protection, threats and silence, blackmail with lapsing, poaching by threat, fraud and fabricated Spin, hostages, `Captive`, the Prison, Rescue missions), § 6 boards and votes. Commit.
4. **The wounded**: § 5 (`Wounded`, `Stabilise`, `Temperament` and Rescue, Trauma subscriptions and calls), the StatTable re-calibration, § 8 levers and god commands, § 9 panels and overlay. Commit. Watch it live: fight deaths must not rise.
5. **Gate and polish**: the M16 scenario and god scenarios, calibration, README and docs, then `/code-review high <base>..HEAD` and a fix commit, then push.

## 13. Risks

- **The murder market runs away.** Every M15 grudge too weak to hunt is now a buyer, every hit makes kin, and kin can buy too: price, `hire_gap` and `gun_lawfulness.hit` are the brakes, with the 25 % Murder bound as the gate. Calibrate on contract killings' share of Murders before touching the revenge rules M15 tuned.
- **The raid refactor moves calibrated gang behaviour.** `raid.rs` drives M8–M12's raids, breaches and riots. Phase 2 lands the `Expedition` factoring alone first, with a byte-identical 120-day gang CSV, before missions call it.
- **Off-screen consistency and the Wounded split.** A ledger contract must give the same answer however it is inspected, and the wound rule touches every fight. Pre-bound holes keep the binder's guarantees; `wound_share` only ever replaces deaths, and the StatTable is re-calibrated once, in phase 4, against a Full run.

## 14. Out of scope (M17 and later)

Contracts across the city boundary, a parent's `Retake` posted as contracts, outside combat maps and their transient casts (M17); the player taking and posting jobs in person, haggling over price as a dialogue render, the quest log beyond the Board panel's viewer filter, story LOD for contract parties (M18); daemons as contractors without a body (the candidate; `Contract.taker` is an `EntityId` so a daemon entity can hold one later), the Blackwall; Power and Water as goods, the Soylent factory as a building (it reads `Deliver { Captive }`), map layers, interiors and room-level prisons; courts and trials, insurance, outsourcing the law's convicts to private prisons, bounty hunters as a profession beyond Locate, and a contract market in Data beyond Steal and Deliver.

## Addendum (2026-10-05, Dylan): attention and distraction

Spec amendment; decisions tabled as overturnable and to be read by the M16 plan and its coders, with hooks in M14 (detection rolls) and M9/M12 (the law's sightings and responses).

**The idea.** Factions and NPCs have only so much attention. A faction fighting a physical war, a riot or a large run may not notice one lone runner stealing something quietly, or one sneaky infiltrator while a tank is outside blowing things up. If you can manipulate a group's attention and threat assessment you can be ignored long enough to get in and out. Sparking a war between the target and another faction is a tactic.

**Attention as a faction resource.** `Faction.attention: f32` (0..1) is a daily pool sized by the faction's management competence (M15) and headcount, consumed by active **threats** in order of assessed severity: a raid or breach in progress, a riot in a district it controls, a loud run against its nodes, a strike, a hostile acquisition, a vendetta, a sanction. Each threat holds a share for its duration plus `attention_linger_days`. What remains is the faction's **alertness**, and every detection and response roll the faction makes scales by it: guard sightings and witness rolls (M9, M12 stances), sweeps and ICE contests on its nodes (M14), camera identification (M14), riot and raid response strength (M12), the Security corp's contract response. Below `alert_floor` (0.2) the faction is **distracted**: quiet runs and infiltrations against it roll at the floor, and the law's reports from its guards slow.

**Threat assessment is manipulable.** The faction ranks threats by what it *knows* (M15: rumours, sightings, the trace), not by what is true. So: a war started through a contract or propaganda (M15 `Spin`, M16 `Hit` on a rival's member framed through a hop chain), a loud decoy run (M14 addendum) that is real but pointless, a riot sparked in its district (M12), a strike bought (M16 coercion of a union-shaped group), a sanction (M17) all consume its attention. The brains read attention too: a distracted faction's own orders tilt to `Hunker`/`Garrison`/`LieLow`.

| Decision | Call |
| --- | --- |
| Where it lives | `attention` on `Gang`, `Corp`, `Law` and `OutsideFaction`; recomputed daily and on each threat event; saved. |
| Pool | `pool = clamp(0.4 + 0.4 × competence + 0.2 × min(headcount ÷ 20, 1), 0.3, 1.0)`; threat shares from a table (raid 0.5, riot 0.4, loud run 0.3, strike 0.2, acquisition 0.15, vendetta 0.15, sanction 0.2), summed and clamped; `alertness = max(pool − Σ shares, alert_floor)`; `attention_linger_days` 2. |
| Detection hook | One multiplier `alertness_mult(faction)` read by: `law::sightings` and the witness roll for the law; M14 sweeps, ICE contests, camera id; M12 riot/raid response; M16 hit-squad and bodyguard reaction; M13 robot sensors. Default 1.0 when the field is absent (old saves). |
| Brains | `OrderInputs.attention`; a consideration `1 − alertness` on Hunker/Garrison/LieLow (Linear{0.5, 0.5}); the law's posture brain reads it on Garrison. |
| Player and NPC tactics | No new action: a distraction is any existing threat the actor can cause; the Quest view (M18) labels a contract's side effect "distracts X" when it would consume X's attention. |
| Bound | Attention is O(factions) daily; no per-agent cost. |
