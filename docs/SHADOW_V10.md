# Shadow V10: the pressure pass at 100k (2026-10-26)

The pressure pass's read, in V9's window so the two compare one for one: main `33f9c55`, the 100k island, seed 42,
days 21-27, two picks per archetype plus the pass's new picks (`miserable`, `owner_broke`, `jailed`, `raider`):
102 diaries. Seven Sonnet critics (`docs/plans/shadow_v10.md`) wrote `docs/shadow_v10/<group>.md`. Runs in
`target/shadow_v10/`; the pass's 30-day 100k counters (one build before phase 5b) in `target/pp100k/analyze.txt`.

## Verdict

**Better than V9 in every group; lives now have consequences, but most trades are still thin.** Every archetype
has candidates at 100k (V9 lacked seven). Long rides are mostly gone (street: 18 legs over 120 min in 16 diaries,
V9 had up to 18 in one). The new machinery shows on screen as chains nobody scripted:
- a broke back-room doc's hours cut, a loan missed, staff walking out unpaid, jail, eviction, and the creditors
  confronting her for their wages (`black_doc_13839`);
- an exec reported, fleeing, fighting a guard, arrested, fined, defrauded, then buying a guard (`exec_81688`);
- a super's daily round turning into a feud with a tenant: a robbery, a grudge, a lost fight (`super_101327`);
- a missed watch costing a member the colours, a court that fails naming who did not come, a leader walking out
  of her gang and converting (`gang_leader_20414`);
- the door refusing a drunk who becomes the bouncer's enemy; a miserable cabbie skipping a shift.

What still reads wrong is mostly two things: the trades' own verbs (fares, bouts, raids, scavenging, patients) still
rarely happen to a followed body, and the petty-law loop (fare evasion) now fills the gaps the emptiness left.

Day 27 is the city's shiftless rest day (weekday 6); several critics read its zero work as a bug. The diary should
say so (below).

## Next pass, ranked by what the player sees

1. **The fare-evasion loop.** Picks jailed three or four times in seven days for fares, released broke onto a far
   street, re-arrested (`scav_31438`, `raider_85728`, `pimp_96574`; 209 sightings in one diary). Petty offences
   should be a fine, a beating by the El's guard, or a ban, not a cell; jail is for what makes story.
2. **Gang verbs that start and stall.** Raids muster and Sleep wins (`purist_25175 d24`, `dealer_51168 d21`; 0
   brawls in the group); JoinGang walks of 300+ min never arrive (`jailed_93738`: 11 goals, 0 joins); a gang
   try-out ends with no accepted or refused line (`homeless_97788 d23`); Scavenge loops a 1-min step that fails
   `PreconditionLost` (52 in the scav diaries, 0 finds); "unpaid: the gang is broke" every night on duty.
3. **Walking pace and the far commute.** Same-district walks still take 4-6 h at well under a tile a minute
   (`joytoy_103306 d23` 384 min for 149 tiles; `office_worker_14234` 383 min for 176), which points at path cost
   or pace, not placement; the loader's 997-min commute and "took a post nearer home" never firing; a guard's
   escort as one 935-min action (`guard_26314`).
4. **The wounded and the sick.** A wounded guard bleeds three hours with no goal and nobody acts
   (`guard_8327 d26`); diagnosed cancer at a wallet near the 12c fee never seeks care and nothing offers credit, a
   pawn or the black doc (`sick_70955`, `sick_61720`); a sick bouncer works 0 h six days and nobody covers him.
5. **The trades' verbs.** Cabbies drive nobody on screen; the pimp never collects; fighters have no bouts, the
   madam and joytoy no bookings; guns never hunt; one black doc sees no patient; teachers have no pupils; reporters
   write no story; a CEO's "work" is a street-market stall with no Job.
6. **Bugs.** Fifty-one friendships flipping to enemy in one minute with no event, then seven assaults in two hours
   (`gun_28759 d21 14:46`); the treatment fee never reaching the doctor; a gossip line with winner and loser
   swapped (one sample).
7. **Noise.** The super's 14-18 identical notices a day and ~40 zero-valence "called on" memories a round;
   "X saw Y watching" (phase 5a); the rest day unmarked in the diary.

## What the pass did to the city (100k, seed 42 x 30, `pp100k` against V9's run)

Printed, not gated: employed mean 51.6k -> 39.3k (F2's per-Lot venue mix at 100k is the main suspect, OPEN-204's
map; misery and jail firings ~1.9k more), thefts 3.0k -> 22.5k, Murders 205 -> 264, starvation deaths 0 -> 34,
jailed mean 470 -> 890, homeless max 10.2k -> 12.8k, ticks/s 29.6 -> 31. A quarter of 100k adults sit under the
misery line at day 21.
