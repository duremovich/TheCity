use super::*;

// ---------------------------------------------------------------------------
// C3a: the live engagement (docs/COMBAT.md § 1)
// ---------------------------------------------------------------------------

/// What a beat did, for the ring log (the view's replay in C4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blow {
    Miss,
    Hit(Region, Severity),
    /// `by` broke off, was knocked out, went down or died.
    Broke(End),
    /// C4: `by` jumped in (on `on`'s side).
    Joined,
    /// C4: `by` took cover (the cover where it now stands, in hundredths).
    Cover(u8),
    /// C4: `by` tried to get away from `on` and could not.
    Stuck,
    /// C4: `by` offered to yield and `on` would not take it.
    Refused,
    /// C4: `by` drew (`true`) or holstered its weapon; changed stance.
    Drew(bool),
    Stance(Stance),
}

/// One entry of an engagement's ring log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeatLog {
    pub tick: Tick,
    /// C4: the duel's beat it fell in (the view's replay groups a beat's
    /// blows; 0 an order or a joiner between beats).
    pub beat: u8,
    pub by: EntityId,
    pub on: EntityId,
    pub blow: Blow,
}

/// One duel's result as a caller reads it: C1's `(winner, loser, died)`,
/// the two (`a` of side 0, `b` of side 1), the loser's end, their names
/// taken before it (a beaten robot is wrecked in it). `fought` false: no
/// blow was struck (a child, a party gone): the starter stands, as C1's
/// backstop read it.
#[derive(Clone, Debug, PartialEq)]
pub struct Clash {
    pub a: EntityId,
    pub b: EntityId,
    pub winner: EntityId,
    pub loser: EntityId,
    pub died: bool,
    pub end: End,
    pub fought: bool,
    pub names: [String; 2],
}

/// A caller's action held while its engagement runs (`Running`); the tail
/// leaves the verdict for the action's next step ([`take_done`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Attack,
    AttackGuard,
    Abduct,
    GuardBody,
}

/// The purpose's on-end hook: each caller's tail, run once whether the
/// fight was live or resolved in one call (C3a: one tail per caller).
#[derive(Clone, Debug)]
pub enum Tail {
    /// None: the caller reads the triple (`law::resolve_fight_mods`).
    Bare,
    /// The Attack action (the attacker is side 0).
    Attack {
        victim: EntityId,
    },
    /// M16a: a Guard taker on post meets the attack first.
    AttackGuard {
        victim: EntityId,
        guard: EntityId,
    },
    /// M13 D36: the abduction (the abductor's Abduct step).
    Abduct {
        victim: EntityId,
    },
    /// W35: a guard over a corpse fights the stripper (the Strip or Rip step).
    GuardBody {
        stripper: EntityId,
    },
    /// The cuffs fought (the guard side 0, the backups after him).
    Arrest {
        berserk: bool,
    },
    /// M16b: a blackmailed culprit goes for the witness.
    Silence(Box<crate::systems::consequence::Silence>),
    /// A fight in the cells.
    Cells(Box<crate::systems::consequence::CellFight>),
    /// The crew's reckoning.
    Reckoning(Box<crate::systems::crew::Reckon>),
    /// A brave squatter stands against the gang's claim (the squatter side 0).
    Squat {
        gname: String,
    },
    /// The pressure pass: a miserable body's brawl.
    Brawl(Box<crate::systems::misery::BrawlTail>),
    /// Protection: the owner beaten.
    Beating(Box<crate::systems::protection::Beating>),
    /// A patrol's attack on a found adult for their chrome.
    Scan {
        tile: crate::components::TilePos,
    },
    /// Underwater: the collector's beating, the stiffed worker's swing.
    Dun(Box<crate::systems::underwater::Dun>),
    Confront(Box<crate::systems::underwater::Confront>),
    /// `raid::fight_out`'s pairings (each an Assault or a Murder by side 0).
    Pairs {
        door: crate::components::TilePos,
        place: String,
    },
    /// A gang's raid on a rival Hideout: the pairings, then the raid's end.
    Raid(Box<crate::systems::raid::RaidEnd>),
}

/// A fight across ticks (C3a): two sides of members in order (the front
/// fights; a loser leaves its side and the next steps up), the caller's
/// mods and tail, the duels' results, the ring log. Not saved: a save
/// resolves every engagement first ([`settle_all`]).
#[derive(Clone, Debug)]
pub struct Engagement {
    pub sides: [Vec<EntityId>; 2],
    pub first: [Option<EntityId>; 2],
    pub mods: crate::systems::law::FightMods,
    pub tail: Tail,
    pub start: Tick,
    pub live: bool,
    /// Side 0 breaks at this many losses (a corp raid's crew).
    pub max_losses: usize,
    pub losses: [usize; 2],
    pub deaths: usize,
    /// Who lost a duel and lived, by side.
    pub beaten: [Vec<EntityId>; 2],
    pub clashes: Vec<Clash>,
    pub beats: u32,
    pub log: std::collections::VecDeque<BeatLog>,
    pub(super) duel: Option<Duel>,
    /// C3b: what each body carries from one duel to the next.
    carry: std::collections::BTreeMap<EntityId, Carry>,
    /// C3b: a principal (`first`) beaten ends it ([`ends_on`]).
    over: bool,
    /// C3b: the joiners it took (`join_max` at most).
    pub joined: usize,
    /// C4: everyone who was ever on a side (the view's replay; a loser
    /// leaves `sides`).
    pub seen: Vec<EntityId>,
    /// C5: the tick whose joiners the view's stepped beats took ([`beat`]).
    recruited: Option<Tick>,
}

impl Engagement {
    /// The last fought duel's result, or C1's "no blow" (the starter stands).
    pub fn result(&self) -> Clash {
        if let Some(c) = self.clashes.iter().rev().find(|c| c.fought).or(self.clashes.last()) {
            return c.clone();
        }
        let a = self.first[0].or(self.first[1]).expect("an engagement has a party");
        let b = self.first[1].unwrap_or(a);
        Clash { a, b, winner: a, loser: b, died: false, end: End::Yielded, fought: false, names: Default::default() }
    }

