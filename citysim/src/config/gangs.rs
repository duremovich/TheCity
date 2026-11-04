use super::*;

/// The pressure pass phase 3 `[protection]` (docs/PRESSURE_PASS.md § Phase
/// 3; `systems/protection.rs`, `leverage::protection_pass`): protection
/// keyed to held ground, refusals punished in person, the owner's choice
/// after, and the gang's purse (bail, the enforcer's cut). Absent: M16b's
/// protection (Expand/Contest weekday, the turf's Homes only). Every value a
/// placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProtectionCfg {
    /// The Hideout's district is held ground (a gang leans where it lives).
    pub hideout_ground: bool,
    /// Businesses leaned on per held district per weekly pass.
    pub lean_per_district: u8,
    /// The weekly levy: `levy_share` of the business's week's revenue, at
    /// least `levy_min` (a business taking less is not leaned on), at most
    /// `[leverage] protection_weekly`.
    pub levy_share: f32,
    pub levy_min: i64,
    /// Missed weeks (nobody met at a round stop, or the till short) before
    /// the gang punishes; a refused lean is punished at once.
    pub punish_misses: u8,
    /// The enforcer calls between these hours (open hours).
    pub punish_hour_from: u8,
    pub punish_hour_to: u8,
    /// The enforcer's Visit urgency.
    pub punish_urgency: f32,
    /// A smashed till: this many weeks of the levy taken from the payer's
    /// purse (what it holds of them) for the gang.
    pub smash_weeks: f32,
    /// A smashed till shuts the business this many days; a fire this many.
    pub smash_close_days: u16,
    pub fire_close_days: u16,
    /// The beating kills on `[crime] fight_death_p x` this.
    pub beat_kill_mult: f32,
    /// The enforcer keeps this share of what he takes; the rest is the gang's.
    pub enforcer_cut: f32,
    /// The punished owner's grudge on the enforcer.
    pub grudge: f32,
    /// A punished owner this lawful files the report himself (the choice's
    /// report is the weighted one).
    pub victim_report_lawful: f32,
    /// The owner's choice after a punishment, one weighted draw: pay
    /// `w_pay x (1.5 - courage)`, report `w_report x lawfulness`, a rival's
    /// cover `w_rival x (0.5 + courage)` (a rival gang with a leader), sell
    /// or close `w_sell x (0.25 + misery) x punishments` (with `[underwater]`).
    pub w_pay: f32,
    pub w_report: f32,
    pub w_rival: f32,
    pub w_sell: f32,
    /// A gang loses this much standing grudge on the rival that gave cover
    /// (its leader's grudge on the rival's leader).
    pub cover_grudge: f32,
    /// Bail: daily, a gang with the treasury frees a jailed member at
    /// `bail_per_day` a day of his sentence left, keeping `bail_reserve`;
    /// not past `bail_max_days` left.
    pub bail_per_day: i64,
    pub bail_reserve: i64,
    pub bail_min_days: u32,
    pub bail_max_days: u32,
    /// The daily stipend goes only to members with no wage post; the
    /// employed are paid at the weekly Collect (their cut of the take).
    pub stipend_jobless_only: bool,
    /// The stipends come out of the treasury above this war chest (a gang
    /// below it pays nobody daily, and its unpaid drift away).
    pub stipend_reserve: i64,
    /// OPEN-157: a contract's party the Fixer cannot meet now is sent there
    /// in person at its next open hour (`service::send_broker`).
    pub broker_visits: bool,
}

impl Default for ProtectionCfg {
    fn default() -> Self {
        ProtectionCfg {
            hideout_ground: true,
            lean_per_district: 3,
            levy_share: 0.15,
            levy_min: 10,
            punish_misses: 2,
            punish_hour_from: 10,
            punish_hour_to: 20,
            punish_urgency: 0.9,
            smash_weeks: 2.0,
            smash_close_days: 1,
            fire_close_days: 7,
            beat_kill_mult: 0.2,
            enforcer_cut: 0.25,
            grudge: 0.5,
            victim_report_lawful: 0.6,
            w_pay: 1.0,
            w_report: 0.6,
            w_rival: 0.5,
            w_sell: 0.3,
            cover_grudge: 0.4,
            bail_per_day: 5,
            bail_reserve: 100,
            bail_min_days: 2,
            bail_max_days: 20,
            stipend_jobless_only: true,
            stipend_reserve: 150,
            broker_visits: true,
        }
    }
}

