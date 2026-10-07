# M14: Data and Virt — the second plane, decks, ICE, the tech tree

Companion to `SPEC.md`, `M8_FACTIONS.md`, `M9_LAW.md`, `M10_SCALE.md`, `M11_OWNERSHIP.md`, `M12_DISTRICTS.md`, `M13_ASSETS.md`, `ROADMAP_POST_M14.md` and `VISION.md`. M11 gave the city owners, M12 places and M13 things. M14 gives it **knowledge as property**: Data that Labs make and corps live on, a second plane where Data sits behind ICE, decks that carry a runner across it while their body sits in a chair, and a tech tree that a corp climbs with Data and loses when the Data stops. A Sump kid with a stolen deck lifts Chrome research out of a Zetatech Lab and sells it back to Zetatech's rival; an Arasaka node traces a runner to a Bar terminal and the law cuffs them in the chair; a gang's runner opens a Vat Farm's door an hour before the raid arrives; a runner who pokes tier-3 ICE with a tier-1 deck never stands up again. Where this document and the earlier ones disagree, this one wins for M14.

The roadmap row (`M11_OWNERSHIP.md`): *a second plane with portal pathing, nodes instead of buildings, decks as movement tiers, ICE as guards, Data as a faction resource produced by Labs and stolen through Virt, a three-track tech tree gating asset tiers.* M14 also takes what `VISION.md` and `ROADMAP_POST_M14.md` assign to it: tech decay when research lapses or Data is deleted (addendum 2), cameras with an identification roll and the first faction database (addendum 3), ICE that fries a brain with reflex as the save (addendum 4), deck against ICE as the fourth user of the tier-contest rule (addendum 7), ICE as a Power consumer with a coin placeholder (addendum 9), missions watched live through a deck (governance addendum), and the Virt plane built as the first second layer (addenda 2 and 10). Power as a good, gossip, contracts and the outside ledger stay out (§ 15).

Scale: 2,000 residents on the 256 × 192 map; every number assumes M10–M13 have landed.

Decisions taken by the implementing agent (overturnable, listed so they are cheap to overturn):

| Question | Decision |
| --- | --- |
| What a node is | A struct in `World::virt.nodes`, indexed by `NodeId(u16)`, not an entity. One per faction-owned non-Home building, one per Lab and Hideout, one Precinct node, one **Ledger** node per corp and one for the Treasury (the Hall), and one **Public** node per district. Homes and city-owned shops ride on their district's Public node. That is 120–250 nodes at 2,000 residents. |
| Links | Three kinds with a tier: `Street` (Public ↔ adjacent Public, tier 1), `Access` (building node ↔ its district's Public, tier 1), `Trunk` (a faction's building nodes ↔ its Ledger and its Labs, tier 2). A deck crosses links of tier ≤ its effective tier, so a tier-1 deck can rob a Lab but never touch a treasury. |
| Portal pathing | A run is a plan that crosses a portal: the meat leg is `GoTo(Chair)` on flow fields, the Virt leg is a Dijkstra over the node graph from the chair's portal node, joined at the chair. Nothing in `pathfind.rs` or `flowfield.rs` changes. The layers milestone generalises the same join (a stair is a portal). |
| What moves on the plane | Nothing per tick. A run is an event chain: each hop and contest is a timestamped step in `World::run_queue`, popped when due. The body holds `ExecState::JackedIn` in its chair at Coarse or Full. |
| Which ICE a runner fights | Every node on the route owned by someone other than the runner's patron, plus the target twice (break-in and extraction). The route minimises `Σ −ln p_pass`, so runners come in through a faction's weakest building: hardening one node and not the rest is a mistake the city can make. |
| The contest | M13's `security::contest` generalised to `contest_f(att: f32, def: f32)` with the same `p = clamp(0.5 + contest_step × (att − def), 0.05, 0.95)`. `att = deck tier + hack_w × Skills.hacking`, `def = ICE tier + alarm`. The u8 callers (locks, robot sensors) are unchanged. |
| Loss outcomes | Bounced, Traced, Fried, Flatlined, Captured (the payload goes back). Fry and trace roll independently on a lost contest; reflex (`Body.reflex + Kit.reflex`) saves against fry as it saves against cars. Tier-3 ICE at a gap ≥ `flatline_gap` kills: the brutality section's "fries your brain instantly". |
| Off-screen runs | **Not a hole.** A run changes a faction's Data, treasury or doors the moment it lands, and the 2-bit `HoleKind` field is full after M13's `Abducted`. A Statistical agent who would run is promoted to Coarse for the run, as M13 episodes are. Runs number ≤ 20 a day. |
| Data | A faction resource in **three tracks** (`Data` is per-track units), stored in a `DataStore` on the node where it sits: a Lab's own output, a Hideout's take. Not money, not a stock good: it is never carried in `Inventory`, only on a deck mid-run or by a freelancer between run and sale. |
| Who buys Data | Tech-niche corps owning a Lab, at `data_price × price_level[Tech]` (`Flow::Data`). Fixers are M16. A corp that steals Data spends it on its own research. Gangs never research: they sell. |
| The tech tree | Per corp, `Tech { tier: [u8; 3] }` over `Track { Chrome, Deck, Industry }`, tier 1..=3, tier 1 the commodity baseline everyone holds and nobody loses. Tiers 2–3 cost Data to reach and Data plus coins per day to hold; seven unpaid days drop a tier; a wipe with no backup drops it at once. |
| What a tier gates | **Sales and effect.** A seller sells kind-tier `t` only while its owner's track tier ≥ `requires[kind][t]`; an asset's effective tier is `min(tier, maker's track tier)`, so a corp that loses Chrome 3 degrades every tier-3 implant it ever sold. ICE is capped by its installer's Deck tier. Agent-owned sellers read tier 1. |
| The Security profile | M12 and M13 never gave it a struct; M14 does: `Building.security: SecurityProfile { ice, ice_maker, ice_arrears }` stored, with `sensor` (best powered robot or camera tier) and `guard` (1 with a contract) derived for the panel. Only ICE is new state. |
| Who installs ICE | The Security niche sells it (`ice_price`, `Flow::Contract`) at a tier capped by the seller's Deck track; a corp with Deck ≥ the tier self-installs at `self_install_frac`. Upkeep is the Power placeholder (`Flow::IceUpkeep` to the Treasury). |
| Cameras | `AssetKind::Camera`, the robot's sensor role without the fight: posted, tier 1–3, cheap. It feeds the owner's `FactionDb` with sightings. The first faction database (addendum 3) is a ring of 32 sightings per faction, read by the law and the gang brain only. |
| Who polices Virt | Nobody patrols it. The **trace is the witness**: a traced runner is a located suspect at the chair (`law.last_seen`), reported by a corp or city owner; a gang owner never calls the law and takes a `Shock` instead. |
| Watching live | UI only: a raid or run with a deck-equipped streamer shows live in its panel; without one the player sees the outcome event. `MissionView` is the M18 hook; it changes no roll. |

