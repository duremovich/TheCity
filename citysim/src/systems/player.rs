//! M18 P1 (the seam, docs/M18_PLAYER.md): a human drives one adult body.
//!
//! `PlayerCommand::Possess` puts `Brain.player` on the body; without
//! autopilot its own `think` and the plan queue skip it (`think::turn`,
//! `plan::run` / `plan::enqueue`), and the human's commands write its plan:
//! `Steps` (a directed plan of `ActionInstance`s, a click's walk), `Goal` (a
//! goal handed to `plan::plan_for`, the planner the NPCs use), `Stop`.
//! `SetAutopilot` hands the body back to its brain and takes it again.
//! Exec runs the plan like any other and re-checks each step at its start.
//!
//! No rule here is the player's own: a body that cannot choose
//! (`think::held`: off camera, sentenced, cuffed, leaving, down) cannot be
//! ordered either, and a step a goal change could not abort
//! (`think::uninterruptible`) cannot be stopped, but for the will terms the
//! human supplies (P2: a Sleep's first-hour lock, and each verb's
//! `Verb::waived`). Nobody under 18 can be possessed, and no step may target
//! a child.
//!
//! P2 (the verbs, below): menus built from precondition checks, the verbs
//! mapped to existing `ActionKind`s, the will-waivers per verb.

use crate::components::{
    ActionInstance, Brain, Building, BuildingKind, GoalKind, Household, Identity, Inventory, Job, PlayerControlled,
    Position, Squatter, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::ExecState;
use crate::goap::{ActionKind, LocationKey, Plan, PlanCtx, StealSource, WorldState};
use crate::systems::{demography, plan, think};
use crate::world::World;
use serde::{Deserialize, Serialize};

/// Possess (`on`) an adult body, or hand it back to its brain for good.
pub fn possess(world: &mut World, id: EntityId, on: bool) {
    let name = world.name_of(id);
    if !on {
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.player = None;
        }
        world.push_event(EventKind::PlayerAction, &[id], format!("{name}: let go (the brain has the body)"));
        return;
    }
    let why = if world.comp::<Brain>(id).is_none() || !world.has::<Identity>(id) {
        Some("no living body")
    } else if !demography::is_adult(world, id) {
        Some("a child (under 18) is never possessed")
    } else {
        None
    };
    if let Some(why) = why {
        world.push_event(EventKind::PlayerActionFailed, &[id], format!("Possess {name}: {why}"));
        return;
    }
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.player = Some(PlayerControlled::default());
    }
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: possessed (autopilot off)"));
}

/// Why the possessed body cannot take an order now (`None`: it can).
fn refusal(world: &World, id: EntityId) -> Option<String> {
    let Some(b) = world.comp::<Brain>(id) else { return Some("no living body".into()) };
    if b.player.is_none() {
        return Some("not possessed".into());
    }
    if let Some(why) = think::held(world, id) {
        return Some(why.into());
    }
    // M18 P2: a Sleep past its first hour is the NPC's anti-flap
    // (`think::uninterruptible`), a will term: the human may get up.
    let sleeping = matches!(b.exec, ExecState::Use { kind: ActionKind::Sleep, until, .. } if until > world.tick);
    if think::uninterruptible(b, world.tick) && !sleeping {
        let step = b.current_step().map_or("-".to_string(), |s| format!("{:?}", s.action));
        return Some(format!("busy: {step} cannot be cut short"));
    }
    None
}

/// The human's order refused: the line to the event ring and the HUD.
fn refuse(world: &mut World, id: EntityId, what: &str, why: &str) {
    let line = format!("{what} refused: {why}");
    let tick = world.tick;
    if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
        p.last_failure = Some((tick, line.clone()));
    }
    let name = world.name_of(id);
    world.push_event(EventKind::PlayerActionFailed, &[id], format!("{name}: {line}"));
}

/// A direct order takes the wheel: autopilot off.
fn take_wheel(world: &mut World, id: EntityId) {
    if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
        p.autopilot = false;
        p.doing = None;
    }
}

/// The steps as one line for the log.
pub fn steps_line(steps: &[ActionInstance]) -> String {
    steps.iter().map(|s| format!("{:?}", s.action)).collect::<Vec<_>>().join(", ")
}