/// Pressure pass round 6, V2 (docs/TURF.md § 1-2): `[turf]`. The city is cut
/// into square cells of `cell_tiles`; a gang's daily presence in a cell (held
/// Homes, members living there, members' hours on its street and corner, its
/// Hideout and fronts) summed over `presence_days` decides who holds it.
/// Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TurfCfg {
    /// A cell's side in tiles.
    pub cell_tiles: u16,
    /// The days of presence a hold reads.
    pub presence_days: u8,
    /// A gang holds a cell with this much presence over the window ...
    pub hold_min: f32,
    /// ... ahead of the next gang by this share of the next's (else the cell
    /// is contested, when the next has at least `keep_share` of `hold_min`).
    pub hold_margin: f32,
    /// A holder keeps its cell while its presence is at least this share of
    /// `hold_min` and nobody passes it by the margin.
    pub keep_share: f32,
    /// A day's presence: a held Home (claim at `CLAIM_HELD`) in the cell ...
    pub w_held_home: f32,
    /// ... a member living there ...
    pub w_resident: f32,
    /// ... the gang's Hideout, a front it owns ...
    pub w_hideout: f32,
    pub w_front: f32,
    /// ... a member's hour on the cell's street (or in a venue there), an
    /// hour on its corner ...
    pub w_street_hour: f32,
    pub w_corner_hour: f32,
    /// ... and a Statistical member's day on the street, credited to his
    /// corner's cell (his Home's when he has none).
    pub stat_street_hours: f32,
    /// The corner is held from this tick of the day to this one (wrapping).
    pub corner_from: u16,
    pub corner_to: u16,
    /// A member holds the corner at most this many ticks a night.
    pub corner_ticks_day: u16,
    /// A member's corner: his Home's cell when his gang holds it, else the
    /// nearest held cell whose corner lies within this of his Home door.
    pub corner_reach_tiles: u32,
    /// A day's tribute to a cell's holder: a Home it holds pays this ...
    pub tribute_held: i64,
    /// ... any other Home in the cell this.
    pub tribute_home: i64,
    /// The holder of a recruit's Home cell takes him on a tie this much weaker.
    pub recruit_affinity_cut: f32,
    /// After dark the residents pass over spots and venues in a contested
    /// cell; a body is told it within this of a contested corner.
    pub avoid_tiles: u32,
    /// Families with children moved out of contested cells a day, per 1,000
    /// residents, into a Home with room within `family_move_tiles`.
    pub family_moves_per_1000: f32,
    pub family_move_tiles: u32,
    /// A district's pimp lives in a cell its gang holds within this many
    /// cells of one of the district's street pitches.
    pub pimp_cells: u8,
    /// Round 6 V5 (docs/TURF.md § 3): the war over a held cell. `None` (the
    /// key absent): cells change hands by presence alone (V2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub war: Option<WarCfg>,
}

/// Round 6 V5 (docs/TURF.md § 3): `[turf.war]`, the war over a held cell:
/// provocation heat, the holder's answer at the corner, hold or flip, the
/// loser's counter-attack, and peace (exhaustion, a truce, the law's
/// crackdown). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WarCfg {
    /// Heat a provocation adds to a held cell, for its holder: a rival's body
    /// an hour on the corner, a tag (once a night), a deal in the cell, a
    /// shakedown of a door there, a beating of a holder's member there ...
    pub heat_corner_hour: f32,
    pub heat_tag: f32,
    pub heat_deal: f32,
    pub heat_shakedown: f32,
    pub heat_beating: f32,
    /// ... and an off-screen night of trespass (the daily roll).
    pub heat_stat: f32,
    /// A cell's heat keeps this share a day.
    pub heat_keep_day: f32,
    /// Past this the holder answers.
    pub answer_heat: f32,
    /// The answer's fight comes this long after the heat passes (the muster
    /// walks to the corner in it).
    pub answer_window_hours: u16,
    /// The members of the cell and of cells within this many cells muster.
    pub muster_cells: u32,
    /// A body counts at the fight within this of the corner.
    pub muster_reach_tiles: u32,
    /// A muster's corner arm scores this flat over the evening's corner.
    pub muster_flat: f32,
    /// Round 8 X4 (`OPEN-241`): a fight due within this many hours rouses
    /// the called members asleep about the cell, off shift (0 = the call
    /// wakes nobody) ...
    pub rouse_hours: u16,
    /// ... above this energy (the spent sleep through it).
    pub rouse_min_energy: f32,
    /// A side fights with at most this many.
    pub side_max: usize,
    /// The off-screen roll: a day's chance a gang provokes a held cell it
    /// borders (x `provoke_order_mult` under Contest, Expand or Retaliate;
    /// never under LieLow) ...
    pub provoke_p_day: f32,
    pub provoke_order_mult: f32,
    /// ... the chance a contested cell is fought over a day ...
    pub front_p_day: f32,
    /// ... and the chance a gang that lost a cell comes back for it a day,
    /// within `retake_days` of the loss.
    pub retake_p_day: f32,
    pub retake_days: u16,
    /// A cell won in a fight holds this many days whatever the presence says.
    pub hold_lock_days: u16,
    /// A raid won or a Hideout sacked moves the loser's cells within this
    /// many cells of its Hideout to the raider.
    pub raid_cells: u32,
    /// A war goes quiet after this many days without a fight.
    pub war_quiet_days: u16,
    /// Exhaustion: members dead, jailed or gone in the war over
    /// `exhaust_days` past `exhaust_share` of the side at the war's start
    /// (and at least `exhaust_min`) turn it to LieLow for `lielow_days`.
    pub exhaust_days: u16,
    pub exhaust_share: f32,
    pub exhaust_min: u16,
    pub lielow_days: u16,
    /// Two exhausted leaders strike a truce held this many days.
    pub truce_days: u16,
    /// The law's crackdown on a cell: each fight there adds 1 and each death
    /// `crackdown_death_w`; the heat keeps `crackdown_keep_day` a day; past
    /// `crackdown_heat` the cell is patrolled and cooled `crackdown_days`.
    pub crackdown_death_w: f32,
    pub crackdown_keep_day: f32,
    pub crackdown_heat: f32,
    pub crackdown_days: u16,
}

