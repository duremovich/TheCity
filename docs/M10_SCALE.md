# M10: Scale — 2,000 residents, off-screen lives, deferred attribution

Companion to `SPEC.md`, `M8_FACTIONS.md` and `M9_LAW.md`; `M11_OWNERSHIP.md` and everything after assume this milestone has landed. Where this document and the earlier ones disagree, this one wins for M10.

Decisions taken with Dylan on 2026-10-05:

| Question | Decision |
| --- | --- |
| Population | 2,000 residents on a 256 × 192 map, with the space for them. |
| Why scale first | Every later system (corps, districts, assets, Virt) should be designed for 2,000 from the start and get its off-screen behaviour from the same table. |
| Individuals off screen | Stay complete individuals (needs, wallet, memories, edges), as today. What is coarse is the brain, never the record. |
| Rich stories | Life events happen off screen at calibrated rates and are **decided fast, attributed lazily, narrated on demand**. An off-screen murder is a death roll today; who did it is bound only when something needs to know, and the answer is the same whenever it is asked. |

Decisions taken by the implementing agent (overturnable):

| Question | Decision |
| --- | --- |
| What binds a hole | Anything that consumes it: the inspector, the law filing a report, a gang reading its heat, a promotion to Full. Holes with sim consequences (a death, a gang member as victim) are bound by a daily pass so the brains can react without a human looking; the rest wait up to `hole_ttl_days` and then bind to "unknown". |
| Victim-side sampling | Violence is sampled on the victim (robbed, assaulted, killed) with the actor as the hole. Property crime against buildings and social outcomes are sampled on the actor, as today. Victims are what the law, demography and stories are about; actor rates come out of the binder's weighting, which the parity test checks. |
| Determinism | A hole's binder RNG is seeded from `(world_seed, hole_id)`, so saving, loading or asking in a different order never changes who did it. Once bound, the answer is written back and saved. |
| Districts before districts | The trace ring and the table need a "where": the map generator emits a zone grid (Spire, Civic, Vats, Mid, Sump). The districts milestone replaces zones with its own aggregates; nothing else changes. |
| Gangs at seed | Still two, with `max_members` raised to 60. Gangs per district are a districts-milestone question. |
| Events ring | Grows to 50,000 and gains an id; per-agent biographies are a separate compact `Life` component, not a search of the ring. |

## Goals and acceptance

On seed 42, 2,000 residents, 120 days, on the new map:

- headless throughput ≥ 8,000 ticks/s with the Statistical tier (target 12,000); `bench_tick_2000_agents` median ≤ 250 µs; sim time per frame at 8× < 4 ms; resident memory < 400 MB; save < 40 MB; save/load round trip < 1.5 s;
- at least 20 `Death` events with cause Violence whose victim was Statistical at the time, and every one of them bound (by the daily pass) to an actor or to "unknown" with a bound witness decision; at least one such death leads to an `Arrest` of the bound actor (a cold case);
- at least 100 `Assaulted` or `Robbed` holes created, at most 40 % still unbound at day 120 (the rest consumed or expired);
- the Full-vs-Statistical parity test, extended to assaults, violent deaths and courtships per 100 agent-days, passes at 500 agents within ±15 % (absolute floor 0.5);
- the v1 acceptance, M8 factions and M9 law gates pass on the new map with population bounds scaled by 2000/300;
- the inspector shows a biography for any agent, on or off screen, with no visible difference in richness between an agent that was Full all run and one that was Statistical all run except that the latter's holes read "unknown" until bound.

## 1. Throughput

### Tier index lists

`World::by_tier: [Vec<EntityId>; 3]` (indexed by `Lod`), rebuilt by `lod::assign` and kept in step by `set_lod`, spawn and despawn. `World::bodies()` returns Full ∪ Coarse. Every per-tick system that today scans `world.citizens()` scans bodies instead: `needs::run`, `think::run`, `exec::run`, `social::colocation`, `law::sightings`. The guard list in `needs::run` is cached per hour. `citizens()` keeps allocating a fresh `Vec` and is used only by daily and hourly passes; a clippy-style grep in CI (`tests/no_per_tick_scan.rs`) fails if a per-tick system calls it.

