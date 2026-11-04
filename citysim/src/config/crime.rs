use super::*;

/// The pressure pass phase 1 `[misery]` (docs/PRESSURE_PASS.md § Phase 1,
/// OPEN-178; `systems/misery.rs`). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MiseryCfg {
    /// Misery is `clamp((mood_line − mood) / mood_span, 0, 1)`.
    pub mood_line: f32,
    pub mood_span: f32,
    /// Absenteeism: a day's skip chance at misery 1, `× (1 − skip_lawful_damp
    /// × lawfulness) × (1 − skip_loyal_damp × loyalty)`.
    pub skip_p: f32,
    pub skip_lawful_damp: f32,
    pub skip_loyal_damp: f32,
    /// Absences counted over this many days; at `fire_absences` the worker is dismissed.
    pub absence_window_days: u32,
    pub fire_absences: u32,
    /// Quitting: consecutive midnights below `quit_mood` while employed.
    pub quit_mood: f32,
    pub quit_days: u16,
    /// Less `quit_commute_days` past this commute (ticks, home door to work door).
    pub quit_commute_ticks: u64,
    pub quit_commute_days: u16,
    /// Less `quit_pay_days` for a day's wage under this many meals.
    pub quit_pay_meals: f32,
    pub quit_pay_days: u16,
    /// Less `quit_bed_days` in a company bed.
    pub quit_bed_days: u16,
    pub quit_min_days: u16,
    /// A day's quit chance once the days are reached.
    pub quit_p: f32,
    /// Petty crime: the planner's steal costs `− steal_cost_cut × misery`;
    /// the vehicle theft's lawfulness gate `+ steal_lawful_cut × misery`.
    pub steal_cost_cut: f32,
    pub steal_lawful_cut: f32,
    /// Round 8 X3 (`OPEN-247`): a wage-holder plans a vehicle theft for
    /// Earn only broke (`life::broke`) or this miserable (any misery at 0);
    /// the jobless as before.
    #[serde(default)]
    pub employed_steal_misery: f32,
    /// Off screen: the hour's `p_steal × (1 + stat_steal_mult × misery)`.
    pub stat_steal_mult: f32,
    /// Fights: misery past `brawl_min`, drunk or high, an hour's chance at
    /// misery 1 `brawl_p × (1 − brawl_lawful_damp × lawfulness)`.
    pub brawl_min: f32,
    pub brawl_p: f32,
    pub brawl_lawful_damp: f32,
    /// A drink (a `Socialised` memory) within this many hours is drunk.
    pub drink_hours: u32,
    /// The street brawl's reach (tiles).
    pub brawl_reach: u32,
    /// The brawl's death chance multiplier (`FightMods.kill_mult`).
    pub brawl_kill_mult: f32,
    /// The struck's grudge on the starter; `× brawl_grudge_back` the starter's on a struck winner.
    pub brawl_grudge_w: f32,
    pub brawl_grudge_back: f32,
    /// Off screen: a day's brawl at misery 1 (`× (1 − brawl_lawful_damp ×
    /// lawfulness)`), a report filed on `stat_brawl_report_p`.
    pub stat_brawl_p: f32,
    pub stat_brawl_report_p: f32,
    /// Recruitment: JoinGang `+ join_flat × misery`; the edge's affinity
    /// `− join_affinity_cut × misery`; past `join_min` misery and under
    /// `join_lawfulness` taken off the street; the join's event names misery
    /// past `join_note_min`; off screen `stat_join_p × misery` a day.
    pub join_flat: f32,
    pub join_affinity_cut: f32,
    pub join_min: f32,
    pub join_lawfulness: f32,
    pub join_note_min: f32,
    pub stat_join_p: f32,
    /// The faith: a Preach draws past `faith_min` misery; `p_convert × (1 +
    /// faith_convert_mult × misery)`.
    pub faith_min: f32,
    pub faith_convert_mult: f32,
    /// F3 (quits, wages and the jobless): a body's skip is acted out at the
    /// first hour within `skip_lead_ticks` of the shift's start (it sets off
    /// or wakes, then does not go); the Unwind goal scores `+ skip_unwind_flat`
    /// until the skipped shift's end (a bar, a friend's, the street).
    pub skip_lead_ticks: u16,
    pub skip_unwind_flat: f32,
    /// F3: out of every hire for `quit_idle_days` after a quit (misery, unpaid)
    /// and `fired_idle_days` after a firing, `+` up to `idle_jitter_days`
    /// (a pure hash); the firing employer refuses him for `fired_block_days`.
    pub quit_idle_days: u32,
    pub fired_idle_days: u32,
    pub idle_jitter_days: u32,
    pub fired_block_days: u32,
    /// F3: the reputation: for `stain_days` a hire pass passes the fired over
    /// on `stain_fired_p`, a quitter on `stain_quit_p` (a hash of the day).
    pub stain_days: u32,
    pub stain_fired_p: f32,
    pub stain_quit_p: f32,
    /// F3: an unpaid quitter's `Betrayed` grudge on a boss who is a person
    /// (the wages owed stay an edge debt, `social::repay_debts`).
    pub stiff_grudge: f32,
    /// F3: a gang recruit's initiation: no walk-away (`gang::desist`) in its
    /// first `recruit_tenure_days` (every recruit).
    pub recruit_tenure_days: u32,
}

