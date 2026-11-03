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

**C1 as built (2026-11-02; deviations, `OPEN-179`).**
- `resolve_now` (`systems/combat.rs`) is `fight_inner`'s body; the outcome sits beside the callers' triple on
  `World::last_fight` (C3 hands it to the on-end hooks). Two sides only; a crew's numbers are the caller's `a_bonus`
  (attack and morale). Fled is a loss for the caller (an arrest whose suspect "fled" still cuffs him until C3 moves
  bodies). Robots keep `law::resolve_robot_fight` (one roll, `power` for both sides, `fight_death_p`), no wounds.
- The purpose is `FightMods.why` (`Brawl`, `Raid`, `Door`) beside `arrest`, `kill_p` and `kill_mult`: an arrest
  subdues, a Hunt (`kill_p`), a Crush or a berserker (`kill_mult > 1`) and a raid go all out, a bouncer and a guard set
  upon subdue; otherwise courage and lawfulness pick. No mugging purpose: no caller is a mugging yet.
- A lethal purpose's win finishes the beaten body on `kill_p x kill_mult` (the old Hunt semantics); otherwise a body
  dies only of a Grave head or torso hit (`lethality x quality x kill_mult`) or bleeding. The M16b second draw
  (`wounded::wound_instead`) is no longer called by the fight; blood at 0 is the way down.
- Healing is a stub: a wound not bleeding is gone `heal_days` after it was dealt; a Grave bleeds until the M16b Rescue
  stabilises its body (then `treated`). No first aid, no Clinic goal, no lost shifts, no scars (C2).
- Kit armour per region: Skin chrome on every region, a guard's vest (`vest_torso`) on the torso: no vest item yet.
- `[[weapon_kinds]]` carry family, accuracy, damage, lethality, blunt, draw, conceal, tiers and classes; reach, noise
  and price wait for C3 and sight (prices stay the tier's). `Asset.wkind` is mapped at the spawn by tier and the
  holder's class; a guard's armoury draw maps by the law's rows.
- `combat::power` is the gate's read with a body at hand; `combat::skill_power` (skills only: the better of melee
  and shooting, plus a share of evasion) is the city means' `MEAN_FIGHTING` slot and the reads that hold only a
  `Skills` (competence, the gun gate, the think's context).
- `[weapons] weapon_w` and `kill_w` are gone; `[crime] fight_death_p` is the robot fight's only.

**C2 as built (2026-11-03; deviations, `OPEN-179`).**
- **Healing.** `Wound.heal_at` (`since` + `heal_days`); seen to (the patch-up, the back room, the M16b Stabilise), what
  is left x `treated_heal_mult`. At the heal a Wound or worse may leave a scar (`scar_p`), a Grave a maim
  (`maim_p`, `maim_p_treated` seen to: the region reads as a Wound for good) and, a limb never seen to, the limb
  (`rot_p`). Scars, maims and lost limbs are `Wounds.marks`; a body holding only marks keeps the component out of
  `World::hurt`. A maim or lost limb also goes through `[ages]`'s maim (a level, the slot: work output, the beggar's
  odds), and `ages::on_rise`'s random maim is skipped for a body with `Wounds`.
- **The treatment goal is the existing Treat goal's patch-up** (`customers::care_choice`): `customers::hurt` is now an
  untreated wound of `treat_from` or worse (it replaces the lost fight's `Lost` memory within `hurt_hours`; a robot's
  loser, who takes no wounds, no longer walks in). The price is `[customers] patch_price` (`back_price`) x
  `treat_price_mult` by the worst wound, paid through `care::settle` (`Flow::Treatment`, the doctor's cut, the slate,
  the pawn); the goal's hurt term is at least `treat_urgency`. One who cannot settle it lives with the wound and says
  so `untreated_note_hours` later. The off-screen walk-in (`customers::stat_care`) sees to a Statistical body's
  wounds the same way.
- **First aid**: a friend, kin or a gang-mate (an adult on its feet, not the one who did it) in the building or within
  `aid_tiles` stops a bleeding Wound and slows a Grave (`aid_grave_mult`). **No street medic exists**: none added.
- **Lost shifts**: `combat::unfit` in `routine::work_refused` and the Statistical wage gate; `manual_posts` (Role
  names or `[[trades]]` ids; Guard is not one: a guard on the books keeps his post). The line comes at the shift's
  first hour.