/// A directed plan: the body drops what it is doing and runs `steps`.
pub fn steps(world: &mut World, id: EntityId, steps: &[ActionInstance]) {
    if let Some(why) = refusal(world, id) {
        return refuse(world, id, "Steps", &why);
    }
    if steps.is_empty() {
        return refuse(world, id, "Steps", "no steps");
    }
    let (w, h) = (world.map.w(), world.map.h());
    for s in steps {
        if s.tile.is_some_and(|t| usize::from(t.x) >= w || usize::from(t.y) >= h) {
            return refuse(world, id, "Steps", "a tile off the map");
        }
        if let Some(t) = s.target {
            if !world.is_alive(t) {
                return refuse(world, id, "Steps", "the target is gone");
            }
            // No child is ever a valid target (CLAUDE.md, 2026-10-27).
            if world.has::<Identity>(t) && !demography::is_adult(world, t) {
                return refuse(world, id, "Steps", "a child is never a target");
            }
        }
    }
    take_wheel(world, id);
    world.abort_plan(id);
    let tick = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(GoalKind::Idle);
        b.goal_since = tick;
    }
    let plan = Plan { goal: GoalKind::Idle, target: None, steps: steps.to_vec(), started_tick: tick };
    plan::install(world, id, plan);
    let name = world.name_of(id);
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: directed [{}]", steps_line(steps)));
}

/// A goal: the planner the NPCs use makes the plan (`plan::plan_for`).
pub fn goal(world: &mut World, id: EntityId, goal: GoalKind) {
    if let Some(why) = refusal(world, id) {
        return refuse(world, id, &format!("{goal:?}"), &why);
    }
    take_wheel(world, id);
    world.abort_plan(id);
    let tick = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(goal);
        b.goal_since = tick;
    }
    plan::plan_for(world, id, goal);
    let planned = world.comp::<Brain>(id).and_then(|b| b.plan.as_ref()).map(|p| steps_line(&p.steps));
    let name = world.name_of(id);
    match planned {
        Some(line) => {
            world.push_event(EventKind::PlayerAction, &[id], format!("{name}: {goal:?} planned [{line}]"));
        }
        None => refuse(world, id, &format!("{goal:?}"), "no plan for it here and now"),
    }
}

/// Stop: the plan dropped, the body stands where it is.
pub fn stop(world: &mut World, id: EntityId) {
    if let Some(why) = refusal(world, id) {
        return refuse(world, id, "Stop", &why);
    }
    take_wheel(world, id);
    world.abort_plan(id);
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = None;
    }
    let name = world.name_of(id);
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: stopped"));
}

/// Autopilot on: the body's brain thinks and plans for it again (its next
/// think replaces a directed plan when another goal wins); off: the human
/// has the wheel (the brain's running plan goes on until it ends or a
/// command replaces it).
pub fn set_autopilot(world: &mut World, id: EntityId, on: bool) {
    let name = world.name_of(id);
    let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) else {
        world.push_event(EventKind::PlayerActionFailed, &[id], format!("Autopilot {name}: not possessed"));
        return;
    };
    p.autopilot = on;
    let text = if on { "autopilot on (the brain has the body)" } else { "autopilot off (the human has the body)" };
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: {text}"));
}

/// The plan as the HUD reads it: the goal, each step (the current one
/// marked), and the step's exec state.
pub fn plan_line(world: &World, id: EntityId) -> String {
    let Some(b) = world.comp::<Brain>(id) else { return "-".into() };
    let Some(p) = b.plan.as_ref() else { return "no plan (standing)".into() };
    let steps: Vec<String> = p
        .steps
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let a = match s.action {
                ActionKind::GoTo(k) => format!("GoTo {k:?}"),
                a => format!("{a:?}"),
            };
            if i == usize::from(b.plan_step) {
                format!("> {a}")
            } else {
                a
            }
        })
        .collect();
    format!("{:?}: {}", p.goal, steps.join(", "))
}

/// The standing building whose rect holds `tile` (the smallest, when a
/// shop-house's units overlap), for a click.
pub fn building_at(world: &World, tile: crate::components::TilePos) -> Option<EntityId> {
    world
        .buildings_by_kind()
        .values()
        .flatten()
        .filter_map(|&b| world.comp::<crate::components::Building>(b).map(|x| (b, x)))
        .filter(|(_, x)| !x.demolished && x.rect.contains(tile))
        .min_by_key(|(b, x)| (u32::from(x.rect.w) * u32::from(x.rect.h), b.index))
        .map(|(b, _)| b)
}

/// The standing building whose rect is nearest `tile` within `reach` tiles
/// (Chebyshev; 0 inside it), for a click on a roof or a wall: the view's lots
/// and the sim's rects disagree by a tile or two at their edges.
pub fn building_near(world: &World, tile: crate::components::TilePos, reach: u16) -> Option<EntityId> {
    let gap = |r: &crate::components::Rect| {
        let d = |p: u16, lo: u16, len: u16| {
            let hi = u32::from(lo) + u32::from(len.max(1)) - 1;
            let p = u32::from(p);
            if p < u32::from(lo) {
                u32::from(lo) - p
            } else {
                p.saturating_sub(hi)
            }
        };
        d(tile.x, r.x, r.w).max(d(tile.y, r.y, r.h))
    };
    world
        .buildings_by_kind()
        .values()
        .flatten()
        .filter_map(|&b| world.comp::<crate::components::Building>(b).map(|x| (b, x)))
        .filter(|(_, x)| !x.demolished)
        .map(|(b, x)| (gap(&x.rect), u32::from(x.rect.w) * u32::from(x.rect.h), b.index, b))
        .filter(|&(g, ..)| g <= u32::from(reach))
        .min()
        .map(|(.., b)| b)
}

