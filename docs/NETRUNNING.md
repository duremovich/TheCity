# Netrunning: quickhacks, the rig, the trace

Designed 2026-11-02 with Dylan (six forks decided, listed at the end; roadmap addendum 25). The milestone after
**sight and the shot** (addendum 24), which comes after the player slice (`M18_PLAYER.md`). It builds on M14
(`M14_VIRT.md`: the node graph, ICE, decks, runs, Data, the trace), which is a background economy today: a run never
touches anything a body can see, and the only street-level effects are the gang Raid prelude's doors and robots (camera
Blind fires only from god). This milestone puts the net in the street.

Dylan's ask: "From the player point of view I imagine one can utilize it to shut down security systems, coop
surveillance systems to find people, even blow up reactors and vehicles ... simple nethacks that can be used as sort
of a techno magic system, like Cyberpunk 2077, but also more involved netrunning that requires a stationary rig that a
player or NPC has to be locked into (and vulnerable in the real world) for more advanced hacks in cyberspace." And:
"cyberspace as a traversable plane mapped on the real world ... a netrunner character tapped in to the building you
are robbing to help smooth security or take over defenses. But likewise that squad may need to leave someone behind in
real life to protect their netrunner once they get traced."

**Rules that hold throughout.** One rule for NPCs and the player (the will gates waived per verb for a human-driven
body, as `M18_PLAYER.md` P2 does; nothing else differs). Every number in `assets/config.toml`. Nobody under 18 is ever
a target of a hack, a blast or a counter-run (CLAUDE.md, a hard rule). Proof per phase is a play session or a diary
read, not a test (the doctrine).

## Depends on

- **The sight rule** (`OPEN-288`, addendum 24): one per-storey near field and a 3D far field, replacing
  `law.rs::sees_scene`. A quickhack's reach is sight.
- **The combat model** (`OPEN-179`): wounds, ranged attacks, a blast's radius. Short circuit and Overheat wound
  through it; today a fight is the co-located roll `law::fight_inner`.
- **Room-aware interiors** (`OPEN-285`) only through the sight rule.
- Not needed: Power (reactors are a stub until it lands), real traffic (`OPEN-275`: a parked or driven car is an
  asset today and is hackable as one).

## 1. Devices and subnets (the foundation)

Every hackable thing is a **`Device`**: `{ kind, node, building, tier, owner, state, until }`, kinds `Camera`, `Lock`
(a building's doors, from its lock tier), `Turret`, `Robot`, `Vehicle`, `Deck`, `Chrome(Slot)` (a body's implant,
read from `Kit`, not stored), `Reactor` (no effect until Power). The existing `AssetKind::Camera`, `Robot`, the
vehicles (`Motorcycle`, `Car`, `Truck`, `Flyer`) and `Deck` (`components.rs` `AssetKind`) become devices by a lookup,
not a second store. A node's devices are its building's **subnet**; the node's ICE guards it (M14 § 3).

States: `Normal`, `Blind` (a camera writes nothing), `Looped` (a camera shows a loop: writes nothing, and a guard
watching it sees nothing wrong), `Open` / `Sealed` (a door), `Turned { owner }` (a turret or robot fights for the new
owner), `Wrecked`. Each with an expiry (`until`), in config per hack.

**New: street cameras.** The city posts cameras on each district's Public node (CCTV), a count per district by
`[netrun] cctv_per_district` and its wealth tier; they feed the city's `FactionDb` (M14 § 6) like a corp's cameras
do. This is the surveillance a runner taps to find people.