    /// Everyone still on a side.
    pub fn members(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.sides[0].iter().chain(self.sides[1].iter()).copied()
    }
}

/// What [`engage`] did: the fight is live (the caller's action holds), or
/// it was resolved in the call and its tail ran (the tail's verdict).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Begun {
    Live,
    Over(bool),
}

/// C3a's state on the world (`World::combat`, boxed: the world's serde
/// visitor sits near a debug test thread's stack): the live engagements by
/// id, the bodies in them, the held steps' verdicts, the fled bodies whose
/// flee plans wait for `step`, the last gang raid's outcome. Not saved (a
/// save resolves the engagements first, [`settle_all`]).
#[derive(Clone, Debug, Default)]
pub struct Live {
    pub engagements: std::collections::BTreeMap<u32, Engagement>,
    pub next: u32,
    /// A body in a live engagement -> its id.
    pub engaged: std::collections::BTreeMap<EntityId, u32>,
    /// A held step's engagement ended: (tick, which, the verdict).
    pub done: std::collections::BTreeMap<EntityId, (Tick, Held, bool)>,
    pub fleeing: Vec<EntityId>,
    pub last_raid: Option<crate::systems::raid::Outcome>,
    /// C3b: a fighter's fatigue at its last fight's end (and when).
    pub tired: std::collections::BTreeMap<EntityId, (f32, Tick)>,
    /// C3b: a Sandevistan's cooldown, until.
    pub sandy_cd: std::collections::BTreeMap<EntityId, Tick>,
    /// C3b, the flight holds: a body that fled lies low until.
    pub low: std::collections::BTreeMap<EntityId, Tick>,
    /// C3b: a joiner joins no other fight until.
    pub joined: std::collections::BTreeMap<EntityId, Tick>,
    /// C4: the human's fight orders and standing choices (stance, holster),
    /// by the body it drives (not saved).
    pub orders: std::collections::BTreeMap<EntityId, Orders>,
    /// C4: the live engagements that ended this tick: who was in them and
    /// their ring logs (the view's replay of a fight's last beats).
    pub ended: Vec<(Vec<EntityId>, std::collections::VecDeque<BeatLog>)>,
    /// C4: a hunter who gave chase, and after whom (one "gave chase" line a
    /// chase).
    pub chased: std::collections::BTreeMap<EntityId, EntityId>,
    /// C5: the body whose engagement the view steps a beat at a time inside
    /// the tick ([`beat`]); `step` leaves that engagement alone.
    pub stepper: Option<EntityId>,
    /// C5 (`OPEN-295`): the knocked out, down and still until they come to.
    pub out: std::collections::BTreeMap<EntityId, Out>,
    /// S2 (`OPEN-313`): a bystander who reacted to a fight (or walked on) is
    /// not asked again until (`stir::bystanders`).
    pub stirred: std::collections::BTreeMap<EntityId, Tick>,
}

/// C5: a knockout in progress: until when, who did it, what was taken.
#[derive(Clone, Copy, Debug)]
pub struct Out {
    pub until: Tick,
    pub by: Option<EntityId>,
    pub robbed: i64,
    pub robber: Option<EntityId>,
}

/// C5: is `id` knocked out (down and still, robbable)?
pub fn knocked_out(world: &World, id: EntityId) -> bool {
    !world.combat.out.is_empty() && world.combat.out.get(&id).is_some_and(|o| o.until > world.tick)
}

/// C5: the minutes `id` stays out: `ko_minutes` + `ko_head_minutes` by its
/// worst head wound.
fn ko_span(world: &World, id: EntityId) -> Tick {
    let c = &world.config.combat;
    let head = world
        .comp::<Wounds>(id)
        .and_then(|w| w.list.iter().filter(|x| x.region == Region::Head).map(|x| x.severity).max());
    Tick::from(c.ko_minutes) + head.map_or(0, |s| Tick::from(c.ko_head_minutes[s.index()]))
}

/// C5: `id` knocked out by `by` (a fight's loser): down and still for
/// [`ko_span`]; a winner who wants it goes through its pockets ([`rob`]).
pub(super) fn knock_out(world: &mut World, id: EntityId, by: Option<EntityId>, mods: &crate::systems::law::FightMods) {
    if crate::systems::ages::is_protected(world, id) || !crate::systems::law::living(world, id) {
        return;
    }
    let until = world.tick + ko_span(world, id);
    world.abort_plan(id);
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = None;
    }
    world.combat.out.insert(id, Out { until, by, robbed: 0, robber: None });
    let Some(w) = by else { return };
    let c = &world.config.combat;
    let wants = !mods.arrest
        && mods.why != Purpose::Bout
        && !crate::systems::robots::is_robot(world, w)
        && !crate::systems::law::is_guard(world, w)
        && !crate::systems::player::drives(world, w)
        && world.comp::<Personality>(w).is_some_and(|p| p.lawfulness < c.ko_rob_lawful);
    if wants && crate::systems::law::living(world, w) {
        rob(world, w, id);
    }
}

/// C5: `robber` goes through the pockets of the knocked-out `victim`: its
/// coins x `ko_rob_share` (a Theft, witnessed). Returns the coins taken.
pub fn rob(world: &mut World, robber: EntityId, victim: EntityId) -> i64 {
    let share = world.config.combat.ko_rob_share.clamp(0.0, 1.0);
    let have = world.comp::<crate::components::Wallet>(victim).map_or(0, |w| w.coins.max(0));
    let n = (have as f32 * share).floor() as i64;
    let what = if n > 0 { format!("{n}c") } else { "nothing".to_string() };
    let text =
        format!("{} went through {}'s pockets as they lay there: {what}", world.name_of(robber), world.name_of(victim));
    if n > 0 {
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(victim) {
            w.coins -= n;
        }
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(robber) {
            w.coins += n;
        }
    }
    if let Some(o) = world.combat.out.get_mut(&victim) {
        o.robbed += n;
        o.robber = Some(robber);
    }
    crate::systems::social::robbed_by(world, victim, robber);
    world.remember(victim, MemoryKind::WasRobbed, Some(robber), 0.6, -0.6, false);
    world.push_event(EventKind::Theft, &[robber, victim], text);
    let tile = world.comp::<Position>(robber).map_or(Default::default(), |p| p.tile);
    crate::systems::law::raise_crime_on(
        world,
        robber,
        Some(victim),
        Some(victim),
        crate::components::Crime::Theft,
        tile,
    );
    n
}

