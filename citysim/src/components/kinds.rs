use super::*;

/// Ordered by severity (`Crime::severity`), not by declaration: Vagrancy
/// (M12 D15, appended so saves keep their variant names) is the least.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Crime {
    Theft,
    Extortion,
    Assault,
    Murder,
    /// M12 D15: sleeping rough where the law sweeps.
    Vagrancy,
    /// M13 D26: a vehicle stolen (or a botched attempt).
    GrandTheft,
    /// M13 D25: a crash that killed, the driver seen.
    Manslaughter,
    /// M13 D36: a Harvest crew dragged someone off for their chrome.
    Abduction,
    /// M13 D38: a dealer's sale of illegal Stims.
    Dealing,
    /// M14 V18: a traced run on a node (no Data or coins taken).
    Intrusion,
    /// M14 V18: a traced run for Data or a Ledger.
    DataTheft,
    /// M16a (plan C29): the placing agent of a fulfilled contract record,
    /// named to the law (phase 3 files it); ranks just below Murder.
    Conspiracy,
    /// Transit (docs/TRANSIT.md T10, Dylan 2026-10-10): jumping the El's
    /// gate. Petty: a night in the cells as Vagrancy's, ranked just above it.
    FareEvasion,
    /// M16b § 4 (phase 1): a renege or a fake job, filed by its lawful
    /// victim; ranks just above Theft.
    Fraud,
    /// M16b § 4 (phase 1): a coercion on a secret, filed by the coerced
    /// on a failed move; ranks just above Shakedown.
    Blackmail,
    /// Jobs & lives phase 3 (§ 4, D16): a dealer's sale of a weapon.
    ArmsDealing,
    /// Jobs & lives phase 3 (§ 4, D16): a weapon carried without a licence,
    /// seen by a guard. Petty: a night in the cells when it is the worst
    /// open report.
    UnlicensedArms,
    /// Jobs & lives phase 3 (§ 4, D16): a crate seized at customs, filed
    /// on the receiving gang's leader.
    Smuggling,
    /// Jobs & lives phase 4 (§ 5.1, D16): a street pitch worked where the
    /// `street_legal` lever is off, filed at a sweep. Petty (a night in the
    /// cells), just above Fare Evasion.
    Solicitation,
    /// Jobs & lives phase 5 (§ 5.2, D16): a captive fitted with a behaviour
    /// chip at a gang's holding, filed on its members; just above Abduction.
    Trafficking,
    /// Jobs & lives phase 6 (§ 7, D16): fake papers exposed at an arrest,
    /// filed on the holder and the forger; just above Fraud.
    Forgery,
    /// Jobs & lives phase 6 (§ 7, D16): a black clinic found by the law,
    /// filed on its staff; just above Dealing.
    BlackMarket,
    /// Pressure pass round 7, W5 (`OPEN-237`): a parent who broke his own
    /// child out of a work Camp; the warden's matter, not an Abduction.
    /// Just above Theft.
    CampEscape,
}

