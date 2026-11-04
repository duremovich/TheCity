use super::*;

/// A gang's hold on a Home: `count` extortions by `gang`; at 3 the Home is territory.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Claim {
    pub gang: EntityId,
    pub count: u8,
}

// ---------------------------------------------------------------------------
// Agent components
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub tile: TilePos,
    /// `Some` if inside a building rect (including its door).
    pub building: Option<EntityId>,
    /// The tick the agent entered `building` (co-location is measured from here).
    #[serde(default)]
    pub entered: Tick,
    /// The city milestone C6a: the storey the body stands on inside
    /// `building` (0 the ground, the street's; `World::floor_in` sets it on
    /// entry from the tenancy's `span`, his flat's floor at home). 0 on
    /// the street until C7 gives the street a z.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub floor: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identity {
    /// `"<First> <Last>"` from the name tables.
    pub name: String,
    /// `0..=12000` (100 years); +1 per day.
    pub age_days: u32,
    pub sex: Sex,
    /// Negative for the initial population.
    pub born_tick: i64,
    /// Widowed at this tick (Grief; the Spouse edge stays).
    #[serde(default)]
    pub spouse_died_tick: Option<Tick>,
}

/// Under 18: Statistical only, no Brain or Needs; fed from the Home pantry.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Child {
    /// Consecutive days the pantry could not feed them; three kills.
    pub hunger_days: u8,
    /// Jobs & lives phase 7 (§ 6.1, D11): today's routine (`Play` and not
    /// written with `[ages]` off).
    #[serde(default, skip_serializing_if = "Routine::is_play")]
    pub routine: Routine,
    /// School days attended; Street days running.
    #[serde(default, skip_serializing_if = "is_zero_u16_child")]
    pub edu_days: u16,
    #[serde(default, skip_serializing_if = "is_zero_u16_child")]
    pub street_days: u16,
    /// Lives at work phase 3 (§ 2.2, teacher): the School days taught, each
    /// `teacher_floor + (1 − teacher_floor) × the teacher's knowledge`; the
    /// education at maturity under `[service] civic` (`edu_days` off).
    #[serde(default, skip_serializing_if = "is_zero_f32_child")]
    pub edu_taught: f32,
    /// Round 4 T1 (OPEN-231): the day the service came to the door over
    /// this child's unfed days (the warning); neglect again inside `[camp]
    /// warn_repeat_days` (and past `warn_grace_days`) takes the child. Set
    /// too on a child let home (`camp::let_home`: home on a warning).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warned_day: Option<u32>,
    /// Round 4 T1: in a Camp, the consecutive midnights a parent's Home
    /// held `[camp] home_pantry_days` of the children's food (the let-home
    /// count, `[camp] let_home_days`).
    #[serde(default, skip_serializing_if = "is_zero_u16_child")]
    pub home_fed_days: u16,
    /// Round 4 T2 (OPEN-232): today's School (the roll it is on, `Routine::
    /// School`), or the School that turned it away or was too dear (`school_
    /// miss` `Full` or `Purse`); set at midnight by `ages::children`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub school: Option<crate::entity::EntityId>,
    /// Round 4 T2: why a school-age child is not at School on a school day
    /// (the parent's `[family]` line reads it); `None` at School, too young,
    /// on a rest day, in a Camp or on the street.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub school_miss: Option<SchoolMiss>,
    /// Round 6 V1 (OPEN-242): the Camp this child slipped out of and the
    /// day (`camp::escape`); the next midnight the service fetches it back
    /// (`camp::fetch_daily`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slipped: Option<(crate::entity::EntityId, u32)>,
    /// Round 7 W5 (`OPEN-237`): on a `Purse` miss, the payer's coins in hand
    /// at the midnight pick and what of them was kept back (the rent's week
    /// and the meals, `leisure::spendable`); the `[family]` line reads the
    /// purse the pick read, not the one the morning's wage refilled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub school_purse: Option<(i64, i64)>,
}

/// Round 4 T2 (OPEN-232): a school-age child kept home on a school day.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SchoolMiss {
    /// No School within `[ages] school_trip_ticks`, on foot or by the El.
    Far,
    /// Every School within it is full (its roll at `school_places_per_
    /// teacher` x its teachers; one with no teacher takes no one).
    Full,
    /// One has room, but the fee (and the El's fares, for one past the walk)
    /// is beyond the household's purse.
    Purse,
}

fn is_zero_u16_child(v: &u16) -> bool {
    *v == 0
}

fn is_zero_f32_child(v: &f32) -> bool {
    *v == 0.0
}

