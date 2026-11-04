use super::*;

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
    /// C4: a downed body: drag it clear to its Clinic (the M16b Rescue's steps).
    Help(EntityId),
    /// C5: a knocked-out body beside you: go through its pockets (`combat::rob`, a Theft).
    Rob(EntityId),
    // --- M18 P3: talk (a body beside you; `talk`)
    /// A person: walk over to stand beside them.
    WalkOver(EntityId),
    /// A Charm: they warm to you (the edge's affinity and trust).
    Befriend(EntityId),
    /// `Flirt` at a Bar or a Market: a date in a courtship (both 18+).
    Court(EntityId),
    /// A Persuade on Coins: a loan (a debt on the edge).
    Loan(EntityId),
    /// An Intimidate on Coins: their coins, by fear.
    Threaten(EntityId),
    /// A Charm on a Job: a word put in at their workplace (a lead), then the ask at its door.
    AskWorkOf(EntityId),
    // --- M18 P3: the board
    /// Take an open record (`contracts::player_take`): at its Fixer's desk, or from its buyer.
    Take(u64),
    /// Walk to a Fixer's desk and get known (`Network`): its board.
    Network(EntityId),
    /// Go on with the record you hold (the Contract goal's plan).
    WorkJob,
    /// A city offer (its `made` tick): take it (`true`) or turn it down.
    Answer(u64, bool),
    /// M18 P5: a witness's demand (an offer, its `made` tick): go for him.
    Resist(u64),
    /// M18 P5h: at a letting desk (a Civic Hall, a corp's office, a gang's
    /// Hideout, a Block's super or owner): apply for this Home (the city's
    /// list, a corp's flat, a room), the FindHome goal's `Apply`.
    Apply(EntityId),
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
            Verb::Befriend(_) | Verb::Loan(_) | Verb::AskWorkOf(_) => &[
                "think: no NPC goal makes this move unasked (the target's resistance is the roll's)",
            ],
            Verb::Threaten(_) => &[
                "lawfulness: an NPC leans on a stranger only as a gang's collector (the target's courage is the roll's)",
            ],
            Verb::Court(_) => &["cost: sociability (the planner's)", "think: the intimacy goal's gate"],
            Verb::Take(_) => &[
                "mode A: accept_min (the price against the risk and the buyer's honour)",
                "the gun gate's day-wage term (a man with a good day job takes no work)",
                "gun_lawfulness (an illegal kind's lawfulness ceiling)",
            ],
            Verb::Network(_) => &["the Contract goal's gun gate wage term and its lawfulness and wealth weights"],
            Verb::GoIn(_)
            | Verb::GoToDoor(_)
            | Verb::Squat(_)
            | Verb::Work
            | Verb::WalkOver(_)
            | Verb::WorkJob
            | Verb::Answer(..)
            | Verb::Resist(_) => &[],
            Verb::Help(_) => &["the Rescue's tie (kin, a friend, a colleague, a called stranger's term): the human's will"],
            Verb::Rob(_) => &["lawfulness: the NPC winner robs a body it knocked out only under [combat] ko_rob_lawful"],
            Verb::Apply(_) => &[
                "think: the FindHome goal's gates (the cheapest deposit in hand, the days homeless, off shift)",
            ],
        }
    }
}

