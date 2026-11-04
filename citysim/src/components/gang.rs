use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gang {
    pub name: String,
    /// Sorted.
    pub members: Vec<EntityId>,
    pub treasury: i64,
    /// Homes held (claim count 3), sorted. The Hideout is `hideout`, not territory.
    pub territory: Vec<EntityId>,
    pub leader: Option<EntityId>,
    pub empty_since: Option<Tick>,
    /// This gang's Hideout building.
    #[serde(default = "EntityId::none")]
    pub hideout: EntityId,
    /// The city milestone C8 (`systems::vertical`): its Hideouts beyond the
    /// seat (`hideout`, which keeps the stash, the armoury and the node), one
    /// per district it held turf in when founded, ascending.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub district_hideouts: Vec<EntityId>,
    /// Standing order from the faction brain.
    #[serde(default)]
    pub order: Order,
    #[serde(default)]
    pub order_since: Tick,
    /// Every order's score from the last rescoring, best first. Not saved.
    #[serde(skip)]
    pub order_trace: Vec<OrderScore>,
    /// Scheduled muster departure while `order.is_raid()`.
    #[serde(default)]
    pub raid_at: Option<Tick>,
    /// Departure tick of the last raid this gang launched (cooldown).
    #[serde(default)]
    pub last_raid_tick: Option<Tick>,
    #[serde(default)]
    pub retaliate_until: Option<Tick>,
    /// The Hideout is unusable until this tick.
    #[serde(default)]
    pub sacked_until: Option<Tick>,
    /// Pending since the last rescoring; drained by the brain. M15 phase 2
    /// (determinism): saved, as `Law.shocks`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shocks: Vec<Shock>,
    /// `(tick, member)` arrested or killed, newest last, capped at 64.
    #[serde(default)]
    pub heat_log: VecDeque<(Tick, EntityId)>,
    /// M9: the leader at the moment of their arrest, while they sit inside.
    /// The acting leader leans toward breaking them out.
    #[serde(default)]
    pub boss: Option<EntityId>,
    /// M9: departure of the last breakout (its own cooldown).
    #[serde(default)]
    pub last_breakout_tick: Option<Tick>,
    /// M9: the captain has been paid or has refused; the gang makes no
    /// further offer until here.
    #[serde(default)]
    pub bribe_until: Option<Tick>,
    /// M9: a bribe was taken; no Crackdown against this gang until here.
    #[serde(default)]
    pub paid_until: Option<Tick>,
    /// M11 D49: who decides; `Dictator` = the leader. No Board in M11.
    #[serde(default)]
    pub governance: Governance,
    /// M12 D36: the old leader, when a decapitation (death or arrest) just
    /// handed the gang a new one; `gang::run` resolves the split check.
    #[serde(default)]
    pub split_check: Option<EntityId>,
    /// M12 D39: the corp building the standing Raid aims at; `None` = the
    /// rival Hideout (M8).
    #[serde(default)]
    pub raid_target: Option<EntityId>,
    /// Pressure pass round 8 X4 (`OPEN-241`): the place the standing raid
    /// musters at, chosen when it was ordered (`raid::pick_muster`); `None`
    /// = `raid::muster_point`'s rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raid_muster: Option<EntityId>,
    /// V8b (`OPEN-276`): when the standing raid's column left its muster
    /// (`raid::depart`; at or after `raid_at`, which a held muster moves to
    /// the departure). `None`, or before `raid_at`: not yet left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raid_left: Option<Tick>,
    /// The city milestone C9b: the rival's Hideout the standing gang raid
    /// lands on, chosen when it was ordered (`vertical::raid_pick`: the
    /// nearest to the raiders' own, or the seat); `None` = the seat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raid_hideout: Option<EntityId>,
    /// The city milestone C8: the standing raid's column climbing to its
    /// target's floor (`vertical::climb_column`), while it climbs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raid_climb: Option<crate::systems::vertical::RaidClimb>,
    /// M12 D40: the roster emptied after having members (a dead gang, not a
    /// fresh one); cleared by the next recruit.
    #[serde(default)]
    pub emptied: bool,
    /// M12 D40: the claims of an empty gang were cleared.
    #[serde(default)]
    pub claims_cleared: bool,
    /// M12 D36: the gang this one split from (lineage for the Hideout panel).
    #[serde(default)]
    pub split_from: Option<EntityId>,
    /// M13 D36: the Harvest target (`chrome::harvest_target`), cached at each
    /// rescore so a member's GangWork never rescans the chromed.
    #[serde(default)]
    pub harvest_target: Option<EntityId>,
    /// L2 (plan L37) god `FactionStrike`: the order pinned to Contest on this
    /// district until the tick (faction violence touches it off screen).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<(DistrictId, Tick)>,
    /// M14 V37 (phase 3 sets it): the member streaming a departed raid.
    #[serde(default)]
    pub stream_by: Option<EntityId>,
    /// M14 V30 (plan deviation): the last traced run on our nodes that named
    /// someone, `(by, tick)`: the VirtRaid `hacked` input and its wipe target
    /// outlive the pending `Shock::Hacked` the daily rescore consumes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hacked_by: Option<(EntityId, Tick)>,
    /// M14 V34: the gang a standing Retaliate fights, set when it was chosen
    /// on a pending `Shock::Hacked { by }` naming another gang; `None`: the
    /// rival (`World::rival_of`). See `raid::raid_rival`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retaliate_on: Option<EntityId>,
    /// M15 W31: a creed (The Unplugged are Purist).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creed: Option<crate::word::Creed>,
    // --- L2 phase 2 (plan L20): written only with `leisure::on`.
    /// The leader's Call: the Hideout is the members' top HangOut spot
    /// until this tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_until: Option<Tick>,
    /// Plan field: the order (by its `order_since`) the leader last Called for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub called_for: Option<Tick>,
    /// Plan field: the day of the leader's last Collect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collected_day: Option<u64>,
    /// Plan field: territory tribute since the last Collect (`daily_economy`).
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub tribute_week: i64,
    // --- M17 phase 4b: the succession.
    /// The civil war for the chair, while it runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub war: Option<CivilWar>,
    /// The war's heir while it runs, then its winner: the leader while a
    /// free member (over `leader_ranking`'s top); cleared once not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crowned: Option<EntityId>,
    // --- The pre-view pass phase 6 (P13-P15): written only with `[leaders]`.
    /// P15: the order the brain chose, not yet given to a lieutenant in
    /// person (`systems::leaders`); the members follow `order` till then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_order: Option<PendingOrder>,
    /// P13: the day the leader's last round was begun.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round_day: Option<u64>,
    /// P14: the day the leader last held court.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub court_day: Option<u64>,
}

