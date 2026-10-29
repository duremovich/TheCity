# Shadow V15: labour critic

Group: worker, worker_friday, night_shift, jobless, fabber, loader, builder, foreman, courier, cabbie, day_labourer (two each, 22 diaries).
100k island, seed 42, d21-27, main `bf63dee`. The picks are different people from V14's. Method: DAY SUMMARY and `[event]` lines of all 22, greps (CabFare, Fare, Hire, Housed, Report, Murder, turned away, Turf), full stretches of builder_86331 d21-26, courier_50890 d21-23, cabbie_105079 d21, cabbie_67266 d23-24, jobless_6204/6217 d23-27, night_shift_27757 d21-22. Cites are `file dDD HH:MM`. A missing line is not evidence.
Minors: no harm line in the 22 diaries prints an age under 18 (grep empty). Nobody under 18 hurt, in vice or dead. `day_labourer_11346`'s children (17 y, 14 y) sit in Work Camp#4869, fed.

## 1. Verdict

Marginally better than V14: the job-hunt arc now ends in a hire and then a better hire inside one diary, the cab fares pay the right person, and one brawl-to-report-to-cells chain (builder_86331) is a real story. The shift floor is still a metronome (6 of 22 diaries have nothing beyond Witness lines), and round 6's violence and turf work barely touches this group: no CorpseFound, no Murder, no war, no door turn-away. The first thing a player sees is a builder sent 190-220 tiles to a different D1 shop every day.

## 2. Per archetype (V14 word in brackets)