/// A click on `tile`: the walk there, into the building that holds it (on a
/// roof or a wall, `on_building`: the nearest within `snap` tiles), else to
/// the street tile (`LocationKey::Picked`).
pub fn walk_to(world: &World, tile: crate::components::TilePos, on_building: bool, snap: u16) -> Vec<ActionInstance> {
    let target = if on_building { building_near(world, tile, snap) } else { building_at(world, tile) };
    vec![ActionInstance { action: ActionKind::GoTo(crate::goap::LocationKey::Picked), target, tile: Some(tile) }]
}

// ---------------------------------------------------------------------------
// M18 P2: the verbs
// ---------------------------------------------------------------------------
//
// A click on a building or a body asks the sim for a menu (`menu_building`,
// `menu_body`; the HUD's own rows are `menu_self`). Each row is a `Verb`, and
// a verb lists only when `plan_of` would make its plan now: the verb's last
// action passes its own preconditions (`ActionKind::allowed` and
// `preconditions` on a `PlanCtx`, as if the walk there were done), with the
// verb's will terms waived (`waive`, `Verb::waived`). `act` builds the same
// plan again (the menu is a frame old), installs it as a directed plan, and
// exec re-checks each step at its start, the waiver applied there too
// (`exec::start_step` calls `waive` for a body the human drives). Nothing
// under 18 is ever a row's target.

/// A menu verb: what the player can pick, with its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verb {
    /// Walk in at a door that opens to you (`[view] play_public_kinds`, or yours).
    GoIn(EntityId),
    /// Walk to the street outside a door that does not.
    GoToDoor(EntityId),
    /// `EatFromInventory`: eat what you carry.
    EatCarried,
    /// `GoTo(Home)`, `EatAtHome`: a meal from the pantry.
    EatHome,
    /// At a Market: `BuyFood`, `EatFromInventory` (a meal now).
    BuyMeal(EntityId),
    /// At a Market: `BuyFood`, `GoTo(Home)`, `StoreFood` (a grocery run).
    Groceries(EntityId),
    /// `GoTo(Home)`, `Sleep`.
    SleepHome,
    /// `Sleep` where you stand, on the street (rough).
    SleepHere,
    /// `GoTo` the booked bed or the squat, `Sleep`.
    SleepBed,
    /// At a Hotel or a Mission's cot house: `CheckIn` (tonight's bed).
    TakeBed(EntityId),
    /// At a derelict: `Occupy` (a squat).
    Squat(EntityId),
    /// At a door with a post you can take, or a hiring board: the ask (`Vend` bound to the door).
    AskWork(EntityId),
    /// `Goal(Work)` through the planner: the shift.
    Work,
    /// At a Market: `StealFood(Market)`.
    Shoplift(EntityId),
    /// At a stranger's Home: in through the door and `StealFood(Home)` (a break-in).
    BreakIn(EntityId),
    /// At a stranger's Home, a gang member: `Extort`.
    Extort(EntityId),
    /// A person in a Market, Bar, Farm or your home: `Chat`.
    Chat(EntityId),
    /// A person: `Attack`.
    Attack(EntityId),
}

impl Verb {
    /// The will terms this verb waives (M18 "Kept": willingness is the
    /// human's; ability and circumstance stay). A cost term is the planner's
    /// (a directed plan reads no cost), named here so the audit is in one place.
    pub fn waived(self) -> &'static [&'static str] {
        match self {
            Verb::Attack(_) => &[
                "threat_removed: an Enemy edge within 4 tiles (the NPC swings only at a foe)",
                "cost: courage, lawfulness, fighting (the planner's)",
            ],
            Verb::Shoplift(_) | Verb::BreakIn(_) => &[
                "cost: lawfulness x20, [life] steal_can_buy, the misery cut, withdrawal (the planner's)",
                "think: the Steal goal's hunger and poverty gates (never read: the human picks the verb)",
            ],
            Verb::Extort(_) => &["cost: lawfulness x12 (the planner's)"],
            Verb::Chat(_) => &["cost: sociability (the planner's)", "think: the Socialise goal's belonging gate"],
            Verb::TakeBed(_) => {
                &["hotel_available: the nearest bed in hotel_reach, homeless or far from home (away_hotel)"]
            }
            Verb::SleepHere => {
                &["rough_ok: exhausted with every bed far; one with a bed or the means of one never lies in the street"]
            }
            Verb::SleepHome | Verb::SleepBed => {
                &["uninterruptible: a Sleep past 60 min cannot be stopped (the NPC's anti-flap)"]
            }
            Verb::AskWork(_) => &[
                "searching: jobless days, [search] need_wealth, the idle days after a quit",
                "the search hours, the day's tries, the crowd at a door",
            ],
            Verb::BuyMeal(_) | Verb::Groceries(_) => {
                &["cost: greed (the planner's)", "think: the Eat goal's hunger gate"]
            }
            Verb::EatCarried | Verb::EatHome => &["think: the Eat goal's hunger gate"],
            Verb::GoIn(_) | Verb::GoToDoor(_) | Verb::Squat(_) | Verb::Work => &[],
        }
    }
}

