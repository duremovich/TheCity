# M8: Factions — rival gangs, a faction brain, turf and raids

Companion to `SPEC.md`. Everything here builds on the v1 gang (`SPEC.md › Gang`), the social graph, the law system and the utility/GOAP machinery as implemented through M7 plus the 2026-10-04 calibration commit. Where this document and `SPEC.md` disagree, this one wins for M8.

Decisions taken with Dylan on 2026-10-04 (brainstorm):

| Question | Decision |
| --- | --- |
| M8 scope | Rivals first: second gang, faction brain with orders, turf conflict. Jailbreaks and the law as a faction are M9. |
| Second gang origin | Seeded at world init with its own Hideout. Splits are later polish. |
| How orders act | A loyalty-scaled bonus in the member's normal utility scoring. Hungry or scared members still eat or flee; the disloyal freelance. |
| What conflict is | Both: contested Homes (the everyday grind) and Hideout raids (the spectacle). |
| Where the brain lives | A daily rescoring in the gang system, plus immediate rethinks on shocks. Not an embodied leader action. |
| Emergencies | Shock-driven rethinks without hysteresis, a Retaliate order, sacked Hideouts. Full Hideout seizure (a homeless gang) is M9. |

## Goals and acceptance

The city should read as two factions fighting over the same Homes. Concretely, on seed 42 over 120 days:

- both gangs recruit (peak headcount ≥ 5 each) and a territory border forms (each gang's territory is nearer its own Hideout than the rival's, on average);
- at least one Home flips between gangs; at least one raid resolves; at least one Assault has members of both gangs as actor and victim;
- at least one emergency rethink changes an order; at least one Retaliate order is issued;
- Assault events per day stay ≤ 2× the M7 baseline (≈ 3/day), starvation deaths and population stay within the existing v1 acceptance bounds, and the v1 acceptance and Full-vs-Statistical parity tests still pass;
- throughput stays ≥ 8k ticks/s with the Statistical tier.

## 1. Data model

### Gang (extends `components::Gang`)

| Field | Type | Meaning |
| --- | --- | --- |
| `hideout` | `EntityId` | This gang's Hideout building. Replaces every `building_of_kind(Hideout)` lookup in gang code. |
| `order` | `Order` | Standing order. Default `Expand`. |
| `order_since` | `Tick` | When the order was last changed. |
| `order_trace` | `Vec<(Order, f32)>` | Scores from the last rescoring, for the inspector. Not saved (`serde(skip)`). |
| `raid_at` | `Option<Tick>` | Scheduled muster departure while `order ∈ {Raid, Retaliate}`. |
| `last_raid_tick` | `Option<Tick>` | Last raid this gang launched (cooldown). |
| `retaliate_until` | `Option<Tick>` | Retaliate expires here. |
| `sacked_until` | `Option<Tick>` | The Hideout is unusable until this tick. |
| `shocks` | `Vec<Shock>` | Pending shocks since the last rescoring; drained by the brain. Not saved: a save taken mid-shock loses at most one rethink. |
| `heat_log` | `VecDeque<(Tick, EntityId)>` | Members arrested or killed in the last 7 days (capped at 64). |

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Order { Expand, Contest, Raid, Retaliate, LieLow }

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Shock {
    MemberKilled { by_rival: bool },   // severity 1.0 / 0.6
    MemberArrested,                    // 0.3
    RaidLost,                          // 0.8
    HomeFlippedAgainst,                // 0.4
    LeaderChanged,                     // 0.5
    Sacked,                            // 1.0
}
```

`Gang` keeps `members`, `treasury`, `territory`, `leader`, `empty_since`, `name` as today.

### Building

`extort_count: u8` becomes `claim: Option<Claim>` with `Claim { gang: EntityId, count: u8 }`. A Home in a gang's `territory` has `claim = Some(Claim { gang: holder, count: 3 })`. A rival extortion of a held Home resets the claim to the rival at count 1 (the first blow lands hard; the holder's three visits are forgotten), then counts up; at 3 the Home flips. An unclaimed Home's claim counts up for whoever extorts it; a different gang extorting an unclaimed-but-claimed Home takes the claim at count 1 the same way. `extort_count` is kept in saves as `#[serde(default)]` and migrated into `claim` on load for the first gang.

### GangMember

Unchanged (`gang`, `rank`, `joined_tick`). `gang` already names the gang entity.

### Brain

| Field | Type | Meaning |
| --- | --- | --- |
| `following_order` | `Option<Order>` | Set by the GangWork/Raid target picker when the member acts on the order; `None` while freelancing. Inspector only. |
| `raid_target` | `Option<EntityId>` | The rival Hideout for the current Raid plan. |

### World