/// C5: the knocked out whose time is up come to (a line each).
fn wake(world: &mut World) {
    let now = world.tick;
    let up: Vec<(EntityId, Out)> =
        world.combat.out.iter().filter(|(_, o)| o.until <= now).map(|(&x, &o)| (x, o)).collect();
    for (x, o) in up {
        world.combat.out.remove(&x);
        if !crate::systems::law::living(world, x) {
            continue;
        }
        let place = match world.comp::<Position>(x).and_then(|p| p.building) {
            Some(b) => format!("on the floor of {}", world.name_of(b)),
            None => "in the street".to_string(),
        };
        let lost = match (o.robbed, o.robber) {
            (n, Some(r)) if n > 0 => format!(", {n}c lighter ({} went through the pockets)", world.name_of(r)),
            (_, Some(r)) => format!(", pockets turned out by {} (nothing in them)", world.name_of(r)),
            _ => String::new(),
        };
        let by = o.by.map_or(String::new(), |b| format!(", knocked out by {}", world.name_of(b)));
        let text = format!("{} came to {place}{by}{lost}", world.name_of(x));
        world.push_event(EventKind::Fight, &[x], text);
    }
}

/// Is `id` in a live engagement (it neither thinks nor acts)?
pub fn engaged(world: &World, id: EntityId) -> bool {
    !world.combat.engaged.is_empty() && world.combat.engaged.contains_key(&id)
}

/// A caller's 1-on-1 fight: live when both are Full, else at once.
pub fn fight(world: &mut World, a: EntityId, b: EntityId, mods: crate::systems::law::FightMods, tail: Tail) -> Begun {
    engage(world, [vec![a], vec![b]], mods, tail, true, usize::MAX)
}

/// The fight resolved in one call with no tail: the triple's source.
pub fn clash_now(world: &mut World, a: EntityId, b: EntityId, mods: crate::systems::law::FightMods) -> Clash {
    run_now(world, [vec![a], vec![b]], mods, Tail::Bare, usize::MAX).result()
}

/// Sides fought out in one call (the tail runs); the ended engagement.
pub fn run_now(
    world: &mut World,
    sides: [Vec<EntityId>; 2],
    mods: crate::systems::law::FightMods,
    tail: Tail,
    max_losses: usize,
) -> Engagement {
    let mut e = build(world, sides, mods, tail, max_losses);
    advance(world, &mut e, u32::MAX);
    end(world, &mut e);
    e
}

fn build(
    world: &mut World,
    mut sides: [Vec<EntityId>; 2],
    mods: crate::systems::law::FightMods,
    tail: Tail,
    max_losses: usize,
) -> Engagement {
    let first = [sides[0].first().copied(), sides[1].first().copied()];
    let seen: Vec<EntityId> = sides.iter().flatten().copied().collect();
    // No child is ever a party (CLAUDE.md): counted as C1's backstop was.
    for s in &mut sides {
        let n = s.len();
        s.retain(|&x| !crate::systems::ages::is_protected(world, x));
        if s.len() < n && n == 1 {
            crate::systems::ownership::jobs2_row(world).harm_blocked += 1;
        }
    }
    Engagement {
        sides,
        first,
        mods,
        tail,
        start: world.tick,
        live: false,
        max_losses,
        losses: [0; 2],
        deaths: 0,
        beaten: [Vec::new(), Vec::new()],
        clashes: Vec::new(),
        beats: 0,
        log: std::collections::VecDeque::new(),
        duel: None,
        carry: std::collections::BTreeMap::new(),
        over: false,
        joined: 0,
        seen,
        recruited: None,
    }
}

/// Start a fight between `sides` (C3a § 1). Live when `live_ok` and every
/// party is Full and free (in camera range a party not Full is promoted
/// first); else it runs to its end here, tail and all.
pub fn engage(
    world: &mut World,
    sides: [Vec<EntityId>; 2],
    mods: crate::systems::law::FightMods,
    tail: Tail,
    live_ok: bool,
    max_losses: usize,
) -> Begun {
    let mut e = build(world, sides, mods, tail, max_losses);
    // C3b: those at hand with a reason join at the start (off camera, no
    // more after it).
    recruit(world, &mut e, None, false);
    if live_ok && go_live(world, &e) {
        e.live = true;
        let id = world.combat.next;
        world.combat.next = world.combat.next.wrapping_add(1);
        for x in e.members().collect::<Vec<_>>() {
            world.combat.engaged.insert(x, id);
        }
        world.combat.engagements.insert(id, e);
        return Begun::Live;
    }
    recruit(world, &mut e, None, true);
    advance(world, &mut e, u32::MAX);
    Begun::Over(end(world, &mut e))
}