/// The pre-view pass phase 6 (P15): an order chosen by the faction brain
/// (`faction::rescore`) and waiting to be given to a lieutenant in person.
/// Applied by `faction::apply_order` (the muster timed from then).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingOrder {
    pub order: Order,
    /// When it was chosen (the wait's cap counts from here).
    pub decided: Tick,
    /// M12 D39: the corp building a corp Raid aims at.
    pub corp_target: Option<EntityId>,
    /// M14 V34, M15 W18: the faction a Retaliate fights.
    pub retaliate_on: Option<EntityId>,
    /// The rescore's note (`daily` or `shock`, the scores).
    pub why: String,
}

/// M17 phase 4b: a gang's civil war for the chair after a close succession.
/// `heir` (the ranking's top) holds the chair while it runs; `challenger`
/// (the runner-up) contests it. The sides are the free members who took
/// each claimant's part (the claimants included), fixed at the start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CivilWar {
    pub heir: EntityId,
    pub challenger: EntityId,
    /// The leader whose fall opened it.
    pub fallen: Option<EntityId>,
    pub since: Tick,
    pub until: Tick,
    pub heir_side: Vec<EntityId>,
    pub challenger_side: Vec<EntityId>,
}

impl Gang {
    pub fn new(name: String, hideout: EntityId, treasury: i64) -> Gang {
        Gang {
            name,
            members: Vec::new(),
            treasury,
            territory: Vec::new(),
            leader: None,
            empty_since: Some(0),
            hideout,
            district_hideouts: Vec::new(),
            order: Order::Expand,
            order_since: 0,
            order_trace: Vec::new(),
            raid_at: None,
            last_raid_tick: None,
            retaliate_until: None,
            sacked_until: None,
            shocks: Vec::new(),
            heat_log: VecDeque::new(),
            boss: None,
            last_breakout_tick: None,
            bribe_until: None,
            paid_until: None,
            governance: Governance::Dictator,
            split_check: None,
            raid_target: None,
            raid_muster: None,
            raid_left: None,
            raid_hideout: None,
            raid_climb: None,
            emptied: false,
            claims_cleared: false,
            split_from: None,
            harvest_target: None,
            strike: None,
            stream_by: None,
            hacked_by: None,
            retaliate_on: None,
            creed: None,
            call_until: None,
            called_for: None,
            collected_day: None,
            tribute_week: 0,
            war: None,
            crowned: None,
            pending_order: None,
            round_day: None,
            court_day: None,
        }
    }

    pub fn is_sacked(&self, now: Tick) -> bool {
        self.sacked_until.is_some_and(|t| t > now)
    }
}

/// Stock lives on the Market building's `Building.stock_food`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Market {
    /// `1..=30`.
    pub price_food: i64,
    /// Cap 120, one per day.
    pub price_history: VecDeque<i64>,
    /// M11 D16: units sold today, and the last seven days' sales and
    /// opening stock (newest last), rolled in `economy::daily_price`.
    #[serde(default)]
    pub sales_today: u32,
    #[serde(default)]
    pub sales: VecDeque<u32>,
    #[serde(default)]
    pub stock_hist: VecDeque<u32>,
    /// M11 phase 5: the price in tenths of a coin (`[economy] price_tenths`);
    /// 0 reads as `price_food x 10` (an old save, the flag off).
    #[serde(default)]
    pub price_tenths: i64,
    /// M13 D6/D40: legal Stims doses sold today (phase 4).
    #[serde(default)]
    pub stim_sales_today: u32,
    /// Hunger as sickness § 1: synth meals sold out of the shelf unit last
    /// opened (`0..synth_stretch`).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub synth_frac: u32,
}

impl Market {
    /// The price in tenths of a coin.
    pub fn tenths(&self) -> i64 {
        if self.price_tenths > 0 {
            self.price_tenths
        } else {
            self.price_food * 10
        }
    }

    pub fn new(price_food: i64) -> Market {
        Market {
            price_food,
            price_history: VecDeque::new(),
            sales_today: 0,
            sales: VecDeque::new(),
            stock_hist: VecDeque::new(),
            price_tenths: 0,
            stim_sales_today: 0,
            synth_frac: 0,
        }
    }
}

/// May go negative; wages unpaid while `< 0`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Treasury {
    pub coins: i64,
}

/// Stored in `World::crime_reports`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrimeReport {
    pub crime: Crime,
    pub suspect: EntityId,
    /// `None` for a Statistical-tick theft.
    pub witness: Option<EntityId>,
    pub tick: Tick,
    pub resolved: bool,
}