/// The pressure pass phase 6 `[underwater]` (docs/PRESSURE_PASS.md § Phase 6;
/// `systems/underwater.rs`): a person-owned business whose till cannot meet
/// its wages becomes a story with choices (cut back, borrow, sell, close),
/// and a worker walked out unpaid comes back for his wages. Absent: none of
/// it. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnderwaterCfg {
    /// Under water: the owner's purse under `short_cover_days` of the day's
    /// wage bill (the staff outside his household), or one of them owed
    /// `short_unpaid_days` days.
    pub short_cover_days: f32,
    pub short_unpaid_days: u8,
    /// The owner decides after `decide_days` short midnights in a row, and
    /// again no sooner than `decide_cooldown_days` later.
    pub decide_days: u16,
    pub decide_cooldown_days: u16,
    /// The till back over `recover_days` of the bill: afloat (the freeze lifts).
    pub recover_days: f32,
    /// The choice weights (`x the owner's personality`, see the module):
    /// hang on, shorter hours, lay staff off, borrow, sell, close.
    pub w_hold: f32,
    pub w_hours: f32,
    pub w_layoff: f32,
    pub w_borrow: f32,
    pub w_sell: f32,
    pub w_close: f32,
    /// Each earlier decision multiplies sell and close by `1 + escalate x n`.
    pub escalate: f32,
    /// Shorter hours: each shift and its wage cut by this share.
    pub hours_cut: f32,
    /// A layoff lets this share of the staff go (at least one), newest first.
    pub layoff_share: f32,
    /// A loan: `loan_days` of the wage bill, repaid over `loan_weeks` weekly
    /// installments with the lender's rate on top.
    pub loan_days: u32,
    pub loan_weeks: u32,
    /// A lender lends what it can spare up to `loan_days` of the bill, never
    /// under `loan_min_days` of it.
    pub loan_min_days: u32,
    pub rate_shark: f32,
    pub rate_gang: f32,
    pub rate_corp: f32,
    /// A loan shark: a free adult under `shark_lawful_max` holding
    /// `shark_wealth_mult x` the principal (the owner's district first).
    pub shark_lawful_max: f32,
    pub shark_wealth_mult: f32,
    /// A corp lends only to an owner at least `corp_lawful_min` lawful, from
    /// its treasury over its reserve by `corp_spare_mult x` the principal.
    pub corp_lawful_min: f32,
    pub corp_spare_mult: f32,
    /// A gang lends or buys from its treasury over `gang_reserve`.
    pub gang_reserve: i64,
    /// Missed installments: the gang's or the shark's collector comes; at
    /// `seize_misses` the lender takes the business; a corp repossesses at
    /// `corp_repo_misses`. The collector's beating kills on `x collector_kill_mult`.
    pub seize_misses: u8,
    pub corp_repo_misses: u8,
    pub collector_kill_mult: f32,
    /// A fire sale at `sell_price_mult x` the building's value; an owner over
    /// `front_lawful_max` lawfulness sells to no gang unless past `front_misery`.
    pub sell_price_mult: f32,
    /// A gang buys a front at `front_price_mult x` the value (muscle, not money).
    pub front_price_mult: f32,
    pub front_lawful_max: f32,
    pub front_misery: f32,
    /// A bought front: its staff join the gang on `front_join_p x (1 -
    /// lawfulness)` a day; `launder_share` of the gang's week's tribute is
    /// booked through its till (its weekday); a rival holding turf in its
    /// district hits it on `rival_hit_p` a week, taking up to `rival_take`
    /// from the gang and shutting it `rival_close_days`.
    pub front_join_p: f32,
    /// M18 P3: a front's staff drawn who is a body a human drives is
    /// offered the gang instead of enlisted; the offer stands this long.
    pub front_offer_hours: u64,
    pub launder_share: f32,
    pub rival_hit_p: f32,
    pub rival_take: i64,
    pub rival_close_days: u32,
    /// A shut business stands for sale at `shut_price_mult x` its value to a
    /// would-be founder holding `buyer_reserve_days` of its wage bill on top;
    /// `shut_buys_per_day` a day; a Bar or Hotel unsold `shut_derelict_days`
    /// goes derelict.
    pub shut_price_mult: f32,
    pub buyer_reserve_days: u32,
    pub shut_buys_per_day: u32,
    pub shut_derelict_days: u32,
    /// The stiffed worker (an unpaid walkout or a layoff owed wages): from
    /// `confront_wait_days` after, on `confront_p x (0.5 + pride + misery)` a
    /// day, he goes to the owner in person (a Visit between the hours) at
    /// most `confront_max` times.
    pub confront_wait_days: u32,
    pub confront_p: f32,
    pub confront_max: u8,
    pub confront_hour_from: u8,
    pub confront_hour_to: u8,
    /// The Visit goal's urgency for the confrontation and the collector's call.
    pub confront_urgency: f32,
    pub dun_urgency: f32,
    /// The owner pays (what he holds, up to the owed) on `pay_base + pay_lawful
    /// x lawfulness + pay_loyal x loyalty - pay_greed x greed`.
    pub pay_base: f32,
    pub pay_lawful: f32,
    pub pay_loyal: f32,
    pub pay_greed: f32,
    /// Refused: the worker's grudge grows by `confront_grudge`; at
    /// `fight_grudge` he swings (`x confront_kill_mult`); under it, a worker
    /// at least `report_lawful` reports the owner to the Hall, which takes
    /// the owed out of the owner's purse as it comes in.
    pub confront_grudge: f32,
    pub fight_grudge: f32,
    pub confront_kill_mult: f32,
    pub report_lawful: f32,
    /// The beaten owner reports the collector (or the worker) at this lawfulness.
    pub victim_report_lawful: f32,
}

