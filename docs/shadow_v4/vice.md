# Shadow V4: venues and vice

Archetypes: club_staff, bouncer, dancer, madam, companion, joytoy, fighter, cook. 46 diaries, 7 days each, from three
runs:
- `k2_s43_d26_full` (shortened to `s43f`)
- `k2_s44_d90_full` (`s44f`)
- `k20_s42_d20` (`k20`)

The 2k runs are the full-LOD regeneration. The first 2k runs overflowed `[lod] max_full`, so many diaries had
Coarse rows. Claims taken from them are flagged **(first run)** and kept only where the diary was Full at that moment
and the code confirms the mechanism. Every visit, tip and bout is a seeded game abstraction, read here as lives at
systems level. Cites are `run/file dD HH:MM`.

## The picks and their weeks

### Club staff (Concierge, Croupier, Attendant, Club Staff)
- **Mei Quarry**, 48, Street, Concierge at the Spire Lounge, home Spire (`s43f/club_staff_2041`). 34 h worked, 53 h
  idle, of which about 20 h is 30-minute Wanders. No events.
- **Mirabel Varley**, 40, Dreg, Croupier, homeless (`s43f/club_staff_2283`). Housed on d29 00:00, with about 21 h of
  Wander.
- **Tess Achebe**, 34, Corp, Braindance Attendant (`s44f/club_staff_611`). Works a 54 h day shift and makes one deck
  sale (30c, d90 17:02). Robbed by Kofi Volkov (d95 04:47), she reports it, and he leans on her until the report is
  withdrawn (d96 00:00, `Coerced`). This is the liveliest week in the group.
- **Echo Kemp**, 25, Club Staff (`s44f/club_staff_967`). 72 h idle, no events.
- **Philippa Wozniak**, 47, Croupier (`k20/club_staff_12201`). **Rhys Blackwood**, 40, Attendant and Choir member
  (`k20/club_staff_12732`, 84 h in jail for Grand Theft).

*Verdict:* **thin**. The job is one 6 h ClerkWork, and the life is whatever crime comes to the door.

### Bouncer
- **Rhys Brandt**, 37, Club#527 in Vats, home Sump East (`s43f/bouncer_682`). 43 h of travel, 10 h in jail for fare
  evasion (d30 16:41).
- **Elspeth Zhou**, 50, Strip Club (`s43f/bouncer_691`). 42 h idle, no events.
- **Kofi Jessop**, 55, Fight Pit (`s44f/bouncer_1260`). No events.
- **Brick Yates**, 29, Strip Club (`s44f/bouncer_1660`). Strips two corpses (d90 07:00 and 09:22) and steals a
  motorcycle (d91 04:09).
- **Rosalind Brandt**, 41 (`k20/bouncer_16445`, 71 h idle). **Benedikt Dunmore**, 64 (`k20/bouncer_20252`).

*Verdict:* **not alive as a bouncer**. None of the six turns anyone away, stops a fight or meets a customer. The door
rule exists only at a Club (`leisure.rs:265`) and leaves no event or memory when it fires.

### Dancer
- **Elowen Volkov**, 38, Club#527 (`s43f/dancer_1021`). Attacks Slate Yardley, an enemy, and loses (d28 13:16).
  45 h of travel.
- **Odessa Upton**, 33, Club#482 (`s43f/dancer_1239`). Attacks Aldo Okafor (d28 02:24) and draws 2 grudges.
- **Kofi Wexford**, 20, Strip Club (`s44f/dancer_1526`). 46 h idle, no events.
- **Wren Ambrose**, 20 (`s44f/dancer_2346`). Steals a motorcycle (d91 15:30) and gets 6 days (103 h inside).
- **Thea Fairweather**, 37 (`k20/dancer_17899`). Fare evasion, then a stolen motorcycle and 6 days; the owner beats
  her. **Gideon Duarte**, 22 (`k20/dancer_25716`). No events.

*Verdict:* **thin**. Across the six dancers, not one tip in the week, although the trade row is `tipped`.

### Madam
- **Piers Quarry**, 27, Brothel#484 (`s43f/madam_2008`). Earns 16c a day with 57 h idle and no events. He buys visits
  at his own brothel (d30 00:02, d32 00:02).
- **Garrick Singh**, 58 (`s44f/madam_602`). Visits his own brothel after two shifts (d93 00:02, d94 00:02), steals a
  motorcycle (d94 02:22) and is beaten by its owner (d94 20:46). On days he idles far from home he Wanders the Civic
  street for about 5 h before his shift (d90 11:48 onward).
- **Hex Reyes**, 24 (`k20/madam_12080`). 94 h idle. He buys a visit at his own brothel at 08:51, then Rests there for
  7.5 h until the 18:00 shift (d22 10:24-18:02).
