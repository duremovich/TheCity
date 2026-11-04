use super::*;

/// `[verbs]` (the pressure pass round 2, R6b; OPEN-217, OPEN-207).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VerbsCfg {
    /// Corps without an Hq that build one a day (`industry::grow_hq`; the
    /// base built one a day city-wide); per capita in `scaled_to`.
    pub hq_builds_per_day: u32,
    /// A corp holding only Homes (and Lots) gives its exec a lettings desk in
    /// its Block nearest his Home (`life::office_of`, OPEN-207).
    pub landlord_desk: bool,
    /// Stories a reporter files a night from the deeds he interviewed on his
    /// beat (`news::run_stories`, ahead of the Feed's ranked picks).
    pub beat_stories: u8,
    /// The gangs' gun trade (`weapons::gang_arms_trade`): a gang short of its
    /// armoury buys a lot from a gang with a surplus at this share of the
    /// dealer's price a weapon ...
    pub trade_price_share: f32,
    /// ... at most this many weapons a deal ...
    pub trade_lot: u8,
    /// ... keeping this many days of its stipends after paying ...
    pub trade_reserve_days: u8,
    /// ... one deal a gang every this many days.
    pub trade_days: u8,
    /// Round 3 S5 (OPEN-217): a cab driver's body takes this share of each
    /// fare he drives, in hand when the rider pays (`Flow::FareCut` from the
    /// Garage's owner, who took the fare); above 0 the cut is his pay and
    /// the shift's wage is not paid (`customers::fare_scale`: the wage
    /// scaled by the fares paid 0c and wrote "Paid" on a fare-less shift).
    pub cab_fare_cut: f32,
    /// ... standing by with no fare, he waits at a rank: the door of a
    /// building of `cab_rank_kinds` within this many tiles of his Garage
    /// (a hash of the driver and the wait), not a Rest inside the Garage ...
    pub cab_rank_tiles: u32,
    /// ... this many ticks at one rank, then he cruises to the next.
    pub cab_rank_ticks: u64,
    pub cab_rank_kinds: Vec<crate::components::BuildingKind>,
    /// A client served by a tipped worker at a venue a bouncer keeps (the
    /// customer ledger: a dancer's StripClub or Club) turns rough on this x
    /// (1 - lawfulness) (a hash of the pair and the night): the bouncer on
    /// the door throws him out and bars him from the venue, as the madam's
    /// rough booking (`[trade_verbs] rough_p`) ...
    pub door_rough_p: f32,
    /// ... for this many days; the barred is turned away at the door
    /// ("barred"), the bouncer's refusal.
    pub door_ban_days: u64,
    /// A Hit, Beat or Abduct taken on a corp's exec reaches him through his
    /// security (`missions::exec_guards`) at once; on anyone else it reaches
    /// him on this (a hash of the contract): the street's tip ...
    pub hit_heard_p: f32,
    /// ... his safety falls by this (a `Threatened` memory, the event in
    /// his diary and his guards').
    pub hit_heard_safety: f32,
    /// Round 8 X2 (OPEN-247): the word acted on (`contracts::react`). A
    /// target at this `Needs.wealth` or more whose purse holds the quote
    /// hires a guard on himself (a Guard record, brokered at his nearest
    /// Fixer): V16 `ceo_20314`, a 1,453c hit taken, slept on and idled ...
    pub hit_guard_wealth: f32,
    /// ... one at least this brave and at most this lawful swears on the
    /// taker's lead (a `Hired` grudge of `hit_counter_w`, which the Hunt
    /// and the hire read: a counter-hit when he is weak and can pay) ...
    pub hit_counter_courage: f32,
    pub hit_counter_lawful: f32,
    pub hit_counter_w: f32,
    /// ... and one who does neither lies low at home this many days (his
    /// shifts struck and counted as absences, his free hours indoors). 0 off.
    pub hit_low_days: u32,
    /// Round 4 T4 (OPEN-217): the door's arrivals, refusals and rough
    /// clients go to the bouncer standing the door: a body among the present
    /// door staff first (the hash of the visitor and the hour among them),
    /// else the hash among all (an off-screen colleague counted present by
    /// the clock took every let-in as the lowest id: V12's 100k bouncers);
    /// and a visit goes to a body of `night_rows` on the floor with room
    /// first (`service::pick_server`).
    pub door_body_first: bool,
    /// Round 4 T4: the `[[serves]]` keys whose body tells its night at the
    /// shift's end (served, tips, regulars, the room) and whose venue's
    /// off-screen visits are drawn into its shift (`leisure::queue_at_counter`,
    /// as a counter body's: a Club with a body Host took its whole evening
    /// in one 21:00 batch).
    pub night_rows: Vec<String>,
    /// ... the keys among them whose stations blocks are sets on the stage
    /// (one line a set: the room, the tips, the tippers).
    pub set_rows: Vec<String>,
    /// ... names a night's, a set's or a shape-up's line gives (the rest "and N more").
    pub night_names: u8,
    /// Round 4 T4: a Yard's foreman with a body walks to its gate in the dawn
    /// window (the Pitch goal) and the shape-up is his morning: one line a
    /// Yard (who was picked, who was sent home), the picks' Hire lines no
    /// longer his (174 a week), a line for each body sent home.
    pub shape_up_gate: bool,
    /// ... he is at the gate this many minutes before `[jobs2] corner_hour`
    /// (he sets out, or wakes, by the travel from where he is).
    pub shape_up_lead_min: u16,
    /// Round 4 T7 (OPEN-233): a face turned away at a door and back at it
    /// inside this many ticks is a return (`service::refused`,
    /// `vice::turned_again`): T4's 100k door turned one armed man away nine
    /// times in four nights. 0 off.
    pub door_return_ticks: u64,
    /// ... a return is barred from the house this many days (today counted;
    /// `vice::bar`, which `leisure::door_seen` passes over).
    pub door_return_ban_days: u64,
    /// ... and the scene a return makes (drunk, high, a brawler, carrying)
    /// swings on `[customers] door_swing_p` times this.
    pub door_return_swing_mult: f32,
    /// Round 5 U6 (OPEN-217): a body already inside his workplace waits
    /// there for a shift at most this many ticks off (the routine's base
    /// rule waited 600: V13's ripperdoc came in at 04:00 to collect a wage
    /// and rested at the post five hours before her 09:00 shift). A day
    /// post's hire still waits the base rule's.
    pub wait_at_work_ticks: u64,
}

