use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Brain {
    /// Default Coarse.
    pub lod: Lod,
    pub current_goal: Option<GoalKind>,
    pub goal_since: Tick,
    pub plan: Option<Plan>,
    pub plan_step: u8,
    pub cooldowns: BTreeMap<GoalKind, Tick>,
    pub pinned: bool,
    pub betraying: bool,
    /// Top-5 goals and consideration outputs, for the inspector.
    pub last_think: Option<ThinkTrace>,
    pub last_think_tick: Tick,
    /// Coarse: when the current action resolves.
    pub action_until: Tick,
    /// Execution state of the current plan step.
    #[serde(default)]
    pub exec: ExecState,
    /// Day on which the dole was last collected.
    #[serde(default)]
    pub last_dole_day: Option<u64>,
    /// Last think triggered by a plan ending or failing (one per 30 ticks).
    #[serde(default)]
    pub last_urgent_think_tick: Tick,
    /// The last plan failure: a second consecutive failure of the same goal
    /// cools it; the first is simply replanned.
    #[serde(default)]
    pub last_plan_failure: Option<(GoalKind, Tick)>,
    /// Round 3 S2 (OPEN-222): by goal, the cools in a row (no plan of it
    /// finished between) and the tick the last one ends: each backs off
    /// longer (`[brain] cool_backoff`).
    #[serde(default)]
    pub cool_streak: BTreeMap<GoalKind, (u8, Tick)>,
    /// Round 9 Y1 (`OPEN-249`): the food sources (a bin's building, a
    /// kitchen, a Market, a Home) a starving body's Eat failed at, each
    /// passed over until its tick while it tries the next.
    #[serde(default)]
    pub food_tried: Vec<(EntityId, Tick)>,
    /// Waiting in `World::plan_queue`.
    #[serde(default)]
    pub plan_queued: bool,
    /// Cuffed by this guard: no thinking or acting until jailed or freed.
    #[serde(default)]
    pub cuffed_by: Option<EntityId>,
    /// A guard escorting this cuffed suspect to the Jail.
    #[serde(default)]
    pub escorting: Option<EntityId>,
    /// Guards: the current patrol loop (buildings) and legs completed this shift.
    #[serde(default)]
    pub patrol_route: Vec<EntityId>,
    #[serde(default)]
    pub patrol_legs: u8,
    /// The shift key the patrol route belongs to.
    #[serde(default)]
    pub patrol_shift_key: Option<i64>,
    /// The pre-view pass phase 4 (P8): a sweeper's round of street stops this
    /// shift (`jobs::sweep_plan`; its legs are `patrol_legs`, its shift
    /// `patrol_shift_key`). Written only while set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sweep_route: Vec<TilePos>,
    /// Gang: the day the extortion task was done, and coins taken today (for SplitLoot).
    #[serde(default)]
    pub gang_task_day: Option<u64>,
    #[serde(default)]
    pub loot_today: i64,
    /// `(day, any edge with affinity >= 0.3 to an unmarried agent)`: the Court
    /// gate, refreshed daily because the neighbour scan is O(degree).
    #[serde(default)]
    pub court_candidate: Option<(u64, bool)>,
    /// The corpse being carried to the Cemetery; its Position follows.
    #[serde(default)]
    pub carrying_corpse: Option<EntityId>,
    /// Walking to the map edge to leave the city; no goals, no plans.
    #[serde(default)]
    pub emigrating: bool,
    /// The day the spouse intimacy bonus was last granted (once a night).
    #[serde(default)]
    pub last_spouse_night: Option<u64>,
    /// Gang: the order the current GangWork plan serves; `None` while freelancing.
    #[serde(default)]
    pub following_order: Option<Order>,
    /// Gang: the day a `Disobeyed` event was last logged for this member.
    #[serde(default)]
    pub disobeyed_day: Option<u64>,
    /// M10: the last day the agent was not Statistical at an hourly
    /// assignment (the trace's `STATISTICAL_ALL_DAY` is `body_day != today`).
    #[serde(default)]
    pub body_day: Option<u64>,
    /// M11 D26: the day of the last `Register` (the Found cooldown).
    #[serde(default)]
    pub last_found_day: Option<u64>,
    /// Real economy E28: the day of the last gift to a Mission (one a day:
    /// the give test is keyed on the day, so a second Socialise would give
    /// again).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_gave_day: Option<u64>,
    /// M13 D29: what a Shop plan is buying, fixed when the seller is bound.
    #[serde(default)]
    pub shop_pick: Option<ShopPick>,
    /// M13 D36: dragged off by this gang member (for the UI and the law).
    #[serde(default)]
    pub abducted_by: Option<EntityId>,
    /// M14 V12: dazed after a lost contest until this tick: no Flee, the
    /// wait is not interrupted, an arrest is not contested.
    #[serde(default)]
    pub dazed_until: Option<Tick>,
    /// M15 W26: a failed Intimidate's backlash: `(actor, bonus, until)`,
    /// the Fight goal's flat against that actor until the tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fight_bonus: Option<(EntityId, f32, Tick)>,
    /// L1: a guard's re-chases of a suspect seen moving, this plan.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub chase_hops: u8,
    /// L1: the day an exec last kept office hours at the HQ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub office_day: Option<u64>,
    /// L1: `(employer, until)`: quit there; not rehired there before the tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quit_from: Option<(EntityId, Tick)>,
    /// L1: the day a Statistical agent last decided on the dole (with or
    /// without a visit; `last_dole_day` is the last collection).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stat_dole_day: Option<u64>,
    /// L2 (L29): dry Scavenge hours in a row (saved: a streak survives a load).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub scavenge_dry: u8,
    /// Hunger fix G4: dry Begs in a row (saved, as `scavenge_dry`).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub beg_dry: u8,
    /// L2 phase 5 (gang desistance): the day a gang stipend or tribute last
    /// reached this member (written only with `[living]` on).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gang_paid_day: Option<u64>,
    /// L2 shadow fixes item 7: when the tier last changed (written only
    /// with the fixes on).
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub lod_since: Tick,
    /// L2 shadow fixes item 6: a tile fled from and until when the spot
    /// pick avoids it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avoid_spot: Option<(TilePos, Tick)>,
    /// Lives at work phase 2 (§ 3.3): spots left empty (the tile and the
    /// tick), the last four; `best_spot` scores them × `empty_penalty` for
    /// `empty_memory_hours`. Not written while empty (the off save).
    #[serde(default, skip_serializing_if = "smallvec::SmallVec::is_empty")]
    pub empty_spots: smallvec::SmallVec<[(TilePos, Tick); 4]>,
    /// Workplace room: the building that last refused this agent at a full
    /// door, and when (`World::local` passes it over for `[exec]
    /// refused_ticks`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused: Option<(EntityId, Tick)>,
    /// The pre-view pass phase 2 (OPEN-137): this shift's shape state
    /// (`systems/shift.rs`). Not written while `None` (the off save).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift: Option<Box<ShiftState>>,
    /// M18 P1 (the seam): a human drives this body. Without autopilot its
    /// own `think` and the plan queue skip it (`systems/think.rs` `turn`,
    /// `systems/plan.rs`); the human's commands write its plan
    /// (`PlayerCommand::{Steps, Goal, Stop}`) and exec runs it like any
    /// other. Not written while `None` (an unpossessed save is unchanged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player: Option<PlayerControlled>,
}