/// C3b, joiners (§ 1): Full adults within `join_tiles` of the fight (or in
/// its building) with a reason to take a side join it on a keyed courage
/// roll: a gang-mate of a member, kin, a friend at `join_bond`, a guard on
/// duty who sees an assault (the victim's side; the guard's own on an
/// arrest). One with a reason on both sides stays out; no child ever joins.
/// `id`: the live engagement's (its joiners are held). `coarse`: a fight
/// fought at once off camera also takes Coarse bodies (the off-screen
/// city's stand-ins; a live fight takes Full bodies only). Returns the
/// joined.
pub(super) fn recruit(world: &mut World, e: &mut Engagement, id: Option<u32>, coarse: bool) -> usize {
    let c = world.config.combat.clone();
    if e.joined >= c.join_max || e.mods.why == Purpose::Bout || e.sides.iter().any(|s| s.is_empty()) {
        return 0;
    }
    let at = e.sides[1]
        .iter()
        .chain(e.sides[0].iter())
        .find_map(|&x| world.comp::<Position>(x).map(|p| (p.tile, p.building)));
    let Some((tile, building)) = at else { return 0 };
    let near = world.agents_near(tile, c.join_tiles, building);
    let ctx = JoinCtx::of(world, &e.sides, &e.mods, &e.tail, tile, building, coarse);
    let salt = e.start ^ e.first[0].map_or(0, key).rotate_left(17) ^ e.first[1].map_or(0, key);
    let mut joined = 0;
    for x in near {
        if e.joined >= c.join_max {
            break;
        }
        // R4: nor does a body aboard a passing vehicle jump in
        if e.sides.iter().any(|s| s.contains(&x)) || engaged(world, x) || world.aboard.contains_key(&x) {
            continue;
        }
        // One fight at a time: a body that joined one lately is not in the next.
        if world.combat.joined.get(&x).is_some_and(|&t| t > world.tick) {
            continue;
        }
        let Some((k, p, how)) = ctx.check(world, &e.sides, x) else { continue };
        if hash(world, PURPOSE_JOIN, key(x), salt) >= p {
            continue;
        }
        let Some(&with) = e.first[k].as_ref().filter(|f| e.sides[k].contains(f)).or(e.sides[k].first()) else {
            continue;
        };
        e.sides[k].push(x);
        e.seen.push(x);
        e.joined += 1;
        joined += 1;
        if id.is_some() {
            push_log(world, e, x, with, Blow::Joined);
        }
        let until = world.tick + Tick::from(c.join_cooldown_min);
        world.combat.joined.insert(x, until);
        // C5 (`OPEN-297`): a joiner in the building comes to the fight's
        // storey and stands by the one it backs (the view draws it there).
        if building.is_some() {
            let at = world.comp::<Position>(with).map(|p| (p.tile, p.floor));
            if let (Some((t, fl)), Some(p)) = (at, world.comp_mut::<Position>(x)) {
                p.tile = t;
                p.floor = fl;
            }
        }
        if let Some(id) = id {
            world.combat.engaged.insert(x, id);
        }
        let foe = e.sides[1 - k].first().copied().unwrap_or(with);
        let text = format!(
            "{} jumped in on {}'s side against {} ({how})",
            world.name_of(x),
            world.name_of(with),
            world.name_of(foe)
        );
        world.push_event(EventKind::Fight, &[x, with, foe], text);
    }
    if joined > 0 {
        crate::systems::ownership::jobs2_row(world).joiners += joined as u32;
    }
    joined
}

/// C3b, the joiners' reasons (C4: shared with the odds): where the fight
/// is, the sides' gangs, which side the law takes.
pub(super) struct JoinCtx {
    tile: crate::components::TilePos,
    building: Option<EntityId>,
    gangs: [Vec<EntityId>; 2],
    law_side: Option<usize>,
    coarse: bool,
}

impl JoinCtx {
    pub(super) fn of(
        world: &World,
        sides: &[Vec<EntityId>; 2],
        mods: &crate::systems::law::FightMods,
        tail: &Tail,
        tile: crate::components::TilePos,
        building: Option<EntityId>,
        coarse: bool,
    ) -> JoinCtx {
        let gangs: [Vec<EntityId>; 2] = [0, 1].map(|k| sides[k].iter().filter_map(|&x| world.gang_of(x)).collect());
        // The law joins an assault on the victim's side (side 1), an arrest
        // its own; never a gang raid, a turf brawl or a fight in the cells
        // (the jail's own rules).
        let law_side = if mods.arrest {
            Some(0)
        } else if mods.why == Purpose::Raid || matches!(tail, Tail::Pairs { .. } | Tail::Raid(_) | Tail::Cells(_)) {
            None
        } else {
            Some(1)
        };
        JoinCtx { tile, building, gangs, law_side, coarse }
    }

    /// `x` free, at hand and with a reason on one side only: (the side, its
    /// join chance, how it is tied).
    pub(super) fn check(
        &self,
        world: &World,
        sides: &[Vec<EntityId>; 2],
        x: EntityId,
    ) -> Option<(usize, f32, &'static str)> {
        use crate::components::{Lod, RelKind};
        let c = &world.config.combat;
        let coarse = self.coarse;
        let free = crate::systems::law::living(world, x)
            && crate::systems::demography::is_adult(world, x)
            && !crate::systems::ages::is_protected(world, x)
            && !crate::systems::wounded::is_down(world, x)
            && !knocked_out(world, x)
            && !world.has::<crate::components::Sentence>(x)
            && !crate::systems::player::drives(world, x)
            && world.comp::<Brain>(x).is_some_and(|b| {
                (b.lod == Lod::Full || (coarse && b.lod == Lod::Coarse)) && b.cuffed_by.is_none() && !b.emigrating
            });
        if !free {
            return None;
        }
        let inside = world.comp::<Position>(x).is_some_and(|p| match (p.building, self.building) {
            (Some(a), Some(b)) => a == b,
            (None, None) => crate::systems::law::chebyshev(p.tile, self.tile) <= c.join_tiles,
            _ => false,
        });
        if !inside {
            return None;
        }
        // The reasons, by side: (any, kin, the law).
        let mut why = [(false, false, false); 2];
        let g = world.gang_of(x);
        for k in 0..2 {
            if g.is_some_and(|g| self.gangs[k].contains(&g)) {
                why[k].0 = true;
            }
            for &m in &sides[k] {
                if let Some(edge) = world.edge(m, x) {
                    let kin = matches!(edge.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent);
                    if (kin && edge.affinity >= 0.0) || (edge.kind == RelKind::Friend && edge.affinity >= c.join_bond) {
                        why[k].0 = true;
                        why[k].1 |= kin;
                    }
                }
            }
            if self.law_side == Some(k) && crate::systems::law::on_duty(world, x) {
                why[k] = (true, false, true);
            }
        }
        let k = match (why[0].0, why[1].0) {
            (true, false) => 0,
            (false, true) => 1,
            _ => return None,
        };
        let courage = crate::systems::law::courage(world, x);
        let p = c.join_base
            + c.join_courage_w * (courage - 0.5)
            + if why[k].2 { c.join_guard } else { 0.0 }
            + if why[k].1 { c.join_kin } else { 0.0 };
        let how = if why[k].2 {
            "the law"
        } else if why[k].1 {
            "kin"
        } else if g.is_some_and(|g| self.gangs[k].contains(&g)) {
            "a gang-mate"
        } else {
            "a friend"
        };
        Some((k, p, how))
    }
}