impl Default for VerbsCfg {
    fn default() -> Self {
        VerbsCfg {
            hq_builds_per_day: 1,
            landlord_desk: true,
            beat_stories: 1,
            trade_price_share: 0.75,
            trade_lot: 4,
            trade_reserve_days: 1,
            trade_days: 3,
            cab_fare_cut: 0.5,
            cab_rank_tiles: 60,
            cab_rank_ticks: 120,
            cab_rank_kinds: vec![
                crate::components::BuildingKind::Home,
                crate::components::BuildingKind::Market,
                crate::components::BuildingKind::Bar,
                crate::components::BuildingKind::Hotel,
            ],
            door_rough_p: 0.03,
            door_ban_days: 14,
            hit_heard_p: 0.5,
            hit_heard_safety: 0.3,
            hit_guard_wealth: 1.0,
            hit_counter_courage: 0.6,
            hit_counter_lawful: 0.4,
            hit_counter_w: 0.6,
            hit_low_days: 3,
            door_body_first: true,
            night_rows: vec!["dancer".to_string(), "Host".to_string()],
            set_rows: vec!["dancer".to_string()],
            night_names: 3,
            shape_up_gate: true,
            shape_up_lead_min: 20,
            door_return_ticks: 2880,
            door_return_ban_days: 2,
            door_return_swing_mult: 2.0,
            wait_at_work_ticks: 120,
        }
    }
}