/// M18 P1: the body a human drives (`Brain.player`). Never on anyone under
/// 18 (`PlayerCommand::Possess` refuses a child).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerControlled {
    /// The body's own brain thinks and plans for it (the NPC's day) while set.
    pub autopilot: bool,
    /// The last plan failure or refusal the human should read (the tick and
    /// the line), for the HUD.
    pub last_failure: Option<(Tick, String)>,
    /// M18 P2: the menu row the running plan came from (its label), for the
    /// HUD; cleared by a goal, a walk or Stop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doing: Option<String>,
    /// M18 W: the shift key the human sent the body to work ("Go to work"):
    /// each of the shift's plans that ends is followed by the next (the
    /// commute, the blocks, the break, the second half) to the shift's
    /// close, as an NPC's think would (`player::shift_next`); cleared by any
    /// other command, Stop, or the shift's end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift: Option<i64>,
    /// M18 P3: the contract record the human set the body to work (taken at
    /// a Fixer's desk, or taken and being worked): each of its plans that
    /// ends is followed by the next (`player::job_next`), as the Contract
    /// goal's would; cleared by any other command or the record's close.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<u64>,
    /// M18 P3: what the city offers the body instead of writing it (a cover
    /// shift, a gang's draw): the human takes it or lets it lapse.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offers: Vec<CityOffer>,
}