/// One menu row.
#[derive(Clone, Debug)]
pub struct Row {
    pub verb: Verb,
    pub label: String,
}

/// A menu: its title and its legal rows.
#[derive(Clone, Debug, Default)]
pub struct Menu {
    pub title: String,
    pub rows: Vec<Row>,
}

/// The human has the wheel: possessed, autopilot off.
pub fn drives(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).and_then(|b| b.player.as_ref()).is_some_and(|p| !p.autopilot)
}

/// The will-gate waived for a step of a body the human drives (exec's step
/// re-check and the menu's check): per action, each term named in
/// `Verb::waived`. Ability and circumstance are left as observed.
pub fn waive(world: &World, id: EntityId, kind: ActionKind, ws: &mut WorldState, ctx: &mut PlanCtx) {
    match kind {
        // An Enemy edge within 4 tiles is the NPC's reason to swing; the
        // target within 4 tiles (`hostile_adjacent`) stays.
        ActionKind::Attack => ws.threat_removed = false,
        // Any bed here he can pay for tonight (the Hotel's free beds and
        // price kept; `street::book` re-checks both at completion).
        ActionKind::CheckIn => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            ctx.hotel_available |= here.is_some_and(|h| bed_for(world, id, h).is_some());
        }
        // The pavement, by choice: the place (the street) stays.
        ActionKind::Sleep => ctx.rough_ok = true,
        _ => {}
    }
}

/// A bed tonight at `h` for `id`: its price, when the Hotel or cot house
/// has a free bed and he can pay it (`None` otherwise, or booked there).
fn bed_for(world: &World, id: EntityId, h: EntityId) -> Option<i64> {
    use crate::systems::street;
    if !street::is_hotel(world, h)
        || street::booked_hotel(world, id) == Some(h)
        || street::free_beds_for(world, h, id) == 0
    {
        return None;
    }
    let price = street::hotel_price(world, h);
    (world.comp::<Wallet>(id).map_or(0, |w| w.coins) >= price).then_some(price)
}

fn step(action: ActionKind, target: Option<EntityId>, tile: Option<TilePos>) -> ActionInstance {
    ActionInstance { action, target, tile }
}

/// The walk into `b` (P1's `GoTo(Picked)`, the building bound).
fn go_in(world: &World, b: EntityId) -> ActionInstance {
    let door = world.comp::<Building>(b).map(|bd| bd.door);
    step(ActionKind::GoTo(LocationKey::Picked), Some(b), door)
}

/// The walk to the street outside `b`'s door.
fn go_to_door(world: &World, b: EntityId) -> Option<ActionInstance> {
    let t = world.comp::<Building>(b).map(|bd| world.outside_door(bd))?;
    Some(step(ActionKind::GoTo(LocationKey::Picked), None, Some(t)))
}

fn home_of(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Household>(id).and_then(|h| h.home)
}

fn here(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Position>(id).and_then(|p| p.building)
}

/// Does `b`'s door open to `id` from a click: a public kind (`[view]
/// play_public_kinds`), or his own (home, workplace, squat, booked bed, his
/// gang's Hideout).
pub fn opens(world: &World, id: EntityId, b: EntityId) -> bool {
    let Some(bd) = world.comp::<Building>(b).filter(|bd| !bd.demolished) else { return false };
    world.config.view.play_public_kinds.contains(&bd.kind)
        || home_of(world, id) == Some(b)
        || world.comp::<Job>(id).and_then(|j| j.employer) == Some(b)
        || world.comp::<Squatter>(id).is_some_and(|s| s.building == b)
        || crate::systems::street::booked_hotel(world, id) == Some(b)
        || world.gang_of(id).is_some_and(|g| crate::systems::vertical::is_hideout_of(world, g, b))
}