impl Crime {
    /// The display name (M11 section 1).
    pub fn label(self) -> &'static str {
        match self {
            Crime::Theft => "Theft",
            Crime::Extortion => "Shakedown",
            Crime::Assault => "Assault",
            Crime::Murder => "Murder",
            Crime::Vagrancy => "Vagrancy",
            Crime::GrandTheft => "Grand Theft",
            Crime::Manslaughter => "Manslaughter",
            Crime::Abduction => "Abduction",
            Crime::Dealing => "Dealing",
            Crime::Intrusion => "Intrusion",
            Crime::DataTheft => "Data Theft",
            Crime::Conspiracy => "Conspiracy",
            Crime::FareEvasion => "Fare Evasion",
            Crime::Fraud => "Fraud",
            Crime::Blackmail => "Blackmail",
            Crime::ArmsDealing => "Arms Dealing",
            Crime::UnlicensedArms => "Unlicensed Arms",
            Crime::Smuggling => "Smuggling",
            Crime::Solicitation => "Solicitation",
            Crime::Trafficking => "Trafficking",
            Crime::Forgery => "Forgery",
            Crime::BlackMarket => "Black Market",
            Crime::CampEscape => "Camp Escape",
        }
    }

    /// The order the law ranks crimes by (the most severe open report sets
    /// a sentence): Vagrancy, Theft, Intrusion, Grand Theft, Data Theft,
    /// Dealing, Shakedown, Assault, Manslaughter, Abduction, Conspiracy,
    /// Murder (M14 V18 inserts Intrusion above Theft and Data Theft above
    /// Grand Theft; the M13 crimes keep their relative order; M16a C29
    /// inserts Conspiracy just below Murder, which moves 10 -> 11). Unique
    /// per crime (`Ord` reads it).
    pub fn severity(self) -> u8 {
        match self {
            Crime::Vagrancy => 0,
            // Transit (T10): just above Vagrancy; everything above moves up one.
            Crime::FareEvasion => 1,
            // Jobs & lives phase 4 (§ 5.1): just above Fare Evasion;
            // everything above moves up one.
            Crime::Solicitation => 2,
            // Jobs & lives phase 3 (§ 4), ranked petty at the phase-8 review:
            // sentenced as Vagrancy, so above Theft it shortened every
            // worse crime's term it outranked (and fined a suspect in a full
            // Jail); everything from Theft to BlackMarket moves up one.
            Crime::UnlicensedArms => 3,
            Crime::Theft => 4,
            // Round 7 W5: a Camp escape just above Theft; everything above
            // moves up one.
            Crime::CampEscape => 5,
            // M16b: Fraud just above Theft, Blackmail just above Shakedown;
            // everything above each moves up (the order is all that is read).
            Crime::Fraud => 6,
            // Jobs & lives phase 6 (§ 7): Forgery just above Fraud,
            // BlackMarket just above Dealing; everything above moves up.
            Crime::Forgery => 7,
            Crime::Intrusion => 8,
            Crime::GrandTheft => 9,
            Crime::DataTheft => 10,
            Crime::Dealing => 11,
            Crime::BlackMarket => 12,
            // Jobs & lives phase 3 (§ 4): the other arms crimes between
            // Dealing and Shakedown.
            Crime::ArmsDealing => 13,
            Crime::Smuggling => 14,
            Crime::Extortion => 15,
            Crime::Blackmail => 16,
            Crime::Assault => 17,
            Crime::Manslaughter => 18,
            Crime::Abduction => 19,
            // Jobs & lives phase 5 (§ 5.2): just above Abduction; everything
            // above moves up one.
            Crime::Trafficking => 20,
            Crime::Conspiracy => 21,
            Crime::Murder => 22,
        }
    }
}