impl Default for WarCfg {
    fn default() -> Self {
        WarCfg {
            heat_corner_hour: 0.25,
            heat_tag: 0.25,
            heat_deal: 0.5,
            heat_shakedown: 0.5,
            heat_beating: 1.0,
            heat_stat: 1.0,
            heat_keep_day: 0.5,
            answer_heat: 1.0,
            answer_window_hours: 3,
            muster_cells: 1,
            muster_reach_tiles: 6,
            muster_flat: 0.5,
            rouse_hours: 0,
            rouse_min_energy: 0.2,
            side_max: 6,
            provoke_p_day: 0.004,
            provoke_order_mult: 3.0,
            front_p_day: 0.05,
            retake_p_day: 0.12,
            retake_days: 5,
            hold_lock_days: 5,
            raid_cells: 1,
            war_quiet_days: 7,
            exhaust_days: 14,
            exhaust_share: 0.2,
            exhaust_min: 2,
            lielow_days: 5,
            truce_days: 21,
            crackdown_death_w: 1.0,
            crackdown_keep_day: 0.8,
            crackdown_heat: 3.0,
            crackdown_days: 7,
        }
    }
}

impl Default for TurfCfg {
    fn default() -> Self {
        TurfCfg {
            cell_tiles: 24,
            presence_days: 7,
            hold_min: 30.0,
            hold_margin: 0.25,
            keep_share: 0.5,
            w_held_home: 2.0,
            w_resident: 1.0,
            w_hideout: 6.0,
            w_front: 3.0,
            w_street_hour: 1.0,
            w_corner_hour: 2.0,
            stat_street_hours: 3.0,
            corner_from: 1080,
            corner_to: 120,
            corner_ticks_day: 240,
            corner_reach_tiles: 60,
            tribute_held: 2,
            tribute_home: 1,
            recruit_affinity_cut: 0.1,
            avoid_tiles: 40,
            family_moves_per_1000: 0.2,
            family_move_tiles: 120,
            pimp_cells: 1,
            war: None,
        }
    }
}

