# Shadow V20: commute critic

Group: worker, office_worker, courier, cabbie, night_shift, school_parent, day_labourer, teacher, exec, guard (one pick each, 10 diaries; `worker_39756` exists). 100k island, seed 42, d21-27. Method: DAY SUMMARY of all ten, CROSS/jaywalk/[el]/CLIMB/raid/Precinct/Skipped/Wage greps, and full stretches (guard d22, exec d22, teacher d23, worker d24). V19 had no commute group; `docs/shadow_v19/labour.md` is the nearest comparison.

## 1. Verdict
- Against V19 the walk reads **more like a city**: 313 lights-crossings in ten diaries, 82% with a real wait at the kerb (mean 40 s, max 75 s). The day around the walk reads about the same.
- Work still barely reads as work: a Teacher who does ClerkWork, a Guard who patrols 3.5 h of a 9 h shift, a Cab Driver who drives no fare. Mostly V19's bugs, unchanged.
- No one starves, no minor is harmed, every harm line names adults. The El and the cells each show once, thin.

## 2. The exit's checks
- **(a) A street crossed at the lights with a wait: yes, everywhere.** `courier_36217 d21 09:39` "CROSS at the lights at (1279, 642) (junction 1279,643), 61 s at the kerb for the green man"; `guard_16979 d22 09:43` 61 s, `10:44` 7 s, `11:51` "on the green man" (one junction 556,612, three different waits in two hours); `office_worker_24590 d22 18:19` 74 s. Lights / waited: courier 47/34, office 39/34, guard 35/23, day_labourer 32/25, teacher 31/26. A footbridge shows once, `exec_78884 d23 19:12` "DECK on a footbridge at (555, 793), level 1".
- **(b) An El ride a person would take: one ride, and it is a wash.** `worker_39756 d24 06:05` "EL ride to Block#559: on foot to the stop at (1243, 731) in D1 The Sump, the Terrace Line at 06:10, off at the stop at (823, 535) in D4 Mid-Levels at 06:27 (2 legs, paying the fare)"; aboard `06:10`, off `06:27`, floor 11 at `06:31`. Door to door 26 min against "ACTION GoTo(Home) ... 26 min, 678 tiles" for the walk (`06:05`), minus a coin (`06:10` Fare -1c). The other nine have no `[el]` line (the commutes are 2-3 min, rightly walked, or live beside work).
- **(c) A raid climbed in a member's own body: not seen in this group.** No Muster, raid or "went up the stairs" line; `courier_36217 d25 00:01` only hears a rumour "Raided by gang The Wire Hands on corp Nutrix 6". `day_labourer_107293` is a gang member (joined The Salt Hounds Splinter 182 `d21 08:02`, summoned to court at Hideout#11143 `d22 20:01` and `d24 19:36`, missed the guard `d27 00:07`) and never raids. The gangs critic owns this.
- **(d) A cell stay that varies or ends by bail or early release: a long stay seen, no end.** `exec_78884 d23 19:08` and `d27 12:19`: "Zoe Dunmore came to the cells of Precinct#6407 to see Anselm Thorne and told him the post as Bouncer at Club#7384 [d27: Gambling Den#10809] is gone". Same inmate, her Parent, in the cells at least four days; the news changes, nothing releases him and no bail line appears in the ten. `guard_16979 d27` arrests and escorts Rook Wexford (39), Priya Carrow (48), Wren Novak (73), 18-21 min each, and never sees an outcome.
- **(e) Any minor harmed: none.** Every Assault, Arrest, Witness and Murder line names ages 18+ (`day_labourer d21 21:00` Mags Inchbald 42 y attacked Rook Underhill 23 y; the murder `d22 07:45` is Juniper Oakes 70 y). `school_parent_38324` has a 7 y and a 1 y: the 7 y is at School#9052 d21-24 and at home from `d25`; not harm (bug 7).
- **(f) Anyone starving: none.** Header malnutrition is 0.00-0.08 for all ten; "ate a REAL meal | malnutrition 0.00" (`exec d22 23:54`); no death line. Thinnest are teacher and school_parent, both 0c wallets.

