# M12: Districts — control, coverage, litter, the street, riots

Companion to `SPEC.md`, `M8_FACTIONS.md`, `M9_LAW.md`, `M10_SCALE.md`, `M11_OWNERSHIP.md` and `VISION.md`. M11 gave the city owners; M12 gives it **places**. A district is where the law is thin or thick, whose gang or corp runs the street, how much garbage is on the road, who sleeps rough, and where the anger pools until it boils over. Every system that M11 left city-wide because it had nowhere to happen (riots, raids on corps, squats, per-district law) lands here. Where this document and the earlier ones disagree, this one wins for M12.

The roadmap row (`M11_OWNERSHIP.md`): *district aggregates (law coverage, control, litter, class), per-district law allocation and private security, litter as an event-driven tile state with cleanup jobs, bystanders in brawls, riots and strikes.* M12 also takes the items M11 § 13 deferred to it (districts and anything per district, riots and crossfire, raids on corp buildings) and the survival-loop rungs `VISION.md` and `ROADMAP_POST_M14.md` addenda 5–7 assign to it (Vagrancy, the Hotel, the Squat, litter as the first `Damage` state, private security as the first security tier). Corps hiring gangs, a corp buying a Hideout, trials and the contract entity stay out (§ 13).

Scale: 2,000 residents on the 256 × 192 map; every number assumes M10 and M11 have landed.

Decisions taken by the implementing agent (overturnable, listed so they are cheap to overturn):

| Question | Decision |
| --- | --- |
| How many districts | Eight, cut from the five zones along road lines: Spire, Civic, Vats, Mid split at x = 64 (71 / 69 Blocks), the Sump split at x = 72 and x = 136 (70 / 64 / 66 Blocks). The two seeded Hideouts fall in Sump West and Sump East; Sump Central is the unclaimed middle a splinter or a new gang can take. Cuts are config, not map data; the cap is 12. |
| What a district is | A plain struct in `World::districts: Vec<District>` indexed by `DistrictId(u8)`, not an entity. Nothing owns a district; control is computed. An entity would add a component store and despawn rules for something that never dies. |
| Zones | `Map.zone` stays as the generator's label and the tier source. Everything that read a zone for *where* (the binder, the trace, the watch tally, class fear) reads a district. `District.zone` keeps the parent. |
| Control | Derived daily from presence (held Homes, owned buildings, guard coverage), never claimed directly. A district below `control_min_share` is `Contested`. Control is a read-out that the brains consume, not a new territory system. |
| One law, many beats | The captain keeps one `Posture` for Jail duty (M9) and gains a per-district `Stance` and a guard allocation. Crackdown moves from the city to a district: up to `max_crackdowns` at once, each against the gang that dominates crime there. |
| Litter cost | Litter delays a Full mover's next step (`next_move_tick`), never the path cost, so flow fields are never invalidated by garbage. Only the `Damage` hook's rubble (disabled in M12) changes walkability, through the existing wall-change invalidation. |
| Cleaning is daily | Sanitation work is credited from the shift ledger at midnight like rent, so every LOD tier cleans the same; the Full sweeper's walk is the visible half. |
| Dregs stay Dregs | Hotel guests and squatters keep `Household.home == None`; they are Dregs who found a rung, not housed. The class is the measure of the bottom of the city, the rung is how it survives. |
| Where Dregs come from | Housing is exactly full at 2,000 (400 Blocks × 5), so supply is the honest lever: 8 Sump Blocks start derelict, unsold estates and abandoned Blocks go derelict, and `rehouse_wait_days` 3 → 7. Not a forced quota. |
| Riots reuse raids | A riot is a muster and a march to a door with a brawl at the end (`raid::fight_out`), with a crowd instead of a gang. Riot-specific code is target choice, looting and the law's response. |
| Crossfire | Every brawl (raid, breach, riot clash) rolls once per bystander body within `crossfire_radius` of the door. Statistical bystanders are sampled only for riots (a riot happens in a crowd; a raid at a Hideout door does not). |
| Fines | A Vagrancy arrest fines `vagrancy_fine` when the Wallet holds it and jails for one night otherwise. Fines for other crimes and trials stay out (§ 13). |
| Gang split | A decapitation shock rolls a split only when the gang holds Homes in two or more districts and has a second lieutenant; the splinter needs a vacant Lot or a derelict building in its district for a Hideout, or there is no split. |

## Goals and acceptance

The city should read as a map of places with owners and moods: a Spire nobody dares rob, a Civic core under the guns, a Sump where the law walks in pairs or not at all, garbage piling up where nobody pays to sweep, Dregs sleeping in a cheap Hotel or a derelict Block, and a district that has been angry for a week marching on its landlord. The target spiral: **eviction wave in a Sump district → Dregs squat a derelict Block → litter and crime rise → the captain pulls guards toward the Spire that pays → unrest above the threshold for three days → a riot loots a corp Market → crossfire kills a bystander → the law Crushes it → submission up, happiness down.** On seed 42, 2,000 residents, 120 days:

- eight districts, each with a non-empty `trace` every day; at least two `DistrictControl` events; at least one district controlled by a gang for ≥ 14 consecutive days;
- per-district allocation moves: on ≥ 60 % of days the inhabited district with the highest crime rate has ≥ 2× the guard allocation of the lowest; a district-level Crackdown held at least once; a district whose control turns to a gang holding ≥ 20 Homes there gets a Crackdown or a lifted allocation within 14 days (the god v2 gang-landlord gap);
- litter: the dirtiest district's mean litter in 0.15–0.50 on ≥ 60 % of days after day 14, the cleanest below 0.05; at least one cleanup reallocation (`Sanitation` assignment change) per 30 days;
- the street: ≥ 10 `Vagrancy` arrests; ≥ 100 Hotel nights sold; ≥ 1 `Squatted` and ≥ 1 `SquatEvicted`; the Dreg class at 1–5 % of adults on ≥ 80 of 120 days;
- riots 1–4, each with ≥ 6 rioters; at least one `Looted`; at least one `Crossfire` hit; no district above unrest 0.8 for 30 consecutive days without a riot;
- raids: ≥ 3 raiders at the door in at least half of all raids; at least one raid on a corp building; no raid departs into a district under Garrison or under a Crackdown against the raider;
- a gang `Split` at least once across seeds 42–44 (the gate checks the three seeds; seed 42 alone is a calibration target);
- Assault events per day stay ≤ 42.7 (the 6.4 v1 bound × 2000/300), starvation deaths and population stay within the scaled v1 bounds, and the v1, M8, M9, M10, M11 and Full-vs-Statistical parity gates still pass;
- throughput stays ≥ 8,000 ticks/s with the Statistical tier.

## 1. Districts