/// The pressure pass phase 5b `[crew]` (docs/PRESSURE_PASS.md § Phase 5;
/// `systems/crew.rs`): the gang's side of consequence. Every value a
/// placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrewCfg {
    // --- raids write a result
    /// A defender beaten at his door holds this grudge on the raiding gang;
    /// a raider beaten holds it on the defending gang.
    pub raid_grudge: f32,
    /// The losing side's leader's grudge on the other gang.
    pub raid_leader_grudge: f32,
    /// A repelled raid is answered: the defenders' gang may retaliate for
    /// this many days (`Gang.retaliate_until`, the order brain's input).
    pub repelled_retaliate_days: u64,
    // --- duty
    /// A gang posts this share of its free members to the order's duty a
    /// day (a watch on held ground, a guard at the Hideout), at least one,
    /// at most `duty_max`. A raid order calls every member to the muster.
    pub duty_share: f32,
    pub duty_max: u8,
    /// The duty's hour: a watch on held ground, a guard at the Hideout.
    pub watch_hour: u8,
    pub guard_hour: u8,
    /// A member's duty place past this travel (ticks, from his Home) is not
    /// posted to him (the walk would eat the duty).
    pub duty_reach_ticks: u64,
    /// Coins a kept duty pays from the treasury above `pay_reserve`.
    pub duty_pay: i64,
    pub pay_reserve: i64,
    /// The duty's Visit urgency (the enforcer's is `[protection]
    /// punish_urgency`).
    pub duty_urgency: f32,
    /// A member whose shift falls on the duty chooses: the gang on
    /// `choose_base + choose_loyalty x (loyalty - 0.5) + choose_misery x
    /// misery + choose_rank` (a lieutenant) `- choose_unpaid` (the gang
    /// cannot pay), clamped to 0.05..0.95, else the shift. The gang's
    /// choice skips the shift (`misery::skip_for`: the absence counts toward
    /// the firing); the shift's is a missed duty.
    pub choose_base: f32,
    pub choose_loyalty: f32,
    pub choose_misery: f32,
    pub choose_rank: f32,
    pub choose_unpaid: f32,
    /// Standing (the ladder: `gang::leader_scores` adds `standing_w x
    /// standing`): a kept duty adds `kept_standing`, a missed one takes
    /// `miss_standing`, a sentence served without talking adds
    /// `served_standing`.
    pub standing_w: f32,
    pub kept_standing: f32,
    pub miss_standing: f32,
    pub served_standing: f32,
    /// Missed duties (the muster, a watch, a guard, a court summons) in
    /// `miss_window_days` that cost the colours: a beating on the way out.
    pub colours_misses: u32,
    pub miss_window_days: u32,
    /// The beating kills on `[crime] fight_death_p x` this.
    pub beat_kill_mult: f32,
    // --- court
    /// Members summoned to court by name a day (the leader holds court in
    /// person, `[leaders]`); a court with fewer than `court_quorum` of them
    /// there fails, with the reason.
    pub court_summons: u8,
    pub court_quorum: u8,
    /// `fold_fails` failed courts in `fold_window_days`: the gang cannot
    /// muster. It takes a rival's offer (absorbed: the members, the
    /// treasury, `absorb_offer` coins to the leader from the buyer) on
    /// `w_absorb x (1.5 - pride)`, or folds (the members walk, the treasury
    /// shared out) on `w_fold x (0.5 + pride)`. A creed gang folds.
    pub fold_fails: u32,
    pub fold_window_days: u32,
    pub absorb_offer: i64,
    pub w_absorb: f32,
    pub w_fold: f32,
    // --- release and return
    /// A jailed member (a sentence of `talk_min_days` or more) talks once a
    /// sentence on `talk_p x (1 - loyalty)`: a report naming a gang-mate.
    pub talk_p: f32,
    pub talk_min_days: u32,
    /// The bail the gang paid is the member's debt, taken from his duty pay
    /// until paid.
    pub bail_debt: bool,
    /// A released member walks to the Hideout this many hours after (a
    /// Visit); a release past 21:00 is met at once.
    pub welcome_after_hours: u8,
    // --- rest days
    /// A free member with no shift today goes to the crew (the Hideout under
    /// a fighting order, the gang's own venue or the Hideout under Expand,
    /// nobody under LieLow) on `crew_p`, at `crew_hour`.
    pub crew_p: f32,
    pub crew_hour: u8,
    // --- Pressure pass round 2, R5 (OPEN-214): the verbs that stalled
    /// A recruit tries out at his gang's Hideout or, past this travel from
    /// where he stands, its nearest front within it; with neither in reach
    /// the gang is too far to join today (JoinGang is not scored).
    pub tryout_reach_ticks: u64,
    /// A recruit turned away at the try-out does not try again for this
    /// many days.
    pub tryout_cool_days: u64,
    /// A member posted to today's muster (who did not choose the shift)
    /// scores the Raid goal this much over its considerations: the muster
    /// is a duty, as a crew Visit is (`duty_urgency`), not an evening's whim.
    pub muster_flat: f32,
    /// A corp raid's prize is a building of the hoarding corp within this
    /// travel of the gang's Hideout (the march's window is 6 h).
    pub raid_target_reach_ticks: u64,
    /// A court summons missed at work is excused: it takes the standing but
    /// does not count toward `colours_misses` (the attrition, judged after
    /// the stalls: two thirds of the misses behind a lost colour were the
    /// daily court rota).
    pub court_work_excused: bool,
    /// Round 7 W2 (`OPEN-241`): the first marchers at a raid's door hold the
    /// brawl for the column's tail, a marcher due at the door within this
    /// many ticks (V15: the first three went in and the rest, a Coarse
    /// body's timed walk behind, found it over; 88 released on the way at 20k).
    pub march_wait_ticks: u64,
    /// V8b (`OPEN-276`): a gang raid's muster holds past its hour for its
    /// column, at most this many ticks, until `[gangs] raid_min_members`
    /// stand there; short of them then, it breaks up at the muster (the lone
    /// member marched the island to the door and called it off there).
    /// 0 = the old rule (the muster leaves at its hour, whoever came).
    pub muster_hold_ticks: u64,
}