Measured on 2026-10-05, v1 map, 10 days, seed 42: 300 agents 31k ticks/s; 1,500 agents 6k ticks/s. The cost is linear in population because of these scans, not because of the 150 bodies.

### Spread hourly tick

`run_statistical` runs every tick for the agents with `id.index % 60 == tick % 60`, applying 60 ticks of decay as now. The parity test is unaffected (each agent is still hourly); the spike at `tick % 60 == 0` disappears. Children and jailed agents are handled the same way.

### Flow fields

`FlowField` stores a direction byte per tile (`next: Vec<u8>`, 0 = none, 1–8 = neighbour) and no cost array after the build. Fields are built lazily on first request and kept in an LRU of `[exec] flow_field_cache` (default 512; 49 KB each at 256 × 192). Any wall change (`BuildHome`, a Lot being built) clears the cache. A Coarse `GotoTimed` keeps its Manhattan × 2 estimate.

### Everything else

`agents_by_tile` is Full-only already. `edges` at 2,000 agents is ~30k entries; the weekly decay pass stays O(edges). Gang target picking is O(Homes) = 400 per think, which is within budget; the districts milestone narrows it. The events ring grows to `EVENT_RING_CAP = 50_000` with a monotonically increasing `Event.id: u64`.

## 2. Map v2 and the 2,000-resident world

`assets/gen_map.py` is rewritten. `Map` carries `w` and `h` from the file's first line; `MAP_W`/`MAP_H` constants are removed and every consumer (flow fields, view rect default, `edge_roads`, the screenshot helper, the Hideout positions in `M8`'s tests) reads them from `Map`. Zones are a second grid after the tile grid: one character per tile from `S` Spire, `C` Civic, `V` Vats, `M` Mid, `U` Sump; `Map.zone(tile) -> Zone`.

| Item | Value |
| --- | --- |
| Size | 256 × 192; road grid every 8 tiles; a 3-tile Water strip along the south edge |
| Population | 2,000 (`[world] population`), age and coins as v1 |
| Blocks | 400 at 5 residents: 60 Spire (`tier 2`), 140 Mid (`tier 1`), 200 Sump (`tier 0`) |
| Vat Farms | 12, in the Vats zone, east |
| Street Markets | 3: one Civic, two Mid |
| Bars | 3: one Civic, two Mid |
| Precinct | 1, Civic, capacity 80 |
| Civic Hall, Recycler, Reserve Depot | 1 each; Reserve Depot capacity 20,000 |
| Security Offices | 2, Vats zone, inert until M11 (loaded, drawn, no actions) |
| Hideouts | 2, SE and SW corners of the Sump |
| Lots | 60 `Lot` rects 8×6, at least eight per zone, door on a road, no walls; a `Building` of kind `Lot`, capacity 0, inert until M11 |
| Jobs | 160 Farmers, 36 Guards, 12 Clerks, 8 Bartenders, 4 Gravediggers; the rest draw the dole |
| Stocks | Market 1,500 each, Reserve 10,000, Treasury 35,000, `price_ref_stock` per Market unchanged |

`Building.tier: u8` is read from a seventh field on the `B` line, default 1. In M10 it only scales the Sleep safety bonus (`+0.2 / +0.3 / +0.4`); rent is M11. `BuildingKind` gains `Lot` and `SecurityOffice`; `Role::workplace` is unchanged. Multiple Markets: `LocationKey::Market` resolves to the nearest Street Market by door distance; `economy::daily_price` runs per Market from its own stock; clerks restock their own Market from the Reserve; a Farm hauls to the nearest Market. The multi-building lookups that today assume singletons (`building_of_kind(Market)`, `(Bar)`, `(Farm)`) are audited and replaced with nearest-of-kind or all-of-kind calls.

The old map is kept as `assets/map_v1.txt`; nothing but the unit tests that hand-build small worlds may load it.

## 3. The Statistical tier produces lives

### StatTable v2