- **Lost limbs**: a Grave limb hit from an edged or ranged kind takes the limb on `sever_p` (and the rot above).
  **There is no `[[chrome]]` table**: the chrome limb is the existing `Implant(Arms)` / `Implant(Legs)`, bought by
  `[ages]`'s midnight prosthetics at the nearest Clinic (Ripperdoc) when the coins allow (a T1 and the install fee,
  `Flow::Asset`); `chrome::installed_now` calls `combat::fitted`, which clears the slot's lost or maimed marks.
- **Off screen**: `lod::stat_hit`'s beating writes `stat_hits` hits from `stat_kind`'s damage table, and the
  `stat_kill_p` split's survivor one Grave from `stat_grave_kind`'s (which may sever). Not bleeding (the split
  decided the blood), not seen to. `fviolence` strikes through `stat_hit`, so it writes them too.
- **C1's wrong-reads.** (a) Age: past `age_from`, attack and evasion x (1 - `age_fight` a year), morale -
  `age_morale` a year, a hit's grave weight x (1 + `age_grave` a year). (b) A post's training at midnight
  (`combat::seed_all`): a gang member or a `shooting_posts` role is brought up to its shooting seed; `melee_posts`
  and `evasion_posts` floors. (c) The pit's bout is `combat::resolve_now` with `Purpose::Bout` (both subdue, bare
  hands, no fight memories, edges or weapons' tail; a loser knocked out or with a Wound was beaten; its dice keyed on
  the pair and the tick, so the world stream stays the city's); a Fighter unfit by his wounds is off the card.
  `[leisure] bout_beating` and `bout_hurt_margin` are unread. (d) Guards: the training floors plus `morale_posts`
  (a trained post's nerve) are the feature; see the numbers.
- **A chrome limb is not lost or maimed** (the hit there wrecks the chrome in C3); C2 skips the sever and the maim.
- **The cells dress a held body's wounds** (`combat::hourly`): the M16b downed state lets go of the sentenced, so a
  prisoner at no blood "went down" every hour (C1's loop too: one body 153 times in a run before the fix).
- **The numbers** (20k island, seed 42, 30 days, before -> after, one run each; printed, not tuned): Murders 117 ->
  127, fight deaths 66 -> 62, bled out 20 -> 27, went down 95 -> 49, fights 6493 -> 5175; patch-ups 2917 (a lost
  fight's) -> 2902 (each a wound seen to); shifts lost to wounds 0 -> 1751 (475 workers); limbs lost 0 -> 8, maims
  0 -> 7, chrome limbs fitted 0 -> 0 (1 and 1 in two earlier runs of the phase); first aid 0 -> 18; arrests 6169 ->
  5971, arrests fought off 955 -> 318, backup fights 328 -> 153; a 65+ side loses 53 % -> 66 % and takes a Grave
  4.7 % -> 8.0 % (under 50: 6.3 % -> 5.1 %). Runs vary a lot on one seed (bled out 39, 10 and 27 across the phase's
  builds).
- **Not done:** no street medic; robot fights still deal no wounds; a lost limb leaves the post open to a manual worker
  once the stump heals (`[ages]` maim's output cut, no refusal).

**C3a as built (2026-11-03; deviations, `OPEN-179`).**
- **The engagement is a chain of duels.** `combat::Engagement` holds two sides of members in order; the fronts fight
  C1's duel (`duel_start`, `duel_beats`, `duel_end`: `resolve_now` is the same three in one call), a loser leaves his
  side and the next steps up. One-on-one callers are a side of one each; a gang raid's brawl is its raiders against
  its defenders, the strongest first, so `raid::fight_out`'s pairings and tally are the engagement's (`losses`,
  `deaths`, `beaten`). C3b's joiners append to a side; nothing caps one body against many.
- **Live or at once.** `combat::engage` (`fight` for one-on-one) makes it live when every party is a Full body in no
  other fight; in camera range (the ring holds a party, or the human drives one) a party not Full is promoted first
  (`lod::set_lod_at`). Else it runs to its end in the call. The live ones (`World::combat`, a boxed `combat::Live`) run `beats_per_tick` beats a tick
  in `combat::step`, right after exec (a fight begun by a step runs its first beats that tick). A body in one
  (`Live::engaged`) neither thinks (`think::held`: "in a fight") nor steps (exec skips it); a pending member waits
  his turn there too. A party gone (dead of something else, cuffed, taken in, under 18) leaves his side; one demoted
  from Full has the rest resolved at once; a duellist who moved out of `reach_tiles` of the other (they began
  within it) has fled. The ring log (`log_beats`, `BeatLog`: miss, hit by region and severity, broke) is kept for
  C4; a duel's Fight line is unchanged.
- **One tail per caller** (`combat::Tail`, run by `end` whether live or not): the Attack (`actions::attack_end`) and
  its contract guard (`attack_guard_end`), the abduction (`chrome::abduct_end`), the guard over a corpse, the arrest
  (`law::arrest_end`, the backups as `arrest_duel`'s joiners), the blackmailed culprit's silencing
  (`consequence::silence_end`), the cells (`cell_fight_end`), the crew's reckoning (`crew::reckoning_end`), the
  squatter's stand (`gang::squat_end`), the brawl (`misery::brawl_end`), protection's beating
  (`protection::beating_end`, then `punish_tail`), the patrol's chrome scan (`tags::attack_end`), the collector and
  the stiffed worker (`underwater::dun_end`, `confront_end`), a pairing (`raid::pair_end`) and the gang raid's end
  (`raid::raid_end`, which also credits the head's ladder). A step that needs the answer holds `Running`: the Attack,
  the Abduct, the Strip and Rip (`grudges::guard_contests` is `Option<bool>`); the tail leaves the verdict
  (`Live::done`, read by `combat::take_done`). A berserker in its episode never breaks (morale as a robot's: it
  fights the cuffs to the end), so it is never an escape. The Arrest step needs nothing new: it waits the fight out
  (exec skips the guard) and its end reads the escort as before.
- **Still on the triple at once** (`law::resolve_fight_mods` is `combat::clash_now`, a one-call engagement with no
  tail): the bin's shakedown (the dive needs the answer), the door's scene (it returns its line to
  `service::refused`), a mission's Hit finisher (the contract settles after), the off-screen brawl (both
  Statistical), the robot's sense (`law::resolve_robot_fight`, from `combat::instant`) and the pit's bout
  (`resolve_now`). The other brawls at a door (a corp raid, the Precinct breach, a riot's clash, a mission's brawl, a
  camp breakout, a turf corner) are `run_now` engagements with `Tail::Pairs`: their pairings' tails are hooks, their
  callers' tails read the tally at once.
- **Fleeing is movement.** A loser who fled gets a flee plan (`FleeToHome`: his Home, else `life::refuge`) at the
  tick's `step` (a body fleeing its own step's fight would have the step's end move the new plan on); a body the
  human drives, a Statistical one, the downed and the cuffed do not. **A suspect who flees the cuffs is not cuffed**:
  "X broke from the guard Y and ran (wanted for ...)", `arrest_escapes`, and the guard calls a second
  (`call_backup`: the nearest free guard takes him as his collar and gives chase).
- **Save**: engagements are not saved; a save resolves them first (`combat::settle_all`, at the CLI's `--save-at`,
  the shadow's `--save-at` and the 2D app's F5). `SAVE_VERSION` 131.
- **The numbers** (20k island, seed 42, 30 days, before -> after, one run each; printed, not tuned): fights (the arms
  counter) 5175 -> 6062, Fight lines 14958 -> 17860, fight deaths 62 -> 83, Murders 127 -> 122, arrests 5971 -> 6198,
  arrests fought off 318 -> 532, escapes from arrest 0 -> 257, backup fights 153 -> 241 (cuffed 89 -> 93), flee plans
  0 -> 2082; live engagements 1082, mean 1.62 ticks; ticks/s days 25-30 126 -> 111 (an earlier build of the phase, the
  same engagement code: 126; the box was busy). FINDING: an escape is a second
  arrest later (the backup's collar), so arrests, fights and arrests fought off all rise with the escapes. Headless
  (no camera) every gang raid ran at once (its defenders off camera are not all Full; a posted robot keeps it at once
  too). A live duel's Fight line ends "N beats over M min".
- **The shadow's wrong-reads** (d12-18, guard, raider, gang member, homeless, worker): the flight does not hold (a
  FleeToHome of 1-7 min, then think's next goal: safety is back up); no live gang raid was seen in a diary; off camera
  one body takes a crowd in a minute with no fatigue between duels; no mugging caller exists yet.
- **Not done (C3b):** joiners, the chase, chrome in the fight, robots in a live fight (a robot party keeps the fight at
  once); a body who arrives at a live raid is not in it (`raid::resolve` returns).

**C3b as built (2026-11-03; deviations, `OPEN-179`).**
- **Numbers count, on the chain of duels.** The fronts still duel; every other member of each side strikes the other
  side's front each beat (`Duel.aid`, built at each duel's start, a joiner since added at the next tick): every gun,
  and as many melee bodies as reach one body (`crowd_reach` 3 with the front; the rest wait; geometry, and it helps
  the lone body). The lone body strikes only its front (plus a Sandevistan's extra strikes); a body set upon by
  several splits its defence (`split_evade` off its evasion for each attack after the first in a beat, to 0). Before
  `crowd_reach`, 6 soft rioters beat 3 hard guards (`districts` test): every helper striking every beat was Lanchester
  without the geometry. Morale reads the head count
  (`numbers_morale` x ln(own standing / theirs); C1 had no head-count term, only the caller's `a_bonus`). **Fatigue**
  (`tire_beat` a beat, `tire_wound` a wound, cut by Kit strength) cuts attack and evasion and rests off at
  `rest_hour` out of fights (`Live.tired`, not saved); within one engagement a body also carries its morale, its
  Kerenzikov charges and its Sandevistan's beats from duel to duel (`Carry`). A robot never tires. No clamp caps one
  body against many: a strong, armoured, chromed body still beats a crowd one front at a time.
- **A principal beaten ends the fight** (`ends_on`): a brawl's pairings and a gang raid fight to the last; an arrest
  ends on the suspect (the guard's backups step up as before); every other caller ends when either `first` is beaten,
  so the joiners stand down and the caller's tail reads its own two.
- **Joiners** (`recruit`): at the start of every engagement and each tick of a live one. A body within `join_tiles`
  (or in the building), adult, free, in no other fight, with a reason on one side only: a gang-mate of a member, kin
  (Spouse, Family, Parent, affinity >= 0), a friend at `join_bond`, or a guard on duty (the victim's side, side 1; an
  arrest's side 0; never a gang raid or a turf brawl). It joins on `join_base + join_courage_w x (courage - 0.5)` (+
  `join_guard`, + `join_kin`), one keyed roll a fight; `join_max` a fight; never a bout. "X jumped in on Y's side
  against Z (a friend)". **Deviation:** a live fight takes Full bodies only, but a fight fought at once off camera
  also takes Coarse ones (headless, only 500 of 20,000 are Full: Full-only joiners were all guards). A raider who
  comes to the door of his gang's live raid joins side 0 (`combat::join_raid`, "came late to the door and joined").
- **The flight holds** (`lying_low`): a body that broke and fled lies low `low_hours` (`Live.low`, not saved): only
  Eat, Sleep, Flee, Idle and, with a wound not seen to, Treat score (`utility::think_scored`). This is stronger than
  "no goal back to the fight's tile": nothing takes it out. **The chase**: a Hunt whose target fled keeps the Hunt
  ("X gave chase after Y", `chases`; the hunter's next Hunt goes after him); every other caller lets him go.
- **Chrome** on the `Implant(kind)` tiers (no `[[chrome]]` table): **Nerves** at any tier is a Kerenzikov (a dodge
  charge at the first would-be hit and a second when first hurt, taken on `keren_p[tier]`); Nerves from `sandy_tier`
  is a Sandevistan too (`sandy_actions[tier]` extra strikes a beat for `sandy_beats` beats, once a fight, then
  `sandy_cooldown_hours`, `sandy_sanity` off `Body.sanity`). **Arms** from `mantis_tier` are mantis blades (the
  `mantis` `[[weapon_kinds]]` row at the implant's tier quality, in place of the bare hand or a melee weapon; never
  taken off the body; sheathed under a subdue stance and in a bout); below it gorilla arms (the bare hand's grave
  weight x `gorilla_grave`, lethality at least `gorilla_lethality`, knocks out as a blunt kind). **A hit on a chrome
  limb** (Arms for either arm, Legs for either leg) wounds no flesh: a Graze turned, a Wound wears `chrome_wear`
  condition (its value from its condition, the Kit rebuilt), a Grave (or a Wound on a worn one) wrecks it
  (`assets::wreck`: the sanity shock, the Kit rebuilt).
- **Robots in the beats.** `combat::instant` lost its robot roll; `law::resolve_robot_fight` is gone. A robot fights
  as a Fighter at `robot_fighting[tier]` for all three skills with its tier's kind (`robot_kinds`: a club, a club, a
  pistol; bare hands a robot could no longer kill), all out, never breaks or tires, takes no wounds: a
  Wound counts 1 and a Grave `robot_wreck_hits`; at `robot_wreck_hits` it is wrecked (`End::Wrecked`, Parts at the
  door). A robot counts as Full for a live fight. Its wound on a body is its owner's doing, never a Murder.
- **C2's leftovers.** The pit's referee stops a bout at the first wound of `bout_stop` (the hurt one yields).
  `sever_p` is per `[[weapon_kinds]]` row (pistol 0.08 ... sniper and mantis 0.3; `[combat] sever_p` gone). The Treat
  goal's hurt term grows `treat_urgency_day` a day the oldest untreated wound waits (to 1). The back room sees to
  wounds up to `back_room_max` (a Wound): a Grave goes to a Clinic at `patch_price x 6` (72c), and the broke live
  with it ("lives with the wound: the Clinic's patch-up is 72c and they have 11c": 2 -> 158 lines). **Guards
  bare-handed: not a bug.** A guard's draw is a keyed share of the Precinct's stock over the city guards, and the
  stock's target is `[weapons] stock_per_guard` 0.5, so half the city's guards (and private guards whose office has
  no stock) fight bare-handed by config; a number, left for the calibration pass.
- **The numbers** (20k island, seed 42, 30 days, before -> after, one run each; printed, not tuned): fights (the arms
  counter) 6062 -> 5696, fight deaths 83 -> 85, Murders 122 -> 161, bled out 32 -> 40, Arrest lines 6248 -> 5774,
  escapes from arrest 257 -> 288, backup fights 241 -> 78 (guards on duty now join the arrest as joiners), flee plans
  2082 -> 2074, live engagements 1082 -> 1026 (1.62 -> 1.47 ticks mean); joiners 4405 (1956 the law, 1421 friends,
  956 gang-mates, 72 kin), about 0.8 a fight; the lone body's duels against 2: 867, won 254 (29 %); against 3 or more:
  858, won 140 (16 %); chases 199; chrome limbs wrecked 43; robots in fights 17 (none wrecked); Fight lines with mantis
  blades 6, gorilla arms 166; Kerenzikov dodges 7, Sandevistan uses 0 (the city's chrome is T1 by its installs: one
  run's 30 days installed 359 Legs, 86 Arms, 41 Eyes and one Nerves, all T1); "lives with the wound" 2 -> 184; limbs
  lost 139 -> 157; ticks/s days 25-30 124 -> 139 (a print). FINDING: Kenshi-brutal as asked: a lone body wins under a
  third of its duels against two and one in six against three or more. FINDING: Murders +32 % (a crowd's blows on the
  set-upon side are the starter side's, so a bleed-out from a helper's is a Murder too); left to features, not a knob.
- **The shadow's wrong-reads** (d12-18, raider, gang member, guard, gang leader, gun, fighter; on the build before the
  cell fix): a guard on jail duty "jumped in" on six cell fights in one minute (fixed: the law never joins a fight in
  the cells, and a joiner joins no other fight for `join_cooldown_min`); lying low is Idle's Wander for 6 h at night,
  not a bed; the law is the most common joiner (46 %); a robot's line reads like flesh ("body gravely wounded" before
  "was wrecked"); a Hunt's target who keeps fleeing gives a "gave chase" line each time; no raider came late to a
  live raid (headless, every gang raid ran at once); gorilla arms were invisible in the line until the chrome notes
  ("gorilla arms", "Sandevistan", "a Kerenzikov dodge") were added after the shadow.
- **Not done:** the lone body strikes only its front (a Sandevistan's extra strikes too: never a helper behind it);
  fatigue, the Sandevistan's cooldown and the flight's hold are not saved; the view's beat replay of the helpers'
  blows (the ring log has them) waits for C4.

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
