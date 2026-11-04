use super::*;

/// `f32 0.0..=1.0`, initial `U(0.1, 0.4)`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skills {
    pub stealth: f32,
    /// The combat model C1 (docs/COMBAT.md § 3): the old `fighting`, the
    /// hand-to-hand skill (grows when the body lands a melee hit).
    pub melee: f32,
    /// C1: firearms, seeded from a per-role base x a keyed draw
    /// (`combat::seed_skills`; -1 until seeded); grows when a shot lands.
    #[serde(default = "Skills::unset_hacking")]
    pub shooting: f32,
    /// C1: not being hit, from a keyed draw x an age curve (-1 until
    /// seeded); grows when the body dodges. Kit reflex adds at the fight.
    #[serde(default = "Skills::unset_hacking")]
    pub evasion: f32,
    pub farming: f32,
    /// M14 (plan V35): `hack_seed_scale x U(0,1)^3` from a keyed stream.
    #[serde(default = "Skills::unset_hacking")]
    pub hacking: f32,
    /// M15 W25: the four social skills (`0..=1`), one primary per adult,
    /// from the Skill word stream.
    #[serde(default)]
    pub persuasion: f32,
    #[serde(default)]
    pub intimidation: f32,
    #[serde(default)]
    pub knowledge: f32,
    #[serde(default)]
    pub deception: f32,
    /// M15 W25: the day each social skill was last used (`SocialSkill`
    /// order), for the 30-day rust.
    #[serde(default, skip_serializing_if = "is_zero_days")]
    pub last_used: [u16; 4],
}

fn is_zero_days(v: &[u16; 4]) -> bool {
    *v == [0; 4]
}

impl Skills {
    pub fn unset_hacking() -> f32 {
        -1.0
    }

    /// M15: a fresh `Skills` with the three v1 skills and hacking given;
    /// the social four 0 (seeded by `moves::give_social`).
    pub fn basic(stealth: f32, melee: f32, farming: f32, hacking: f32) -> Skills {
        Skills {
            stealth,
            melee,
            shooting: -1.0,
            evasion: -1.0,
            farming,
            hacking,
            persuasion: 0.0,
            intimidation: 0.0,
            knowledge: 0.0,
            deception: 0.0,
            last_used: [0; 4],
        }
    }

    /// M15: one social skill.
    pub fn social(&self, s: crate::word::SocialSkill) -> f32 {
        use crate::word::SocialSkill as S;
        match s {
            S::Persuasion => self.persuasion,
            S::Intimidation => self.intimidation,
            S::Knowledge => self.knowledge,
            S::Deception => self.deception,
        }
    }

    pub fn social_mut(&mut self, s: crate::word::SocialSkill) -> &mut f32 {
        use crate::word::SocialSkill as S;
        match s {
            S::Persuasion => &mut self.persuasion,
            S::Intimidation => &mut self.intimidation,
            S::Knowledge => &mut self.knowledge,
            S::Deception => &mut self.deception,
        }
    }

    /// The social four in `SocialSkill` order.
    pub fn social_all(&self) -> [f32; 4] {
        [self.persuasion, self.intimidation, self.knowledge, self.deception]
    }

    pub fn set_social_all(&mut self, v: [f32; 4]) {
        [self.persuasion, self.intimidation, self.knowledge, self.deception] = v;
    }
}

/// Present only while jailed.
/// M12 D27: living in a derelict building, rent-free and unhoused (a Dreg).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Squatter {
    pub building: EntityId,
    pub since: Tick,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sentence {
    pub until_tick: Tick,
    pub crime: Crime,
}

/// `rank`: 0 grunt, 1 lieutenant, 2 leader.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GangMember {
    pub gang: EntityId,
    pub rank: u8,
    #[serde(default)]
    pub joined_tick: Tick,
    /// Lives at work phase 6 (§ 4.5, the ladder): raids led (the highest
    /// rank at the door) and won (`[gangs] lieutenants`; not written at 0).
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub raids_led: u16,
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub raids_won: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Corpse {
    pub died_tick: Tick,
    pub cause: DeathCause,
    pub buried: bool,
    /// Freed a day after this.
    #[serde(default)]
    pub buried_tick: Option<Tick>,
    /// M13 D13: the coins and goods the dead carried, until stripped or settled.
    #[serde(default)]
    pub loot: Loot,
    /// M13 D13: stripped (by hand or off screen).
    #[serde(default)]
    pub stripped: bool,
    /// M13 D13: inheritance has run on what was left (`assets::settle_corpse`).
    #[serde(default)]
    pub settled: bool,
    /// Round 6 V3 (`OPEN-238`): who killed it (a violent death's named
    /// killer), for the law's case when the body is found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub killer: Option<EntityId>,
    /// Round 6 V3: the killer and the dead knew each other (an edge at the
    /// death): the case's motive lead.
    #[serde(default)]
    pub tied: bool,
    /// Round 6 V3: someone found the body and the law has the case
    /// (`demography::find_bodies`); off for a death nobody has come upon.
    #[serde(default)]
    pub found: bool,
    /// Round 8 X1 (`OPEN-246`): the dead struck first and fell to the one he
    /// went for (self-defence), or died resisting arrest at a guard's hands:
    /// the found body's case names nobody for a Murder.
    #[serde(default)]
    pub defended: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActionInstance {
    pub action: ActionKind,
    /// Building or agent.
    pub target: Option<EntityId>,
    pub tile: Option<TilePos>,
}