### Data model

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub struct DistrictId(pub u8);

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Controller { #[default] Contested, City, Gang(EntityId), Corp(EntityId) }

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Stance { #[default] Patrol, Crackdown(EntityId), Sweep, Cordon, Withdrawn }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct District {
    pub id: DistrictId,
    pub name: String,
    pub zone: Zone,
    #[serde(skip)] pub buildings: Vec<EntityId>,   // doors inside, sorted; rebuilt on load and on any wall change
    #[serde(skip)] pub walk_tiles: u32,            // walkable non-building tiles (the litter denominator)
    // daily aggregates (recomputed at midnight; `trace` explains each)
    pub population: u32, pub adults: u32,
    pub classes: [u32; 3],                         // Corp, Street, Dreg residents (a Dreg's district is where it slept)
    pub happiness: f32,                            // mean (mood + 1) / 2 of resident adults
    pub coverage: f32,                             // § 2, normalised 0.5..=2.0
    pub fear: f32,
    pub unrest: f32,                               // § 6, Street + Dreg residents only
    pub unrest_streak: u16,                        // consecutive days unrest > riot_threshold
    pub litter: f32,                               // mean litter / 255 over walk_tiles
    pub crimes: VecDeque<u16>,                     // per day, last 7
    pub crime_rate: f32,                           // crimes per 100 residents per day over 7 days
    pub control: Controller, pub control_share: f32,
    pub control_since: Tick,
    // the law (§ 2)
    pub stance: Stance, pub stance_since: Tick,
    pub guards: u8,                                // city guards allocated today
    pub sweepers: u8,                              // sanitation workers allocated today (§ 3)
    pub curfew: bool,
    pub last_riot: Option<Tick>,
    pub last_strike: Option<Tick>,
    #[serde(skip)] pub trace: Vec<(&'static str, f32)>,
}

impl World {
    pub fn district_of(&self, p: TilePos) -> DistrictId;          // one byte read from `district_grid`
    pub fn district(&self, d: DistrictId) -> &District;
    pub fn district_of_building(&self, b: EntityId) -> DistrictId; // by door
}
```

`World::districts: Vec<District>` (saved) and `World::district_grid: Vec<u8>` (one byte per tile, 48 KB, rebuilt on load from `Map.zone` and the cuts). A map without zones (v1) gets one district, `City`, so every rule degrades to a single place.

### Config

```toml
[districts]
# One row per district: name, zone letter, and an x-range inside that zone (tiles, cut on road lines).
names  = ["Spire", "Civic", "Vats", "Mid West", "Mid East", "Sump West", "Sump Central", "Sump East"]
zones  = ["S", "C", "V", "M", "M", "U", "U", "U"]
x_from = [0, 0, 0, 0, 64, 0, 72, 136]
x_to   = [256, 256, 256, 64, 256, 72, 136, 256]
control_min_share = 0.4            # below this the district is Contested
control_weights = { held_home = 1.0, owned_home = 1.0, owned_other = 3.0, city_per_coverage = 1.0 }
```

### Aggregates (`systems/districts.rs`, daily)

Runs at midnight after `classes`, before the economy: tick order `commands, lod, needs, memory, mood, think, plan, exec, ownership, classes, districts, economy, [bind], law, social, gang, corp_brain, demography, stats`. One O(agents) pass bins every living adult by the district of their Home door, or for the homeless by the district they ended the day in (`DayTrace`), and fills `population`, `classes`, `happiness`, `fear` (M11's `home_fear`, which replaces `classes::zone_fear`); the crimes counter is fed event-driven by `law::raise_crime` at the crime tile. Control presence per faction:

| Faction | Presence |
| --- | --- |
| Gang `g` | `held_home` × Homes with `claim == Claim { gang: g, count ≥ CLAIM_HELD }` + `owned_*` × buildings `g` owns |
| Corp `c` | `owned_home` × Blocks + `owned_other` × other buildings `c` owns |
| City | `owned_*` × city-owned buildings × `coverage` × `city_per_coverage` |

`control` = the top presence if its share of the total ≥ `control_min_share`, else `Contested`. A change logs `DistrictControl` with both controllers and pushes `Shock::LostDistrict` on a gang that lost it and `CorpShock::LostDistrict` on a corp. `classes::compute` is unchanged; the district aggregates are a second, local cut of the same members.

### What moves from zones to districts

| Before (M10/M11) | M12 |
| --- | --- |
| `ZoneWatch { today: [u32; 5] }`, `law::tally_watch` | `DistrictWatch { today: [u32; 12], yesterday }`; the same per-tick tally keyed by `district_of(tile)` |
| `bind::zone_law_coverage(zone)` | `bind::district_coverage(d)`, the same formula over district Homes; `District.coverage` caches it |
| `DayTrace.zone` (3 bits) | `DayTrace.district` (4 bits in the same `u32`); `zone()` reads `district.zone` |
| `Hole.zone`; binder candidates in the victim's zone (weight 1), any other zone × `other_zone_weight` | `Hole.district`; same district 1, same zone `same_zone_weight` (0.5), elsewhere `other_zone_weight` (0.25) |
| `classes::zone_fear` for a Dreg | the district's mean Home fear |

## 2. The law in districts

### Allocation

Daily after the captain's posture rescoring, the city's patrol guards (those not on Jail duty under the current `Posture`) are dealt to districts. Weight per district:

| Term | Value |
| --- | --- |
| base | `alloc_base` × (`adults` > 0) |
| crime | `alloc_crime` × `crime_rate` ÷ city mean crime rate, clamped to 3 |
| paid | `alloc_paid` × the share of `Lobby` holds and city taxes paid by owners in the district (tax ledger by building door) |
| gang landlord | `alloc_gang_landlord` while `control == Gang(_)` and the gang holds ≥ `gang_landlord_homes` there |
| riot | `alloc_riot` while a riot is mustering or under way there |
| lever | `levers.guard_weight[d]` (default 1.0, 0 abandons the district) |

`guards[d] = largest_remainder(patrol_guards, weight)`. A Withdrawn district (weight 0) gets none. Each patrol guard's beat is the district at its rank among patrol guards; `law::new_patrol_route` draws its Market, Bar and two Homes inside that district (the nearest outside it when the district has none) and keeps the Hall stop for the Civic beat only. `crackdown_route` is unchanged but runs only for guards allocated to a Crackdown district. A guard's beat changes only at its next route, so a reallocation never strands a guard mid-shift.

### Stances (the captain's per-district brain)

The captain scores every inhabited district daily, after allocation, with the M9 template (`utility::curves`, hysteresis `[law] hysteresis`, shocks force a rescore). Inputs per district: `pressure_d` (reports against the district's top gang ÷ `crackdown_reports`), `crime_d` (crime rate ÷ city mean, clamped to 2, halved), `vagrants_d` (Dregs sleeping rough there ÷ `sweep_full`), `riot_d` (1 while a riot musters), `landlord_d` (1 while a gang landlord holds it), `coverage_d`, captain `C`.

| Stance | Effect while held | Considerations |
| --- | --- | --- |
| Patrol | M9's loop, inside the district | `1 − pressure_d` → Linear{0.6,0.4}; flat `stance_flat.patrol` |
| Crackdown(gang) | Crackdown routes for this district's guards; gangs read it (§ 5) | `Can(top gang ∧ guards_d ≥ min_guards ∧ crackdowns < max_crackdowns)` → GATE; `max(pressure_d, landlord_d)` → Logistic{8,0.5}; `C.courage` → Linear{0.5,0.5}; `C.lawfulness` → Linear{0.5,0.5}; flat `stance_flat.crackdown` |
| Sweep | Vagrancy enforcement × `sweep_mult`, one squat cleared per day (§ 4) | `vagrants_d` → Logistic{8,0.5}; `C.lawfulness` → Quadratic{2,1,0,0}; `1 − U(class Corp share)` → Linear{0.5,0.5}; flat `stance_flat.sweep` |
| Cordon | Every allocated guard holds the riot target's door (§ 6) | `Can(riot_d)` → GATE; flat 1.0 |
| Withdrawn | No allocation; set only by the lever or `alloc` weight 0 | — (pinned only) |

The Jail's own `Posture` keeps M9's semantics (Garrison: every city guard holds the Jail); under Garrison every district's allocation is 0 and every stance reads Patrol, which is why the gangs see Garrison as an open city everywhere but the Civic. `Law.target` is kept for saves and set to the first Crackdown target.

### Private security where the law is thin

`corp_brain::secure` already contracts guards for buildings with losses. M12 adds a coverage term: a corp building in a district with `coverage < private_fill_coverage` counts as "at risk" even without a loss, and Secure's `losses` input becomes `max(losses, at_risk_share × private_fill_weight)`. A contracted private guard's route (`private_patrol_route`) is unchanged, and its guard-ticks count in `DistrictWatch`, so private security raises district coverage (and fear) where it works. A corp `Lobby` raises `paid` for every district where it owns buildings for `bribe_days`; that is "the law for sale" made spatial.

### Vagrancy (the street rung)

`Crime::Vagrancy` (label "Vagrancy", salience 0.2). A nightly pass at 03:00 walks the Dregs (O(Dregs)): every adult with no Home, not in a Hotel bed, not in a squat and not inside any building rolls `p = vagrancy_base × coverage_d × (sweep_mult if Sweep) × (curfew_mult if curfew)`. A hit: the nearest on-shift city guard in the district (or none: a sweep is a guard's report with no body) files the report with the guard as witness and the existing arrest path runs for Full and Coarse; a Statistical vagrant is fined or jailed in place (`law::sentence` for one night, `vagrancy_sentence_ticks`). Fine first: `vagrancy_fine` from the Wallet to the Treasury when the Wallet holds it.

### Config

```toml
[law]                              # additions
alloc_base = 1.0
alloc_crime = 1.0
alloc_paid = 0.5
alloc_gang_landlord = 1.5
alloc_riot = 3.0
gang_landlord_homes = 20
max_crackdowns = 2
sweep_full = 10                    # rough sleepers that read as full Sweep pressure
private_fill_coverage = 0.7
private_fill_weight = 0.5
vagrancy_base = 0.04               # per rough night at coverage 1.0
sweep_mult = 3.0
curfew_mult = 2.0
vagrancy_fine = 3
vagrancy_sentence_ticks = 600      # one night
[law.stance_flat]
patrol = 0.2
crackdown = 0.0
sweep = 0.0
```

## 3. Litter and damage

### State

`World::litter: Vec<u8>`, one byte per tile, saved (run-length encoded in the save). Bands: 0–31 clean, 32–95 littered, 96–191 trashed, 192–254 heaped, 255 rubble (the `Damage` hook, below). Deposits are event-driven, at the event tile and spread to Chebyshev radius `r` with half the amount at the rim:

| Event | Amount | r |
| --- | --- | --- |
| Theft (Market or Home) | 6 | 0 |
| Assault, Shakedown | 12 | 1 |
| Murder, Violence death | 40 | 1 |
| Corpse not buried within a day | 24 | 0 |
| Raid / breach brawl | 32 | 2 |
| Riot clash | 64 | 3 |
| Eviction (belongings on the street at the Block door) | 24 | 1 |
| Bankruptcy, derelict building (each door) | 48 | 2 |
| A squat, daily | 4 | 1 |
| A rough sleeper, daily | 2 | 0 |

Decay: daily, every non-zero tile below 255 loses `litter_decay` (1). Statistical crimes deposit at a random walk tile of the victim's district (seeded from the hole id, so a replay matches).

### Effects

- **Movement.** `exec::advance_goto` adds `litter_step_ticks[band]` ([0, 0, 1, 2]) to `next_move_tick` when a Full mover steps onto a tile; a Coarse `GotoTimed` multiplies its estimate by `1 + litter_timed_mult × district.litter` at planning. Flow fields never change.
- **Sleep.** The Sleep safety bonus (`exec/actions.rs`, `+0.2 + 0.1 × tier`) loses `litter_sleep_penalty × litter_at_door / 255`; a rough sleeper loses `0.1 + litter_sleep_penalty × litter_here / 255`.
- **Happiness.** `Mood` gains `district_bias`, set daily to `−litter_mood × district.litter`, added in the mood update; it is how a dirty district reads unhappy without a new need.
- **Witnesses.** A dirty street sees less: the binder's witness roll becomes `p_witness × coverage × (1 − 0.3 × district.litter)`.

### Cleanup

Three cleaners, all credited daily at midnight from work actually done:

1. **The city's Sanitation.** `Role::Sanitation` (label "Sanitation"), city employees at the Recycler (`BuildingKind::Cemetery`), hired and fired to `levers.sanitation_count` by the guard reconcile path (`law::reconcile_guards` generalised to `reconcile_city_role`). Daily, workers are dealt to districts by `litter_d × walk_tiles × levers.sanitation_weight[d]` (largest remainder; a reallocation logs `Sanitation`). Each worker whose shift was completed (`Job.last_shift_day`) removes `clean_per_shift` units from its district's dirtiest tiles. A Full sweeper walks `PatrolLeg`s between the five dirtiest road tiles of its district (the walk is for the eye; the credit is the ledger). Paid from the Treasury like any city job.
2. **Owners.** A corp holding Secure, or any owner with `revenue > upkeep` that day, spends `owner_clean_cost` per unit to clean `owner_clean_units` around each of its doors (radius 2). A broke owner does not, which is how a dying corp's block goes to seed.
3. **The ruling gang.** A gang with `control == Gang(g)` and leader `pride ≥ clean_pride` removes `gang_clean_per_member` per member-day from its held Homes' doors; a gang under a raid order or LieLow does not. Pride keeps turf clean; greed does not care.

### The Damage hook (designed, off)

`litter == 255` is **rubble**. With `[litter] rubble_blocks = true` (default false) a rubble tile is unwalkable: `Map::walkable` reads `litter == 255` as a wall, and writing or clearing rubble calls the existing wall-change invalidation (`exec::invalidate_flow_fields`). Nothing in M12 deposits 255 (deposits clamp at 254); explosions and vehicle wrecks (M13/M16) do. Repair is a sanitation job at `rubble_clean_mult` cost. A damaged building (capacity loss, M13+) uses the same byte on its door tile.

### Config

```toml
[litter]
decay = 1
step_ticks = [0, 0, 1, 2]          # per band: clean, littered, trashed, heaped
timed_mult = 0.5
sleep_penalty = 0.15
mood = 0.2
clean_per_shift = 120
owner_clean_cost = 1               # coins per 32 units
owner_clean_units = 64
gang_clean_per_member = 8
clean_pride = 0.5
rubble_blocks = false
rubble_clean_mult = 4
[levers]                           # additions
sanitation_count = 12
```

## 4. The street: Hotel, derelicts, Squat

### Hotel

`BuildingKind::Hotel` (label "Capsule Hotel"), appended to the enum; capacity = beds (`[street] hotel_beds`, 12); foundable through `Register` alongside Bar and Home (`[corps] found_cost.hotel`, 250). At seed `seed_hotels` Sump Lots are built as Hotels, one in Sump West and one in Sump Central, owned by the jobless adults living nearest them (as M11 D11 deals Bars). A night costs `night_price × owner price_level` (Food niche markup for a corp owner), paid to the owner through `ownership::pay` (`Flow::Hotel`).

- **Full and Coarse:** a homeless agent whose Sleep goal fires with `coins ≥ price` and a Hotel with a free bed within `hotel_reach` tiles plans `GoTo(Hotel) → CheckIn → Sleep`. `CheckIn` (new `ActionKind`, dur 5, cost 4) pays and books a bed until 08:00 (`World::hotel_beds: BTreeMap<EntityId, (EntityId, Tick)>`). Sleep's `at_home` check accepts a booked bed; the tier bonus applies.
- **Statistical:** the nightly 03:00 pass books beds for homeless Statistical adults in order of coins, nearest Hotel first, before the Vagrancy roll.
- A Hotel guest is not rough: no Vagrancy roll, no rough-sleep litter. A night in a Hotel marks `trace_flags::SLEPT_AT_HOME` (it is a bed) and `HOTEL`.

### Derelict buildings

`Building.derelict: bool` (`#[serde(default)]`). A derelict building has `owner = None`, rent 0, no staff, no vacancies, and is not a Home for re-housing. It becomes derelict when:

1. **Seed:** `seed_derelict_blocks` (8) Sump Blocks start derelict and empty, spread by even stride over the three Sump districts; their 40 would-be residents start homeless.
2. **Unsold estates:** `corps::bankrupt` sells to the richest buyer as M11; the city buys the remainder only while the Treasury stays above `city_absorb_floor` (5,000) after paying; otherwise the building goes derelict (`Derelict` event, litter deposit).
3. **Abandonment:** a non-city Block with no occupants for `abandon_days` (14) whose owner's purse is negative is abandoned (derelict, owner None).
4. **Demolition:** `DemolishHome` leaves a demolished Block derelict at half capacity instead of inert.

A derelict building returns to use when bought (`Nationalise`, `Acquire` at `derelict_value` = value(kind) / 4, or a `BuyBuilding` god command), which evicts its squatters (`SquatEvicted`).

### Squat

`Squatter { building: EntityId, since: Tick }` component. A squatter sleeps in the derelict building (Sleep bonus `+0.1`, no tier term), is never rolled for Vagrancy, and stays a Dreg.

- **The Squat goal** (Full and Coarse), for homeless adults who cannot afford a Hotel: considerations `Can(a derelict with a free slot within squat_reach ∧ not banned there)` → GATE; `U(wealth)` (= 1 − wealth: the poor squat; the fix pass corrected this line, which read `1 − U(wealth)`) → Linear{0.6,0.4}; `courage` → Linear{0.5,0.5}; `1 − lawfulness` → Linear{0.4,0.6}; `Can(night)` → Step{1,0.3,1}. Plan: `GoTo(TargetBuilding) → Occupy` (new `ActionKind`, dur 10, cost 6): adds the agent to `occupants`, inserts `Squatter`, logs `Squatted` the first time the building is taken. Statistical Dregs are assigned daily (03:00 pass, after Hotels) to the nearest derelict slot in their district.
- **Securing it** is the gangs' claim machinery: a squat held by a gang (§ 5) has `claim = Some(Claim { gang, count: CLAIM_HELD })`, counts as held territory, and its non-member squatters are evicted on the flip.
- **Eviction** by three hands: the **owner** when it is bought (above); the **law** under a Sweep stance, one squat per district per day, every squatter rolled as a Vagrancy arrest at `p = 1`; a **stronger squatter**, a gang's Squat order (§ 5). An evicted squatter is banned from that building for `squat_ban_days` and remembers `Evicted` (salience 0.6).

### Re-housing

`[rent] rehouse_wait_days` 3 → 7. A Hotel guest or a squatter re-houses by M11's rule once the wait and the deposit (`rehouse_coins_mult × rent`) allow; the derelict seed and abandonment keep supply below 2,000 beds so the wait has a queue. The target, 1–5 % of adults as Dregs, is a calibration target on `seed_derelict_blocks`, `rehouse_wait_days` and `city_absorb_floor`.

```toml
[street]
hotel_beds = 12
night_price = 3
hotel_reach = 48
seed_hotels = 2
seed_derelict_blocks = 8
city_absorb_floor = 5000
abandon_days = 14
squat_reach = 40
squat_ban_days = 14
[corps]                            # addition
found_cost = { bar = 200, home = 400, hotel = 250 }
[rent]                             # change
rehouse_wait_days = 7
```

## 5. Gangs in districts

The fixes the god suites asked for, where they are district-shaped. `faction::OrderInputs` gains:

| Input | Definition |
| --- | --- |
| `target_cover` | the raid target's district: 1 when the Jail is in Garrison and the target is the Jail, or when the district's stance is `Crackdown(self)` or `Cordon`; else `(coverage − 0.5) / 1.5` |
| `districts_held` | districts with `control == Gang(self)` |
| `open_districts` | districts that are `Contested` or held by a gang with no fit members |
| `derelicts` | derelict buildings in held or open districts not held by this gang |
| `corp_prize` | the hoarding corp's (`faction::hoard`) richest-revenue building in a district held by or adjacent to this gang, and its value |

### Orders change

- **Raid, Retaliate, BreakOut** gain `1 − target_cover` → Linear{0.8,0.2} and a GATE `target_cover < 1`: no raid departs into a garrisoned district or a district cracking down on the raider. "Nobody reads the law's strength" (god v1) is closed by this term.
- **Expand**: `frontier` becomes the unclaimed inhabited Homes in held and open districts (the Hideout midline goes), so a vanished rival's district is Expand territory on purpose.
- **Raid** gains a second target: with `corp_prize` set and `hoard_tilt > 0` (0.2 in M12, 0 in M11) the brain scores Raid twice (rival Hideout, corp building) and keeps the better; `Gang.raid_target: Option<EntityId>` names the door. Defenders at a corp building: the corp's private guards on contract within `raid_gather_radius` of the door, its employees inside, and city guards allocated to the district within radius. A win takes `corp_raid_frac` of the corp treasury capped at `corp_raid_cap`, loots the building's food stock into members' inventories, and pushes `CorpShock::Robbed(amount)` and `CorpShock::Raided` (0.7) on the corp.
- **Squat** (new seventh `Order`): `Can(derelicts > 0 ∧ own ≥ 3)` → GATE; homeless members ÷ own → Linear{0.6,0.4}; `L.greed` → Linear{0.5,0.5}; `1 − heat` → Linear{0.6,0.4}; flat `order_flat.squat` (0.05). Members walk to the nearest such derelict and `Occupy` it; three member-visits flip it to the gang (the claim machinery), resident non-member squatters with `courage > 0.6` fight (`law::resolve_fight`), the rest leave. A gang squat is a muster point and a splinter's possible Hideout.

### Musters near the target

`LocationKey::MusterPoint` (new): the gang's held Home or squat nearest the raid target's door, when one lies within `muster_near_tiles` (48) of it; else the Hideout (M8). The plan becomes `GoTo(MusterPoint) → Muster → GoTo(RaidTarget) → Brawl`. Members holding the order gather from wherever they are, so the march is short and `raiders_at` finds them; the god scenarios' 1–2-raider arrivals came from 6-hour marches from a corner Hideout. `raid_gather_hours` stays 8.

### No instant hydra

A gang that empties loses its claims after `empty_claims_days` (3), not at the 30-day disband. The arrested-bootstrap in `gang::recruit_gang` needs, besides an empty gang and a `WasArrested` memory: the empty gang's Hideout district is not held by another gang, its coverage < `reform_max_coverage` (1.2), and the gang has been empty ≥ `reform_days` (14). A sacked Hideout can be re-founded as soon as the sack ends.

### Splits

On `Shock::LeaderChanged` caused by the leader's death or arrest, when the gang holds Homes in ≥ 2 districts: the runner-up of `recompute_leader`'s ranking is the **lieutenant**. If the lieutenant's strength ≥ `split_strength_ratio` × the new leader's and the gang's mean member loyalty < `split_loyalty`, roll `p = split_base × (1 − mean loyalty)`. On a hit: the lieutenant founds a splinter in the held district other than the new leader's that holds the most Homes, taking every member whose Home (or squat) lies there plus anyone with higher affinity to the lieutenant than to the new leader, those Homes' claims, and the treasury share by headcount. Its Hideout is a gang squat in that district, else a vacant Lot there built as a Hideout by `founding::build_on_lot`, else no split. Name from `[gangs] splinter_names`. `Split` event; each side gets an Enemy edge set and a `Shock::Split` (severity 0.8, a grudge). Total gangs are capped at `max_gangs`.

```toml
[gangs]                            # additions
muster_near_tiles = 48
empty_claims_days = 3
reform_days = 14
reform_max_coverage = 1.2
split_strength_ratio = 0.8
split_loyalty = 0.6
split_base = 0.5
max_gangs = 4
splinter_names = ["Rust Saints", "Low Choir", "Gutter Kings"]
corp_raid_frac = 0.05
corp_raid_cap = 500
[gangs.order_flat]                 # addition
squat = 0.05
[corps]                            # change
hoard_tilt = 0.2
```

## 6. Unrest, riots, crossfire, strikes

### District unrest

Per district, the M11 formula over its Street and Dreg resident adults: `loyalty = happiness × min(employment + 0.5, 1)`, `submission = 0.3 + 0.7 × fear` (+ `curfew_fear` under curfew), `unrest = (1 − loyalty) × (1 − submission) + 0.1 × evictions_7d_d / n + rent_burden_w × burden_d`, where `burden_d` is the mean of `rent_per_day ÷ max(daily income, 1)` over its renters (the god v2 "rent never moves unrest" gap). `unrest_streak` counts consecutive days above `riot_threshold`.

### Riots (`systems/riot.rs`)

A district with `unrest_streak ≥ riot_days`, no riot within `riot_cooldown_days`, and ≥ `riot_min` eligible rioters riots. Eligible: resident Street and Dreg adults, free, not Corp class, `lawfulness < riot_lawfulness`, mood < 0. Up to `riot_max` of them, most miserable first, are promoted to Coarse for the riot (LOD priority, as M11 D36). The target, scored once:

| Target | Grievance |
| --- | --- |
| A corp Market, Block or Hotel in the district | the owner's evictions in the district in 14 days + its `price_level − 1` × 10 |
| The Precinct | the district's Vagrancy arrests in 14 days, × 2 under Crackdown or Sweep |
| The controlling gang's Hideout or squat | its Shakedowns in the district in 14 days |

The riot musters at the district's most central Bar or Market at `riot_muster_hour` (20:00) and marches; it reuses `raid::depart`'s stamp and `raid::fight_out`, with rioters as raiders. Defenders: city guards allocated to the district or within `raid_gather_radius` of the door, the target's private guards, its gang's members inside. Outcomes:

- **Won:** looting. A Market loses `loot_frac` (0.3) of its stock into rioters' inventories; a Block or Hotel's owner loses `loot_frac` × the building's 7-day revenue from its purse, split among the rioters' wallets; the Precinct releases `breakout_max_freed` convicts. The building is closed (`closed_until`, `riot_close_days`), `Looted` event, `CorpShock::Rioted` (0.9).
- **Lost / dispersed:** each rioter who lost a pairing is arrested (Assault).
- Either way: `Riot` event (district, target, rioters, defenders, dead), litter, unrest × `riot_vent` (0.5), streak reset, `last_riot` set, every corp owning buildings in the district gets `CorpShock::Rioted` at half severity.

**The law's riot response**, `RiotResponse { Contain, Disperse, Crush }`, chosen by the captain (Contain for courage < 0.4, Crush for lawfulness < 0.4 and courage ≥ 0.6, Disperse otherwise) or pinned by `SetRiotResponse`. Contain: the district stance becomes Cordon and guards fight only at the door. Disperse: guards also arrest stragglers within 8 tiles for 2 hours. Crush: Cordon plus `resolve_fight` with the kill chance × `crush_kill_mult` (3), and the district's fear + 0.3 for 14 days (submission up, happiness down: Garrison-state quiet bought with blood).

### Crossfire

`raid::fight_out` gains a bystander pass, once per brawl: every body (Full or Coarse) within `crossfire_radius` (3) of the door that is not a party rolls `p_crossfire` (0.15); a hit is an Assault on the bystander by the opposing side's strongest fighter (memory `CaughtInCrossfire`, salience 0.7, valence −0.7, safety −0.6), and `p_crossfire_kill` (0.1) of hits kill (`DeathCause::Violence`, by that fighter). A riot clash also samples `riot_stat_bystanders` (4) Statistical residents of the district at the same odds. `Crossfire` event per hit. This is the hazard roll `VISION.md`'s brutality section asks for; M13 vehicles and M16 combat reuse it.

### Strikes become district-aware

M11's strike (`classes::strike`) runs per district: when a district's Street unrest > `strike_threshold`, the corp with the highest `price_level` among those employing its residents loses those residents' next shift; `last_strike` per district replaces the city-wide cooldown. `Strike` names the district.

```toml
[riots]
riot_threshold = 0.55
riot_days = 3
riot_cooldown_days = 14
riot_min = 6
riot_max = 40
riot_lawfulness = 0.5
riot_muster_hour = 20
loot_frac = 0.3
riot_close_days = 3
riot_vent = 0.5
crush_kill_mult = 3.0
crossfire_radius = 3
p_crossfire = 0.15
p_crossfire_kill = 0.1
riot_stat_bystanders = 4
[classes]                          # additions
rent_burden_w = 0.2
curfew_fear = 0.2
```

## 7. Player levers

| Command | Effect |
| --- | --- |
| `SetGuardWeight { district, weight: f32 }` | The lever term in § 2's allocation; 0 withdraws the law. |
| `SetSanitation { count: u8 }`, `SetSanitationWeight { district, weight }` | Sanitation headcount and focus. |
| `SetCurfew { district, on }` | Vagrancy × `curfew_mult`, fear + `curfew_fear`, happiness − 0.05 for residents. |
| `SetRiotResponse(Option<RiotResponse>)` | Pin or release the captain's riot response. |
| `SetStance { district, stance: Option<Stance> }` | Pin a district's stance (Crackdown needs a gang). |

CLI: `--lever day:SetGuardWeight:6:2.0`, `--lever day:SetCurfew:7:on`, as the existing syntax. **God commands** (`GOD_SCENARIOS` style, out of the rules): `Riot(district)` starts a riot at the next muster hour regardless of unrest; `Litter(district, level)` sets every walk tile; `SplitGang(gang)` forces a split with the documented rule minus the roll; `Derelict(building)`; `BuyBuilding { buyer, building, price }` (the v2 gap).

## 8. UI, events, CSV

New `EventKind`s: `DistrictControl`, `Stance`, `Sanitation`, `Vagrancy` (via the crime path), `Squatted`, `SquatEvicted`, `Derelict`, `Riot`, `Looted`, `Crossfire`, `Split` (district colour: amber).

- **District panel** (new; click any street tile with the District overlay on, or a row in the City panel): name and zone, population and class mix, happiness, unrest and streak, coverage and guards allocated, stance and since, fear, litter with its band, crime rate, controller and share (top three presences), sweepers, curfew, last riot, the stance trace (top three), and the aggregate `trace`.
- **Map overlay** (toggle `D`): district borders; fill in the controller's colour (gang colours as M8, corp colours as M11, city grey, Contested hatched); a litter heat layer (toggle `L`) reading `World::litter`; derelict buildings drawn cracked; Hotels with a bed icon.
- **City panel**: a Districts section, one row per district (controller, guards, unrest, litter, crime), and the levers above.
- **Inspector**: district, "squatting in X", "slept at Y Hotel", Vagrancy record.
- **CSV** (`--report`): per district `d{i}_coverage`, `d{i}_control` (0 Contested, 1 City, 2 Gang, 3 Corp), `d{i}_litter`, `d{i}_unrest`, `d{i}_crime`, `d{i}_guards`; city-wide `dregs`, `hotel_nights`, `squatters`, `derelicts`, `vagrancy`, `riots`, `crossfire`, `gangs`.

## 9. Save compatibility

Every new field is `#[serde(default)]`. A pre-M12 save loads with districts rebuilt from its map and config (aggregates zero until the first midnight), `litter` all zero, no derelicts, no Hotels, no squatters, `ZoneWatch` migrated into `DistrictWatch` by giving each zone's count to its first district. `DayTrace` decodes the old 3-bit zone as that zone's first district; `Hole.zone` migrates the same way. `Order::Squat`, `BuildingKind::Hotel` and `Crime::Vagrancy` are appended, so serialized indices do not shift. `Config::v1_profile` turns districts into one, and litter, the street, riots, splits and crossfire off, so the v1, M8 and M9 unit tests run unchanged.

## 10. Testing and calibration

**Unit tests** (`citysim/tests/districts.rs`, `street.rs`, `riots.rs`): `district_of` matches the cuts on the v2 map and is one district on v1; aggregates on a hand-built world give the documented population, class mix, happiness, crime rate and controller; a gang holding most Homes controls and a city building under high coverage beats a lone corp; allocation follows crime and the lever and a weight 0 withdraws; a gang landlord draws a Crackdown; Sweep wins with rough sleepers and a lawful captain; Garrison zeroes allocations; a Vagrancy roll fines a payer and jails a broke vagrant; litter deposits per event and decays; a littered step delays the next move and leaves flow fields cached; sanitation removes the credited units dirtiest-first; a Hotel books, charges the owner's price and blocks the Vagrancy roll; bankruptcy below the floor leaves a derelict; Occupy makes a squatter who stays a Dreg; a Sweep clears one squat; a gang Squat flips a derelict and evicts the meek; Raid is gated by a Crackdown on the raider; the muster point is the held Home nearest the target; an empty gang cannot re-form in a held district or before `reform_days`; a decapitation with a strong lieutenant and two held districts splits and with one does not; a corp-building raid takes the documented prize; a riot fires at the streak, picks the documented target and loots on a win; Crush raises fear; crossfire hits only non-parties within the radius; a pre-M12 save loads.

**Scenario** (`citysim/tests/scenario.rs`, `#[ignore]` `test_m12_districts_seed_42`, and a three-seed `test_m12_split_seeds`): the bullets under *Goals and acceptance*. **God scenarios** (`tests/god.rs`): `Riot` in the Spire (does the law Cordon, does the corp Secure), `Litter` 0.8 in Mid East (does sanitation reallocate, does unrest rise), `SplitGang` (do the halves fight), `SetGuardWeight 0` on a Sump district (does a gang take control within 30 days).

**Calibration**, tuned via `[districts]`, `[law]`, `[litter]`, `[street]`, `[riots]` and `[gangs]` only:

| Target | Band |
| --- | --- |
| Dirtiest district litter (days 14–120) | 0.15–0.50 on ≥ 60 % of days |
| Cleanest district litter | < 0.05 |
| Riots per 120 days | 1–4 |
| Dregs as share of adults | 1–5 % on ≥ 80 of 120 days |
| Raids with ≥ 3 raiders at the door | ≥ 50 % |
| Gang splits, seeds 42–44 | ≥ 1 |
| Max consecutive days any district > 0.8 unrest without a riot | < 30 |
| Hotel occupancy | 30–90 % of beds on average |
| Vagrancy arrests | 10–60 |

Re-run `calibrate`: CheckIn and Occupy are new actions, litter changes walk times, and the Sleep bonus moved.

**Throughput.** Aggregates, allocation, stances, control, sanitation credit, litter decay (one 48 KB pass) and the nightly street pass are daily and O(agents) or O(tiles); Vagrancy, Hotel booking and squat assignment are O(Dregs) nightly; deposits, crossfire and riots are event-driven; the litter step delay is one byte read on a step already being taken; `district_of` is one byte read. Districts are ≤ 12. Nothing new runs per agent per tick.

## 11. Phases

1. **Districts**: § 1, the watch and binder move, `DayTrace` and `Hole` migration, the aggregates, a read-only District panel and the CSV columns. Behaviour unchanged; a 10-day CSV matches before and after outside the new columns. Commit.
2. **The law in districts**: § 2, allocation, stances, the private-security fill, the gang-landlord term, Vagrancy, the gang inputs `target_cover` and the Raid gate. Commit.
3. **The street**: § 3 and § 4, litter, sanitation and the Damage hook (off), Hotels, derelicts, the Squat goal, re-housing; calibrate the Dreg share. Commit. Watch it live: litter heat and rough sleepers are the first visible M12.
4. **Gangs and riots**: § 5 and § 6, muster points, the bootstrap rule, splits, the Squat order, corp raids, crossfire, district unrest, riots, looting, the riot response and district strikes. Commit.
5. **Gate and polish**: § 7 levers and god commands, § 8 overlay and panels, the M12 scenario and god scenarios, calibration, README and docs, then `/code-review high <base>..HEAD` and a fix commit, then push.

## 12. Risks

- **The Dreg share is a housing-supply knob.** If 8 derelict Blocks plus abandonment overshoots, the Dreg pool feeds theft and gang recruitment and the Assault bound breaks. Calibrate phase 3 against the M11 theft rate before riots exist.
- **Riots and LOD.** Promoting up to 40 Statistical rioters to Coarse for a march costs bodies at the exact moment the city is busiest; if the gate drops below 8k ticks/s, resolve the march with `GotoTimed` and promote only at the door.
- **Allocation starves the Civic.** Dealing patrol guards by crime can leave the Precinct district thin, which `target_cover` reads as an open door for a BreakOut. The Garrison posture is the backstop; watch the jailbreak rate against M9's 1–6.

## 13. Out of scope (M13 and later)

Assets and vehicles, chrome, stims and the security robot (M13); Data, Virt, ICE and the tech tree (M14); gossip, reputation, grudges and news (M15); the contract entity, hired guns, corps hiring gangs, a corp buying a Hideout, bounties and trials (M16); the outside world and parents retaking districts (M17); the player (M18); explosions and wreckage that write rubble, rails, map layers and verticality, building interiors and room templates, Power and Water, a second food good beyond the Hotel's night as a service.

## 14. Fix pass (after phase 4): what changed from the text above

The phase 1-3 reviews and the phase 3-4 carry-overs, applied before phase 5. The numbers are seeds 42 / 43 / 44, 120 days.

- **Litter scale (§ 3).** `District.litter` is the share of the district's street tiles at or above `[litter] visible` (32, the littered band), not the mean byte: the mean over ~30k street tiles pinned at 0.001. Every deposit in the table is scaled by `[litter] deposit_mult` (8) and capped at 254, so a theft (48) leaves a visible mark and a death or brawl heaps. An event on a tile inside a building (an indoor theft, a death at home, a brawl at a door) lands outside that building's door (`litter::street_anchor`). Sleep's litter penalty reads the street outside the door (the door tile is inside the rect, where nothing lands). Decay and the sweepers needed no change: the dirtiest district holds 0.26-0.28, in band on 96-98 % of days 14-120; the Vats stay under 0.01. The calibration table's "mean litter" rows read the share.
- **Presence (D8).** A gang-owned Home weighs `owned_home` (1.0), not `owned_other` (3.0); owned and held it is 2x a held Home. A gang's held squat counts whatever the derelict's kind.
- **Allocation (D10), kept.** The phase 2 review asked to gate the paid and gang-landlord terms on a district having residents too. Tried and reverted: the paid term is what puts a beat on the Civic's Markets and the Vats' Farms, and without it the watch piled onto the Homes (extortion 3.8k → 1.3k, gang joins halved on seed 42) and the M11 spiral gate (an evictee joins a gang within 14 days, ≥ 3) fell to 0.
- **Garrison and Patrol (D11-D12, documented costs).** Garrison empties every district: allocation 0, so the district watch reads 0, coverage falls to its floor and the binder's witness roll halves. Patrol's Linear{0.6,0.4} floor means a Crackdown or a Sweep needs a brave, lawful captain.
- **Vagrancy (D15).** A Vagrancy report never feeds the law's report log (wanted gang, pressure, a district's top gang). `District.rough` counts only the sleepers the law can roll. A district without Homes rolls at coverage 1 (it read `coverage_max`, doubling the sweep where no guard walks). `releases` runs every tick, so a night's sentence ends at its tick, not at midnight (verified, item 12).
- **The street (§ 4).** Any path to a Home (`set_home`: re-housing, marriage) ends a squat and a Hotel booking. Abandonment and the City's re-letting take any derelict kind (a Bar or Hotel with no trade for `abandon_days` under an owner in the red is abandoned), and a gang-held squat is not re-let while held. A restored building returns at the capacity it stood at (`Building.full_capacity`), not the config's. A broke owner, corp or not, does not pay to clean. A bankruptcy marks every door of the estate (48 r2; a building that goes derelict lays it once). Hotel upkeep 10 → 4 (a Hotel grosses ~7 a day). Hotel occupancy runs ~20 % of beds (band 30-90 %): a calibration note, not a fix.
- **Corp raids (D39).** A won raid takes `[gangs] corp_raid_loot_frac` (0.25) of the building's food, the coins as before. Up to `corp_raid_posted` (5) of the owner's or contractor's private guards on shift (any shift while the owner holds Secure) are posted at the door; the beat's on-shift guards answer as a riot's do (in the Precinct's district its watch on shift answers too); a crew breaks once `corp_raid_break` (0.5) of it has lost a pairing. A raided or looted corp reads Secure/Lobby `losses` of at least `[corps] raided_losses` (0.5) for `raided_days` (7). Lost 1 / 1 / 2 of 4 / 6 / 6 (25 %); the raided Market's price stayed at or under 8 (the price-12 spells are Market#412 stocking out under Greenline, as before the fix pass).
- **Strikes (D41).** `strike_threshold` 0.45 → 0.51 and `strike_cooldown_days` 7 → 30 (district Street unrest runs higher than the city's): 8 / 4 / 3 strikes.
- **Riots (D30-D31).** `riot_cooldown_days` 14 → 21: 1 / 2 / 2 riots. A crowd too thin to fight at the door (`dispersed`) is a fizzle: the cooldown and the streak, no count, vent, litter or corp shock (the acceptance reads riots of ≥ 6). Only a Market, Bar or Hotel closes after a looting, and a closed Market takes no restock or haul. `riot_promote_hours` 24 → 9 (the value phase 4 measured). `riot_promote_max` (20): only a riot's 20 most miserable rioters take a body. The per-day throughput dips (5.8k) were the CLI's unbuffered `--events` output on heavy days (1,000-1,500 lines, mostly PlanAborted) on top of a 50 % think/plan rise on gang-growth and raid days; the CLI now buffers events and flushes daily: seed 42 runs 8.3k minimum, 11.9k mean with `--events` (three seeds at once).
- **Emigration (§ 7 of M11).** `dreg_emigrate_mood` −0.5 → −0.3 and `dreg_emigrate_days` 7 → 10: 2 / 3 / 7 Dregs leave.
- **Crossfire (D35).** A hit raises an Assault (a Murder when it kills, after the death) as a pairing does, and nobody on either side is a bystander: the parties' gangs, the riot's own rioters, and every guard when a guard fights.
- **Splits and the cap (D40).** A gang's old boss stays with the gang it ran (unless it is the lieutenant leading the splinter). An emptied gang (a ghost waiting to re-form) does not count toward `max_gangs`. A dead gang whose Hideout district stays at coverage ≥ `reform_max_coverage` never re-forms: documented, kept.
- **The M10 save bound** 40 → 45 MB: the day-120 save on seed 42 is 40.5 MB, mostly the social graph (edges 21 MB, 14 MB at phase 4), which grows as residents live longer (murders 1.4 → 0.8 a day); each single retune moved it 34-38 MB.
- **The M16 hook.** `faction::alertness_mult(world, faction)` (1.0 until M16) scales the riot response (the beat's answering guards), the corp-raid response (posted guards and answering beat) and the stance sweep (`vagrancy_p`).
- **Splits.** A decapitated gang holding one district does not split (the rule needs two held districts); god_decapitate's gang held one. Kept as specified; a weaker one-district splinter is left for later.
- **Accepted as they are.** Legacy traces and holes migrate to the zone's first district (D4, cosmetic). `zone_law_coverage` sums the district watch by `District.zone`: districts are cut inside zones, so the sum equals the old per-tile zone tally (parity passed). Crackdown held on 34 of 120 days in the M9 gate (band 20-60 %). Disperse reports the rioters within 8 tiles once, at the clash (the spec's two hours of arrests): kept.

## Implemented: deviations

Where the build departs from the text above. The numbered rows are the plan's Decisions table (`~/.claude/plans/m12-districts.md`, D1-D48); the rest are the phase commits' deviations, the calibration calls (each changed value carries its reason as a comment in `assets/config.toml`) and the fix pass's rulings (§ 14 has the detail). One line each.

### Decisions that changed the spec

- **D1:** `World::districts` (saved) plus a `#[serde(skip)]` byte per tile, `World::district_grid`: low nibble the district, bit 7 a street tile (walkable, inside no building rect). `districts::rebuild` runs at creation, on load and after every wall change. A zoneless map is one district, `City`.
- **D2:** `district_of`, `is_street` and `district_of_building` are one byte read each; `district(d)` returns an empty default before `rebuild` (review fix).
- **D3:** the binder, the watch, the trace and class fear moved to districts; the Statistical meeting rule (`lod::stranger_in_zone`), founding tiers and the ground tint stay per zone (districts are 3-5x smaller and would cut the calibrated meeting rate).
- **D4:** `DayTrace` packs the district into bits 15-18 of the same `u32` with a has-district bit; a legacy entry migrates to its zone's first district (cosmetic, kept).
- **D5:** `Hole` gains `district`; event texts name the district ("in Sump West") from phase 1.
- **D6:** tick order `..., classes, districts, economy, ...`; the street's nightly pass at 03:00 (`NIGHTLY_TOD` 180); allocation and stances run inside the law brain's daily rescore.
- **D7:** aggregates bin adults by Home door, else by tile; crime is fed at the crime tile; a district without Homes or adults reads crime 0 (fix pass).
- **D8:** control from presence: held Homes 1.0, owned Homes 1.0, other owned buildings 3.0, the city's buildings × coverage; a gang-owned Home weighs `owned_home` (owned and held is 2x; fix pass); Contested below 0.4; a change needs a real takeover to hold (phase 2 review).
- **D9:** the M9 posture brain is unchanged; `Law.target` keeps its M9 meaning; `law::cracking_down_on` reads the posture or any district stance.
- **D10:** allocation weight `(base + crime + paid + gang landlord + riot) × lever`, dealt by largest remainder over inhabited districts. Phase 5 makes the crime term `alloc_crime × min(3, rate ÷ mean) ^ alloc_crime_exp` (see calibration). The fix pass kept the paid term on uninhabited districts (gating it removed the beat on the Civic's Markets and the Vats' Farms and broke the M11 spiral gate).
- **D11:** district beats (a Market and Homes, or four Homes where there is no Market); Garrison empties every district (allocation 0, coverage at its floor): a documented cost.
- **D12:** per-district stances with hysteresis and `max_crackdowns`; a Lobby hold forces its Crackdown; Patrol's Linear{0.6,0.4} floor means a Crackdown or Sweep needs a brave, lawful captain (documented).
- **D13:** reports carry their district (`World::report_places`, 512).
- **D14:** private security fills thin districts through the corp brain's `losses` input.
- **D15:** Vagrancy fines a payer, jails a broke Statistical sleeper for the night, reports a broke Full or Coarse one; never feeds the gang report log (fix pass); a Homeless district rolls at coverage 1 (fix pass).
- **D16:** litter is a byte per tile, saved run-length encoded. The fix pass changed `District.litter` from the mean byte to the share of street tiles at or above `visible` (32), the number every band reads.
- **D17:** deposits ×`deposit_mult` (8, fix pass), capped at 254; an event inside a building lands outside its door (`street_anchor`).
- **D18:** litter delays a Full mover's next step and a Coarse walk; flow fields never change; Sleep reads the street outside the door (fix pass); a mood term, no new field.
- **D19:** the rubble hook exists and is off; `Litter` god levels clamp to 254, so god litter never blocks a tile.
- **D20-D22:** Hotels: a foundable kind, a bed a night paid to the owner, booked by Sleep (Full) or nightly (Statistical); upkeep 10 → 4 (fix pass); occupancy runs 10-20 % of beds against the 30-90 % band (a calibration note, not a fix).
- **D23:** `Role::Sanitation`, hired toward `levers.sanitation_count` (12), dealt daily by litter × street tiles × the district weight; a changed deal logs `Sanitation`.
- **D24:** owners pay the City to sweep their doors; a broke owner skips (fix pass); a proud gang sweeps its held Homes.
- **D25-D26:** derelicts: only Blocks, Bars and Hotels; 16 seeded Sump Blocks (the spec said 8); abandonment and re-letting cover every derelict kind (fix pass); a restored building keeps the capacity it stood at; a gang-held squat is not re-let.
- **D27:** the Squat goal and squatter component; any path to a Home ends a squat and a booking (fix pass).
- **D28:** `rehouse_wait_days` 7 and `rehouse_coins_mult` (21 × rent in hand) for everyone without a Home.
- **D29:** district unrest over Street and Dreg residents with the rent-burden term; strikes read Street unrest only.
- **D30-D34:** riots muster at a Bar, Market, Hotel or Block door, march as a raid, loot only a Market, Bar or Hotel closed afterwards (fix pass); a crowd too thin at the door is a fizzle (fix pass); Contain and Crush cordon; Disperse reports the rioters within 8 tiles once, at the clash (kept); rioters take a body `riot_promote_hours` (9) before the muster, at most `riot_promote_max` (20).
- **D35:** crossfire rolls once per brawl for bodies within the radius and, for a riot, sampled Statistical residents; a hit is an Assault or Murder; neither party is a bystander (fix pass).
- **D36:** splits after a decapitation, two held districts needed (a one-district gang never splits: kept); the old boss stays (fix pass); emptied gangs do not count toward `max_gangs` (fix pass).
- **D37:** musters at the held Home nearest the target within `muster_near_tiles` (180); the first marcher waits for `min(3, marchers)`; a march that has left is committed.
- **D38:** target cover gates Raid, Retaliate and BreakOut; `raid::depart` refuses into cover; `raids_into_cover` counts arrivals under a cover that turned mid-march (phase 4), so the M12 gate checks departures tick by tick instead.
- **D39:** corp raids on the hoard corp's best building in or next to the gang's turf: loot capped (`corp_raid_loot_frac` 0.25 of food, the coin cap), posted private guards, a crew that breaks at half its pairings lost, a raided corp hardens (fix pass).
- **D40:** no instant hydra: claims cleared after `empty_claims_days`, re-forming gated by district control, coverage and `reform_days`; a dead gang whose Hideout district stays at high coverage never re-forms (fix pass ruling, documented).
- **D41:** strikes per district on Street unrest, threshold 0.51, cooldown 30 days (fix pass).
- **D42:** the levers and god commands as listed, CLI spellings `guard_weight`, `stance`, `sanitation`, `sanitation_weight`, `curfew`, `riot_response`, `riot`, `litter`, `split_gang`, `derelict`, `buy_building` (the spec's `day:SetGuardWeight:6:2.0` form does not exist). Phase 5 deviation: the god `SplitGang` skips the lieutenant-strength and loyalty tests as well as the roll (refused only on the structure: no lieutenant, one held district, the gang cap, no Hideout site); with them it was refused for both seed-42 gangs on day 45 and was no lever. `BuyBuilding` charges the buyer (a corp or the City may go negative), restores a derelict to the buyer, and is a god command, not a market.
- **D43:** overlay keys `B` (districts) and `L` (litter); `D` stays camera pan.
- **D44:** the M12 event kinds appended; amber in the log.
- **D45:** eight district slots in the CSV, 56 columns plus eight city columns, before `ticks_per_sec`.
- **D46:** every new field serde-defaulted; a pre-M12 save loads (phase 1 test).
- **D47:** nothing new per agent per tick; the gate's 11-12k ticks/s is unchanged from M11.
- **D48:** the calibration city; `calibrate` re-run in phase 3 (largest move 0.045 on `p_eat`), in the fix pass, and in phase 5 after the allocation and Vagrancy retunes (the calibration city runs the district law; largest move 0.078 on `p_chat_home`).

### Phase deviations and calibration calls

- **Phase 1:** byte-identical 10-day CSV outside the new columns; event texts normalised for place names.
- **Phase 2:** the allocation's highest-crime district held 2x the lowest's guards on 16 of 120 days (the base weight dominated) and Garrison emptied the districts on 52; carried to phase 5.
- **Phase 3:** 16 derelict Blocks, not 8 (the Dreg band needed them); litter missed its band by two orders of magnitude and went to the fix pass; Dreg emigration never fired until the fix pass moved it to mood −0.3 for 10 days.
- **Phase 4:** riots 0.54 × 4 days; `muster_near_tiles` 180; corp raids never lost and loot uncapped (fixed in the fix pass); throughput dips were the CLI's unbuffered `--events` (buffered in the fix pass).
- **Fix pass:** 41 items (§ 14); the M10 save bound 40 → 45 MB, restored to 40 MB in phase 5.
- **Phase 5, allocation:** `alloc_crime` 1.0 → 1.5 and a new `alloc_crime_exp` 2.0 (the crime ratio squared). Linear, the worst district held 2x the best's guards on 40 of 87 days outside Garrison; squared (with the phase 5 stat table), 48 of 56. The whole-run figure stays under the bullet's 60 % (48 of 120 on seed 42) because Garrison (D11) zeroes every allocation on 64 days, so the gate asserts the bullet on days outside Garrison and prints the whole-run figure; likewise the gang-landlord bullet is asserted on windows not wholly under Garrison.
- **Phase 5, Vagrancy:** `vagrancy_base` 0.04 → 0.25. The street rungs (Hotels, squats) leave about two rough sleepers a night, and the fix pass's "rollable only" cut Vagrancy to 0-3 a run. The bullet's "arrests" are read as D15 hits, fines plus jailings (the CSV `vagrancy` column): the jailings alone swung 2-36 on seed 42 between stat tables and loyalty settings, the hits 19-68.
- **Phase 5, splits:** `split_loyalty` 0.6 → 0.66. After the phase 5 recalibration seeds 42-44 decapitated gangs dozens of times and never split: mean member loyalty sat at 0.61-0.71, just over the bar. At 0.66 seeds 43 and 44 split (2 and 1), seed 42 does not; 0.68 split as often but lost seed 42's Sanitation reallocation in days 90-119, a reminder that the single-seed bullets are chaotic.
- **Phase 5, riots:** "each with ≥ 6 rioters" is read as the crowd that gathered (`riot_min` 6 is the start condition); the count at the door is printed (10 of 11 on seed 42; 3 is the fizzle line).
- **Phase 5, raids into cover:** the bullet "no raid departs into cover" is checked per tick at each departure (0 of 15 on seed 42); the arrivals-under-a-turned-cover counter is reported.
- **Phase 5, the save:** the social graph is saved as one packed string (`edge_map.rs`), lossless and loading the old map form; the day-120 seed-42 save is 21.7 MB (40.5 before, edges 21 → 3.3 MB) and the M10 bound is back at 40 MB.
- **Phase 5, curfew:** `District.curfew` follows the lever at the midnight aggregate (and at once on the command); residents' happiness reads 0.05 lower under it.
- **Hotel occupancy** stays under its band (10-20 % of beds against 30-90 %): Hotels are few and the homeless who can pay are fewer; left as a calibration note for M13's corp Hotels.