/// The location key `id` would be at inside `b` (`WorldState::observe`'s
/// reading for a plan bound to `target`), for the menu's check.
fn key_in(world: &World, id: EntityId, b: EntityId, target: Option<EntityId>) -> LocationKey {
    let Some(bd) = world.comp::<Building>(b) else { return LocationKey::Street };
    if home_of(world, id) == Some(b) {
        return LocationKey::Home;
    }
    if Some(b) == target && bd.kind == BuildingKind::Home {
        return LocationKey::TargetHome;
    }
    if Some(b) == target && crate::systems::street::is_derelict(world, b) {
        return LocationKey::Squat;
    }
    if crate::systems::charity::is_mission(world, b) {
        return if crate::systems::distance::free(world).is_some() { LocationKey::Cot } else { LocationKey::Hotel };
    }
    match LocationKey::of_building(bd.kind) {
        LocationKey::Home => LocationKey::Street,
        LocationKey::Workplace if world.comp::<Job>(id).and_then(|j| j.employer) != Some(b) => LocationKey::Street,
        k => k,
    }
}

/// The last action's check as the step re-check reads it with the walk
/// done: `at` the place, `arrived` the inputs the walk would change, the
/// will waived.
fn passes(
    world: &World,
    id: EntityId,
    kind: ActionKind,
    target: Option<EntityId>,
    at: LocationKey,
    arrived: impl FnOnce(&mut WorldState, &mut PlanCtx),
) -> bool {
    let mut ctx = PlanCtx::build_light(world, id, target);
    let mut ws = WorldState::observe(world, id, target);
    ws.at = at;
    waive(world, id, kind, &mut ws, &mut ctx);
    arrived(&mut ws, &mut ctx);
    kind.allowed(&ctx) && kind.preconditions(&ws, &ctx)
}

/// A Market's food as `id` would meet it inside `m`: free stock, its price.
fn market_inputs(world: &World, id: EntityId, m: EntityId) -> (bool, i64) {
    let free =
        world.comp::<Building>(m).is_some_and(|bd| bd.stock_food > WorldState::reserved_by_others(world, m, Some(id)));
    (free, crate::systems::diet::price_at(world, id, m).max(1))
}

/// A person a verb may name: a living adult with a brain, not `id`.
fn valid_person(world: &World, id: EntityId, o: EntityId) -> bool {
    o != id
        && world.is_alive(o)
        && world.has::<Brain>(o)
        && world.has::<Identity>(o)
        // No child is ever a valid target (CLAUDE.md, 2026-10-27).
        && demography::is_adult(world, o)
        && crate::systems::law::living(world, o)
}

fn is_market(world: &World, m: EntityId) -> bool {
    world.comp::<Building>(m).is_some_and(|b| b.kind == BuildingKind::Market && !b.demolished)
}

