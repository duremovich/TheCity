use super::*;

/// Rows in a v2 table: 4 phases x 3 lawfulness buckets x 2 hunger buckets.
pub const STAT_ROWS: usize = 24;

/// One row of the calibrated hourly table (M10 v2). The first four are one
/// outcome drawn per hour, as in v1; the rest are independent hourly rolls.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct StatRow {
    /// "Morning law0 hunger1", for humans.
    #[serde(default)]
    pub label: String,
    pub p_eat: f32,
    pub p_work: f32,
    pub p_social: f32,
    pub p_sleep: f32,
    /// Actor side: Market theft beyond the eat path's desperation theft.
    pub p_steal: f32,
    /// Actor side: a Flirt with a known edge of affinity >= 0.3.
    pub p_flirt: f32,
    /// Victim side: the actor is a hole until bound.
    pub p_robbed: f32,
    pub p_assaulted: f32,
    pub p_killed: f32,
    /// Independent roll: the agent meets a stranger from its zone (a new
    /// edge). `calibrate`: half the Full agents' new non-co-worker edge ends
    /// per agent-hour (each meeting gives two agents an edge), pooled over
    /// hunger like the other rolls. Full agents meet in every phase (the
    /// Market, the street, home), not in Social hours: per Social hour the
    /// rate exceeded 1 in every row. 0 in a table without it.
    #[serde(default)]
    pub p_meet: f32,
    /// Independent roll: a Chat (a known edge's drift). `calibrate`: Full
    /// Chats begun per agent-hour, pooled over hunger. A Chat is a few
    /// minutes of an hour some other category dominates, so the Social
    /// outcome (dominant hours) undercounted them by about half.
    #[serde(default)]
    pub p_chat: f32,
    /// Given a Chat: it is with a housemate rather than a random known edge.
    /// `calibrate`: the share of Full Chats begun with someone of the same
    /// Home, pooled over hunger (a Full agent chats with whoever is there,
    /// and at home that is the household).
    #[serde(default)]
    pub p_chat_home: f32,
}

impl StatRow {
    /// A copy without the human label: the hourly tick reads only the
    /// numbers, and cloning the label allocated once per agent-hour.
    /// Exhaustive, so a new field fails to compile until it is copied.
    pub fn numbers(&self) -> StatRow {
        let StatRow {
            label: _,
            p_eat,
            p_work,
            p_social,
            p_sleep,
            p_steal,
            p_flirt,
            p_robbed,
            p_assaulted,
            p_killed,
            p_meet,
            p_chat,
            p_chat_home,
        } = *self;
        StatRow {
            label: String::new(),
            p_eat,
            p_work,
            p_social,
            p_sleep,
            p_steal,
            p_flirt,
            p_robbed,
            p_assaulted,
            p_killed,
            p_meet,
            p_chat,
            p_chat_home,
        }
    }
}