**The faction database grows** from M14's ring of 32 sightings to a per-person last-seen (`who -> (tile, tick,
confidence)`, capped by `[netrun] db_people`) so a query can answer "where was X last seen".

## 2. Quickhacks (the techno-magic)

**Reach (decided: C).** A quickhack needs its target **in sight**: the hacker's own (the `OPEN-288` rule), or the
view of a camera the hacker holds (Blind/Looped cameras are not eyes; a camera taken with **Hijack** is). A deep run
(§ 5) is the only way to reach what nobody can see.

**The breach.** The first hack on a subnet contests the node's ICE (`security::contest_f`, M14's `att = deck_eff +
hack_w × hacking`, `def = ICE + alarm`); a win **holds the subnet** for `[netrun] breach_hold_min`, and each hack
after it rolls only against the device's own tier. A loss is M14's loss: fry and trace roll (§ 4).

**The deck** (`[[quickhacks]]` in config, a starter set to grow from): each row has `ram`, `cast_ticks` (the hand deck
multiplies it, § 3), `mode` quiet or loud (§ 4), `target`, `effect`, `until`, and its line.

| Hack | Target | Effect |
|---|---|---|
| Ping | a subnet in sight | reveals its devices and the chromed bodies on it (the menu then lists them) |
| Hijack | a camera | the camera becomes the hacker's eyes until it expires |
| Blind / Loop | a camera | dark, or a loop that writes no sightings and alarms nobody |
| Open / Seal | a door | |
| Turn | a turret or robot | `Turned`: fights on the hacker's side until it expires |
| Distract | a device near a guard | the guard walks to it (an interrupt to its plan) |
| Locate | a named person, through the subnet's cameras | their position, live, if a camera there sees them |
| Cripple | one chrome kind on a body | legs: walk slowed; eyes: sight cut; arms: the weapon dropped |
| Short circuit | a chromed body | a wound scaled by the chrome carried (`OPEN-179` wounds) |
| Overheat | a device or vehicle | a blast (§ 7) |

**Body hacks need chrome.** A body with no implant offers nothing to grab: a Purist is immune to Cripple and Short
circuit, and only a device blowing up beside them reaches them. No body under 18 is ever listed.

## 3. Hardware (decided: C)

- **A hand deck** (the existing `Deck` asset, tier 1-3): quickhacks at `[netrun] hand_cast_mult` × the cast time,
  the deck held out and **seen** (a witness sees a hacker, the sight rule), stealable (M14's `Strip`), RAM
  `hand_ram[tier]`.
- **Neural chrome**: a new chrome kind (`Slot` `Neural`, a `[chrome]` row like the others): instant casts, RAM
  `neural_ram[tier]`, invisible, with the chrome rules as they are: `sanity_cost`, Purists refuse it (the creed, flesh
  only; a deck in the hand stays a tool, Dylan 2026-10-19), financed and **repossessed** like any implant
  (`assets.rs` `repossess`; `OPEN-286`'s credit and repo crews read it too).
- Deck tier (or chrome tier) sets RAM and the attack term. RAM refills at `ram_regen_per_min`.

So a Sump runner fumbles with a stolen hand deck, a corp security runner fries you with a thought, and a Purist
hacker is slower and seen but sane and unrepossessable.

## 4. Trace and response (decided: RAM + trace, then C for the response)

**RAM is the budget of one encounter; trace is the bill.** Each subnet keeps a trace meter on each hacker:
quiet hacks add `trace_quiet`, loud `trace_loud` (M14's quiet/loud, V66, carried to quickhacks: quiet costs more RAM
and leaves little trace, loud is cheap and loud). At `trace_lock` the ICE locks on: a `Sighting` of the hacker at
their position (a quickhack's is exact: they are in sight of the subnet; a deep run's decays per hop, M14 V65, to a
district or nothing) and the node's alarm (M14).

**What comes, by owner:**

- **a gang**: bodies, the gang brain's existing response to a sighted runner (M14 V30: a shakedown, a Harvest
  target), dispatched now to the sighting;
- **a corp**: first a **counter-runner** (fast: a run of its own against the hacker's deck, which can fry or **pin**
  them: a pinned runner cannot jack out for `pin_ticks`), then a **strike team** (slow: bodies that travel from the
  corp's Security Office or HQ to the sighting, searching a district when the sighting is one);
- **the city**: the police, the law's normal path from a located suspect (M14 § 6, `Crime::Intrusion` /
  `DataTheft`).

Trace precision is time: a direct run hands over the chair, a hop chain a district to search. A guard at the chair
buys the minutes to finish and jack out (§ 6).

## 5. Deep runs at the rig (decided: C)

M14's runs, made visible and played. **The plane is drawn on the real city** (M14 addendum: nodes and links at their
buildings, over a ghosted map; bridges dashed): the runner's **ghost** moves along the route in cityview while the
body sits in the chair (a picture-in-picture of the body for the player).

**At each guarded node a choice**, with its odds shown: **slip** (quiet: lower odds, little trace), **force** (loud:
better odds, a certain trace), **decoy** (spend RAM to send the trace to another node), **exploit** (spend RAM for a
better attack term), **back out**. NPC runners choose by considerations (lawfulness, courage, the route's fry risk:
M14 § 6's Hack terms); the player chooses. The dice decide; there is no minigame (a player-only skill rule an NPC
cannot play, and a second game).

**What only the rig reaches:** anything out of sight; Data and Ledgers (M14); **subnet takeover** (hold a building's
devices for the length of a heist: its doors, cameras and turrets the runner's to set); **database query** (a
faction's sighting store: where a named person was last seen, city-wide, as confidence decays with age); bridges and
firewalls (M14 addendum, V64/V67: bridges are planted in person, `AssetKind::Bridge`).

The body: M14's `JackedIn` as built (asleep to theft and assault rolls, `jacked_fight_mult`, a hit or an arrest dumps
the run with Dumpshock).

## 6. The crew

- **Overwatch in the raid**: a gang Raid's runner (M14 § 6's prelude and `Overwatch`) holds the target building's
  subnet and acts on it turn by turn as the bodies move through its floors: opens the next door, loops the stairwell
  camera, turns the turret, warns of guards on the lift (a line to the raid). In the overlay: the bodies in the
  building and the runner's ghost riding the same floors.
- **The guard left behind**: the raid's muster decides how many members stay at the chair, a consideration of the
  trace risk (the target's ICE and owner kind) against the strength the raid needs. Each guard is one less in the
  fight.
- **The player** hires a guard for their chair on the contract board, as an NPC would (a contract kind, the same
  machinery). A player-commanded crew stays parked (`BIG_PICTURE_2026-11-02.md` § 3).

## 7. Destruction (decided: B)

**Overheat** makes a device, a vehicle or a deck a **blast**: a radius and a wound per body in it by distance
(`OPEN-179` wounds), a `Wrecked` device or vehicle left on the tile (a wreck asset), a deed every witness in sight
sees (law and gossip as for an assault; the hacker named only if seen or traced). A car blown with its driver in it is
a Murder. Reactors are a `Device` kind with no effect until Power lands; their overload (a block blacked out: dark
cameras, dead ICE, failing chrome) is the scorched-earth deep-run target then.

## 8. NPCs and LOD

- **A new post: the security netrunner**, at Security Offices and corp HQs: the counter-runner (§ 4) and the corp's
  chair guard on its own runs.
- **Gang runners quickhack in fights** (a fight reads the hacks in sight as actions, through the combat model).
- **A hit by hack**: the Hit market's taker may Overheat the target's car or Short circuit their chrome, an approach
  chosen by the GOAP planner like any other (VISION: approaches are action chains, never scripts).
- **Off camera**, a quickhack is a roll inside the fight or episode it belongs to, as Statistical fights are; a deep
  run promotes its runner to Coarse as M14 does (V13).

## 9. Phases (one fresh agent each; proof by a play session or diary read)

1. **Devices**: the `Device` lookup, states and expiries, street cameras, the per-person last-seen. Proof: Ping a
   building, see its devices; a camera's sightings of the player in the db.
2. **Quickhacks and hardware**: the deck, the breach, RAM, the hand deck's cast time and visibility, the Neural chrome
   kind; the player's hack menu on a body or a device in sight (P2's menu). Proof: Blind a camera, Open a door,
   Cripple a chromed guard's legs.
3. **Trace and response**: the meter, the lock, the response by owner (counter-runner, strike team, police). Proof: a
   loud hack brings a strike team to the player's tile; a quiet one does not.
4. **Deep runs, visible**: the route on the overlay, the per-node choices, subnet takeover, the database query, the
   picture-in-picture. Proof: find a named person through a corp's database from a Bar terminal.
5. **Crew and overwatch**: the raid's runner acting per turn, the chair guards in the muster, the player's guard
   contract. Proof: a gang raid's diary where the runner opens doors as the bodies climb, and a traced runner whose
   guard fights the strike team.
6. **The blast**: Overheat on devices, vehicles and decks, the wreck, the witnesses. Proof: a car blown on the street
   and the deed travelling (P4's log).
7. **The view**: the overlay and the ghost, hacked devices drawn (a dark camera, an open door, a turned turret), the
   cast visible on a hand deck.

## Decisions (Dylan, 2026-11-02)

| # | question | decided |
|---|---|---|
| 1 | What a quickhack needs to reach its target | **C**: sight, with hijacked cameras as eyes; deep runs reach the rest |
| 2 | The hardware | **C**: a hand deck (slow cast, seen, stealable, small RAM) and neural chrome (instant, invisible, more RAM, sanity, Purists refuse, repossessable) |
| 3 | What stops spam | **C**: RAM per encounter, trace as the consequence; quiet costs RAM, loud costs trace |
| 4 | How a deep run is played | **C**: the route on the overlay with a choice at each guarded node, odds shown, the dice decide; no minigame. Dylan: cyberspace as a traversable plane on the real world, the runner present in the squad's heist |
| 5 | What comes for a traced runner | **C**: by owner; a corp's counter-runner (fast) then strike team (slow); trace precision sets where they go |
| 6 | Destruction | **B**: a blast model for devices, vehicles and decks through `OPEN-179`'s wounds; reactors a stub until Power ("we can add complexity later") |

Defaults the orchestrator chose (overturnable): the breach holding the subnet; the security netrunner as a new post;
the starter `[[quickhacks]]` list; the reactor as a stub; the player's chair guard as a contract (a commanded crew
stays parked).