impl PartialOrd for Crime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Crime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.severity().cmp(&other.severity())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum DeathCause {
    Starvation,
    Violence,
    OldAge,
    Execution,
    /// M13 D47: a crash (phase 2).
    Accident,
    /// M13 D47: a stim overdose (phase 4).
    Overdose,
    /// M14 V14: killed by a node's ICE in a lost contest (not violence).
    Flatline,
    /// Hunger as sickness § 3: a cancer run its course.
    Disease,
    /// M17 review: killed on a job abroad (no Murder, no witness, no heat,
    /// no litter; the kin grieve, a gang's succession opens).
    Abroad,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum RelKind {
    Acquaintance,
    Friend,
    Family,
    Spouse,
    Parent,
    Rival,
    Enemy,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum MemoryKind {
    Ate,
    Starved,
    WasRobbed,
    SawCrime,
    WasArrested,
    Fought,
    Won,
    Lost,
    Socialised,
    Rejected,
    Courted,
    Married,
    Grief,
    SawCorpse,
    Paid,
    Unpaid,
    /// Jail co-location (Social graph, M5).
    MetInJail,
    /// Witnessed outburst of a very low mood (Mood table).
    Tantrum,
    /// M9: broken out of the Jail by the gang.
    Escaped,
    /// M11: could not pay the day's rent in full.
    RentShort,
    /// M11: put out of the Home for arrears.
    Evicted,
    /// M12 D35: hit as a bystander in a brawl at a door.
    CaughtInCrossfire,
    /// M13 D11: a lender towed, bricked or took an asset.
    Repossessed,
    /// M13 D25: hit by a vehicle and lived.
    Crashed,
    /// M13 D35: an heir saw the body of their kin stripped.
    Stripped,
    /// M13 D33: came down from a cyberpsychotic episode.
    Episode,
    /// M13 D33: saw someone go berserk.
    SawEpisode,
    /// M13 D36: dragged off and ripped, and lived.
    Abducted,
    /// M14 V14: fried by a node's ICE on a run.
    Fried,
    /// M14 (plan 3.1): a gang member whose Hideout was robbed on the plane.
    Hacked,
    /// M15 W1: a deed heard (in `Memory.heard`, never in `entries`).
    Rumour,
    /// M15 W12: a hunted, wanted or hated person seen (in `Memory.heard`).
    Sighting,
    /// M15 § 6: a failed Intimidate suffered (phase 2; in `Memory.heard`).
    Threatened,
    /// M15 § 6: a successful Persuade or Charm suffered (phase 2; in `Memory.heard`).
    Persuaded,
    /// L2 § 2: had fun at a venue or the street (the drink's `Socialised`
    /// analogue for "had fun today").
    Enjoyed,
    /// L2 shadow fixes item 9: a sanctioned bout at a Fight Pit, won
    /// (valence > 0) or lost; not a deed (`memory::deed_of` skips it).
    Bout,
    /// Real economy E38: a parent's child was taken by the child protective
    /// service (Grief-class salience and mood; not a deed: `deed_of` reads
    /// `None`, so no killing rumour spreads).
    ChildTaken,
    /// Hunger as sickness § 3: told the cancer has a name.
    Diagnosed,
    /// The Hit market (H17): a Hit or a Beat this agent paid for was done
    /// (subject the target); relief for a Revenge buyer (`closure_val`),
    /// neutral otherwise. Not a deed (`deed_of` reads `None`).
    Commissioned,
    /// M16b phase 4: a tag found on this agent by a scanner (subject the
    /// head of the tag's owner). Not a deed.
    Tagged,
    /// Lives at work phase 1 (D5): served, or served by, the subject at a
    /// counter (`systems::service`). Mood weight 0 (`mood::memory_term`
    /// skips it); not a deed. Bodies only: a Statistical agent keeps none.
    Served,
    /// The pressure pass phase 1 (OPEN-178, `systems::misery`): dismissed
    /// for the absences of a miserable spell (subject the workplace). Not a
    /// deed.
    Sacked,
}

/// Lives at work phase 1 (docs/LIVES_AT_WORK.md § 1.1): what a service
/// encounter serves; a `[[serves]]` row names one per trade. Appended.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServeKind {
    Visit,
    Patient,
    Pupil,
    Inmate,
    Tenant,
    Fare,
    Parcel,
    Crate,
    Client,
    Door,
    Staff,
    Buyer,
    Attendee,
    Source,
    Broker,
}

impl ServeKind {
    pub const ALL: [ServeKind; 15] = [
        ServeKind::Visit,
        ServeKind::Patient,
        ServeKind::Pupil,
        ServeKind::Inmate,
        ServeKind::Tenant,
        ServeKind::Fare,
        ServeKind::Parcel,
        ServeKind::Crate,
        ServeKind::Client,
        ServeKind::Door,
        ServeKind::Staff,
        ServeKind::Buyer,
        ServeKind::Attendee,
        ServeKind::Source,
        ServeKind::Broker,
    ];