/// Calibrated hourly behaviour table for Statistical agents (`citysim-cli
/// calibrate`, v2). Rows by `phase x 6 + lawfulness bucket x 2 + hunger bucket`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatTable {
    /// 2.
    pub version: u32,
    pub seed: u64,
    pub days: u64,
    pub agents: u32,
    /// `[0.3, 0.7]`: lawfulness `< e0` is bucket 0, `< e1` bucket 1, else 2.
    pub lawfulness_edges: [f32; 2],
    /// `0.4`: hunger `< edge` is bucket 0 (hungry), else 1.
    pub hunger_edge: f32,
    /// The chance a jobless Full agent who starts the Work phase below its
    /// savings line (`SAVINGS_DAYS` meals) collects the dole that day; it
    /// has to walk to the Hall. 1 in a table without it.
    #[serde(default = "one_f32")]
    pub p_dole_day: f32,
    /// The chance an off-screen theft is reported and its thief given a body
    /// (`lod::stat_theft`): Full arrests per Full theft. Off screen that
    /// report is the only road to an arrest (an Assaulted hole files its
    /// witness's report only when bound, at `hole_ttl_days`), and nearly
    /// every one ends in one, so it carries the Full city's whole arrest
    /// rate; the Full share of thefts reported (0.53) put Statistical
    /// arrests 18 % over Full. A table without it falls back to
    /// `[crime] stat_theft_caught_p`.
    #[serde(default)]
    pub p_theft_caught: Option<f32>,
    /// The no-net income path (flip readiness, read only with `econ::wages_on` (P10)):
    /// the chance a jobless adult who is broke (`life::broke`, the Full
    /// Scavenge's and Beg's gate; the Earn goal's savings line before the
    /// parity fix) spends the hour scavenging, per row (phase, lawfulness and
    /// hunger at the hour's start, as `rows`): the share of such Full
    /// agent-hours with a Scavenge begun. A table without it: no off-screen
    /// scavenging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_scavenge: Option<[f32; STAT_ROWS]>,
    /// As `p_scavenge`, for the hours the Full agent begs (`lod::stat_beg`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_beg: Option<[f32; STAT_ROWS]>,
    /// Shadow V4 bug 1: `p_scavenge` and `p_beg` over the hours of a jobless
    /// adult short (`life::earn_short`) but not broke: the near-broke's own
    /// hustle (one rate over both spread the broke's off screen).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_scavenge_short: Option<[f32; STAT_ROWS]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_beg_short: Option<[f32; STAT_ROWS]>,
    /// The no-net need theft (flip readiness, read only with
    /// `econ::wages_on` (P10)): the chance a hungry adult (below `hunger_edge`) who
    /// cannot buy a meal steals one in the hour, per lawfulness bucket.
    /// `calibrate` writes it only in a no-net city: Full thefts by a hungry
    /// thief below the meal price per such agent-hour (those thefts leave
    /// `p_steal`). A table without it: the M10 desperation theft (starving,
    /// whatever the lawfulness).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_desperate: Option<[f32; 3]>,
    /// Parity (the law1 actor share): the hour's theft roll is the broke
    /// adult's alone (below the meal price at the hour's start, the
    /// calibration's `broke`), per row, pooled over hunger as `p_steal`:
    /// Full thefts not `p_desperate`'s by a thief broke at the hour's start,
    /// per such agent-hour. A Full agent with a meal's coins all but never
    /// steals (`steal_can_buy`; seed 2000: 1 of ~240 thefts), while the
    /// rows' `p_steal` is per agent-hour of every agent and, rolled by
    /// everyone off screen, put a third of the lawless bucket's thefts on
    /// agents with the coins. With it the rows' `p_steal` is not read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_steal_broke: Option<[f32; STAT_ROWS]>,
    /// Hunger as sickness § 5 (read only with `[sickness] bins`): the chance
    /// an adult who cannot buy a meal, with a bin in reach (`bins::dive_target`
    /// at the hour's start), dives in the hour, per row. `calibrate` writes it
    /// with the bins on. A table without it: every such hour dives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_dive: Option<[f32; STAT_ROWS]>,
    /// Parity (hunger as sickness, after P5): the chance an agent eats in
    /// the hour, by phase and its hunger at the hour's start (`meal_index`:
    /// 4 phases x `MEAL_BANDS` tenths): Full agent-hours with a meal (hunger
    /// rose) over agent-hours. A Full agent neither eats the hour it gets
    /// hungry nor only when hungry; this is both. A table without it: the M10
    /// rule (every hungry hour eats, `p_eat` the rest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_meal: Option<Vec<f32>>,
    /// Diet parity (the off-screen grocery run): the Full city's pantry
    /// meals per resident (the daily mean over inhabited Homes at midnight),
    /// the level `lod::stat_grocery` restocks a household to. A table
    /// without it: a shift's worth per resident.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pantry_per_resident: Option<f32>,
    /// P5f (wage parity): by role (`{role:?}`), the share of a Full
    /// worker's free, healthy workday shift minutes spent at the work
    /// itself (the commute, a meal, a sleep run over, the guard's patrol
    /// counted as work). A Statistical worker's shift hour is credited on
    /// it, and the wage paid pro rata on the hours credited. A role not
    /// listed, or a table without it: every hour (the M10 rule).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_attend: Option<AttendTable>,
    /// P5f: Full's wage share of wages owed over its attendance share (a
    /// cut shift past half is paid pro rata, a whole one in full): the
    /// Statistical pro rata wage x this, capped at the whole wage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_wage_ratio: Option<f32>,
    /// Transit phase 3 (parity): the share of the Full city's jumped gates
    /// that drew a Fare Evasion sentence; a Statistical commuter's jumped
    /// leg is reported on it (`transit::stat_commute`). A table without it:
    /// no off-screen dodger is caught.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_dodge_caught: Option<f32>,
    /// Parity (commute rides): a Statistical commute's `transit::choose`
    /// reads the trip from the Home door, but a third of the Full city's
    /// commutes set out from somewhere else (a Market, a Bar), and those ride
    /// more. Learned against the same call: of the Full first commutes of a
    /// workday whose Home-to-work `choose` at the start would walk (`[0]`) or
    /// ride (`[1]`), the share that rode. A Statistical commute rides on
    /// its cell's share (a pure hash of the agent and the day); a walk the
    /// model refused rides the Home-to-work route when the rider can pay or
    /// would jump the gate. A table without it: the model's own answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_commute_ride: Option<[f32; 2]>,
    /// Parity (arrests): the El's rides that are not a commute (the first
    /// ride to work of a workday and the first ride Home after it), per
    /// agent-hour of an adult who could pay the fare or would jump the gate
    /// at the hour's start, per row. Off screen only commutes rode, while
    /// these were ~55 % of the Full city's boardings and ~70 % of its jumped
    /// gates (seed 2000), so its Fare Evasion arrests had no off-screen
    /// counterpart. `transit::stat_errand` rides on it: the fare, or the gate
    /// jumped and reported at `p_dodge_caught`. A table without it: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_errand_ride: Option<[f32; STAT_ROWS]>,
    /// Jobs & lives (spec R5, the gate's one regen): of the Full city's
    /// pitch-hours due (a Pitch in its hours, its owner free and well), the
    /// share the owner was at the spot. A Statistical owner works the hour on
    /// it (`streetwork::working`, a pure hash of the owner and the hour). A
    /// table without it: every hour due (the phase-2 rule).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_pitch: Option<f32>,
    /// Jobs & lives phase 7b (the parity mirror of paid leisure): per venue
    /// kind (`leisure::venue_key`), the Full tier's paid visits of the kind
    /// per agent-day eligible for it at the evening (`leisure::evening_kinds`:
    /// a venue of the kind open, affordable and within the walk cap of the
    /// agent's day; may pass 1: a Full visit of the kind at an hour the
    /// evening's purse no longer affords). The Statistical evening takes
    /// each eligible kind on it (`leisure::stat_daily`: its whole part in
    /// visits, the rest a chance). A table without it: the best rung.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_venue: Option<BTreeMap<String, f32>>,
    pub rows: Vec<StatRow>,
}