/// M18 P3: an offer the city makes a body the human drives where it would
/// write an NPC (`care::covers`' pick, `underwater`'s front draw): the same
/// code decides who is offered; taking it runs the NPC's write.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CityOffer {
    /// The tick it was made (its key in the menu).
    pub made: Tick,
    /// It lapses at this tick.
    pub until: Tick,
    pub what: OfferKind,
    /// The menu row's line.
    pub line: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OfferKind {
    /// A sick worker's post for the day (`care::covers`): the employer, the
    /// sick body, the role, the cover's end.
    Cover { employer: EntityId, sick: EntityId, role: Role, until: Tick },
    /// A gang's front draws its staff (`underwater::fronts_daily`).
    Gang { gang: EntityId, front: EntityId },
    /// M18 P5: a witness wants coins to keep the player's deed quiet
    /// (`consequence::blackmailed`): pay, refuse, or go for him.
    Blackmail { witness: EntityId, demand: i64 },
}

/// The pre-view pass phase 2 (docs/PREVIEW_PASS.md § 1): one shift's
/// stations, drawn when its first block is planned (`shift::plan`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShiftState {
    /// The shift key the state belongs to (`Job::next_shift_key`).
    pub key: i64,
    /// The spots inside the workplace, in the order they are worked.
    pub spots: Vec<TilePos>,
    /// The current shape plan's blocks in minutes, one per work step in
    /// plan order; 0 runs to the shift's end (the last block, paid).
    pub blocks: Vec<u16>,
    /// `started_tick` of the shape plan `blocks` belongs to.
    pub plan_tick: Tick,
    /// 0 the first half, 1 the break planned, 2 the second half.
    pub stage: u8,
    /// Blocks drawn so far (the block draw's ordinal; the next spot).
    pub drawn: u8,
    /// The tick the first block was planned: the span the output and a
    /// pro-rata cut are read over (one block's start before).
    pub from: Tick,
    /// The longest block worked this shift, in minutes (the device row).
    pub longest: u16,
    /// The spot indices worked this shift (a bit each, the device row).
    pub worked: u8,
    /// The break was a meal (the device row).
    pub meal: bool,
    /// The pre-view pass phase 3 (`haul`): the shift's destinations, turned
    /// through trip by trip (empty: the trips run inside the post, from the
    /// stock tile to `spots[1..]`). Not written while empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dests: Vec<crate::entity::EntityId>,
    /// The pre-view pass phase 3 (`haul`): trips unloaded this shift.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub trips: u16,
}

pub(super) fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

impl Default for Brain {
    fn default() -> Self {
        Brain {
            lod: Lod::Coarse,
            current_goal: None,
            goal_since: 0,
            plan: None,
            plan_step: 0,
            cooldowns: BTreeMap::new(),
            pinned: false,
            betraying: false,
            last_think: None,
            last_think_tick: 0,
            action_until: 0,
            exec: ExecState::Idle,
            last_dole_day: None,
            last_urgent_think_tick: 0,
            last_plan_failure: None,
            cool_streak: BTreeMap::new(),
            food_tried: Vec::new(),
            plan_queued: false,
            cuffed_by: None,
            escorting: None,
            patrol_route: Vec::new(),
            patrol_legs: 0,
            patrol_shift_key: None,
            sweep_route: Vec::new(),
            gang_task_day: None,
            loot_today: 0,
            court_candidate: None,
            carrying_corpse: None,
            emigrating: false,
            last_spouse_night: None,
            following_order: None,
            disobeyed_day: None,
            body_day: None,
            last_found_day: None,
            last_gave_day: None,
            shop_pick: None,
            abducted_by: None,
            dazed_until: None,
            fight_bonus: None,
            chase_hops: 0,
            office_day: None,
            quit_from: None,
            stat_dole_day: None,
            scavenge_dry: 0,
            beg_dry: 0,
            gang_paid_day: None,
            lod_since: 0,
            avoid_spot: None,
            empty_spots: smallvec::SmallVec::new(),
            refused: None,
            shift: None,
            player: None,
        }
    }
}

impl Brain {
    /// The plan step currently being executed.
    pub fn current_step(&self) -> Option<&ActionInstance> {
        self.plan.as_ref().and_then(|p| p.steps.get(usize::from(self.plan_step)))
    }

    /// Drop the plan and reset execution.
    pub fn clear_plan(&mut self) {
        self.plan = None;
        self.plan_step = 0;
        self.exec = ExecState::Idle;
        self.following_order = None;
        self.chase_hops = 0;
    }

    /// The current plan's goal, if any.
    pub fn plan_goal(&self) -> Option<GoalKind> {
        self.plan.as_ref().map(|p| p.goal)
    }