| archetype | V15 | excerpt |
| --- | --- | --- |
| worker | thin (thin) | `worker_39246 d25 08:00` Skipped "sets off for it, then does not go in; heads for Braindance Parlour#5869 (mood -0.17 ... absence 1 in 14 days)" on a 137-tile commute. `worker_55966 d23 16:08` buys a Legs implant T1 "for 60 on finance" (net -41c), naps 241 min on the street; `d27 00:08` takes up the Church. |
| worker_friday | alive one (alive one) | `worker_friday_35309 d25 00:05` Housed "moved the children out of the blocks around Bar#4357 (The Unplugged 2 and The Iron Knives at odds there)"; `d27 13:21` -12c Stims to gang The Unplugged 2. `_59301 d25 00:10` Custody for a neighbour household: baby "went 2 days unfed", friends lend 1c each: covered. |
| night_shift | thin (thin) | `night_shift_27757 d21 12:41` Gamble -4c, then `13:12` Drink -2c (V4's order). `d22` no sleep at all, idle 13.1 h, net -13c. `night_shift_23419`: one Visited line in a week. |
| jobless | alive one, thin (alive) | `jobless_6204 d27 04:14` SoupLine "starving, no bin nearer", `05:37` "ate a Mission's SYNTH meal" (U3 fires). `jobless_6217 d24 07:00` a friend "told Juniper of a Clerk post"; nothing done with it (bug 3). |
| fabber | dead, thin (thin) | `fabber_34558` zero events, travel 6.5 h on 261 tiles each way d22-26. `fabber_87498` two Witness lines. |
| loader | dead both (thin both) | `loader_70521` zero events; `loader_70632` one Witness, `d27 22:52`. |
| builder | alive one, thin (alive one, thin) | `builder_86331 d25 14:00` Brawl at Strip Club#2500 (the site she was sent to), `15:00` she tells a passer-by, `d26 00:47` "Kofi Whitlock learned who put him inside". `builder_13320 d21 09:30` three children home "the 2c fee ... beyond the purse", `d22 09:30` all three at School after a +13c day. |
| foreman | alive, thin (alive) | `foreman_21236 d21-26 06:00` picks 10/13/18/21/20/15, 567-620 sent home; hires land at 10:12, 14:54, 07:20 (not 00:11). `foreman_25916 d22-26 06:00` "took nobody: nobody at the gate he would take" five days running. |
| courier | alive one, dead (alive one, thin) | `courier_50890 d23 03:56, 04:00` two Intimidate-from-him while he sleeps, `04:40` GOAL Fight, `04:55` Assault on Dmitri Kemp, no law. `courier_69030`: zero events, 9.8 h shifts to the decimal. |
| cabbie | alive one, thin (thin) | `cabbie_105079 d21 04:17` Earn picks StealVehicle, `10:32/10:43/11:57` fights off three guards, `12:54` Arrest, `13:46` 6 days, `d27 13:46` "post as Cab Driver gone". `cabbie_67266 d24-26` "stood by all shift ... no fare, no pay" x3 after one fare a shift d21-23; `d26 00:00` RentShort. |
| day_labourer | alive both (alive) | `day_labourer_11346 d22-24 06:00` sent home at the gate three dawns, 8+ board reads, `d25 06:00` hired Day Labourer, `d26 00:11` hired Packer. `_96309 d21` evicted, GangJoin, strips and harvests a body, `d24 01:24` Raid on the Salt Ghosts Hideout, `03:12` patched in a back room for 6c. |

## 3. Round 6's mechanisms seen at 100k

Fired in this group:
- **V1 households**: `builder_13320` fee-beyond-purse then School (`d21 09:30` to `d22 09:30`); the split pantry "5 of it ours (48 households)" (`builder_13320 d26`); one Custody warning per household with friends covering (`worker_friday_59301 d25 00:10`).
- **V2 turf**: `worker_friday_35309 d25 00:05` moves its children off a contested block; `jobless_6217 d24 03:47, 21:23` and `cabbie_105079 d27 21:27` "kept off the blocks ... after dark"; `day_labourer_96309 d24 19:00` "holds The Red Widows' corner by Block#604 (3 more of theirs on it)"; two workers buy stims from a gang on d27 (corner deals 127 to 6,395 per 30 days, r5 to r6).
- **V3**: only "Cells: ... learned who put him inside" (`builder_86331 d26 00:47`) and a Report told to a person on the street (`d25 15:00`).
- **V4**: gamble before drink (`night_shift_27757 d21 12:41`); the cab fare goes to the driver (`cabbie_67266 d21 07:48` "CabFare +6 from Percival Ferreira, CabTake -3"); commuters pay "Fare -1 to the City".
- **V6**: the back room patches after a Raid (`day_labourer_96309 d24 03:12`).

Expected, never seen: V3's robbed victim crying out or telling a guard (`courier_50890` robbed asleep four times, no Report); a body found, a case, a killer named (no killing in the group); a busted Fixer office; a retiree's move to kin; any V5 war, corner fight or truce line; a Clinic spill or door turn-away; a Bout or undercard; a founded School (`schools_founded=0` in r6 analyze).

## 4. Bugs, ranked by what the player sees first

1. **Builder sent 190-220 tiles to a different shop every day.** `builder_86331` (yard#5898 in D3, 4 tiles from home): `d21 09:19` GoTo(TargetHome) Noodle Bar#2632 D1 "151 min, 189 tiles", `d22` 128 min, `d24` Garage#2696 131 min/202 tiles, `d25` Strip Club#2500 134 min/206, `d26` Noodle Bar#2503 151 min/219. Work 11-11.6 h, travel 3-7.3 h, net -5c to +4c on 8c. The sites are always D1, never the yard's own district. Likely cause: the build-site picker is not distance-aware. New to this read.
2. **Sleeping man robbed, nobody told.** `courier_50890 d21 05:10, 05:56; d23 03:56, 04:00` "SETTLED APART: Coerced 3c ... I am asleep at Block#566", 3c from a 5-8c purse each time, "Enemy affinity -1.00". `d23 04:40` Fight, `04:55` Assault on the robber (both adult). No cry-out, Report or guard d21-27. V3's "victim cries out" does not reach a sleeper.
3. **A lead dies on the board.** `jobless_6217 d24 07:00` "Solene Oyelaran told Juniper of a Clerk post at Street Market#4300", `13:22` the board shows a Club Staff post at Club#5005; her plans after are Pitch/Vend then Scavenge to d27. 68 y; the Loan line repeats verbatim `d23 00:00` and `d26 00:00`.
4. **Far post, unchanged.** `fabber_34558` 277 tiles, GoTo(Workplace) 185 min and GoTo(Home) 212 min every shift d22-26, 6.5 h travel for 8.9 h work on 7c; `worker_friday_59301` 210 tiles/135 min. No move-nearer line anywhere in the group.
5. **Cab still walks more than it drives.** `cabbie_67266 d23` six GoTo(Spot) legs of 19/26/45/26/79/28 min (179 tiles at `10:39`) for one fare. City counters r5 to r6: rides driven 3,302 to 3,031, unmanned 12,410 to 5,869, so two in three are still unmanned (better).
6. **A cab driver steals a bike at 04:17 and loses the post to the sentence.** `cabbie_105079 d21 04:17` Earn picks StealVehicle (shift is 16:00-24:00), `d27 13:46` "post as Cab Driver gone", `14:00` JobSearch "nobody knew of an opening". The three identical "resisted arrest and fought off the guard" lines at 10:32, 10:43, 11:57 are the same loop three times, no wound on either side.
7. **Child alone, unchanged.** `builder_86331 d21-25 09:30` Freya (7 y) "at home (play: no School within 1 h)" with a mother away 11 h plus 2.3 h each way (bug 1); `worker_friday_35309` Dorian (2 y) and Thea (1 y) "at home (play, under 6 for School)" d21-27 with a mother at work 5.8 h and travelling 6.7 h. Nobody is named as minding them.
8. **Eviction told twice.** `day_labourer_96309 d21 00:00` Evicted event, then `d26 00:01` "Story: Nutrix 3 Now: Vatra 25 evicted Lena Inchbald" five days later as if news. Low.
9. **No-sleep day.** `night_shift_27757 d22` sleep 0.0 h, idle 13.1 h with energy 0.66 at 19:58; `06:33` one meal at -12c on a 7c wage (others pay 1-2c). Low.
10. **Gang call from far.** `day_labourer_96309 d22 00:07, d23 00:07` "stays off The Red Widows's trade today: the nearest spot ... is 2 h 14 from home". Shorter than V14's 7-13 h.

V14 labour bugs: 1 rehoused far: not seen (no move-in in the group); 2 no law after a kill: unchanged (no kill; sleeper robbery above); 3 CabFare to a stranger or corp: fixed; 4 cab walk: better; 5 child alone: unchanged; 6 gang call from far: better; 7 duplicate BountyPaid: fixed (one at `night_shift_27757 d21 10:11`); 8 midnight hire stamp: better for foremen, still `day_labourer_11346 d26 00:11`; 9, 10, 11 (own-employer try-out, clinic hire picks, coercion with no scene): not seen.

## 5. Dull spots, ranked (mechanism, not a knob)

1. **The shift is still one tile.** `loader_70521/70632`, `fabber_34558`, `courier_69030`, `night_shift_23419`: no event in a week, hours to the decimal. Mechanism: a per-shift incident with a memory (a short count, a wrong crate, an inspector, a mate who did not show) and a shift-end line saying what.
2. **A hired father leaves his children in the camp.** `day_labourer_11346 d25-27` is hired and paid while Axel (17) and Volt (14) sit in Work Camp#4869. Mechanism: a parent with a Home and a wage fetches them on the first full week's pay.
3. **The told lead.** `jobless_6217` above. Mechanism: a told post becomes a plan that goes to the door, and the diary says whether it was taken, turned away or filled.
4. **A foreman with no one at the gate.** `foreman_25916 d22-26` while 600 queue two districts away at yard#5328. Mechanism: the unpicked are sent on to the next yard on the board.
5. **A cab with no fares.** `cabbie_67266 d24-26`. Mechanism: a night's fare as an errand (the hurt to the Clinic, a wanted man away from a raid) so a shift has one real ride.
6. **A marriage with no scene.** `cabbie_67266 d23 21:40` "Viktor Ironside married Volt Singh"; the home changes by `d24 03:36`, `d26 00:00` RentShort. Mechanism: a ceremony line and what it cost.
7. **Rest day.** `courier_50890 d27` idle 17.2 h, `foreman_21236 d27` 18.5 h, `worker_friday_35309 d27` 17.6 h. Mechanism: church, the Den, kin.

## 6. Too much

- **Witness spam.** 73 Witness lines out of about 150 events in the group (Theft 21, Dealing 19, Fare Evasion 18): `jobless_6217 d23` eight "saw X commit Theft", `worker_friday_35309 d27 13:11-17:31` seven Dealing, `night_shift_27757 d21-22` six Dealing at 00:50-01:45 and 12:48-13:03. A corner-deal ticker, not a story; one line per scene ("saw three deals on the corner") would read better.
- **600 sent home at one yard.** `foreman_21236 d21 06:00` 620, then 608, 567, 575, 570, 593. A ticker, unchanged from V14 (counter 9,571 to 10,100).
- **Eviction rumours.** Every diary hears "Evicted by corp X" on most days (`fabber_34558 d21 00:01, d24, d27`): city-level noise in a labour diary.
- Nothing else reads broken. The corner and the war are not visible to this group.