/// Jobs & lives phase 7 (§ 6.1, D11): a child's day, chosen at midnight
/// from the household's state.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub enum Routine {
    #[default]
    Play,
    School,
    Chores,
    Street,
    Camp,
}

impl Routine {
    pub fn is_play(&self) -> bool {
        *self == Routine::Play
    }
}

impl Identity {
    pub fn age_years(&self) -> u32 {
        self.age_days / crate::time::DAYS_PER_YEAR as u32
    }
}

/// All `f32` in `0.0..=1.0`; 1.0 = fully satisfied, 0.0 = critical.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Needs {
    pub hunger: f32,
    pub energy: f32,
    pub safety: f32,
    /// Derived hourly: `clamp(coins / (7 * price_food * (1 + greed)), 0, 1)`.
    pub wealth: f32,
    pub belonging: f32,
    pub intimacy: f32,
    /// `Some(t)` while `hunger == 0.0`.
    pub starving_since: Option<Tick>,
    /// L2 § 2 (plan L13): 1.0 at seed and in a pre-L2 save; only
    /// `systems::leisure` writes it (with `leisure::on`), so a city with L2
    /// off saves without it.
    #[serde(default = "one_f32", skip_serializing_if = "is_one_f32")]
    pub fun: f32,
    /// Jobs & lives phase 4 (§ 9): 1.0 at seed and in an older save; only
    /// `systems::fashion` writes it (with `[fashion]` on).
    #[serde(default = "one_f32", skip_serializing_if = "is_one_f32")]
    pub status: f32,
}

/// All `f32` in `0.0..=1.0`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Personality {
    pub lawfulness: f32,
    pub greed: f32,
    pub pride: f32,
    pub sociability: f32,
    pub courage: f32,
    pub loyalty: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mood {
    /// `-1.0..=1.0`, default 0.0.
    pub value: f32,
    /// `Some` while `value < -0.8`.
    pub low_since: Option<Tick>,
    pub last_computed: Tick,
}

impl Default for Mood {
    fn default() -> Self {
        Mood { value: 0.0, low_since: None, last_computed: 0 }
    }
}

/// `coins >= 0` for agents; debts live on edges.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wallet {
    pub coins: i64,
}

/// `food 0..=20` (carry cap 20); `stolen_food <= food`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub food: u32,
    pub stolen_food: u32,
    /// M13 D7: stim doses carried (load 1 each).
    #[serde(default)]
    pub stims: u16,
    /// M13 D7: Parts carried (load 3 each).
    #[serde(default)]
    pub parts: u16,
    /// Hunger as sickness § 1: the synth meals among `food` (a subset, read
    /// through `synth_held`: removals take real units first, so a stale
    /// count is clamped, never trusted).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub synth: u32,
}

