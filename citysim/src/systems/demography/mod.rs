//! Demography: ageing, children, births, old age, death effects, corpses and
//! burial, inheritance, job search, emigration, immigration. Daily at
//! `tick_of_day == 0`; `on_death` per death; `bury` per BuryCorpse.

use rand::seq::IndexedRandom;
use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Child, Corpse, DeathCause, Household, Identity, Inventory, Job, Lod, Memory,
    MemoryKind, Mood, Needs, Personality, Position, RelKind, Role, Sentence, Sex, Skills, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::{Tick, DAYS_PER_YEAR, TICKS_PER_DAY};
use crate::world::World;

mod death;
mod hiring;
mod lifecycle;
mod migration;

pub use death::*;
pub use hiring::*;
pub use lifecycle::*;
pub use migration::*;

/// 18 years.
pub const ADULT_AGE_DAYS: u32 = 18 * DAYS_PER_YEAR as u32;
/// 45 years: the end of child-bearing.
pub const FERTILE_UNTIL_DAYS: u32 = 45 * DAYS_PER_YEAR as u32;
pub const CHILD_FOOD_PER_DAY: f32 = 0.5;
pub const BIRTH_SPACING_DAYS: u64 = 120;

pub fn is_adult(world: &World, id: EntityId) -> bool {
    world.comp::<Identity>(id).is_some_and(|i| i.age_days >= ADULT_AGE_DAYS)
}

pub fn run(world: &mut World) {
    // The perf pass phase 7b: the day in three slots (`systems::midnight`),
    // split before the job search and before the relocation.
    use crate::systems::midnight::{due, Mid};
    if due(world, Mid::DemoEarly) {
        early(world);
    }
    if due(world, Mid::DemoJobs) {
        jobs(world);
    }
    if due(world, Mid::DemoHomes) {
        homes(world);
    }
}

fn early(world: &mut World) {
    // Not on the first day (`world.tick > 0` at tick 0, before the stagger).
    if world.day() > 0 {
        crate::prof::timed!("demo::ageing", ageing(world));
        // Round 6 V1 (OPEN-231): the households under each roof and the
        // pantry stakes, before the children's meal reads them.
        crate::prof::timed!("demo::shelf::daily", crate::systems::shelf::daily(world));
        crate::prof::timed!("demo::children", children(world));
        crate::prof::timed!("demo::births", births(world));
        crate::prof::timed!("demo::old_age", old_age(world));
        // The Real economy E38: the child protective service, after the
        // Blocks fed their children (a child is judged on today's meal).
        crate::prof::timed!("demo::camp::take_daily", crate::systems::camp::take_daily(world));
    }
    crate::prof::timed!("demo::corpses", corpses(world));
    crate::prof::timed!("demo::gravedigger_notices", gravedigger_notices(world));
    // Scale S5: the Ambient tier's donors and district aggregates, before
    // the job search reads its labour pool (nothing without the tier).
    crate::prof::timed!("demo::ambient::daily", crate::systems::ambient::daily(world));
    // The island pass phase 4 (I2): a far commuter with no Home found takes
    // a nearer post of his role first (nothing off).
    crate::prof::timed!("demo::commute::job_moves", crate::systems::commute::job_moves(world));
}

fn jobs(world: &mut World) {
    crate::prof::timed!("demo::job_search", job_search(world));
    // Jobs and room J19: the day's labour market for the migrants' means.
    crate::prof::timed!("demo::note_labour_market", note_labour_market(world));
    // Hunger fix item 10 (P5h): the city grows Homes toward unmet demand,
    // then relocation fills them.
    crate::prof::timed!("demo::life::grow_housing", crate::systems::life::grow_housing(world));
    // Lives at work phase 7 (§ 5.3): an exec housed low moves up when the
    // Spire has room, before the relocation reads his Home.
    crate::prof::timed!("demo::life::spire_homes", crate::systems::life::spire_homes(world));
}

