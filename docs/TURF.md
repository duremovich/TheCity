# Turf: ground a gang holds, pays for and fights over (2026-10-28)

Dylan 2026-10-28, on Shadow V14 ("gang life without a raid", `OPEN-241`; the empty street, `OPEN-217`): do gangs need
turf, as San Andreas had? Taken from SA: turf is **visible and local** (blocks you can colour on the isometric map),
a war is **an event with a shape** (provoked, answered, held or lost), the loser **comes back for it**, and turf
**pays**. Not taken: enemy waves. A city of 100k at war every week is noise; wars here end (exhaustion, a truce, the
law's crackdown). Built in pressure pass round 6 as two phases: V2 (§ 1-2) and V5 (§ 3).

## What is there today (codemap 2026-10-28)

- A gang holds **Homes**: `Gang.territory` (`components.rs` `Gang`), a Home flips after `CLAIM_HELD` (3) shakedowns
  (`gang::claim`, `Building.claim`); `districts::update_control` derives a district's controller from held Homes.
- Turf pays 2c a day a held Home in tribute (`gang::daily_economy`); dealing spots, pimp pitches (`vice::name_pimps`,
  `round_pitch`), Preach spots and protection ground read held districts.
- **Raids never move territory** (`raid::settle` moves treasury, goods, the sack). `World::rival_of` is the biggest
  gang, not the nearest; `faction::rival_in_reach` drops a rival whose Hideout is past `[crew]
  raid_target_reach_ticks` (240), which at 100k shuts Raid and most Retaliate; `raid_cooldown_days` is 30; members
  past `duty_reach_ticks` from a muster sit it out; off screen there is no muster (`fviolence::stat_gang_day`).
- So V14: 0 musters, 0 raids in 16 gang diaries; one door paying three gangs in 15 hours.

## 1. The block (V2)

- **Turf cells**: a fixed grid of `[turf] cell_tiles` (a few street blocks; the view colours them later). A cell's
  **holder** is the gang with the most **presence** over `[turf] presence_days`, past `hold_min` and ahead of the
  next by `hold_margin`: held Homes in the cell (today's claims), members' hours there (hanging, dealing, the
  corner), the gang's spots (the Hideout, a front, a deal spot). Else the cell is open or **contested** (two gangs
  close). Derived at the midnight pass from both tiers (a Statistical member's day credits his Home's cell and his
  order's cell), so the off-screen city holds turf the same way.
- **The corner**: each held cell has one (the busiest street tile a member can reach). A member with no order and
  no shift **holds the corner** (a GangWork arm, his own cell or the nearest held one within reach); the dealer works
  it. This is where the player sees a gang.
- A holder change is one event ("The Rust Saints hold the blocks around Market#412 now: 9 of their people on the
  corner most nights"), the losing gang's `LostTurf` shock (a grudge), the district's control still derived as
  today (from cells, not Homes).
- `rival_of` becomes **the neighbour**: the gang holding the most cells bordering ours (ties by presence), so a
  rival is always in reach.

## 2. Turf that pays (V2)

- **Tribute and protection** by cell: the shops and Homes in a held cell pay (today's per-Home tribute and
  `protection.rs` ground read the cell's holder); a shop in a contested cell is squeezed by both or shuts early.
- **The trades on the turf**: the deal spot is the corner; the **pimp's pitch opens in a held cell where members
  live** and the pimp is named among them (replaces U6's `pimp_trip_ticks` bound, which emptied the 100k street:
  pimps 1.63 -> 0.2 a day); the preacher preaches on the gang's own corner.
- **Recruits from the turf**: JoinGang draws the held cells' residents first (the corner is where you are asked).
- **The residents** read the colours: a held cell is quiet (the gang keeps its own street), a contested one is
  avoided after dark by those who can choose their route, and a family with children moves out of a contested cell
  at the next rehouse when it can.

## 3. The war (V5)

- **Provocation** on a held cell by another gang: a shakedown, a deal or a hang on its corner, a beating of a
  holder's member there, a tag. Each raises the cell's **heat** for the holder, seen by holders present (`Trespass`).
- **The answer**: past `answer_heat` the holder musters **that cell's and the neighbouring cells' members** to the
  corner within `answer_window_hours` (local, so reach is no gate), and pushes the trespassers off: a brawl on the
  corner, the law's attention if it is seen.
- **Hold or flip**: a corner fight won keeps the cell; a cell the attacker out-presences, or whose defenders lose
  the corner fight, flips (its held Homes' claims go with it). A raid on a Hideout can now move the cells around it.
- **Counter-attack**: the loser's `LostTurf` grudge scores Retaliate on that cell within days (not the 30-day raid
  cooldown, which stays for Hideout raids).
- **Peace**: a war ends. **Exhaustion** (members dead, jailed or desisted in the war over `exhaust_days` past
  `exhaust_share`) turns a side to LieLow; two exhausted leaders strike a **truce** (an event, a border, held
  `truce_days`); the **law's crackdown** on a contested cell (`law::crackdown_turf`) cools both sides.
- **Off screen**: a war in a Statistical cell resolves as a daily roll on the two sides' presence and strength,
  with the same events; bodies in camera range are promoted (`lod::raid_window`'s path).

## The rule

**No one under 18 is a target or a combatant**, in a corner fight, a push-back, a squat eviction or a crossfire:
`ages::is_protected` gates every pick (today's raid paths do; `gang::contested` and the squat-eviction fight
`gang.rs` ~815 do not, and V2 adds the gate). A family moving out of a contested cell is the children's protection,
not a harm.

## Proof

V2: a gang_member, gang_leader, dealer and pimp shadow at 20k (a member on his corner, the pitch in the turf, a
holder change told); the cell count held per gang and pimps a day, 20k seed 42 x 30 before and after. V5: a
gang_member, gang_leader and raider shadow (a provocation answered, a cell flipped and taken back, a truce); wars
started, cells flipped, truces, gang deaths, 20k seed 42 x 30 before and after, and one 100k x 30 at the round's
close. Every number in `[turf]`.