/// The plan `verb` makes for `id` now, with its label (`Err`: why not).
pub fn plan_of(world: &World, id: EntityId, verb: Verb) -> Result<(Plan, String), String> {
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    let carried = world.comp::<Inventory>(id).map_or(0, |i| i.food);
    let home = home_of(world, id);
    let at = here(world, id);
    let tick = world.tick;
    let mk = |target: Option<EntityId>, steps: Vec<ActionInstance>, label: String| {
        Ok((Plan { goal: GoalKind::Idle, target, steps, started_tick: tick }, label))
    };
    let no = |why: &str| Err(why.to_string());
    let walk = |b: EntityId| (at != Some(b)).then(|| go_in(world, b));
    match verb {
        Verb::GoIn(b) => {
            if !opens(world, id, b) {
                return no("that door is not yours to open");
            }
            if at == Some(b) {
                return no("already inside");
            }
            mk(None, vec![go_in(world, b)], format!("Go in ({})", world.name_of(b)))
        }
        Verb::GoToDoor(b) => {
            let Some(s) = go_to_door(world, b) else { return no("no door") };
            mk(None, vec![s], format!("Walk to the door of {}", world.name_of(b)))
        }
        Verb::EatCarried => {
            if !passes(world, id, ActionKind::EatFromInventory, None, LocationKey::Street, |_, _| {}) {
                return no("no food on you");
            }
            mk(None, vec![step(ActionKind::EatFromInventory, None, None)], format!("Eat what you carry ({carried})"))
        }
        Verb::EatHome => {
            let Some(h) = home else { return no("no home") };
            if !passes(world, id, ActionKind::EatAtHome, None, LocationKey::Home, |_, _| {}) {
                return no("the pantry is empty");
            }
            let pantry = world.comp::<Building>(h).map_or(0, |b| b.stock_food);
            let mut steps = vec![];
            if at != Some(h) {
                steps.push(step(ActionKind::GoTo(LocationKey::Home), None, None));
            }
            steps.push(step(ActionKind::EatAtHome, None, None));
            let label = if at == Some(h) { "Eat a meal" } else { "Go home and eat a meal" };
            mk(None, steps, format!("{label} ({pantry} in the pantry)"))
        }
        Verb::BuyMeal(m) | Verb::Groceries(m) => {
            let (free, price) = market_inputs(world, id, m);
            let ok = is_market(world, m)
                && passes(world, id, ActionKind::BuyFood, None, LocationKey::Market, |ws, ctx| {
                    ws.has_coins = coins >= price;
                    ctx.market_free = free;
                    ctx.price = price;
                });
            if !ok {
                return no(if coins < price { "not the coins for it" } else { "nothing to buy here" });
            }
            let mut steps: Vec<ActionInstance> = walk(m).into_iter().collect();
            steps.push(step(ActionKind::BuyFood, None, None));
            if let Verb::BuyMeal(_) = verb {
                steps.push(step(ActionKind::EatFromInventory, None, None));
                return mk(None, steps, format!("Buy a meal and eat it ({price}c)"));
            }
            if home.is_none() {
                return no("no home to store it in");
            }
            steps.push(step(ActionKind::GoTo(LocationKey::Home), None, None));
            steps.push(step(ActionKind::StoreFood, None, None));
            mk(None, steps, format!("Buy groceries and take them home ({price}c a meal)"))
        }
        Verb::SleepHome => {
            let Some(h) = home else { return no("no home") };
            if !passes(world, id, ActionKind::Sleep, None, LocationKey::Home, |_, _| {}) {
                return no("not at home tonight");
            }
            let mut steps = vec![];
            if at != Some(h) {
                steps.push(step(ActionKind::GoTo(LocationKey::Home), None, None));
            }
            steps.push(step(ActionKind::Sleep, None, None));
            mk(None, steps, if at == Some(h) { "Sleep".into() } else { "Go home and sleep".into() })
        }
        Verb::SleepHere => {
            if at.is_some() {
                return no("indoors");
            }
            if !passes(world, id, ActionKind::Sleep, None, LocationKey::Street, |_, _| {}) {
                return no("not here");
            }
            mk(None, vec![step(ActionKind::Sleep, None, None)], "Sleep here on the street".into())
        }
        Verb::SleepBed => {
            let booked = crate::systems::street::booked_hotel(world, id);
            let squat = world.comp::<Squatter>(id).map(|s| s.building);
            let (b, key) = match (booked, squat) {
                (Some(h), _) => (h, key_in(world, id, h, None)),
                (None, Some(s)) => (s, LocationKey::Squat),
                _ => return no("no bed booked and no squat"),
            };
            if !passes(world, id, ActionKind::Sleep, None, key, |ws, _| ws.checked_in = booked.is_some()) {
                return no("not that bed");
            }
            let mut steps: Vec<ActionInstance> = walk(b).into_iter().collect();
            steps.push(step(ActionKind::Sleep, None, None));
            mk(None, steps, format!("Sleep at {}", world.name_of(b)))
        }
        Verb::TakeBed(h) => {
            let Some(price) = bed_for(world, id, h) else { return no("no bed here you can pay for") };
            let key = key_in(world, id, h, None);
            if !passes(world, id, ActionKind::CheckIn, None, key, |ws, ctx| {
                ws.checked_in = false;
                ctx.hotel_available = true;
            }) {
                return no("no bed");
            }
            let mut steps: Vec<ActionInstance> = walk(h).into_iter().collect();
            steps.push(step(ActionKind::CheckIn, None, None));
            let what = if price == 0 { "a free cot".to_string() } else { format!("{price}c") };
            mk(None, steps, format!("Take a bed here tonight ({what})"))
        }
        Verb::Squat(b) => {
            let ok = crate::systems::street::is_derelict(world, b)
                && passes(world, id, ActionKind::Occupy, Some(b), LocationKey::Squat, |ws, _| ws.squatting = false);
            if !ok {
                return no("no room to squat (or you have a home)");
            }
            let mut steps: Vec<ActionInstance> = walk(b).into_iter().collect();
            steps.push(step(ActionKind::Occupy, Some(b), None));
            mk(Some(b), steps, "Squat here (derelict)".into())
        }
        Verb::AskWork(b) => {
            let Some(label) = crate::systems::search::door_offer(world, id, b) else {
                return no("nothing to ask about here");
            };
            let mut steps = vec![];
            if !crate::systems::search::at_door(world, id, b) {
                steps.extend(go_to_door(world, b));
            }
            steps.push(step(ActionKind::Vend, Some(b), None));
            mk(None, steps, label)
        }
        Verb::Work => {
            let Some(j) = world.comp::<Job>(id) else { return no("no job") };
            if !j.on_shift(world.tick_of_day()) || j.last_shift_day == Some(j.shift_key_at(tick)) {
                return no("not on shift now");
            }
            let at = j.employer.map_or("-".to_string(), |e| world.name_of(e));
            // The planner makes the plan (`act`); the steps here are empty.
            Ok((
                Plan { goal: GoalKind::Work, target: None, steps: vec![], started_tick: tick },
                format!("Go to work ({} at {at})", j.role.label()),
            ))
        }
        Verb::Shoplift(m) => {
            let (free, _) = market_inputs(world, id, m);
            let ok = is_market(world, m)
                && passes(
                    world,
                    id,
                    ActionKind::StealFood(StealSource::Market),
                    None,
                    LocationKey::Market,
                    |_, ctx| {
                        ctx.market_free = free;
                    },
                );
            if !ok {
                return no("nothing to steal (or your hands are full of it)");
            }
            let mut steps: Vec<ActionInstance> = walk(m).into_iter().collect();
            steps.push(step(ActionKind::StealFood(StealSource::Market), None, None));
            mk(None, steps, "Steal food (shoplift)".into())
        }
        Verb::BreakIn(b) => {
            let theirs = world.comp::<Building>(b).is_some_and(|x| x.kind == BuildingKind::Home && !x.derelict);
            if !theirs || home == Some(b) {
                return no("not a stranger's home");
            }
            let kind = ActionKind::StealFood(StealSource::Home);
            if !passes(world, id, kind, Some(b), LocationKey::TargetHome, |_, _| {}) {
                return no("nothing in the pantry (or your hands are full)");
            }
            let mut steps: Vec<ActionInstance> = walk(b).into_iter().collect();
            steps.push(step(kind, Some(b), None));
            mk(Some(b), steps, "Break in and steal food".into())
        }
        Verb::Extort(b) => {
            if home == Some(b) {
                return no("your own home");
            }
            if !passes(world, id, ActionKind::Extort, Some(b), LocationKey::TargetHome, |_, _| {}) {
                return no("not here (a gang's work, a lived-in home, no guard near)");
            }
            let mut steps: Vec<ActionInstance> = walk(b).into_iter().collect();
            steps.push(step(ActionKind::Extort, Some(b), None));
            mk(Some(b), steps, "Extort the household".into())
        }
        Verb::Chat(o) => {
            if !valid_person(world, id, o) {
                return no("nobody to talk to");
            }
            let Some(ob) = here(world, o) else { return no("they are on the street (talk happens inside)") };
            let key = key_in(world, id, ob, Some(o));
            if !passes(world, id, ActionKind::Chat, Some(o), key, |_, ctx| ctx.partner = Some(o)) {
                return no("not a place to talk");
            }
            let mut steps: Vec<ActionInstance> = walk(ob).into_iter().collect();
            steps.push(step(ActionKind::Chat, Some(o), None));
            mk(Some(o), steps, format!("Chat with {}", world.name_of(o)))
        }
        Verb::Attack(o) => {
            if !valid_person(world, id, o) || crate::systems::law::held_apart(world, id, o) {
                return no("not someone you can reach");
            }
            let ok = passes(world, id, ActionKind::Attack, Some(o), LocationKey::Street, |_, ctx| {
                ctx.hostile_adjacent = true
            });
            if !ok {
                return no("not now");
            }
            let mut steps = vec![];
            if !crate::systems::law::near(world, id, o, 4) {
                match here(world, o) {
                    Some(ob) => steps.push(go_in(world, ob)),
                    None => {
                        let t = world.comp::<Position>(o).map(|p| p.tile);
                        steps.push(step(ActionKind::GoTo(LocationKey::Picked), None, t));
                    }
                }
            }
            steps.push(step(ActionKind::Attack, Some(o), None));
            mk(Some(o), steps, format!("Attack {}", world.name_of(o)))
        }
    }
}