## Goals and acceptance

The city should read as one with a second economy behind the first: Labs humming in the Spire and the Vats, Data flowing into research and out through the Sump, corps hardening the node that was robbed last week and leaving the one next to it soft, a Bar terminal where the same three runners jack in every night, a corp that let its Lab budget slide and watched its tier-3 chrome go grey on its customers' arms. The target spiral: **Zetatech's Chrome Lab holds 600 units under tier-2 ICE → a Sump gang buys a deck for its best runner → three thefts in two weeks drain the Lab below upkeep → Zetatech `Secure` raises the Lab to ICE 3 → the next run fries the runner → the gang's second runner wipes the backup instead → Zetatech loses Chrome 3 → its tier-3 implants run at tier 2 and Clinic sales drop → its rival `Research`es Chrome on the stolen Data it bought.** On seed 42, 2,000 residents, 120 days:

- the plane: 120–250 nodes on day 120, every Public node linked, the graph connected for a tier-1 deck (Ledgers excepted); ≥ 4 Labs on day 120, ≥ 1 built by a `Research` order;
- Data: ≥ 2,000 units produced; ≥ 10 `DataStolen` events moving ≥ 300 units; ≥ 1 `DataWiped`; ≥ 200 units sold to a Tech corp;
- runs: ≥ 40 runs; success share 30–70 %; ≥ 1 `Fried`; `Flatline` deaths 1–10; ≥ 1 runner traced and arrested at the chair (an `Arrest` on an `Intrusion` or `DataTheft` report whose tile is the chair); ≥ 1 Door hack followed by a corp-building raid departing inside its window; ≥ 1 robot turned or camera blinded; ≥ 1 Ledger theft, with the money-conservation test passing;
- tech: ≥ 1 `TechGained`; ≥ 1 corp losing a tier (`TechLost`, by lapse or wipe); after a loss, ≥ 1 asset in use at an effective tier below its tier;
- ICE: ≥ 10 `IceRaised`; on day 120 the Spearman correlation across corps between 30-day ICE spend and mean node ICE ≥ 0.6; after a Virt loss on a corp node, its ICE rises within 7 days in ≥ 50 % of cases;
- decks owned on day 120: 30–150;
- Assault events per day stay ≤ 42.7, Murders per 120 days rise by ≤ 15 % over the M13 run on the same seed (Flatlines count separately), starvation deaths and population stay within the scaled v1 bounds, and the v1, M8–M13 and Full-vs-Statistical parity gates still pass;
- throughput is reported per gate; the shared floor is 4,000 ticks/s (2026-10-06: relaxed from 8,000 while systems are still being built; optimisation is a later pass).

## 1. The Virt plane

