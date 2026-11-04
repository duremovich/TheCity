use super::*;

// ---------------------------------------------------------------------------
// M16a contracts (docs/M16_CONTRACTS.md § 1-3; plan C3). Contract records,
// Fixers, missions and bounties are game state between fictional agents:
// records with a price and a deadline, matched by a score, resolved by a
// seeded dice roll. Every section's `off()` carries the spec's values with
// the master switch off; nothing here is read without `contracts::on`.
// ---------------------------------------------------------------------------

/// A value per 16a `ContractKind` (spec tables `price_base`,
/// `deadline_days`); M16b appends its kinds' keys (serde default).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KindTable<T> {
    pub hit: T,
    pub beat: T,
    pub guard: T,
    pub locate: T,
    /// M16b phase 1.
    pub persuade: T,
    /// M16b phase 2.
    pub abduct: T,
    pub rescue: T,
    pub escort: T,
    pub steal: T,
    pub deliver: T,
    pub spin: T,
    /// M16b phase 3.
    pub trauma: T,
    /// M16b phase 4.
    pub tag: T,
    pub vote: T,
    /// M17 phase 3.
    pub retake: T,
}

impl<T: Default + Copy> KindTable<T> {
    pub fn get(&self, k: crate::contract::ContractKind) -> T {
        use crate::contract::ContractKind;
        match k {
            ContractKind::Hit => self.hit,
            ContractKind::Beat => self.beat,
            ContractKind::Guard => self.guard,
            ContractKind::Locate => self.locate,
            ContractKind::Persuade => self.persuade,
            ContractKind::Abduct => self.abduct,
            ContractKind::Rescue => self.rescue,
            ContractKind::Escort => self.escort,
            ContractKind::Steal => self.steal,
            ContractKind::Deliver => self.deliver,
            ContractKind::Spin => self.spin,
            ContractKind::Trauma => self.trauma,
            ContractKind::Tag => self.tag,
            ContractKind::Vote => self.vote,
            ContractKind::Retake => self.retake,
        }
    }
}

/// `[contracts]` (spec § 1-2; plan keys C20).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContractsCfg {
    /// Open and taken records at once; a post past it is refused.
    pub max_open: usize,
    /// Closed records are dropped at midnight after this many days
    /// (fulfilled Hits and Beats: `max(this, [law] accessory_days)`, C2).
    pub closed_keep_days: u32,
    /// C7: the base price per kind.
    pub price_base: KindTable<i64>,
    /// The deadline per kind, in days.
    pub deadline_days: KindTable<u16>,
    /// C7: `× (1 + risk_w × risk)`.
    pub risk_w: f32,
    /// C7: `× (1 + standing_w × standing)`.
    pub standing_w: f32,
    /// C16: failed attempts before `Failed`.
    pub max_attempts: u8,
    /// C12, C13: a taker accepts at this `accept_score`.
    pub accept_min: f32,
    /// C13: the Contract goal's flat (mode A).
    pub contract_flat: f32,
    /// C6: `renege_base × (1 − lawfulness) × (1 − honour)`.
    pub renege_base: f32,
    /// C6: the reneged taker's `Betrayed` grudge.
    pub renege_grudge: f32,
    /// C20: a Hunt holder with `might_gap` below this hires instead.
    pub hire_gap: f32,
    /// C33 (phase 3): a gang below this strength ratio hires a Hit.
    pub hire_ratio: f32,
    /// C33 (phase 3): an exec below this lawfulness posts a Beat instead of the bribe.
    pub dirty_lobby: f32,
    /// C33 (phase 3): the law's public Locate after this many days unseen.
    pub law_bounty_days: u32,
    /// C33 (phase 3): a captain below this lawfulness posts a Hit.
    pub death_squad_lawfulness: f32,
    /// C20 (plan key, placeholder): the hire pass's Hunt score without the
    /// might term.
    pub hire_min: f32,
}

