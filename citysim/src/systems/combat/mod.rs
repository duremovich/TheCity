//! The combat model C1, the beat, and C2, wounds that last (docs/COMBAT.md
//! § 1-5, § 10 phases 1-2).
//!
//! A game abstraction between fictional agents of a simulated city: a fight
//! is a handful of **beats** between two duellists ([`resolve_now`] runs one
//! duel in one call; the [`FightOutcome`] sits on `World::last_fight`).
//! - **C3a, the engagement** ([`Engagement`], [`engage`]): two sides of
//!   members, a duel at a time (the fronts; a loser leaves his side and the
//!   next steps up). Every party Full: it lives in `World::combat`
//!   across ticks ([`step`], `beats_per_tick` a tick after exec; its bodies
//!   neither think nor act), else it runs to its end in the call. Each
//!   caller's tail is a [`Tail`], run once either way; a caller whose step
//!   needs the answer holds it `Running` ([`take_done`]). A body that breaks
//!   and flees runs (a flee plan); one who moves out of reach has fled.
//! - **C3b, numbers, joiners, chrome**: the rest of each side strike the
//!   other side's front every beat (the front splits its defence, morale
//!   reads the head count, fatigue builds across the duels and rests off
//!   out of fights); a principal beaten ends the fight ([`ends_on`]);
//!   joiners with a reason (gang, kin, friend, the law on duty) take a side
//!   ([`recruit`]); a body that fled lies low ([`lying_low`]); the Nerves'
//!   Kerenzikov and Sandevistan, the Arms' blades or gorilla arms, chrome
//!   limbs that break instead of bleeding; robots fight in the beats and
//!   are wrecked (`End::Wrecked`).
//!
//! - **A beat.** Each standing side acts in reflex order: it may break
//!   (morale under its stance's line: it flees if its legs let it, else
//!   yields if the other side takes yields), else it attacks: the skill for
//!   its weapon's family (melee or shooting) against the target's evasion,
//!   the weapon kind's accuracy x its tier's quality, the stances. A hit
//!   lands on a region (`region_weights`) at a severity from the kind's
//!   damage table, less the armour on that region. A head hit may knock
//!   the body out; a Grave head or torso wound may kill (the kind's
//!   lethality x the caller's `kill_mult`; a subdue stance knocks out
//!   instead). Every wound takes blood at once; at none the body is down.
//! - **Wounds** ([`Wounds`]) stay on the body: an arm weakens the attack,
//!   a leg slows the dodge, the flight and the walk (`Kit.walk_mult`), the
//!   head makes a later knockout likelier and the work worse
//!   (`health::effects`). The hourly pass drains the blood of a bleeding
//!   wound into the M16b downed state (`wounded::down`) and stops a Wound's
//!   bleeding on a roll.
//! - **C2, wounds that last** (§ 2): a wound heals at its `heal_at`, sooner
//!   when seen to (the Treat goal's patch-up, `customers::hurt`, priced by
//!   the worst wound; the M16b Stabilise); first aid from a friend, kin or a
//!   gang-mate beside the body; a manual post turns a bad arm or leg away
//!   ([`unfit`]: the shift and its pay lost); a healed Grave may leave a
//!   maim, a lost limb (also severed at the hit) until a ripperdoc fits
//!   chrome ([`fitted`]), and a scar (`Wounds.marks`); the off-screen
//!   beatings write wounds ([`stat_wounds`]). Age slows the fighter and
//!   breaks the bone; a post trains its skills at midnight; the pit's bout
//!   is a `Bout` here.
//! - **The skills** `melee`, `shooting`, `evasion` grow with hits landed
//!   and dodges; every gate reads [`power`].
//! - **No child is ever harmed**: a fight with a minor never starts
//!   (`law::resolve_fight_mods`), and every hit checks again.

use crate::components::{
    AssetLoc, Brain, Gang, Identity, Job, Kit, Mark, MemoryKind, Needs, Personality, Position, Region, Severity,
    Skills, Slot, Wound, Wounds,
};
use crate::config::{WeaponFamily, WeaponKindCfg};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::personality::Drift;
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

mod clash;
mod duel;
mod fighter;
mod odds;
mod orders;
mod stir;
mod view;
mod wounds;