fn rows(world: &World, id: EntityId, verbs: impl IntoIterator<Item = Verb>) -> Vec<Row> {
    // a body that cannot take an order (in cuffs, sentenced, down) lists nothing
    if refusal(world, id).is_some() {
        return vec![];
    }
    verbs.into_iter().filter_map(|v| plan_of(world, id, v).ok().map(|(_, label)| Row { verb: v, label })).collect()
}

/// The HUD's rows: what the body can do wherever it stands.
pub fn menu_self(world: &World, id: EntityId) -> Menu {
    let verbs = [Verb::EatCarried, Verb::EatHome, Verb::SleepHome, Verb::SleepBed, Verb::SleepHere, Verb::Work];
    Menu { title: "you".into(), rows: rows(world, id, verbs) }
}

/// A click on building `b`: its door, then what can be done there.
pub fn menu_building(world: &World, id: EntityId, b: EntityId) -> Menu {
    let Some(bd) = world.comp::<Building>(b) else { return Menu::default() };
    let door = if opens(world, id, b) { Verb::GoIn(b) } else { Verb::GoToDoor(b) };
    let mut verbs = vec![door];
    match bd.kind {
        BuildingKind::Market => verbs.extend([Verb::BuyMeal(b), Verb::Groceries(b), Verb::Shoplift(b)]),
        BuildingKind::Home if home_of(world, id) == Some(b) => verbs.extend([Verb::EatHome, Verb::SleepHome]),
        BuildingKind::Home => verbs.extend([Verb::BreakIn(b), Verb::Extort(b), Verb::Squat(b)]),
        _ => verbs.push(Verb::Squat(b)),
    }
    verbs.extend([Verb::TakeBed(b), Verb::AskWork(b)]);
    let bed = crate::systems::street::booked_hotel(world, id) == Some(b)
        || world.comp::<Squatter>(id).is_some_and(|s| s.building == b);
    if bed {
        verbs.push(Verb::SleepBed);
    }
    if world.comp::<Job>(id).and_then(|j| j.employer) == Some(b) {
        verbs.push(Verb::Work);
    }
    Menu { title: world.name_of(b), rows: rows(world, id, verbs) }
}