impl Default for ContractsCfg {
    fn default() -> Self {
        ContractsCfg {
            max_open: 256,
            closed_keep_days: 30,
            price_base: KindTable {
                hit: 400,
                beat: 120,
                guard: 25,
                locate: 20,
                persuade: 100,
                abduct: 300,
                rescue: 350,
                escort: 80,
                steal: 150,
                deliver: 40,
                spin: 120,
                trauma: 35,
                tag: 150,
                vote: 250,
                retake: 500,
            },
            deadline_days: KindTable {
                hit: 10,
                beat: 7,
                guard: 7,
                locate: 14,
                persuade: 7,
                abduct: 10,
                rescue: 7,
                escort: 3,
                steal: 7,
                deliver: 3,
                spin: 5,
                trauma: 7,
                tag: 10,
                vote: 14,
                retake: 10,
            },
            risk_w: 1.5,
            standing_w: 1.0,
            max_attempts: 2,
            accept_min: 0.35,
            contract_flat: 0.0,
            renege_base: 0.3,
            renege_grudge: 0.6,
            hire_gap: -0.1,
            hire_ratio: 0.7,
            dirty_lobby: 0.3,
            law_bounty_days: 3,
            death_squad_lawfulness: 0.2,
            hire_min: 0.3,
        }
    }
}

/// `[fixers]` (spec § 2; plan keys C8, C13, C15, C20, C30).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FixersCfg {
    /// C16: the Fixer's share of a settled brokered record.
    pub fixer_cut: f32,
    /// C8: a founder below this lawfulness may open a Fixer's office.
    pub fixer_lawfulness: f32,
    /// C8: … with `persuasion + knowledge` at least this.
    pub fixer_skill: f32,
    /// C10: a regular's stamp lasts this many days.
    pub regular_days: u32,
    /// C13: the gun gate's best skill.
    pub gun_skill_min: f32,
    pub gun_lawfulness: GunLawfulnessCfg,
    /// C34 (phase 3): run orders a Fixer hands out a day.
    pub run_offers_per_day: u8,
    /// C12: the hour of the daily matching.
    pub match_hour: u8,
    /// C12: records a Fixer offers a day.
    pub offers_per_day: u8,
    /// C25 (phase 2): a squad's size.
    pub crew_max: u8,
    /// C8 (plan key, placeholder): open Fixers ≤ residents ÷ this, for NPC founding.
    pub fixers_per_pop: u32,
    /// C13 mode B (plan key, placeholder): the Network goal's flat.
    pub network_flat: f32,
    /// C13, C20 (plan key, placeholder): a Fixer within reach of a Home door.
    pub hire_reach_tiles: u32,
    /// C30 (plan key, placeholder, phase 3): a Fixer owner bribes at this score.
    pub bribe_min: f32,
    /// C15 (plan key, placeholder): a Guard taker's daily post, in hours.
    pub guard_hours: u32,
    /// Round 9 Z2: the records a walk-in reads on a Fixer's board (its book
    /// ranked as the daily match ranks it, the top this many).
    pub board_size: u8,
    /// Round 9 Z2: the Contract goal's flat for a walk-in (a would-be taker
    /// going to the Fixer to take a record off its board).
    pub walk_in_flat: f32,
    /// Round 9 Z2: the minutes a walk-in spends at the desk taking the job.
    pub take_job_mins: u64,
    /// Round 9 Z2: the minutes a Fixer holds a record for a walk-in on his
    /// way (the others reading the board pass it over meanwhile).
    pub walk_in_hold_mins: u32,
}

impl Default for FixersCfg {
    fn default() -> Self {
        FixersCfg {
            fixer_cut: 0.2,
            fixer_lawfulness: 0.35,
            fixer_skill: 0.8,
            regular_days: 14,
            gun_skill_min: 0.45,
            gun_lawfulness: GunLawfulnessCfg::default(),
            run_offers_per_day: 1,
            match_hour: 18,
            offers_per_day: 4,
            crew_max: 4,
            fixers_per_pop: 800,
            network_flat: -0.1,
            hire_reach_tiles: 64,
            bribe_min: 0.5,
            guard_hours: 8,
            board_size: 8,
            walk_in_flat: 0.0,
            take_job_mins: 15,
            walk_in_hold_mins: 180,
        }
    }
}