impl Default for UnderwaterCfg {
    fn default() -> Self {
        UnderwaterCfg {
            short_cover_days: 1.0,
            short_unpaid_days: 2,
            decide_days: 2,
            decide_cooldown_days: 5,
            recover_days: 3.0,
            w_hold: 0.5,
            w_hours: 1.0,
            w_layoff: 1.0,
            w_borrow: 1.0,
            w_sell: 0.6,
            w_close: 0.4,
            escalate: 0.6,
            hours_cut: 0.35,
            layoff_share: 0.5,
            loan_days: 14,
            loan_weeks: 4,
            loan_min_days: 3,
            rate_shark: 0.6,
            rate_gang: 0.4,
            rate_corp: 0.15,
            shark_lawful_max: 0.35,
            shark_wealth_mult: 2.0,
            corp_lawful_min: 0.4,
            corp_spare_mult: 3.0,
            gang_reserve: 20,
            seize_misses: 3,
            corp_repo_misses: 2,
            collector_kill_mult: 0.2,
            sell_price_mult: 0.6,
            front_price_mult: 0.25,
            front_lawful_max: 0.5,
            front_misery: 0.6,
            front_join_p: 0.1,
            front_offer_hours: 24,
            launder_share: 0.3,
            rival_hit_p: 0.15,
            rival_take: 30,
            rival_close_days: 2,
            shut_price_mult: 0.5,
            buyer_reserve_days: 7,
            shut_buys_per_day: 2,
            shut_derelict_days: 21,
            confront_wait_days: 1,
            confront_p: 0.3,
            confront_max: 3,
            confront_hour_from: 10,
            confront_hour_to: 20,
            confront_urgency: 0.8,
            dun_urgency: 0.9,
            pay_base: 0.2,
            pay_lawful: 0.5,
            pay_loyal: 0.3,
            pay_greed: 0.4,
            confront_grudge: 0.2,
            fight_grudge: 0.5,
            confront_kill_mult: 0.2,
            report_lawful: 0.5,
            victim_report_lawful: 0.4,
        }
    }
}

