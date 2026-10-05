# M9: The Law — jailbreaks, the law as a faction, bribery

Companion to `SPEC.md` and `M8_FACTIONS.md`. Everything here builds on the M8 factions (orders, musters, brawls) and the v1 law system (`SPEC.md › Law and jail`). Where this document and the earlier ones disagree, this one wins for M9.

Decisions taken on 2026-10-05 (Dylan was away; these are the implementing agent's calls, listed so they can be overturned cheaply):

| Question | Decision |
| --- | --- |
| Jailbreak | A sixth order, `BreakOut`: muster at the Hideout, march to the Jail door, breach (a brawl against the guards present), free own members. Same machinery as a raid. |
| Who decides | The acting leader. The gang remembers its `boss` (the leader at the moment of arrest) and leans toward getting them back. |
| The law's brain | A `Law` component on the Jail. A captain (the most lawful guard) scores three postures daily and on shocks: `Patrol`, `Crackdown` (against the most-reported gang), `Garrison`. |
| What a posture changes | The Jail/Patrol duty split (Garrison: every guard holds the Jail; Crackdown: one in three) and the patrol loop (Crackdown: through the target gang's territory and its Hideout). |
| Bribery | A gang under crackdown may pay the captain from its treasury. A bribed captain drops the crackdown and cannot resume it against that gang for `bribe_days`. A captain with lawfulness ≥ `incorruptible` refuses, and the crackdown hardens. |
| Player | A `SetLawPosture` command pins the posture, or hands it back to the captain. |
| Escapees | A freed convict's warrant reopens for the original crime; re-arrest means a fresh full sentence. |
| Empty Hideout (M8 residual) | Fixed here: members sleep and idle at the Hideout while lying low, and whenever homeless, so a raid on a gang that is lying low meets defenders. |

## Goals and acceptance

The city should read as three factions now: two gangs and the law. Concretely, on seed 42 over 120 days:

- at least one `BreakOut` order is issued and at least one breach resolves; at least one convict is freed (a `Jailbreak` event);
- the law changes posture at least once, `Crackdown` is held at least once, and `Garrison` follows a jailbreak;
- at least one raid meets defenders (a `Raid` event with `defenders ≥ 1`) and not every raid is a sack;
- Assault events per day stay ≤ 2× the M7 baseline (6.4), starvation deaths and population stay within the v1 bounds, and the v1 acceptance, M8 factions and Full-vs-Statistical parity tests still pass;
- throughput stays ≥ 8k ticks/s with the Statistical tier.

Bribes are reported, not gated: whether one happens in 120 days depends on the captain the seed deals.

## 1. Data model

### Order

```rust
pub enum Order { Expand, Contest, Raid, Retaliate, LieLow, BreakOut }
```

`Order::ALL` has six entries. `is_raid()` is true for `Raid`, `Retaliate` and `BreakOut`: all three muster at the own Hideout at `raid_at` and march. `Order::target_is_jail()` is true for `BreakOut` only.

### Gang (extends M8)

| Field | Type | Meaning |
| --- | --- | --- |
| `boss` | `Option<EntityId>` | The leader at the moment of their arrest; cleared when they are released, freed, killed or leave. The acting leader favours a breakout while the boss sits inside. |
| `last_breakout_tick` | `Option<Tick>` | Departure of the last breakout (its own cooldown; raids keep theirs). |
| `bribe_until` | `Option<Tick>` | The captain has been paid (or has refused): no further bribe, and no `Crackdown` against this gang, until here. |

All `#[serde(default)]`.

### Shock (extends M8)

`Shock::BreakoutFailed` (severity 0.8, not a grudge: the law is not the rival) and `Shock::MemberFreed` (0.3, consumed like the rest; it exists so the inspector's shock list reads right).

### Law (new component, on the Jail building)

```rust
pub struct Law {
    pub posture: Posture,                 // default Patrol
    pub posture_since: Tick,
    /// The gang a Crackdown is against (None under the other postures).
    pub target: Option<EntityId>,
    pub captain: Option<EntityId>,
    /// The player's pin; None = the captain decides.
    pub pinned: Option<Posture>,
    pub last_breakout_tick: Option<Tick>,
    /// A refused bribe hardens the crackdown until here.
    pub hardened_until: Option<Tick>,
    /// (tick, gang) per report filed against a gang member, capped at 128.
    pub report_log: VecDeque<(Tick, EntityId)>,
    #[serde(skip)] pub posture_trace: Vec<PostureScore>,
    #[serde(skip)] pub shocks: Vec<LawShock>,
}

pub enum Posture { Patrol, Crackdown, Garrison }

pub enum LawShock {
    Jailbreak,        // 1.0
    GuardKilled,      // 0.8
    GuardBeaten,      // 0.4  (a guard lost a fight: contested arrest or breach)
    BribeRefused,     // 0.5
}
```

`World::law()` / `law_mut()` find it as `market()` finds the Market. A save from before M9 has no `Law`: `migrate_legacy` inserts `Law::default()` on the Jail.

### Config

```toml
[gangs]
breakout_cooldown_days = 15
breakout_min_members = 2       # fit members needed to attempt one
breakout_max_freed = 3         # convicts freed per successful breach
breakout_boss_flat = 0.3       # added to BreakOut's score while the boss is inside
[gangs.order_flat]
breakout = 0.2

[law]
window_days = 7                # reports against gang members counted for pressure
crackdown_reports = 6          # reports in the window that read as full pressure
garrison_days = 7              # a jailbreak holds the Jail this long
hysteresis = 0.10
shock_severity_rethink = 0.5
min_guards = 2                 # Crackdown and Garrison need this many guards on the payroll
incorruptible = 0.8            # captain lawfulness at or above this refuses bribes
bribe_base = 10                # price = bribe_base + bribe_per_guard x guards
bribe_per_guard = 1
bribe_days = 10
bribe_threshold = 0.35         # the gang pays when its bribe score reaches this
[law.posture_flat]
patrol = 0.2
crackdown = 0.0
garrison = 0.05
```

New `[gangs]` keys and `[law]` have serde defaults, so an M8 save loads.

## 2. Lying low at the Hideout (the M8 residual)

- **Sleep** is allowed at the Hideout for a member whose gang is lying low or who is homeless, while the Hideout is not sacked and not full. While lying low, Sleep at Home is *not* allowed unless the Hideout is full: lying low means holing up. `PlanCtx` gains `hideout_bed` and `hideout_full`.
- **Rest** is allowed at the Hideout for any member.
- **Idle** (`routine::idle_plan`) sends a member who is lying low, or homeless, to the Hideout to Rest instead of Home.
- Nothing else changes. A raid on a gang that is lying low now finds its members inside; a raid on a gang out expanding still finds an empty Hideout and sacks it. That trade-off is the point.

## 3. BreakOut

### Brain

`OrderInputs` gains `jailed` (members with a `Sentence`), `boss_jailed`, `breakout_ready` (`last_breakout_tick` older than `breakout_cooldown_days`, or `None`; not sacked) and `garrison` (the law's posture is `Garrison`).

| Order | Considerations (input → curve) |
| --- | --- |
| BreakOut | `Can(jailed > 0 ∧ breakout_ready ∧ own ≥ breakout_min_members)` → GATE; `jailed / (jailed + own)` → Linear{0.6,0.4}; `L.courage` → Linear{0.6,0.4}; `L.loyalty` → Linear{0.5,0.5}; `1 − garrison` → Linear{0.5,0.5}; flat `order_flat.breakout`, plus `breakout_boss_flat` while `boss_jailed` |

BreakOut competes with LieLow by design: arrests raise heat, heat feeds LieLow, and a brave loyal leader breaks out where a timid one hides. Choosing it sets `raid_at = next_muster(raid_muster_hour)` exactly as Raid does.

`recompute_leader` records `boss = Some(old leader)` when the leader loses the post to a `Sentence`; `boss` is cleared by release, escape, death and leaving.

### Plan

The Raid goal, its gate (`raid_pending`), `Muster` and the march are unchanged. `LocationKey::RivalHideout` becomes `LocationKey::RaidTarget` (`#[serde(alias = "RivalHideout")]`): the street tile outside the current expedition's door, which `raid::target_tile` resolves from the order (`BreakOut` → the Jail, else the rival Hideout). `Brawl` at `RaidTarget` dispatches on the order: `raid::brawl` for a raid, `raid::breach` for a breakout. `plan::bind_target` binds the Jail for a BreakOut so the inspector shows it. `depart` stamps `last_breakout_tick` instead of `last_raid_tick` under BreakOut.

### Breach (`raid::breach`)

1. Raiders as in a brawl. Defenders: living guards inside the Jail or within `raid_gather_radius` of its door.
2. No defenders → the breach succeeds unopposed.
3. Otherwise strongest-against-strongest through `law::resolve_fight`, as in a brawl. Each pairing raises Assault or Murder for the raider with the usual witnesses (the other guards file reports: a breakout is loud). A beaten guard pushes `LawShock::GuardBeaten`; a dead one `GuardKilled`.
4. Raiders win (no defenders left standing) → up to `breakout_max_freed` of the gang's convicts are freed, the boss first, then the longest remaining sentence. `law::escape(who)`: the `Sentence` goes, they stand at the Jail door, memory `Escaped` (0.7, +0.4), `safety = 0.3`, and the warrant **reopens** for the sentence's crime (`file_report(crime, who, None)`), so the guards hunt them and a re-arrest sentences them afresh. `Law.last_breakout_tick = now`, `LawShock::Jailbreak`, `Shock::MemberFreed` per convict, one `Jailbreak` event.
5. Raiders lose → `Shock::BreakoutFailed`.
6. Settle: the gang's `raid_at` clears, `last_breakout_tick` is set, the brain rethinks at once (M8 rule).

## 4. The law as a faction

### Captain

Daily, and whenever a guard is hired, fired or killed: `captain = argmax(lawfulness)` over living guards, ties by lower index. No guards → no captain → the posture stays `Patrol` and nothing is scored.

### Posture brain (`systems::law_brain`, mirroring `faction`)

Runs in `law::run`: daily at `tick_of_day == 0` with `hysteresis`, and at once when pending `LawShock` severities reach `shock_severity_rethink`. While `pinned` is set the trace is still computed but the posture is the pin.

Inputs:

| Symbol | Definition |
| --- | --- |
| `reports[g]` | `report_log` entries for gang `g` within `window_days` |
| `wanted_gang` | the gang with the most reports, excluding gangs whose `bribe_until > now`; `None` if no gang has any |
| `pressure` | `reports[wanted_gang] / crackdown_reports`, clamped to 1 |
| `jailed_gang` | gang members with a `Sentence` ÷ Jail capacity, clamped to 1 |
| `breakout_recent` | `last_breakout_tick` within `garrison_days` |
| `guards` | guards on the payroll |
| `hardened` | `hardened_until > now` |
| `C` | captain's `courage`, `lawfulness` |

| Posture | Considerations |
| --- | --- |
| Patrol | `1 − pressure` → Linear{0.5,0.5}; `1 − jailed_gang` → Linear{0.5,0.5}; flat `posture_flat.patrol` |
| Crackdown | `Can(wanted_gang ∧ guards ≥ min_guards)` → GATE; `pressure` → Logistic{8,0.5}; `C.courage` → Linear{0.5,0.5}; `C.lawfulness` → Linear{0.5,0.5}; flat `posture_flat.crackdown`, +0.3 while `hardened` |
| Garrison | `Can(guards ≥ min_guards)` → GATE; `jailed_gang` → Logistic{8,0.3}; `1 − C.courage` → Linear{0.4,0.6}; flat `posture_flat.garrison`, +0.5 while `breakout_recent` |

A change logs `Posture` with both postures, the target gang and the reason (daily / shock / pinned). `report_log` is fed by `file_report` whenever the suspect is a gang member.

### What a posture does

- **Duty split.** `law::jail_duty(world, guard, shift_key)` replaces `jail_day` everywhere: Patrol → `index % 2 == key % 2` (v1); Crackdown → `index % 3 == key % 3` (one in three holds the Jail); Garrison → always. A posture change mid-shift re-routes guards at their next think; a shift already running finishes.
- **Patrol loop.** Under Crackdown `new_patrol_route` is `[Market, Home(t), Home(t), Hideout(t), Home(t)]`: Homes drawn from the target's territory (or, with fewer than three held, the inhabited Homes nearest its Hideout), and the target's Hideout as a stop. Guards inside the Hideout see wanted members and arrest them; guards near the target's Homes make them unextortable (`gang_work_target` already skips Homes with a guard within 8). Patrol and Garrison keep the v1 loop.

### Player

`PlayerCommand::SetLawPosture(Option<Posture>)` sets `Law.pinned` and logs `PlayerAction`. Pinned Crackdown with no `wanted_gang` behaves as Patrol (there is nobody to crack down on); the Jail panel says so.

## 5. Bribery

Evaluated daily in `gang::run`, after the law has rescored (the system order runs `law` before `gang`), by `faction::consider_bribe(world, gang)`:

- Gate: the law's posture is `Crackdown` with `target == gang`; a captain exists; `bribe_until` has passed; `treasury ≥ price` where `price = bribe_base + bribe_per_guard × guards`.
- Score: `(1 − L.pride)` → Linear{0.6,0.4} × `captain.greed` → Linear{0.8,0.2} × `heat` → Linear{0.5,0.5}. The gang pays when the product reaches `bribe_threshold`.
- Captain `lawfulness ≥ incorruptible` → **refused**: no coins move, `LawShock::BribeRefused`, `hardened_until = now + bribe_days`, the gang's `bribe_until = now + bribe_days` (it does not try again tomorrow), event `Bribe` ("refused").
- Otherwise **taken**: `price` moves from the treasury to the captain's wallet, the captain drifts `lawfulness −0.05, greed +0.02` (`Drift::TookBribe`) and remembers `Paid`, the gang's `bribe_until = now + bribe_days`, event `Bribe`, and the law rethinks at once: with the gang gated out of Crackdown the posture falls to Patrol or turns on the other gang.

The leader does not walk anywhere: as in M8, the brain is not embodied.

## 6. UI and events

New `EventKind`s: `Jailbreak` (gang purple), `Posture`, `Bribe` (law white).

- **Jail panel**: a Law section: posture and since when, target gang (link), captain (link), pinned flag, hardened/bribed notes, last jailbreak, the posture trace (top three), inmates per gang.
- **Hideout panel**: the boss (link, "inside") when jailed, jailed headcount, breakout cooldown, the muster line reads "Breakout musters in …" under BreakOut.
- **City panel**: a Law row (posture · target) and a pin selector (Auto / Patrol / Crackdown / Garrison) committed by the existing Apply button.
- **Inspector**: a guard who is captain shows "captain"; a convict's line shows "boss" when their gang remembers them as such.

## 7. Save compatibility

All new fields are `#[serde(default)]`; `[law]` and the new `[gangs]` keys default from the assets; `migrate_legacy` adds a `Law` to a Jail that has none. `LocationKey::RaidTarget` accepts the old name through a serde alias, so a save taken mid-raid still loads.

## 8. Testing and calibration

**Unit tests** (`citysim/tests/factions.rs`, `citysim/tests/law_faction.rs`): BreakOut wins with the boss inside and a brave leader; LieLow wins over it for a timid one under heat; the cooldown gates it; a breach with no guards frees the boss first and reopens the warrant; a breach against three guards with a weak raider fails and shocks; `jail_duty` per posture; the captain is the most lawful guard; Crackdown wins under pressure and names the right gang; a bribed gang cannot be the target; Garrison follows a `Jailbreak` shock; the crackdown patrol loop visits the target's Homes and Hideout; a bribe is taken by a greedy captain and refused by an incorruptible one; a member lying low plans Sleep at the Hideout; a pre-M9 save loads with a default `Law`.

**Scenario** (`citysim/tests/scenario.rs`, new `#[ignore]` 120-day test on seed 42): the bullets under *Goals and acceptance*.

**Calibration** targets, tuned via `[gangs]` and `[law]` only: jailbreaks 1–6 per 120 days; the law under Crackdown 20–60 % of days; raids meeting defenders at least a third of the time; Assault/day ≤ 6.4. Re-run `calibrate` (planner costs are unchanged, but the Sleep precondition is new; the regenerated table is committed).

**Throughput**: the law brain is O(guards + reports) daily and on shocks; `jail_duty` is a component read; nothing new per tick.

## 9. Out of scope (M10 and later)

Guards who take bribes individually (looking away from a crime), corrupt guards as gang assets, the law hiring from the gangs' enemies, trials and fines beyond the full-Jail fine, emergent gang splits, order propagation by gossip, homeless gangs after a full Hideout seizure, and the jail-capacity residual itself (a breakout empties it a little; that is a side effect, not the fix).