/// `[search]` (the pressure pass round 4, T5; OPEN-217, OPEN-228's follow-up).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchCfg {
    /// A jobless adult searches each day for this many days after his last
    /// post (`Jobs2State.idle_since`); after, he is the job search's
    /// candidate and the shape-up's (broke) as before.
    pub search_days: u64,
    /// From this many days without a post he takes worse work: the dawn
    /// shape-up whatever his purse (while he searches), and the midnight job
    /// search hires him to a post past `[distance.choice] hire_max_ticks` up
    /// to `far_ticks` before the Ambient pool takes it.
    pub worse_after_days: u64,
    pub far_ticks: u64,
    /// A body's search hours (ticks of the day, from and to).
    pub hours_from: u16,
    pub hours_to: u16,
    /// A hiring board (the door of an open post he can take) within this
    /// travel of where he stands; a lead a Friend gives within `lead_ticks`.
    pub reach_ticks: u64,
    pub lead_ticks: u64,
    /// With no open post within reach, the nearest of these kinds within
    /// `reach_ticks` is the hiring board: it names the nearest open post he
    /// can take within `lead_ticks` of it (the next door), or nothing.
    pub board_kinds: Vec<crate::components::BuildingKind>,
    /// Minutes asking at a door; doors asked a day.
    pub ask_ticks: u16,
    pub tries: u8,
    /// Friends and kin asked a day (the closest by affinity); names the
    /// line gives of them (the rest "and N more").
    pub asks: u8,
    pub friend_names: u8,
    /// A door is no searcher's next while this many searchers a post open
    /// there are making for it already.
    pub crowd: u8,
    /// The dawn's hand starts work this many minutes after `[jobs2]
    /// corner_hour`, to the day shift's end: V12's hand rested three hours
    /// at the Yard for a 09:00 shift.
    pub day_start_min: u16,
    /// Round 8 X2 (OPEN-247): past `search_days` a jobless adult under this
    /// `Needs.wealth` keeps searching every day (V16 `day_labourer_11600`,
    /// 0c and no post, read no board all week: the search stopped at day 7).
    pub need_wealth: f32,
    /// ... and a quit or a firing leaves one under `need_wealth` out of every
    /// hire this many days, not `[misery]`'s idle days (V16 `cabbie_80901`,
    /// fired on d22 with the rent short, sat out the week); the stain stays.
    pub need_idle_days: u32,
}

/// `[grief]` (the pressure pass round 8, X2; OPEN-247).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GriefCfg {
    /// An adult of the dead's household (the spouse, a parent or a grown
    /// child under his roof) mourns from `mourn_days_min` days at affinity 0
    /// to `mourn_days_max` at 1 (V16 `school_parent_87579`: widowed at
    /// 21:00, at work at 06:09 the next morning) ...
    pub mourn_days_min: u32,
    pub mourn_days_max: u32,
    /// ... each shift inside it struck (no work, no wage); the employer's
    /// leave is this many shifts, the rest absences on `[misery]`'s count
    /// (the firing at `fire_absences`). A household business gives any leave.
    pub leave_days: u32,
    /// The wake: the tick of the day after the death the household holds it
    /// at the Home, the dead's Friends in reach there ...
    pub wake_tick: u16,
    /// ... a Friend whose own Home is within this many tiles walks round (the
    /// wake's line names the ones who came) ...
    pub wake_reach_tiles: u32,
    /// ... names the line gives (the rest "and N more").
    pub wake_names: u8,
    /// A mourner's Hunt on the one who killed his kin scores + this for
    /// `hot_days` after the death (the grudge is hot; V16's widow's Hunt
    /// scored 0.097 and lost to her shift the next morning).
    pub hunt_hot_flat: f32,
    pub hot_days: u32,
    /// Nobody proposes to a widow(er), nor a widow(er) to anyone, inside
    /// this many days of the death (20k: remarried 22 hours after).
    pub wed_after_days: u32,
}

impl Default for GriefCfg {
    fn default() -> Self {
        GriefCfg {
            mourn_days_min: 2,
            mourn_days_max: 5,
            leave_days: 3,
            wake_tick: 1080,
            wake_reach_tiles: 60,
            wake_names: 4,
            hunt_hot_flat: 0.3,
            hot_days: 7,
            wed_after_days: 14,
        }
    }
}

impl Default for SearchCfg {
    fn default() -> Self {
        SearchCfg {
            search_days: 7,
            worse_after_days: 2,
            far_ticks: 360,
            hours_from: 420,
            hours_to: 960,
            reach_ticks: 120,
            lead_ticks: 240,
            board_kinds: vec![
                crate::components::BuildingKind::Hall,
                crate::components::BuildingKind::Market,
                crate::components::BuildingKind::ConstructionYard,
                crate::components::BuildingKind::FreightYard,
            ],
            ask_ticks: 15,
            tries: 3,
            asks: 8,
            friend_names: 3,
            crowd: 2,
            day_start_min: 30,
            need_wealth: 0.5,
            need_idle_days: 1,
        }
    }
}