/// C3b: a raider who comes to the door of his gang's live raid joins it
/// (side 0, behind those already in). True: he is in.
pub fn join_raid(world: &mut World, gid: EntityId, actor: EntityId) -> bool {
    if engaged(world, actor) || crate::systems::ages::is_protected(world, actor) {
        return false;
    }
    let full = world.comp::<Brain>(actor).is_some_and(|b| b.lod == crate::components::Lod::Full);
    let Some((&id, e)) =
        world.combat.engagements.iter_mut().find(|(_, e)| matches!(&e.tail, Tail::Raid(r) if r.gid == gid))
    else {
        return false;
    };
    if !full {
        return false;
    }
    e.sides[0].push(actor);
    e.seen.push(actor);
    world.combat.engaged.insert(actor, id);
    crate::systems::ownership::jobs2_row(world).joiners += 1;
    true
}

/// Every party a Full body, none in another fight; one the camera holds
/// (or the human drives) brings the rest up to Full first (`OPEN-149`).
fn go_live(world: &mut World, e: &Engagement) -> bool {
    use crate::components::Lod;
    let ids: Vec<EntityId> = e.members().collect();
    if ids.is_empty() || e.sides.iter().any(|s| s.is_empty()) {
        return false;
    }
    // C3b: a robot fights live beside Full bodies.
    let lod = |w: &World, x: EntityId| {
        if crate::systems::robots::is_robot(w, x) {
            return Some(Lod::Full);
        }
        w.comp::<Brain>(x).map(|b| b.lod)
    };
    // R4: a body aboard a moving vehicle (a bus's rider, a car's) is carried off by it: its fight is never staged
    // live on the street it passes (the view's provoke picked a rider passing the arrival and fought him from the
    // bus 20 tiles off, the foe never drawn)
    if ids.iter().any(|&x| lod(world, x).is_none() || engaged(world, x) || world.aboard.contains_key(&x)) {
        return false;
    }
    let seen = ids.iter().any(|&x| crate::systems::player::drives(world, x) || world.ring.holds(x));
    if seen {
        let place = ids.iter().find_map(|&x| world.comp::<Position>(x).and_then(|p| p.building));
        for &x in &ids {
            if lod(world, x) != Some(Lod::Full) && !world.has::<crate::components::Sentence>(x) {
                crate::systems::lod::set_lod_at(world, x, Lod::Full, place);
            }
        }
    }
    ids.iter().all(|&x| lod(world, x) == Some(Lod::Full))
}

/// The duel's instant case (C1's `fight_inner`): a downed party finished
/// where it lies. C3b: a robot fights in the beats.
fn instant(world: &mut World, a: EntityId, b: EntityId) -> Option<Clash> {
    // C5: a body knocked out puts up no fight (it lies there).
    if !world.combat.out.is_empty() {
        let (out_a, out_b) = (knocked_out(world, a), knocked_out(world, b));
        if out_a != out_b {
            let names = [world.name_of(a), world.name_of(b)];
            let (winner, loser) = if out_b { (a, b) } else { (b, a) };
            return Some(Clash { a, b, winner, loser, died: false, end: End::KnockedOut, fought: true, names });
        }
    }
    if !world.wounded.is_empty() {
        let (down_a, down_b) = (world.wounded.contains_key(&a), world.wounded.contains_key(&b));
        if down_a != down_b {
            let names = [world.name_of(a), world.name_of(b)];
            let (winner, loser) = if down_b { (a, b) } else { (b, a) };
            crate::systems::wounded::finish(world, loser, winner);
            return Some(Clash { a, b, winner, loser, died: true, end: End::Dead, fought: true, names });
        }
    }
    None
}

/// Every fought duel's after-tail (C1's `resolve_fight_mods`): a beaten
/// member on his own blocks provokes the holder; a body jacked in is
/// dumped from its run.
fn after_duel(world: &mut World, c: &Clash) {
    if world.config.turf.as_ref().is_some_and(|t| t.war.is_some()) {
        crate::systems::turf::beaten(world, c.winner, c.loser);
    }
    if !world.runner_of.is_empty() {
        for x in [c.a, c.b] {
            crate::systems::virt::dump(world, x, "attacked in the chair");
        }
    }
}

/// A live engagement's party who is gone: dead of something else, taken
/// in, cuffed, or under 18.
pub(super) fn gone(world: &World, x: EntityId) -> bool {
    if crate::systems::robots::is_robot(world, x) {
        return !world.is_alive(x);
    }
    !crate::systems::law::living(world, x)
        || world.has::<crate::components::Sentence>(x)
        || world.comp::<Brain>(x).is_some_and(|b| b.cuffed_by.is_some())
        || crate::systems::ages::is_protected(world, x)
}