pub use clash::*;
pub use duel::*;
pub use fighter::*;
pub use odds::*;
pub use orders::*;
pub use stir::come_for;
pub use view::*;
pub use wounds::*;

const PURPOSE_BEAT: u64 = 0x434F_0001;
const PURPOSE_KIND: u64 = 0x434F_0002;
const PURPOSE_SHOOT: u64 = 0x434F_0003;
const PURPOSE_EVADE: u64 = 0x434F_0004;
const PURPOSE_BLEED: u64 = 0x434F_0005;
const PURPOSE_HEAL: u64 = 0x434F_0006;
const PURPOSE_STAT: u64 = 0x434F_0007;
const PURPOSE_JOIN: u64 = 0x434F_0008;

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

// ---------------------------------------------------------------------------
// Stances, purposes, the outcome
// ---------------------------------------------------------------------------

/// A participant's stance (docs/COMBAT.md § 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Stance {
    AllOut,
    Defensive,
    Subdue,
    HoldBack,
}

impl Stance {
    fn i(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Stance::AllOut => "all out",
            Stance::Defensive => "on guard",
            Stance::Subdue => "to subdue",
            Stance::HoldBack => "holding back",
        }
    }

    /// C4: the four, in the HUD's order.
    pub const ALL: [Stance; 4] = [Stance::AllOut, Stance::Defensive, Stance::Subdue, Stance::HoldBack];

    /// C4: the order's name on the HUD.
    pub fn order_label(self) -> &'static str {
        match self {
            Stance::AllOut => "all out",
            Stance::Defensive => "defensive",
            Stance::Subdue => "subdue",
            Stance::HoldBack => "hold back",
        }
    }

    /// Does this side take the other's yield? All out does unless the
    /// purpose is lethal.
    fn takes_yield(self, lethal: bool) -> bool {
        !(self == Stance::AllOut && lethal)
    }
}

/// Why a fight was started: the caller's purpose (with `FightMods`'s
/// `arrest`, `kill_p` and `kill_mult`, it picks the stances).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Purpose {
    /// A brawl, a beating, an attack: stances from the personalities.
    #[default]
    Brawl,
    /// A gang raid or a turf fight: both sides all out.
    Raid,
    /// A visitor swinging at a door: the bouncer (`b`) subdues.
    Door,
    /// C2, C1's wrong-read (c): a FightPit's bout. Both subdue, bare hands;
    /// sanctioned: no memories of a fight, no edges, no weapons' tail (the
    /// pit writes its own `Bout` memories).
    Bout,
}

/// How a participant's fight ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Won,
    Fled,
    Yielded,
    KnockedOut,
    Downed,
    Dead,
    /// C3b: a robot beaten in the beats.
    Wrecked,
}

impl End {
    pub fn label(self) -> &'static str {
        match self {
            End::Won => "won",
            End::Fled => "fled",
            End::Yielded => "yielded",
            End::KnockedOut => "was knocked out",
            End::Downed => "went down bleeding",
            End::Dead => "was killed",
            End::Wrecked => "was wrecked",
        }
    }
}

/// One side of a [`FightOutcome`].
#[derive(Clone, Debug, PartialEq)]
pub struct SideOut {
    pub id: EntityId,
    pub stance: Stance,
    /// The `[[weapon_kinds]]` row it fought with (0 the bare hand).
    pub kind: usize,
    pub end: End,
    /// The wounds it took, in order.
    pub took: Vec<(Region, Severity)>,
    pub landed: u8,
    pub dodged: u8,
    /// C3b: the chrome that showed in it ("gorilla arms", "Sandevistan",
    /// "a Kerenzikov dodge").
    pub chrome: Vec<&'static str>,
}

/// The fight's result (docs/COMBAT.md § 1): the sides (`a` the starter
/// first), the winner, the beats run.
#[derive(Clone, Debug, PartialEq)]
pub struct FightOutcome {
    pub sides: [SideOut; 2],
    pub winner: EntityId,
    pub loser: EntityId,
    pub died: bool,
    pub beats: u8,
    /// C3a: the ticks a live duel ran past its first (0: in one tick).
    pub ticks: u32,
    /// C3b: by side, the members behind its front who struck with it and
    /// the hits they landed.
    pub crowd: [(u8, u8); 2],
}