fn one_f32() -> f32 {
    1.0
}

/// `StatTable::p_attend`'s commute bands (`transit::BANDS`' five).
const ATTEND_BANDS: usize = 5;

/// P5f and transit T14: `StatTable::p_attend`, the learned attendance by
/// role (`"{role:?}"`) and by role and commute band (`"{role:?}/b{n}"`).
/// The table file keeps the readable string map; the cells are parsed once
/// into role and band so `lod::stat_attend` looks a worker's hour up with
/// no formatted key (it ran per Statistical worker-hour on shift).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "BTreeMap<String, f32>", into = "BTreeMap<String, f32>")]
pub struct AttendTable {
    /// The cells as written (the file's order).
    cells: BTreeMap<String, f32>,
    /// `Role::ALL`'s roles: the role's row, then bands 0-4.
    base: Vec<[Option<f32>; 1 + ATTEND_BANDS]>,
    /// A trade's (`Trade("key")`), by its key.
    trades: BTreeMap<String, [Option<f32>; 1 + ATTEND_BANDS]>,
    /// The mean over the roles' own rows (not the banded ones), summed in
    /// the file's order; `None` empty.
    mean: Option<f32>,
}

impl From<BTreeMap<String, f32>> for AttendTable {
    fn from(cells: BTreeMap<String, f32>) -> AttendTable {
        let mut base = vec![[None; 1 + ATTEND_BANDS]; Role::ALL.len()];
        let mut trades: BTreeMap<String, [Option<f32>; 1 + ATTEND_BANDS]> = BTreeMap::new();
        for (k, &p) in &cells {
            let (role, slot) = match k.rsplit_once("/b") {
                Some((r, b)) => match b.parse::<usize>() {
                    Ok(b) if b < ATTEND_BANDS => (r, b + 1),
                    _ => continue,
                },
                None => (k.as_str(), 0),
            };
            if let Some(key) = role.strip_prefix("Trade(\"").and_then(|r| r.strip_suffix("\")")) {
                trades.entry(key.to_string()).or_insert([None; 1 + ATTEND_BANDS])[slot] = Some(p);
            } else if let Some(i) = Role::ALL.iter().position(|r| format!("{r:?}") == role) {
                base[i][slot] = Some(p);
            }
        }
        let roles: Vec<f32> = cells.iter().filter(|(k, _)| !k.contains('/')).map(|(_, &v)| v).collect();
        let mean = (!cells.is_empty()).then(|| roles.iter().sum::<f32>() / roles.len().max(1) as f32);
        AttendTable { cells, base, trades, mean }
    }
}

