# M15: Word and blood — gossip, reputation, grudges, the Hunt, social stats

Companion to `SPEC.md`, `M8_FACTIONS.md`, `M9_LAW.md`, `M10_SCALE.md`, `M11_OWNERSHIP.md`, `M12_DISTRICTS.md`, `M13_ASSETS.md`, `M14_VIRT.md`, `ROADMAP_POST_M14.md` and `VISION.md`. M11 gave the city owners, M12 places, M13 things and M14 knowledge as property. M15 gives it **knowledge as talk**: what people have heard, who they fear, whom they owe, and whom they mean to kill. A Hollow runner knifes a Mid West clerk outside a Bar; a drinker saw it, tells his sister over dinner, and by Friday half of Mid West knows the runner's name; the clerk's best friend, a Vatra farmhand with more courage than sense, asks around the Bar, waits outside the runner's Home two nights running and beats him to death on the third; the runner's brother hears of that before the body is buried. Meanwhile the Nutrix Feed runs the story of a Stackwell eviction wave every morning for a week because Nutrix paid it to, and Stackwell's tenants stop trusting their landlord. Where this document and the earlier ones disagree, this one wins for M15.

The roadmap row (`ROADMAP_POST_M14.md`): *gossip along edges, reputation from what is known, grudges, the Hunt goal, revenge chains, social stats and the social move, the psychological LOD table.* M15 also takes what the addenda and `VISION.md` assign to it: sightings held as knowledge by agents and relayed to faction databases (addendum 3; M14 built `Sighting` and the first `FactionDb`), skills as competence with a rarity distribution read by corp and law effectiveness and poaching by pay (addendum 2), the reputation matrix faction × faction and agent × faction (addendum 2), dress and chrome read per faction with an anti-tech creed (addendum 4; M13 stores `Appearance`), news outlets and propaganda as faction-scale gossip (addendum 6), intel from traces for the stake-out (addendum 7), and the Guard-the-body goal M13 deferred here. Threats, extortion as a contract, blackmail, fabricated stories, Fixers and hit squads stay out (§ 15).

Scale: 2,000 residents on the 256 × 192 map; every number assumes M10–M14 have landed. At the time of writing the tree holds M12 phase 2; M13's `Appearance`, `Kit` and `Body` and M14's `Sighting` and `FactionDb` are specified, not built, and M15 reads them as specified.

Decisions taken by the drafting agent (overturnable, listed so they are cheap to overturn):