/// Run `e` until it ends (true) or `left` beats are spent (false).
fn advance(world: &mut World, e: &mut Engagement, mut left: u32) -> bool {
    use crate::components::Lod;
    let not_full = |w: &World, x: EntityId| w.comp::<Brain>(x).is_some_and(|b| b.lod != Lod::Full);
    loop {
        if e.duel.is_none() {
            if e.losses[0] >= e.max_losses || e.over {
                return true;
            }
            if e.live {
                // A party gone leaves its side; one no longer Full has the
                // rest resolved at once.
                for k in 0..2 {
                    let out: Vec<EntityId> = e.sides[k].iter().copied().filter(|&x| gone(world, x)).collect();
                    for x in out {
                        e.sides[k].retain(|&y| y != x);
                        world.combat.engaged.remove(&x);
                    }
                }
                if e.members().any(|x| not_full(world, x)) {
                    left = u32::MAX;
                }
            }
            let (Some(&a), Some(&b)) = (e.sides[0].first(), e.sides[1].first()) else { return true };
            if let Some(c) = instant(world, a, b) {
                after_duel(world, &c);
                after_clash(world, e, c);
                continue;
            }
            let mut d = duel_start(world, a, b, &e.mods);
            // C3b: the rest of each side strike with its front; what each
            // body carries from the duels before.
            let sides = e.sides.clone();
            duel_aid(world, &mut d, &sides, &e.mods);
            for f in d.f.iter_mut().chain(d.aid.iter_mut().flatten()) {
                if let Some(c) = e.carry.get(&f.id) {
                    f.morale += c.morale;
                    f.keren = c.keren;
                    f.hurt_once = c.hurt_once;
                    f.sandy_left = c.sandy_left;
                    f.sandy_armed &= !c.sandy_used;
                    f.struck |= c.struck;
                    f.swung |= c.swung;
                }
            }
            e.duel = Some(d);
        }
        let Some(mut d) = e.duel.take() else { continue };
        if e.live {
            let out: Vec<EntityId> = [d.f[0].id, d.f[1].id].into_iter().filter(|&x| gone(world, x)).collect();
            if !out.is_empty() {
                // The duel is off: the gone leave their sides.
                for x in out {
                    for s in &mut e.sides {
                        s.retain(|&y| y != x);
                    }
                    world.combat.engaged.remove(&x);
                }
                continue;
            }
            // A helper gone strikes no more; a joiner since the duel began
            // strikes from now.
            for k in 0..2 {
                d.aid[k].retain(|h| !gone(world, h.id) && e.sides[k].contains(&h.id));
                let new: Vec<EntityId> = e.sides[k]
                    .iter()
                    .copied()
                    .filter(|&x| x != d.f[k].id && !d.aid[k].iter().any(|h| h.id == x))
                    .collect();
                if !new.is_empty() {
                    let add = aid_of(world, k, &new, &e.mods);
                    d.aid[k].extend(add);
                }
            }
            if not_full(world, d.f[0].id) || not_full(world, d.f[1].id) {
                left = u32::MAX;
            }
            // One moved out of reach has left the fight (C3a: he fled).
            if d.near && d.f[0].end.is_none() && d.f[1].end.is_none() {
                let (a, b) = (d.f[0].id, d.f[1].id);
                if !crate::systems::law::near(world, a, b, world.config.combat.reach_tiles) {
                    let pos = |x: EntityId| world.comp::<Position>(x).map(|p| (p.tile, p.building));
                    let i = usize::from(pos(a) == Some(d.at[0]));
                    d.f[i].end = Some(End::Fled);
                }
            }
        }
        let before = d.beat;
        let log = if e.live { Some(&mut e.log) } else { None };
        let ended = duel_beats(world, &mut d, &e.mods, &mut left, log);
        e.beats += u32::from(d.beat.saturating_sub(before));
        if !ended {
            e.duel = Some(d);
            return false;
        }
        // C3b: the carry into the next duel (the morale moved, the charges).
        for f in d.f.iter().chain(d.aid[0].iter()).chain(d.aid[1].iter()) {
            let moved = if f.morale.is_finite() { f.morale - f.morale0 } else { 0.0 };
            let used = !f.sandy_armed && f.sandy > 0;
            e.carry.insert(
                f.id,
                Carry {
                    morale: moved,
                    keren: f.keren,
                    hurt_once: f.hurt_once,
                    sandy_left: f.sandy_left,
                    sandy_used: used,
                    struck: f.struck,
                    swung: f.swung,
                },
            );
        }
        let (a, b) = (d.f[0].id, d.f[1].id);
        let names = std::mem::take(&mut d.names);
        let out = duel_end(world, d, &e.mods);
        let li = usize::from(out.loser == b);
        let c = Clash {
            a,
            b,
            winner: out.winner,
            loser: out.loser,
            died: out.died,
            end: out.sides[li].end,
            fought: true,
            names,
        };
        after_duel(world, &c);
        after_clash(world, e, c);
    }
}

// A duel ended: the loser leaves his side (counted), a body that broke and
/// fled runs (C3a: fleeing as movement), the purpose's per-duel tail.
fn after_clash(world: &mut World, e: &mut Engagement, c: Clash) {
    let k = usize::from(!e.sides[0].contains(&c.loser));
    e.sides[k].retain(|&y| y != c.loser);
    e.losses[k] += 1;
    if c.died {
        e.deaths += 1;
    } else {
        e.beaten[k].push(c.loser);
    }
    if e.live {
        world.combat.engaged.remove(&c.loser);
    }
    // Run at the tick's `step` (after exec: a body fleeing its own step's
    // fight would have the step's end move its new plan on).
    if c.end == End::Fled && e.mods.why != Purpose::Bout {
        world.combat.fleeing.push(c.loser);
    }
    // C3b: a principal beaten ends the fight (the joiners stand down).
    let ends = ends_on(&e.tail);
    if (0..2).any(|k| ends[k] && e.first[k] == Some(c.loser)) {
        e.over = true;
    }
    let tail = std::mem::replace(&mut e.tail, Tail::Bare);
    match &tail {
        Tail::Pairs { door, place } => crate::systems::raid::pair_end(world, *door, place, &c),
        Tail::Raid(x) => crate::systems::raid::pair_end(world, x.door, &x.place, &c),
        Tail::Arrest { .. } => crate::systems::law::arrest_duel(world, e, &c),
        _ => {}
    }
    e.tail = tail;
    e.clashes.push(c);
}

/// C3b: which side's principal (`first`), beaten, ends the engagement: a
/// brawl's pairings and a gang raid fight to the last; an arrest ends on the
/// suspect (the guard's backups step up); every other caller on either.
pub(super) fn ends_on(tail: &Tail) -> [bool; 2] {
    match tail {
        Tail::Pairs { .. } | Tail::Raid(_) => [false, false],
        Tail::Arrest { .. } => [false, true],
        _ => [true, true],
    }
}

/// C3b, the flight holds: is `id` lying low after fleeing a fight (only
/// Eat, Sleep, Flee, Idle and, hurt, Treat score; `utility::think_scored`)?
pub fn lying_low(world: &World, id: EntityId) -> bool {
    !world.combat.low.is_empty() && world.combat.low.get(&id).is_some_and(|&t| t > world.tick)
}