Rows are conditioned on `(phase, lawfulness bucket, hunger bucket)`: 4 phases × 3 lawfulness buckets (`< 0.3`, `0.3..0.7`, `≥ 0.7`) × 2 hunger buckets (`< 0.4`, `≥ 0.4`) = 24 rows.

```rust
pub struct StatRow {
    // one of these fires, as today
    pub p_eat: f32, pub p_work: f32, pub p_social: f32, pub p_sleep: f32,
    // independent per-hour rolls, actor side
    pub p_steal: f32,      // Market theft, in addition to the eat path's desperation theft
    pub p_flirt: f32,      // a Flirt with an existing edge of affinity ≥ 0.3; accepted at the v1 rate
    // independent per-hour rolls, victim side (actor is a hole)
    pub p_robbed: f32,
    pub p_assaulted: f32,
    pub p_killed: f32,
}
```

`calibrate` v2 builds a Full-only world of 500 agents on map v2 with straight-line walks, runs 30 days, and tallies per agent-hour the dominant exec category (as now) plus the events the agent was actor or victim of in that hour, bucketed by the agent's lawfulness and hunger at the start of the hour. The header records seed, days, agents and the bucket edges. `World::new` panics on a table with the wrong row count.

### Outcomes

Per Statistical agent-hour, after the existing single draw:

| Roll | Effect |
| --- | --- |
| `p_steal` | As the eat path's theft: `Theft` event, nearest Market stock −1, inventory +1, `p_caught = 0.25` files a report with no witness |
| `p_flirt` | Pick the highest-affinity edge ≥ 0.3 with a living, unmarried, adult counterpart; accepted with the v1 Flirt probability: intimacy +0.1 and affinity +0.1 both sides; a Court success follows the v1 courtship rule (two accepted visits) so Statistical agents marry at the Full rate |
| `p_robbed` | Wallet −`extort_amount` clamped, memory `WasRobbed` (0.6, −0.6), safety −0.4, hole `Robbed { victim, zone, tick }` |
| `p_assaulted` | memory `Assaulted` (0.7, −0.7), safety −0.6, energy −0.2, hole `Assaulted { victim, zone, tick }`; a Fight memory `Lost` |
| `p_killed` | `demography::kill(victim, DeathCause::Violence, by: None)` so the corpse, inheritance, widowhood and grief all run as today; hole `Killed { victim, zone, tick }` |

Victims are sampled, not actors: the actor is the hole. The eat-path desperation theft, work, social and sleep outcomes are unchanged. Gang members are still never Statistical, so extortion, raids and breakouts remain Full/Coarse only; a Statistical victim's `Robbed` hole can bind to a gang member, which is how gangs rob off-screen.

### Holes

```rust
pub type HoleId = u64;    // hash(tick, victim.index, kind)
pub struct Hole {
    pub id: HoleId,
    pub kind: HoleKind,         // Robbed, Assaulted, Killed
    pub victim: EntityId,
    pub zone: Zone,
    pub tick: Tick,
    pub event_id: u64,          // the ring entry to rewrite on binding
    pub consequential: bool,    // Killed, or the victim is a GangMember or Guard
}
pub enum Bound { Actor(EntityId), Unknown }
```

`World::holes: BTreeMap<HoleId, Hole>` (saved) and `World::holes_by_agent: BTreeMap<EntityId, SmallVec<[HoleId; 4]>>` (rebuilt on load). Per-agent cap 8 open holes; the oldest expires to Unknown.

### The binder (`systems/bind.rs`)

`bind(world, hole_id) -> Bound`, deterministic:

1. RNG = `world.rng.hole(hole_id)` (ChaCha8 from `(world_seed, hole_id)`).
2. Candidates = agents who, per their trace ring (§ 4) on `day(hole.tick)`, were alive, not jailed, adults, and in the same zone as the victim (or any zone with weight 0.25), excluding the victim and their Spouse. Weight per candidate = `(1 − lawfulness)² × (1 + 2·is_gang_member × claims_victim_home) × (1 + enemy_of_victim × 3) × (1 + 0.5·was_Statistical_that_day)`. With no candidate of weight > 0, `Unknown`.
3. Draw one. Write the actor into the hole's event text and `actors`, give the actor a memory (`Robbed`/`Assaulted`/`Murdered` as the v1 actor memories, with the victim as subject), the victim's memory gets its `subject` filled, and `social::robbed_by`-style edges are created between them.
4. Witness roll: `p_witness = 0.25` (the Statistical theft constant) scaled by `zone_law_coverage` (guards' patrol hours in the zone last day ÷ zone Homes, clamped 0.5..2). A witness is a random co-zoned agent; `law::file_report(crime, suspect: actor, witness: Some(w))` runs the normal arrest path, dated now (a cold case). No witness: `Unpunished` as v1.
5. Consequences consumed at bind: a gang member actor pushes the gang's heat only if arrested (as today); a gang member victim pushes `Shock::MemberKilled { by_rival }` on their gang with `by_rival` from the actor's membership; a Guard victim pushes `LawShock` as a Full-tier assault on a guard would.
6. The hole is removed; `Bound` events: `Attributed` (law white, "the Sump killing of X on day 34 is laid to Y").

Consumers:

| Consumer | When |
| --- | --- |
| Daily pass (`bind::run`, `tick_of_day == 0`) | Every `consequential` hole from yesterday; then any hole older than `hole_ttl_days` (30) binds or expires to Unknown |
| Inspector | Opening an agent binds their open holes as victim (the UI calls `bind` through a `PlayerCommand::Bind(hole_id)` so replays stay deterministic) |
| Promotion to Full | A promoted agent's open holes bind, so a Full agent's memories are never half-filled |
| The law | `expire_warrants`/`sightings` never see a hole; only a bound hole files a report |

The actor's own trace and `Life` get the bound event appended at the binding tick with the original tick, so an actor's biography can grow a crime they committed 20 days ago. That is the intended reading: nobody knew until someone looked.

## 4. Trace and Life

### Trace

`Trace` component on every adult: `VecDeque<DayTrace>` cap `[lod] trace_days` (120).

```rust
pub struct DayTrace {
    pub zone: Zone,        // where the agent ended the day
    pub flags: u8,         // alive, jailed, homeless, employed, gang, statistical_all_day, slept_at_home, ate
    pub hunger: u8,        // 0..=3 band at day end
    pub mood: u8,          // 0..=3 band at day end
}
```

Written by the daily stats pass. 4 bytes × 2,000 × 120 ≈ 1 MB. The binder reads it; the biography renders it as "a hungry week in the Sump".

### Life

`Life` component: `Vec<LifeEvent>` cap 48, with `Birth`, `Marriage`, `Death` and bound crimes never evicted and the rest evicted by `salience × recency` as memories are.

```rust
pub struct LifeEvent { pub tick: Tick, pub kind: LifeKind, pub other: Option<EntityId>, pub hole: Option<HoleId>, pub salience: f32 }
pub enum LifeKind { Born, Married, Widowed, Hired, Fired, Quit, Paid, Starving, Robbed, Assaulted, Stole, Robbed_, AssaultedSomeone, Killed, KilledSomeone, Arrested, Released, Escaped, JoinedGang, LeftGang, Betrayed, Evicted, Housed, Immigrated, Buried, Witnessed }
```

Every `push_event` with the agent in `actors` appends a `LifeEvent` through one table in `events.rs`; nothing else writes `Life`. A `LifeEvent` with `hole: Some(h)` renders as "by an unknown assailant" until the hole is gone.

### Biography

Inspector gains a **Story** tab: the `Life` list newest first, each line rendered from a template per `LifeKind` with names linked, interleaved with trace summaries where a run of 5+ days shares a band ("ten quiet days at home", "a hungry week in the Sump", "nine days inside"). An "unknown" line has a `find out` button that issues `PlayerCommand::Bind`. The tab works identically for Full and Statistical agents; the only difference is unbound holes.

## 5. UI, CLI and events

New `EventKind`s: `Robbed`, `Assaulted` (victim-side, red), `Attributed` (law white). `Theft`, `Death` and `Murder` are reused.

- **City panel**: tier counts (Full / Coarse / Statistical), open holes, ticks/s.
- **Event log**: a filter "unattributed" lists open holes; clicking binds.
- **CLI**: `run --report` adds columns `holes_open`, `holes_bound`, `holes_unknown`, `deaths_violence_offscreen`; `calibrate --agents 500 --map assets/map.txt` writes the v2 table; `bench` adds `bench_tick_2000_agents`.
- **Screenshot helper** takes `--map`.

## 6. Save compatibility

All new components and world fields are `#[serde(default)]`; a pre-M10 save loads with empty traces, lives and holes, and a `Map` without zones gets every tile `Mid`. A save written on map v1 keeps its dimensions. The old four-row `stat_table.toml` is rejected with the calibrate message, as a missing table is today.

## 7. Testing and calibration

**Unit tests** (`citysim/tests/scale.rs`, `citysim/tests/bind.rs`): `bodies()` matches the tier lists after assignment, spawn and death; each Statistical agent runs exactly once per 60 ticks and never on the same tick as more than a sixtieth of the tier; a flow field is built on first use, evicted past the cap and cleared on a wall change; `Map` loads 256 × 192 with zones and the v1 file without; nearest Market resolves per agent; a `p_killed` roll kills with cause Violence and opens a consequential hole; the daily pass binds it; binding is identical across save/load and across call order; a victim with an enemy in the zone binds to the enemy with the documented weight; no candidate binds to Unknown; a bound gang member victim shocks their gang; a bound actor with a witness is arrested; the inspector's `Bind` command is logged and replays; `Life` keeps Birth and Death past the cap; the biography renders an unknown line and then a named one.

**Parity** (`citysim/tests/lod.rs`, extended): 500 agents, 30 days, Full vs Statistical, within ±15 % on thefts, hunger-days, arrests, assaults, violent deaths and marriages per 100 agent-days (floor 0.5). Actor-side rates by lawfulness bucket (who gets bound) are included, which is what tunes the binder weights.

**Scenario** (`citysim/tests/scenario.rs`, new `#[ignore]` 120-day test on seed 42 at 2,000): the bullets under *Goals and acceptance*. The v1, M8 and M9 gates move to map v2 with bounds scaled by 2000/300; the M8 gate's "border near the midpoint" reads the Hideout positions from the map.

**Calibration** targets, tuned via `[lod]`, `[crime]` and the binder weights only: off-screen violent deaths per 100 agent-days within 15 % of on-screen; holes bound within a day for every consequential one; Unknown share 20–50 %. `calibrate` is re-run (new table shape, new map, new actions).

**Throughput** gate: `bench_tick_2000_agents` median ≤ 250 µs on the v2 map with the default tiers; CI fails above it.

## 8. Phases

1. **Throughput**: § 1 tier lists, spread tick, the per-tick scan audit and its CI test. Measure at 1,500 on the v1 map (≥ 8k expected) before anything else. Commit.
2. **Map v2 and 2,000**: § 2, lazy flow fields, nearest-of-kind lookups, scaled gates, `calibrate` on the new map (old table shape). Commit. Watch it live in the app at 128× before phase 3; nothing of M8 or M9 has been watched live either.
3. **Lives**: § 3 table v2, outcomes, holes, binder, daily pass, parity v2. Commit.
4. **Stories**: § 4 trace, Life, the Story tab, `Bind` command. Commit.
5. **Gate and polish**: § 5 panels and CSV, the M10 scenario, bench, calibration, README, docs, then `/code-review high <base>..HEAD` and a fix commit, then push.

## 9. Out of scope (M11 and later)

Ownership, rent and corps (M11), districts replacing zones (M12), assets (M13), Virt and Data (M14); group-level aggregates beyond what the binder needs; gossip propagating bound crimes; the Statistical tier running gang orders; more than two gangs at seed; a hierarchical pathfinder (lazy fields are enough at 256 × 192; revisit if the map grows again).

## Implemented: deviations

Where the build departs from the text above. The numbered rows are the plan's Decisions table (`~/.claude/plans/m10-scale.md`, D1-D40); the rest are the phase commits' deviations. One line each.

### Decisions that changed the spec

- **D1:** `by_tier` and the guard index are kept exact by `Component` insert/remove hooks on `Brain` and `Job`, `despawn`, `set_lod` and `World::retier`; sorted, `#[serde(skip)]`, rebuilt on load, checked hourly in debug builds.
- **D2:** the hourly guard cache of § 1 is replaced by an exact incremental role index, `World::by_role`.
- **D3:** phase 1 step A was verified byte-identical against a baseline CSV before the spread tick changed results on purpose.
- **D4:** children and the jailed need no code for the spread: children have no Brain, the jailed are always Coarse.
- **D5:** each Statistical agent runs on a fixed slot `id.index % 60`, 24 times a day, with 60 ticks of decay per run; wealth is recomputed per processed agent.
- **D6:** the spread test asserts at most `2 x ceil(n / 60)` agents per tick, not a sixtieth.
- **D7:** `SimRng::hole(id)` seeds a ChaCha8 stream from the world seed and the hole id; it is never stored.
- **D8:** `HoleId` is a packed collision-free key `(tick << 24) | (victim << 2) | kind`, not a hash, so the map iterates oldest first.
- **D9:** `holes_by_agent` is a `BTreeMap` of small vectors, rebuilt on load, capped at 8 open holes per victim; the 9th expires the oldest to Unknown.
- **D10:** `Hole` also carries `spouse`, `loot` and, as built, `home` and `gang` (a Killed victim's are gone by bind time).
- **D11:** the Unknown share comes from a first `[bind] p_unknown` draw (0.30), because the weighted candidate list is almost never empty.
- **D12:** the event ring is 50,000 with contiguous ids, so `event_mut(id)` is O(1); `push_event` returns the id.
- **D13:** victim-side events carry `[EntityId::NONE, victim]` until bound; the app shows `NONE` as "someone".
- **D14:** map v2 is a `<w> <h>` header, tile rows, a blank line, zone rows, a blank line, then `B` lines with an optional tier; a headerless file is v1.
- **D15:** `Map` stores `w`, `h` and `zones`; a save keeps its own map, so a v1 save keeps its dimensions.
- **D16:** `Rect` arithmetic is widened to `u16`, so a rect may reach column 255; the app's whole-map frame still clamps its world view rect to 255 (the on-screen margin covers the last column).
- **D17:** blocks are 7 x 7 cells between roads, so Lots are 7 x 6 (spec 8 x 6); Markets and Bars are 15 x 6, Farms 15 x 7, Hideouts 15 x 5 and the Precinct 15 x 15.
- **D18:** a Lot has no walls (one door, ground inside), capacity 0, and is inert; a Security Office is a normal walled building, capacity 12, inert.
- **D19:** capacities and levers per the plan (jail 80, farm 16, hideout 30, `guard_count` 36, `immigration_per_week` 13, clamps 60 and 30); final restock is 1,200 floor and 1,200 batch, not 900 and 450.
- **D20:** guards and gravediggers rank above gang members for the Coarse tier and `max_coarse` is 150; after the phase 2 review guards and gravediggers rank 3 against gang members 2, pinned on top.
- **D21:** `World::local(agent, kind)` resolves the nearest Market or Bar; `mean_price()` replaces `market()`, which is deleted.
- **D22:** `ReleaseReserve` splits evenly with the remainder to the lowest id; a Market without room drops its share and the leftover goes to Markets with room in a second pass.
- **D23:** `Config::v1_profile()` puts unit tests back on the v1 map and scale; scenario gates use the v2 default.
- **D24:** a population below capacity spreads over Homes by even stride, so a 500-agent world spans every zone.
- **D25:** `Config::scaled_to(n)` scales jobs, stocks, levers, gang size and restock keys for calibrate and the parity test.
- **D26:** calibrate v2 and the parity test are gangless; `[lod] stat_violence_mult` is the off-screen violence knob, pinned to 1.0 in parity.
- **D27:** off-screen versus body violent-death rates are reported by the scenario, not asserted, because bodies are selected for violence.
- **D28:** `World.stat_table` is not saved; an old four-row file loads as legacy and `run_statistical` panics with the calibrate message.
- **D29:** `p_steal` is counted only for rows the eat-path theft does not model; the eat-path theft stays for a starving agent who cannot pay.
- **D30:** an assault victim gets `Fought` and `Lost` memories (no `Assaulted` kind); the Statistical memory whitelist grows by `Fought, Lost, Won, Courted, Rejected`.
- **D31:** bind-time consequences reuse the death-time shocks; only an other-gang actor adds `MemberKilled { by_rival: true }`, and `GuardBeaten` is added for assaulted guards.
- **D32:** the binder runs after economy and before the law; promotion binds are queued and drained at the end of `lod::run`.
- **D33:** `zone_law_coverage` is `(hours_z / homes_z) / (hours_all / homes_all)` clamped to 0.5 to 2.0, from per-tick guard-on-shift tallies rolled daily.
- **D34:** `Trace` and the hole CSV columns moved to phase 3 because the binder reads them; phase 4 kept Life, the Story tab and `Bind`.
- **D35:** `LifeKind::RobbedSomeone` (not `Robbed_`) and a new `Died` for non-violent deaths.
- **D36:** the life table is one function in `events.rs` that recomputes a hole from its packed key; the binder's entries go through `life_bound`.
- **D37:** the parity actor side compares per-bucket shares of attributed crimes, within `max(15 %, 0.05)`, because Unknown binding lowers attributed rates by construction.
- **D38:** gate bounds scale by 2000/300; count minimums are unchanged; capacity-bound gates use the capacity.
- **D39:** there is no CI config; "CI fails" is `tests/no_per_tick_scan.rs` and the `#[ignore]` release-only median test in `tests/scale.rs`; the criterion bench is informational.
- **D40:** every new field is `#[serde(default)]` or `#[serde(skip)]`; `migrate_legacy` resizes the new stores.

### Phase commit deviations

- **Phase 1:** `bodies()` (Full plus Coarse) replaces `citizens()` in every per-tick system; remaining scans carry a `scan-ok:` marker that a source-scan test enforces. Statistical ids are bucketed by slot (`World::stat_slots`) so finding the due agents is O(due).
- **Phase 2:** initial jobs go to the nearest resident; the patrol beat is a Market, the nearest Bar, the Hall and two nearby Homes; crackdown routes start at the Market nearest the target's Hideout; an arrest or escort on shift credits the shift; `recruit_on_promise` 15 and `raid_gather_hours` 8 (from 5 and 3).
- **Phase 3:** StatTable v2 has 24 rows keyed on phase, lawfulness bucket and hunger bucket, pooled over the hunger buckets; `calibrate --straight-lines` is opt-in; Statistical agents draw the dole by the Full Earn rule, buy up to three meals into the pantry, flirt and court at the Full rate, and drift in co-work and chat.
- **Phase 4:** Life is capped at 48 entries and written only by `events.rs`; the Story tab is an inspector tab, and opening an agent binds their open holes through the `Bind` command; the app gains `--select-name`, `--tab story` and `--no-autobind`.
- **Phase 5:** a guard's arrest or escort now credits the shift's wage once without ending the shift (`Job.shift_credited`), so the guard keeps patrolling; the Coarse ranking, Reserve split, `ReleaseReserve` redistribution, `stat_social` window scan and the whole-map screenshot (`--fit`) were review fixups.

### Spec text not implemented as written

- **§ 2 "clerks restock their own Market from the Reserve":** the daily pass moves food from the Warehouse to each Market with no clerk involved, exactly as v1 did. When the Reserve runs short it is split across Markets in proportion to their shortfall.
- **§ 1 `stat_social`:** an agent with no contacts meets a housemate from a fixed window of 32 Statistical ids around its own, not a scan of the tier.
- **Acceptance, throughput and save size:** when this was written the M10 scenario missed the 8,000 ticks/s gate and the 40 MB save bound; both are calibration work (plan 5.5) and the scenario prints the numbers.
