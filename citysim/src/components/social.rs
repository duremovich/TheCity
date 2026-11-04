use super::*;

// ---------------------------------------------------------------------------
// Relationships
// ---------------------------------------------------------------------------

/// Key is always `(min(a, b), max(a, b))`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    /// `-1.0..=1.0`, default 0.0.
    pub affinity: f32,
    /// `0.0..=1.0`, default 0.3.
    #[serde(default = "first_trust", skip_serializing_if = "is_first_trust")]
    pub trust: f32,
    /// Coins the lower id owes the higher id; negative = reverse.
    /// This, `trust` and the two `Option`s are left out of a save at their
    /// defaults (M10 review: ~55 of ~170 bytes per edge, and edges are most
    /// of a save; an off-screen acquaintance never touched again keeps all four).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub debt: i32,
    pub kind: RelKind,
    pub last_interaction: Tick,
    /// Spouse edges only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_birth_tick: Option<Tick>,
    /// When the current debt was incurred (ageing charges after 14 days).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debt_since: Option<Tick>,
}

fn is_zero(v: &i32) -> bool {
    *v == 0
}

fn first_trust() -> f32 {
    0.3
}

fn is_first_trust(v: &f32) -> bool {
    *v == first_trust()
}

impl Edge {
    pub fn new(kind: RelKind, tick: Tick) -> Self {
        Edge {
            affinity: 0.0,
            trust: 0.3,
            debt: 0,
            kind,
            last_interaction: tick,
            last_birth_tick: None,
            debt_since: None,
        }
    }
}

/// Canonical edge key.
pub fn edge_key(a: EntityId, b: EntityId) -> (EntityId, EntityId) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

// ---------------------------------------------------------------------------
// M10: holes (off-screen crimes whose actor is drawn lazily) and traces
// ---------------------------------------------------------------------------

/// `(tick << 24) | (victim.index << 2) | kind` (M10 D8): unique per victim,
/// kind and tick, and ascending ids are oldest first.
pub type HoleId = u64;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum HoleKind {
    Robbed,
    Assaulted,
    Killed,
    /// M13 D37: taken off screen for the chrome (packs as 3 in `hole_id`).
    Abducted,
}

impl HoleKind {
    /// The crime a witness reports once the hole is bound.
    pub fn crime(self) -> Crime {
        match self {
            HoleKind::Robbed => Crime::Theft,
            HoleKind::Assaulted => Crime::Assault,
            HoleKind::Killed => Crime::Murder,
            HoleKind::Abducted => Crime::Abduction,
        }
    }

    /// "robbery", "beating", "killing".
    pub fn noun(self) -> &'static str {
        match self {
            HoleKind::Robbed => "robbery",
            HoleKind::Assaulted => "beating",
            HoleKind::Killed => "killing",
            HoleKind::Abducted => "abduction",
        }
    }
}

/// The packed hole key (M10 D8).
pub fn hole_id(tick: Tick, victim: EntityId, kind: HoleKind) -> HoleId {
    debug_assert!(victim.index < 1 << 22, "hole key packs the victim index in 22 bits");
    (tick << 24) | (u64::from(victim.index) << 2) | kind as u64
}

/// An off-screen crime against a Statistical victim, actor not yet drawn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hole {
    pub id: HoleId,
    pub kind: HoleKind,
    pub victim: EntityId,
    pub zone: Zone,
    /// M12 D5: the district of the victim's tile.
    #[serde(default = "DistrictId::unset")]
    pub district: DistrictId,
    pub tick: Tick,
    /// The ring entry to rewrite on binding.
    pub event_id: u64,
    /// Killed, or the victim is a gang member or a guard: bound by the next daily pass.
    pub consequential: bool,
    /// Captured at creation: `World::spouses` forgets the dead (M10 D10).
    #[serde(default)]
    pub spouse: Option<EntityId>,
    /// Coins a Robbed victim lost; paid to the actor at bind, gone on Unknown.
    #[serde(default)]
    pub loot: i64,
    /// The victim's Home and gang at creation: a Killed victim's are gone by
    /// bind time, and the binder's gang-claim weight and gang shock need them.
    #[serde(default)]
    pub home: Option<EntityId>,
    #[serde(default)]
    pub gang: Option<EntityId>,
    /// L2 (plan L27): a faction-violence hole's source (`fviolence::daily`);
    /// `None` for the hourly table's and M13's holes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<crate::ledger::ViolenceSource>,
    /// L2 (L27): the acting faction (a gang, a corp, the Law, or the episode
    /// agent itself): the binder draws only among its members.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction: Option<EntityId>,
    /// L2 (L27): a riot hole's riot (the binder draws among its rioters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub riot: Option<u32>,
    /// The island pass phase 10 (OPEN-185): the off-screen hand that made a
    /// faction-violence hole (`fviolence::daily`): the binder binds it as a
    /// contract record's taker (`bind::prebound_ok`), never a member seen
    /// on screen elsewhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prebound: Option<EntityId>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Bound {
    Actor(EntityId),
    Unknown,
}
