# Shadow V20, street critic (elder, homeless, camp_parent, neglect_parent + OPEN-277)

Window: 100k island, seed 42, d21-27, four diaries (`elder_87697`, `homeless_67474`, `camp_parent_44777`, `neglect_parent_95811`), V19's street report beside it (`docs/shadow_v19/street.md`, other ids, so comparison is by archetype). Read: all DAY SUMMARY, Camera tables, every `[event]`/`[family]`/`[order]`, every CROSS/jaywalk/CLIMB/PLAN FAILED line, full days `elder d21`, `camp_parent d26`, `homeless d27`, `neglect_parent d25`. Starvation from `analyze.txt`, `analyze20k.txt`, `events.tsv`, `events20k.tsv`.

## 1. Verdict

Better stage, same people: the street now crosses at the lights and waits (100 CROSS lines in four diaries, 68 with a kerb wait), and nobody here starves or is hurt. Against V19 the lives are about level: the elder and the neglect parent are quieter, the camp parent has the one real turn (retired at 62, moved in with family).
Two things read like a sim, not a city: GetHigh plans that walk 300 tiles to a stim source and fail (26 GetHigh failures in four diaries), and a 64-year-old who wanders the Sump nine hours on 3c.
No minor harmed in these four; OPEN-277 does not reproduce (2-3 deaths a month, section 3).

## 2. The exit's checks (street group)

- **(a) A street crossed at the lights with a wait: found, plenty.** `elder_87697 d21 09:09` "CROSS at the lights at (1190, 748) (junction 1193,749), 18 s at the kerb for the green man"; `camp_parent_44777 d26 15:03` 38 s, `d26 19:24` 10 s; `homeless_67474 d27 06:13` 67 s, `d27 10:13` 65 s. Waits run 1-75 s, most 25-70 s. Not clean: `elder d21 20:05`, `d22 07:31`, `d23 07:01`/`20:15`, `d24 07:01`/`20:20`, `d25 20:10`, `d26 07:01`, `d27 07:01` "CROSS jaywalking the carriageway ... no crossing", nine times at the same spots (1420,618) and (1249,661) on his commute (bug 1); `homeless d23 05:49`/`05:51` two more. A footbridge appears once and only as a DECK spot (`homeless d25 07:16`), never as a crossing.
- **(b) An El ride a person would take: not seen.** Zero `[el]` lines in the four. Their walks are 5-30 min inside D4/D5 or D1/D2 (`homeless d27 08:39` 17 min, 268 tiles).
- **(c) A raid climbed in a member's own body: not seen** (not this group). `homeless_67474` joined The Static Knives (`d26 03:39`) and stood guard at Hideout#6426 floor 8 (`d27 23:57`), no raid.
- **(d) A cell stay that varies / bail / early release: not seen**; nobody here is jailed. The elder reports others into the cells: `elder d21 06:30`, `d24 14:16`, `d26 06:22`, `d26 16:01` (Odessa Bartlett and Emeric Ambrose, twice each in six days: the cells turn people over, which reads right).
- **(e) Any minor harmed: none.** Harm-line ages in these diaries are all adult. Children seen: `neglect_parent` Tristan 15 y and Wisp 0-1 y, `camp_parent` Jocasta 4 y, all fed and at home; no "unfed", "took", or eviction line. See the Lux Garland caveat in section 3.
- **(f) Anyone starving: in the four diaries no** (min hunger 0.44-0.55, 7 meals each). In the city: 2 deaths at 100k, 3 at 20k, section 3.

## 3. OPEN-277, the starvation question

- **The 736 does not reproduce.** `analyze20k.txt` line 3 `deaths_starvation=3`, `starvation_deaths_per_week=0.7`; `analyze.txt` (100k) `deaths_starvation=2`, 0.47 a week. Events: 20k, 3 Death lines (tick 29754 d21.7, 31176 d22.6, 38376 d27.6) and 19 "is starving" lines; 100k, 2 deaths (42846, 42966, both on d30 of 31) and 58 "is starving" lines. Daily starving flags at 100k: 0 on d1-5, then 0-4 a day (2,1,1,2,0,1,4,4,1,1,4,2,1,4,2,1,2,4,2,4,0,1,0,1,2): flat, no spike, no rise. 20k: 0-2 a day from d10.
- **Who:** adult job-seekers (JobSearch "nothing going" repeats for Ulysses Ostrowski, Ursula Jessop, Tess Yates), the 20k victims 25-31 y by their other lines. Death lines carry no id, age, district or home, and 100k names collide (Vale Jessop 59/61/68, Tess Yates 29/35/72), so jailed vs homeless vs child's household cannot be proven from the events. Ulysses ate a cell meal at `events20k` tick 16260 and starved at 29754, so a spell in the cells does not feed or protect after release. SoupLine "starving, no bin nearer" lines (ticks 21900, 23340, 34860) are the same cohort walking 3-37 min to a kitchen.
- **Minors: none confirmed; one name I could not clear.** Of 58 starving names at 100k, 41 resolve to ages with min 14: Lux Garland (starving, tick 7566) shares a name with a 14 y girl (`events.tsv` tick 36332 "Lux Garland (14 y) lost her brother ... at home with her mother") and with Lux Garlands of 20, 25, 28 y. Needs an entity-index lookup, which I did not run. If OPEN-277 is reopened, put the entity id and age on the Starving and Death lines.