    /// The CSV column suffix (`served_<label>`).
    pub fn label(self) -> &'static str {
        match self {
            ServeKind::Visit => "visit",
            ServeKind::Patient => "patient",
            ServeKind::Pupil => "pupil",
            ServeKind::Inmate => "inmate",
            ServeKind::Tenant => "tenant",
            ServeKind::Fare => "fare",
            ServeKind::Parcel => "parcel",
            ServeKind::Crate => "crate",
            ServeKind::Client => "client",
            ServeKind::Door => "door",
            ServeKind::Staff => "staff",
            ServeKind::Buyer => "buyer",
            ServeKind::Attendee => "attendee",
            ServeKind::Source => "source",
            ServeKind::Broker => "broker",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum GoalKind {
    Eat,
    Sleep,
    Work,
    Earn,
    Socialise,
    Court,
    Flee,
    Fight,
    ReportCrime,
    Patrol,
    Arrest,
    JoinGang,
    GangWork,
    /// Gang members: muster at the Hideout and brawl at the rival's door.
    Raid,
    Bury,
    Idle,
    /// M11 D26: open a business (a Bar or a Home) on a vacant Lot.
    Found,
    /// M12 D27: a homeless adult who cannot afford a Hotel moves into a derelict.
    Squat,
    /// M13 D43: buy a vehicle, chrome or a pack.
    Shop,
    /// M13 D34: Therapy or (phase 4, D39) Detox at a Clinic.
    Treat,
    /// M13 D35: strip a fresh body (and rip its chrome).
    Loot,
    /// M13 D39: buy a dose (from a dealer at a Bar, or a legal Market) and use it.
    GetHigh,
    /// M14 V29: jack in at a chair and run a node for Data or a Ledger
    /// (a game abstraction: a dice contest on the Virt plane).
    Hack,
    /// M15 W19: go after the target of a heavy grudge (a scripted plan:
    /// ask around, stake out, attack; `systems::hunt`).
    Hunt,
    /// M15 W35: stand over a dead Spouse's, kin's or Friend's body until
    /// it is buried (a scripted plan; `systems::grudges`).
    GuardBody,
    /// L2 § 2 (plan L14): satisfy `fun` on the rung ladder (a scripted
    /// plan; `systems::leisure`).
    Unwind,
    /// L2 § 2 (plan L14, L20): a gang leader's weekly Collect and Call (a
    /// scripted plan; `systems::leisure`).
    Lead,
    /// M16a (plan C13): work a taken contract record, or network at a
    /// Fixer for one (a scripted plan; `systems::contracts`).
    Contract,
    /// The Hit market (H9): walk to a Fixer and commission the Hit or Beat
    /// one wants (a scripted plan; `systems::hitmarket`).
    Commission,
    /// M16b phase 3 (spec § 5): carry a downed ally (or a Trauma client) to
    /// a Clinic and have it stabilised (a scripted plan; `systems::wounded`).
    Rescue,
    /// Jobs & lives phase 2 (§ 3, D3): the street's day: stand one's own
    /// Pitch through its hours, or a broke jobless adult's walk to the
    /// labour corner before the dawn pick (a scripted plan;
    /// `systems::streetwork`).
    Pitch,
    /// Lives at work phase 2 (§ 3.5): a visit to a hurt kin or an elder
    /// parent (a scripted plan; `systems::household`).
    Visit,
    /// M18 P5h: a homeless adult's errand: walk to a letting desk and apply
    /// for a place (a scripted plan; `systems::housing`).
    FindHome,
}

impl GoalKind {
    /// Round 3 S2 (OPEN-222): a goal that cannot plan, or whose plan failed
    /// twice, cools longer each time in a row (`[brain] cool_backoff`): a
    /// want (a hunt, a court, a shop, a burial, a visit) waits for the
    /// world to change. The body's needs and the day's duties do not: hunger,
    /// sleep, the shift, the coin for a meal, danger, a wound, a downed
    /// friend, the guard's beat and collar, the squatter's night move by
    /// the hour, and a long cool would cost a meal, a night or a shift.
    /// Nor does a Visit (below).
    pub fn backs_off(self) -> bool {
        !matches!(
            self,
            GoalKind::Eat
                | GoalKind::Sleep
                | GoalKind::Work
                | GoalKind::Earn
                | GoalKind::Flee
                | GoalKind::Fight
                | GoalKind::Idle
                | GoalKind::Treat
                | GoalKind::Rescue
                | GoalKind::Patrol
                | GoalKind::Arrest
                | GoalKind::Squat
                | GoalKind::Pitch
                // One goal for many errands (a kin, a duty, a dun): a cooled
                // Visit is dropped (`drop_cooled_visit`), and the next is
                // another visit, not this one again.
                | GoalKind::Visit
        )
    }
}