- **Joss Jessop**, 66 (`k20/madam_17666`). Three deck runs bring 180c against 74c of wages, plus a deck upgrade.

The 2k city has one Brothel, so one madam candidate per run. *Verdict:* **not alive as a madam**. No worker is ever
hired, named, paid or met, and the madam takes no cut.

### Companion
- **Yorick Pembroke**, 30, and **Zed Varley**, 46, at the Spire Lounge, both living in Block#20 in the Spire
  (`s43f/companion_1374`, `_1686`). Idle is 84 h and 94 h, against 99 h and 97 h in the first run.
  - A typical day is about 10 h of 90-minute Rests at home (d29 07:26-16:38).
  - Yorick takes 56c in tips, Zed 0c, on the same shifts at the same lounge.
  - Yorick spends it at his own lounge (d27 11:42, d30 00:02, d32 00:02).
  - Neighbours rob both at home (d26 07:14 and 07:33), and both beat Perrin Volkov 17 minutes later.
- **Fenn Ferreira**, 26 (`s44f/companion_708`). 72c in tips and 120c on two therapy sessions. 82 h idle.
- **Elowen Okafor**, 30, at the same lounge (`s44f/companion_1243`). Her only tip of the week, 4c, comes at d94 18:43,
  while Fenn is still walking in (he leaves home at 20:42).
- **Tobin Garland**, 42, and **Undine Mercer**, 52, at the k20 Lounges in Mid 8 and Mid 3. Both get 0c in tips and
  have no events. Their weeks are 27-30 h idle and 32-35 h of leisure.

*Verdict:* **thin**. At 2k it is nearly **not alive**, with 74-94 idle hours at Full LOD. Tippers now have names, but
they leave no memory or edge.

### Joytoy (chip contract Host)
- **Aldo Nakamura**, 18, sanity 0.57 (`s43f/joytoy_1154`). Buys a deck on finance and is fried by ICE on his first
  run (d31 03:03). **Imani Vasquez**, 38 (`s43f/joytoy_1819`). 59 h idle, no events.
- **Kuro Kimura**, 27, sanity 0.52 (`s44f/joytoy_1356`). Buys a deck and jacks in once (d95). 54 h idle.
- **Joss Hawthorne**, 70 (`s44f/joytoy_1764`). Buys a deck, sells data for 30c, takes therapy.
- **Jocasta Crane**, 20, married d21 (`k20/joytoy_19629`). **Nova Tanner**, 26 (`k20/joytoy_24411`). No events.

*Verdict:* **thin**. The first run's best arc, Kuro's berserk episode ending his contract, was a Coarse artefact:
that diary had 66 Coarse rows, and the episode does not happen at Full. In the week, the chip shows only as sanity
0.52-0.60 in the header.

### Fighter
- **Piers Bartlett**, 67 (`s43f/fighter_1614`). Sells a sighting for 15c, beats a neighbour (d29 07:24) and gets 10
  days (57 h inside).
- **Sabine Wexford**, 61 (`s43f/fighter_953`). Three neighbours rob her in 14 minutes in her own block, and she
  attacks two of them (d27 20:48-21:10). About 23 h of Wander. No bout all week.
- **Viktor Vance**, 55 (`s44f/fighter_1676`). Joins the Drowned Choir (d90 08:55), marries (d91 01:47), and the gang
  takes 3 squats. 54 h of travel.
- **Delia Hawthorne**, 42, Pit#544 (`s44f/fighter_2279`). On shift d91-d92 while Hana Mbeki and Tess Crane fight
  there. She never fights.
- **Philippa Jessop**, 37, against **Undine Adeyemi**, 67, at k20 Pit#4868 (`k20/fighter_8399`, `_11921`). The same
  pair all six nights, Philippa winning 5. Undine also runs decks for 48c.

*Verdict:* **thin**. At 2k: zero bouts in the s43f week, two on s44f, none for any pick. The 20k bouts are a fixed
pair. **Bets paid 0** in all 8 bouts seen.

### Cook
- **Wilhelmina Dunmore**, 60 (`s43f/cook_1825`). Works 54 h, no events.
- **Smoke Bartlett**, 72 (`s43f/cook_1944`). Steals a motorcycle at 07:23 on d26 and spends the next 144 h inside.
- **Varek Ostrowski**, 23 (`s44f/cook_2574`). Robbed twice.
- **Nadia Osgood**, 55 (`s44f/cook_675`). The most alive life in the set:
  - She beats a woman (d90 14:30) and coerces the witness into withdrawing the report (d91 00:00).
  - She serves a night for fare evasion and misses rent (d94).
  - She takes up the Open Circuit (d96 00:00), then joins the Drowned Choir the same day (21:37).