## 3. Per archetype (V19 word in brackets where labour had one, else n/a)
| archetype | V20 | excerpt |
| --- | --- | --- |
| worker | alive (alive, thin) | `worker_39756` Mechanic, 35 min commute, the only El ride (`d24 06:05`), a Brothel `d22`, a Gambling Den `d25`, the Drowned Choir `d26`; 15 crossings, 14 waited. |
| office_worker | thin (n/a) | `office_worker_24590` 39 tiles to work; 39 crossings; `d27` sleeps 1.0 h, idle 12.6 h on Sump streets. |
| courier | alive one (thin, alive one) | `courier_36217` Unload x36, 47 crossings, a Strip Club `d23`, a Back Room `d26`; 7 jaywalks. Steady, no story. |
| cabbie | **dead** (dead, dead) | `cabbie_94847 d22 00:00` to `d27 00:00` "stood by all shift for Garage#13744 and drove no fare: no fare, no pay", 6 of 6; Rests 91 min at the garage each shift. |
| night_shift | thin (alive, thin) | `night_shift_23176` 67 y Night Stocker, Unload x66, ClerkWork x72, events: Visited x1, Witness x1. |
| school_parent | thin (n/a) | `school_parent_38324` HangOut x49, ClerkWork x6, "work" 9 h a day, `d27` 12.4 h leisure with both children; `[family]` line daily. |
| day_labourer | alive (alive one, thin) | `day_labourer_107293` gang join `d21 08:02`, beaten `21:00`, patched for 6c `22:23`, skips the shift `d22 08:00` "sets off for it, then does not go in", bins on the scav round. |
| teacher | thin (n/a) | `teacher_25077` 63 y; 0 Teach, 42 ClerkWork at School#13314; wanders the Sump 02:33-06:15 `d23`, BuyStims and UseStim `02:51`. |
| exec | thin (n/a) | `exec_78884` "Job: none", 8.0-8.3 h "meeting" a weekday as one unbroken 476-499 m action (Camera table), then 12 h of Wander in the Sump `d22 18:02-23:22`. |
| guard | thin (n/a) | `guard_16979 d22 09:04` Patrol legs of 21 min between a Vat Farm, a Market, Block#730 and the Sump; `d27 10:59` three arrests with escorts. |

## 4. Bugs, ranked by what the player sees first
1. **Cabbies earn nothing: unchanged (V19 1).** 6 of 6 shifts "no fare, no pay" (`cabbie_94847 d22-27 00:00`). Counter "cab rides driven=274, cab rides unmanned=2585" (`target/v20/analyze.txt:1038`), V19 288 / 2364: marginally worse. A hail goes to an unmanned cab 90% of the time.
2. **A guard leaves a 9 h shift at midday and is paid anyway: new.** `guard_16979` Patrol ends `d22 12:24`, `d24 12:52`, `d25 12:28`, `d26 12:46`, `d27 12:48` (Camera: work 1.8 h), then GOAL Earn / GetHigh with "not in shift 0.00->0.30" and 3 h of Scavenge ("+3c" `d22 14:57`); yet `d22 18:00` "+4c (Wage) ... Wage +6 from corp Arasaka 4". The Patrol goal drops its `in shift` input about 3.5 h in (likely a leg budget, not the shift). He also sleeps 1.6 h (`d22`) and 2.1 h (`d26`) on shift days.
3. **Job label versus action: unchanged (V19 6).** Teacher 0 Teach / 42 ClerkWork; courier does ClerkWork and Unload; exec "meeting" as one 8 h action.
4. **A cabbie jaywalks the same gap 25 times in a week: new.** `cabbie_94847 d21 00:16, 07:19, d22 01:37 ...` always (245-262, 817), 13 tiles between Block#4881 floor 1 (his hangout) and his flat in Block#4681; no lights near (4 at the lights vs 25 jaywalks). Near-forbidden but the only route: add a zebra, or let the planner route by another block.
5. **The El ride saves no time: new, minor.** `worker_39756 d24 06:05` 26 min ride against 26 min walk, and a jaywalk at the stop `d24 06:09` "(1243, 732), no crossing" beside the stop at (1243, 731). Likely cause: foot_weight 2.0 compares weighted minutes, so the ride wins on a draw.
6. **An exec never bails kin out: new (V8 cells).** `exec_78884` visits her Parent in the cells `d23` and `d27`, a rich corp and a 22c purse, no bail line.
7. **A 7 y misses school for a 2c fee: new.** `school_parent_38324 d25 09:30` "the 2c fee at School#8681 was beyond the purse at midnight (2c in hand, 2c of it kept for the rent and meals)" while the mother works 9.2 h. No harm by the rule; it reads as neglect through poverty, worth a thought.
8. **Skip and Served stamps: unchanged (V19 5).** `day_labourer d22 08:00` and `guard d21 08:00` "skips the shift ... sets off for it" an hour before the 09:00 start; Served "stood by all shift" always at `00:00`.

## 5. Dull and too much
- Dull: exec's 8 h meeting as one action; office_worker `d27` 12.6 h idle; teacher's 6 x 30 min Wander through the night; night_shift with nothing but Witness.
- Too much: CROSS lines (15-54 per diary) drown the Notable events; 25 jaywalks of one man; "1 min waiting in the lobby (1 called the car)" on every lift trip (V19 7 holds); a `[needs]` row every 2 h.