    /// M18 P1: a human drives the body and its own brain is off (no think,
    /// no plan queue).
    pub fn possessed(&self) -> bool {
        self.player.as_ref().is_some_and(|p| !p.autopilot)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub kind: MemoryKind,
    pub subject: Option<EntityId>,
    pub tick: Tick,
    /// `0.0..=1.0` magnitude.
    pub salience: f32,
    /// `-1.0..=1.0` sign of mood impact.
    pub valence: f32,
    /// Heard rather than seen.
    pub second_hand: bool,
    /// For SawCrime: which crime, so a report names it after salience has decayed.
    #[serde(default)]
    pub crime: Option<Crime>,
    /// M15 W1: a Rumour's (or Threatened's) deed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deed: Option<crate::word::Deed>,
    /// M15 W1/W5: the deed's object (the victim; a corp for Struck), on a
    /// Rumour and on a first-hand `SawCrime` or `Stripped`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<EntityId>,
    /// M15 W1: 0 seen, 1 told by a witness, 2+ rumour.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub hops: u8,
    /// M15 W1: `0..=1`.
    #[serde(default = "one_f32", skip_serializing_if = "is_one_f32")]
    pub conf: f32,
    /// M15 W12: a Sighting's building.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<EntityId>,
    /// M15 W38: a story's `slant × 100` (0 = not from a Feed).
    #[serde(default, skip_serializing_if = "is_zero_i8")]
    pub press: i8,
}

fn is_zero_i8(v: &i8) -> bool {
    *v == 0
}

pub(super) fn is_one_f32(v: &f32) -> bool {
    *v == 1.0
}

impl MemoryEntry {
    /// An entry of `kind` at `tick` with every other field at its default
    /// (no subject, salience 0, first-hand, conf 1): the base for struct
    /// update syntax.
    pub fn blank(kind: MemoryKind, tick: Tick) -> MemoryEntry {
        MemoryEntry {
            kind,
            subject: None,
            tick,
            salience: 0.0,
            valence: 0.0,
            second_hand: false,
            crime: None,
            deed: None,
            object: None,
            hops: 0,
            conf: 1.0,
            at: None,
            press: 0,
        }
    }
}

/// Cap 24; evict lowest `salience * recency`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Memory {
    pub entries: Vec<MemoryEntry>,
    /// M15 W1/W2: rumours and sightings, apart from the first-hand
    /// `entries` (which no talk can evict), capped at `[gossip] rumour_cap`
    /// (`rumour_cap_statistical` off screen).
    #[serde(default, skip_serializing_if = "Vec::is_empty", with = "heard_wire")]
    pub heard: Vec<MemoryEntry>,
}

/// M15 save size: the heard store on disk as compact tuples `(kind,
/// subject, tick, salience, valence, second_hand, deed, object, hops, conf,
/// at, press)` with ids as `(index, generation)` (a heard entry never
/// carries a crime). At ~7k entries in a 2,000 city the named-field form was
/// ~185 bytes an entry, 1.4 MB of a 29 MB save.
mod heard_wire {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{MemoryEntry, MemoryKind};
    use crate::entity::EntityId;
    use crate::time::Tick;
    use crate::word::Deed;

    type Id = Option<(u32, u32)>;

    #[derive(Serialize, Deserialize)]
    struct Wire(MemoryKind, Id, Tick, f32, f32, bool, Option<Deed>, Id, u8, f32, Id, i8);

    fn to(id: Option<EntityId>) -> Id {
        id.map(|e| (e.index, e.generation))
    }

    fn from(id: Id) -> Option<EntityId> {
        id.map(|(index, generation)| EntityId { index, generation })
    }

    pub fn serialize<S: Serializer>(v: &[MemoryEntry], s: S) -> Result<S::Ok, S::Error> {
        let wire: Vec<Wire> = v
            .iter()
            .map(|e| {
                Wire(
                    e.kind,
                    to(e.subject),
                    e.tick,
                    e.salience,
                    e.valence,
                    e.second_hand,
                    e.deed,
                    to(e.object),
                    e.hops,
                    e.conf,
                    to(e.at),
                    e.press,
                )
            })
            .collect();
        wire.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<MemoryEntry>, D::Error> {
        let wire = Vec::<Wire>::deserialize(d)?;
        Ok(wire
            .into_iter()
            .map(|Wire(kind, subject, tick, salience, valence, second_hand, deed, object, hops, conf, at, press)| {
                MemoryEntry {
                    subject: from(subject),
                    salience,
                    valence,
                    second_hand,
                    deed,
                    object: from(object),
                    hops,
                    conf,
                    at: from(at),
                    press,
                    ..MemoryEntry::blank(kind, tick)
                }
            })
            .collect())
    }
}