/// The pressure pass phase 5a `[consequence]` (docs/PRESSURE_PASS.md § Phase 5;
/// `systems/consequence.rs`): what happens next. A witness reacts (reports,
/// flees, gossips or blackmails), an expired hit is reposted higher or the
/// buyer goes himself or lets it go, jail has cellmates, contacts, fights and
/// a release into a changed life, and a creed holds a weekly service its
/// followers walk to. Absent: none of it. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConsequenceCfg {
    /// A witness's reaction (one weighted draw, a hash of the witness and the
    /// minute): report (`w_report x lawfulness`: today's SawCrime memory and
    /// the walk to the Hall), flee (`w_flee x (1 - courage)`, a violent
    /// crime only), gossip (`w_gossip x sociability`, a crime that is a deed)
    /// or blackmail (`w_blackmail x (1 - lawfulness) x (0.5 + greed)`, gated below).
    pub w_report: f32,
    pub w_flee: f32,
    pub w_gossip: f32,
    pub w_blackmail: f32,
    /// A fleeing witness's edge to the culprit falls by this (an Enemy: the Flee goal's "hostile near").
    pub flee_affinity: f32,
    /// A gossiping witness tells this many of his closest people at once (hops 1).
    pub gossip_tell: u8,
    /// The culprit sees who saw him on this chance (an edge: `hitmarket::knows_witness`).
    pub learn_p: f32,
    /// Blackmail's gate: the witness at most this lawful and holding at most this many coins.
    pub bm_lawful_max: f32,
    pub bm_broke_coins: i64,
    /// The first demand comes after this many days, in person, between these hours (a `Visit`).
    pub bm_wait_days: u16,
    pub bm_hour_from: u8,
    pub bm_hour_to: u8,
    pub bm_urgency: f32,
    /// The demand: this share of the culprit's purse, within these bounds.
    pub bm_demand_share: f32,
    pub bm_demand_min: i64,
    pub bm_demand_max: i64,
    /// The culprit pays on `bm_pay_base + bm_pay_fear x (1 - courage) + bm_pay_lawful x
    /// lawfulness - bm_pay_pride x pride` (the lawful have more to lose).
    pub bm_pay_base: f32,
    pub bm_pay_fear: f32,
    pub bm_pay_lawful: f32,
    pub bm_pay_pride: f32,
    /// Refused, the culprit silences the witness on `bm_silence_w x (1 - lawfulness) x (0.5 +
    /// courage)` (a beating at `bm_kill_mult`), else the witness goes to the law.
    pub bm_silence_w: f32,
    pub bm_kill_mult: f32,
    /// A paid demand comes back after this many days, at most this many times;
    /// a demand not made (the visit failed) is tried at most `bm_tries` times.
    pub bm_interval_days: u16,
    pub bm_max_demands: u8,
    pub bm_tries: u8,
    /// Open blackmails at most (the city's).
    pub bm_max_open: usize,
    /// An expired (or failed) hit or beating a person bought: repost
    /// (`hit_w_repost x (0.5 + grudge)`, at `hit_repost_mult` the old price,
    /// at most `hit_repost_max` times a head), go himself (`hit_w_self x
    /// courage x grudge`, the grudge at least `hit_self_min`), or let it go
    /// (`hit_w_giveup x (1.5 - grudge)`: the grudge x `hit_giveup_decay`).
    pub hit_w_repost: f32,
    pub hit_repost_mult: f32,
    pub hit_repost_max: u8,
    pub hit_w_self: f32,
    pub hit_self_min: f32,
    pub hit_w_giveup: f32,
    pub hit_giveup_decay: f32,
    /// A sentenced prisoner's Betrayed grudge on the person who reported him.
    pub snitch_grudge: f32,
    /// The cells, a day per prisoner: a gang member cellmate's offer taken on `cell_recruit_p
    /// x (1 - lawfulness) x (0.5 + misery)`; a Fixer's regular cellmate's name on
    /// `cell_contact_p` (a regular there too); a fight with a cellmate he dislikes (affinity
    /// under `cell_fight_affinity`, a grudge or a rival's colours) on `cell_fight_p x (0.5 +
    /// pride)` at `cell_kill_mult`; a broke prisoner (under `cell_loan_broke`) borrows
    /// `cell_loan_coins` from a cellmate with twice that on `cell_loan_p` (an edge debt).
    pub cell_recruit_p: f32,
    pub cell_contact_p: f32,
    pub cell_fight_p: f32,
    pub cell_fight_affinity: f32,
    pub cell_kill_mult: f32,
    pub cell_loan_p: f32,
    pub cell_loan_coins: i64,
    pub cell_loan_broke: i64,
    /// At most this many cell events a jail a day.
    pub cell_events_max: u16,
    /// Round 8 X6 (`OPEN-217`): the cells' day. At this hour the prisoners
    /// walk the yard: the day's one thing (a fight, an offer, a name, a
    /// loan) happens there, else he walks with the cellmate he likes best
    /// (their affinity up by `cell_yard_affinity`) or keeps to himself, a
    /// line either way; a cellmate in a fight is in no second one that day.
    /// 24 or more: the midnight pass, no yard line (as before).
    pub cell_yard_hour: u8,
    pub cell_yard_affinity: f32,
    /// The service: a creed's place (a cult's or the Purists' chapel, a church's Mission)
    /// holds one a week, its day and hour hashed on the place within `svc_hour_from..to`;
    /// the congregation is called `svc_lead_hours` before (a `Visit`), at most `svc_cap`.
    pub svc_hour_from: u8,
    pub svc_hour_to: u8,
    pub svc_lead_hours: u8,
    pub svc_cap: usize,
    /// A congregant whose home is past this many travel ticks from the place is not called
    /// (the preacher always is).
    pub svc_reach_ticks: u64,
    pub svc_urgency: f32,
    /// A congregant brings an unchurched friend on this chance.
    pub svc_guest_p: f32,
    /// The plate: this share of an attendee's purse, at most `svc_tithe_max`.
    pub svc_tithe_share: f32,
    pub svc_tithe_max: i64,
    /// Belonging an attendance gives.
    pub svc_gain: f32,
    /// A guest converts on `svc_convert_p x (0.5 + misery + (1 - belonging))`.
    pub svc_convert_p: f32,
    /// A congregant walks out on `svc_walkout_p x (1.5 - loyalty) x (0.5 + misery)`.
    pub svc_walkout_p: f32,
    /// A Purist (or a creed row without `apostasy_w`) leaver's grudge from the preacher.
    pub svc_heretic_w: f32,
}

