# Combat: the engagement, wounds, three skills

Designed 2026-11-02 with Dylan (four forks decided, listed at the end; roadmap addendum 26; `OPEN-179`). The player
slice's P4 found that a fight reads as one dice roll (slice decision 5), so the close-range half of this design is the
slice's next work, ahead of P5. The ranged half comes with sight and the shot (addendum 24), and the blast with
netrunning (`NETRUNNING.md` § 7).

**Today:** every fight is one call, `law::fight_inner` (`systems/law.rs`). It makes one `p_win` roll (`fighting`,
courage, Kit reflex, the weapon tier's `p_win_term`, clamped 0.1-0.9), then one death roll (`fight_death_p x (1 +
fighting) x kill_mult x (1 - armour)`), which a second draw may turn into the M16b downed state (`wounded.rs`). There
are 23 call sites (raids, crushes, Hunts, arrests, muggings, the Attack action, a guard over a corpse), and each one
branches on `(winner, loser, died)` on the spot. A weapon is an asset with a tier and nothing else. A tick is one game
minute (about 1.4 s of real time at the view's X1).

Dylan's ask (`OPEN-179`, 2026-10-22): specific skills, gear and chrome that set stats and can be weapons themselves,
and "fights that play out over ticks between moving bodies. The player orders an attack and the sim resolves it
(Kenshi, KotOR), never blow by blow."

**Rules that hold throughout.**
- **One rule for NPCs and the player.** A human-driven body only adds orders (§ 5).
- **Every number lives in `assets/config.toml`**, in a new `[combat]` section and the `[[weapon_kinds]]` rows.
- **No child is ever harmed.** No beat lands on anyone under 18, and a minor is never a valid joiner or target. This
  is checked at the start and again at every hit; `resolve_fight_mods`'s backstop stays.
- **Each phase is proven by a diary read or a play session.** Keyed draws stay a habit.

## 1. The engagement (decided: live)

A fight is an **`Engagement`** in `World::engagements` that lives across ticks:
- **Its fields:** the place (tile, building, floor), two **sides** (members, each member's stance), the purpose (who
  started it and why), the caller's mods, the start tick, and a ring log of its beats (read by the view and the story
  log).

Each tick, `combat::step` runs `[combat] beats_per_tick` **beats** (6 to start, about 10 s each).

**What happens in a beat.** Every standing participant acts by stance and state:
1. **Pick a target** on the other side within reach (a melee weapon's `reach`; a ranged weapon within `close_tiles`
   until § 7).
2. **Roll the attack:** the skill for the weapon (melee or shooting), plus the weapon's accuracy and tier, plus
   chrome, against the target's evasion, cover (ranged only, § 6) and stance.
3. **On a hit, roll a region** (weights in config), then a severity from the weapon's damage table, less the armour
   on that region.

A wound takes effect at once (§ 2): an arm wound weakens the next attack, a leg wound slows dodging and fleeing, and a
head hit can knock the body out.

**Morale and the break.** Each participant has a morale that starts from courage, stance, the numbers on each side and
side loyalty (gang, kin, friend). It falls with the participant's own wounds and with the side's losses. Below
`break_at` the body **breaks**:
- It **flees** if its legs allow and there is a way out.
- Otherwise it **yields**, if the other side's stance accepts yields. Subdue and hold back always accept. All out
  accepts unless the purpose is lethal: a Hit, or a Hunt with `kill_p`.

**Most fights end in a break, not a death.** Deaths come from weapons (a grave head or torso wound) and from bleeding
out. A knockout leaves the loser lying there, robbed if the winner wants it, and waking hurt.

**The end.** An engagement ends when:
- one side has nobody standing (fled, yielded, knocked out, downed, dead);
- both sides disengage;
- or it hits `max_beats`.

Its result is a `FightOutcome`: the winning side, and each participant's end (`Won`, `Fled`, `Yielded`,
`KnockedOut`, `Downed`, `Dead`) with the wounds dealt. The purpose's **on-end hook** runs the caller's tail: the crime,
the memories, `weapons::after_fight`, fashion, braindance, `turf::beaten`.

**Joiners.** At each beat, a Full body within `join_tiles` that has a reason can join a side: a gang-mate, kin, a
friend above `join_bond`, or a guard on duty who sees the assault (the guard joins the victim's side and arrests after).
Its courage decides. Joining is a rule in `combat::step`, not a GOAP goal.

**Moving bodies.**
- A body that flees gets a flee plan and leaves.
- The winner may give chase, and the purpose decides whether it does: a mugger does not, a Hunt does. An engagement
  follows a chase while the bodies stay within reach, then ends.

**LOD.** Only Full bodies are live.
- **In camera range,** a Coarse or Statistical party is promoted first (nothing in camera range is Statistical).
- **Off camera,** **`resolve_now`** runs the same beat loop to its end inside one call, with no movement and no
  joiners beyond those listed at the start. The off-screen city gets the same distribution of outcomes for no extra
  state.

**The callers.** Phase C1 (§ 10) makes `resolve_now` the body of `fight_inner`. Every caller keeps its synchronous
`(winner, loser, died)`, now with a `FightOutcome` beside it, so the whole city gets wounds, skills and morale in one
step. C3 moves the callers to live engagements for Full bodies, with each caller's tail moved into its purpose's on-end
hook. A caller that needs the answer before its action can finish (`grudges::guard_contests` over a Strip) holds its
action as `Running` until the engagement ends.

## 2. Wounds (decided: body regions)

A **`Wounds`** component holds a small list of `Wound { region, severity, bleeding, since, treated, by }`:
- **The regions:** Head, Torso, ArmL, ArmR, LegL, LegR.
- **The severities:** Graze, Wound, Grave.

**Effects** come from a config table (region x severity) and are read through `health::effects`, the hook that
already carries `frail_fight`:

| region | effect |
|---|---|
| arms | melee and shooting |
| legs | evasion, walk speed (`walk_mult`) and fleeing |
| head | knockout chance, work |
| torso, and any Grave | bleeding |

**Bleeding.** An untreated bleeding wound drains a blood meter. At zero the body goes down: the existing M16b
`wounded` state, with the Rescue and the Trauma Team, then bleeding out. How each severity stops bleeding:
- **A Graze** stops on its own.
- **A Wound** needs first aid: a friend beside the body, a street medic, or a stop roll each hour.
- **A Grave** wound needs a Clinic.

**Wounds that last.**
- **Healing** takes `heal_days` per severity, cut by treatment at a Clinic (coins, `Flow::Treatment`). A wound
  that is not healed is visible on the body: a limp, a sling, a bandage.
- **Treatment as a goal.** A body that is hurt and can afford a Clinic goes there. One that cannot afford it lives
  with the wound, and that is the story.
- **Lost shifts.** A manual job (by role, in config) refuses a worker with an arm or leg Wound or worse, and the shift
  and its pay are lost.
- **Scars.** A Grave wound left untreated may heal badly, leaving a permanent maim on its region and a scar (scars are
  sim state).
- **Lost limbs.** A Grave limb wound may cost the limb. The body is maimed until a ripperdoc fits a chrome arm or leg,
  which brings the chrome economy into the fight.

**Off camera.** A Statistical violence roll (`fviolence`, `stat_kill_p`) splits its outcome by the same
distribution (dead, grave, wounded, knocked out) and writes the wound, so a body promoted later is already limping.

## 3. Skills (decided: melee, shooting, evasion)

`Skills.fighting` becomes **`melee`**, and two skills are added: **`shooting`** and **`evasion`**.

| skill | seeded from | grows when |
|---|---|---|
| melee | the old `fighting` | the body lands a melee hit |
| shooting | background: a per-role base in config (guards, police, gang roles, ex-military) times a keyed draw | the body lands a shot |
| evasion | a keyed draw, an age curve and Kit reflex | the body dodges |

Growth replaces today's flat +0.01 to the winner.

**The readers.** `fighting` is read in 35 places across 17 files: the odds in `moves`, `competence`, the ability
gates in `contracts`, the Hit's quote and more. They all read one function, **`combat::power(world, id)`** (the
better of melee and shooting with a carried gun, plus a share of evasion), so no gate picks its own mix. A robot's
`robot_fighting[tier]` sets all three skills.

## 4. Weapons, gear and chrome

**Weapon kinds.** A weapon asset gains a **kind** from `[[weapon_kinds]]`: knife, blade, club/pipe, pistol, SMG,
shotgun, rifle and sniper rifle, with unarmed implicit. Each row gives:

| field | what it sets |
|---|---|
| family | Melee or Ranged |
| reach / range bands | how far it hits |
| accuracy | the attack roll |
| damage | weights over the severities |
| lethality | the death chance on a Grave head or torso wound |
| draw | the beats it takes to ready |
| noise | the radius it is heard in (addendum 24's sound) |
| conceal | its term in today's concealment contest, in place of the tier's |
| price | what it costs |

**The tier stays, as quality:** a multiplier on accuracy and damage, and wear. At the migration the existing stock is
mapped onto kinds by tier and owner class.

**Gear.** Kit's `armour` becomes armour per region: a vest covers the torso, and subdermal plating covers everything
by its share. `strength` adds melee damage, and `reflex` adds evasion and goes first in a beat.

**Chrome as stats and as weapons.** The `[[chrome]]` rows gain combat effects:

| chrome | effect |
|---|---|
| mantis blades | a melee kind on the arms: cannot be taken off the body, seen when drawn |
| gorilla arms | melee damage, and an unarmed hit becomes lethal |
| a Sandevistan | `extra_actions` per beat for a few beats, then a cooldown and a sanity cost per use |
| a Kerenzikov | a dodge bonus on the first attack against the body, or when it is hurt |
| cyberlimbs | replace a lost limb; a hit there wrecks the chrome rather than wounding the body, and the ripperdoc repairs it |

## 5. Stance and orders (decided: stance and orders, the clock slowed)

Every participant has a **stance**:
- **All out:** lethal. It accepts no yield when the purpose is lethal.
- **Defensive:** more evasion, less attack, and the body breaks later.
- **Subdue:** non-lethal. It prefers blunt and unarmed blows, a knockout replaces a Grave wound where it can, and it
  accepts yields. This is a guard's arrest.
- **Hold back:** strikes only when attacked, and flees early.

An NPC's stance comes from the purpose and its personality: a Hunt goes all out, an arrest subdues, a mugging holds
back, a bouncer subdues.

**The player's orders** are `PlayerCommand`s that apply at the next beat: Attack (a body), Stance, Draw or Holster
(a weapon), Take cover (the best tile within `cover_tiles`, § 6), Flee, Yield, and Help (drag the downed away, the
existing Rescue).

**The clock.** While the possessed body is in an engagement, cityview runs the sim at `[view] combat_speed`: the whole
sim slows on one clock, so a 10 s beat plays in about a second. Space pauses the game, and normal speed comes back when
the engagement ends. The view replays the beat log (a swing, a hit or a miss, the region, a dodge, a stagger, someone
fleeing) as animation cues and as story-log lines.

**The HUD** shows:
- the participants and their stances;
- the wounds you can see: your own exactly, other bodies' as they show (limping, bleeding);
- the odds on an Attack, as P3's talk rows show them;
- a morale hint ("he's wavering").

## 6. Cover (abstract until the interiors)

`combat::cover_at(world, pos, from) -> f32` comes from the kind of tile or building: the open street is 0; a
doorway, an alley, street furniture or an interior give partial cover. A ranged hit chance is multiplied by (1 -
cover); melee ignores cover. When room-aware interiors (`OPEN-285`) and the sight rule (`OPEN-288`) land, geometry
replaces the table behind the same function.

## 7. Ranged and the long shot (with addendum 24)

Range bands (close, medium, long, extreme) per weapon kind; line of fire from the far-field ray; scope, light, angle.
A shot from outside an engagement opens one, with the shooter on one side and the target alone on the other: a
sniper's engagement. A miss is heard and seen. Built with the sight milestone, not in the slice.

## 8. The blast (with netrunning § 7)

`combat::blast(world, origin, radius, power)` gives a wound to each body in the radius, by distance and with region
weights (torso and legs), through the same `Wounds`. Grenades join as a weapon kind then. Overheat, a car blown up and
a reactor all go through it.

## 9. The city's numbers

**Murders and fight deaths will move.** Death now comes from wounds, not from `fight_death_p`. The phase that changes
it prints both numbers before and after, from one run each on the same seed. A drop is a FINDING, and the tone comes
back through features, never a knob (doctrine 5). Wounds also add pressure on their own: lost shifts, Clinic bills, a
limp that makes the next mugging easier.

## 10. Phases (one fresh agent each; proof by a diary read or a play session)

1. **C1 the beat (synchronous).**
   - **Build:** `systems/combat.rs` with the beat loop as `resolve_now`, which becomes `fight_inner`'s body (every
     caller keeps its API); `Wounds` with the regions, severities, bleeding and effects through `health::effects`,
     bleeding down into the M16b downed state; morale and the breaks; the three skills behind `combat::power`;
     `[[weapon_kinds]]` with the stock mapped; stances from the caller's mods and the purpose; `[combat]`.
   - **Proof:** diaries of a mugger, a gang soldier in a turf war and a guard making arrests, showing wounds,
     yields, flights and knockouts; fight deaths and Murders before and after, one run each.
2. **C2 wounds that last.**
   - **Build:** healing over days; the treatment goal and the Clinic's bill; first aid; manual jobs refused (a lost
     shift); scars and maims; a lost limb and the ripperdoc's chrome limb; the off-screen rolls writing wounds.
   - **Proof:** a diary where someone limps for a week, loses shifts and can't pay the Clinic; a chrome arm fitted
     after a lost one.
3. **C3 live.**
   - **Build:** the `Engagement` across ticks for Full bodies; the purpose on-end hooks for the 23 callers; joiners;
     fleeing as movement; the chase; chrome in the fight (Sandevistan, mantis blades, Kerenzikov, cyberlimbs).
   - **Proof:** a raid's diary where gang-mates join and someone flees bleeding; a mugging where a friend jumps in.
4. **C4 the player.**
   - **Build:** stance and orders as `PlayerCommand`s; the combat clock; the view's beat replay (swings, hits, a
     stagger, a limp); the HUD (participants, wounds, odds, morale); abstract cover and Take cover.
   - **Proof:** a play session where the player picks a fight they lose, yields, wakes up robbed and limps to a Clinic
     they can't afford.

These come later: ranged (§ 7, with addendum 24) and the blast (§ 8, with netrunning).

## Decisions (Dylan, 2026-11-02)

| # | question | decided |
|---|---|---|
| 1 | What a fight is | **Live engagement**: an entity across ticks, beats inside each tick replayed by the view, joiners, flee and chase; off camera the same beats run to the end in one call |
| 2 | Wounds | **Body regions**: six regions, three severities, bleeding, effects by region; healing over days, the Clinic, scars, lost limbs to chrome; the M16b downed state at the top |
| 3 | Skills | **Melee, shooting, evasion**: the weapon family is a stat on the weapon; `fighting` becomes melee; one `combat::power` for every gate |
| 4 | The player's control | **Stance and orders, the clock slowed**: four stances, orders at the next beat, the whole sim slowed while the player fights, space pauses |

**Defaults the orchestrator chose (overturnable):**
- Most fights end in a break, not a death.
- Sides with joiners.
- Cover stays abstract until the interiors.
- The close range only, inside the slice.
- No ammo in v1 (wear already exists; ammo as a gunsmith good later).
- Six regions.
- `beats_per_tick` 6.
- A lost limb is a ripperdoc's sale.