impl Inventory {
    /// The synth meals carried (`synth` clamped to `food`).
    pub fn synth_held(&self) -> u32 {
        self.synth.min(self.food)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    /// Building entity.
    pub employer: Option<EntityId>,
    pub role: Role,
    pub wage_per_day: i64,
    /// `tick_of_day` ranges `[start, end)`; default `[(540, 1080)]`.
    pub shifts: Vec<(u16, u16)>,
    /// Quits at 7.
    pub days_unpaid: u8,
    pub tax_accum: f32,
    /// Key of the last shift worked (see `shift_key_at`); one shift per key.
    #[serde(default)]
    pub last_shift_day: Option<i64>,
    /// Day of the last CollectWage that the Treasury could not pay in full;
    /// one such visit per day, so a short Treasury is not hammered.
    #[serde(default)]
    pub last_wage_attempt_day: Option<u64>,
    /// Guards: ticks of the running shift spent on law duty (Patrol, Arrest,
    /// or Work on a Jail day); `law::credit_guard_shifts` reads and clears it
    /// when the shift ends.
    #[serde(default)]
    pub duty_ticks: u16,
    /// M11 D17: when hired (seeded jobs 0); Hunker fires the newest first.
    #[serde(default)]
    pub hired_tick: Tick,
    /// M11 D35: the shift key a strike struck (`classes::walk_out`): no
    /// work, no wage, a guard's shift clock included.
    #[serde(default)]
    pub struck_shift: Option<i64>,
    /// M15 W29: a poached hire's wage multiplier (`poach_premium`), read at
    /// `economy::collect_wage`; 1 otherwise (a new Job resets it).
    #[serde(default = "one_f32", skip_serializing_if = "is_one_f32")]
    pub premium: f32,
    /// L2 fix round: this job has paid a wage (read by `gang::desist`'s
    /// employed factor). A hire with `jobs::on` starts false; with L2 off,
    /// and in a pre-L2 save, it is true from the start. It no longer gates
    /// the dole (roadmap addendum 17: a Job holder never draws it).
    #[serde(default = "true_", skip_serializing_if = "is_true")]
    pub paid_once: bool,
    /// L2 shadow fixes item 10: a guard's duty fixed for a shift key
    /// (`(key, jail)`), set as the key comes due (`law::credit_guard_shifts`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duty_fixed: Option<(i64, bool)>,
    /// Class pay (docs/CLASS_PAY.md): the post's grade (0 labour, 1
    /// skilled, 2 professional, 3 management), fixed in `wage_per_day` at
    /// hire or promotion; always 0 with class pay off (not written).
    #[serde(default, skip_serializing_if = "is_zero_grade")]
    pub grade: u8,
    /// Jobs & lives phase 1 (§ 1, the `day` model): a one-day post ends
    /// at this tick (`trades::end_day_posts`); `None` for a standing post.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<Tick>,
    /// Lives at work phase 6 (§ 4.2): the seeded seniority in days (0 for
    /// any hire after day 0, and with `[careers]` off; not written at 0).
    /// Experience is this plus the days since `hired_tick`.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub seniority_days: u16,
    /// Lives at work phase 6 (§ 4.2): the building's foreman (set at
    /// midnight, `careers::daily`): the wage × `[careers] foreman_premium`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub foreman: bool,
    /// Lives at work 8d (OPEN-123, § 2.1): the coins tipped to a Brothel
    /// worker at her house this shift; `service::shift_end` takes the
    /// madam's `[service] madam_cut` of the total once and clears it (both
    /// tiers). Not written at 0.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub shift_tips: i64,
}

impl Job {
    /// Lives at work phase 6 (§ 4.2): the tick experience counts from
    /// (`hired_tick` less the seniority; `hired_tick` itself with none):
    /// the earlier, the more experienced.
    pub fn start_key(&self) -> i64 {
        self.hired_tick as i64 - i64::from(self.seniority_days) * crate::time::TICKS_PER_DAY as i64
    }
}

fn is_zero_grade(v: &u8) -> bool {
    *v == 0
}

fn true_() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

impl Job {
    pub fn on_shift(&self, tick_of_day: u16) -> bool {
        self.shifts.iter().any(|&(s, e)| tick_of_day >= s && tick_of_day < e)
    }

    /// Is a Hall visit for wages worthwhile today: wages owed, and no
    /// short payment yet today.
    pub fn wage_collectable(&self, day: u64) -> bool {
        self.days_unpaid >= 1 && self.last_wage_attempt_day != Some(day)
    }

    /// Ticks until the next shift start, or 0 if on shift now.
    pub fn ticks_until_shift(&self, tick_of_day: u16) -> u16 {
        if self.on_shift(tick_of_day) {
            return 0;
        }
        self.shifts
            .iter()
            .map(|&(s, _)| if s > tick_of_day { s - tick_of_day } else { 1440 - tick_of_day + s })
            .min()
            .unwrap_or(0)
    }

    /// The day that owns the shift containing `tick`. A segment starting at 0
    /// that continues a 1440-ending segment (night guards) belongs to the
    /// previous day, so the 21:00-06:00 shift has one key, not two.
    pub fn shift_key_at(&self, tick: Tick) -> i64 {
        let day = (tick / 1440) as i64;
        let tod = (tick % 1440) as u16;
        let in_tail = self.shifts.iter().any(|&(s, e)| s == 0 && tod < e);
        let wraps = self.shifts.iter().any(|&(_, e)| e == 1440);
        if in_tail && wraps {
            day - 1
        } else {
            day
        }
    }

    /// Review fix 3: is `tick_of_day` within the first hour of one of the
    /// Job's shifts? A segment starting at 0 that continues a 1440-ending
    /// segment (the night guard's 00:00, as [`Job::shift_key_at`] and
    /// [`Job::shift_end`] read it) is the same shift, not a new one. The
    /// madam's headcount, the School roster, the Statistical round and the
    /// reporter's beat read this.
    pub fn first_hour(&self, tick_of_day: u16) -> bool {
        let wraps = self.shifts.iter().any(|&(_, e)| e == 1440);
        let hour = crate::time::TICKS_PER_HOUR as u32;
        self.shifts
            .iter()
            .any(|&(s, _)| !(s == 0 && wraps) && tick_of_day >= s && u32::from(tick_of_day) < u32::from(s) + hour)
    }

    /// Key of the shift in progress, or of the next one to start.
    pub fn next_shift_key(&self, tick: Tick) -> i64 {
        let tod = (tick % 1440) as u16;
        self.shift_key_at(tick + Tick::from(self.ticks_until_shift(tod)))
    }