impl Default for ConsequenceCfg {
    fn default() -> Self {
        ConsequenceCfg {
            w_report: 1.0,
            w_flee: 0.6,
            w_gossip: 0.8,
            w_blackmail: 1.2,
            flee_affinity: -0.4,
            gossip_tell: 3,
            learn_p: 0.3,
            bm_lawful_max: 0.35,
            bm_broke_coins: 20,
            bm_wait_days: 1,
            bm_hour_from: 9,
            bm_hour_to: 21,
            bm_urgency: 0.85,
            bm_demand_share: 0.3,
            bm_demand_min: 5,
            bm_demand_max: 80,
            bm_pay_base: 0.2,
            bm_pay_fear: 0.5,
            bm_pay_lawful: 0.3,
            bm_pay_pride: 0.3,
            bm_silence_w: 1.0,
            bm_kill_mult: 0.3,
            bm_interval_days: 7,
            bm_max_demands: 3,
            bm_tries: 3,
            bm_max_open: 200,
            hit_w_repost: 1.0,
            hit_repost_mult: 1.5,
            hit_repost_max: 2,
            hit_w_self: 1.0,
            hit_self_min: 0.6,
            hit_w_giveup: 0.6,
            hit_giveup_decay: 0.6,
            snitch_grudge: 0.4,
            cell_recruit_p: 0.08,
            cell_contact_p: 0.1,
            cell_fight_p: 0.05,
            cell_fight_affinity: -0.2,
            cell_kill_mult: 0.1,
            cell_loan_p: 0.1,
            cell_loan_coins: 10,
            cell_loan_broke: 5,
            cell_events_max: 20,
            cell_yard_hour: 24,
            cell_yard_affinity: 0.02,
            svc_hour_from: 18,
            svc_hour_to: 21,
            svc_lead_hours: 1,
            svc_cap: 30,
            svc_reach_ticks: 240,
            svc_urgency: 0.7,
            svc_guest_p: 0.15,
            svc_tithe_share: 0.1,
            svc_tithe_max: 10,
            svc_gain: 0.25,
            svc_convert_p: 0.25,
            svc_walkout_p: 0.03,
            svc_heretic_w: 0.8,
        }
    }
}

/// Jobs & lives phase 4 `[vice]` (docs/JOBS_AND_LIVES.md § 5.1): an
/// economic and legal simulation of venues and labour: the StripClub and
/// the Brothel (venues on the leisure ladder: visits, prices, tips), the
/// Lounge's companions, the `street_worker` Pitch (its night visits, the
/// pimp's cut, the street's assault roll), the sweeps (`Solicitation`) and
/// the policy levers seeded into `World::levers`. Live only under `[jobs2]
/// enabled`. Every number is a placeholder (R8: the income small and taxed
/// by cuts and fines).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViceCfg {
    pub enabled: bool,
    /// `[corps]` rows given the Vice niche at seed (and the seeded venues).
    pub corp_rows: Vec<usize>,
    /// Venues seeded per copy of each `corp_rows` row.
    pub strip_clubs: u32,
    pub brothels: u32,
    /// A visit's price by district tier (Sump, Mid, Spire).
    pub strip_price: [i64; 3],
    pub brothel_price: [i64; 3],
    /// The fun a visit gives, and the intimacy a Brothel's or a
    /// companion's visit gives.
    pub gain_strip: f32,
    pub gain_brothel: f32,
    pub intimacy_gain: f32,
    /// The ladder weighs a Brothel `1 + intimacy_pull × (1 − intimacy)`.
    pub intimacy_pull: f32,
    /// The visitor's tip: this share of the price, customer -> the worker
    /// on duty (`Flow::Tip`, untaxed).
    pub tip_share: f32,
    /// A visit's corp demand reference (`corp_brain::demand_of`'s Vice
    /// arm): visits a day per venue.
    pub vice_demand_ref: f32,
    // --- The street (on the phase-2 Pitch).
    /// A jobless opener under this lawfulness works a street pitch on
    /// `p_street` (else the phase-2 trades).
    pub street_lawful: f32,
    pub p_street: f32,
    /// A working street pitch's visit: on this chance an hour, at this
    /// price, from a resident of the district whose intimacy is under
    /// `street_intimacy`.
    pub p_street_visit: f32,
    pub street_price: i64,
    pub street_intimacy: f32,
    /// A pimp's gang takes this share of a street pitch's day (instead of
    /// `[jobs2] cut_frac`).
    pub pimp_cut: f32,
    /// A working street pitch-hour's assault roll (× the district's fear
    /// + 0.5), with no pimp assigned and the worker unarmed.
    pub p_assault_street: f32,
    /// A street pitch in a `Sweep` district is swept on this chance a day
    /// (`Solicitation`, the phase-2 fine and closing) while `street_legal`
    /// is off; reports a day at most per 1,000 residents (R3).
    pub p_street_sweep: f32,
    pub solicit_per_1000_day: f32,
    // --- The policy levers' seeds (`World::levers.vice`).
    pub brothels_licensed: bool,
    pub street_legal: bool,
    /// Phase 5's slots, seeded now.
    pub dollhouse_contracts: bool,
    pub captive_chip_trafficking: bool,
    // --- Jobs & lives phase 5 (§ 5.2, § 5.3). Sub-switches under
    // `enabled` (`chip`, `bd`, `patrol_sweep`), each byte-identical off.
    /// The chip contract and the Dollhouse (§ 5.2).
    pub chip: bool,
    /// The BD studios, the Braindance units and the Recorder (§ 5.3).
    pub bd: bool,
    /// Phase 4's carry-over: a street pitch outside a `Sweep` district is
    /// swept on `p_patrol_sweep` × the law's alertness (under the same caps).
    pub patrol_sweep: bool,
    pub p_patrol_sweep: f32,
    // --- The chip contract (§ 5.2). Numbers only.
    /// Dollhouses seeded per copy of each `corp_rows` row.
    pub dollhouses: u32,
    /// A visit's price by district tier (Sump, Mid, Spire), and its fun.
    pub dollhouse_price: [i64; 3],
    pub gain_dollhouse: f32,
    /// A contract runs this many days.
    pub chip_days: u32,
    /// Its wage is the row's × this (`Job.premium`).
    pub chip_mult: f32,
    /// The chip's `Kit.load` (above any implant tier's `sanity_cost`).
    pub chip_load: f32,
    /// The install and the removal, each paid by the holder to the Clinic.
    pub chip_fee: i64,
    /// The `Chipped` trait's one-time sanity cut at the removal.
    pub chipped_sanity: f32,
    /// A gang Dollhouse front's captives at most (a gang order each).
    pub captives_per_front: u32,
    // --- Braindance (§ 5.3).
    /// BdStudios seeded per copy of each `corp_rows` row.
    pub bd_studios: u32,
    /// Units a studio's produce-model staff make an hour, × (0.5 + skill).
    pub bd_rate: f32,
    /// A studio keeps this many units; the rest go to the Arcades and the World.
    pub bd_floor: u32,
    /// An Arcade's shelf of units, bought from a studio at `bd_wholesale`.
    pub arcade_bd: u32,
    pub bd_wholesale: i64,
    /// A black unit's price: `bd_wholesale × black_bd_price_mult`; a Bar
    /// drinker with a dealer present buys one on `p_black_buy`.
    pub black_bd_price_mult: f32,
    pub p_black_buy: f32,
    /// A gang with this many members keeps one Recorder, bought abroad;
    /// alive at most per 1,000 residents.
    pub recorder_members: usize,
    pub recorders_per_1000: f32,
}