impl Default for CrewCfg {
    fn default() -> Self {
        CrewCfg {
            raid_grudge: 0.3,
            raid_leader_grudge: 0.5,
            repelled_retaliate_days: 3,
            duty_share: 0.3,
            duty_max: 4,
            watch_hour: 18,
            guard_hour: 21,
            duty_reach_ticks: 180,
            duty_pay: 3,
            pay_reserve: 30,
            duty_urgency: 0.85,
            choose_base: 0.3,
            choose_loyalty: 0.6,
            choose_misery: 0.5,
            choose_rank: 0.15,
            choose_unpaid: 0.15,
            standing_w: 0.2,
            kept_standing: 0.1,
            miss_standing: 0.2,
            served_standing: 1.0,
            colours_misses: 3,
            miss_window_days: 10,
            beat_kill_mult: 0.2,
            court_summons: 3,
            court_quorum: 1,
            fold_fails: 3,
            fold_window_days: 7,
            absorb_offer: 20,
            w_absorb: 1.0,
            w_fold: 0.6,
            talk_p: 0.3,
            talk_min_days: 3,
            bail_debt: true,
            welcome_after_hours: 1,
            crew_p: 0.6,
            crew_hour: 19,
            // R5 (OPEN-214)
            tryout_reach_ticks: 180,
            tryout_cool_days: 3,
            muster_flat: 0.5,
            raid_target_reach_ticks: 240,
            court_work_excused: true,
            march_wait_ticks: 60,
            muster_hold_ticks: 90,
        }
    }
}

/// Jobs & lives phase 6 `[gray]` (docs/JOBS_AND_LIVES.md § 7): papers,
/// forgers and a corruptible Clerk, black clinics and struck-off docs,
/// organs as a good (rotting, sold to Clinics and the World's capped book),
/// customs on the organs' export, and the gang types as data. A board
/// game's counters: tiers, units, rolls and coins. Live only under
/// `[jobs2] enabled`. Every number is a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GrayCfg {
    pub enabled: bool,
    /// Fake papers by tier (1, 2) at a forger; a Clerk under `clerk_lawful`
    /// sells real ones for `id_bribe` (`Flow::Bribe`).
    pub id_price: [i64; 2],
    pub id_bribe: i64,
    pub clerk_lawful: f32,
    /// A wanted holder passes a guard's sighting for the hour on
    /// `id_w × tier` (papers bought while wanted).
    pub id_w: f32,
    /// At an arrest fake papers are exposed on this chance (`Forgery`).
    pub p_expose: f32,
    /// The Fixer's owner's share of a forger's sale.
    pub fixer_cut: f32,
    /// Round 5 U5 (OPEN-217): the forger's second commission, a forged
    /// weapons licence: an adult who wants a weapon, holds no licence, is
    /// under `licence_lawful` (the lawful buy the Clerk's) and has the
    /// coins for it, the cheapest weapon and `[weapons] coin_reserve`
    /// orders one at midnight from the nearest forger within
    /// `licence_reach_ticks` of home, picked up and paid in the Fixer's
    /// open hours as the papers are. 0: no forged licences.
    pub licence_price: i64,
    pub licence_lawful: f32,
    pub licence_reach_ticks: u64,
    /// A forged licence passes a guard who notices the weapon on this
    /// chance (a pure hash of the holder and the hour); else it is
    /// exposed: void, `Forgery` on the holder and the forger, and the
    /// weapon's `UnlicensedArms` filed. At an arrest, `p_expose`.
    pub licence_pass: f32,
    /// Organs a body yields to the gang that rips it, the share lost a day,
    /// a Clinic's shelf (bought from a gang at `organ_price`), a
    /// stabilisation's draw on it, and customs on the World's book.
    pub organs_per_body: u32,
    pub organ_rot: f32,
    pub clinic_organs: u32,
    pub organ_price: i64,
    pub p_organ_needed: f32,
    /// Jobs & lives phase 8 (the scavs): an abductor dragging its take home
    /// scores its GangWork this (and the plan's hysteresis), as a hunter its
    /// Hunt (`[hunt] hold_score`), so only an emergency drops the drag.
    pub drag_hold_score: f32,
    pub p_customs: f32,
    /// A gang builds a BlackClinic with its found cost and this to spare.
    pub black_clinic_spare: i64,
    /// A BlackClinic in a district the law sweeps or cracks down on its
    /// gang is found on this chance a day (`BlackMarket`, shut this long).
    pub p_bm_raid: f32,
    pub bm_close_days: u64,
    /// A licensed Ripperdoc is struck off on this × (1 − lawfulness) a day.
    pub p_struck_off: f32,
    /// The gang types (D-scavs): rows by share of the gangs (a pure hash).
    pub gang_types: Vec<GangTypeCfg>,
}