/// `[noise]` (the pressure pass round 2, R4; OPEN-218, OPEN-219).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NoiseCfg {
    /// Names a contested claim's line gives of the holders who saw the blow
    /// and now count the actor an enemy (`gang::contested`; the rest "and N more").
    pub contest_names: u8,
    /// Names the super's one notice line a round gives (`service::notices_folded`).
    pub notice_names: u8,
    /// The last hour a Camp answer moves to when the parent's shift covers
    /// the `[household] visit_hour_from..visit_hour_to` hash (`household::visits_due`).
    pub camp_visit_last_hour: u8,
    /// Round 3 S7 (OPEN-225): names a raid's result line gives of its dead,
    /// its hurt and the freed (`crew::raid_result`; the rest "and N more").
    pub raid_names: u8,
    /// Round 3 S7: a civilian who saw the same actor's same petty crime
    /// within this many days takes no new record of it (`law::raise_crime_on`;
    /// 0 off).
    pub petty_rewitness_days: u16,
    /// Round 7 W6 (OPEN-217): a shadow diary gives the word about its agent
    /// (GOSSIP ABOUT ME) at most this many rumours a day a role (the doer,
    /// the object), each its own folded line, and only a rumour reaching new
    /// ears opens one; the rest fold into one line a day a role (0: no cap).
    pub gossip_lines_day: u8,
}

impl Default for NoiseCfg {
    fn default() -> Self {
        NoiseCfg {
            contest_names: 3,
            notice_names: 3,
            camp_visit_last_hour: 22,
            raid_names: 4,
            petty_rewitness_days: 7,
            gossip_lines_day: 0,
        }
    }
}

/// `[rest]` (the perf pass phase 1, OPEN-195): a Rest is cut only by a work
/// signal the Work goal will answer. No keys: the fix has no number.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RestCfg {}

/// `[mood]`: the hourly mood target's config-read terms (the older terms
/// read their own sections: `[litter] mood`, `[fashion] status_w`, ...).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MoodCfg {
    /// The island pass phase 3c (OPEN-177): `[mood.housing]`, the Home's
    /// term (`mood::housing_bias`). `None`: no term.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub housing: Option<MoodHousingCfg>,
}

impl MoodCfg {
    pub fn is_empty(&self) -> bool {
        self.housing.is_none()
    }
}

/// Jobs & lives phase 6 `[faith]` (docs/JOBS_AND_LIVES.md § 8; Dylan's
/// answer 6): creeds as data (`creeds` rows; no creed name in code), the
/// churches (a Mission with a creed), the cults (a gang with a creed), the
/// daily Preach, conversion, the followers' spread along social edges,
/// tithes, and apostasy hunted through the grudges. Live only under
/// `[jobs2] enabled`. Every number is a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FaithCfg {
    pub enabled: bool,
    pub creeds: Vec<CreedCfg>,
    /// The daily Preach: a district resident attends on `p_attend` (a
    /// follower, or one under `attend_belonging` or grieving within
    /// `grief_days`), gaining `attend_gain` belonging; a non-follower
    /// attendee under `convert_belonging` or grieving converts on the row's
    /// `p_convert`.
    pub p_attend: f32,
    pub attend_belonging: f32,
    pub attend_gain: f32,
    pub convert_belonging: f32,
    pub grief_days: u64,
    /// A follower's Friend, Family or Spouse converts on the row's
    /// `p_spread` a day (the creed spreads through people).
    pub spread_affinity: f32,
}

impl Default for FaithCfg {
    fn default() -> Self {
        FaithCfg {
            enabled: false,
            creeds: Vec::new(),
            p_attend: 0.3,
            attend_belonging: 0.5,
            attend_gain: 0.2,
            convert_belonging: 0.4,
            grief_days: 7,
            spread_affinity: 0.3,
        }
    }
}

/// One `[[faith.creeds]]` row (D14): a church (a Mission with a creed) or a
/// cult (a gang with a creed), seeded per copy in its zone's district at
/// `share` of the way along it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CreedCfg {
    pub id: String,
    pub label: String,
    pub church: bool,
    pub cult: bool,
    pub zone: crate::components::Zone,
    pub share: f32,
    /// Chrome tolerated (`Kit.visible`), as the Purists'.
    pub tolerance: u8,
    /// Of each wage, into the church's purse or the cult's treasury (`Flow::Tithe`).
    pub tithe_frac: f32,
    /// A cult recruits only along a social edge to a member (past `founders`).
    pub recruit_edges: bool,
    pub founders: usize,
    /// A leaver's grudge from the leader (0: none); `hunt` reads it at `hunt_min`.
    pub apostasy_w: f32,
    /// The flat a cult adds to its Contest and Raid orders.
    pub violence_bias: f32,
    /// A cult member's daily walk-away chance × this (`gang::desist`).
    pub desist_mult: f32,
    pub p_convert: f32,
    pub p_spread: f32,
}