/// One menu row.
#[derive(Clone, Debug)]
pub struct Row {
    pub verb: Verb,
    pub label: String,
    /// M18 P3: a row shown greyed: why it cannot be done now (cots full,
    /// the post gone, too far to talk); `None`: it can.
    pub why_not: Option<String>,
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

pub(super) fn step(action: ActionKind, target: Option<EntityId>, tile: Option<TilePos>) -> ActionInstance {
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

pub(super) fn home_of(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Household>(id).and_then(|h| h.home)
}

pub(super) fn here(world: &World, id: EntityId) -> Option<EntityId> {
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
pub(super) fn valid_person(world: &World, id: EntityId, o: EntityId) -> bool {
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
        Verb::Apply(home) => {
            let Some(desk) = crate::systems::housing::desk_of(world, home) else { return no("no desk lets it") };
            if !crate::systems::housing::desk_hours(world) {
                let c = &world.config.housing;
                return no(&format!("the desk is shut (open {:02}:00-{:02}:00)", c.desk_open, c.desk_close));
            }
            let label = crate::systems::housing::row(world, id, home)?;
            Ok((crate::systems::housing::plan_for(world, id, home, desk), label))
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
            // M18 P5h: a Capsule Hotel's bed is a capsule kept while each night is paid at 08:00.
            let label = if price == 0 {
                "A free cot for the night".to_string()
            } else {
                format!("A bed for the night ({price}c; the capsule is yours while you pay at 08:00)")
            };
            mk(None, steps, label)
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
            // M18 P3: on the street, beside them (`[talk] street_tiles`)
            let Some(ob) = here(world, o) else {
                if !passes(world, id, ActionKind::Chat, Some(o), LocationKey::Street, |_, ctx| {
                    ctx.partner = Some(o);
                    ctx.street_partner = true;
                }) {
                    return no("not now");
                }
                let mut steps = vec![];
                if !beside(world, id, o) {
                    let t = world.comp::<Position>(o).map(|p| p.tile);
                    steps.push(step(ActionKind::GoTo(LocationKey::Picked), None, t));
                }
                steps.push(step(ActionKind::Chat, Some(o), None));
                return mk(Some(o), steps, format!("Chat with {}", world.name_of(o)));
            };
            let key = key_in(world, id, ob, Some(o));
            if !passes(world, id, ActionKind::Chat, Some(o), key, |_, ctx| ctx.partner = Some(o)) {
                return no("not a place to talk (a Market, a Bar, a Farm, a home, or the street)");
            }
            if at != Some(ob) && !opens(world, id, ob) {
                return no("their door is not yours to open");
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
            // C4: the odds, as the talk rows show theirs (mock fights on the
            // beat's rules, the likely joiners at their chance).
            let odds = crate::systems::combat::odds(world, id, o);
            let near = odds.theirs - 1.0;
            // C5: the duel's odds, and the whole fight's when their side may grow
            let label = if near >= 0.5 {
                let one = crate::systems::combat::duel_odds(world, id, o);
                format!(
                    "Attack {} (one on one {:.0}%; {near:.1} of theirs may jump in: {:.0}%)",
                    world.name_of(o),
                    100.0 * one,
                    100.0 * odds.p
                )
            } else {
                format!("Attack {} ({:.0}%)", world.name_of(o), 100.0 * odds.p)
            };
            mk(Some(o), steps, label)
        }
        Verb::Help(o) => {
            let Some(rec) = world.wounded.get(&o) else { return no("not down") };
            if rec.stable {
                return no("already seen to");
            }
            if rec.rescuer.is_some_and(|x| x != id) {
                return no("someone has them");
            }
            let Some(clinic) = rec.clinic else { return no("no Clinic to take them to") };
            let steps = vec![
                step(ActionKind::GoTo(LocationKey::Victim), Some(o), None),
                step(ActionKind::CarryWounded, Some(o), None),
                step(ActionKind::GoTo(LocationKey::Clinic), Some(clinic), None),
                step(ActionKind::Stabilise, Some(clinic), None),
            ];
            let label = format!("Drag {} clear to {}", world.name_of(o), world.name_of(clinic));
            Ok((Plan { goal: GoalKind::Rescue, target: Some(o), steps, started_tick: tick }, label))
        }
        Verb::Rob(o) => {
            if !crate::systems::combat::knocked_out(world, o) {
                return no("not knocked out");
            }
            if !crate::systems::law::near(world, id, o, 2) {
                return no("too far: walk over first");
            }
            mk(Some(o), vec![], format!("Go through {}'s pockets", world.name_of(o)))
        }
        Verb::WalkOver(o) => {
            if !valid_person(world, id, o) {
                return no("nobody to walk to");
            }
            if beside(world, id, o) {
                return no("beside them already");
            }
            let s = match here(world, o) {
                Some(ob) if opens(world, id, ob) => go_in(world, ob),
                Some(ob) => match go_to_door(world, ob) {
                    Some(s) => s,
                    None => return no("no door"),
                },
                None => step(ActionKind::GoTo(LocationKey::Picked), None, world.comp::<Position>(o).map(|p| p.tile)),
            };
            mk(None, vec![s], format!("Walk over to {}", world.name_of(o)))
        }
        Verb::Court(o) => {
            if !valid_person(world, id, o) {
                return no("nobody to court");
            }
            if !crate::systems::social::suitor(world, id, o) || !crate::systems::social::suitor(world, o, id) {
                return no("not free to court (married, kin)");
            }
            let Some(ob) = here(world, o).filter(|&b| {
                world.comp::<Building>(b).is_some_and(|x| matches!(x.kind, BuildingKind::Bar | BuildingKind::Market))
            }) else {
                return no("court at a Bar or a Market");
            };
            if world.rejected_recently(id, o) {
                return no("turned you down this week");
            }
            let key = key_in(world, id, ob, Some(o));
            if !passes(world, id, ActionKind::Flirt, Some(o), key, |_, ctx| ctx.partner_reachable = true) {
                return no("not now (married?)");
            }
            let mut steps: Vec<ActionInstance> = walk(ob).into_iter().collect();
            steps.push(step(ActionKind::Flirt, Some(o), None));
            mk(Some(o), steps, format!("Court {} ({:.0}%)", world.name_of(o), 100.0 * court_p(world, id, o)))
        }
        Verb::Befriend(o) | Verb::Loan(o) | Verb::Threaten(o) | Verb::AskWorkOf(o) => {
            let (m, what) = talk_move(world, id, verb)?;
            let (p, refused) = crate::systems::moves::p_of(world, &m);
            if refused {
                return no("they will not hear you out (your standing)");
            }
            talk_ready(world, id, o)?;
            mk(Some(o), vec![], format!("{what} ({:.0}%)", 100.0 * p))
        }
        Verb::Take(c) => {
            if let Some(why) = crate::systems::contracts::player_take_refusal(world, id, c) {
                return Err(why);
            }
            let rec = &world.contracts[&c];
            let label = format!("Take the {}", job_line(world, rec));
            let Some(f) = rec.broker else { return mk(None, vec![], label) };
            let steps = vec![
                step(ActionKind::GoTo(LocationKey::Seller), Some(f), None),
                step(ActionKind::TakeJob, Some(f), None),
            ];
            Ok((Plan { goal: GoalKind::Contract, target: Some(f), steps, started_tick: tick }, label))
        }
        Verb::Network(f) => {
            if !crate::systems::contracts::fixer_open(world, f) {
                return no("shut");
            }
            let best = world.comp::<crate::components::Skills>(id).map_or(0.0, |s| {
                crate::systems::combat::skill_power(world, s)
                    .max(s.stealth)
                    .max(s.hacking)
                    .max(s.persuasion)
                    .max(s.intimidation)
            });
            let min = world.config.fixers.gun_skill_min;
            if best < min {
                return Err(format!("the fixer has no use for you (best skill {best:.2} < {min:.2})"));
            }
            let steps = vec![
                step(ActionKind::GoTo(LocationKey::Seller), Some(f), None),
                step(ActionKind::Network, Some(f), None),
            ];
            let far =
                world.comp::<Building>(f).zip(world.comp::<Position>(id)).map_or(0, |(b, p)| b.door.manhattan(p.tile));
            let open = world.comp::<crate::contract::Broker>(f).map_or(0, |k| {
                k.book
                    .iter()
                    .filter(|c| {
                        world.contracts.get(c).is_some_and(|x| x.status == crate::contract::ContractStatus::Open)
                    })
                    .count()
            });
            Ok((
                Plan { goal: GoalKind::Contract, target: Some(f), steps, started_tick: tick },
                format!("Get known at {} (its board: {open} open; {far} tiles)", world.name_of(f)),
            ))
        }
        Verb::WorkJob => {
            let Some(c) = held_job(world, id) else { return no("no job held") };
            let rec = &world.contracts[&c];
            mk(None, vec![], format!("Go on with the {}", job_line(world, rec)))
        }
        Verb::Answer(made, take) => {
            let Some(o) = offer_of(world, id, made) else { return no("the offer lapsed") };
            let until = crate::time::clock(o.until);
            let label = match (&o.what, take) {
                // M18 P5: a witness's demand
                (OfferKind::Blackmail { witness, demand }, true) => {
                    format!("Pay {} {demand}c to keep quiet (till {until})", world.name_of(*witness))
                }
                (OfferKind::Blackmail { witness, .. }, false) => {
                    format!("Refuse {} (he goes to the law)", world.name_of(*witness))
                }
                (_, true) => format!("Take it: {} (till {until})", o.line),
                (_, false) => format!("Turn down: {}", o.line),
            };
            mk(None, vec![], label)
        }
        Verb::Resist(made) => {
            let Some(o) = offer_of(world, id, made) else { return no("the offer lapsed") };
            let OfferKind::Blackmail { witness, .. } = o.what else { return no("not a demand") };
            mk(None, vec![], format!("Go for {} (shut him up)", world.name_of(witness)))
        }
    }
}