/// A gang type row (Dylan's answer 1): its share of the gangs, the flat it
/// adds to the Harvest order, its rip's kill chance and whether its Harvest
/// target counts organs (anyone in its ground).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GangTypeCfg {
    pub id: String,
    pub share: f32,
    pub harvest_flat: f32,
    pub kill_p: f32,
    pub organ_targets: bool,
}

impl Default for GangTypeCfg {
    fn default() -> Self {
        GangTypeCfg { id: String::new(), share: 0.0, harvest_flat: 0.0, kill_p: 0.5, organ_targets: false }
    }
}

impl Default for GrayCfg {
    fn default() -> Self {
        GrayCfg {
            enabled: false,
            id_price: [40, 120],
            id_bribe: 60,
            clerk_lawful: 0.35,
            id_w: 0.15,
            p_expose: 0.3,
            fixer_cut: 0.2,
            licence_price: 0,
            licence_lawful: 0.5,
            licence_reach_ticks: 120,
            licence_pass: 0.6,
            organs_per_body: 2,
            organ_rot: 0.25,
            clinic_organs: 3,
            organ_price: 30,
            p_organ_needed: 0.5,
            drag_hold_score: 0.75,
            p_customs: 0.1,
            black_clinic_spare: 200,
            p_bm_raid: 0.2,
            bm_close_days: 3,
            p_struck_off: 0.002,
            gang_types: Vec::new(),
        }
    }
}