/// C3b: a goal a body lying low may take: nothing that takes it out
/// (back to the fight or the one it fled); a Treat only when hurt.
pub fn low_ok(world: &World, id: EntityId, goal: crate::components::GoalKind) -> bool {
    use crate::components::GoalKind;
    match goal {
        GoalKind::Eat | GoalKind::Sleep | GoalKind::Flee | GoalKind::Idle => true,
        GoalKind::Treat => worst_untreated(world, id).is_some(),
        _ => false,
    }
}

/// C3a: a body that broke and fled runs from the fight: the flee plan
/// (`FleeToHome`: its Home, else the nearest refuge, `life::refuge`). C3b:
/// it lies low `low_hours` after (nothing takes it back out).
pub(super) fn flee(world: &mut World, id: EntityId) {
    use crate::components::{ActionInstance, GoalKind, Lod};
    let free = crate::systems::law::living(world, id)
        && !world.has::<crate::components::Sentence>(id)
        && !crate::systems::wounded::is_down(world, id)
        && world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical && b.cuffed_by.is_none() && !b.emigrating);
    if !free {
        return;
    }
    // C4: the body the human drives runs too (its legs broke it off, or the
    // human's Flee); a Help drags the downed clear instead (the Rescue).
    if crate::systems::player::drives(world, id) {
        let help = world.combat.orders.get_mut(&id).and_then(|o| o.help.take());
        if let Some(w) = help {
            if let Some(plan) = crate::systems::wounded::plan_for(world, id, w) {
                world.abort_plan(id);
                crate::systems::plan::install(world, id, plan);
                let text = format!("{} broke off to drag {} clear", world.name_of(id), world.name_of(w));
                world.push_event(EventKind::Fight, &[id, w], text);
                return;
            }
        }
    }
    world.abort_plan(id);
    // C5 (`OPEN-297`): a flight whose refuge is the building it fled (or
    // none) runs out to the street, `flee_tiles` from the door.
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let refuge = crate::systems::life::refuge(world, id).map(|(_, b)| b);
    let out = here.filter(|&b| refuge.is_none_or(|r| r == b)).and_then(|b| street_tile(world, id, b));
    let step = match out {
        Some(t) => ActionInstance {
            action: crate::goap::ActionKind::GoTo(crate::goap::LocationKey::Picked),
            target: None,
            tile: Some(t),
        },
        None => ActionInstance { action: crate::goap::ActionKind::FleeToHome, target: None, tile: None },
    };
    let plan = crate::goap::Plan { goal: GoalKind::Flee, target: None, steps: vec![step], started_tick: world.tick };
    crate::systems::plan::install(world, id, plan);
    let now = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(GoalKind::Flee);
        b.goal_since = now;
    }
    crate::systems::ownership::jobs2_row(world).fled_runs += 1;
    let until = world.tick + Tick::from(world.config.combat.low_hours) * TICKS_PER_HOUR;
    world.combat.low.insert(id, until);
}

/// C5: a road tile about `flee_tiles` from `b`'s door (the ring's first
/// road from a side keyed on the body): where a flight from inside runs.
fn street_tile(world: &World, id: EntityId, b: EntityId) -> Option<crate::components::TilePos> {
    use crate::components::{TileKind, TilePos};
    let door = world.comp::<crate::components::Building>(b)?.door;
    let r0 = i32::from(world.config.combat.flee_tiles.max(1));
    let (w, h) = (world.map.w() as i32, world.map.h() as i32);
    let spin = (key(id) % 4) as usize;
    for r in r0..r0 + 8 {
        let mut ring = Vec::with_capacity(8 * r as usize);
        for k in -r..r {
            ring.push((k, -r));
            ring.push((r, k));
            ring.push((-k, r));
            ring.push((-r, -k));
        }
        let n = ring.len();
        for i in 0..n {
            let (dx, dy) = ring[(i + spin * n / 4) % n];
            let (x, y) = (i32::from(door.x) + dx, i32::from(door.y) + dy);
            if x < 0 || y < 0 || x >= w || y >= h {
                continue;
            }
            let t = TilePos { x: x as u16, y: y as u16 };
            if world.map.tile_at(t) == TileKind::Road {
                return Some(t);
            }
        }
    }
    None
}

/// The engagement is over: its members free, its tail run; a held action's
/// verdict left for its next step. Returns the verdict.
pub(super) fn end(world: &mut World, e: &mut Engagement) -> bool {
    if e.live {
        for x in e.members().collect::<Vec<_>>() {
            world.combat.engaged.remove(&x);
        }
        // C4: its last beats, for the view's replay this tick.
        if !e.log.is_empty() {
            world.combat.ended.push((e.seen.clone(), e.log.clone()));
        }
        let ticks = world.tick.saturating_sub(e.start) + 1;
        let row = crate::systems::ownership::jobs2_row(world);
        row.live_fights += 1;
        row.live_fight_ticks += ticks as u32;
    }
    let tail = std::mem::replace(&mut e.tail, Tail::Bare);
    let c = e.result();
    let verdict = match &tail {
        Tail::Bare | Tail::Pairs { .. } => true,
        Tail::Attack { victim } => {
            crate::exec::actions::attack_end(world, c.a, *victim, &c);
            true
        }
        Tail::AttackGuard { victim, guard } => crate::exec::actions::attack_guard_end(world, c.a, *victim, *guard, &c),
        Tail::Abduct { victim } => crate::systems::chrome::abduct_end(world, c.a, *victim, &c),
        Tail::GuardBody { stripper } => c.winner == *stripper,
        Tail::Arrest { berserk } => crate::systems::law::arrest_end(world, e, *berserk),
        Tail::Silence(x) => {
            crate::systems::consequence::silence_end(world, x, &c);
            true
        }
        Tail::Cells(x) => {
            crate::systems::consequence::cell_fight_end(world, x, &c);
            true
        }
        Tail::Reckoning(x) => {
            crate::systems::crew::reckoning_end(world, x, &c);
            true
        }
        Tail::Squat { gname } => {
            crate::systems::gang::squat_end(world, gname, &c);
            true
        }
        Tail::Brawl(x) => {
            crate::systems::misery::brawl_end(world, x, &c);
            true
        }
        Tail::Beating(x) => {
            crate::systems::protection::beating_end(world, x, &c);
            true
        }
        Tail::Scan { tile } => {
            crate::systems::tags::attack_end(world, *tile, &c);
            true
        }
        Tail::Dun(x) => {
            crate::systems::underwater::dun_end(world, x, &c);
            true
        }
        Tail::Confront(x) => {
            crate::systems::underwater::confront_end(world, x, &c);
            true
        }
        Tail::Raid(x) => {
            crate::systems::raid::raid_end(world, x, e);
            true
        }
    };
    if e.live {
        let held = match &tail {
            Tail::Attack { .. } => e.first[0].map(|x| (x, Held::Attack)),
            Tail::AttackGuard { .. } => e.first[0].map(|x| (x, Held::AttackGuard)),
            Tail::Abduct { .. } => e.first[0].map(|x| (x, Held::Abduct)),
            Tail::GuardBody { stripper } => Some((*stripper, Held::GuardBody)),
            _ => None,
        };
        if let Some((x, h)) = held {
            world.combat.done.insert(x, (world.tick, h, verdict));
        }
    }
    e.tail = tail;
    verdict
}