/// A click on `tile` (`on_building`: a roof or a wall, the nearest within
/// `snap`): the menu of the building it names, every unit of a shop-house
/// that shares its rect merged (a Block over a Market over a Vat Farm), the
/// rows in id order. `None`: no building there (the street).
pub fn menu_at(world: &World, id: EntityId, tile: TilePos, on_building: bool, snap: u16) -> Option<Menu> {
    let first = if on_building { building_near(world, tile, snap) } else { building_at(world, tile) }?;
    let rect = world.comp::<Building>(first)?.rect;
    let mut units: Vec<EntityId> = world
        .buildings_by_kind()
        .values()
        .flatten()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|x| !x.demolished && x.rect == rect))
        .collect();
    units.sort_by_key(|b| b.index);
    // a vacant Lot sharing the rect (a big lot's unbuilt plots) has nothing to do at it
    let lot = |b: &EntityId| world.comp::<Building>(*b).is_some_and(|x| x.kind == BuildingKind::Lot);
    if units.iter().any(|b| !lot(b)) {
        units.retain(|b| !lot(b));
    }
    let menus: Vec<Menu> = units.iter().map(|&b| menu_building(world, id, b)).collect();
    let title = menus.iter().map(|m| m.title.clone()).collect::<Vec<_>>().join(" / ");
    let mut rows: Vec<Row> = menus.into_iter().flat_map(|m| m.rows).collect();
    // one door walk for the units of one rect (they share the street outside)
    let mut seen = Vec::new();
    let mut door = false;
    rows.retain(|r| {
        let walk = matches!(r.verb, Verb::GoToDoor(_));
        let keep = !seen.contains(&r.verb) && !(walk && door);
        door |= walk;
        seen.push(r.verb);
        keep
    });
    Some(Menu { title, rows })
}

/// A click on a body: the people verbs (a child: an empty menu, never a target).
pub fn menu_body(world: &World, id: EntityId, o: EntityId) -> Menu {
    let title = world.name_of(o);
    if !valid_person(world, id, o) {
        let child = world.has::<Identity>(o) && !demography::is_adult(world, o);
        return Menu { title: if child { format!("{title} (a child)") } else { title }, rows: vec![] };
    }
    Menu { title, rows: rows(world, id, [Verb::Chat(o), Verb::Attack(o)]) }
}

/// The human picked `verb`: its plan, made again now, runs as a directed
/// plan (Work through the planner, `plan::plan_for`).
pub fn act(world: &mut World, id: EntityId, verb: Verb) {
    let what = format!("{verb:?}");
    if let Some(why) = refusal(world, id) {
        return refuse(world, id, &what, &why);
    }
    let (plan, label) = match plan_of(world, id, verb) {
        Ok(p) => p,
        Err(why) => return refuse(world, id, &what, &why),
    };
    if verb == Verb::Work {
        goal(world, id, GoalKind::Work);
        if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
            p.doing = Some(label);
        }
        return;
    }
    take_wheel(world, id);
    world.abort_plan(id);
    let tick = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(GoalKind::Idle);
        b.goal_since = tick;
        if let Some(p) = b.player.as_mut() {
            p.doing = Some(label.clone());
        }
    }
    let line = steps_line(&plan.steps);
    // The shared things the plan uses are held as a planned NPC's are
    // (`plan::reserve_for`: the food, the partner); a bed by the Hotel the
    // menu named (its `bed_of` reads the NPC's keyed walk, not a picked one).
    let held = Plan {
        steps: plan.steps.iter().filter(|s| s.action != ActionKind::CheckIn).cloned().collect(),
        ..plan.clone()
    };
    plan::install(world, id, plan);
    plan::reserve_for(world, id, &held);
    if let Verb::TakeBed(h) = verb {
        let until = tick + world.config.brain.plan_timeout_ticks;
        world.reserve(id, crate::exec::reservations::ReservationKind::Bed { home: h }, until);
    }
    let name = world.name_of(id);
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: {label} [{line}]"));
}
