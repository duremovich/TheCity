# M13: Assets — vehicles, chrome, stims, the security robot

Companion to `SPEC.md`, `M8_FACTIONS.md`, `M9_LAW.md`, `M10_SCALE.md`, `M11_OWNERSHIP.md`, `M12_DISTRICTS.md` and `VISION.md`. M11 gave the city owners and M12 gave it places; M13 gives its people **things**. A car that halves the commute and runs a pedestrian down in a chase, an arm that wins the brawl and eats the mind, a stim that gets a double shift done and then owns the user, a robot on a Vat Farm door that never takes a bribe. Every one is an owned thing with upkeep, the same shape as a building: bought, financed, maintained, repossessed, stolen, inherited, looted. Where this document and the earlier ones disagree, this one wins for M13.

The roadmap row (`M11_OWNERSHIP.md`): *owned assets with upkeep and repossession: vehicles (motorcycle, car, truck, flyer) as GoTo cost multipliers and haul capacity, chrome at a Ripperdoc as skill modifiers, stims and addiction, the security robot.* M13 also takes what `VISION.md` and `ROADMAP_POST_M14.md` addenda 2, 4, 7 and 8 assign to it: inventory capacity from gear and strength, loot on corpses, the Clinic (the Ripperdoc is one), cyberpsychosis and addiction with treatment, reflex as a saving roll against cars and bullets, chrome tiers as the attacker side of the tier-contest rule, scavs harvesting implants, the second good, crashes, and `appearance` stored for M15. Power and Water as goods, Data, the wounded, contracts and layers stay out (§ 14).

Scale: 2,000 residents on the 256 × 192 map; every number assumes M10, M11 and M12 have landed.

Decisions taken by the implementing agent (overturnable, listed so they are cheap to overturn):