- **Hale Bartlett**, 65 (`k20/cook_14156`). Plays street dice.
- **Priya Coldwater**, 43 (`k20/cook_23651`). Has 0c, steals food, misses rent, joins a gang and leaves the next day,
  strips a corpse for 1c.

*Verdict:* **alive as work, thin as a life**, with Nadia as the exception. No meal is ever cooked or served, and a
noodle cook still steals food (Priya).

## 2k vs 20k, day 26 vs day 90

- **Idle is not a Coarse artefact.** At Full, the 2k companions still run 74-94 idle hours and Hex at k20 runs 94.
  The other 20k picks run 10-35. 2k picks who idle far from home Wander in 30-minute pieces, 20-23 h a week (Mei,
  Mirabel, Sabine, Garrick).
- **Tips.** At 2k the tips are real and go to one worker per venue (below). The k20 Lounges paid 0c all week. These
  Lounges are named "Spire Lounge" while standing in Mid districts.
- **Bouts.** 2k pits stage 0-2 bouts a week. The k20 pit stages one every night between the same two fighters.
- **Travel.** The cross-district hires still pay at 2k: Viktor 54 h, Elowen 45 h, Rhys 43 h, Delia 41 h. At 20k most
  travel goes to spots, markets and a hideout.
- **Day 26 vs day 90.** Jail is similar: 3 of 15 picks on s43f (Rhys, Piers, Smoke) against 2 of 15 on s44f (Wren,
  Nadia). The first run's "d90 jails more" was a Coarse artefact. The mature city adds the Choir's squats, coerced
  witnesses (Tess, Nadia) and a Tax inflow.
- **Crime in the home.** Robbery in one's own block is the most common interaction: 11 of the 30 2k picks are
  Intimidated in the week. Motorcycle theft is the common crime of the old and the Corp class (Smoke 72, Garrick 58,
  Brick, Wren, Thea). Harshness is by design and is not flagged.

## V3 gaps that touch these archetypes

| V3 gap | Status |
|---|---|
| 1 Idle hours after the shift | **Unchanged at 2k, and worse for the companions** (74-94 h at Full, against 62). Better at 20k, except Hex. |
| 2 The work is a counter | **Unchanged.** Every venue shift is one 358-minute ClerkWork. |
| 3 Distance | **Unchanged at 2k** (41-54 h of travel). Mild at 20k. |
| 4 Ages vs the trades | **Unchanged.** Fighters 61/67/67, a 70-year-old joytoy, a 72-year-old cook, a 66-year-old madam. Age plays no part in bout odds. |
| 8 Cost of the chip invisible | **Unchanged at Full.** Sanity shows in the header; no episode, debt or creditor in any week. |
| Madam has nothing to manage | **Unchanged.** |
| Clients are coins | **Partly fixed.** Tips name the tipper, with no memory or edge. |
| Dancer ladder never moves | **Unchanged.** |

## Interactions

- **What works.**
  - Witness coercion reads like people: the robber makes Tess's report go away, and Nadia does the same to her own
    witness.
  - Gossip carries robberies and beatings with hop counts.
  - Faith and the Choir pull in the cook and the fighter.
- **What does not.**
  - The block robbery is the main contact neighbours have. The victim who hits back draws the robber's friends'
    grudges (Delia Bellamy against Yorick and Zed, d27 00:00).
  - Bout opponents (Philippa and Undine, six nights running) form no rivalry beyond the Bout memory.
  - Tippers never become regulars (Delia Kettering tips Yorick three times on d26 21:00 and is never met again).

## Gaps, ranked by how much they distort a life

1. **[feature] The venue shift is a counter.** For every trade here the shift is a single 358-minute ClerkWork: the
   bouncer never works a door, the madam has no staff or cut, the companion and dancer never meet a client, and the
   cook cooks nothing. Concrete fix: a nightly roll of service encounters bound to real patrons in the building.
   - A tipper writes a `Served` memory and an edge on both sides, and repeat visits make regulars.
   - The bouncer files a `TurnedAway` event and memory on a door refusal, and breaks up an Assault inside the venue
     during the shift.
   - The madam hires, docks and fires brothel workers and takes a nightly cut.
   - The cook turns stock into meals, with one staff meal per shift.
2. **[feature] The idle day (V3 gap 1). Holds at Full.** One 91-minute Enjoy fills `fun` for about a day. Socialise
   needs a partner in the room. The rest of the day goes to Rest at home (about 10 h blocks) or 30-minute Wanders when
   far from home (20-23 h a week). Concrete fix: daytime satisfiers by trade and class:
   - training for fighters and dancers (skill xp, energy cost);
   - dress upkeep for companions and madams;
   - visits to a Family edge's home;
   - more weight on the side jobs already present (Hack, Deal) for the idle.