/// `[missions]` (spec § 3; plan key C23). Phase 1 reads `max_missions`,
/// `squad_below`, `strike_k`, `ledger_death_p`, `ally_cap_n`; the rest are
/// phase 2's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MissionsCfg {
    /// C17, C35: live records at once (the rest queue by price).
    pub max_missions: usize,
    /// C25: a solo estimate below this calls for a squad (phase 1 counts it).
    pub squad_below: f32,
    /// C24: `p_win = logistic(strike_k × (S_crew ÷ S_def − 1))`.
    pub strike_k: f32,
    pub loss_w: f32,
    pub pol_w: f32,
    pub hold_days: u8,
    pub sell_premium: f32,
    pub sell_regard: f32,
    /// C18: a ledger loss below this `p_win` may cost the taker its life.
    pub ledger_death_p: f32,
    /// C23 (plan key, placeholder): allies expected at the target's door.
    pub ally_cap_n: u8,
}

impl Default for MissionsCfg {
    fn default() -> Self {
        MissionsCfg {
            max_missions: 12,
            squad_below: 0.6,
            strike_k: 3.0,
            loss_w: 0.5,
            pol_w: 1.0,
            hold_days: 2,
            sell_premium: 1.5,
            sell_regard: 0.0,
            ledger_death_p: 0.15,
            ally_cap_n: 3,
        }
    }
}

/// `[bounty]` (spec § 3; 16a's three keys, M16b appends the tag keys).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BountyCfg {
    /// C27: coins per paid sighting.
    pub per_sighting: i64,
    /// C27: paid sightings per Locate record.
    pub cap_sightings: u8,
    /// C27: one paid sighting per this many hours.
    pub min_gap_hours: u32,
}

impl Default for BountyCfg {
    fn default() -> Self {
        BountyCfg { per_sighting: 15, cap_sightings: 10, min_gap_hours: 6 }
    }
}