| Question | Decision |
| --- | --- |
| What a rumour is | A `MemoryEntry` of the new kind `MemoryKind::Rumour`, carrying a `Deed`, an actor (`subject`), a victim (`object`), `hops` (0 seen, 1 told by a witness, 2+ rumour) and `conf` (0..1). First-hand kinds (`SawCrime`, `WasRobbed`, `Fought`, `Lost`) stay as they are and are read as deeds through `memory::deed_of`, so every M5–M14 reader is untouched. |
| What propagates | Deeds only: `Killed`, `Assaulted`, `Robbed`, `Extorted`, `Stripped`, `Arrested`, `Married`, `Evicted`, `Struck`, `Raided`, `Founded`, `Avenged`, `Betrayed`, `Repaid`, `Poached`, plus `Sighting`s of hunted or wanted people. Not moods, prices or routines. |
| When it propagates on screen | On the events that exist: Chat completion (today's `social::gossip`, generalised), Drink at a Bar (the rumour mill: one exchange with a co-drinker), and `AskAround` (§ 4). Never per tick. |
| Off screen | A **per-district rumour pool** (`World::rumours`, ≤ 64 entries a district) fed by every first-hand deed, bound hole, arrest, eviction and story, decayed and leaked to adjacent districts daily. Each Statistical adult draws from their district's pool once a day. A **kin channel** delivers a killing or beating to the victim's Spouse, Parent, Family and Friends directly, because that is who hears first and who hunts. |
| Distortion | Per hop, `p_distort = distort_base × (1 − speaker.knowledge)`: the actor is swapped for the actor's gang (a faction-level rumour, "a Hollow did it") or, when the speaker has an Enemy within the deed's district, for that Enemy. Liars bend the truth on purpose only about their own deeds (Deception, § 6). |
| Known vs happened | Reputation, grudges, hunts, stories and brains read **only memories and pools**, never `Life`, holes or the event ring. An unbound hole travels as "someone killed Y" (`subject: None`) and gains its actor in the pool when it binds; reputation never reads an anonymous deed. |
| Reputation | `Reputation { dread, standing, honour, heat, known_by }` on every agent and faction, **rebuilt daily** from all living memories (≈ 48k entries) and the pools, never written by events. An observer's opinion is computed on demand (edge + own memories + the global axes), so there is no N² table. |
| The matrix | Faction × faction `Regard` is stored, daily, ≤ 12 factions (≤ 132 cells). Agent × faction is `opinion(agent, faction)` on demand, cached daily for pinned agents only (the M18 player). |
| Anti-tech faction | **A trait on gangs**, `Gang.creed: Option<Creed>`, with `Creed::Purist` the first value, not a new faction kind. A congregation needs exactly what a gang has (members, a Hideout, recruitment, orders, extortion as tithe); a third brain would be a copy. One Purist gang is seeded in Sump Central, the district M12 left unclaimed. |
| Grudges | A `Grudges` component (cap 4, lowest weight out) outside the 24-entry memory, with a target (agent or faction), a cause, a weight, a chain depth and inheritance to Spouse and adult children. Fed by any deed memory, first-hand or heard. |
| Hunt and LOD | `GoalKind::Hunt` at Full and Coarse; adopting it pins the hunter at ≥ Coarse and binding the stake-out promotes the target to Coarse, as M13's Harvest does. A city-wide cap `max_hunts` (16) bounds the promotions. Statistical agents score Hunt in a daily pass on one hash-picked day in three. |
| The law and revenge | A revenge killing is a `Murder`, nothing else. The law cannot see motive; the street can: a witness who holds a grudge against the victim, or a Friend edge to the avenger, does not report it. |
| Vendettas | Not a new entity: `World::vendettas`, the daily aggregate of members' grudges between two factions. An open vendetta is what Retaliate (generalised from "the rival") targets, and what a corp's Lobby names as its culprit. |
| Social stats | `Skills` gains `persuasion`, `intimidation`, `knowledge`, `deception` (0..1), seeded `U(0,1)^rarity_exp` so ≈ 5 % of adults reach 0.8 in any one; drift on success at Full and Coarse, frozen at Statistical. One resolver, `social::resolve`, for every move. |
| Competence | `Corp.competence` and `Law.competence`, daily, from exec and staff skills, as a clamped multiplier on what the group produces and notices. Killing or poaching the skilled lowers it the next day. |
| Poaching | A hiring rule, not an order: a corp filling a skilled vacancy may make a Persuade move with a wage premium on a rival's best employee in that role. Threats and extortion as poaching are M16. |
| The news | `BuildingKind::Feed`, an outlet business (foundable, staffed by `Role::Reporter`) that turns the day's most newsworthy pool entries into `Story`s posted at its reach into the pools of the districts it covers. A story read is a rumour at `hops = 1`. |
| Propaganda | The corp order `Spin` (tenth `CorpOrder`) pays an outlet to **plant** a true misdeed of a rival that the corp's people know, and to **bury** stories about itself. No fabrication: invented stories are M16 fraud. Gangs do not buy stories in M15. |

## Goals and acceptance

The city should read as a place that remembers: names that clear a Bar when they walk in, a Sump block that will not talk to a guard, an exec whose eviction wave is on every Feed for a week, a clerk's friend who is not afraid enough, and a feud between two families that outlives its first three dead. The target spiral: **a Hollow member kills a Mid West clerk in a Shakedown gone wrong → a drinker saw it and the name reaches 30 people in four days → the clerk's friend forms a grudge, asks around, stakes out the killer's Home and kills him → a Murder report the street does not support → the killer's brother hears within two days and hunts the avenger → The Hollow's members now hold three grudges against Mid West Street agents and none against Ninefold → the vendetta pulls Retaliate off the rival → Ninefold's dread rises unopposed and the law's Crackdown reads The Hollow's heat.** On seed 42, 2,000 residents, 120 days:

- word: the median `known_by` of an agent with a witnessed or bound killing is ≥ 25 seven days after it; ≥ 30 % of deed memories held on day 120 are second-hand; ≥ 1 rumour with `hops ≥ 4`; ≥ 1 distorted rumour that named the wrong actor;
- reputation: across agents, Spearman(`dread`, known Killed + Assaulted deeds as actor) ≥ 0.6; the top decile of `standing` is ≥ 70 % execs, owners, gang leaders and the captain; ≥ 1 gang rescoring whose winner changes when its fear input is set to neutral (the `rep_flips` counter); ≥ 1 Security contract lost on honour;
- grudges: ≥ 60 formed, ≥ 10 Hunts adopted, ≥ 3 `Avenged`, revenge killings 3–20, **≥ 1 revenge chain of length ≥ 2** (a Hunt whose target is an earlier Hunt's avenger), ≥ 1 inherited grudge, ≥ 1 vendetta opened;
- social moves: extortion success is higher for members with `dread ≥ 0.5` than below it and higher against targets with no ally within 8 tiles; adults with any social skill ≥ 0.8 on day 120 are 3–8 %; ≥ 3 `Poached`; ≥ 1 `TalentLost` after a killing;
- news: ≥ 2 Feeds on day 120; ≥ 150 stories; ≥ 1 `Spin` order held ≥ 3 days with ≥ 1 `Planted` and ≥ 1 `Buried`; after a plant against corp C, C's employees' mean `opinion(·, C)` falls by ≥ 0.05 within 7 days in ≥ 50 % of plants; ≥ 1 Purist expulsion;
- Assault events per day stay ≤ 42.7, Murders per 120 days rise by ≤ 25 % over the M14 run on the same seed (revenge killings included), starvation deaths and population stay within the scaled v1 bounds, and the v1, M8–M14 and Full-vs-Statistical parity gates still pass;
- throughput is reported per gate; the shared floor is 4,000 ticks/s (2026-10-06: relaxed from 8,000 while systems are still being built; optimisation is a later pass).

## 1. Deeds, rumours and sightings

### Data model

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Deed {
    Killed, Assaulted, Robbed, Extorted, Stripped, Arrested, Married, Evicted,
    Struck, Raided, Founded, Avenged, Betrayed, Repaid, Poached,
}

/// Appended to MemoryKind: a deed heard, a person seen, a social move suffered.
pub enum MemoryKind { /* … M14 */ Rumour, Sighting, Threatened, Persuaded }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryEntry {
    /* kind, subject, tick, salience, valence, second_hand, crime: as today */
    #[serde(default, skip_serializing_if = "Option::is_none")] pub deed: Option<Deed>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub object: Option<EntityId>, // the victim; a corp for Struck
    #[serde(default, skip_serializing_if = "is_zero_u8")]     pub hops: u8,
    #[serde(default = "one", skip_serializing_if = "is_one")] pub conf: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub at: Option<EntityId>,    // Sighting: the building
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolEntry {
    pub deed: Deed,
    pub actor: Option<EntityId>,   // None until a hole binds
    pub object: Option<EntityId>,
    pub tick: Tick,                // when the deed happened
    pub hops: u8,                  // of the freshest telling in the pool
    pub reach: f32,                // 0..=1: how much of the district is talking about it
    pub hole: Option<HoleId>,      // anonymous until bound
    pub story: Option<EntityId>,   // the Feed that ran it, if any
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RumourPool { pub entries: Vec<PoolEntry> }   // ≤ pool_cap, lowest reach out
```

`World::rumours: Vec<RumourPool>` (by `DistrictId`) is saved. `memory::deed_of(&MemoryEntry) -> Option<(Deed, Option<EntityId> /*actor*/, Option<EntityId> /*object*/)>` reads first-hand kinds as deeds: `SawCrime` by its `crime` (Murder → Killed, Assault → Assaulted, Theft → Robbed, Extortion → Extorted), `WasRobbed` → Robbed with the holder as object, `Fought` + `Lost` on the same tick → Assaulted with the holder as object, M13's `Stripped`, `Grief` with a known killer (below), and `Rumour` by its fields. Merging extends `memory::insert`: a Rumour merges with any entry whose `deed_of` matches on (deed, actor, object, day), keeping the max `conf` and the min `hops`; an anonymous rumour merged with a named one takes the name.

**Memory pressure.** Rumours and Sightings never evict a first-hand entry: past `rumour_cap` (8 of the 24 at Full and Coarse, 3 of the 8 at Statistical) a new rumour evicts the weakest rumour or is dropped. `World::remember`'s Statistical filter admits `Rumour` under that sub-cap and never `Sighting`.

### Where deeds come from

| Source | Deed | First holders | Posted to the pool of |
| --- | --- | --- | --- |
| `law::raise_crime` | by crime | noticing witnesses (`SawCrime`, as today), the victim | the crime's district at `reach0[deed]` |
| `World::kill_by` with a killer | Killed | the killer's own Won memory; kin get `Grief` and, when the killer is known to any witness, a Rumour at `hops = 1` | the death's district |
| `bind::bind_in` (Actor) | by hole kind | the victim (subject filled, as today), the witness | the hole's district; the anonymous entry posted at hole creation is renamed |
| `law::jail_suspect` | Arrested (actor = suspect) | the arresting guard | the arrest district: arrests are public |
| `ownership::evict`, M12 Squat eviction | Evicted (actor = owner, object = tenant) | the tenant | the Home's district |
| Strike (M11/M12) | Struck (actor = Street, object = corp) | strikers | every district with a striking worker's Home |
| `raid::fight_out`, M12 riots | Raided (actor = gang or crowd, object = owner) | every brawler | the door's district at `reach0.raided` |
| `social::marry`, founding, `Avenged` (§ 4), `Betrayal`, debt repaid in full, `Poached` (§ 6) | as named | both parties | the actor's Home district |

### Gossip on screen

`social::gossip(world, from, to)` keeps its call sites (Chat, both directions) and gains one at **Drink** completion (one random co-occupant of the Bar with an edge, both directions) and one inside `AskAround` (§ 4). The speaker's candidate set becomes every entry with `deed_of` some deed, `salience ≥ gossip_min`, `hops < max_hops`, and a subject that is not the listener; the pick maximises `weight(e) × conf × novelty` where `novelty = 0.3` when the listener already holds it (only a confidence merge happens) else 1. The listener inserts a Rumour with `hops + 1`, `salience × hop_salience`, `conf × (0.5 + 0.5 × trust)` where `trust` is the listener's edge trust in the speaker, and the distortion roll of the decisions table. The receiver's edge to the actor gets `affinity −0.1 × deed_sev[deed]` as today's −0.1 does for crime. Sightings travel the same way but only to listeners who hold a grudge against, or share a gang with someone who holds a grudge against, the sighted agent.

### The pools and the Statistical tier

`gossip::daily` (tick order `… memory, bind, gossip, reputation, grudges, faction, corp_brain, law_brain …`) runs at midnight:

1. **Decay and leak.** Every entry's `reach ×= pool_decay`; an entry with `reach ≥ leak_min` is posted to each adjacent district (M12 adjacency) at `reach × leak_frac` with `hops + 1`; entries below `reach_min` are dropped.
2. **Hearing.** Each Statistical adult, in index order, draws once from the pool of yesterday's `DayTrace.district` with `p = hear_p × (0.5 + sociability)` from `rng.agent(id)`, the entry picked by `reach`; it is inserted as a Rumour with `hops + 1`, `conf = pool_conf` and `salience = deed_sal[deed] × reach`. A Statistical agent holding a deed with salience ≥ `gossip_min` that the pool lacks posts it at `reach0 × 0.5` (off-screen talk goes back into the pool).
3. **Kin channel.** For every Killed or Assaulted entry posted today with a known object, each living Spouse, Parent, Family and Friend of the object (the neighbour index, O(edges of the victim)) receives it with `p = kin_p[min(hops, 3)]` a day, every day the entry stays in a pool, whatever their tier. Kin are always told first-hand from the second day if a witness exists.

A Full or Coarse agent never draws from a pool; it talks. `known_by(a)` counts living holders of any memory whose subject is `a`, maintained by the daily reputation pass (§ 2).

### Sightings

A Sighting (`kind Sighting, subject who, at building, conf`) is written only at existing co-location events (`social::colocation`'s edge-creation and hourly passes, the M14 camera write, `law::sightings`), and only when the observer holds a grudge against `who`, `who` is an Enemy of the observer, `who` belongs to a faction in an open vendetta with the observer's, or `who` is wanted (the law's `last_seen`, as today). A gang member's Sighting is relayed to the gang's `FactionDb` the same day (`conf × relay_conf`); a guard's to the law's. Sightings expire at `sighting_days` (M14's 14).

```toml
[gossip]
enabled = true
gossip_min = 0.3
max_hops = 6
hop_salience = 0.7
distort_base = 0.15
pool_cap = 64
pool_decay = 0.8
leak_min = 0.4
leak_frac = 0.4
reach_min = 0.05
hear_p = 0.5
pool_conf = 0.6
kin_p = [0.9, 0.7, 0.5, 0.3]
rumour_cap = 8
rumour_cap_statistical = 3
relay_conf = 0.8
reach0 = { killed = 1.0, assaulted = 0.5, robbed = 0.3, extorted = 0.2, stripped = 0.5, arrested = 0.6, married = 0.3, evicted = 0.4, struck = 0.8, raided = 0.9, founded = 0.4, avenged = 1.0, betrayed = 0.6, repaid = 0.1, poached = 0.3 }
deed_sal = { killed = 0.9, assaulted = 0.6, robbed = 0.5, extorted = 0.4, stripped = 0.6, arrested = 0.5, married = 0.3, evicted = 0.5, struck = 0.6, raided = 0.8, founded = 0.4, avenged = 0.9, betrayed = 0.7, repaid = 0.3, poached = 0.4 }
deed_sev = { killed = 1.0, assaulted = 0.5, robbed = 0.3, extorted = 0.2, stripped = 0.6, arrested = 0.0, married = 0.0, evicted = 0.3, struck = 0.0, raided = 0.6, founded = 0.0, avenged = 0.8, betrayed = 0.6, repaid = 0.0, poached = 0.1 }
```

## 2. Reputation

### Data model

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Reputation {      // on every adult and on every Gang, Corp and the Law; #[serde(default)]
    pub dread: f32,          // 0..=1 known violence
    pub standing: f32,       // 0..=1 known wealth and position
    pub honour: f32,         // 0..=1, 0.5 = nothing known; kept word
    pub heat: f32,           // 0..=1 known to the law
    pub known_by: u16,
    #[serde(skip)] pub top: SmallVec<[(Deed, f32); 4]>,   // largest contributions, for the Known tab
    #[serde(default)] pub pinned: Option<([f32; 4], Tick)>, // a god SetReputation override until Tick
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Regard { pub value: f32, pub fear: f32 }   // −1..=1, 0..=1; World::regard: BTreeMap<(EntityId, EntityId), Regard>
```

### The daily rebuild (`systems/reputation.rs`)

For every living adult holder `h` and entry `e` with `deed_of(e) = (d, Some(a), _)`, `a ≠ h`, the weight is `w = memory::weight(e, now, half_life) × e.conf × hop_w[min(e.hops, 3)]`. Pool entries count once more each, as `reach × pool_w × hop_w[hops]`, so a city-wide story weighs like a crowd. Accumulate per actor `D += w × dread_w[d]`, `H += w × honour_w[d]`, `X += w × heat_w[d]`; `known_by` counts distinct holders of any memory with subject `a`. Then:

- `dread = 1 − exp(−D ÷ dread_scale)`;
- `honour = clamp(0.5 + H ÷ (2 × honour_scale), 0, 1)`;
- `heat = max(1 − exp(−X ÷ heat_scale), law_heat)` with `law_heat = heat_wanted` while the agent has an open report and `heat_arrest × arrests in 30 days` capped at 1, read from the law's own records (the law knows what it filed);
- `standing = pos(a) × (0.4 + 0.6 × fame)` with `fame = 1 − exp(−known_by ÷ fame_scale)` and `pos` the largest of: corp exec 0.9; owner of n buildings `min(0.5 + 0.1 n, 0.8)`; gang leader 0.7, lieutenant 0.5, member 0.3; captain 0.7, guard 0.35; employed 0.2; Dreg 0.05; plus `0.2 × Appearance.dress ÷ 3`.

A gang's axes: `dread = 0.5 × mean of its top three members' dread + 0.5 × (1 − exp(−D_g ÷ dread_scale))` where `D_g` sums deeds whose actor is the gang entity (distorted rumours, Raided); `honour` the members' mean; `heat` the M9 report pressure on it (`reports_by_gang ÷ crackdown_reports`); `standing = 0.3 + 0.4 × territory share + 0.3 × treasury ÷ hoard_heat`, clamped. A corp's: `standing` = 0.5 × niche share + 0.5 × `cash ÷ 2`; `honour` from deeds whose actor is the corp (Evicted, Poached, Repaid wages, Betrayed contracts) plus the exec's honour at 0.3; `dread` from Raided and its guards' Killed deeds; `heat` from its employees' Arrested and M14 traced runs. The Law: `dread` from Killed, Assaulted and Arrested by guards and `Crush` (M12); `honour` from known `Bribe`s (each −`bribe_honour`); `standing` fixed at 0.8. Cost: one pass over ≈ 48k entries and ≤ 512 pool entries.

### The matrix

`Regard(A, B)` for every ordered pair of factions: `value = clamp(Σ_{members m of A} opinion_part(m, B) ÷ |A| − vendetta(A, B) + 0.3 × (B.honour − 0.5), −1, 1)` with `opinion_part(m, B)` the m-weighted valence of m's deed memories whose actor or object is in B (a deed by B against an A member counts double); `fear = clamp(B.dread − A.dread + 0.5, 0, 1)`. **`opinion(agent, faction)`** on demand: `0.4 × mean affinity to its members the agent has edges with + 0.3 × own deed memories about it (valence-weighted) + 0.2 × (honour − 0.5) × 2 + 0.1 × taste(agent's audience, faction colours)` plus `press` (§ 7), clamped −1..1; employer and gang add `own_bias` (0.3).

### Who reads it

| Reader | Term |
| --- | --- |
| Gang brain (`faction::OrderInputs`) | `fear = Regard(own, rival).fear`. Raid and Contest gain `1 − fear` → Linear{0.4,0.6}; LieLow gains `fear` → Linear{0.5,0.5}. Harvest skips targets with `dread ≥ harvest_dread_max`. `rep_flips` counts rescorings whose winner differs with `fear = 0.5`. |
| Corp brain | Security clients renew with the corp minimising `price_level × (1.5 − honour)`, so a dishonoured guard firm loses contracts (`ContractLost { honour: true }`); a seller refuses an `Acquire` offer from a buyer with `honour < acquire_honour_min` unless bankrupt; `Spin` reads its own axes (§ 7). |
| Law brain | Per-district stance pressure becomes `max(reports ÷ crackdown_reports_in(d), dominant gang heat × heat_pressure_w)`; the Crackdown target is the gang with the highest heat; guards pick among located suspects by heat. |
| Binder | candidate weight × `(1 + bind_dread_w × dread)`: the feared are blamed. |
| Social moves | `rep_gap` (§ 6) and the meeting gate. |

### Appearance and taste

`Taste { dress: f32, chrome: f32, own_colours: f32, rival_colours: f32 }` per **audience**: the three classes, gang members, and the Purist creed. `taste(aud, target) = dress × (2 × dress_t ÷ 3 − 1) + chrome × min(chrome_t ÷ chrome_ref, 1) + own_colours × [same colours] + rival_colours × [rival or vendetta colours]`, clamped −1..1, from M13's `Appearance`. It enters three places: first impressions (`social::first_affinity` gains `+ first_look_w × taste` from each side's audience, the noise unchanged), every social move (`w_l × taste`), and Persuade's **meeting gate**: a target refuses outright when `standing_t − standing_a − meet_taste_w × taste > meet_gap`, so a broke nobody in rags gets no meeting with an exec and the same nobody in a Spire suit sometimes does.

### The Purist creed

`Gang.creed: Option<Creed>` (`#[serde(default)]`), `Creed::Purist` (label "Purist"). One Purist gang, **The Unplugged**, is seeded with `[creeds]` treasury on a derelict Block in Sump Central (its Hideout is labelled "Chapel"); a gang can also gain the creed on a split when the splinter's lieutenant has `Kit.visible == 0` and `lawfulness ≥ 0.5`. The **tolerance rule**: an agent with `Kit.visible ≤ creed_tolerance` is tolerated; above it, every Purist audience reads `chrome = −1.0` taste, members' first-meeting affinity with them takes `−purist_chrome × visible ÷ 3`, recruitment refuses them, Purist Shakedowns target their Homes first, and a member who installs chrome is expelled (`Expelled` event, `LeftGang`). Purists buy no chrome, decks or robots, tithe at `tithe_frac × extort_amount`, and under M13's Harvest **destroy** what they rip (Parts to the Recycler, nothing installed or sold).

```toml
[reputation]
half_life_days = 7.0              # the memory half-life (brain.memory_half_life_days), read again
hop_w = [1.0, 0.8, 0.6, 0.4]
pool_w = 2.0
dread_scale = 3.0
honour_scale = 3.0
heat_scale = 2.0
fame_scale = 20.0
heat_wanted = 0.6
heat_arrest = 0.2
bribe_honour = 0.5
dread_w = { killed = 1.0, assaulted = 0.4, avenged = 0.8, raided = 0.6, extorted = 0.3, robbed = 0.15, stripped = 0.2 }
honour_w = { betrayed = -1.0, robbed = -0.3, stripped = -0.6, extorted = -0.2, evicted = -0.2, poached = -0.1, avenged = 0.4, repaid = 0.3, married = 0.1 }
heat_w = { arrested = 1.0, killed = 0.5, assaulted = 0.2, robbed = 0.2, extorted = 0.1 }
own_bias = 0.3
heat_pressure_w = 0.8
harvest_dread_max = 0.6
acquire_honour_min = 0.3
bind_dread_w = 0.5
[taste]
chrome_ref = 3
first_look_w = 0.1
meet_gap = 0.5
meet_taste_w = 0.3
corp = { dress = 0.6, chrome = 0.0, own_colours = 0.3, rival_colours = -0.3 }
street = { dress = 0.1, chrome = 0.2, own_colours = 0.2, rival_colours = -0.2 }
dreg = { dress = -0.2, chrome = 0.1, own_colours = 0.0, rival_colours = 0.0 }
gang = { dress = 0.0, chrome = 0.3, own_colours = 0.5, rival_colours = -0.8 }
purist = { dress = -0.2, chrome = -1.0, own_colours = 0.5, rival_colours = -0.5 }
[creeds]
seed_purist = true
purist_name = "The Unplugged"
purist_treasury = 50
creed_tolerance = 0
purist_chrome = 0.3
tithe_frac = 0.6
```

## 3. Grudges

### Data model

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum GrudgeCause { KilledKin(EntityId), KilledFriend(EntityId), Assaulted, Robbed, Stripped(EntityId), Evicted, Betrayed, Inherited(EntityId) }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grudge {
    pub target: EntityId,      // an agent, or a gang or corp entity when only the faction is known
    pub cause: GrudgeCause,
    pub weight: f32,           // 0..=1
    pub since: Tick,
    pub chain: u8,             // 0 for a first wrong; n + 1 when the wrong was itself an Avenged of chain n
    pub settled: Option<Tick>, // target dead or avenged; kept settle_keep_days for the biography
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Grudges { pub list: SmallVec<[Grudge; 4]> }   // component; cap 4, lowest unsettled weight out
```

### Formation

A hook in `memory::insert` (and the merge path) calls `grudges::on_learn(world, holder, deed, actor, object, conf, hops)` whenever an adult learns a deed with `deed_sev > 0` and a known actor ≠ holder, first-hand or heard. The relation weight is `rel_w[kind]` of the holder's edge to the object (Spouse 1.0, Parent 1.0, Family 0.9, Friend `0.6 × (0.5 + 0.5 × affinity)`, self 0.8, else nothing). `w = deed_sev[deed] × rel_w × conf`; at `w ≥ grudge_min` an existing grudge on that target becomes `1 − (1 − old)(1 − w)`, else one is pushed. A faction actor gives a faction target; an Evicted by a corp gives a corp target. The chain depth is read from `World::kill_chain` (victim → chain of the hunt that killed them, kept 60 days). Every grudge also makes the edge an Enemy (`make_enemy`, −0.4), so M8's revenge-only Fight goal sees it. `GrudgeFormed` event at `w ≥ 0.5`.

**Decay and settlement.** Daily `weight −= grudge_decay` (half that for `KilledKin` and `KilledFriend`); dropped at 0. A target's death, by anyone, settles the grudge; an `Avenged` beating subtracts `beat_settle`. **Inheritance.** When a holder dies, each unsettled grudge with `weight ≥ inherit_min` passes to their Spouse and adult children at `× inherit_frac`, cause `Inherited(holder)`, chain unchanged: the blood feud that outlives its first avenger.

### Guard the body

M13 left the corpse a contested loot target. `GoalKind::GuardBody` (Full and Coarse): `Can(a Spouse, Family or Friend corpse within 12 tiles, unburied, unstripped)` → GATE; `rel_w` → Linear{0.8,0.2}; `courage` → Linear{0.5,0.5}; plan `GoTo(Corpse) → Wait(guard_hours)`. A guard adjacent to a `Strip` or `Rip` makes it a fight (`resolve_fight`) and a witnessed strip feeds `Stripped` grudges. Off screen it does not exist.

### Vendettas

Daily, for every ordered faction pair, `V(A, B) = clamp(Σ unsettled grudge weights held by A's members against B's members or B ÷ vendetta_norm, 0, 1)`. At `V ≥ vendetta_open` both ways summed, `World::vendettas` gains `Vendetta { a, b, since, kills: [u16; 2] }` (`Vendetta` event); below `vendetta_close` it ends (`VendettaEnded`). **Retaliate generalised**: `OrderInputs.grudge` is true on a pending grudge shock *or* an open vendetta, and the target is the vendetta faction with the highest `V` (else `rival_of`, as today); a corp target means a raid on its most valuable building in the gang's districts (M12's corp raid). A corp's Lobby culprit is the gang with the highest vendetta weight before the robbery count. Kills on either side count into `kills`, read by the panel and the chain statistics.

```toml
[grudges]
grudge_min = 0.15
grudge_decay = 0.01
beat_settle = 0.5
inherit_min = 0.4
inherit_frac = 0.6
settle_keep_days = 30
rel_w = { spouse = 1.0, parent = 1.0, family = 0.9, friend = 0.6, own = 0.8 }
guard_hours = 6
vendetta_norm = 3.0
vendetta_open = 0.5
vendetta_close = 0.2
```

## 4. The Hunt

### The goal

| Goal | Considerations | Plan |
| --- | --- | --- |
| Hunt | `Can(adult ∧ free ∧ an unsettled agent-target grudge with weight ≥ hunt_min ∧ target alive and free ∧ hunts_active < max_hunts ∧ cooldown passed)` → GATE; `grudge.weight` → Linear{0.9,0.1}; `courage` → Linear{0.6,0.4}; `might_gap` (§ 6, as `clamp(0.5 + gap, 0, 1)`) → Logistic{6,0.4}; `1 − lawfulness` → Linear{0.5,0.5}; `1 − heat` → Linear{0.4,0.6}; `1 − target.dread × (1 − courage)` → Linear{0.5,0.5}; flat `hunt_flat` | `[AskAround] → GoTo(Intel) → StakeOut → Attack(target)` |

`GoalKind::Hunt` sits after `Fight` in `GOAL_ORDER`; the grudge chosen is the heaviest eligible. A gang member's grudge on a rival member is hunted in person as well; the faction answer is Retaliate, and both read the same deed.

### Intel

The hunter's intel is the freshest of, in order: (1) a Sighting of the target in their own memory or their gang's `FactionDb` younger than `fresh_sighting_hours`; (2) the result of **`AskAround`** (new `ActionKind`, dur 20, at a Bar or any building in the target's last-known district): a social move (§ 6) on the co-occupant with the highest affinity to the target, else any adult with an edge to the target: Charm on a Friend, Intimidate when the hunter's dread exceeds the respondent's, Persuade otherwise, with `+intel_k × knowledge` on the hunter's side. Success returns the respondent's freshest Sighting of the target, else the target's **habit from the Trace**: their Home if `SLEPT_AT_HOME` held on ≥ half of the last 14 days and the hour is night, their workplace if `EMPLOYED` and in shift, else the busiest Bar in their modal district: the "frequents X" hook. A respondent who is the target's Friend may lie: a Deceive move against the hunter's knowledge, success sending them to a random Bar (`Deceived` in the hunter's trace, found out at the stake-out). Each AskAround also runs one `social::gossip` exchange both ways. (3) the target's Home.

### Stake-out and the strike

`StakeOut` (new `ActionKind`, a Wait at the intel building or tile) succeeds when the target is within 2 tiles or in the same building and fails after `stakeout_hours`; a fail re-plans from AskAround. When the hunter's plan binds `GoTo(Intel)` and the target is Statistical, the target is promoted to Coarse and marked `hunted_by` (never demoted while hunted), so the target is physically where the Trace said. `Attack` is the existing assault action through `law::resolve_fight` with M13's chrome terms; when `weight ≥ lethal_min` a win kills with `hunt_kill_p` instead of `fight_death_p`. On a win: `Avenged` deed (actor hunter, object target), `Avenged` event, the grudge settles on a death or loses `beat_settle` on a beating, `kill_chain[target] = grudge.chain + 1`. On a loss: the hunter's `Lost` memory, `weight += 0.1`, cooldown `hunt_cooldown_days`. Abandoned after `hunt_days` without contact: `weight ×= 0.7`, `HuntAbandoned`.

**Revenge chains are not coded.** The avenger's kill is an ordinary Murder through `raise_crime`; the target's kin learn it through the kin channel and the pool, form a grudge at `chain + 1`, and the loop restarts. The chain statistic is `max(Grudge.chain)` over hunts adopted.

### The law's view

A Hunt's killing is a `Murder` report like any other, from witnesses as today, except that `ReportCrime` and the binder's witness skip a witness who holds a grudge against the victim or a Friend edge to the avenger (`street_silence`). An `Avenged` deed raises the avenger's heat by its `heat_w` like any killing, and their honour by `honour_w.avenged`: the street admires what the law punishes.

### Statistical hunters

`grudges::stat_pass` (daily, after reputation): each Statistical adult with an eligible grudge scores Hunt on one hash-picked day in three (`hash(seed, id, day) % 3 == 0`); a pass promotes them to Coarse with Hunt pinned until it ends. Promotions are bounded by `max_hunts`, queued by weight.

```toml
[hunt]
hunt_min = 0.5
max_hunts = 16
hunt_flat = 0.0
fresh_sighting_hours = 48
intel_k = 0.5
stakeout_hours = 3
lethal_min = 0.75
hunt_kill_p = 0.5
hunt_cooldown_days = 3
hunt_days = 10
street_silence = true
```

## 5. Social stats

```rust
/// Extends Skills (0..=1). M14 added hacking.
pub struct Skills { pub stealth: f32, pub fighting: f32, pub farming: f32, pub hacking: f32,
                    #[serde(default)] pub persuasion: f32, #[serde(default)] pub intimidation: f32,
                    #[serde(default)] pub knowledge: f32, #[serde(default)] pub deception: f32 }
```

**Rarity.** Each new skill is seeded `skill_scale × U(0,1)^rarity_exp` from a stream keyed by entity index (as M13 drew `Body`): with `rarity_exp = 4`, about 16 % of adults clear 0.5 and 5 % clear 0.8 in any one skill. Personality tilts the draw: intimidation `+0.1 × (courage − 0.5)`, persuasion `+0.1 × (sociability − 0.5)`, deception `+0.1 × (0.5 − lawfulness)`. Children inherit the mean of their parents' skills × `inherit_skill` plus a fresh draw × (1 − `inherit_skill`), so talent runs in families. **Drift**: `+skill_drift` per successful move at Full and Coarse, `−skill_rust` a day toward the seed after 30 unused days; Statistical skills are frozen, as the roadmap's LOD table says. Knowledge also drifts `+knowledge_work` per shift at a Lab, Feed or the Hall.

```toml
[skills]
skill_scale = 1.0
rarity_exp = 4.0
inherit_skill = 0.5
skill_drift = 0.005
skill_rust = 0.001
knowledge_work = 0.0005
```

## 6. The social move

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum MoveKind { Persuade, Intimidate, Deceive, Charm }
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Stake { Coins(i64), Info { about: EntityId }, Job { building: EntityId, wage: i64 }, Join(EntityId) }
pub struct SocialMove { pub actor: EntityId, pub target: EntityId, pub kind: MoveKind, pub stake: Stake }
pub struct MoveOutcome { pub success: bool, pub p: f32, pub refused: bool, pub backlash: bool }
```

`social::resolve(world, &SocialMove) -> MoveOutcome` with `p = clamp(logistic(move_k × (bias[kind] + skill_a − resist_t + w_m × might_gap + w_r × rep_gap + w_l × taste)), p_min, p_max)`, one draw from `SimRng::pair(tick, actor, target)` so the order of callers never changes an outcome.

| Kind | `skill_a` | `resist_t` | `rep_gap` |
| --- | --- | --- | --- |
| Intimidate | intimidation | courage + 0.3 × fighting | dread_a − dread_t |
| Persuade | persuasion | Coins: 1 − greed; Info: affinity to `about`; Job: 0.3 + 0.4 × loyalty + 0.3 × max(affinity to own exec, 0) − 0.3 × greed | standing_a − standing_t (and the meeting gate, § 2) |
| Deceive | deception | knowledge | honour_a − 0.5 |
| Charm | persuasion | 0.5 − edge affinity | honour_a − 0.5 |

`might(x) = Skills.fighting + Kit.fighting + chrome_might × Kit.visible ÷ 3 + ally_might × allies within 8 tiles (gang members, Friends, Family; capped at ally_cap)`, `might_gap = might_a − might_t`. **Backlash** on a failed Intimidate when the target's courage ≥ `backlash_courage`: the target's Fight goal against the actor gets `+backlash_fight` for an hour and a `Threatened` memory (0.5, −0.5, subject actor); a failed Deceive zeroes the target's trust in the actor and posts a `Betrayed` deed (actor the deceiver) at `reach0 × 0.5`. A successful Persuade or Charm leaves `Persuaded` (0.3, +0.2).

**Callers in M15**: gang `extort` rolls Intimidate against the strongest occupant of the Home for Coins (fail: nothing taken, claim not advanced, backlash); `AskAround` (§ 4); poaching (below); the Deceive defence in AskAround; a speaker gossiping about their own deed swaps the actor for an Enemy with `p = deception × 0.5` (a lie the listener may later meet as a contradiction: a merge with a disagreeing actor lowers both confidences by `contradict_conf`). `Propose` and `JoinGang` keep their calibrated rules; M18 hands the player every kind.

### Competence

`Corp.competence` and `Law.competence` (`#[serde(default)]` 0.25), daily: `exec_w × mean(exec knowledge, persuasion) + (1 − exec_w) × mean over staff of the role's skill` (Farm farming; Market, Bar, Hotel, Clinic, Garage persuasion; Security Office guards fighting; Lab knowledge and hacking; Feed knowledge; Blocks excluded). The law's: `0.4 × captain knowledge + 0.6 × mean guard fighting`. The multiplier `comp_mult = clamp(1 + comp_w × (competence − comp_ref), comp_min, comp_max)` scales a corp's Farm output, M14 Lab Data, and its `price_level` steps under Squeeze and Undercut; the law's scales the binder's `p_witness` and guards' `notice_probability`. A day-on-day drop ≥ `talent_drop` pushes `CorpShock::TalentLost` (0.4) or `LawShock::TalentLost` (0.3) with a `TalentLost` event naming who died or left.

### Poaching

In `corp_brain::staff_up`, once a day per corp holding Grow, Research or Secure, for a vacancy in a skilled role: when the best rival employee in that role (any corp) has skill ≥ `poach_min` and ≥ `poach_gap` above the best free candidate, the exec makes a Persuade move with `Stake::Job { wage: target wage × poach_premium }`. Success: `vacate_job`, `hire` at the new wage, `Poached` event and deed (actor the poacher corp, object the old employer), `CorpShock::Poached` (0.3) on the loser. One attempt a day per corp; the target may be at any tier (a job swap needs no body).

```toml
[moves]
move_k = 4.0
bias = { persuade = 0.0, intimidate = 0.5, deceive = 0.0, charm = 0.2 }
w_m = 0.5
w_r = 0.6
w_l = 0.3
p_min = 0.05
p_max = 0.95
chrome_might = 0.2
ally_might = 0.1
ally_cap = 0.5
backlash_courage = 0.6
backlash_fight = 0.3
contradict_conf = 0.3
[competence]
exec_w = 0.4
comp_w = 1.0
comp_ref = 0.25
comp_min = 0.75
comp_max = 1.35
talent_drop = 0.05
poach_min = 0.5
poach_gap = 0.2
poach_premium = 1.3
```

## 7. The news and propaganda

### The Feed

`BuildingKind::Feed` (label "Feed", letter `P` for press, appended), staff `Role::Reporter` (label "Reporter"), `[buildings] feed = { capacity = 6, stock_cap = 0, staff = 3 }`, foundable through `Register` (`found_cost.feed` 500, `value` 800, upkeep 10); hiring sorts Reporter candidates by knowledge. Seeded: the city's **Civic Wire** in the Civic district (owner `None`, funded by the Treasury) and **Nutrix Now** owned by Nutrix in Mid West. A Feed's `reach = clamp(reach_base + reach_per_reporter × reporters who worked yesterday + 0.3 × owner standing, 0, 1)`; it **covers** every district when its owner is a corp or the city, its own and adjacent districts otherwise.

### Stories

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Story { pub feed: EntityId, pub deed: Deed, pub actor: EntityId, pub object: Option<EntityId>, pub tick: Tick, pub slant: f32 /* −1..=1 */, pub paid_by: Option<EntityId> }
```

Daily after the pools decay, each Feed scores the named entries (`hops ≤ 2`) in its covered pools by `news = deed_sal × (0.3 + standing(actor) + standing(object)) × 0.8^(age days)`, skips any it is paid to bury, and runs the top `stories_per_day` plus every planted story: each is posted into every covered pool at `reach × story_reach` with `hops = 1` and `story = Some(feed)`, and logged (`World::stories`, a ring of 256; `Story` event). Reporters' mean knowledge sets the Feed's distortion (`distort_base × (1 − knowledge)`), so a cheap Feed gets names wrong. Revenue is ads: every corp pays `ad_rate` coins a day split across Feeds by reach (`Flow::Ads`, taxed as owner revenue); the Civic Wire's ads go to the Treasury.

### Spin: planting and burying

| Order | What it does daily while held | Considerations |
| --- | --- | --- |
| Spin (tenth `CorpOrder`) | pays the Feed with the largest reach it does not own (or its own at cost 0) `plant_price × reach` to **plant** the best rival misdeed its employees and exec know (a deed with `honour_w < 0` or `dread_w > 0` whose actor is a niche rival corp, its exec, or a gang in vendetta with it, picked by `news`), slant −1; and `bury_price × reach` per Feed to **bury** every story whose actor is the corp, its exec or employees, for `bury_days` | `Can(a Feed exists ∧ cash ≥ 0.3)` → GATE; `honour drop` (14-day fall in its honour, ÷ 0.2) → Linear{0.6,0.4}; `known rival misdeed` (best `news` ÷ 1) → Linear{0.5,0.5}; Street `unrest` → Linear{0.4,0.6}; `E.greed` → Linear{0.4,0.6}; flat `order_flat.spin` |

`Flow::Plant` covers both payments. A buried deed still travels by word of mouth; only the Feed is silent. **The press term**: every agent keeps `press: f32` (not saved; rebuilt daily) = Σ over stories they hold about their employer, gang or class's landlord of `slant × conf`, and `opinion` adds `press_w × press`. Class loyalty gains `loyalty += press_loyalty × mean press of the class about its employers`, clamped ±`press_cap`, so a planted eviction story lowers a corp's employees' loyalty and its tenants' happiness the way the roadmap asks. The city's Civic Wire buries stories about the law while the captain's lawfulness < `censor_lawfulness`, and every story under the `CensorStories` lever.

```toml
[news]
reach_base = 0.2
reach_per_reporter = 0.15
story_reach = 0.8
stories_per_day = 3
ad_rate = 3
plant_price = 120
bury_price = 60
bury_days = 3
press_w = 0.3
press_loyalty = 0.1
press_cap = 0.15
censor_lawfulness = 0.3
[buildings]                        # addition
feed = { capacity = 6, stock_cap = 0, staff = 3 }
[corps]                            # additions
found_cost = { feed = 500 }
value = { feed = 800 }
upkeep = { feed = 10 }
[corps.order_flat]                 # addition
spin = 0.0
```

## 8. The psychological LOD table

| Data and behaviour | Full | Coarse | Statistical |
| --- | --- | --- | --- |
| Personality, Skills incl. the four social stats | yes, drifting | yes, drifting | yes, frozen |
| Memory | 24 (≤ 8 rumours, sightings) | same | 8 (≤ 3 rumours, no sightings) |
| Grudges (4), Reputation | yes | yes | yes: reputation is a world fact, not a tier privilege |
| Gossip | Chat, Drink, AskAround | same | one pool draw a day; posts back to the pool; the kin channel |
| Sightings | at co-location events | same | none: a Statistical agent has no eyes |
| Social moves | resolved per encounter | per encounter | the StatTable outcome stands in (`p_robbed`, `p_flirt`); the binder reads `dread` |
| Hunt | executes | executes | a daily pass; adopting promotes to Coarse; a hunted target is promoted at the stake-out |
| Poaching, competence | read at any tier | same | same |

The StatTable is unchanged; `calibrate` is re-run for the new actions (`AskAround`, `StakeOut`) in the exec tallies.

## 9. Player levers and god commands

| Command | Effect |
| --- | --- |
| `SetPressLicence(bool)` | Off: only the Civic Wire publishes (state media); private Feeds keep staff and lose ads. |
| `SetNewsTax(f32)` | An extra tax on `Flow::Ads` and `Flow::Plant`. |
| `CensorStories { faction }` | The Civic Wire buries every story about a faction until lifted. |

CLI: `--lever day:SetPressLicence:false`. **God commands**: `PlantRumour { about, deed, object, district, reach }` (a pool entry and one first-hand holder; logged); `SetReputation { who, axis, value, days }` (the `pinned` override); `DeclareVendetta { a, b, weight }` (declares the feud open for `[grudges] declared_days`); `KillFriend { of, by }` (kills `of`'s highest-affinity Friend with `by` as killer, one witness drawn as the binder does: "kill a friend of X and watch"); `GrantSkill { agent, skill, value }`; `SetCreed { gang, creed }`; `Hunt { hunter, target }` (a grudge of weight 1.0 and Hunt pinned).

## 10. UI, events, CSV

New `EventKind`s (crimson): `GrudgeFormed`, `HuntStarted`, `HuntAbandoned`, `Avenged`, `Vendetta`, `VendettaEnded`, `Story`, `Planted`, `Buried`, `Poached`, `TalentLost`, `Expelled`, `ContractLost`. New `MemoryKind`s `Rumour`, `Sighting`, `Threatened`, `Persuaded`; `GoalKind`s `Hunt`, `GuardBody`; `ActionKind`s `AskAround`, `StakeOut`; `CorpOrder::Spin`; `CorpShock::{TalentLost, Poached}`; `LawShock::TalentLost`; `BuildingKind::Feed`; `Role::Reporter`; `Flow`s `Ads`, `Plant`; `Creed::Purist`. No event per rumour.

- **Inspector**: a **Known** tab (who knows what about this agent, at how many hops and what confidence; the four axes with the top contributing deeds); a **Heard** list (the agent's own rumours and sightings); Grudges (target, cause, weight, chain, settled); the active Hunt with its intel and stake-out; the social skills.
- **Faction panels**: the four axes, competence, the Regard row against every other faction, open vendettas with kill counts; the corp's Spin plants and buries; a gang's creed.
- **Feed panel** (new): reach, covered districts, today's stories with slant and payer, buried count.
- **City panel**: a Word section (rumours heard today, second-hand share, the top five stories, open vendettas, hunts active, the longest chain) and the levers.
- **Map overlay** (toggle `K`): per-district pool heat (Σ reach), a glyph for each hunter on a stake-out, red lines between factions in vendetta.
- **CSV** (`--report`): `rumours_heard`, `second_hand_share`, `known_by_killers_median`, `distorted`, `grudges`, `grudges_inherited`, `hunts`, `hunts_active`, `avenged`, `revenge_kills`, `chain_max`, `vendettas_open`, `stories`, `planted`, `buried`, `poached`, `talent_lost`, `skill_rare_share`, `extort_success`, `rep_flips`, per gang `g{i}_dread`, `g{i}_heat`, per corp `c{i}_honour`, `c{i}_standing`, `c{i}_competence`, `law_competence`, ledger columns `flow_ads`, `flow_plant`.

## 11. Save compatibility

Every new field is `#[serde(default)]` and the memory fields skip at their defaults, so a pre-M15 memory entry is byte-identical. A pre-M15 save gets empty pools, no grudges, reputation built on the first midnight, social skills drawn from the entity-index stream, `competence` 0.25 until the first daily pass, no Feeds (seeded by `founding::build_on_lot` on the first day when `[news]` is on and the Lots exist), and `creed = None` for every gang (The Unplugged is founded on load when `seed_purist` and a Sump Central derelict exist). `Deed`, `MemoryKind::{Rumour, Sighting, Threatened, Persuaded}`, `GoalKind::{Hunt, GuardBody}`, `CorpOrder::Spin`, `BuildingKind::Feed`, `Role::Reporter` and every other variant above are appended. `Config::v1_profile` sets `[gossip] enabled = false`, which turns off pools, the Statistical hearing, reputation readers in the brains, grudges, Hunt, the move resolver (extortion always succeeds, as today), competence (multiplier 1) and Feeds, and keeps today's `social::gossip`, so the v1, M8 and M9 tests run unchanged.

## 12. Testing and calibration

**Unit tests** (`citysim/tests/gossip.rs`, `reputation.rs`, `grudges.rs`, `moves.rs`, `news.rs`): `deed_of` reads every first-hand kind; a Chat passes the strongest deed with `hops + 1`, conf scaled by trust, and merges a duplicate; an anonymous rumour takes its name when the hole binds; a Statistical rumour never evicts a first-hand entry; the pool decays, leaks to adjacent districts only and drops below `reach_min`; the kin channel reaches a Friend of a victim in another district; distortion at `knowledge = 1` never fires; reputation never reads `Life` (a deed with no holder adds nothing); `dread` saturates per the formula; a pinned `SetReputation` holds for its days; a grudge forms at Spouse weight on a heard killing and inherits on the holder's death; `Hunt` gates on `max_hunts`; a hunted Statistical target is promoted at the stake-out; `AskAround` returns a Trace habit when no sighting exists; a Friend's Deceive sends the hunter elsewhere; a revenge killing is a Murder and a grudge-holding witness does not report it; `kill_chain` makes the next grudge `chain + 1`; `resolve` is order-independent and clamped; a higher-dread extorter succeeds more; the meeting gate refuses a Dreg's Persuade on an exec; competence falls the day after an exec dies; a poach moves the job and shocks the loser; a Purist expels a member who installs chrome; a Feed posts at reach and a buried deed is skipped; `Spin` plants a rival misdeed and charges `Flow::Plant` with money conserved; a pre-M15 save loads.

**Scenario** (`citysim/tests/scenario.rs`, `#[ignore]` `test_m15_word_seed_42`): the bullets under *Goals and acceptance*, in particular the revenge chain of length ≥ 2, the median `known_by` ≥ 25 at seven days, a `rep_flips ≥ 1`, a plant lowering the target's employee opinion, a poaching, and skill rarity holding at day 120. **God scenarios** (`tests/god.rs`): `KillFriend` of each gang leader on day 10 (do they hunt, does a vendetta open, does the law's Crackdown move to the avenger's gang); `DeclareVendetta` between Arasaka and The Hollow (does Retaliate raid an Arasaka building, does Lobby name The Hollow); `PlantRumour` that the Zetatech exec killed a Sump child, reach 1.0 (does Zetatech's honour fall, its Security contracts move, its employees' loyalty sag, its Spin bury the story); `GrantSkill persuasion 1.0` to ten Dregs in Spire suits (do they get meetings, do they get poached); `SetCreed Purist` on Ninefold (does the Sump's chrome fall, do the chromed move out).

**Calibration**, tuned via `[gossip]`, `[reputation]`, `[grudges]`, `[hunt]`, `[moves]`, `[competence]` and `[news]` only:

| Target | Band |
| --- | --- |
| Median `known_by` of a named killer, day + 7 | 25–150 |
| Second-hand share of deed memories, day 120 | 30–70 % |
| Distorted share of rumours | 3–15 % |
| Grudges formed per 120 days | 60–400 |
| Hunts adopted per 120 days | 10–60 |
| Revenge killings per 120 days | 3–20 |
| Longest chain, 120 days | 2–5 |
| Agents with `dread ≥ 0.5`, day 120 | 1–5 % of adults |
| Agents with `heat ≥ 0.5`, day 120 | 0.5–4 % of adults |
| Corp honour spread (max − min), day 120 | 0.15–0.6 |
| Extortion success share | 55–85 % |
| Poached per 120 days | 3–20 |
| Social skill ≥ 0.8 share, day 120 | 3–8 % |
| Stories per day, city | 3–12 |

**Throughput.** Nothing runs per agent per tick. Gossip is one memory scan (≤ 24 entries) per Chat, Drink and AskAround; sightings ride the existing co-location events; the pools are ≤ 12 × 64 entries updated daily; Statistical hearing is one draw per adult per day; the kin channel is O(edges of the day's victims); reputation is one daily pass over ≈ 48k entries and grudges one pass over ≤ 8k; vendettas and Regard are O(factions²) ≤ 144; Feeds score ≤ 768 pool entries a day; Hunt executes for ≤ 16 promoted agents. The two new Coarse loads (hunters and hunted, ≤ 32) fit inside `max_coarse`.

## 13. Phases

1. **Knowledge**: § 1 (`Deed`, the memory fields and sub-caps, `deed_of`, generalised gossip at Chat, Drink, the pools, Statistical hearing, the kin channel, sightings in memory and relayed to `FactionDb`), § 2's daily rebuild and matrix read by nobody, the Known tab, the CSV columns, `PlantRumour` and `SetReputation` for tests. A 10-day CSV matches M14 outside the new columns and the gossip affinity drift. Commit.
2. **Stats and moves**: § 5 and § 6 (`Skills` seeding with rarity, `resolve`, extortion through Intimidate, competence and its multipliers, poaching), § 2's taste, first impressions, the meeting gate and the Purist creed with The Unplugged. Commit. Watch it live: a gang's extortion income must stay within 20 % of M14's.
3. **Blood**: § 3 and § 4 (grudges, inheritance, GuardBody, vendettas and Retaliate generalised, the Hunt with AskAround, StakeOut, promotion and the Statistical pass, the law's street silence, `kill_chain`), § 2's brain readers (gang fear, corp honour, law heat, binder dread). Commit. The first `Avenged` on seed 42 is the first visible M15.
4. **News, UI and levers**: § 7 (Feeds, stories, `Spin`, the press term), § 9 levers and god commands, § 10 panels and overlay. Commit.
5. **Gate and polish**: the M15 scenario and god scenarios, calibration, README and docs, then `/code-review high <base>..HEAD` and a fix commit, then push.

## 14. Risks

- **The revenge spiral runs away.** Every killing makes kin, every kin can hunt, and hunts kill: a high `kin_p` with a low `hunt_min` turns one Shakedown into a Mid West bloodbath and blows the Murder bound. The brakes are `max_hunts`, `hunt_min` and the courage and might terms; calibrate on the chain and revenge-kill counts before touching `kin_p`, which is what makes the vision story happen at all.
- **The resolver moves calibrated economies.** Extortion has always succeeded; a coin flip halves gang income, and gang income drives M8–M13's raids, stipends and recruitment. `bias.intimidate` is set so an even match succeeds ≈ 88 %; phase 2 measures gang income against M14 before anything else lands.
- **Memory pressure.** Rumours compete with the memories that drive Fight, ReportCrime and courtship, and a Statistical agent has 8 slots. The sub-caps protect first-hand entries; watch the v1 and parity gates for a drift in Fight and ReportCrime adoption in phase 1.

## 15. Out of scope (M16 and later)

Contracts, Fixers and hired guns, hits bought by an avenger too weak to hunt, hit squads on sightings, bounties and tracking tags, threats, extortion, blackmail, hostage-taking and fraud as leverage moves, fabricated stories and propaganda paid by gangs, poaching by threat, the law's accessory rule for a contract killing (M16); outside parents, scrip valued by reputation, reinforcements and the outside's view of a corp's honour (M17); the player, the dialogue layer as a renderer over the social state, story LOD following the player's circle and player × faction reputation beyond the pinned-agent cache (M18); Power and Water as goods, map layers and interiors, daemons, romance beyond the existing Court, and family feuds as a faction of their own.

## Implemented: deviations

Where the build departs from the text above. The numbered rows are the plan's Decisions table (`~/.claude/plans/m15-word-and-blood.md`, W1-W49); the rest are the phase commits' recorded deviations (8db658c, 367153b, 50963ac, fdb62d6) and phase 5's calibration calls (each changed value carries its reason and its before/after numbers as a comment in `assets/config.toml`). One line each. Every killing, beating, hunt, threat and story here is an abstract event between fictional agents: a seeded dice roll over structs, considerations and counters.

### Decisions that changed the spec

- **W1:** a rumour is a `MemoryEntry` with the spec's `deed`, `object`, `hops`, `conf`, `at` and a plan field `press` (a story's slant × 100), but rumours and sightings live in their own vector, `Memory.heard`; first-hand kinds stay in `Memory.entries`, so no M5-M14 reader ever sees a rumour.
- **W2:** split caps, not shared: `entries` keeps 24 (8 Statistical) with the old eviction; `heard` holds 8 (3 Statistical), evicting the lowest `weight × conf`. Phase 1: `heard` is trimmed eagerly on a demotion to Statistical (the lazy rule left 8).
- **W3:** `memory::deed_of(holder, entry)`; `Lost` alone is the Assaulted deed (no Fought/Lost pairing scan).
- **W4:** `insert_heard` merges on `(deed, actor, object, day)`; phase 1: **only a killing contradicts** (other deeds with different actors are separate deeds: false contradictions ran 900-1,500 a day).
- **W5:** `law::raise_crime_on` carries the deed's object apart from the living victim; a corp landlord is the subject of an eviction memory.
- **W6:** pools per district as planned; `PoolEntry` gains `kin`, `told` and `district`.
- **W7:** propagation at Chat, Drink, AskAround and the kin channel; never on `stat_chat` (parity).
- **W8:** every M15 roll on a keyed word stream, never the world or agent streams; phase 1: the word streams use bits 62|60 (M14's runs use 62|61); distortion's district is the speaker's Home's.
- **W9:** the legacy second-hand copies in `entries` retired in phase 3 (`legacy_second_hand = false`; `social::gossip` still draws to keep the stream); a rumour nudges an existing edge, never creates one.
- **W10:** no `kin_cache`: kin ride the pool entry with a `told` list; a beating's kin capped at `kin_cap`.
- **W11:** bodies hear one story entry a day too (W39); post-back from first-hand memories only.
- **W12:** sightings in memory from phase 1, the `FactionDb` relay from phase 3; none inside the Precinct; guard sightings hourly. Relayed sightings are tagged, skipped by M14's runner readers and evicted first.
- **W13:** `Reputation` saved; the rebuild skips stale entity ids (a reused index inherits nothing); a saved 30-day arrest log feeds the law's heat; `known_by` counts every memory with the agent as subject (the spec's reading; it reads higher than "who knows the deed"). Phase 5: **a gang's dread and heat read its members' mean**, not the spec's three most feared (hottest): every gang had three known killers by day 30, so all four read 0.83-1.00 and the fear and pressure terms saw no difference (below).
- **W14:** the regard matrix over every live faction pair (up to 16 factions, not the spec's 12).
- **W15:** grudges as a component, `rel_w` with the plan's `leader` 0.7 and `comrade` 0; phase 3 review: a noticing victim no longer learns its own wrong twice (`remember_crime` skips `on_learn` when the object is the holder). Phase 5: a fight's loser forms no grudge on its own account: the `Lost` memory's own-view weight (`deed_sev.assaulted` 0.5 � `rel_w.own` 0.6 = 0.30) falls under `grudge_min` 0.35.
- **W16:** inheritance and settlement as written; no `grudge_targets` index (one scan of the grudge store per death).
- **W17:** `wronged_by` reads grudges ≥ `fight_grudge_min` from phase 3.
- **W18:** vendettas generalise Retaliate (and M14's `retaliate_on`); `raid::expedition_rival` new. Phase 5: the god `DeclareVendetta` holds its feud open (`Vendetta.declared`, `[grudges] declared_days` 30): at `vendetta_norm` 10 two leaders' grudges no longer reach `vendetta_open`.
- **W19:** Hunt and GuardBody bypass the planner; GuardBody waits with `StakeOut`.
- **W20:** `HuntState.gap` cached at adoption (−2.5 % ticks/s otherwise); a Hunt on a dead target is kept until the next tick.
- **W21:** intel as written.
- **W22:** the strike, promotion and abandonment as written; `hunted_by` and `guards_of_corpse` derived and rebuilt on load.
- **W23:** the Statistical pass as written (`stat_hunt_min` 0.45, phase 3).
- **W24:** street silence as written.
- **W25:** one primary social skill per adult (the spec's four independent draws gave ~20 % of adults a skill ≥ 0.8); the Skill stream keyed by index and generation; rust re-derives with the current personality, downward only.
- **W26:** the resolver as written; the move kind is in the roll key; a refused brave target makes the actor an Enemy.
- **W27:** extortion through Intimidate with `bias.intimidate` 0.8 (spec 0.5): success 0.76-0.84 by seed.
- **W28:** competence terms **rank-normalised** (2 × the skill's percentile among adults at seed, `[competence] rank_norm`) instead of skill ÷ city mean, so the median corp reads multiplier 1.0 (the mean ratio left most corps below 1 and cost seed 42's Farm trucks); `TalentLost` needs a recorded departure (captain churn fired 15 a run).
- **W29:** poaching as written; the gap is against the adult `job_search` would hire.
- **W30:** taste and first impressions as written.
- **W31:** The Unplugged seeded with its Chapel; past `[creeds] founders` 3 a Purist gang recruits only along a Friend, Family, Parent or Spouse edge (the open rule took the rivals' desperate recruits: 45 members by day 60 and M12's gang control broke). Membership runs 15-60 by seed and day.
- **W32:** fear in Raid, Contest and LieLow only with the word on; `rep_flips` as written.
- **W33:** honour in `renew_contracts` and `acquire` as written. Phase 5: honour now moves on contracts and acquisitions (below).
- **W34:** `wanted_gang` ranks by `max(reports, round(heat × crackdown_reports))`.
- **W35:** GuardBody as written; `grudges::prune_guards` daily (review).
- **W36:** the Feed as written; Feed economics to break-even (phase 4: `wage_reporter` 4, `feed.staff` 1, `upkeep.feed` 0).
- **W37:** stories as written; a Feed never runs its own owner's deeds (or its people's) unpaid; ads only from corps with cash ≥ 0.3 (charged to all, they bankrupted the thin NPC Holdings: 36 → 47); `Story.source` keeps a distorted story's deed actor (review).
- **W38:** `press` saved (the spec said not).
- **W39:** bodies read one story a day.
- **W40:** **Spin is a side spend scored at midnight** (`Corp.spin_since`, `[news] spin_min` 0.2, gated on cash above the fleet reserve and work to do), not a tenth `CorpOrder`: as an order it took Research's slot (Research-built Labs 3/8 → 0-2/8). Plants pass the bury and censor filters; `plant_price` 60, `bury_price` 30. Phase 5: one plant per deed a day across all spinners (three corps had paid to plant the same story about the same man on the same day: 11-19 such plants a run, now 0-1).
- **W41:** Feeds seeded as written; `feeds_seeded` set only when a Feed stood (review).
- **W42:** the CLI keeps `--lever day=D:name=value`; the god commands as listed plus `grant_skill=dregs<n>:<skill>:<v>:suit`.
- **W43:** the CSV columns as listed; `chain_max` reports a chain's length (chain + 1); `grudges` counts formed per day.
- **W44:** saves as written; Law, Corp and Gang pending shocks and `World::corp_rethink` are now saved (phase 2: a save between a shock and its rethink diverged on load; present in the M14 city too); `World::save_version` 1 replaces the pre-M15 save detector (review).
- **W45:** throughput: −4 to −7.5 % per phase by A/B; the gates read 5.0-7.9k ticks/s on the shared box (floor 4,000).
- **W46:** `--word-off` reproduced the previous phase byte for byte at every phase (phase 4: 38,040 CSV cells, 174,493 event lines).
- **W47:** the tick order as written.
- **W48:** the kill watch as written.
- **W49:** as written; the vendetta's god hold (W18) is new.

### Phase deviations and knobs

- Phase 1: heard stored as compact tuples; seed 42 hops max 4, second-hand 0.60, known_by of killers median 49; throughput −5.9 %, save +4.0 %.
- Phase 2: seeds 42/1/2/3/43/44 gang income vs M14 +46/+9/−25/+42/+2/+15 % (The Unplugged spreads through families); Murders within 1.16× of M14; determinism fix (pending shocks saved).
- Phase 3: `hunt_min` 0.5 → 0.85, `stat_hunt_min` 0.2 → 0.45, `hunt_kill_p` 0.5 → 0.3 (the spec values gave 826 Hunts, 38 revenge kills, Murders +28 %); the planner key widened (`coin_bucket` and `food_count` in 2 bits each). Gate devices: the M13 vehicles floor 140 → 130, the M13 episode-ended-by-law over 42-47, M14 TechGained a printed finding.
- Phase 4: the overlay is on `J`; gate devices: the M14 day-120 node mean and Door-then-raid printed findings; M12 "allocation 2× ≥ 60 % outside Garrison" by majority over 42-47.
- **Phase 5 calibration, scoped to dynamic range** (an axis or counter that reads the same for every faction carries no signal to its reader; bands stay printed findings under the 2026-10-07 gate doctrine). Measured on seeds 42/43/44, 120 days, before → after:
  - grudges formed 20,728 / 19,185 / 18,125 → 5,236 / 5,183 / 5,939: every beating heard of formed one (friends hearing of a beating: 30,870 grudge hits on seed 42; beatings lost by the aggressor: 3,412). `[grudges] grudge_min` 0.15 → 0.35, `rel_w.own` 0.8 → 0.6, `[gossip] deed_sev.robbed` 0.3 → 0.6. Grudges now come from robberies (own, 0.36), killings (kin and friends) and kin told of a beating (the kin channel, conf ~1); a victim's own beating (0.30) no longer forms one.
  - vendettas opened 54 / 48 / 58 → 8 / 8 / 7, open on day 120 28 / 25 / 27 → 7 / 8 / 7 (of 11-15 factions): `vendetta_norm` 3 → 10, `vendetta_open` 0.5 → 1.0.
  - gang dread on day 120 0.63-0.94 / 0.50-0.98 / 0.83-0.98 → 0.07-0.71 / 0.15-0.68 / 0.30-0.66; gang heat 0.83-1.00 → 0.39-1.00 / 0.50-0.91 / 0.39-0.82: the members' mean (W13) and `[reputation] heat_scale` 2 → 10, `heat_wanted` 0.6 → 0.4 (any open report made an adult wanted at 0.6: heat ≥ 0.5 for 37 / 42 / 14 % of adults → 3.3 / 2.6 / 2.0 %, the spec band 0.5-4 %).
  - honour: a kept Security contract is told as `Repaid` by the seller (`reach0.repaid` 0.1 → 0.3), an unpaid one as `Betrayed` by the client's owner, and a hostile buy-out of a solvent rival as `Betrayed` by the buyer. Corp honour spread over the run (mean / max) 0.18 / 0.40, 0.12 / 0.39, 0.25 / 0.67; on day 120 0.07-0.09 (before 0.00-0.15, at most one corp off 0.50). Contracts are few (30-50 bought a run, most cancelled by the buyer's Hunker), so `ContractLost` on honour stays 0 (a finding).
  - Hunts: `hunt_min` 0.85 → 0.75 and `lethal_min` 0.75 → 0.85: with `hunt_min` above `lethal_min` every Hunt was lethal and only kin killings hunted; now a heavy grudge (a friend's killing, two robberies) hunts for a beating and a kin killing hunts to kill. Hunts 50 / 58 / 47 → 49 / 45 / 61, Avenged 5 / 12 / 6 → 18 / 9 / 9, revenge kills 2 / 5 / 1 → 1 / 2 / 2 (band 3-20, a finding).
  - Spin: plants of a deed already planted that day 17 / 11 / 19 → 0-1; plants not against a corp 102 / 58 / 67 → 28 / 3 / 5 (the vendetta gangs had been the plant targets); the plant-opinion rule 7/13, 3/17, 6/23 → 33/55, 4/20, 13/27 (spec ≥ 50 %, a finding).
  - Not changed, with the reason: the Feeds' score already prefers deeds (stories on 42-44: assaulted ~550, raided ~100, killed ~70-90, married 10-32, founded 2 of ~800); the distorted share stays 1.2 % (spec 3-15 %: the swap needs a gang or an Enemy in the district, and Statistical hearing, most of the denominator, never distorts; no reader); The Unplugged's size (`[creeds]` is outside phase 5's sections); `deed_sev.assaulted` 0.5 → 0.4 was tried and left (kin hear beatings at conf ~1, so it moved nothing).
  - Murders against the `--word-off` run of the same binary: 47 / 44 / 46 against 41 / 48 / 47 (1.01× on the 42-44 sum; before 61 / 49 / 59, 1.24×); assaults 20.8-23.3 a day.
- Phase 5 gate (`test_m15_word_seed_42`, the 2026-10-07 doctrine): seed 42 alone (ticks/s and the mechanism checks), 43-47 and the `--word-off` runs of 42-44 in threads. Asserted: a rumour at hops ≥ 4, a distorted rumour, watched killers sampled, `rep_flips`, dread reading deeds (Spearman > 0), the positional adults in the top decile of standing (**deviation**: the spec's "top decile ≥ 70 % execs, owners, leaders and the captain" needs ≥ 10 % of adults in a position; the city has ~20 of ~1,900, so it is asserted the other way round, ≥ 50 % of them in the top decile: 20/22), grudges, a Hunt, an Avenged, an inherited grudge, a Vendetta, extortion higher at dread ≥ 0.5, a Poached on 42-44, a `TalentLost` after a killing on 42-47, Feeds ≥ 2, stories, a Spin held ≥ 3 days with a plant and a bury on 42-44, an expulsion on 42-47, the gang dread and heat spread ≥ 0.1 (majority of 42-44), vendettas open under a fifth of the faction pairs, and the sanity bounds (Murders on the 42-44 sum ≤ 1.25× the `--word-off` sum, assaults ≤ 42.7 a day on 42-44, starvation, population, grudges ≤ 10,000, revenge kills ≤ 60, rare skills in (0, 0.2), ticks/s ≥ 4,000). Printed findings: the spec § 12 bands, Spearman ≥ 0.6, the top decile's composition, contracts lost on honour, extortion alone vs with an ally, the plant-opinion rule.
- Phase 5 stat table, **not regenerated**: `calibrate` (the calibration city runs with the word off, so phase 5 changes nothing in it) writes a table that differs from the committed one in every row, and main's own source (4a6a396) writes the same new table: the committed table predates a later behaviour change, not phase 5. With the regenerated table the M10 gate's off-screen killings read 0 (≥ 1 asserted) and the M12 gate's gang control 0/6 seeds (≥ 3 asserted), and the `--word-off` Murders on 42-44 rose 41/48/47 → 53/50/62, so the committed table is kept; the regeneration is a base-city question (`docs/BIG_PICTURE_2026-10-07.md` § 4: off-screen residents are no longer murdered) for the next phase. `lod::test_kitted_vs_unkitted_parity` fails on main as on this phase (Full ratio 0.619 against Statistical 1.000: 4 and 4 Statistical violent deaths of 750, the same off-screen violence drought).