impl Default for CreedCfg {
    fn default() -> Self {
        CreedCfg {
            id: String::new(),
            label: String::new(),
            church: false,
            cult: false,
            zone: crate::components::Zone::Sump,
            share: 0.5,
            tolerance: 0,
            tithe_frac: 0.1,
            recruit_edges: true,
            founders: 3,
            apostasy_w: 0.0,
            violence_bias: 0.0,
            desist_mult: 1.0,
            p_convert: 0.1,
            p_spread: 0.01,
        }
    }
}

/// Jobs & lives phase 4 `[fashion]` (docs/JOBS_AND_LIVES.md § 9, Dylan's
/// answer 1): clothing as an item kind of the owned-item store, the
/// Boutique (a venue on the leisure ladder: grooming and clothes), the
/// `status` need, and the dress readers. Live only under `[jobs2] enabled`.
/// Every number is a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FashionCfg {
    pub enabled: bool,
    /// `[corps]` rows that own the seeded Boutiques, one per copy each.
    pub boutique_rows: Vec<usize>,
    /// A Boutique visit's grooming price by district tier (Sump, Mid, Spire).
    pub groom_price: [i64; 3],
    /// The fun and the `status` a Boutique visit gives; the `status` a
    /// High-rung evening gives.
    pub gain_boutique: f32,
    /// The ladder weighs a Boutique `1 + status_pull × (1 − status)`.
    pub status_pull: f32,
    pub status_gain: f32,
    pub status_high: f32,
    /// `status` decays `status_decay × (0.5 + pride) × class_w[class]` a
    /// day (Corp, Street, Dreg).
    pub status_decay: f32,
    pub class_w: [f32; 3],
    /// Mood's `status_w × (status − 0.5)`.
    pub status_w: f32,
    /// Clothing items alive at most, per 1,000 residents (R4).
    pub clothing_per_1000: f32,
    /// A dress-0 carrier in the Spire is noticed at a carry sighting × this.
    pub spire_dress_mult: f32,
    /// A fight's winner (no guard) takes the loser's worn clothing on this
    /// chance (under the petty cap, `[weapons] petty_per_1000_day`).
    pub p_mug_clothes: f32,
    /// A Boutique sale leaves this many meals' coins beyond the evening's
    /// reserve (clothes after food).
    pub clothes_reserve_meals: i64,
}

impl Default for FashionCfg {
    fn default() -> Self {
        FashionCfg {
            enabled: false,
            boutique_rows: vec![5],
            groom_price: [3, 6, 12],
            gain_boutique: 0.2,
            status_pull: 4.0,
            status_gain: 0.35,
            status_high: 0.15,
            status_decay: 0.01,
            class_w: [1.5, 1.0, 0.5],
            status_w: 0.1,
            clothing_per_1000: 200.0,
            spire_dress_mult: 1.5,
            p_mug_clothes: 0.3,
            clothes_reserve_meals: 10,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SocialCfg {
    pub edge_create_ticks: u32,
    pub affinity_per_hour: f32,
    pub friend_threshold: f32,
    pub rival_threshold: f32,
    pub enemy_threshold: f32,
    pub propose_affinity: f32,
    pub propose_trust: f32,
    pub gang_stipend: i64,
    pub extort_amount: i64,
    pub join_gang_affinity: f32,
    pub join_gang_desperation_hunger: f32,
    pub join_gang_desperation_lawfulness: f32,
    /// The pressure pass phase 4 (OPEN-210): an Acquaintance this weak
    /// (`|affinity|` below it) is pruned after `weak_prune_days` without
    /// contact.
    pub weak_affinity: f32,
    /// Days without contact before a weak Acquaintance is pruned.
    pub weak_prune_days: u64,
    /// The most Acquaintances one person keeps; past it, the daily pass
    /// drops the weakest (`|affinity|`), then the stalest. Friends, kin,
    /// spouses, Rivals, Enemies and an edge holding a debt are never
    /// dropped or counted.
    pub acquaintance_cap: usize,
    /// Round 8 X5 (`OPEN-248`): a proposal ends a courtship of at least
    /// this many dates (accepted flirts, one a day) ...
    #[serde(default)]
    pub court_dates: u16,
    /// ... the first of them at least the pair's pace ago, from this many
    /// days ...
    #[serde(default)]
    pub court_days_min: u64,
    /// ... to this many (a hash of the pair, `social::court_pace_days`).
    #[serde(default)]
    pub court_days_max: u64,
    /// A courtship with no date for this many days is over.
    #[serde(default)]
    pub court_lapse_days: u64,
    /// The share of the founding's unmarried adult housemate pairs courting
    /// already when the story starts (`social::seed_courting`; 0: none).
    #[serde(default)]
    pub court_seed_share: f32,
}

/// M12 D12: flat terms added to each district stance's product.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StanceFlatCfg {
    pub patrol: f32,
    pub crackdown: f32,
    pub sweep: f32,
}

impl Default for StanceFlatCfg {
    fn default() -> Self {
        StanceFlatCfg { patrol: 0.2, crackdown: 0.0, sweep: 0.0 }
    }
}

/// Flat terms added to each posture's product of considerations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostureFlatCfg {
    pub patrol: f32,
    pub crackdown: f32,
    pub garrison: f32,
}

/// M15 § 1: the word.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GossipCfg {
    pub gossip_min: f32,
    pub max_hops: u8,
    pub hop_salience: f32,
    pub distort_base: f32,
    pub pool_cap: usize,
    pub pool_decay: f32,
    pub leak_min: f32,
    pub leak_frac: f32,
    pub reach_min: f32,
    pub hear_p: f32,
    pub pool_conf: f32,
    pub kin_p: [f32; 4],
    pub rumour_cap: usize,
    pub rumour_cap_statistical: usize,
    pub relay_conf: f32,
    pub reach0: DeedTable,
    pub deed_sal: DeedTable,
    pub deed_sev: DeedTable,
    /// W10: kin captured at death.
    pub kin_cap: usize,
}