- `gang_id()` is removed. New: `gangs() -> Vec<EntityId>` (sorted, stable), `gang_of(agent) -> Option<EntityId>` (via `GangMember`), `rival_of(gang) -> Option<EntityId>` (the other gang; with more than two gangs, the one with the most territory). Every one of the ~15 current call sites moves to these.
- `spawn_gang()` becomes `spawn_gangs()`: one gang per Hideout in map file order, named from config.

### Config (`assets/config.toml`)

```toml
[gangs]
names = ["The Hollow", "Ninefold"]
treasury_initial = [50, 50]
order_weight = 0.35          # GangWork bonus = order_weight × loyalty
freelance_loyalty = 0.3      # below this a member ignores the order
heat_days = 7
hysteresis = 0.10            # a new order must beat the current one by this (daily rescoring only)
contest_min_ratio = 0.8      # own / rival headcount needed to Contest
raid_min_ratio = 1.2         # … to Raid
raid_min_prize = 40          # rival treasury worth raiding
raid_cooldown_days = 10
raid_prize_frac = 0.5        # treasury fraction taken on a won raid; 1.0 on a sack
raid_muster_hour = 22        # departure at this hour of the day the order is set
raid_gather_radius = 6
retaliate_days = 3
sacked_days = 3
shock_severity_rethink = 0.3 # the sum of pending shock severities that forces an immediate rescore
```

`[world] gang_name` and `gang_treasury_initial` are removed (migrated into `[gangs]`). `[social] gang_stipend`, `extort_amount`, `join_gang_*` stay where they are.

## 2. Map and seeding

`assets/gen_map.py` places a second 7×5 Hideout at `(1, 56)` with its door on the top perimeter opening onto the row-55 road, bottom-left, opposite the existing Hideout at `(80, 56)`. `map.txt` is regenerated and committed. Loader rules are unchanged (one door, road outside it).

`World::new` spawns one gang per Hideout, members empty, `leader: None`, `empty_since: Some(0)`, `order: Expand`. Recruitment is unchanged except that eligibility and `join` take the gang: a recruit joins the gang of the member they have the qualifying edge to (the highest-affinity qualifying edge decides when they know members of both); a desperate or bootstrap recruit joins the gang whose Hideout is nearest their Home (ties: lower building index). `JoinGang` plans to that gang's Hideout. The planner's `LocationKey::Hideout` resolves to the acting agent's own gang's Hideout, or the join target for a recruit.

Expansion targets are chosen relative to the gang's own Hideout, not the actor (see § 4), so the two gangs grow from their corners and meet along a border instead of interleaving.

## 3. The faction brain

Runs in `systems::gang::run`:

- **Daily**, at `tick_of_day == 0`, for every gang: `rescore(world, gang, hysteresis = cfg.hysteresis)`.
- **Immediately**, any tick, when `Σ severity(pending shocks) ≥ shock_severity_rethink`: `rescore(world, gang, hysteresis = 0.0)`, then clear the shocks. Shocks are pushed by `kill`, `sentence`, `extort` (flip), `recompute_leader` and the raid resolver.

