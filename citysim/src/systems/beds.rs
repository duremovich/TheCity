//! The island fix round F2 (Shadow V8 bugs 2, 4, 6; docs/ISLAND_PASS.md I6,
//! I8; OPEN-189, OPEN-191, OPEN-193): a bed is taken only if it is the
//! resident's own workplace's or within the commute cap (`[beds]`).
//!
//! - **Company beds** ([`company_take`], [`company_keep`]): a company bed is
//!   taken only by its workplace's own staff whose post is within
//!   `company_post_ticks` of the bed (`distance::travel`, door to door): the
//!   move-in (`company::fill`), the commute pressure's move
//!   (`commute::company_for`), and so the buyouts' far-staff move. A corp's
//!   worker of another of its workplaces keeps a bed only within the same
//!   cap (`company::on_staff`, the eviction and the rent): the base let any
//!   of the corp's posts take or keep one at any distance (a D3 clinic's
//!   ripperdoc in a D6 Site Barracks). A Home trade never hands a company
//!   bed to the other side ([`trade_ok`]; the base's relocation swap did),
//!   and the retiree's kin take-in goes to a relative's plain Home only
//!   ([`kin_ok`]; the base moved a retiree into a child's coffin rack, 721
//!   min from anything of his). The staff's own household (a spouse wed or
//!   reunited, the children) still lives with them, as I6 has it.
//! - **The Hideout bed** ([`hideout_ok`], Dylan's D2 = A): tonight's Hideout
//!   bed for a housed member (the nearer bed `life::hideout_bed`, the watch
//!   and lying low `gang::holes_up_at`) only within `hideout_walk_ticks` of
//!   where the body stands, and, for a member with a post, with the Hideout
//!   within `hideout_work_ticks` of the post (tomorrow's commute); past
//!   either, Home. A homeless member takes it as before. Rounds, court and
//!   orders still go there (their own plans). The base took the nearer of
//!   two beds both past the cap and sent the watch at any distance; a gang
//!   has one Hideout (a Hideout per district is `OPEN-187`'s).
//! - **The builder's site day** (`site_from_home`, `shift::haul_plan`): a far
//!   site's day begun away from the Yard walks straight to the site, the
//!   crate carried from the Yard counted on the same leg.
//!
//! Statistical mirror: none needed. The company move-in, the relocation, the
//! eviction and the kin take-in are tier-blind passes over every citizen;
//! the Hideout bed and the site day are Full-body plans (a Statistical body
//! sleeps and works by the table, which has no Hideout bed or site leg). No
//! random draw. With `[beds]` absent every function here
//! answers the base's rule.

use crate::components::{Building, Household, Job, TilePos};
use crate::config::BedsCfg;
use crate::entity::EntityId;
use crate::world::World;

fn cfg(world: &World) -> Option<&BedsCfg> {
    world.config.beds.as_ref()
}

/// `[beds]` on.
pub fn on(world: &World) -> bool {
    cfg(world).is_some()
}

fn door(world: &World, b: EntityId) -> Option<TilePos> {
    world.comp::<Building>(b).map(|bd| bd.door)
}

fn employer(world: &World, a: EntityId) -> Option<EntityId> {
    world.comp::<Job>(a).and_then(|j| j.employer)
}

/// `a`'s post (building `e`) is within `company_post_ticks` of company bed
/// `h`.
fn post_near(world: &World, cap: u64, h: EntityId, e: EntityId) -> bool {
    match (door(world, h), door(world, e)) {
        (Some(hd), Some(ed)) => crate::systems::distance::travel(world, hd, ed) <= cap,
        _ => false,
    }
}

/// Company bed `h` may take `a`'s household: `a` holds a post at its
/// workplace within `company_post_ticks` of it. `None` off (the caller's
/// base rule).
pub fn company_take(world: &World, a: EntityId, h: EntityId) -> Option<bool> {
    let cap = cfg(world)?.company_post_ticks;
    let w = crate::systems::company::of(world, h);
    Some(match (w, employer(world, a)) {
        (Some(w), Some(e)) => e == w && post_near(world, cap, h, e),
        _ => false,
    })
}

/// `a` keeps company bed `h` as staff: a post at its workplace, or at
/// another workplace of the bed's owner, within `company_post_ticks` of it.
/// `None` off (the caller's base rule).
pub fn company_keep(world: &World, a: EntityId, h: EntityId) -> Option<bool> {
    let cap = cfg(world)?.company_post_ticks;
    let (Some(w), Some(e)) = (crate::systems::company::of(world, h), employer(world, a)) else {
        return Some(false);
    };
    let corp = e == w || world.owner_of(h).is_some_and(|o| world.owner_of(e) == Some(o));
    Some(corp && post_near(world, cap, h, e))
}

/// A Home trade may hand `home` to the other side: not a company bed (on).
pub fn trade_ok(world: &World, home: EntityId) -> bool {
    !on(world) || crate::systems::company::open_to_all(world, home)
}

/// The kin take-in may move an elder into `h`: a relative's plain Home, not
/// a company bed (on).
pub fn kin_ok(world: &World, h: EntityId) -> bool {
    !on(world) || crate::systems::company::open_to_all(world, h)
}

/// I8 (D2 = A): Hideout `h` may be tonight's bed for `id`: homeless, or the
/// walk from where the body stands within `hideout_walk_ticks` (0 inside)
/// and, with a post, `h` within `hideout_work_ticks` of it. `None` off.
pub fn hideout_ok(world: &World, id: EntityId, h: EntityId) -> Option<bool> {
    let c = cfg(world)?;
    if world.comp::<Household>(id).is_none_or(|x| x.home.is_none()) {
        return Some(true);
    }
    let Some(hd) = door(world, h) else { return Some(false) };
    let inside = world.comp::<crate::components::Position>(id).and_then(|p| p.building) == Some(h);
    if !inside {
        let Some(here) = crate::systems::life::origin(world, id) else { return Some(false) };
        if crate::systems::distance::travel(world, here, hd) > c.hideout_walk_ticks {
            return Some(false);
        }
    }
    let work = employer(world, id).and_then(|e| door(world, e));
    Some(work.is_none_or(|wd| crate::systems::distance::travel(world, hd, wd) <= c.hideout_work_ticks))
}

/// `site_from_home`: a far site's day begun away from the Yard walks
/// straight to the site.
pub fn site_from_home(world: &World) -> bool {
    cfg(world).is_some_and(|c| c.site_from_home)
}