impl From<AttendTable> for BTreeMap<String, f32> {
    fn from(t: AttendTable) -> BTreeMap<String, f32> {
        t.cells
    }
}

impl AttendTable {
    fn row(&self, role: Role) -> Option<&[Option<f32>; 1 + ATTEND_BANDS]> {
        match role {
            Role::Trade(t) => self.trades.get(t.key()),
            r => Role::ALL.iter().position(|&x| x == r).and_then(|i| self.base.get(i)),
        }
    }

    /// The role's own row (`band` `None`) or its row in commute band `band`.
    pub fn get(&self, role: Role, band: Option<usize>) -> Option<f32> {
        let slot = match band {
            Some(b) if b < ATTEND_BANDS => b + 1,
            Some(_) => return None,
            None => 0,
        };
        self.row(role)?[slot]
    }

    /// Does the table hold any banded row for `role`?
    pub fn banded(&self, role: Role) -> bool {
        self.row(role).is_some_and(|r| r[1..].iter().any(Option::is_some))
    }

    /// The mean over the roles' own rows; `None` for an empty table.
    pub fn mean(&self) -> Option<f32> {
        self.mean
    }

    /// The cells as written (`"{role:?}"`, `"{role:?}/b{n}"`).
    pub fn cells(&self) -> &BTreeMap<String, f32> {
        &self.cells
    }
}

/// `StatTable::p_meal`'s hunger bands (tenths of `[0, 1]`).
pub const MEAL_BANDS: usize = 10;
/// `StatTable::p_meal`'s cells: 4 phases x `MEAL_BANDS`.
pub const MEAL_CELLS: usize = 4 * MEAL_BANDS;
/// Diet parity (after items 8-13): `p_meal` learned apart for the hours a
/// worker starts on shift (cells `MEAL_CELLS..2 x MEAL_CELLS`): a Full
/// worker on shift defers its meal (item 5, P5i's meal before the shift),
/// the off-shift and the jobless eat when hungry. A table of `MEAL_CELLS`
/// cells (before the split) reads the pooled rate for both.
pub const MEAL_CELLS_SPLIT: usize = 2 * MEAL_CELLS;

impl StatTable {
    /// Morning 0, Work 1, Evening 2, Night 3 (`DayPhase` declares Night first).
    pub fn phase_index(phase: time::DayPhase) -> usize {
        match phase {
            time::DayPhase::Morning => 0,
            time::DayPhase::Work => 1,
            time::DayPhase::Evening => 2,
            time::DayPhase::Night => 3,
        }
    }