| Question | Decision |
| --- | --- |
| What an asset is | An entity with an `Asset` component (a new store through the `components!` macro), not a field on its owner. Theft, repossession, inheritance and looting are then one write to `Asset.owner` and `Asset.loc`. At 2,000 residents the city holds 1–2k assets. |
| Who can own | Exactly M11's owners: an agent, a gang, a corp, or `None` (the city), resolved through `World::purse`. Every asset coin moves through `ownership::pay`/`charge` with a new `Flow`, so the ledger and the conservation test (M11 D2/D3) cover them. |
| The per-agent reading | A derived `Kit` component (vehicle, summed chrome modifiers, sanity load, visible chrome) rebuilt on any asset change, never per tick. Every system at every tier reads the Kit. The Statistical tier reads it as **multipliers on its existing row**; no row, feature or table conditioning is added. |
| The second good | `Good { Food, Stims, Parts }`, designed as goods from the start (addendum 2). Food keeps `Building.stock_food`; Stims and Parts live in `Building.stock_goods: [u32; 2]` behind one `World::stock(b, good)` accessor. Stims are the consumable and the first illegal good; Parts are what assets break down to and are built from, so chop shops and harvesting have a market. Vehicles, chrome and robots stay entities, never stock units. |
| Where assets come from | Imported. A seller pays `import_frac` of the list price to the Treasury (`Flow::Import`: the port and customs), less `part_credit` per Part it spends from stock. Money stays inside the city (M11 D3 holds); M17 re-points `Import` at the outside ledger. |
| Who sells | Two new kinds, both foundable: **`Clinic`** (label "Ripperdoc": chrome, therapy, detox; M16's wounded reuse it) and **`Garage`** (vehicles, repairs, secure parking). Security Offices sell robots. Street Markets sell packs, and stims only while legal. |
| A new niche | `Niche::Tech` (Clinics and Garages) and a ninth seed corp, so the corp brain grows, squeezes and undercuts the asset trade with no new orders. |
| Movement | A vehicle changes the **duration of a step**, read from a per-tile-kind table when the step is taken. Paths and flow fields stay pedestrian; a car on a Ground tile is walked. Steps shorter than a tick are taken as several flow-field steps in the same call (cap `max_steps_per_tick`). The flyer ignores paths: a timed straight-line hop at every tier, the first layer-crossing mover. |
| Stims are illegal | By default (`levers.stims_legal = false`). Gangs cook them at the Hideout and deal them at Bars; legalising lets Food-niche Markets sell them, which is the prohibition lever. |
| Repossession | Two hands: the **lender** (the seller, on a finance plan) tows a vehicle or **bricks** chrome remotely; the **city** impounds a vehicle whose upkeep (registration and fuel) is unpaid. Nobody cuts an implant out of a debtor; scavs do that. |
| Crashes | One hazard roll per vehicle trip at arrival, not per step. A hit picks a body near a tile of the trip; the victim's reflex is the saving roll. A chase (driver fleeing or pursuing) multiplies the odds. |
| Loot | A corpse keeps its Wallet, inventory and assets until stripped, buried, or `loot_window_hours` pass; then inheritance runs as today on what is left. Installed chrome on a buried corpse goes to the Recycler as Parts. |
| Scavs | Not a new faction. A gang under the new **Harvest** order abducts the visibly chromed and rips them at the Hideout. Off screen it is a hole, `HoleKind::Abducted`, the fourth value the 2-bit kind field already has room for (M10 D8). |
| The robot | An asset `Posted` at a building: a defender in `raid::fight_out`, a sensor and detainer for thefts there, with no Brain, Needs or LOD. Corp `Secure` buys one instead of a contract when it is cheaper over `robot_horizon_days`. |
| Fights | `law::resolve_fight` keeps its formula and adds chrome terms only, so two unchromed agents fight exactly as in M12 and the v1/M8/M9 fight calibration stands. |
| Body stats | A new `Body` component (`strength`, `reflex`, `sanity`, `addiction`, `last_use`) on every agent, all tiers. Strength sets carry; reflex is the hazard save. Neither enters the base fight roll. |

## Goals and acceptance

The city should read as a place where what you own decides how you live and how you die: Spire execs flying over the Sump, Vat Farm trucks on the arterials, a gang that bolted chrome onto every member and then lost two of them to psychosis, a Bar where the same dealer sells to the same shaking regulars every night, a car stolen in Mid East and stripped in a Hideout by morning, a pedestrian run over because a fleeing thief took the corner too fast. The target spiral: **a Sump gang chromes up on dealing money → its members win raids → their sanity sinks and nobody pays for neuroblockers → one goes berserk at a Bar → the law Crushes the episode → the gang's heat brings a Crackdown → dealing income collapses → the financed chrome is bricked → a rival Harvest crew abducts the strongest of them for parts.** On seed 42, 2,000 residents, 120 days:

- vehicles: ≥ 150 owned on day 120 across all four kinds (≥ 1 flyer); ≥ 8 of the 12 Vat Farms run a truck by day 30 and ≥ 50 % of Farm → Market hauls after day 30 go by truck; on `GoTo(Workplace)`, drivers' mean ticks per Manhattan tile ≤ 0.5 × walkers';
- chrome: ≥ 60 installs, ≥ 20 of them on gang members; ≥ 1 implant harvested (from a corpse, an abduction or a hole); ≥ 1 Clinic or Garage founded by an NPC through `Register`;
- psychosis: 1–8 `Episode` events; ≥ 1 Therapy sold; ≥ 1 episode ended by the law;
- stims: ≥ 500 doses sold by dealers; ≥ 10 `Dealing` reports; adults at `addiction ≥ hooked` on day 120 in 1–8 %; ≥ 1 Detox; dealing ≥ 20 % of all gang income over the run;
- ≥ 3 repossessions (tow, brick or impound); ≥ 1 `Crash` fatality and ≤ 10; ≥ 1 vehicle theft ending in a `Chopped` event; ≥ 1 robot posted by a corp `Secure`; ≥ 10 corpses stripped before burial;
- Assault events per day stay ≤ 42.7, Murders per 120 days rise by ≤ 40 % over the M12 run on the same seed, starvation deaths and population stay within the scaled v1 bounds, and the v1, M8, M9, M10, M11, M12 and Full-vs-Statistical parity gates still pass;
- throughput stays ≥ 8,000 ticks/s with the Statistical tier.

## 1. The asset model

### Data model

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Slot { Arms, Legs, Nerves, Eyes, Skin }

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum AssetKind { Motorcycle, Car, Truck, Flyer, Implant(Slot), Robot, Pack }

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Good { Food, Stims, Parts }

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AssetLoc {
    Parked(EntityId),     // at a building (its door): Homes, workplaces, Garages, Hideouts
    InUse(EntityId),      // being driven by an agent
    Carried(EntityId),    // a pack on an agent
    Installed(EntityId),  // chrome in a body (a living agent or a corpse)
    Posted(EntityId),     // a robot guarding a building
    Stock(EntityId),      // for sale at a Clinic, Garage or Security Office, or a gang's take at its Hideout
    Limbo(HoleId),        // taken by an unbound Abducted hole: to the binder's gang, destroyed on Unknown
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finance { pub lender: Option<EntityId>, pub remaining: i64, pub per_day: i64, pub arrears: u8 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asset {
    pub kind: AssetKind,
    pub tier: u8,                     // 1..=3; also the lock tier (vehicles) and the sensor tier (robots)
    pub owner: Option<EntityId>,      // M11 owner; None = the city
    pub loc: AssetLoc,
    pub condition: u8,                // 0..=100; 0 = wrecked (vehicles, robots) or failed (chrome)
    pub value: i64,                   // list price at purchase × condition / 100, recomputed daily
    pub upkeep_per_day: i64,
    pub upkeep_arrears: u8,
    pub finance: Option<Finance>,
    pub bricked: bool,                // chrome locked by its lender: modifiers off, load stays
    pub stolen: bool,                 // set by theft or a rip; cleared by a chop, a fence or recovery
    pub bought: Tick,
}

/// Derived, never saved: rebuilt on load and by `assets::rekit(agent)` after any change.
#[derive(Clone, Debug, Default)]
pub struct Kit {
    pub vehicle: Option<EntityId>,
    pub fighting: f32, pub stealth: f32, pub reflex: f32, pub strength: f32,
    pub sight: u8, pub armour: f32, pub walk_mult: f32,   // chrome modifiers; bricked and failed excluded
    pub load: f32,                    // Σ sanity_cost of installed chrome (bricked included)
    pub chrome_value: i64,
    pub visible: u8,                  // installed tiers in Arms, Eyes, Skin: what a scav or an M15 observer sees
    pub timed_mult: f32,              // Coarse GotoTimed multiplier (§ 2)
    pub flash: f32,                   // visible chrome + vehicle tier, 0..=1: wealth on display
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Body {
    pub strength: f32, pub reflex: f32,   // U(0.2, 0.6) at seed and immigration; children inherit as Personality does
    pub sanity: f32,                      // 0..=1, default 1
    pub addiction: f32,                   // 0..=1
    pub last_use: Option<Tick>,           // the last stim
    pub episode_until: Option<Tick>,      // in a cyberpsychotic episode (§ 3)
}

/// For M15: stored daily, read by nobody in M13.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Appearance { pub dress: u8, pub chrome: u8, pub colours: Option<EntityId> }
```

`World::assets_at: BTreeMap<EntityId, SmallVec<[EntityId; 4]>>` indexes assets by the building or agent in their `loc` (rebuilt on load). `Appearance.dress` is set daily from class and wealth band (0 Dreg rags … 3 Spire suit), `chrome` from `Kit.visible`, `colours` from gang membership or a corp employer. `Mood` gains `body_bias` (stims, withdrawal, edginess), added in the mood update as M12's `district_bias` is.

### Buying, finance, upkeep

`BuyAsset` (new `ActionKind`, dur 20, cost 10) at a seller with one of its staff inside: the buyer pays the list price (`[assets] price[kind][tier] × seller price_level`) to the seller's owner (`Flow::Asset`, taxed) or, when short, `down_frac` of it with a `Finance { lender: seller's owner, remaining, per_day = remaining × (1 + interest) / term_days }`. The seller pays its import. Chrome goes through `Install` (§ 3), a vehicle is `Parked` at the seller and driven away, a pack is `Carried`.

Daily, in the new `assets` pass after `ownership` (tick order `commands, lod, needs, memory, mood, think, plan, exec, ownership, assets, classes, districts, economy, [bind], law, social, gang, corp_brain, demography, stats`):

1. **Upkeep.** Every asset charges `upkeep_per_day` to its owner (`Flow::AssetUpkeep` to the Treasury: fuel, registration, neuroblockers, power; the goods milestone re-points the robot and vehicle share at Power). Unpaid: `upkeep_arrears += 1` and wear × `unpaid_wear_mult`; a robot powers down (no fights, no sensing).
2. **Finance.** `per_day` from the owner to the lender (`Flow::Finance`); short: `arrears += 1`.
3. **Repossession.** At `finance.arrears ≥ repo_days` a vehicle is towed (owner = lender, loc `Stock(lender's nearest Garage)`; an `InUse` vehicle is towed at its next park) and chrome is **bricked** until the arrears clear, then after `brick_repo_days` more it is transferred to the lender in place (the body carries the lender's dead metal, its load, and no modifiers). At `upkeep_arrears ≥ impound_days` a vehicle is impounded by the city (`Stock(Precinct)`) and sold to the nearest Garage at `impound_frac × value`. `Repossessed` event, memory `Repossessed` (0.6, −0.6).
4. **Wear and value.** `condition −= wear[kind]`. At 0 a vehicle or robot is wrecked (`Wrecked` event, `parts_per[kind] / 2` Parts to the stock of the building it stood at, despawn) and an implant fails (modifiers off, sanity − `fail_shock`). An owner whose vehicle is below `repair_below` and who can pay buys a repair from the nearest Garage (`repair_price` per point, one Part per 25 points), credited daily as M12 sanitation is.

### Theft, loot, inheritance

- **Corpses are loot.** `demography::on_death` no longer moves the Wallet at once. The corpse keeps its coins, inventory and every `Carried` and `Installed` asset until stripped, buried or `loot_window_hours` pass; then M11 D45 inheritance runs on whatever is left. Parked vehicles and posted robots pass by D45 at death (spouse, child, city). A financed asset with arrears goes to its lender.
- **`Strip`** (new `ActionKind`, dur 10, adjacent to the corpse) takes coins, inventory up to capacity and any pack. **`Rip`** (dur 40, needs `courage ≥ rip_courage` or gang membership) takes one implant: it becomes `Stock` at the ripper's gang Hideout, or `parts_per[Implant]` Parts in the ripper's inventory. Both are a Theft against the dead (`Crime::Theft`); a witnessed strip gives the heir a `Stripped` memory (0.7, −0.7) with the stripper as subject, the M15 grudge hook.
- **Buried chrome** goes to the Recycler: `parts_per[Implant]` Parts into its stock, sold to Clinics and Garages at `parts_price` (city revenue, `Flow::Parts`).
- **Off screen.** Daily, every unburied, unstripped corpse rolls `scav_strip_base × (2 − coverage_d)`: a hit strips it to the Hideout of the gang that controls the district (`Stripped` event), or destroys the loot when the district is Contested or the city's.

### Inventory capacity

`Inventory` gains `stims: u16` and `parts: u16`; food stays. Load counts food 1, stims 1, parts 3. `capacity = carry_base + carry_per_strength × (Body.strength + Kit.strength) + pack[tier]`. With the defaults nobody carries less than the v1 cap of 20, so `economy::buy_quantity` and the food economy are unchanged for anyone without gear. A Hideout, Garage or Clinic holds goods in `stock_goods`; safe houses beyond that are out of scope.

### Config

```toml
[assets]
enabled = true
# list prices per tier [1, 2, 3]; a kind with fewer entries has fewer tiers
price = { motorcycle = [300], car = [800, 1400], truck = [1500], flyer = [6000], implant = [150, 500, 1500], robot = [800, 2000, 5000], pack = [30, 80] }
upkeep = { motorcycle = [1], car = [2, 3], truck = [4], flyer = [15], implant = [0, 1, 3], robot = [4, 8, 15], pack = [0, 0] }
wear = { motorcycle = 1, car = 1, truck = 1, flyer = 1, implant = 0, robot = 1, pack = 0 }
parts_per = { motorcycle = 4, car = 8, truck = 12, flyer = 30, implant = 3, robot = 10 }
parts_price = 15
import_frac = 0.6
part_credit = 20                   # import coins one Part from a seller's stock replaces
down_frac = 0.25
interest = 0.2
term_days = 60
repo_days = 5
brick_repo_days = 10
impound_days = 10
impound_frac = 0.5
unpaid_wear_mult = 3
repair_below = 50
repair_price = 2                   # coins per condition point
fail_shock = 0.1
loot_window_hours = 12
rip_courage = 0.5
scav_strip_base = 0.15
carry_base = 20
carry_per_strength = 10
pack = [10, 25]
```

## 2. Vehicles

### Movement

A trip uses a vehicle when the agent's `Kit.vehicle` is `Parked` at the building the agent is leaving (Home, workplace, a Garage); the vehicle goes `InUse` and is `Parked` at the destination on arrival, so a car follows its owner door to door. A trip from a street tile is walked. Step durations are counted in quarter ticks: `step_q = max(1, round(4 × move_ticks_full × mult[vehicle][tile kind]))`, plus M12's litter delay, a lookup on a step already being taken. `ExecState::Goto` gains `carry_q: u8`; a step is taken each time the accumulated quarters reach 4, up to `max_steps_per_tick` steps per call. Walking uses `mult = Kit.walk_mult` (1.0 without Legs chrome), so an unchromed walker moves exactly as in M12.

| Vehicle | Road | Ground, Door | Farmland | Haul (× `haul_batch`) |
| --- | --- | --- | --- | --- |
| Motorcycle | 0.25 | 0.6 | 0.8 | 1 |
| Car | 0.2 | walk | walk | 2 |
| Truck | 0.3 | walk | 0.6 | 6 |
| Flyer | timed | timed | timed | 1 |

A **Coarse** `GotoTimed` multiplies its Manhattan estimate by `Kit.timed_mult = road_share × mult_road + (1 − road_share) × mult_off` (1.0 for "walk"). The **flyer** is a timed hop at every tier, Chebyshev distance × `flyer_ticks_per_tile`, landing at the door: it never touches a flow field, and a new `ExecState::Fly` draws it over walls. That is the verticality hook: a flyer already ignores the ground layer, so M14's portal pathing and the layers milestone extend `Fly` instead of inventing a second mover.

### Hauls and fleets

`HaulToMarket` takes `haul_batch × haul[kind]` when a truck or car is `Parked` at the Farm; the hauler drives it to the Market and the plan gains `GoTo(Farm)` to bring it back, so the next hauler finds it. Fewer haul trips free Farm labour; calibration watches the food price for it (§ 11). Fleets are corp and gang purchases (§ 6): a truck per Farm for Food corps, cars for Security Offices (private guards drive their client route), bikes for gangs (a raid march rides). A fleet vehicle is `Parked` at its owner's building and driven by whoever works there.

### Crashes

At the arrival of every vehicle trip (Full and Coarse), roll `p_crash = crash_per_tile × road_tiles × speed[kind] × (1 + litter_d) × chase × (1 − 0.5 × driver reflex)`, where `chase = chase_mult` when the driver holds Flee or HideFromLaw or is a guard on Arrest, else 1. A hit picks a tile of the trip (the midpoint for a timed trip) and the nearest Full or Coarse body within `crash_radius` not in a vehicle; with none, one Statistical resident of the district is sampled as M12 riots do. The victim saves with `p_dodge = min(dodge_w × (Body.reflex + Kit.reflex), dodge_cap)`. Unsaved: `p_crash_kill` kills (`DeathCause::Accident`, by the driver), else an injury (memory `Crashed` 0.7 −0.7, safety −0.6, energy −0.3). The vehicle loses `crash_wear`; litter `crash_litter` at r 1. A kill files `Crime::Manslaughter` against a driver who is seen (the M9 witness path), Murder when the driver was fleeing. `Crash` event. M12's crossfire bystander roll gains the same dodge term, so reflex chrome saves from bullets too.

### Theft and chop shops

A parked vehicle at a building that is not a Garage and has no powered robot is street-parked.

- **Full and Coarse:** `StealVehicle` (new `ActionKind`, dur 15, cost `steal_vehicle_cost`) joins the theft family's plans for adults with `lawfulness < 0.4`. The thief beats the lock by `security::contest(thief_tier, vehicle tier)` with `thief_tier = 1 + round(2 × (Skills.stealth + Kit.stealth))`; a loss is a botched theft (witness roll, as Market theft). A gang member drives it to the Hideout; anyone else to the nearest Hideout to `Fence` it (the existing action) for `fence_frac × value` from that gang's treasury. `Crime::GrandTheft`, `VehicleStolen` event.
- **Off screen:** daily per street-parked vehicle whose owner is Statistical, `p = vehicle_theft_base × crime_rate_d ÷ city mean × (2 − coverage_d)` (× `garage_mult` in a Garage). A hit is bound at once to the gang that controls the district, else the nearest Hideout's gang: no hole, because the vehicle has to go somewhere now.
- **Chop:** daily, a gang keeps a stolen bike or car while it has fewer than `bikes_max_share × members` vehicles and chops the rest at its Hideout: `parts_per[kind]` Parts into its stock, `Chopped` event, the asset despawns. Gangs sell Parts to Clinics and Garages at `parts_price` (`Flow::Parts`, taxed as wholesale), which spend them against imports: the chop shop has a buyer.

### Config

```toml
[vehicles]
road_share = 0.7
max_steps_per_tick = 4
flyer_ticks_per_tile = 0.3
mult = { motorcycle = [0.25, 0.6, 0.8], car = [0.2, 1.0, 1.0], truck = [0.3, 1.0, 0.6] }   # road, ground, farmland
haul = { motorcycle = 1, car = 2, truck = 6, flyer = 1 }
crash_per_tile = 0.00004
speed = { motorcycle = 1.5, car = 1.0, truck = 1.2, flyer = 0.0 }
chase_mult = 6.0
crash_radius = 2
dodge_w = 0.6
dodge_cap = 0.9
p_crash_kill = 0.3
crash_wear = 30
crash_litter = 24
steal_vehicle_cost = 10.0
fence_frac = 0.2
vehicle_theft_base = 0.004
garage_mult = 0.2
bikes_max_share = 0.5
```

## 3. Chrome at the Clinic

### The Clinic

`BuildingKind::Clinic` (label "Ripperdoc", letter `R`), appended; staff `Role::Ripperdoc` (label "Ripperdoc"; `[buildings] clinic = { capacity = 6, stock_cap = 200, staff = 2 }`); foundable through `Register` (`found_cost.clinic` 500, `value` 600, upkeep 10). It sells implants (`Install`) and treatment (`Therapy`, `Detox`), buys used implants at `buyback_frac × value` and resells them at `used_frac` of list (cheap chrome for the poor), and buys Parts. Seeded on Lots by `founding::build_on_lot`: one Tech-corp Clinic in the Spire, and two back-alley docs in Sump West and Mid East owned by the jobless adults living nearest them, as M11 D11 dealt Bars.

### Implants

One implant per `Slot`, tier 1–3. `Install` (new `ActionKind`, dur 120, at a Clinic with a Ripperdoc inside) pays the list price, or `install_fee[tier]` for an implant the agent's gang brings from Hideout stock, and costs `install_shock` sanity at once.

| Slot | Modifier per tier | Read by | Visible |
| --- | --- | --- | --- |
| Arms | `fighting` +0.1, `strength` +0.25 | `resolve_fight`, carry, `Abduct` | yes |
| Legs | `walk_mult` 0.85 / 0.75 / 0.65 | step duration (§ 2) | no |
| Nerves | `reflex` +0.15 | fight, crash and crossfire saves | no |
| Eyes | `sight` +2 tiles, `stealth` +0.05 | witness and sighting radius, thief tier | yes |
| Skin | `armour` 0.15 | fight, crash and crossfire kill rolls | yes |

`law::resolve_fight` becomes `p_win = clamp(0.5 + 0.4 (fi_a − fi_b) + 0.1 (C_a − C_b) + 0.2 (K.reflex_a − K.reflex_b), 0.1, 0.9)` with `fi = Skills.fighting + Kit.fighting`, and the death roll `fight_death_p × (1 + fi_winner) × (1 − K.armour_loser)`. `raid::strength` reads the same `fi`. Unchromed fighters roll exactly as in M12.

**The tier contest.** `security::contest(attacker: u8, defender: u8, rng) -> bool` with `p = clamp(0.5 + contest_step × (attacker − defender), 0.05, 0.95)` is the one rule `VISION.md` asks for. M13 calls it for a thief against a vehicle lock and against a posted robot's sensor (§ 5); M14 calls it for deck against ICE. Chrome is the attacker side: Eyes tiers raise a thief's tier through `Kit.stealth`.

### Sanity and cyberpsychosis

Daily, `sanity` moves toward `target = 1 − Kit.load` by `sanity_drift`, and by an extra `−unmedicated_drift` per implant whose upkeep (neuroblockers) is unpaid. `sanity_cost = [0.08, 0.15, 0.25]` per tier, so three tier-2 implants hold a body at 0.55 and a full tier-3 set drives it to zero.

- Below `edgy` (0.5): `body_bias −edgy_mood`; the Fight goal's flat rises by `edgy_fight`.
- Below `psycho` (0.25): a daily roll `p_episode = episode_base × (psycho − sanity) / psycho` (× `stim_episode_mult` within 12 h of a stim) starts an **episode**: `episode_until = now + episode_hours`, `Episode` event; a Statistical agent is promoted to Coarse for it (LOD priority, as M12 rioters). During an episode the Fight goal is pinned with the nearest living body in sight as the target, `Attack` kills at `fight_death_p × berserk_kill_mult`, and every guard allocated to the district takes an `Arrest` on the agent that kills at `psycho_kill` when contested (× M12's `crush_kill_mult` under Crush). An episode ends at `episode_until`, at arrest or at death; a survivor gains sanity +0.1 and memory `Episode` (0.9, −0.8). Witnesses remember `SawEpisode` (0.6, −0.6). A gang loses each episode survivor whose loyalty is below 0.5 (`GangLeave`): this is how a poor chromed gang devolves.

**Treatment.** `Therapy` (dur 60, `therapy_price`) gives sanity +`therapy_gain`; `Uninstall` (dur 60, the Clinic buys the implant back) lowers the load. The Treat goal (§ 4) carries both.

### Harvesting

- **The Harvest order** (new eighth `Order`), scored with the others. Members under Harvest pick the living non-member with the highest `Kit.chrome_value` among those with `Kit.visible ≥ harvest_min_visible` in held or open districts; only visible chrome is seen, so hidden Nerves and Legs are safe. A Statistical target is promoted to Coarse when the plan binds. Plan: `GoTo(Target) → Abduct → GoTo(Hideout) → Rip`. `Abduct` (dur 5) is `resolve_fight` with the abductor's fighting summed over members within 2 tiles; a win carries the victim as a corpse is carried (`exec::follow_bearer`). `Rip` (dur 60 at the Hideout) moves every implant to Hideout stock and kills the victim with `p_rip_kill` (`DeathCause::Violence`), else releases them at sanity 0.1 with memory `Abducted` (1.0, −1.0). `Crime::Abduction`, `Abducted` and `Harvested` events. The gang installs its take on members at `install_fee` or sells it to a Clinic.
- **Off screen:** daily, while any gang holds Harvest, each Statistical adult with `Kit.visible ≥ harvest_min_visible` rolls `p_abducted = abduct_base × Kit.chrome_value / 1000 × (2 − coverage_d)`. A hit kills the victim (`demography::kill`; the body is found stripped) and opens a `HoleKind::Abducted` hole whose implants sit in `AssetLoc::Limbo`: bound, they go to the stock of the binder's gang; Unknown, they are destroyed, as `Hole.loot` coins are.

| Order | Considerations |
| --- | --- |
| Harvest | `Can(own ≥ 3 ∧ a Clinic exists ∧ a target exists)` → GATE; `L.greed` → Quadratic{2,1,0,0}; `1 − L.lawfulness` → Linear{0.6,0.4}; `1 − U(treasury ÷ hoard_heat)` → Linear{0.5,0.5}; `1 − heat` → Linear{0.6,0.4}; `1 − target_cover` → Linear{0.8,0.2}; flat `order_flat.harvest` |

```toml
[chrome]
install_fee = [30, 80, 200]
install_shock = 0.05
sanity_cost = [0.08, 0.15, 0.25]
sanity_drift = 0.05
unmedicated_drift = 0.02
edgy = 0.5
edgy_mood = 0.2
edgy_fight = 0.1
psycho = 0.25
episode_base = 0.05
stim_episode_mult = 2.0
episode_hours = 3
berserk_kill_mult = 4.0
psycho_kill = 0.3
therapy_price = 60
therapy_gain = 0.15
buyback_frac = 0.4
used_frac = 0.6
contest_step = 0.25
harvest_min_visible = 3
abduct_base = 0.002
p_rip_kill = 0.5
[gangs.order_flat]                 # addition
harvest = 0.0
```

## 4. Stims, addiction, the drug trade

### The good

`Good::Stims`: one unit is a dose. **Supply** is the gangs' cook: daily, a gang with a Hideout cooks `min(cook_per_member × members, cook_target − stock, treasury ÷ cook_cost)` doses into its Hideout stock, paying `cook_cost` a dose as `Flow::Import` (precursors through the port). With `levers.stims_legal`, Street Markets restock stims from the Reserve as they restock food (`stim_restock_floor`, `Flow::Import`) and sell at `stim_price × the owner's Food price_level` (`Flow::Stims`, taxed): legal stims undercut the dealers and the gangs lose the trade.

**Dealing** is a `GangWork` variant. A loyal member under any non-raid order deals instead of extorting when the gang's stock ≥ `deal_batch` and the member falls in the leader's `deal_share = 0.3 + 0.4 × L.greed` of members by a stable hash: `GoTo(Hideout) → PickUp → GoTo(Bar) → Deal`. `Deal` (dur 240, a shift at the Bar nearest the gang's held district) registers the dealer in `World::dealers: BTreeMap<EntityId, SmallVec<[EntityId; 2]>>` (Bar → dealers), so buyers resolve `LocationKey::StimSource` (the nearest Bar with a dealer on shift, or a legal Market) and pay `deal_price × (0.8 + 0.4 × L.greed)` to the gang treasury less `dealer_cut` to the dealer's wallet (`Flow::Stims`, untaxed). Every sale rolls the M9 witness path for `Crime::Dealing` (salience 0.5) against the dealer; an arrest confiscates the dealer's stock. Dealing reports count toward Crackdown pressure as Shakedowns do. M12's raid on a rival Hideout loots its stims and Parts; M12's riot looting takes a Market's stims with its food.

### Use and addiction

`UseStim` (dur 2, from inventory): energy +`stim_energy`, `body_bias +stim_mood` for `stim_hours`, `addiction += addict_per_use × (1.5 − lawfulness)`, `last_use = now`; at `addiction ≥ 0.8` each use rolls `p_overdose` (`DeathCause::Overdose`, `Overdose` event). A day with no use lowers addiction by `addiction_decay`. **Hooked** is `addiction ≥ hooked`. **Withdrawal** is hooked with no use for `withdrawal_hours`, derived from `last_use` on read with no per-tick counter: `body_bias −withdrawal_mood`, energy decay × `withdrawal_energy`, theft cost − `withdrawal_steal_bonus` (as `steal_starving_bonus`), the Fight flat + `edgy_fight`. Sanity is untouched; the episode multiplier is the only stim–chrome coupling.

### Goals (Full and Coarse)

| Goal | Considerations | Plan |
| --- | --- | --- |
| GetHigh | `Can(stims > 0 ∨ (StimSource reachable ∧ coins ≥ price))` → GATE; `craving = addiction × clamp(hours since last_use ÷ withdrawal_hours, 0, 1)` → Logistic{8,0.4}; `1 − energy` while on shift → Linear{0.5,0.2}; `1 − (mood + 1)/2` → Linear{0.4,0.4}; `1 − lawfulness` (illegal only) → Linear{0.5,0.5}; flat `get_high_flat` | `[GoTo(StimSource) → BuyStims] → UseStim` |
| Treat | `Can(a Clinic reachable ∧ coins ≥ price)` → GATE; `max(edgy − sanity, addiction − hooked + 0.2)` clamped 0..1 → Logistic{8,0.3}; `U(wealth)` → Linear{0.6,0.4}; `lawfulness` → Linear{0.4,0.6} | `GoTo(Clinic) → Therapy` or `Detox` |
| Loot | `Can(an unstripped corpse within loot_reach that is not kin, spouse or a fellow member)` → GATE; `1 − lawfulness` → Linear{0.6,0.4}; `1 − U(wealth)` → Linear{0.5,0.5}; `greed` → Linear{0.5,0.5}; `1 − coverage_d ÷ 2` → Linear{0.5,0.5} | `GoTo(Corpse) → Strip [→ Rip]` |
| Shop | § 6 | § 6 |

`Detox` (dur 240 at a Clinic, `detox_price`): addiction × `detox_mult`. **Statistical:** a daily pass over hooked Statistical adults buys and uses `uses_per_day` doses from a dealer in their district (or a legal Market), paying as above; short of either, the agent is in withdrawal and its row's `p_steal` is × `withdrawal_steal_mult`. One with coins ≥ 2 × `detox_price` and lawfulness ≥ 0.5 detoxes with `p_stat_detox`.

```toml
[stims]
cook_cost = 1
cook_per_member = 2
cook_target = 200
deal_batch = 10
deal_price = 5
dealer_cut = 1
stim_price = 6
stim_restock_floor = 300
stim_energy = 0.4
stim_mood = 0.3
stim_hours = 6
addict_per_use = 0.06
addiction_decay = 0.02
hooked = 0.5
withdrawal_hours = 24
withdrawal_mood = 0.4
withdrawal_energy = 1.3
withdrawal_steal_bonus = 3.0
withdrawal_steal_mult = 1.5
p_overdose = 0.002
uses_per_day = 2
detox_price = 80
detox_mult = 0.3
p_stat_detox = 0.05
get_high_flat = 0.0
loot_reach = 12
[levers]                           # addition
stims_legal = false
```

## 5. The security robot

A `Robot` asset, tier 1–3, is sold by a Security Office (the Security niche's second product, revenue to its corp, `Flow::Asset`) and `Posted` at a building. It has a `Position` at the building's door and no Brain, Needs, Memory or LOD. Its upkeep is its power (the Power hook); unpaid, it powers down.

- **Defender.** `raid::fight_out` puts powered robots posted at the target first in the defenders' list (M12 corp raids, riots, breaches). `law::fighting` returns `robot_fighting[tier]` and `law::courage` 1.0 for a robot; `resolve_fight` skips memory, drift and edges on a non-agent side. A robot that loses is wrecked (`condition 0`, Parts at the door), never killed: no corpse, no Murder.
- **Sensor and detainer.** A theft or Shakedown at the building rolls `security::contest(thief tier, robot tier)` before the witness roll. If the robot wins it files a report with itself as witness and fights to detain (`resolve_fight`); a detained thief is a cuffed suspect for the nearest city or private guard.
- **Hack hook.** `Asset.tier` doubles as the ICE tier M14's decks contest; a hacked robot is a `Posted` asset whose owner changes for `hack_hours`. Nothing in M13 hacks.

**Secure buys robots.** M11's `Secure`, per building it would contract: buy a robot instead when `price(tier) + upkeep(tier) × robot_horizon_days < contract_per_guard_day × price_level × robot_horizon_days` and the treasury ≥ `robot_cash_mult × price`, at the tier `robot_tier_by_kind`. A robot replaces the contract on that building. The two Security corps compete on robot prices through `price_level` as on contracts.

```toml
[robots]
robot_fighting = [0.5, 0.75, 1.0]
robot_horizon_days = 60
robot_cash_mult = 2.0
robot_tier_by_kind = { farm = 2, market = 2, home = 1, security_office = 3, clinic = 2, garage = 2 }
hack_hours = 24                    # M14
```

## 6. Who buys: the Shop goal, corps and gangs

**Shop** (Full and Coarse adults, cooldown `shop_cooldown_days`) scores one candidate per call, the best affordable offer among a vehicle (`commute` = Manhattan(Home, workplace) → Logistic{8, `commute_ref`}), chrome (`Can(gang member ∨ guard ∨ courage ≥ 0.6)`, then `courage` → Linear{0.5,0.5} and `1 − Kit.load` → Linear{0.6,0.4}) and a pack (`Can(homeless)`). Shared considerations: `Can(coins ≥ down_frac × price ∧ a seller exists ∧ no finance in arrears)` → GATE; `U(wealth)` → Linear{0.6,0.4}; `pride` → Linear{0.5,0.5} (status); flat `shop_flat`. Plan: `GoTo(Seller) → BuyAsset [→ Install]`. **Statistical** adults run the same scoring in a daily pass (each agent on one hash-picked day in seven, O(adults ÷ 7)) and buy remotely at the nearest seller, as M12 books Hotel beds; Statistical chrome is installed in place.

**Corps** (no new order; existing orders spend, one purchase per corp per day):

| Order | Asset spend |
| --- | --- |
| Grow (Food) | a truck for an owned Farm without one |
| Grow (Security) | a car for an Office, one per three guards |
| Grow (Tech) | builds a Clinic or a Garage on a Lot, the kind with fewer per capita |
| Secure | a robot instead of a contract (§ 5) |
| Hunker | sells one fleet vehicle to the nearest Garage at `buyback_frac × value` |
| Squeeze, Undercut (Tech) | `price_level` moves list prices and fees, as on food |

`Niche::Tech`: `demand` = asset sales ÷ assets in stock over 7 days; `share` = this corp's Tech sales ÷ all Tech sales over 7 days. A ninth seed corp, `Zetatech` (Tech, treasury 5000, a Clinic in the Spire and a Garage in the Civic district; placeholder name in config).

**Gangs** (daily, in the gang pass, after the cook): a gang with treasury ≥ `gang_buy_floor` buys one motorcycle for the highest-rank member without a vehicle, up to `bikes_max_share` of members, and one Arms implant for its strongest member without one; Hideout stock is installed first, at `install_fee`.

### The Garage

`BuildingKind::Garage` (label "Garage", letter `V`), appended; staff `Role::Mechanic` (label "Mechanic"; `[buildings] garage = { capacity = 8, stock_cap = 400, staff = 3 }`); foundable (`found_cost.garage` 600, `value` 800, upkeep 10). It sells vehicles (new, used, repossessed), repairs, buys Parts, takes towed and impounded vehicles, and parks: a vehicle `Parked` in a Garage is not street-parked and pays `garage_rent` a day to its owner (`Flow::Rent`). Seeded: Zetatech's in the Civic district and an agent-owned one in Sump Central.

```toml
[shop]
shop_cooldown_days = 7
commute_ref = 60
shop_flat = 0.02
gang_buy_floor = 600
garage_rent = 1
[corps]                            # additions, merged into the existing tables
found_cost = { bar = 200, home = 400, hotel = 250, clinic = 500, garage = 600 }
value = { farm = 1000, market = 1000, security_office = 500, clinic = 600, garage = 800 }
upkeep = { clinic = 10, garage = 10 }
```

## 7. The Statistical tier and the Kit

Every asset effect has an off-screen reading, each a multiplier on the hour's existing `StatRow` or a daily pass. The 24-row table is unchanged and `calibrate` is re-run, not redesigned; the learned-policy experiment's conclusion stands (keep the table).

| Asset | Full and Coarse | Statistical |
| --- | --- | --- |
| Vehicle | step durations, Coarse `timed_mult`, crashes on arrival, theft by a plan | upkeep, finance and the theft roll in the daily pass; `p_robbed × (1 + flash_w × Kit.flash)` |
| Chrome | `resolve_fight`, sight, stealth, dodge, walk | `p_assaulted` and `p_killed × (1 − dodge_w × Kit.reflex) × (1 − Kit.armour)`; `p_robbed × (1 + flash_w × Kit.flash)`; sanity daily; an episode promotes to Coarse; the abduction roll |
| Stims | GetHigh, UseStim, withdrawal effects | the daily addict pass; `p_steal × withdrawal_steal_mult` in withdrawal; `body_bias` on mood |
| Pack | carry | none |
| Robot | building-side, resolved at any tier | building-side, resolved at any tier |

The multipliers read the cached Kit: no scan, no loop beyond the hour's existing one. The binder (M10) weights a candidate actor by `1 + chrome_bind_w × Kit.fighting` for Assaulted and Killed holes, so the chromed are the likelier culprits off screen too.

```toml
[lod]                              # additions
flash_w = 0.5
chrome_bind_w = 1.0
```

## 8. Player levers and god commands

| Command | Effect |
| --- | --- |
| `SetStimsLegal(bool)` | Prohibition, or legal sale through Markets. |
| `SetAssetTax { kind, rate }` | An extra share of upkeep to the Treasury per asset kind (a vehicle tax, a chrome levy). |
| `SetImpound(bool)` | Whether the city impounds for unpaid upkeep. |

CLI: `--lever day:SetStimsLegal:on`, as the existing syntax. **God commands** (`GOD_SCENARIOS` style, outside the rules): `GrantAsset { agent, kind, tier }`; `Wreck(asset)`; `ChromeEveryone { tier }` (every adult gets Arms and Nerves of that tier); `FloodStims { district, n }` (n doses into every Bar dealer's stock there); `Brick(lender)` (every implant that lender financed is bricked at once); `Chase(agent)` (pins `chase` on the agent's next trip).

## 9. UI, events, CSV

New `EventKind`s (cyan): `AssetBought`, `Repossessed`, `Wrecked`, `VehicleStolen`, `Chopped`, `Crash`, `Installed`, `Episode`, `Treated`, `Abducted`, `Harvested`, `Stripped`, `Overdose`. New `DeathCause`s `Accident`, `Overdose`; `MemoryKind`s `Repossessed`, `Crashed`, `Stripped`, `Episode`, `SawEpisode`, `Abducted`; `Crime`s `GrandTheft` (label "Grand Theft"), `Dealing`, `Abduction`, `Manslaughter`; `GoalKind`s `Shop`, `GetHigh`, `Treat`, `Loot`; `ActionKind`s `BuyAsset`, `Install`, `Uninstall`, `Therapy`, `Detox`, `BuyStims`, `UseStim`, `PickUp`, `Deal`, `Strip`, `Rip`, `StealVehicle`, `Abduct`; `Flow`s `Asset`, `AssetUpkeep`, `Finance`, `Import`, `Stims`, `Parts`, `Treatment`.

- **Inspector**: a Body block (strength, reflex, sanity and its band, addiction, hooked or in withdrawal, episode), a Kit block (vehicle and its condition; each implant with tier, condition and bricked flag; finance and arrears), carry load over capacity, `Appearance`.
- **Asset panel** (new, from any asset link): kind, tier, owner, location, condition, value, upkeep, finance, history (bought, stolen, towed).
- **Building panel**: assets parked, posted or in stock; a Clinic's installs and treatments over 7 days; a Garage's sales and repairs; a Hideout's stims and Parts.
- **City panel**: an Assets section (vehicles by kind, chromed agents, mean sanity, hooked adults, dealer and legal sales, robots), and the levers.
- **Map**: a driver drawn with a vehicle glyph (bike, car, truck), flyers drawn over walls, a robot as a square at its door, an agent in an episode flashing red; an overlay (`A`) heats districts by hooked adults.
- **CSV** (`--report`): `vehicles_moto`, `vehicles_car`, `vehicles_truck`, `vehicles_flyer`, `truck_hauls`, `walk_hauls`, `commute_tpt_walk`, `commute_tpt_drive` (ticks per tile on `GoTo(Workplace)`), `chrome_installs`, `chrome_agents`, `mean_sanity`, `episodes`, `hooked`, `stims_dealt`, `stims_legal`, `dealing_reports`, `repos`, `impounds`, `crashes`, `crash_deaths`, `vehicle_thefts`, `chops`, `abductions`, `stripped`, `robots`, and ledger columns `flow_asset`, `flow_asset_upkeep`, `flow_finance`, `flow_import`, `flow_stims`, `flow_parts`, `flow_treatment`.

## 10. Save compatibility

Every new field is `#[serde(default)]`; `Kit` and `assets_at` are rebuilt on load. A pre-M13 save gets a `Body` per agent drawn from a stream keyed by entity index (deterministic, and the world stream is not consumed), no assets, empty `stock_goods`, no dealers. `BuildingKind::{Clinic, Garage}`, `Role::{Ripperdoc, Mechanic}`, `Niche::Tech`, `Order::Harvest`, `HoleKind::Abducted` and every enum variant above are appended, so serialized indices do not shift. A corpse from an old save has its Wallet already inherited and nothing to strip. `Config::v1_profile` sets `[assets] enabled = false`, which turns every pass, goal, order and Kit term off and leaves `Body` inert, so the v1, M8 and M9 unit tests run unchanged.

## 11. Testing and calibration

**Unit tests** (`citysim/tests/assets.rs`, `vehicles.rs`, `chrome.rs`, `stims.rs`): a purchase pays the seller, the import reaches the Treasury and money is conserved over a daily pass with finance and upkeep; a finance plan in arrears tows a vehicle to the lender's Garage and bricks an implant, then transfers it; unpaid upkeep impounds; wear wrecks at 0 into Parts; a car cuts a road trip to the documented ticks and leaves the flow-field cache untouched; a Coarse driver's timed estimate is `timed_mult` × a walker's; a flyer crosses a wall in Chebyshev time; a truck hauls `6 × haul_batch`; a fleeing driver's crash hits a body within the radius and a high-reflex victim survives more often; the contest is 0.5 at equal tiers; a stolen vehicle is chopped into `parts_per` Parts; an unchromed fight rolls identically to M12; Arms raise `p_win` and Skin lowers the death roll; a load past `psycho` starts an episode that ends at arrest; Therapy raises sanity; a corpse keeps its Wallet until stripped and is inherited after the window; a buried implant becomes Recycler Parts; Rip takes every implant and a bound Abducted hole hands them to the binder's gang; the cook spends the treasury and a Deal pays the gang and the dealer; a witnessed Deal arrests and confiscates; addiction rises per use, withdrawal follows from `last_use` and Detox cuts it; legal stims let a Market sell; a robot defends in a brawl, detains a thief who loses the contest and powers down unpaid; Secure buys a robot when it is cheaper over the horizon; capacity grows with strength and a pack; a pre-M13 save loads.

**Scenario** (`citysim/tests/scenario.rs`, `#[ignore]` `test_m13_assets_seed_42`): the bullets under *Goals and acceptance*. **God scenarios** (`tests/god.rs`): `ChromeEveryone 2` (does the episode rate rise, does the captain's posture move, do Clinics sell Therapy); `FloodStims` in Sump East (does addiction spread, does a rival gang raid for the stock); `SetStimsLegal on` at day 30 (does gang income fall, does a gang turn to Harvest or Raid); `Brick` on the Spire Clinic's owner (does a gang lose its fighters); `Chase` through the Civic core (a crash death, a Manslaughter or Murder report).

**Calibration**, tuned via `[assets]`, `[vehicles]`, `[chrome]`, `[stims]`, `[robots]` and `[shop]` only:

| Target | Band |
| --- | --- |
| Vehicles owned, day 120 | 150–400 |
| Adults with ≥ 1 implant | 5–20 % |
| Gang members with ≥ 1 implant | ≥ 40 % |
| Episodes per 120 days | 1–8 |
| Hooked adults, day 120 | 1–8 % |
| Dealing share of gang income | 20–60 % |
| Crash deaths per 120 days | 1–10 |
| Repossessions per 120 days | 3–40 |
| Vehicle thefts per 120 days | 10–80 |
| Truck share of hauls after day 30 | ≥ 50 % |
| Summer food price | inside the v1 band 2–8 |

Re-run `calibrate`: the new actions enter the exec tallies, step durations change for drivers and Legs, and the fight roll moves for the chromed.

**Throughput.** Upkeep, finance, wear, repossession, sanity, the addict pass, Statistical shopping and the theft, abduction and scav rolls are daily and O(assets) or O(adults); Kit rebuilds and crash rolls are event-driven (one per purchase, install, theft or trip end); the step duration is one table read on a step already taken, and a driver's extra steps per call are bounded by `max_steps_per_tick`; the Statistical reading is a few multiplies on the hour's existing rolls; a flyer never builds a flow field. Assets number in the low thousands. Nothing new runs per agent per tick.

## 12. Phases

1. **The asset model**: § 1 (`Asset`, `Kit`, `Body`, `Appearance`, goods and `stock_goods`, inventory capacity, the `assets` pass with upkeep, finance, repossession and wear, corpses as loot, the Recycler's Parts), the new flows and CSV ledger columns, `GrantAsset` for tests. Nobody buys yet; a 10-day CSV matches M12 outside the new columns and the corpse-inheritance timing. Commit.
2. **Vehicles**: § 2 and the Garage, the Shop goal for vehicles and packs, step durations, `timed_mult`, the flyer, trucks on hauls, corp and gang fleets, crashes, theft, fencing and the chop. Commit. Watch it live: trucks on the arterials and the first crash are the first visible M13.
3. **Chrome and the Clinic**: § 3, `Niche::Tech` and Zetatech, installs, the fight terms, the tier contest, sanity and episodes, Therapy and the Treat goal, Strip and Rip, the Harvest order and the Abducted hole, the Statistical chrome multipliers. Commit.
4. **Stims and the robot**: § 4 and § 5, the cook, dealing, GetHigh, addiction, withdrawal, overdose, Detox, `Crime::Dealing`, the legal lever, the robot as defender and detainer, Secure buying robots. Commit.
5. **Gate and polish**: § 8 levers and god commands, § 9 panels and overlay, the M13 scenario and god scenarios, calibration, README and docs, then `/code-review high <base>..HEAD` and a fix commit, then push.

## 13. Risks

- **Stims eat the Assault bound.** Withdrawal cheapens theft and raises the Fight flat while episodes add lethal attacks; at 8 % hooked the 42.7/day bound can break. Calibrate phase 4 on `addict_per_use` and `withdrawal_steal_bonus` before episodes are tuned, and keep `[chrome] psycho` low until both are in.
- **Trucks move the food economy.** Six-fold hauls free Farm labour and empty Farms faster; M11's restock floors and the Winter famine were tuned against walking hauls. Watch the Summer price and the Reserve level in phase 2 and trim `haul.truck` before touching `[economy]`.
- **Multi-step driving and the door queue.** Up to four steps a call delivers drivers to doors in bursts, and `door_capacity_per_tick` and reservations were sized for walkers. If arrivals block, the driver parks one tile short and walks in, rather than raising capacity.

## 14. Out of scope (M14 and later)

Data, Virt, decks, ICE and hacking robots (M14; `Asset.tier` and `hack_hours` are the hooks); Power and Water as goods, generators and a `powered` building state (the goods milestone after M14; robot and vehicle upkeep are the hooks); gossip, reputation, grudges from `Stripped`, the reading of `Appearance` and the Guard-the-body goal (M15); contracts, hired guns, Trauma Team, the `Wounded` state and rescue, abduction as leverage, hostages and private prisons (M16); outside parents funding fleets and the import ledger (M17); the player (M18); weapons, ammunition and armour as assets, clothing as a bought asset, public transport and rails, map layers and landing pads, building interiors, safe houses beyond Hideout stock, a Soylent factory, and production chains beyond the cook and the chop.