    /// Absolute tick at which the shift containing `tick` ends. A shift that
    /// runs to 1440 and continues from 0 (night guards) is one shift.
    pub fn shift_end(&self, tick: Tick) -> Option<Tick> {
        let tod = (tick % 1440) as u16;
        let &(_, end) = self.shifts.iter().find(|&&(s, e)| tod >= s && tod < e)?;
        let mut until = tick + Tick::from(end - tod);
        if end == 1440 {
            if let Some(&(_, e2)) = self.shifts.iter().find(|&&(s, _)| s == 0) {
                until += Tick::from(e2);
            }
        }
        Some(until)
    }
}

/// `None` = homeless.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Household {
    pub home: Option<EntityId>,
    /// Lives at work phase 2 (§ 3.4): a parent holds a newborn until this
    /// tick; `newborn_carer` names the parent whose nights and away spots are
    /// damped. Not written while unset (the off save).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newborn_until: Option<Tick>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub newborn_carer: bool,
    /// M11 D6: this adult's share of the Home's rent, accrued daily; the
    /// integer part is due at midnight.
    #[serde(default)]
    pub rent_due: f32,
    /// M11 D7: consecutive midnights a due rent went short; eviction at `evict_days`.
    #[serde(default)]
    pub arrears: u8,
    /// M11 D7: `(owner, tick)` of the last eviction; that owner refuses the
    /// agent for `[rent] refuse_days`.
    #[serde(default)]
    pub evicted_by: Option<(Option<EntityId>, Tick)>,
    /// Rent paid per day, `(day, coins)`, the last seven days only (the
    /// inspector's "paid N in 7 days"; `rent_paid_7d`).
    #[serde(default)]
    pub rent_paid_log: VecDeque<(u64, i64)>,
    /// M11 § 7: consecutive midnights as a Dreg with mood below
    /// `[classes] dreg_emigrate_mood`; at `dreg_emigrate_days` they leave.
    #[serde(default)]
    pub miserable_days: u8,
    /// M12 D27: put out of this squat; banned from it until the tick.
    #[serde(default)]
    pub squat_ban: Option<(EntityId, Tick)>,
    /// M12 D28: put on the street without an eviction (a seeded derelict,
    /// a Block gone derelict); the re-housing wait runs from here as from
    /// an eviction. Cleared when housed.
    #[serde(default)]
    pub homeless_since: Option<Tick>,
    /// Round 6 V1 (OPEN-231): the food units this adult stored in his
    /// Home's pantry (`StoreFood`), decayed a day by `1 / [camp]
    /// shelf_days`, his household's children's meals taken off it. In a
    /// shared Home his household's claim on the pantry is its stake's share
    /// (`shelf::claim`).
    #[serde(default, skip_serializing_if = "is_zero_f32_hh")]
    pub pantry_stake: f32,
    /// Round 6 V1: his household under this roof (the lowest id of the kin
    /// living here, `shelf::daily` at midnight); `None`: on his own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kin: Option<EntityId>,
    /// The city milestone C6a: his flat, a unit of his Home's span: `(the
    /// Home, the flat's index)`, `0..` the Home's household slots (its
    /// `floors` field), shared with the kin under his roof
    /// (`World::assign_flat`, at the move-in or the first entry); its
    /// storey is `floors::flat_floor`. A flat naming another Home is stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flat: Option<(EntityId, u16)>,
}

fn is_zero_f32_hh(v: &f32) -> bool {
    *v == 0.0
}

impl Household {
    pub fn new(home: Option<EntityId>) -> Household {
        Household {
            home,
            newborn_until: None,
            newborn_carer: false,
            rent_due: 0.0,
            arrears: 0,
            evicted_by: None,
            rent_paid_log: VecDeque::new(),
            miserable_days: 0,
            squat_ban: None,
            homeless_since: None,
            pantry_stake: 0.0,
            kin: None,
            flat: None,
        }
    }

    /// Rent paid on `today` and the six days before.
    pub fn rent_paid_7d(&self, today: u64) -> i64 {
        self.rent_paid_log.iter().filter(|&&(d, _)| d + 7 > today).map(|&(_, c)| c).sum()
    }

    /// Record `coins` of rent paid on `today`; days older than a week drop.
    pub fn note_rent_paid(&mut self, today: u64, coins: i64) {
        if coins <= 0 {
            return;
        }
        while self.rent_paid_log.front().is_some_and(|&(d, _)| d + 7 <= today) {
            self.rent_paid_log.pop_front();
        }
        match self.rent_paid_log.back_mut() {
            Some((d, c)) if *d == today => *c += coins,
            _ => self.rent_paid_log.push_back((today, coins)),
        }
    }
}