    pub fn lawfulness_bucket(&self, lawfulness: f32) -> usize {
        if lawfulness < self.lawfulness_edges[0] {
            0
        } else if lawfulness < self.lawfulness_edges[1] {
            1
        } else {
            2
        }
    }

    pub fn hunger_bucket(&self, hunger: f32) -> usize {
        usize::from(hunger >= self.hunger_edge)
    }

    /// `p_meal`'s cell: the phase, and the hunger's band of `[0, 1]` in
    /// `MEAL_BANDS` equal bands.
    pub fn meal_index(&self, phase: time::DayPhase, hunger: f32) -> usize {
        let band = (hunger.max(0.0) * MEAL_BANDS as f32).floor() as usize;
        Self::phase_index(phase) * MEAL_BANDS + band.min(MEAL_BANDS - 1)
    }

    /// `meal_index` in the split table (`MEAL_CELLS_SPLIT`): on shift at
    /// the hour's start, the second half.
    pub fn meal_index_split(&self, phase: time::DayPhase, hunger: f32, on_shift: bool) -> usize {
        self.meal_index(phase, hunger) + if on_shift { MEAL_CELLS } else { 0 }
    }

    /// The `p_meal` rate for an hour (the split table's cell when the table
    /// holds one, else the pooled cell).
    pub fn meal_rate(&self, phase: time::DayPhase, hunger: f32, on_shift: bool) -> Option<f32> {
        let p = self.p_meal.as_ref()?;
        let i = if p.len() >= MEAL_CELLS_SPLIT {
            self.meal_index_split(phase, hunger, on_shift)
        } else {
            self.meal_index(phase, hunger)
        };
        p.get(i).copied()
    }

    /// `phase x 6 + lawfulness bucket x 2 + hunger bucket`.
    pub fn index(&self, phase: time::DayPhase, lawfulness: f32, hunger: f32) -> usize {
        Self::phase_index(phase) * 6 + self.lawfulness_bucket(lawfulness) * 2 + self.hunger_bucket(hunger)
    }

    pub fn row(&self, phase: time::DayPhase, lawfulness: f32, hunger: f32) -> &StatRow {
        &self.rows[self.index(phase, lawfulness, hunger)]
    }

    /// "Morning law0 hunger1" for row `i`.
    pub fn label(i: usize) -> String {
        let phase = ["Morning", "Work", "Evening", "Night"][i / 6];
        format!("{phase} law{} hunger{}", (i % 6) / 2, i % 2)
    }
}

/// Read `assets/stat_table.toml`: `(table, legacy)`. Missing gives `(None,
/// false)`; the pre-M10 four-row table `(None, true)` (M10 D28); a v2 table
/// with the wrong row count, or anything unparsable, panics.
pub fn load_stat_table(config: &Config) -> (Option<StatTable>, bool) {
    let path = config.asset("stat_table.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (None, false),
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    };
    match toml::from_str::<StatTable>(&text) {
        Ok(t) => {
            assert!(
                t.rows.len() == STAT_ROWS,
                "{}: {} rows, expected {STAT_ROWS}: run `cargo run --release -p citysim-cli -- calibrate`",
                path.display(),
                t.rows.len()
            );
            (Some(t), false)
        }
        Err(_) if text.contains("[morning]") => (None, true),
        Err(e) => panic!("bad {}: {e}", path.display()),
    }
}

/// `assets/stat_mlp.toml` under `[lod] policy = "mlp"`, else `None`.
pub(super) fn load_stat_mlp(config: &Config) -> Option<crate::systems::stat_policy::StatMlp> {
    match config.lod.policy.as_str() {
        "table" => None,
        "mlp" => Some(crate::systems::stat_policy::StatMlp::load(config)),
        other => panic!("[lod] policy = {other:?}: expected \"table\" or \"mlp\""),
    }
}
