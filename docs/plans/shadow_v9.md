# Shadow V9: the critic brief (2026-10-26)

The first shadow pass under the reset doctrine (CLAUDE.md § The doctrine): one window, read for what a player
following one character would see. No device rows, no V8 comparison tables, no counters beyond what a diary shows.

**The window:** main `4108184`, 100k island (`--population 100000 --map assets/maps/isle_m_k2_100k.txt`), seed 42,
days 21-27, two picks per archetype. Diaries: `D:/TheCity/target/shadow_v9/isle100k_s42_d21/<arch>_<index>.md`
(plus `.jsonl`), the per-day roll-up `camera.tsv` there. The 30-day counter run of the same seed is
`target/shadow_v9/analyze100k.txt` (events by count, murders, arrests, homeless max).

No candidates at 100k on day 21: fighter, joytoy, office_worker, arms_dealer, madam, black_doc, teacher (whole
trades with no post holders at the game's city size; the orchestrator files it).

**Context:** `docs/HANDOFF.md`, `docs/PRESSURE_PASS.md` (the pass this read feeds: misery acts, the dead trades,
protection, reach), your group's V8 critique `docs/shadow_v8/<group>.md` for what was already known.

**Diaries are 80-340 KB.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, event words, `GoTo`), read the
summaries, then read two or three full days that matter. Cite as `file dDD HH:MM`. A missing line is never
evidence.

**Write `D:/TheCity/docs/shadow_v9/<group>.md`, at most 150 lines:**
1. Verdict in three lines: does the group read as people living lives in a harsh city?
2. Per archetype: alive / thin / dead, one excerpt each.
3. **Bugs**, ranked by what the player would see first: what, where (diary cite), likely cause if obvious.
4. **Dull spots**, ranked: where a life is flat or repetitive and a mechanism (not a knob) would make it a story.
   Name the mechanism in one or two sentences. Dylan wants the sim more expansive and interesting even at the
   expense of balance; this section matters as much as the bugs.
5. Anything already in `PRESSURE_PASS.md` you saw evidence for (cite), so the phase knows its targets.

Read-only: write only your report file. Your final reply is the report's first 30 lines.

## Groups

| group | archetypes |
| --- | --- |
| street | homeless, sick, elder, vendor, street_worker, sweeper, camp_warden |
| gangs | gang_member, gang_leader, purist, cultist, scav, dealer, runner |
| crime | forger, fixer, gun, buyer, pimp (arms_dealer, black_doc absent) |
| corp | ceo, exec, guard, reporter (office_worker absent) |
| labour | worker, worker_friday, night_shift, fabber, loader, builder, foreman, courier, cabbie, day_labourer |
| vice | club_staff, bouncer, dancer, companion, cook (fighter, joytoy, madam absent) |
| care | ripperdoc, orderly, super, sick (teacher absent) |