impl Default for ViceCfg {
    fn default() -> Self {
        ViceCfg {
            enabled: false,
            corp_rows: vec![5],
            strip_clubs: 1,
            brothels: 1,
            strip_price: [4, 6, 10],
            brothel_price: [5, 8, 14],
            gain_strip: 0.55,
            gain_brothel: 0.45,
            intimacy_gain: 0.4,
            intimacy_pull: 1.0,
            tip_share: 0.15,
            vice_demand_ref: 4.0,
            street_lawful: 0.35,
            p_street: 0.5,
            p_street_visit: 0.3,
            street_price: 5,
            street_intimacy: 0.7,
            pimp_cut: 0.3,
            p_assault_street: 0.01,
            p_street_sweep: 0.2,
            solicit_per_1000_day: 1.0,
            brothels_licensed: true,
            street_legal: false,
            dollhouse_contracts: true,
            captive_chip_trafficking: true,
            chip: false,
            bd: false,
            patrol_sweep: false,
            p_patrol_sweep: 0.1,
            dollhouses: 1,
            dollhouse_price: [10, 16, 30],
            gain_dollhouse: 0.6,
            chip_days: 30,
            chip_mult: 2.0,
            chip_load: 0.4,
            chip_fee: 80,
            chipped_sanity: 0.1,
            captives_per_front: 1,
            bd_studios: 1,
            bd_rate: 0.5,
            bd_floor: 5,
            arcade_bd: 10,
            bd_wholesale: 6,
            black_bd_price_mult: 2.0,
            p_black_buy: 1.0,
            recorder_members: 6,
            recorders_per_1000: 2.0,
        }
    }
}