3. **[bug] Every tip goes to the lowest-id worker on duty.** Yorick#1374 gets 56c and Zed#1686 0c on the same shifts.
   Elowen#1243's only tip of the week comes while Fenn#708 is late (s44f d94 18:43). Cause:
   `systems/vice.rs::tipped_on_duty` uses `.find`, which returns the lowest id. Fix: pick among the on-duty tipped
   staff by `hash(world, PURPOSE_PAYER, key(visitor), tick)`.
4. **[bug] Dancers at Clubs are never tipped.** 0c across all six dancers, five of whom work Clubs. Cause:
   `systems/vice.rs::on_visit` handles only StripClub, Brothel and staffed Lounge, so a Club returns at `_ => return`.
   Fix: a tip-only `BuildingKind::Club if worker.is_some()` arm.
5. **[bug] Bouts pair the same two fittest fighters, and nothing rides on them.** Philippa vs Undine six nights
   running. Delia is benched while Hana and Tess fight (s44f d91-d92). 2k pits stage 0-2 bouts a week. Bets paid 0 in
   8 of 8 bouts. Cause: `systems/leisure.rs::bouts` (sorted by `fighting`, `fighters[0..2]`). Fix: pair the present
   fighters longest since their last bout, pay the winner a purse, and give the loser a wound roll that scales with
   age.
6. **[bug] Staff spend their leisure at their own workplace, then wait there up to 10 h.** Evidence:
   - Yorick at his own Lounge (s43f d27 11:42, d30 00:02).
   - Piers Quarry and Garrick at their own Brothel (s43f d30 00:02; s44f d93 00:02).
   - Sabine at her own pit (s43f d27 00:03).
   - Hex visits his own brothel at 08:51 and waits there until 18:02 (k20 d22).

   Cause: `systems/leisure.rs::paid_candidates` does not exclude `Job.employer`, and `exec/routine.rs::shift_pending`
   uses `WAIT_AT_WORK_MAX = 600`. Fix: skip the employer, and cut the wait to about 120 ticks.
7. **[bug] A robbery victim who hits back is filed as a plain assailant.** Yorick and Zed are robbed at 07:14 and
   07:33 and beat Perrin at 07:50, and Delia Bellamy then swears against both for "a beating". The same pattern shows
   for Sabine (d27 20:48-21:10). Cause: `systems/grudges.rs::cause_of` maps the answering beating to
   `Deed::Assaulted` for third parties. Fix: when the attacker holds a fresh Robbed grudge on the target, file it as
   payback (no third-party grudge), and let the gossip carry both deeds.
8. **[feature] Ages vs trades (V3 gap 4).** Fighters 61-67, a 70-year-old Host, a 72-year-old cook stealing
   motorcycles. Fix: `max_age` per trade row, and an age term in `bouts`' `p_win`.
9. **[feature] The chip's cost (V3 gap 8).** At Full, no Host shows an episode, debt or creditor in the week. Fix:
   the contract's balance as a daily Finance-like flow, and a signing memory at hire.
10. **[bug] (first run) Off-screen killings are laid on Full-LOD agents whose own day contradicts it.** Kofi's first
    diary had 0 Coarse rows. He spent d92 in Civic and the Spire, yet a Vats killing was laid to him
    (`k2_s44_d90/dancer_1526 d93 00:00`), with Fought/Won memories and 12 grudges. It does not recur in s44f. Cause:
    `systems/bind.rs::candidates_in` only weights by `STATISTICAL_ALL_DAY`. Fix: skip non-Statistical candidates whose
    trace is outside the hole's district.
11. **[bug] (first run) Socialise chats with Enemies.** Yorick, at Full, chatted with his robber Rafe twice around
    beating him (`k2_s43_d26/companion_1374 d26 07:34, 09:11`). Not seen in s43f. Cause: `systems/plan.rs:86` and
    `utility/goals.rs:118` call `best_colocated_partner(world, id, -1.0, false)`. Fix: `min_affinity` 0.0.
12. **[bug] (first run) Courtship ignores the week's refusal and fresh grief.** Ines proposed twice in 30 minutes to a
    two-day widow (`k2_s43_d26/dancer_1021 d31 18:12, 18:42`; mixed LOD). Not seen in s43f. Cause: the co-located
    branch of `systems/social.rs::court_target` skips `rejected_recently_by`. Fix: filter by it, and refuse while the
    target holds a Grief memory under about 30 days old.

**Dropped as Coarse artefacts:**
- Loot beating Idle for anyone (10/46 picks strip a corpse in the first run, 3/46 at Full, and those are low-lawfulness
  or broke).
- A one-day gang member as acting leader (Sabine's Choir arc does not happen at Full).
- Kuro's chip episode as evidence that the chip's cost shows.