fn homes(world: &mut World) {
    // L1: a long commute moves nearer work (a few households a night).
    crate::prof::timed!("demo::life::relocate", crate::systems::life::relocate(world));
    crate::prof::timed!("demo::emigration", emigration(world));
    if world.day() > 0 && world.day().is_multiple_of(7) {
        crate::prof::timed!("demo::immigration", immigration(world));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The island pass phase 1 (OPEN-163, ISLAND_PASS I1): one jobless adult
    /// offered two equal posts, the far one first in the pass's order. Base
    /// (and `[distance]` off): the far post, read first, hires her and the
    /// near one stays open. On: the far pick waits, the near post hires her.
    /// Alone, the far post past the cap still hires her (none is left).
    #[test]
    fn test_job_search_hires_near_before_far() {
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(TICKS_PER_DAY + 7);
        let role = Role::Cook;
        let band = crate::systems::careers::hire_band(&w, role);
        let execs = crate::systems::classes::exec_set(&w);
        // scan-ok: a unit test, once.
        let jobless: Vec<EntityId> = w.citizens().into_iter().filter(|&id| pool_member(&w, id, &execs)).collect();
        let p = *jobless
            .iter()
            .find(|&&id| {
                world_home(&w, id).is_some()
                    && crate::systems::careers::admits(&w, id, band)
                    && crate::systems::fashion::dress_ok(&w, id, role)
            })
            .expect("a jobless adult for the post");
        // Everyone else in the pool holds a placeholder post (no employer).
        let placeholder = w.citizens().into_iter().find_map(|id| w.comp::<Job>(id).cloned()).expect("a worker");
        for &id in jobless.iter().filter(|&&id| id != p) {
            let mut j = placeholder.clone();
            j.employer = None;
            w.insert(id, j);
        }
        let from = world_home(&w, p).expect("her door");
        // The near and far posts: standing buildings, nearest and farthest by the estimate.
        let mut doors: Vec<(Tick, EntityId)> = w
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .chain(w.buildings_of_kind(BuildingKind::NoodleBar))
            .filter_map(|&b| w.comp::<Building>(b).filter(|x| !x.demolished).map(|x| (b, x.door)))
            .map(|(b, d)| (crate::systems::distance::travel(&w, from, d), b))
            .collect();
        doors.sort();
        let (t_near, near) = doors[0];
        let (t_far, far) = *doors.last().expect("posts");
        assert!(t_far > t_near + 2, "the posts differ in distance: {t_near} vs {t_far}");
        // The pass reads its list in order: the far post first.
        let (first, second) = (far, near);
        w.config.distance.as_mut().expect("[distance] on").job_cap_ticks = (t_far + t_near) / 2;
        let vac = vec![(first, vec![role]), (second, vec![role])];
        let run = |mut x: World, vac: &[(EntityId, Vec<Role>)]| {
            for (e, roles) in vac {
                x.vacancies.insert(*e, roles.clone());
            }
            job_search_pass_with(&mut x, vac, None, true);
            x.comp::<Job>(p).and_then(|j| j.employer)
        };
        assert_eq!(run(w.clone(), &vac), Some(second), "the near post hires her");
        let mut off = w.clone();
        off.config.distance = None;
        assert_eq!(run(off, &vac), Some(first), "off (the base): the far post, read first, hires her");
        assert_eq!(run(w.clone(), &vac[..1]), Some(first), "the only post, past the cap, still hires her");
        // The pooled pass and the full walk agree.
        let mut a = w.clone();
        let mut b = w.clone();
        for (e, roles) in &vac {
            a.vacancies.insert(*e, roles.clone());
            b.vacancies.insert(*e, roles.clone());
        }
        job_search_pass_with(&mut a, &vac, None, true);
        job_search_pass_with(&mut b, &vac, None, false);
        assert_eq!(a.comp::<Job>(p).and_then(|j| j.employer), b.comp::<Job>(p).and_then(|j| j.employer));
    }

    fn world_home(w: &World, id: EntityId) -> Option<TilePos> {
        w.comp::<Household>(id).and_then(|h| h.home).and_then(|h| w.comp::<Building>(h)).map(|b| b.door)
    }

    /// Scale S1: the pooled pass hires exactly whom `pick_candidate`'s walk
    /// of every citizen hired: the shipped city on day 3 with a fifth of its
    /// workers dismissed (their posts open, the dismissed back in the pool
    /// and laid off for the rehire bonus), byte for byte after the pass.
    #[test]
    fn test_hire_pool_matches_pick_candidate() {
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(3 * TICKS_PER_DAY + 7);
        // scan-ok: a unit test, once.
        let workers: Vec<(EntityId, EntityId, Role)> = w
            .citizens()
            .into_iter()
            .filter_map(|id| w.comp::<Job>(id).and_then(|j| j.employer.map(|e| (id, e, j.role))))
            .collect();
        for &(id, employer, role) in workers.iter().step_by(5) {
            w.vacate_job(id);
            w.vacancies.entry(employer).or_default().push(role);
            note_laid_off(&mut w, id, role);
        }
        let vacancies: Vec<(EntityId, Vec<Role>)> = w.vacancies.iter().map(|(&e, r)| (e, r.clone())).collect();
        let open: usize = vacancies.iter().map(|(_, r)| r.len()).sum();
        assert!(open > 100, "the test opens posts: {open}");
        let graded_first = crate::systems::classpay::on(&w);
        let passes: &[Option<bool>] = if graded_first { &[Some(true), Some(false)] } else { &[None] };
        let (mut a, mut b) = (w.clone(), w.clone());
        for &pass in passes {
            job_search_pass_with(&mut a, &vacancies, pass, true);
            job_search_pass_with(&mut b, &vacancies, pass, false);
        }
        let hired = |x: &World| x.vacancies.values().map(Vec::len).sum::<usize>();
        assert!(hired(&a) < open, "nobody was hired: the test exercises nothing");
        // Lives at work phase 6: `band_refusals` counts the candidates a
        // pooled pick reads and refuses for age (the full walk's
        // `pick_candidate` is a `&World` read and counts none): the count is
        // the pool's walk, not the hire, so the day's count is cleared on
        // both sides before the compare.
        for x in [&mut a, &mut b] {
            if let Some(j) = x.stats.current.jobs2.as_mut() {
                j.band_refusals = 0;
            }
        }
        assert_eq!(crate::save::to_ron(&a), crate::save::to_ron(&b));
    }

    /// Review finding 8 (the Real economy E41 on a big map): a `CampRaised`
    /// applicant comes first for a Farm vacancy however far away, on a map
    /// whose halved distances pass 512 (the generated 20k map, 1,280 x 376:
    /// up to 827), against the best-placed regular candidate (next door,
    /// the full skill and rehire offset, `skill_w` raised so it reaches the
    /// 512 cap). Both pickers (`pick_candidate` and the pooled pass).
    #[test]
    fn test_camp_raised_first_on_a_big_map() {
        let mut c = crate::config::Config::load();
        c.world.map = "gen:20000".into();
        c.economy2.skill_w = 6.0;
        let mut w = World::new(42, c);
        let farm = w.buildings_of_kind(BuildingKind::Farm)[0];
        let door = w.comp::<Building>(farm).map(|b| b.door).expect("a Farm");
        // scan-ok: a unit test, once.
        let adults: Vec<EntityId> = w
            .citizens()
            .into_iter()
            .filter(|&id| w.has::<Brain>(id) && is_adult(&w, id) && !w.has::<Sentence>(id))
            .collect();
        let far = *adults.iter().max_by_key(|&&id| (hire_from(&w, id).manhattan(door), id)).expect("an adult");
        let near = *adults
            .iter()
            .filter(|&&id| id != far)
            .min_by_key(|&&id| (hire_from(&w, id).manhattan(door), id))
            .expect("a neighbour");
        assert!(hire_from(&w, far).manhattan(door) / 2 > 512, "the map is big enough to test");
        for id in [far, near] {
            if w.has::<Job>(id) {
                w.vacate_job(id);
            }
        }
        if let Some(s) = w.comp_mut::<crate::components::Skills>(near) {
            s.farming = 1.0;
            s.melee = 1.0;
            s.stealth = 1.0;
        }
        note_laid_off(&mut w, near, Role::Farmer);
        let camp = w.spawn();
        w.insert(far, crate::econ::CampRaised { camp, owner: None, days: 30, fed_share: 1.0 });
        let holders = std::collections::BTreeSet::new();
        assert_eq!(pick_candidate(&w, farm, door, Role::Farmer, &holders), Some(far), "pick_candidate");
        let mut pool = HirePool::new(&w);
        assert_eq!(pool.pick(&w, farm, door, Role::Farmer), Some(far), "the pooled pass");
    }
}