/// M12 litter (docs/M12_DISTRICTS.md § 3, plan D16-D19, D23-D24).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LitterCfg {
    /// `off()` = false: no deposits, no delay, no sanitation credit.
    pub enabled: bool,
    /// Lost by every tile in 1..=254 at midnight.
    pub decay: u8,
    /// Whole walker steps (`[exec] step_units` each since T1; ticks before round 9 Y4) added to a Full mover's next step per band (clean, littered, trashed, heaped).
    pub step_ticks: [u8; 4],
    /// A Coarse walk is `1 + timed_mult × district litter` longer.
    pub timed_mult: f32,
    pub sleep_penalty: f32,
    /// Mood: `− mood × district litter` in the hourly update.
    pub mood: f32,
    pub clean_per_shift: u32,
    /// Coins per 32 units an owner cleans around its doors.
    pub owner_clean_cost: i64,
    pub owner_clean_units: u32,
    pub gang_clean_per_member: u32,
    pub clean_pride: f32,
    /// The Damage hook (D19): rubble (255) blocks movement. Off in M12.
    pub rubble_blocks: bool,
    pub rubble_clean_mult: u32,
    /// M12 fix pass: every deposit in the spec's table is scaled by this
    /// (then capped at 254), so one crime leaves a visible mark.
    #[serde(default = "LitterCfg::d_deposit_mult")]
    pub deposit_mult: f32,
    /// M12 fix pass: a district's `litter` is the share of its street tiles
    /// at or above this value (32, the littered band).
    #[serde(default = "LitterCfg::d_visible")]
    pub visible: u8,
}

impl LitterCfg {
    /// No litter at all (a pre-M12 save, `v1_profile`).
    pub fn off() -> LitterCfg {
        LitterCfg {
            enabled: false,
            decay: 1,
            step_ticks: [0, 0, 1, 2],
            timed_mult: 0.5,
            sleep_penalty: 0.15,
            mood: 0.2,
            clean_per_shift: 120,
            owner_clean_cost: 1,
            owner_clean_units: 64,
            gang_clean_per_member: 8,
            clean_pride: 0.5,
            rubble_blocks: false,
            rubble_clean_mult: 4,
            deposit_mult: 1.0,
            visible: 32,
        }
    }
    fn d_deposit_mult() -> f32 {
        1.0
    }
    fn d_visible() -> u8 {
        32
    }
}

/// M12 the street (docs/M12_DISTRICTS.md § 4, plan D20-D28).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreetCfg {
    /// `off()` = false: no Hotels or derelicts seeded, no Squat goal, no nightly booking.
    pub enabled: bool,
    pub hotel_beds: u8,
    pub night_price: i64,
    /// Tiles from a Full or Coarse agent to a Hotel door it will walk to.
    pub hotel_reach: u32,
    pub seed_hotels: usize,
    pub seed_derelict_blocks: usize,
    /// An unsold estate's building goes to the City only while the Treasury holds this.
    pub city_absorb_floor: i64,
    /// A non-city Block empty this long whose owner's purse is negative is abandoned.
    pub abandon_days: u64,
    pub squat_reach: u32,
    pub squat_ban_days: u64,
    /// Phase 3 decision (plan D25 leaves "who re-lets" open): the City
    /// repairs and re-lets a derelict Block after this many days derelict,
    /// while the Treasury holds `city_absorb_floor` (0 = never).
    #[serde(default)]
    pub relet_days: u64,
    /// Hunger as sickness P4b (Dylan, J25: the evicted land on the lower
    /// rungs, not the street): the squat rung grows with demand. Each night
    /// while the rough sleepers outnumber the free squat slots, up to
    /// `shanties_per_night` vacant Lots in the districts with the most rough
    /// sleepers go up as shanties: the Lot stays a Lot (a builder may
    /// clear it), derelict, ownerless, `shanty_capacity` slots; at most
    /// `shanty_lots_max` Lots at once, so the construction P2 kept them for
    /// goes on (always on since P10 with the street rung, `enabled`).
    #[serde(default = "default_shanty_capacity")]
    pub shanty_capacity: u8,
    #[serde(default = "default_shanties_per_night")]
    pub shanties_per_night: usize,
    #[serde(default = "default_shanty_lots_max")]
    pub shanty_lots_max: usize,
    /// P4b (with `shanties`): a Hotel booked full at the night's pass adds a
    /// floor of beds, paid by its owner at `floor_cost`, up to this many floors.
    #[serde(default = "default_hotel_floors_max")]
    pub hotel_floors_max: u8,
}

fn default_hotel_floors_max() -> u8 {
    4
}

fn default_shanty_lots_max() -> usize {
    20
}

fn default_shanty_capacity() -> u8 {
    24
}

fn default_shanties_per_night() -> usize {
    3
}

impl StreetCfg {
    /// The street rung off (a pre-M12 save, `v1_profile`).
    pub fn off() -> StreetCfg {
        StreetCfg {
            enabled: false,
            hotel_beds: 12,
            night_price: 3,
            hotel_reach: 48,
            seed_hotels: 0,
            seed_derelict_blocks: 0,
            city_absorb_floor: 5000,
            abandon_days: 14,
            squat_reach: 40,
            squat_ban_days: 14,
            relet_days: 0,
            shanty_capacity: 24,
            shanties_per_night: 3,
            shanty_lots_max: 20,
            hotel_floors_max: 4,
        }
    }
}