/// Several gangs and their faction brain (M8, `docs/M8_FACTIONS.md`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GangsCfg {
    /// One gang per Hideout, in map file order; a missing name is `Gang N`.
    pub names: Vec<String>,
    /// Per gang; the last entry repeats.
    pub treasury_initial: Vec<i64>,
    /// GangWork's order consideration: `Linear{order_weight, 1 - order_weight}` on loyalty while following.
    pub order_weight: f32,
    /// Below this loyalty a member ignores the order.
    pub freelance_loyalty: f32,
    /// A gang smaller than this recruits on promise; past it the treasury must fund the stipend.
    pub recruit_on_promise: usize,
    /// No gang recruits past this headcount.
    pub max_members: usize,
    pub heat_days: u64,
    /// A new order must beat the current one by this at the daily rescoring.
    pub hysteresis: f32,
    /// own / rival headcount needed to Contest / Raid (Logistic mids).
    pub contest_min_ratio: f32,
    pub raid_min_ratio: f32,
    /// Rival treasury worth raiding.
    pub raid_min_prize: i64,
    pub raid_cooldown_days: u64,
    /// Treasury fraction taken on a won raid; a sack takes everything.
    pub raid_prize_frac: f32,
    /// Departure at this hour of the day the order is set (or the next day if under two hours away).
    pub raid_muster_hour: u16,
    /// The Raid goal opens this many hours before `raid_at`.
    pub raid_gather_hours: u16,
    /// Raiders within this many tiles of the rival door join the brawl.
    pub raid_gather_radius: u32,
    pub retaliate_days: u64,
    pub sacked_days: u64,
    /// Pending shock severities that force an immediate rescoring.
    pub shock_severity_rethink: f32,
    /// The perf pass phase 7d (OPEN-211): a gang still on `Order::Job` after its job settled, with the next
    /// order already held for the giving in person (`Gang.pending_order`), does not rescore every tick; it
    /// rescores at the daily tick or on a shock rethink.
    /// Members who sleep at the Hideout each night (at most half the gang), so a raid meets someone.
    #[serde(default = "GangsCfg::default_night_watch")]
    pub night_watch: usize,
    /// M9 breakouts: their own cooldown, the fit headcount needed, and the
    /// convicts freed per breach.
    #[serde(default = "GangsCfg::default_breakout_cooldown_days")]
    pub breakout_cooldown_days: u64,
    #[serde(default = "GangsCfg::default_breakout_min_members")]
    pub breakout_min_members: usize,
    #[serde(default = "GangsCfg::default_breakout_max_freed")]
    pub breakout_max_freed: usize,
    pub order_flat: OrderFlatCfg,
    /// M12 D37: a raid musters at the gang's held Home or squat nearest the
    /// target door when one lies within this many tiles; 0 = the Hideout (M8).
    #[serde(default)]
    pub muster_near_tiles: u32,
    /// Round 8 X4 (`OPEN-241`): a raid ordered weighs the Hideout and this
    /// many of the held Homes nearest the target (within `muster_near_tiles`)
    /// for the muster most members reach (`raid::pick_muster`; 0 = the old
    /// rule, `raid::muster_point`).
    #[serde(default)]
    pub muster_candidates: usize,
    /// M12 D40: a gang empty this many days loses its claims (0 = only the 30-day disband).
    #[serde(default)]
    pub empty_claims_days: u64,
    /// M12 D40: an emptied gang re-forms only after this many days (0 = at once) ...
    #[serde(default)]
    pub reform_days: u64,
    /// Shadow V4 bug 8: a member this many days in the gang outranks a newer
    /// one for the lead (0 = loyalty and tenure alone).
    #[serde(default)]
    pub leader_min_days: u32,
    /// Round 8 X4 (`OPEN-241`): the sitting leader, free and in the gang,
    /// keeps the post until a member's ranking score passes his by more than
    /// this (0 = the ranking's top leads, as before). V16: the lead passed
    /// back and forth on a day's duty score with no line (`raider_29990`).
    #[serde(default)]
    pub leader_keep_margin: f32,
    /// ... and only while its Hideout's district coverage is below this.
    #[serde(default = "GangsCfg::default_reform_max_coverage")]
    pub reform_max_coverage: f32,
    /// M12 D36: splits after a decapitation. `split_base` 0 = never (M11).
    #[serde(default = "GangsCfg::default_split_strength_ratio")]
    pub split_strength_ratio: f32,
    #[serde(default = "GangsCfg::default_split_loyalty")]
    pub split_loyalty: f32,
    #[serde(default)]
    pub split_base: f32,
    #[serde(default = "GangsCfg::default_max_gangs")]
    pub max_gangs: usize,
    #[serde(default)]
    pub splinter_names: Vec<String>,
    /// M12 D39: a won raid on a corp building takes `corp_raid_frac` of the
    /// corp treasury, capped at `corp_raid_cap`; a cap of 0 = no corp raids (M11).
    #[serde(default)]
    pub corp_raid_frac: f32,
    #[serde(default)]
    pub corp_raid_cap: i64,
    /// M12 fix pass: a won corp raid takes this share of the building's food
    /// stock (the coins stay `corp_raid_frac`, capped at `corp_raid_cap`).
    #[serde(default = "GangsCfg::default_corp_raid_loot_frac")]
    pub corp_raid_loot_frac: f32,
    /// M12 fix pass: the private guards on a corp building's side (its
    /// owner's Security Office or its contractor's) posted at the door when a
    /// raid arrives: up to this many on shift, nearest first.
    #[serde(default = "GangsCfg::default_corp_raid_posted")]
    pub corp_raid_posted: usize,
    /// M12 fix pass: a corp raid's crew breaks once this share of it has
    /// lost a pairing (rounded up, at least one); 0 = to the last raider.
    #[serde(default)]
    pub corp_raid_break: f32,
    /// M12 phase 4: Raid and Retaliate need this many fit members (a crew
    /// for the door); 0 = any (M11).
    #[serde(default)]
    pub raid_min_members: usize,
    /// V8b (`OPEN-276`): a raid's column has this many hours from leaving
    /// its muster to the door before the march is written off (was
    /// `faction::RAID_MARCH_TICKS`, 6 h; riots and missions read it too).
    #[serde(default = "GangsCfg::default_raid_march_hours")]
    pub raid_march_hours: u64,
    /// Shadow V4 bug 11: no Hideout bed further than this many tiles from
    /// the member (0 = any distance).
    #[serde(default)]
    pub hole_up_max_tiles: u32,
    /// L2 (L23): the Statistical GangWork day's priors per member-day and
    /// the member-days of evidence they are worth (ledger maths).
    #[serde(default = "GangsCfg::default_stat_extort_prior")]
    pub stat_extort_prior: f32,
    #[serde(default = "GangsCfg::default_stat_claim_prior")]
    pub stat_claim_prior: f32,
    #[serde(default = "GangsCfg::default_stat_deal_prior")]
    pub stat_deal_prior: f32,
    #[serde(default = "GangsCfg::default_stat_prior_days")]
    pub stat_prior_days: f32,
    /// Lives at work phase 6 (§ 4.5): lieutenants, delegation, the cut and
    /// the ladder, under the `[lives]` master (`[gangs] lieutenants = false`).
    /// Off, every key below is 0 and none is written into a save.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lieutenants: bool,
    /// Rank 1 to the top `1 + members / lieutenant_per` after the leader.
    #[serde(default, skip_serializing_if = "is_zero_usize_cfg")]
    pub lieutenant_per: usize,
    /// The leader delegates the march and the shakedowns while the gang
    /// has this many fit members.
    #[serde(default, skip_serializing_if = "is_zero_usize_cfg")]
    pub delegate_min_fit: usize,
    /// The payout's cut off the top: the leader's, and the lieutenants' shared.
    #[serde(default, skip_serializing_if = "is_zero_f32_cfg")]
    pub leader_cut: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32_cfg")]
    pub lieutenant_cut: f32,
    /// The leader score's weight per raid won as its leader.
    #[serde(default, skip_serializing_if = "is_zero_f32_cfg")]
    pub raid_w: f32,
}