/// A held action's engagement ended: its verdict (taken), when it is
/// `held`'s and fresh (an hour; a body whose plan moved on leaves a stale one).
pub fn take_done(world: &mut World, id: EntityId, held: Held) -> Option<bool> {
    if world.combat.done.is_empty() {
        return None;
    }
    let (t, h, v) = world.combat.done.get(&id).copied()?;
    if h != held {
        return None;
    }
    world.combat.done.remove(&id);
    (world.tick.saturating_sub(t) <= TICKS_PER_HOUR).then_some(v)
}

/// C3a, each tick after exec: every live engagement runs `[combat]
/// beats_per_tick` beats; one that ends runs its tail.
pub fn step(world: &mut World) {
    world.combat.ended.clear();
    if !world.combat.done.is_empty() && world.tick.is_multiple_of(TICKS_PER_HOUR) {
        let now = world.tick;
        world.combat.done.retain(|_, x| now.saturating_sub(x.0) <= TICKS_PER_HOUR);
    }
    if world.tick.is_multiple_of(TICKS_PER_HOUR) {
        // C3b: the lapsed holds, cooldowns and rested fatigue.
        let (now, rest) = (world.tick, world.config.combat.rest_hour.max(0.01));
        let live = &mut world.combat;
        live.low.retain(|_, t| *t > now);
        live.joined.retain(|_, t| *t > now);
        live.sandy_cd.retain(|_, t| *t > now);
        // C4: a chase is over when its Hunt is.
        if !live.chased.is_empty() {
            let hunts = &world.hunts;
            live.chased.retain(|h, v| hunts.get(h).is_some_and(|x| x.target == *v));
        }
        let live = &mut world.combat;
        live.tired.retain(|_, x| x.0 - rest * now.saturating_sub(x.1) as f32 / TICKS_PER_HOUR as f32 > 0.0);
    }
    // C5: the knocked out come to.
    if !world.combat.out.is_empty() {
        wake(world);
    }
    if world.combat.engagements.is_empty() && world.combat.fleeing.is_empty() {
        return;
    }
    let n = u32::from(world.config.combat.beats_per_tick.max(1));
    let stepper = world.combat.stepper;
    let ids: Vec<u32> = world.combat.engagements.keys().copied().collect();
    for id in ids {
        // C5: the view steps the human's fight a beat at a time ([`beat`]),
        // `beats_per_tick` a tick as every fight's.
        if stepper.is_some_and(|p| world.combat.engaged.get(&p) == Some(&id)) {
            continue;
        }
        let Some(mut e) = world.combat.engagements.remove(&id) else { continue };
        // C3b: joiners each tick a live fight runs.
        recruit(world, &mut e, Some(id), false);
        // S2 (`OPEN-313`): and the rest of the street reacts.
        super::stir::bystanders(world, &e, id);
        // C4: the human's orders, at the next beat.
        apply_orders(world, &mut e);
        if advance(world, &mut e, n) {
            end(world, &mut e);
        } else {
            world.combat.engagements.insert(id, e);
        }
    }
    for id in std::mem::take(&mut world.combat.fleeing) {
        flee(world, id);
    }
}

/// C5, the combat clock (§ 5): one beat of the live engagement `id` is in,
/// stepped by the view between two ticks (`Live::stepper`; `step` leaves it
/// alone): the tick's joiners at its first beat, the orders, the beat, the
/// end. Returns the beat's log entries and whether the fight is over;
/// `None`: not in one.
pub fn beat(world: &mut World, id: EntityId) -> Option<(Vec<BeatLog>, bool)> {
    let eid = *world.combat.engaged.get(&id)?;
    let mut e = world.combat.engagements.remove(&eid)?;
    let mut kept = std::mem::take(&mut e.log);
    if e.recruited != Some(world.tick) {
        e.recruited = Some(world.tick);
        recruit(world, &mut e, Some(eid), false);
        super::stir::bystanders(world, &e, eid);
    }
    apply_orders(world, &mut e);
    let over = advance(world, &mut e, 1);
    let new: Vec<BeatLog> = e.log.drain(..).collect();
    let cap = world.config.combat.log_beats.max(1);
    kept.extend(new.iter().copied());
    while kept.len() > cap {
        kept.pop_front();
    }
    e.log = kept;
    if over {
        end(world, &mut e);
    } else {
        world.combat.engagements.insert(eid, e);
    }
    Some((new, over))
}

/// Every live engagement run to its end now (before a save: engagements
/// are not saved).
pub fn settle_all(world: &mut World) {
    let ids: Vec<u32> = world.combat.engagements.keys().copied().collect();
    for id in ids {
        let Some(mut e) = world.combat.engagements.remove(&id) else { continue };
        advance(world, &mut e, u32::MAX);
        end(world, &mut e);
    }
}