## 4. Per archetype (V19)

- **Elder: alive -> thin/alive** (V19 alive). `elder_87697` (Dmitri Park, 64 y, night Vat Tech at Vat Farm#5856, floor 16 flat 2). `d21 04:09` shaken down in his sleep ("-5c (Coerced), purse 3c"), `06:07` goes to report it: "Report: Dmitri Park went to Station#11314 and told the law: Odessa Bartlett, for Shakedown" (`06:30`), `13:29` she learns who did it. Then nine hours of 30-min Wander in the Sump (`d21 06:33`-`15:25`, idle 9.4 h) on a 3c purse, one meal, one Scavenge for 3c. From d22 it is a 9.7 h night shift every day, 538 min "still". The d21 day is the best he gets.
- **Homeless: alive -> alive.** `homeless_67474` (Pixel Singh, 48 y): hired Bartender `d21 13:05`, work 8.8 h a day, capsule hotels, turned away by The Static Knives `d23 01:36` ("the treasury could not feed another mouth"), off sick `d26 00:00`, joins them `d26 03:39`, stands guard `d27 23:57`. Header still "Job: none" with a job (V19 bug 8, unchanged).
- **Camp parent: alive -> alive.** `camp_parent_44777` (Static Wozniak, 62 y, Vat Tech): `d26 00:08` "Retired: ... retired from Vat Tech at Vat Farm#5671 (to family)" and "moved in with family at Block#824, with Jocasta Nakamura", an 878-tile walk at midnight; `d26 13:27`-`18:37` four GetHigh plans fail (PreconditionLost x2, BuildingFull x2) over ~1,100 tiles; `d27` the rest day is 884 min on the street.
- **Neglect parent: alive -> thin/alive.** `neglect_parent_95811` (Hale Inchbald, 35 y, night Loader): buys 3c of food a day and stores it (`d25 12:57`-`13:19`), 15 y Tristan home from school for a 2c fee `d22 09:30` ("beyond the purse at midnight"), back `d23`-`d25`, "no School today" `d26 09:30`, moves with Wendell Park and the children to Block#596 `d27 00:05`. No unfed child, no service visit. "Neglect" reads as thin money, not neglect.

## 5. Bugs, ranked by what the player sees first

1. **A commute with no crossing.** `elder_87697` jaywalks the same two carriageways nine times (cites in 2a), home (1235,639) to work (1462,642). V7 says near-forbidden; here it is a daily habit for a 64 y with lawfulness 0.36. Likely a missing zebra on the route, or the route cost ignores the green-man wait. New.
2. **GetHigh walks that fail.** `camp_parent d26 13:27`-`18:37`; `homeless d22 02:20` StockGone, `d26 04:01` PreconditionLost; `neglect d22 16:37`, `16:47`, `19:41`. 26 GetHigh failures in four diaries, each after a 20-50 min walk. Likely BuyStims picks a source by distance without checking stock or capacity on arrival. Not in V19's list.
3. **A 62 y retires "to family" onto the street.** `camp_parent d26 00:08`; CLAUDE.md says old workers keep posts to ~74. After it: wallet 6c, no income, 884 min on the street d27. Check the retire trigger and what the move does to the post.
4. **Kerb waits that read long.** 65 s and 67 s on a rest day (`homeless d27 06:13`, `10:13`), 75 s the max; roughly a fifth of waits are 50 s or more.
5. **Labels.** `neglect d25 11:35` "PLAN Eat on Block#7950 [EatAtHome]" while at Block#731 (home); homeless header "Job: none" with a Bartender post.

**V19 bugs in this group:** 1 company housing never lived in: not seen. 2 "both parents in the cells" with a dead parent: not seen (no jail here). 4 insomnia Super: better (elder sleeps 5-9 h) but the d21 daytime wander is the same pattern. 6 lifts all "1 min": unchanged (76 of 76 climb lines). 8 labels: unchanged ("Job: none"). The rest are other groups.

## 6. Dull spots and too-much

- **Dull:** the elder's nine-hour Wander d21; the 538-min unchanged work stretch every day (`elder`); `neglect d21` idle 12.8 h; `camp_parent d27` street 884 min.
- **Too much:** GetHigh retry loops (bug 2); `Witness` lines (dozens a day in the elder's diary); the identical 1-min lift line.
- **Not too much:** the shakedown, report and cells-reply chain (`elder d21 04:09` to `d21 13:29`), the kerb-wait lines, "the treasury could not feed another mouth".