A gang with no leader, or whose leader holds a `Sentence`, does not rescore: the standing order stays, `raid_at` is cleared, and nothing new is issued. (M9's jailbreak is what fixes that.)

`rescore` evaluates every `Order` with the utility module's `Consideration`/`Curve` machinery (`utility::curves`), takes the best, and switches when `best.score > current.score + hysteresis`. The trace is kept in `order_trace`. An order change logs `OrderChanged` and, for Raid/Retaliate, sets `raid_at` to the next `raid_muster_hour` at least 2 hours away.

Inputs (computed once per rescoring):

| Symbol | Definition |
| --- | --- |
| `unclaimed` | Homes with `claim == None` or a claim count < 3 by any gang, not demolished, occupied |
| `own`, `rival` | headcounts, excluding members with a `Sentence` |
| `ratio` | `own / max(rival, 1)` |
| `heat` | `heat_log` entries within `heat_days` ÷ `max(own, 1)`, clamped to 1 |
| `prize` | rival treasury |
| `grudge` | 1 if a `MemberKilled{by_rival}` or `RaidLost` or `Sacked` shock is pending or `retaliate_until > now`, else 0 |
| `L` | leader's Personality: `greed`, `courage`, `pride` |
| `raid_ready` | `last_raid_tick` older than `raid_cooldown_days` (or `None`), and `sacked_until` passed |

Order table. A score is the product of the consideration outputs, as for agent goals; a `flat` term is added to that product, as `GoalKind::Fight` does today:

| Order | Considerations (input → curve) |
| --- | --- |
| Expand | `Can(unclaimed > 0)` → Step{1,0,1}; `1 − heat` → Linear{0.8,0.2}; `L.greed` → Linear{0.5,0.5}; flat 0.5 base |
| Contest | `Can(rival territory > 0)` → Step{1,0,1}; `ratio` → Logistic{6, contest_min_ratio}; `1 − heat` → Linear{0.7,0.3}; `L.greed` → Linear{0.6,0.4}; `L.pride` → Linear{0.4,0.6} |
| Raid | `Can(raid_ready ∧ rival exists)` → Step{1,0,1}; `ratio` → Logistic{8, raid_min_ratio}; `prize / 200` clamped → Linear{0.7,0.3} gated at `prize ≥ raid_min_prize`; `L.courage` → Linear{0.8,0.2}; `1 − heat` → Linear{0.5,0.5} |
| Retaliate | `Can(grudge ∧ rival exists ∧ sacked_until passed)` → Step{1,0,1}; `L.pride` → Linear{0.9,0.1}; `L.courage` → Linear{0.5,0.5}; flat +0.3 while a `Sacked` or `MemberKilled{by_rival}` shock is pending |
| LieLow | `heat` → Logistic{8,0.4}; `1 / ratio` clamped to 2 → Linear{0.5,0}; flat 0.15 base so a quiet gang can always choose it |

Retaliate ignores `raid_cooldown` and `raid_min_prize`; once chosen, `retaliate_until = now + retaliate_days`, and when that passes with no raid launched the order falls back to the daily rescoring. A Retaliate or Raid that launches sets `last_raid_tick` on departure.

The leader is still `argmax(loyalty + days_in_gang / 100)` daily and on arrest/death as in v1; a change pushes `Shock::LeaderChanged`.

## 4. Orders in members' heads

**GangWork** (existing goal) gains one consideration: `order bonus` = `cfg.order_weight × loyalty` → Linear{1,0}, added as a multiplicative term exactly like the others, so an order never overrides hunger, sleep or fear; it tilts the day. The target picker reads the order:

| Order | GangWork target | `following_order` |
| --- | --- | --- |
| Expand | the unclaimed Home nearest the **gang's Hideout** (not the actor) with no guard within 8, excluding the actor's own Home | `Some(Expand)` |
| Contest | the rival-held Home nearest the gang's Hideout with no guard within 8 | `Some(Contest)` |
| Raid, Retaliate | GangWork unavailable; the member's goal is **Raid** (§ 6) | — |
| LieLow | GangWork unavailable; members live on the stipend | — |

A member with `loyalty < freelance_loyalty` ignores the order: GangWork stays available under every order, the target is the nearest unclaimed Home to the actor (today's rule), `following_order = None`, and a `Disobeyed` event is logged once per day per member when the order was Raid, Retaliate or LieLow (the visible cases).

Extort, SplitLoot, Fence, stipend and tribute are unchanged except that they act on `gang_of(actor)`; SplitLoot and Fence fail (`PreconditionLost`) while the Hideout is sacked.

**LOD.** Gang members are never assigned the Statistical tier (`assign` treats `GangMember` like `pinned`). Two gangs of ≤ 20 are within the Coarse budget; the parity test is unaffected because the Statistical table never modelled gang actions.

## 5. Contested Homes

`extort` on a Home:

1. Coins and `WasRobbed` memories as today.
2. Claim update per § 1 (`Building`): own claim counts up; a rival's claim is replaced at count 1.
3. If the Home was in the rival's `territory`: every rival member gets an Enemy edge to the actor (`affinity −0.7, trust 0, kind Enemy` via `social::robbed_by`-style helper), and the rival gang gets `Shock::HomeFlippedAgainst` only when the claim reaches 3 (step 4), not on every blow.
4. At count 3: the Home leaves the rival's `territory`, joins the actor's, `TerritoryFlipped` is logged with both gang names, and the rival receives the shock.

Nothing else is new: the existing revenge-only Fight goal (`Can(enemy within 4 tiles)`) turns border contact and Bar evenings into fights, `resolve_fight` handles them, and the law responds to the Assault as today. Enemy edges between gangs decay like any other edge (`×0.98` per idle week), so a border that stops moving cools down.

## 6. Hideout raids

**Scheduling.** When the brain sets Raid or Retaliate, `raid_at` is the next `raid_muster_hour` at least 2 hours away. `raid_target` on each member's Brain is the rival Hideout.

**Raid goal** (new `GoalKind::Raid`, GangMember only):

| Considerations | GOAP goal state |
| --- | --- |
| `Can(order ∈ {Raid, Retaliate} ∧ raid_at set ∧ loyalty ≥ freelance_loyalty)` → Step{1,0,1}; `courage` → Linear{0.7,0.3}; `loyalty` → Linear{0.6,0.4}; `U(safety)` → Linear{0.5,0.5}; `Can(not in shift)` → Step{1,0.5,1} | `{raid_done: true}` |

Plan: `GoTo(own Hideout) → Muster → GoTo(rival Hideout) → Brawl`. `Muster` is a `Wait` until `raid_at` (fails with `PreconditionLost` if the order changes). `Brawl` is a new `ActionKind` with cost 15, `dist` via the rival door.

**Brawl** resolves on the first raider's arrival at the rival door:

1. Raiders = members of the attacking gang within `raid_gather_radius` of the door whose current goal is Raid. Defenders = members of the defending gang inside the Hideout (its `occupants`).
2. If there are no defenders the Hideout is **sacked** outright (step 5).
3. Sort both sides by `fighting + 0.25 × courage`, descending. Pair strongest with strongest and call `law::resolve_fight(raider, defender)`. The loser is out; the winner stays in the queue and fights the next opponent. Repeat until one side is out of fighters. Each fight raises Assault or Murder for the attacker of that pairing with the usual witnesses, so guards in sight respond.
4. Raiders win → `raid_prize_frac × defender treasury` moves to the raiders' treasury; defenders win → raiders get `Lost` memories (already from `resolve_fight`) and the attacking gang receives `Shock::RaidLost`. Both: `Raid` event with the tally; the attacking gang's `raid_at` clears and the order falls back to the daily rescoring (Retaliate is satisfied by a launched raid, win or lose).
5. **Sacked** (raiders win and every defender present lost, or no defenders): the whole treasury and the Hideout's `stock_food` go to the raiders' gang, `sacked_until = now + sacked_days`, `Shock::Sacked`. While sacked: no JoinGang, SplitLoot, Fence or stipend for the victims; members already inside are moved to the door.
6. Later raiders arriving after resolution complete the plan trivially (`raid_done` is true once `raid_at` is cleared).

Deaths follow `fight_death_p` as in any fight. A raid that loses its leader (killed or arrested on the way) continues; the brain just stops issuing new orders until a leader exists.

## 7. UI and events

New `EventKind`s: `OrderChanged`, `TerritoryFlipped`, `Raid`, `Disobeyed`, `Sacked` (log colours: gang purple).

- **Hideout panel** (`citysim-app/src/ui/building.rs`): gang name, order and since when, the order trace (top three with scores), heat, own vs rival headcount, treasury, sacked countdown, `raid_at` countdown, roster and territory as today. Territory links show the holder's colour.
- **Agent inspector**: for a GangMember, the gang name and "following `Order`" or "freelancing".
- **Map**: territory Homes get a thin outline in the holder gang's colour (two fixed colours from the gang index); a sacked Hideout is drawn dashed for its duration.
- **City panel**: one row per gang (headcount, territory, treasury, order).

## 8. Save compatibility

All new `Gang`, `Building` and `Brain` fields are `#[serde(default)]`. Loading a v1 save migrates `extort_count` into `claim` for the first gang and gives it `hideout` from the first Hideout; a map without a second Hideout simply yields one gang, and every rule above degrades to v1 behaviour (no rival → Contest/Raid/Retaliate never available).

## 9. Testing and calibration

**Unit tests** (`citysim/tests/factions.rs`): build a world, hand-set leaders, headcounts, treasuries, territories and shocks, and assert the chosen order for: unclaimed Homes and low heat → Expand; parity and rival territory → Contest; strength edge and prize → Raid; pending `MemberKilled{by_rival}` → Retaliate regardless of ratio; heat 0.6 → LieLow; shocks over the threshold force a mid-day rescoring; no leader → no change. Claim mechanics: rival extortion resets the claim, third blow flips. Brawl: a 3-vs-1 raid sacks; a 1-vs-3 raid loses and pushes `RaidLost`.

**Scenario** (`citysim/tests/scenario.rs`, new `#[ignore]` 120-day test on seed 42): the acceptance bullets in *Goals and acceptance*.

**Calibration** targets after implementation, tuned via `[gangs]` only: gang peak sizes 5–20 each; a border that stays within a few Homes of the midpoint; raids 2–6 per 120 days; Assault/day ≤ 2× M7 baseline. Re-run `calibrate` (planner costs change with Brawl and the GangWork consideration).

**Throughput**: the brain is O(members + homes) daily and on shocks; target picking stays the existing O(homes) scan; nothing new per tick.

## 10. Out of scope (M9 and later)

Jailbreaks (the orders are built so `BreakOut` slots in as a sixth order), the law as a faction with its own brain and levers, bribery, emergent splits, order propagation by gossip, full Hideout seizure and homeless gangs, more than two gangs (the data model allows it; the brain's `rival_of` picks the strongest), and the jail-capacity residual.