/// M12 riots and crossfire (docs/M12_DISTRICTS.md § 6, plan D29-D35).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RiotsCfg {
    /// `off()` = false: no district riots (crossfire still reads `p_crossfire`).
    pub enabled: bool,
    pub riot_threshold: f32,
    pub riot_days: u16,
    pub riot_cooldown_days: u64,
    pub riot_min: usize,
    pub riot_max: usize,
    pub max_active_riots: usize,
    pub riot_lawfulness: f32,
    pub riot_muster_hour: u16,
    pub loot_frac: f32,
    pub riot_close_days: u64,
    pub riot_vent: f32,
    pub crush_kill_mult: f32,
    pub crossfire_radius: u32,
    pub p_crossfire: f32,
    pub p_crossfire_kill: f32,
    pub riot_stat_bystanders: usize,
    /// Plan risk 3 (LOD): a rioter ranks with the gangs (class 2) from this
    /// many hours before the muster until the riot ends; 24 or more = from
    /// the trigger at midnight.
    #[serde(default = "RiotsCfg::default_promote_hours")]
    pub riot_promote_hours: u16,
    /// M12 fix pass (throughput): only the first this many of a riot's
    /// rioters (most miserable first) rank with the gangs for a body; the
    /// rest march as they are. 0 = all of them.
    #[serde(default)]
    pub riot_promote_max: usize,
}

impl RiotsCfg {
    fn default_promote_hours() -> u16 {
        24
    }

    /// No riots and no crossfire (a pre-M12 save, `v1_profile`, the calibration city).
    pub fn off() -> RiotsCfg {
        RiotsCfg {
            enabled: false,
            riot_threshold: 0.55,
            riot_days: 3,
            riot_cooldown_days: 14,
            riot_min: 6,
            riot_max: 40,
            max_active_riots: 2,
            riot_lawfulness: 0.5,
            riot_muster_hour: 20,
            loot_frac: 0.3,
            riot_close_days: 3,
            riot_vent: 0.5,
            crush_kill_mult: 3.0,
            crossfire_radius: 3,
            p_crossfire: 0.0,
            p_crossfire_kill: 0.1,
            riot_stat_bystanders: 4,
            riot_promote_hours: 24,
            riot_promote_max: 0,
        }
    }
}

/// M15 § 4: the Hunt (phase 3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HuntCfg {
    pub hunt_min: f32,
    pub max_hunts: usize,
    pub hunt_flat: f32,
    pub fresh_sighting_hours: u32,
    pub intel_k: f32,
    pub stakeout_hours: u32,
    pub lethal_min: f32,
    pub hunt_kill_p: f32,
    pub hunt_cooldown_days: u32,
    pub hunt_days: u32,
    pub street_silence: bool,
    /// W20.
    pub hold_score: f32,
    /// W23.
    pub stat_hunt_min: f32,
    /// Round 9 Z1 (the avenger): a grudge over a killing (kin or friend)
    /// hunts from this weight (0: `hunt_min`, as before).
    pub killed_hunt_min: f32,
    /// Z1: at most this many hunt one man at once (0: no cap).
    pub hunters_per_target: usize,
    /// Z1: a hunt with no word of its man and nowhere near to ask waits
    /// this long, the hunt kept (0: the goal cools as any unplannable goal).
    pub wait_hours: u32,
    /// Z1: a weak or cowardly hunter who cannot pay hands the grudge to
    /// the strongest of his own people at this share of its weight (0: off).
    pub hand_w: f32,
    /// Z1: a held man's cellmate handed the grudge goes for him in the
    /// yard on this daily chance (0: off).
    pub inside_fight_p: f32,
    /// Z1: the hunted gets word of the hunt on this chance (0: off).
    pub heard_p: f32,
    /// Z1: word of the hunt takes this off his safety need.
    pub heard_safety: f32,
    /// Z1: a hunted man this brave and lawless swears to get the hunter
    /// first (a grudge at `heard_counter_w`) ...
    pub heard_counter_courage: f32,
    pub heard_counter_lawful: f32,
    pub heard_counter_w: f32,
    /// ... any other lies low at home this many days.
    pub heard_low_days: u32,
}

impl Default for HuntCfg {
    fn default() -> Self {
        HuntCfg {
            hunt_min: 0.5,
            max_hunts: 16,
            hunt_flat: 0.0,
            fresh_sighting_hours: 48,
            intel_k: 0.5,
            stakeout_hours: 3,
            lethal_min: 0.75,
            hunt_kill_p: 0.5,
            hunt_cooldown_days: 3,
            hunt_days: 10,
            street_silence: true,
            hold_score: 0.6,
            stat_hunt_min: 0.2,
            killed_hunt_min: 0.0,
            hunters_per_target: 0,
            wait_hours: 0,
            hand_w: 0.0,
            inside_fight_p: 0.0,
            heard_p: 0.0,
            heard_safety: 0.0,
            heard_counter_courage: 1.1,
            heard_counter_lawful: 0.0,
            heard_counter_w: 0.0,
            heard_low_days: 0,
        }
    }
}