impl GangsCfg {
    fn default_night_watch() -> usize {
        4
    }
    fn default_raid_march_hours() -> u64 {
        6
    }
    fn default_breakout_cooldown_days() -> u64 {
        15
    }
    fn default_breakout_min_members() -> usize {
        2
    }
    fn default_breakout_max_freed() -> usize {
        3
    }
    fn default_reform_max_coverage() -> f32 {
        f32::MAX
    }
    fn default_stat_extort_prior() -> f32 {
        0.02
    }
    fn default_stat_claim_prior() -> f32 {
        0.02
    }
    fn default_stat_deal_prior() -> f32 {
        0.01
    }
    fn default_stat_prior_days() -> f32 {
        30.0
    }
    fn default_split_strength_ratio() -> f32 {
        0.8
    }
    fn default_split_loyalty() -> f32 {
        0.6
    }
    fn default_max_gangs() -> usize {
        4
    }
    fn default_corp_raid_loot_frac() -> f32 {
        0.25
    }
    fn default_corp_raid_posted() -> usize {
        3
    }

    /// M12 D46: the phase 4 gang keys at their M11 behaviour (no muster
    /// points, no claim loss, instant re-forming, no splits, no corp raids).
    pub fn m11_behaviour(&mut self) {
        self.muster_near_tiles = 0;
        self.empty_claims_days = 0;
        self.reform_days = 0;
        self.reform_max_coverage = f32::MAX;
        self.split_base = 0.0;
        self.corp_raid_cap = 0;
        self.raid_min_members = 0;
    }

    /// Lives at work phase 6 (§ 4.5): the lieutenants' keys off (0, not written).
    pub fn strip_lieutenants(&mut self) {
        self.lieutenants = false;
        self.lieutenant_per = 0;
        self.delegate_min_fit = 0;
        self.leader_cut = 0.0;
        self.lieutenant_cut = 0.0;
        self.raid_w = 0.0;
    }

    /// The `[gangs]` block of `assets/config.toml`, for saves written before it existed.
    pub fn from_assets() -> GangsCfg {
        let dir = Config::find_assets_dir()
            .unwrap_or_else(|| panic!("assets/config.toml not found; set CITYSIM_ASSETS or run from the repo"));
        Config::load_from(&dir).gangs
    }
}

/// Flat terms added to each order's product of considerations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderFlatCfg {
    pub expand: f32,
    pub contest: f32,
    pub raid: f32,
    pub retaliate: f32,
    pub lielow: f32,
    #[serde(default = "OrderFlatCfg::default_breakout")]
    pub breakout: f32,
    /// M12 D38 (phase 3): the gang's Squat order.
    #[serde(default)]
    pub squat: f32,
    /// M13 D36 (phase 3): the gang's Harvest order.
    #[serde(default)]
    pub harvest: f32,
    /// M14 V30 (phase 3): the gang's VirtRaid order.
    #[serde(default)]
    pub virt_raid: f32,
    /// M14 review: added to VirtRaid's flat while the gang holds a hack
    /// grudge whose wipe is in its best runner's reach.
    #[serde(default)]
    pub virt_grudge: f32,
    /// M16a (plan C26, phase 2): the gang's `Order::Job`.
    #[serde(default)]
    pub job: f32,
}

impl OrderFlatCfg {
    fn default_breakout() -> f32 {
        0.28
    }
}