/// `[hitmarket]` (docs/HIT_MARKET.md § 6; every value a placeholder): the
/// organic demand for Hits and Beats, priced at what the guns ask.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HitMarketCfg {
    /// H20: the switch in flight (off: M16a's hire and brain postings at
    /// `quote`).
    pub enabled: bool,
    /// H1: the price is the ask of the `ask_rank`-th cheapest candidate.
    pub ask_rank: usize,
    /// H1: no market price under this (Hit, Beat).
    pub price_floor: KindTable<i64>,
    /// Decision 7: a gun's weekly-wage floor is `max(wage_floor_min,
    /// wage_floor_mult × 7 × the city's median daily wage)`.
    pub wage_floor_min: i64,
    pub wage_floor_mult: f32,
    /// H4: an agent buyer puts this share down and owes the rest.
    pub deposit_share: f32,
    /// H3: a sharing relative's chip-in, a share of its wallet.
    pub kin_share: f32,
    /// H11: organic records open at once (`Contract.motive != None`).
    pub max_open: usize,
    /// H11: new organic records a day, city-wide.
    pub posts_per_day: usize,
    /// H12: a holder under this courage hires even when stronger.
    pub hire_courage: f32,
    /// H6: a stiffed taker's grudge on the buyer, × the unpaid share.
    pub stiff_grudge: f32,
    /// H8: after an organic record closes unfulfilled, the buyer waits this long.
    pub repost_days: u32,
    /// H19: a gang spends at most this share of its treasury on one record.
    pub gang_spend: f32,
    /// H18: a corp spends at most treasury ÷ this on one record.
    pub corp_reserve_mult: f32,
    /// H18: one corp Rival record per corp per this many days.
    pub corp_cooldown_days: u32,
    /// Decision 4: corp Rival Hits on execs, city-wide, per 365 days.
    pub corp_year_cap: u32,
    /// H18: an exec in `[dirty_lobby, dirty_beat)` buys a Beat.
    pub dirty_beat: f32,
    /// H18: a wrong by a rival corp counts this many days.
    pub rival_window_days: u32,
    /// P3 follow-up: an exec's expected defenders add up to this many
    /// private guards of the corps protecting its corp
    /// (`missions::exec_guards`). A stand-in for real bodyguards (roadmap
    /// addendum 13, ambient LOD: execs travel with company); retire it when
    /// those land.
    pub exec_guards: usize,
    // --- Phase 2: silence, debt and the errand.
    /// H8: a want not acted on lapses after this many days.
    pub want_days: u32,
    /// H9: the `Commission` step at the Fixer, in ticks (minutes).
    pub commission_mins: u64,
    /// H9: the Commission goal's flat.
    pub commission_flat: f32,
    /// H17: a Revenge buyer's `Commissioned` memory valence at fulfilment.
    pub closure_val: f32,
    /// Silence (§ 2): a suspect under this lawfulness buys its witness.
    pub silence_lawfulness: f32,
    /// H15: a beaten witness recants at `recant_p × (1 − courage)`.
    pub recant_p: f32,
    /// H14: a dead witness's report goes cold this many days after the death.
    pub cold_days: u32,
    /// Debt (§ 2): an edge debt of at least this many coins …
    pub debt_min: i32,
    /// … standing this many days …
    pub debt_days: u32,
    /// … owed to a creditor under this lawfulness buys a Beat.
    pub debt_lawfulness: f32,
    /// Debt: a Hit when a Debt Beat on the same debtor is this many days old
    /// and the debt still stands.
    pub debt_hit_days: u32,
    // --- The gaps pass (docs/HIT_MARKET.md "Gaps pass").
    /// Reach: an agent's direct Hit or Beat with fewer than `ask_rank` asks
    /// among its own people goes to the open Fixer nearest its Home (any
    /// distance) at that Fixer's market price × (1 + this): the go-between's
    /// premium for a Fixer out of walking reach.
    pub reach_premium: f32,
    /// Lawful buyers wait for the law: a revenge buyer above this lawfulness
    /// whose target is wanted (an open report) or held buys nothing while it lasts.
    pub lawful_wait: f32,
    /// Takers who can't close: a taken organic Hit or Beat still queued for
    /// a live slot (`[missions] max_missions`) this many hours after the take
    /// is worked on the ledger.
    pub queue_hours: u32,
    /// Round 8 X2 (OPEN-247): a solo taker's live run not planned this many
    /// hours (the taker took a post, or the goal always lost) is let go: an
    /// attempt failed with the line, the record back on the board (20k seed
    /// 42: a hit taken on d1 by a gun hired as Club Staff the next midnight
    /// sat seven days unsaid until its buyer was gone). 0 off.
    #[serde(default)]
    pub stall_hours: u32,
}

impl Default for HitMarketCfg {
    fn default() -> Self {
        HitMarketCfg {
            enabled: true,
            ask_rank: 2,
            price_floor: KindTable { hit: 20, beat: 8, ..Default::default() },
            wage_floor_min: 50,
            wage_floor_mult: 0.5,
            deposit_share: 0.5,
            kin_share: 0.5,
            max_open: 16,
            posts_per_day: 3,
            hire_courage: 0.4,
            stiff_grudge: 0.8,
            repost_days: 7,
            gang_spend: 0.5,
            corp_reserve_mult: 5.0,
            corp_cooldown_days: 30,
            corp_year_cap: 2,
            dirty_beat: 0.45,
            rival_window_days: 30,
            exec_guards: 3,
            want_days: 30,
            commission_mins: 30,
            commission_flat: 0.0,
            closure_val: 0.3,
            silence_lawfulness: 0.3,
            recant_p: 0.5,
            cold_days: 3,
            debt_min: 15,
            debt_days: 21,
            debt_lawfulness: 0.35,
            debt_hit_days: 30,
            reach_premium: 0.5,
            lawful_wait: 0.6,
            queue_hours: 24,
            stall_hours: 36,
        }
    }
}