impl Default for GossipCfg {
    fn default() -> Self {
        GossipCfg {
            gossip_min: 0.3,
            max_hops: 6,
            hop_salience: 0.7,
            distort_base: 0.15,
            pool_cap: 64,
            pool_decay: 0.8,
            leak_min: 0.4,
            leak_frac: 0.4,
            reach_min: 0.05,
            hear_p: 0.5,
            pool_conf: 0.6,
            kin_p: [0.9, 0.7, 0.5, 0.3],
            rumour_cap: 8,
            rumour_cap_statistical: 3,
            relay_conf: 0.8,
            // M16b § 4: `blackmailed` reach0 0.2, sal 0.5, sev 0.4.
            reach0: DeedTable {
                blackmailed: 0.2,
                ..DeedTable::of([1.0, 0.5, 0.3, 0.2, 0.5, 0.6, 0.3, 0.4, 0.8, 0.9, 0.4, 1.0, 0.6, 0.1, 0.3, 0.3, 0.2])
            },
            deed_sal: DeedTable {
                blackmailed: 0.5,
                ..DeedTable::of([0.9, 0.6, 0.5, 0.4, 0.6, 0.5, 0.3, 0.5, 0.6, 0.8, 0.4, 0.9, 0.7, 0.3, 0.4, 0.7, 0.3])
            },
            deed_sev: DeedTable {
                blackmailed: 0.4,
                ..DeedTable::of([1.0, 0.5, 0.3, 0.2, 0.6, 0.0, 0.0, 0.3, 0.0, 0.6, 0.0, 0.8, 0.6, 0.0, 0.1, 0.7, 0.0])
            },
            kin_cap: 12,
        }
    }
}

/// `[attention]` (M16b phase 4; the addendum's table). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AttentionCfg {
    pub enabled: bool,
    pub alert_floor: f32,
    pub linger_days: u32,
    pub raid: f32,
    pub riot: f32,
    pub loud_run: f32,
    pub strike: f32,
    pub acquisition: f32,
    pub vendetta: f32,
    /// Plan key (deviation): the flat tilt `brain_tilt × (1 − alertness ÷
    /// pool)` a distracted brain adds to Hunker, Garrison and LieLow (a
    /// consideration at no distraction would move every rescore).
    pub brain_tilt: f32,
}

impl Default for AttentionCfg {
    fn default() -> Self {
        AttentionCfg {
            enabled: true,
            alert_floor: 0.2,
            linger_days: 2,
            raid: 0.5,
            riot: 0.4,
            loud_run: 0.3,
            strike: 0.2,
            acquisition: 0.15,
            vendetta: 0.15,
            brain_tilt: 0.2,
        }
    }
}