### Data model

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct NodeId(pub u16);

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum NodeKind {
    Public(DistrictId),   // the street net: terminals, Homes, city-owned shops
    Building(EntityId),   // a faction-owned non-Home building, a Lab, a Hideout, the Precinct
    Ledger(EntityId),     // a corp's treasury; Ledger(hall) is the Treasury
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    pub owner: Option<EntityId>,         // faction; None = the city
    pub pos: TilePos,                    // door, or the district's centroid: where the overlay draws it
    pub alarm_until: Option<Tick>,       // after a traced run: +1 to def until then
    pub hacked: Option<(HackEffect, Tick)>,
    pub store: DataStore,                // empty except Labs and Hideouts
    pub breaches: VecDeque<Tick>,        // last 8 successful runs, for the panel and Secure
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum LinkKind { Street, Access, Trunk }

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Link { pub a: NodeId, pub b: NodeId, pub kind: LinkKind, pub tier: u8 }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VirtPlane {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    #[serde(skip)] pub adj: Vec<SmallVec<[(NodeId, u16); 6]>>,  // (neighbour, link index); rebuilt
    #[serde(skip)] pub of_building: BTreeMap<EntityId, NodeId>,
    #[serde(skip)] pub ledger_of: BTreeMap<EntityId, NodeId>,
    #[serde(skip)] pub public_of: Vec<NodeId>,                  // by DistrictId
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SecurityProfile {          // on Building, #[serde(default)]
    pub ice: u8,                      // 0..=3 installed
    pub ice_maker: Option<EntityId>,  // the corp whose Deck tier caps it; None = the city's own
    pub ice_arrears: u8,
}
```

`World::virt: VirtPlane` is saved; `adj` and the indices are rebuilt on load. **Relink** (`virt::relink`) runs on any event that changes a building's owner or kind (`Acquired`, founding, bankruptcy, `Derelict`, a gang Hideout founded or sacked) and on load: it adds or drops nodes and links and keeps the state of surviving nodes keyed by building, so a relink never resets ICE or Data. A node's ICE lives on the building's `SecurityProfile`; a Ledger node's ICE lives on the corp (`Corp.ledger_ice: SecurityProfile`), the Treasury's on the Hall. District adjacency for `Street` links is read from the M12 cuts (districts sharing a border run). Effective ICE: `ice_eff(n) = if arrears ≥ ice_off_days { 0 } else { min(ice, maker's Deck tier) }` (uncapped for `ice_maker = None`).

### Portals and the chair

`LocationKey::Chair` resolves, for an agent with a deck, to the nearest of: their Home, their gang's Hideout, their workplace when it is a Lab, or a **public terminal** (any Bar or Hotel; the runner pays `terminal_fee` to its owner, `Flow::Terminal`, taxed). The chair's **portal node** is the chair building's own node when its owner is the runner's patron or employer, else the district's Public node. That is the only coupling of the two planes: a `Portal { building, node }` lookup, the shape the layers milestone gives a stair.

`JackIn` (new `ActionKind`, dur 2, at the chair) creates the run (§ 3) and sets `ExecState::JackedIn { run }`. The body stays in the building: it counts as asleep for theft and assault rolls there (no witness roll from the runner, `fighting × jacked_fight_mult` if attacked), its Needs decay as Wait's. An attack on the body, an arrest or a fire alarm dumps the run: `Dumpshock` (sanity − `dump_sanity`, the run ends Bounced). A jacked-in agent is pinned at Coarse for the run (never Statistical); in view it is Full and drawn slumped with a cable glyph.

### Routing

`virt::route(world, from: NodeId, to: NodeId, deck_eff: u8, att: f32, patron) -> Option<Route>` is Dijkstra over ≤ 250 nodes: links with `tier > deck_eff` are skipped; entering a node whose owner is not the patron costs `−ln p_pass(att, def(n))` (0 for unguarded nodes and Public nodes), plus `hop_eps` per hop so equal-risk routes prefer fewer hops. `Route { nodes, p_success }` with `p_success = Π p_pass × p_pass(target)²` (break-in and extraction). Planners call it once per candidate target per scoring; at ≤ 30 candidate targets per scorer it is a few microseconds each.

### Config

```toml
[virt]
enabled = true
terminal_fee = 3
jacked_fight_mult = 0.3
dump_sanity = 0.1
hop_ticks = [8, 5, 3]              # per deck tier
contest_ticks = 5
hop_eps = 0.01
min_route_p = 0.25                 # no plan targets a node below this
public_links_tier = 1
access_links_tier = 1
trunk_links_tier = 2
```

## 2. Decks

`AssetKind::Deck`, appended to M13's enum, tier 1–3, `Carried` like a pack, bought at a **Clinic** (the Ripperdoc is the netrunner's shop) with M13's `BuyAsset` and finance rules; stolen by `Strip` from a corpse or a jacked-in body (a `Theft`, the deck's lock is `security::contest(thief tier, deck tier)`); upgraded by `UpgradeDeck` (new `ActionKind`, dur 30, at a Clinic) for `(price[t+1] − price[t]) × upgrade_frac`, allowed while the seller can sell `t + 1`. A corp fleet deck is `Parked` at a Lab and used by whoever is on shift there, as M13 fleet vehicles are.

`Kit` gains `deck: Option<EntityId>` and `deck_tier: u8` (effective, § 5); `Skills` gains `hacking: f32` (seeded `hack_seed_scale × U(0,1)^3`, so about a fifth of adults clear `deck_shop_min`; children inherit as `stealth`; drifts `+hack_drift` per run survived, `+2 × hack_drift` on a success). M13's **Shop** goal gains a deck candidate: `Can(hacking ≥ deck_shop_min)` → GATE; `hacking` → Linear{0.6,0.4}; `1 − lawfulness` → Linear{0.5,0.5}; with the shared Shop considerations. Gangs buy one deck a day, in the M13 buy step, for the member with the highest hacking and no deck while treasury ≥ `gang_deck_floor`, up to `gang_decks_max`. Corps buy one fleet deck per Lab under `Research` (§ 6).

```toml
[decks]
deck_shop_min = 0.4
hack_seed_scale = 0.8
hack_drift = 0.01
upgrade_frac = 0.7
gang_deck_floor = 900
gang_decks_max = 3
[assets]                           # additions, merged into the M13 tables
price = { deck = [400, 1500, 5000], camera = [120, 400, 1200] }
upkeep = { deck = [1, 3, 8], camera = [1, 2, 4] }
wear = { deck = 0, camera = 1 }
parts_per = { deck = 4, camera = 2 }
```

## 3. Runs, ICE and the contest

### The run

```rust
pub type RunId = u64;  // (tick << 16) | runner index low bits, collision-checked

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Purpose { Data { wipe: bool }, Ledger, Door, Robot(EntityId), Camera(EntityId), Overwatch(EntityId) }

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum HackEffect { DoorOpen, RobotTurned(EntityId /* new owner */), Blind }

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RunOutcome { Success, Bounced, Traced, Fried, Flatlined, Captured, Dumped }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub runner: EntityId,
    pub deck: EntityId,
    pub chair: EntityId,
    pub patron: Option<EntityId>,       // the gang or corp that ordered it; None = freelance
    pub purpose: Purpose,
    pub target: NodeId,
    pub route: SmallVec<[NodeId; 8]>,
    pub at: u8,                         // index into route
    pub phase: RunPhase,                // Hop, BreakIn, Act, Extract, Out
    pub next_at: Tick,
    pub payload: [u32; 3],              // Data in hand, per track
    pub stream: bool,                   // § 7
    pub log: SmallVec<[(Tick, NodeId, bool); 8]>,   // contests, for the run panel
}
```

`World::runs: BTreeMap<RunId, Run>` and `World::run_queue: BTreeSet<(Tick, RunId)>`, both saved. The new `virt` system (tick order `… plan, exec, virt, ownership, assets, tech, classes …`) pops due steps only. A step: **Hop** to the next node (`hop_ticks[deck_eff]`); entering a guarded node contests it (`contest_ticks`); at the target **BreakIn** contests, **Act** takes `act_ticks[purpose]`, **Extract** contests again, **Out** applies the effect and ends the run with `JackOut`. A lost contest ends the run where it stands and rolls the outcome below. Runs are re-routed at JackIn only; a node that gains ICE mid-run is met as it is.

### ICE

ICE is the node's guard. `def(n) = ice_eff(n) + 1 if alarm_until > now`. Every corp, gang and the city pays **ICE upkeep** daily per node (`ice_upkeep[ice]`, `Flow::IceUpkeep` to the Treasury: the Power placeholder the goods milestone re-points). Unpaid: `ice_arrears += 1`; at `ice_off_days` ICE reads 0 until a day is paid. Installation (one tier at a time) is bought from the cheapest Security corp whose Deck tier ≥ the new tier, at `ice_price[tier] × its price_level[Security]` (`Flow::Contract`), or self-installed at `self_install_frac` of that (to the Treasury, `Flow::Import`) when the owner's own Deck tier suffices; `ice_maker` is the seller or the owner. Seeding: `ice_seed` per kind, Ledgers by treasury through `ice_value_steps`, the Treasury and Precinct at `levers.city_ice`.

**Target tier** (read by `Secure`, § 6): `ice_target(n) = #{ s in ice_value_steps : value_at_risk(n) ≥ s }`, where `value_at_risk` is `Σ store × data_price` for a Lab or Hideout, the treasury for a Ledger, and M11 `value(kind)` otherwise (phase 3 deviation: posted asset value is not added, since runs only target Labs, Hideouts, Ledgers and robots; a robbed building is still raised through the loss term). A Lab with 500 units reads 1,500 (tier 2); a Ledger at 6,000 coins reads tier 3. This is `VISION.md`'s "harden a facility by the resources inside it".

### The contest and its outcomes

`att = deck_eff + hack_w × Skills.hacking`; `p_pass = contest_f(att, def)`; one draw from `rng.world()` per contest. On a **loss** with `gap = def − att`:

1. **Fry.** `p_fry = fry_base[ice_eff] × (1 + fry_gap × max(gap, 0))`. The runner saves with M13's `p_dodge = min(dodge_w × (Body.reflex + Kit.reflex), dodge_cap)`. Unsaved: `Fried` event; sanity − `fry_sanity`, energy − `fry_energy`, memory `Fried` (0.9, −0.8); then death with `p_fry_kill[ice_eff]`, or with `p_flatline` when `ice_eff == 3 ∧ gap ≥ flatline_gap` (`DeathCause::Flatline`, `Flatlined` event, the killer is the node's owner for the biography and nobody for the law). The deck takes `fry_wear` condition.
2. **Trace** (independent, the dead included). `p_trace = trace_p[ice_eff] × (1 − stealth_w × (Skills.stealth + Kit.stealth))`. Traced: a `Sighting { who: runner, tile: chair door, tick, confidence: 1.0 }` in the owner's `FactionDb`; the node's `alarm_until = now + alarm_hours`. A **corp or city** owner files `law::file_report(crime, runner, None)` with `Crime::DataTheft` for a Data or Ledger purpose and `Crime::Intrusion` otherwise, and `law.last_seen` gets the chair tile, so the runner is a located suspect and the district's guards come to the chair (the body is dazed `daze_ticks` after any loss: an arrest at the chair). A **gang** owner pushes `Shock::Hacked { by }` (severity 0.5) where `by` is the runner's gang, which Retaliate reads as `MemberKilled` reads a rival; a freelance runner traced by a gang becomes that gang's next Harvest or assault target through the sighting (the gang brain reads its database).
3. **Captured.** A loss on Extract returns `payload` to the store it came from.
4. Otherwise **Bounced**.

A **success** is still seen afterwards: the owner's `breaches` ring and, for a corp, `loss_log` gain the value (`units × data_price`, coins taken, or `door_loss` for a door), so `Secure` reads Virt losses as it reads robberies.

```toml
[ice]
contest_step = 0.25                # shared with [chrome]; M13's value
hack_w = 1.0
ice_price = [0, 150, 600, 2000]
self_install_frac = 0.5
ice_upkeep = [0, 3, 8, 20]
ice_off_days = 2
ice_value_steps = [200, 1500, 6000]
ice_seed = { lab = 2, security_office = 2, farm = 1, market = 1, hideout = 0, clinic = 1, garage = 1, bar = 0, hotel = 0 }
alarm_hours = 48
fry_base = [0.0, 0.05, 0.25, 0.6]
fry_gap = 1.0
fry_sanity = 0.3
fry_energy = 0.5
p_fry_kill = [0.0, 0.0, 0.05, 0.3]
flatline_gap = 1.5
p_flatline = 0.8
fry_wear = 20
trace_p = [0.0, 0.2, 0.45, 0.7]
stealth_w = 0.5
daze_ticks = 30
act_ticks = { data = 30, ledger = 30, door = 10, robot = 15, camera = 5 }
[levers]                           # addition
city_ice = 2
```

## 4. Data and Labs

### The Lab

`BuildingKind::Lab` (label "Lab", letter `Q`), appended; staff `Role::Researcher` (label "Researcher"; `[buildings] lab = { capacity = 8, stock_cap = 0, staff = 4 }`); foundable through `Register` and by the corp `Research` order (`found_cost.lab` 800, `value` 1200, upkeep 15). A Lab has a **focus** track, set by its owner corp's `Tech.focus` when built and changed only by `Research`. Hiring prefers adults by `hacking` (the M11 hiring pass sorts candidates by it for Labs), so Labs collect the city's runners as staff. Seeded on Lots by `founding::build_on_lot`: Zetatech a Chrome Lab in the Spire and an Industry Lab in the Civic district, Arasaka a Deck Lab in the Vats, Militech a Deck Lab in Mid West.

### Production and storage

Daily, credited from the shift ledger at midnight as M12 sanitation is (every LOD tier produces the same), each Lab adds `data_per_shift × (0.5 + hacking)` per Researcher-shift worked to `store[focus]`, capped at `store_cap`. Stores live on nodes:

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DataStore { pub units: [u32; 3] }   // per Track; Data from a Chrome Lab is Chrome Data
```

A Lab's store is its owner's; a Hideout's is its gang's take. A faction's **holding** in a track is the sum over its nodes. Data never decays in a store; it is spent by research, upkeep, sale, theft or deletion.

### Theft, sale, deletion

- **Steal** (`Purpose::Data { wipe: false }`): Act takes `min(store[t], steal_units[deck_eff])` from each track, largest first, into `payload`; Out credits it to the patron's store (a gang's Hideout node, a corp's nearest Lab of the same focus or its first Lab) or, for a freelancer, to the deck (`Asset.data: [u32; 3]`, `#[serde(default)]`). `DataStolen` event.
- **Sell.** A freelancer's `SellData` (new `ActionKind`, dur 10, at a Lab owned by a Tech-niche corp) and a gang's daily sale from Hideout stock (every unit above `gang_data_keep`) go to the Tech corp with the lowest Data holding in that track that can pay: `data_price × price_level[Tech]` a unit (`Flow::Data`, taxed as wholesale), units into its Lab of that focus, else its first Lab. With no buyer the Data waits. A corp holding Data in a track it does not research may sell it under `Research` the same way.
- **Wipe** (`Purpose::Data { wipe: true }`): Out sets the target store to zero; `DataWiped` event. Then, per track the wipe touched, if the owner's remaining holding is below `upkeep_data[tier] × backup_days`, that track drops a tier at once (§ 5). Redundant Labs are the defence against a wipe, as ICE is the defence against a theft.
- **Ledger** (`Purpose::Ledger`, deck tier ≥ 2 by the Trunk link): Out moves `min(ledger_frac × treasury, ledger_cap[deck_eff])` from the corp's treasury, or the Treasury, to the patron's purse or the runner's Wallet through `ownership::pay` with `Flow::Hack` (untaxed), so the conservation test covers it. `LedgerHacked` event, `CorpShock::Robbed(amount)`.

```toml
[data]
data_per_shift = 3.0
store_cap = 2000
steal_units = [60, 150, 400]
data_price = 3
gang_data_keep = 0
backup_days = 3
ledger_frac = 0.05
ledger_cap = [0, 400, 1500]
door_loss = 100
[corps]                            # additions, merged into the existing tables
found_cost = { lab = 800 }
value = { lab = 1200 }
upkeep = { lab = 15 }
```

## 5. The tech tree

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Track { Chrome, Deck, Industry }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tech {                     // on Corp, #[serde(default)]; default tier [1, 1, 1]
    pub tier: [u8; 3],                // 1..=3
    pub progress: [u32; 3],           // Data spent toward the next tier
    pub lapse: [u8; 3],               // consecutive unpaid upkeep days
    pub focus: Track,
}
```

| Track | Gates (sale and effective tier) | Also |
| --- | --- | --- |
| Chrome | implants | — |
| Deck | decks, ICE, robots, cameras | the ICE tier a Security corp can sell |
| Industry | vehicles | Farm production × `1 + industry_prod[tier]` on owned Farms |

**Gating.** A seller sells kind `k` at tier `t` only while its owner's tier in `track(k)` ≥ `requires[k][t]`; agent-owned and city sellers read tier 1. Used and repossessed stock is exempt (it was made already). Every sold asset records `Asset.maker: Option<EntityId>` (the seller's owner when a corp). **Effective tier** `eff(a) = min(a.tier, maker's tier in track)`, `orphan_cap` when the maker is gone, uncapped when `maker` is `None`; `Kit`, `law::fighting` for robots, the vehicle tables and the deck contest all read `eff`. A tier change re-kits every agent holding that maker's assets (O(assets), on the event).

**Research.** Daily, every corp with a track at tier ≥ 2 pays that tier's **upkeep**: `upkeep_coins[tier]` (`Flow::Research` to the Treasury: licences and compute, a Power consumer later) and `upkeep_data[tier]` units from its holding in that track. Short on either: `lapse += 1`; paid: `lapse = 0`. At `lapse ≥ decay_days` the track drops one tier, `progress = 0`, `TechLost` event, `CorpShock::TechLost` (0.6). Under the `Research` order (§ 6) the corp also moves up to `research_rate` units a day from its holding in `focus` into `progress[focus]`; at `progress ≥ tier_cost[next]` with treasury ≥ `tier_coins[next]` it pays and gains the tier (`TechGained`). Tier 1 never decays. Gangs and agents have no `Tech`.

Seeds keep M13's sales whole: Zetatech Chrome 3, Deck 1, Industry 3 (its Clinic sells tier-3 chrome, its Garage flyers); Arasaka Deck 3; Militech Deck 2; everyone else 1. Zetatech's two Labs at four Researchers make ≈ 20 units a day against 16 of upkeep, so it holds both tiers with little slack: theft or a wipe tips it. That is deliberate.

```toml
[tech]
requires = { implant = [1, 2, 3], deck = [1, 2, 3], robot = [1, 2, 3], camera = [1, 2, 3], motorcycle = [1], car = [1, 2], truck = [2], flyer = [3], pack = [1, 1] }
tier_cost = [0, 0, 400, 1200]      # Data to reach tier i
tier_coins = [0, 0, 800, 2500]
upkeep_data = [0, 0, 3, 8]
upkeep_coins = [0, 0, 15, 40]
decay_days = 7
research_rate = 40
orphan_cap = 2
industry_prod = [0.0, 0.0, 0.1, 0.2]
focus_by_niche = { food = "Industry", housing = "Industry", security = "Deck", tech = "Chrome" }
seed = { Zetatech = [3, 1, 3], Arasaka = [1, 3, 1], Militech = [1, 2, 1] }   # [chrome, deck, industry]
```

## 6. Hacking: goals, orders, the law

### Purposes

| Purpose | Target node | Effect at Out | Who orders it |
| --- | --- | --- | --- |
| `Data { wipe: false }` | a Lab or Hideout with Data | payload to the patron or the deck | freelance, gang `VirtRaid`, corp `VirtRaid` |
| `Data { wipe: true }` | a rival's Lab | store to 0, maybe a tier lost | corp `VirtRaid`, gang `VirtRaid` when paid by nothing but grudge (`Shock::Hacked` pending) |
| `Ledger` | a corp's Ledger (or the Treasury) | coins via `Flow::Hack` | freelance (deck ≥ 2), gang `VirtRaid` against the hoarding corp |
| `Door` | the raid target's building node | `DoorOpen` until `raid_at + door_hours` | the Raid prelude (below) |
| `Robot(asset)` | the robot's building node | `RobotTurned(patron)` for M13's `hack_hours` | gang `VirtRaid`, the Raid prelude when the target has a robot |
| `Camera(asset)` | the sensor's building node | `Blind` for `blind_hours` | the M13 theft family when a camera or robot sensor watches the target (freelance thief with a deck) |
| `Overwatch(raid)` | none (stays on the portal node) | streams the raid (§ 7) | the gang when a raid departs |

**DoorOpen**: the building's powered robots neither defend nor sense, contracted private guards are not summoned to its brawl (those physically at the door still fight), and its ICE reads 0. **RobotTurned**: the robot fights on the patron's side in `raid::fight_out` at its building and senses for nobody. **Blind**: the building's cameras and robot sensors skip their contest. A thief with a deck jacks in at the nearest chair first; this is `VISION.md`'s "hacking vs security" approach as one more action chain.

### Hack (Full and Coarse goal)

| Goal | Considerations | Plan |
| --- | --- | --- |
| Hack | `Can(Kit.deck ∧ hacking ≥ hack_min ∧ a Chair reachable ∧ best target p_success ≥ min_route_p ∧ cooldown passed)` → GATE; `U(EV ÷ hack_ref)` with `EV = value × p_success` → Logistic{8,0.3}; `1 − lawfulness` → Linear{0.6,0.4}; `greed` → Linear{0.5,0.5}; `1 − U(wealth)` → Linear{0.5,0.5}; `1 − p_fry_route × (1 − courage)` → Linear{0.6,0.4}; flat `hack_flat` | `GoTo(Chair) → JackIn → (run) → [GoTo(DataBuyer) → SellData]` |

A freelancer picks among Data and Ledger targets outside their employer by EV (`value` = Data units at `data_price`, or the Ledger take); `p_fry_route` is the route's chance of a fry, so a timid runner with a weak deck leaves the Spire alone. Cooldown `hack_cooldown_days` after any run, `fried_cooldown_days` after a fry.

### Gangs

| Order | Considerations |
| --- | --- |
| VirtRaid (ninth `Order`) | `Can(a runner: member with deck ∧ hacking ≥ hack_min ∧ a target with p_success ≥ min_route_p)` → GATE; `L.greed` → Linear{0.5,0.5}; `U(best EV ÷ hoard_heat)` → Logistic{8,0.3}; `1 − U(treasury ÷ hoard_heat)` → Linear{0.5,0.5}; `1 − heat` → Linear{0.6,0.4}; `hacked_shock` (a pending `Shock::Hacked`) → Linear{0.5,0.5}; flat `order_flat.virt_raid` |

`VirtRaid` is not a muster order (`is_raid()` false): up to `virt_runners` runners each take one run a day from the Hideout chair while the rest keep `GangWork`. **The Raid prelude**: when M12's Raid targets a corp building whose node has ICE ≥ 1, a powered robot or a contract, and the gang has a runner, the best runner gets a `Door` run (or `Robot` when a robot is posted) timed to jack in `door_lead_hours` before `raid_at` at the Hideout; a failed door leaves the raid departing as M12. `raid::strength` is unchanged; the door changes who defends.

### Corps

| Order | What it does daily while held | Considerations |
| --- | --- | --- |
| Research (eighth `CorpOrder`) | spends `research_rate` Data into `focus` (§ 5); builds a Lab on a Lot when it owns none in `focus`; fills Lab staff; buys one fleet deck per Lab without one; a Tech corp buys offered Data (§ 4); resets `focus` to the track with the largest rival tier gap in the niche, else `focus_by_niche` | `Can((a Lab ∨ lots > 0) ∧ cash ≥ 0.3)` → GATE; `tech_gap` (max rival tier − own in focus, ÷ 2) → Linear{0.6,0.4}; `max(lapse) ÷ decay_days` → Logistic{8,0.4}; `cash` → Linear{0.5,0.5}; `E.greed` → Linear{0.4,0.6}; flat `order_flat.research` |
| VirtRaid (ninth `CorpOrder`) | one run a day by an on-shift Researcher with the fleet deck, chair the Lab, against the niche rival with the most Data in `focus`: a wipe when the rival's tier in that track exceeds its own and `1 − E.lawfulness ≥ wipe_min`, else a theft | `Can(a fleet deck ∧ a rival Lab with p_success ≥ min_route_p)` → GATE; `tech_gap` → Linear{0.7,0.3}; `1 − E.lawfulness` → Quadratic{2,1,0,0}; `1 − share` → Linear{0.5,0.5}; `cash` → Linear{0.4,0.6} |
| Secure (extended) | per building, as M11–M13 (contract or robot), then raises ICE one tier on the node with the largest `ice_target − ice_eff` gap, newest Virt loss first, up to `secure_per_day` nodes; buys a camera for a building with no sensor and `ice_target ≥ 1` | `losses` now includes Virt losses from `loss_log`; the rest unchanged |
| Hunker (extended) | also lowers ICE one tier a day, cutting upkeep, only while the treasury is below `treasury_ref / 4` and only on a node where 30 days of saved upkeep cover the cheapest rebuy (`ice_price × self_install_frac`), the most excess over `ice_target` first (phase 3 deviation: `ice_target` governs buys only; at `ice_upkeep` [0, 0, 1, 2] no shed repays itself, so Hunker keeps its ICE until phase 5 restores upkeep teeth) | unchanged |

Gangs buy tier-1 ICE for their Hideout node, in the M13 buy step, while it holds Data above `ice_value_steps[0]` worth.

### The law

`Crime::Intrusion` (label "Intrusion", salience 0.4, severity just above Theft, `sentence_hours.intrusion` 48) and `Crime::DataTheft` (label "Data Theft", salience 0.6, severity just above Grand Theft, `sentence_hours.data_theft` 120). Reports come only from traces: no guard sees a run. A located runner is arrested by the normal path; a runner still dazed in the chair cannot flee or contest. Arrest confiscates a carried deck's payload to the Precinct (destroyed). Intrusion and Data Theft reports count toward a gang's Crackdown pressure as Shakedowns do. The binder never sees a run.

```toml
[hack]
hack_min = 0.3
hack_ref = 500
hack_flat = 0.0
hack_cooldown_days = 2
fried_cooldown_days = 10
virt_runners = 2
door_lead_hours = 2
door_hours = 6
blind_hours = 12
wipe_min = 0.6
stream_min = 0.2
[gangs.order_flat]                 # addition
virt_raid = 0.0
[corps.order_flat]                 # addition (Research needs a nudge to start a Lab)
research = 0.05
[law.sentence_hours]               # additions
intrusion = 48
data_theft = 120
```

### Cameras and the faction database

A `Camera` asset is `Posted` at a building. A theft, Shakedown, `StealVehicle` or Strip at that building rolls `security::contest(thief tier, camera eff)`; a camera win writes a `Sighting { who, tile, tick, confidence: p }` (with `p` the contest's win probability) to the owner's `FactionDb` and, for a corp or the city, files the report with no witness, as M13's robot does. `World::db: BTreeMap<EntityId, FactionDb>`, `FactionDb { sightings: VecDeque<Sighting> }` capped at `db_cap`, oldest out, dropped at `sighting_days`. M14 readers: the law (a corp's or the city's sighting of a wanted agent sets `last_seen`) and the gang brain (`Shock::Hacked` and Retaliate pick the sighted runner's gang). M15 generalises sightings to agents and relays.

```toml
[db]
db_cap = 32
sighting_days = 14
```

## 7. Watching a mission live

When a gang Raid departs and a member with a deck and `hacking ≥ stream_min` is at the Hideout and not marching, that member jacks in with `Purpose::Overwatch(raid)` for the raid's duration and `Gang.stream_by` is set; the Raid panel then shows the march and the brawl live (positions, each fight roll, the door state) and is tagged LIVE. Every Virt run is streamable the same way through its Run panel (route, contests and outcomes as they resolve). Without a streamer the panel shows only the outcome events. M14 changes no roll for a streamed mission. The hook for M18 is `MissionView { source: EntityId /* the streamer */, subjects: SmallVec<[EntityId; 8]> }`, built by the app from sim state each frame and never saved; M16's contracts reuse it as the second of the three mission renderers.

## 8. The Statistical tier and LOD

| Concern | Full and Coarse | Statistical |
| --- | --- | --- |
| Lab output | credited from the shift ledger | the same |
| Freelance hacking | the Hack goal | a daily pass: a Statistical adult with a deck, on one hash-picked day in seven, scores Hack; a pass promotes it to Coarse with Hack pinned until the run ends |
| Gang and corp runners | chosen at scoring | chosen the same way and promoted to Coarse at JackIn |
| The jacked-in body | Coarse at least, Full in view | never |
| Effective tiers | `Kit` | `Kit` (M13 multipliers read `eff`) |
| ICE, Data, tech | faction-side, any tier | faction-side, any tier |

The StatTable is unchanged; `calibrate` is re-run for the new actions in the exec tallies.

## 9. Player levers and god commands

| Command | Effect |
| --- | --- |
| `SetCityIce(u8)` | The Treasury's and Precinct's ICE tier (upkeep from the Treasury). |
| `SetDataTax(f32)` | An extra tax on `Flow::Data` sales. |
| `SetHackSentence { crime, hours }` | Sentences for Intrusion and Data Theft. |

CLI: `--lever day:SetCityIce:3`. **God commands** (`GOD_SCENARIOS` style): `WipeData(building)` (the store to zero with the § 4 tier rule); `GrantDeck { agent, tier }`; `SetIce { building, tier }`; `Fry(agent)` (a fry outcome with no save, death roll at tier 3); `GrantData { faction, track, units }`; `SetTech { corp, track, tier }`; `RunNow { agent, target, purpose }` (pins a run, the contest rules apply).

## 10. UI, events, CSV

New `EventKind`s (violet): `JackedIn`, `DataStolen`, `DataWiped`, `DataSold`, `LedgerHacked`, `DoorHacked`, `RobotTurned`, `Blinded`, `Traced`, `Fried`, `Flatlined`, `Dumpshock`, `IceRaised`, `IceLowered`, `TechGained`, `TechLost`. New `DeathCause::Flatline`; `MemoryKind`s `Fried`, `Hacked`; `Crime`s `Intrusion`, `DataTheft`; `GoalKind::Hack`; `ActionKind`s `JackIn`, `SellData`, `UpgradeDeck`; `Flow`s `Data`, `Hack`, `IceUpkeep`, `Research`, `Terminal`; `CorpShock::TechLost`; `Shock::Hacked`.

- **Map overlay** (toggle `N`): the plane over the city. Nodes at doors (Public nodes at district centroids, Ledgers at the owner's richest building), coloured by `ice_eff`, a Data glyph sized by store; links as lines, Trunks thicker; live runs as a moving dot along their route, red on a lost contest.
- **Node panel** (new, from the overlay): owner, ICE and its maker, arrears, alarm, store per track, breaches, hack effect in place.
- **Run panel** (new, from an event or a jacked-in agent): runner, deck, patron, purpose, route with each contest's p and result, payload, outcome; LIVE while it runs.
- **Corp panel**: a Tech block (tiers, progress bars, lapse days, focus, Labs) and ICE spend over 30 days.
- **Inspector**: hacking, deck and its effective tier, payload, runs and outcomes, "jacked in at X".
- **City panel**: a Virt section (runs today, success share, Data produced and stolen, fried and flatlined, mean ICE by owner type) and the levers.
- **CSV** (`--report`): `nodes`, `labs`, `decks`, `runs`, `runs_ok`, `data_made`, `data_stolen`, `data_wiped`, `data_sold`, `ledger_hacks`, `doors_hacked`, `traced`, `fried`, `flatlined`, `hack_arrests`, `ice_mean_corp`, `ice_spend`, per corp `c{i}_tier_chrome`, `c{i}_tier_deck`, `c{i}_tier_industry`, `c{i}_data`, and ledger columns `flow_data`, `flow_hack`, `flow_ice_upkeep`, `flow_research`, `flow_terminal`.

## 11. Save compatibility

Every new field is `#[serde(default)]`. A pre-M14 save gets `World::virt` built by `relink` with `ice_seed` ICE, empty stores, no runs; `Corp.tech` from `[tech] seed` by corp name, else `[1, 1, 1]`; no Labs (the `Research` order builds them); `Skills.hacking` drawn from a stream keyed by entity index, as M13 drew `Body`; `Asset.maker = None` (uncapped). `BuildingKind::Lab`, `Role::Researcher`, `AssetKind::{Deck, Camera}`, `Order::VirtRaid`, `CorpOrder::{Research, VirtRaid}`, `Crime::{Intrusion, DataTheft}`, `DeathCause::Flatline` and every enum variant above are appended, so serialized indices do not shift. `Config::v1_profile` sets `[virt] enabled = false`, which turns off relink, runs, ICE upkeep, Lab production, research upkeep and every tier cap, so the v1, M8 and M9 unit tests and the M13 asset tests run unchanged.

## 12. Testing and calibration

**Unit tests** (`citysim/tests/virt.rs`, `data.rs`, `tech.rs`): relink keeps ICE and stores across an `Acquired`; a tier-1 deck's route never uses a Trunk; the route enters through the weaker of two building nodes; `contest_f` equals M13's `contest` on integers; a run pops only when due and a JackedIn body is never Statistical; a lost contest at tier-3 ICE with gap ≥ 1.5 flatlines at `p_flatline` and a high-reflex runner is fried less often; a trace by a corp files `DataTheft`, sets `last_seen` to the chair and a guard arrests the dazed runner there; a trace by a gang pushes `Shock::Hacked`; a loss on Extract returns the payload; a Lab produces from the shift ledger at every tier; a theft moves units, a Ledger hack conserves money; a wipe with no backup drops the tier at once and with a backup does not; unpaid upkeep for `decay_days` drops a tier; a seller cannot sell above its tier and a maker's loss lowers `Kit` for its customers; ICE unpaid for `ice_off_days` reads 0; `Secure` raises the node with the largest gap; DoorOpen keeps robots and summoned guards out of a brawl; a turned robot fights for the patron; a camera writes a sighting; a pre-M14 save loads.

**Scenario** (`citysim/tests/scenario.rs`, `#[ignore]` `test_m14_virt_seed_42`): the bullets under *Goals and acceptance*, in particular Data produced and stolen, ≥ 1 fried runner, ≥ 1 traced-and-arrested runner, a corp losing a tech tier, a door hacked for a raid, and ICE tiers moving with security spend. **God scenarios** (`tests/god.rs`): `WipeData` on both Zetatech Labs at day 20 (does it lose Chrome 3 and Industry 3, do Clinic tier-3 sales stop, does a rival `Research` Chrome); `GrantDeck 3` to ten Sump adults (does the Treasury get hacked, does the city raise ICE, how many flatline); `SetIce 0` across Arasaka (how fast is it bled); `SetCityIce 0` (the hacker-army test from `VISION.md`: can a gang drain the Treasury); a gang `RunNow Door` on a Vat Farm before a Raid (does the raid win more often).

**Calibration**, tuned via `[virt]`, `[decks]`, `[ice]`, `[data]`, `[tech]` and `[hack]` only:

| Target | Band |
| --- | --- |
| Decks owned, day 120 | 30–150 |
| Runs per 120 days | 40–400 |
| Run success share | 30–70 % |
| Fried per 120 days | 3–30 |
| Flatline deaths per 120 days | 1–10 |
| Traced share of lost contests | 20–50 % |
| Data stolen ÷ produced | 5–30 % |
| Tech tiers lost per 120 days | 1–4 |
| Tech tiers gained per 120 days | 1–4 |
| Mean corp node ICE, day 120 | 1.0–2.2 |
| Summer food price | inside the v1 band 2–8 (Industry tiers move Farm output) |

**Throughput.** The plane is ≤ 250 nodes and ≤ 600 links; relink is event-driven; routing is one small Dijkstra per candidate per scoring, and scorings are daily (gangs, corps, the Statistical pass) or goal-rate (Full and Coarse with a deck). Runs are events in a time-ordered queue popped when due; ≤ 20 runs a day make ≤ 200 steps a day. ICE upkeep, Lab production, research upkeep and decay are daily and O(nodes + corps). A jacked-in body is a Coarse agent in a wait state. Nothing new runs per agent per tick.

## 13. Phases

1. **Plane, Data and tech**: § 1 data model and relink (no runs), the Lab, Researchers, production and `DataStore`, `Tech` with seeds, research upkeep, decay, the tier gates and `Asset.maker` with effective tiers, the `Research` order, the `SecurityProfile` with ICE seeds and upkeep, CSV columns, `WipeData` and `SetTech` for tests. A 10-day CSV matches M13 outside the new columns and Lab hiring; the M13 scenario still passes with the caps on. Commit.
2. **Decks and runs**: § 2 and § 3, `Skills.hacking`, `contest_f`, `JackIn` and `JackedIn`, the run queue, routing, outcomes (fry, flatline, trace, capture), the crimes and the arrest at the chair, the freelance Hack goal, `SellData` and the Data market, the Ledger hack. Commit. Watch it live: the first trace and arrest at a Bar terminal is the first visible M14.
3. **Orders and targets**: § 6 (gang `VirtRaid` and the Raid prelude, corp `VirtRaid`, `Secure` and `Hunker` on ICE, Door, Robot and Camera effects, the Camera asset and `FactionDb`, the wipe tier rule), § 8's Statistical pass. Commit. The addendum's bridge sweeps (plan V67) are deferred to M16: no Bridge is planted before M16's contracts, so a sweep would have nothing to find. Deviations signed off in the phase 3 fix round: `value_at_risk` leaves out posted asset value (§ 3 ICE); Hunker sheds ICE only below `treasury_ref / 4` and only where 30 days of saved upkeep repay the rebuy (§ 6 Corps); Secure's ICE, cameras and the VirtRaid fleet deck spend only above `treasury_ref / 4`. V33's freelance blind-first `Camera` order is kept out: a prepended `JackIn` never completes inside a non-Hack plan (the chair arrival is observed only under the Hack goal), so `Blinded` comes only from god until the exec observes the chair arrival outside the Hack goal.
4. **UI and levers**: § 7 streaming and `MissionView`, § 9 levers and god commands, § 10 overlay and panels. Commit.
5. **Gate and polish**: the M14 scenario and god scenarios, calibration, README and docs, then `/code-review high <base>..HEAD` and a fix commit, then push.

## 14. Risks

- **Tier caps move M13 on landing.** Gating sales and capping effective tiers touches every asset M13 calibrated, and a Zetatech that decays mid-run takes flyers and tier-3 chrome with it, which can break M13's flyer and install bullets. Phase 1 runs the M13 scenario with caps on before anything can be stolen; if it fails, raise Zetatech's Lab staff before loosening `requires`.
- **Fry lethality is bimodal.** With `p_flatline` high and a skewed `hacking` seed, runners either never touch tier-3 ICE (zero fried) or die in a week (no runner population). The route's `p_fry_route` term and `min_route_p` are the brakes; calibrate `fry_base` and `flatline_gap` on the run log before touching `hacking`.
- **Door hacks hand gangs the corp raids.** DoorOpen removes robots and summoned guards from M12's corp raids, which can multiply corp raid wins, Murders and the corp `Robbed` shocks that drive M11's brain. Watch corp raid win share against M13 in phase 3 and shorten `door_hours` before weakening the effect.

## 15. Out of scope (M15 and later)

Power and Water as goods, generators and a `powered` state (the goods milestone; `Flow::IceUpkeep` and `Flow::Research` are the hooks); gossip, reputation, sightings held by agents and relayed along edges, intel as Data about a person's routine (M15); contracts, Fixers, Data sold through fixers, hit squads, bounties and tracking tags, `Heist` as a contract kind (M16); parents, uplink nodes behind tier-3 Trunks and the `edge` cut from Data theft (M17); the player and first-person runs (M18); map layers beyond the Virt plane, stairs and elevators as portals, interiors and room-level security; Virt combat between runners, AI agents on the plane, gang tech trees, Data as currency, and outside combat maps.

## Addendum (2026-10-05, Dylan): the overlay, firewalls on links, bridges, hop chains, quiet and loud

Spec amendment; decisions tabled as overturnable and to be read by the M14 plan and its coders.

**The overlay.** The plane is drawn as an overlay on a ghosted real city: instead of buildings and roads there are nodes, servers and links, positioned at the buildings they belong to, so a runner's route reads against the streets it crosses. Rendering only; the graph is unchanged.

**Firewalls on links.** A link, not only a node, can carry ICE (a `firewall` tier on the link). A route must pass every firewall on the way, each a contest. A near-impenetrable firewall is the normal state of a megacorp's trunk links.

**Bridges (the physical bypass).** The common tactic against a strong firewall is to go around it in the real world: enter the building and plant a **bridge** (an M13 asset kind `Bridge`, carried then installed at a building tile) that creates a new link from a node behind the firewall to a node the runner can reach. Planting one is a physical mission (an M16 contract kind, or the player in person: credentials, disguise, a break-in, a bribed guard) resolved on the raid machinery or the social resolver. Owners detect bridges by **sweeps** (a `Secure` spend: a daily detection roll per building against the bridge's tier, higher for private security and robots), then **cut** them (destroyed, the asset lost) or **firewall** them (a firewall tier placed on the bridge link, which makes it a normal link). A found bridge is a deed that gossip carries and the law can trace to the planter through the binder.

**Hop chains and the trace.** A runner routes through intermediate nodes on purpose: each hop is one more contest but lowers the trace. The trace outcome's attribution confidence falls per hop (`trace_decay_per_hop`), so a long chain yields a `Sighting` with low confidence or none, while a direct run yields a certain one. Hops through nodes owned by other factions spread heat onto them (a wrongly traced run is a cause for a grudge, M15).

**Quiet and loud.** Every run has a mode: **quiet** (lower success per contest, lower detection, no alarm) or **loud** (brute force: higher success, guaranteed trace, an alarm that raises the node's ICE for a day and summons the owner's response: a robot, private security, or a counter-run). The mode is a choice for the player and a consideration for the brains (a corp's `VirtRaid` goes loud when it does not care who knows).

| Decision | Call |
| --- | --- |
| Where firewalls live | On links as well as nodes; a node's own ICE is the last contest. |
| Bridge as an asset | `AssetKind::Bridge` (M13), tier 1–3, planted at a building tile by an agent inside; one bridge per building per tier; the link it creates has no firewall until the owner adds one. |
| Detection | A daily sweep roll per building with a Security profile: `p = sweep_base × security_tier ÷ bridge_tier`, clamped; sweeps cost the owner a coin upkeep as part of `Secure`. |
| Hop decay | `trace_decay_per_hop` 0.35: the trace confidence after h hops is `0.65^h`; below `trace_floor` 0.15 no Sighting is written. |
| Quiet/loud | Quiet: contest tier −0.5, detection ×0.5, no alarm. Loud: contest tier +1.0, trace certain, alarm (node ICE +1 for a day, owner response). |
| Rendering | Nodes and links as an overlay on the ghosted map at their buildings; bridges drawn as dashed links. |
