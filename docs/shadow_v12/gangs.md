# Shadow V12: the gangs (critic report)

Group: gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider (two picks each, 16 diaries). 100k island, seed 42, d21-d27 (d27 a rest day). Cites are `<arch>_<id> dDD HH:MM` under `target/shadow_v12/isle100k_s42_d21/`. Counts are greps over the 16 diaries. The ids differ from V11, so comparisons are by archetype, not by life.

## 1. Verdict

- **Better than V11: for the first time a raid has a headline.** `gang_leader_19331` musters 242 min, marches 177 min, brawls, and the log says "Raid on Zetatech 6's Garage#5483: The Black Dogs won (7 raiders, 0 defenders); the till smashed, 500c ... next: Squat" (`d24 01:02`). A cultist leader preaches to hearers (`cultist_26195 d21 19:00`), a hit contract has a taker and sold sightings, a succession turns a walking leader back. Fare Evasion jail loops stay gone.
- **Still a day job with a badge for 10 of 16.** `gang_member_53886`, `_6764`, `purist_57388`, `dealer_17998`, `raider_15822`, `cultist_66947` and both runners live their shift; the gang shows up as a nightly 00:07 "stays off the trade" / "sat out the raid" / "chose the shift" line. S1's distance rule works, but it turns the gang into a daily refusal.
- **No minor harmed, hit, raided or in vice in any of the 16 diaries.** Children appear only as `[family]` lines (0-7 y, "at home (play)"). Under-18 top bugs: zero here.

## 2. Per archetype (V11 word in brackets)

- **gang_member: thin (thin).** `_53886` (Iron Knives, 23): `d22 03:23` "sat out The Iron Knives's raid: the muster at Hideout#4493 is 6 h 01 from home", same line `d23 00:07`; nothing else gang-shaped in 7 days. `_6764` (Grey Teeth, 33): `d21 00:07` "stays off ... trade today: the nearest spot, the corner in D1 The Sump, is 11 h 10 from home", repeated `d26`, `d27`; a Birth `d22 00:10`.
- **gang_leader: alive, one (thin).** `_19331` (Black Dogs, 6, homeless, 66 y): `d21 23:43` welcomes a member back from 3 days served, `d22 00:06` corps post a locate (33) and a tag (249) on her, `d22 18:00` "took the tag (LIVE)", `d24 01:02` raid won, `d24 02:03`/`04:26` steals a Flyer and a Car, `d24 15:18` 6 days for Grand Theft, `d26 23:50` Marriage in the cells. `_71953` (Hollow, 11, a Vat Tech): leans on three Fight Pits for 40/week, all refuse (`d21 00:09`); `d24 06:43` HuntStarted on Ulric Varley, nothing follows; misses his own court (`d25 00:07`).
- **purist: thin (alive, one).** `_42201` (Unplugged 4): `d22 00:07` "Ronin Park is out of The Unplugged 4 (talked to the law: thrown out, beaten out by Freya Sallow)"; shakes Block#474 for 3c on `d21 04:24`, `d24 07:11`, `d25 02:20`, `d26 03:20` (a GrudgeFormed only on the fourth, `d26 03:58`); `d26 21:24` drunk, turned away at Gambling Den#4981, swings at the bouncer. `_57388`: `d22 00:05` GangLeave "walked away", `d24 18:47` steals a Motorcycle, `d25 03:36` arrested, 6 days.
- **cultist: alive, one (thin).** `_26195` (Drowned Choir 7, leader, homeless): `d21 19:00` "Cipher Whitlock heard Kestrel Brandt preach ... put 1c on the plate", `22:00` a second hearer, `d22 03:17` Squatted a derelict block, `d23 00:41` beaten on the El for Fare Evasion, `d26 16:36` "banned from the El for 14 days", `d27 22:14` arrested. `_66947` (70 y Fab Tech): `d24 20:30` hears a sermon; `d22 22:00` loses a brawl to Ronin Park (the same man the purist beat out), a GrudgeFormed; otherwise a job.
- **scav: thin (thin).** `_47612` (leader of 2): five "GoTo(Clinic): BuildingFull" on `d21` then patched at a Back Room `18:41`; "picked over the bins ... nothing worth selling" x4 (`d22 02:43`-`d27 02:06`); `d23 20:26` SquatEvicted. `_48740`: skips the Vat Tech shift (`d21 08:00` "walks the streets instead"), `11:35` assaults Liesl Underhill who robbed him, `17:07` arrested, `20:06` "10 days for Assault on Liesl Underhill" (the charge names its victim), then six jail days.
- **dealer: thin (thin).** `_17998` (Unplugged, 60): `d21 00:00` a 178-min Deal at Club#4849 for +1c; `d27 21:55` "Punished: kicked over the stall at Street Market#4202 for The Unplugged: took 80c (Dante Brandt kept 20c); Zane Crane owes the week", with GrudgeFormed and Report: the best collection story in the group. `_41540` (a Jailer, Iron Teeth): `d21 00:39` arrested for Dealing, `d26 01:08` "came out to a changed life: the post as Jailer at Precinct#4485 gone", `d26 19:11` strikes the Red Brothers' hold, `22:21` high, puts a bouncer down, `d27 03:17` 10 more days, `17:00` blackmailed in his cell.
- **runner: dead (alive/thin).** Neither pick is a runner in play: `_46482` and `_90375` are Guards with one JackedIn each (`d21 20:50`, `d23 00:42`), no data event, no gang.
- **raider: thin (dead).** `_70326` (Red Brothers): `d21 14:01` Raid plan, Muster 353 min, 276-min march "cut at street", no Brawl and no result; `d23 23:45` "Protected: collected 40 coins of protection at Street Market#4323"; `d24 16:36` "turned back on the way to Fight Pit#2159: Dorian Sato leads The Red Brothers now"; `d26 23:00` "Tagged ... done by Garrick Singh". `_15822`: a Bartender; `d21 10:04` "avenged themself on" him, two Story lines `d22 00:01`, then six quiet days.

## 3. Round 3's mechanisms seen at 100k

Fired, quoted:
- **S7 raid result line:** `gang_leader_19331 d24 01:02`. V11's top dull spot is closed. The other raid (`raider_70326 d21`) has no result because the march was cut (bug 5).
- **S1 distance:** "stays off ... trade" 14+ lines (`scav_48740 d21 00:07`: "18 h 11 from home"); "sat out ... raid: the muster ... is 6 h 01 from home" (`gang_member_53886 d22 03:23`, `cultist_26195 d24 00:07`); "chose the shift as Recycler Tech over ... watch (28% the gang's way)" (`dealer_17998 d24 00:07`). Replaces V11's 500-min leader walks; `gang_leader_71953` travels 1-4 h a day.
- **S2 plans that fail at the end:** `raider_70326 d24 16:36` turn-back; a muster sat is kept (`gang_leader_19331` 242 min, then the march and raid). The back-off "N in a row" fires (bug 6).
- **S3 justice:** "Arrest ... for Assault on Liesl Underhill" (`scav_48740 d21 17:07`), "Grand Theft from Razor Kimura" (`purist_57388 d25 03:36`), "Omar Ravensworth resisted arrest and fought off the guard Ivo Marlow" (`runner_46482 d27 03:35`), the fare beating with its cool-down ("left on the platform 30 min", "picked herself up", `cultist_26195 d23 00:41`), "banned from the El for 14 days" (`d26 16:36`).
- **S4 care:** `scav_47612 d21 18:41` and `gang_leader_19331 d22 12:59` patched at a Back Room after BuildingFull.
- **S5 trades:** a hit heard of and run: ContractPosted/Taken/BountyPaid/Tagged in `gang_leader_19331` and `raider_70326`; Protected once.
- **S6:** the `[family]` line present in four diaries; no child harmed anywhere.

Expected, never seen: a Collect that pays (`raider_70326 d24 06:03` Collect 1 min "ended"), a second Raid result, a street Preach, a Deal with a named buyer, a runner's data sale, a jail-duty line for the dealer who was a Jailer, a cell meal line (jail days read "sleep 8.0h, jail 16.0h").

## 4. Bugs, ranked by what the player sees first

1. **A 6-7 year old at home on a school morning.** `gang_member_6764` "Benedikt Sallow#107760 (6 y) at home (play)" `d21`-`d27` 09:30 every day including weekdays; `runner_90375 d23 09:30` "Rin Mercer#117703 (7 y) at home (play)". S6 says School from 6. Either the line is a snapshot or the school path is not reached from these homes. A visibility bug, not a harm.
2. **A Guard whose Work plan is a dead GoTo.** `runner_90375`: "PLAN FAILED: Some(Work) failed at GoTo(Jail): Timeout" 25 times in 6 days (`d23 08:53`, `09:25`, `14:20`, `14:52`; five on `d25`), each a "31 min, 0 tiles (ended)"; the Earn plan then does the same GoTo in 20 min and GuardJail works (`d23 09:46`, 59 min). A guard works 4-5 h of an 8-h shift. S3 jail duty in blocks.
3. **Guard patrol across the island.** `runner_46482 d22 01:25` GoTo(PatrolWaypoint) 287 min, 359 tiles from the Precinct to the Neon Quarter; travel 9-13.8 h a day on `d22`-`d27`, work 0.2-0.9 h; an Escort of 183 min (`d22 23:10`).
4. **Two voices give the standing order.** `raider_70326` (rank 1): Dorian Sato gives him Retaliate (`d21 02:13`), he holds court and gives Ulysses Achebe the order Squat (`d24 15:27`), then `16:36` "Dorian Sato leads The Red Brothers now" with no Succession line.
5. **A mustered raid never lands, and the muster is not credited.** `raider_70326 d21 16:07` Muster 353 min, `22:00` march 276 min "cut at street", no Brawl, no result, then "missed ... court summons" (`d22 00:07`) and "missed ... guard" (`d24 00:07`). V11 bug 3: **unchanged**.
6. **Back-off counter spam: "goal Bury cooled ... backed off, 35 in a row".** `gang_member_53886` to 33, `dealer_17998` to 11, 2-3 lines a day, with "Bury unplannable (no feasible action)" (`d21 02:48`). A goal that cannot plan for a non-gravedigger should not be picked, let alone counted.
7. **263 witnesses to a jail fight.** `dealer_41540 d27 17:00`: "Blackmail: Amara Haas caught Kuro Wozniak (off screen) at Precinct#4485 ... he told her where to go and went for her; Amara Haas won", then "Witness: 263 saw Kuro Wozniak commit Assault (... and 260 more)". Folded to one line, but 263 in a cell block.
8. **Members sell the leader for 3-15c.** `gang_leader_19331 d22 18:22` "Hex Lockridge sold a sighting of Quinn Hawthorne for 15" at her own Hideout, repeated `d23 05:33`, `14:39`; she sells Hex back `d23 19:52`. No court, no ejection.
9. **A strong grudge never walks.** `gang_leader_71953` Robbed grudge 0.87 on Ulric Varley (`d21 00:00`), HuntStarted `d24 06:43`, then silence.
10. **Fare Evasion witness lines.** `gang_leader_71953 d27 11:28`, `12:11`, `17:40`, `18:52` plus `d26 10:47`: five on a rest day, no Fare event follows.

V11 bugs in this group: 1 Collect/Court 1-min no-op after a long walk: **better** (`raider_70326 d23 14:25-23:34` four 11-min Collects; one 1-min `d24 06:03` remains). 2 Walk is the day: **better** for leaders and members, **unchanged** for guards (bugs 2, 3). 3 Mustered raider marked missed: **unchanged**. 4 Raid wall of noise / no result: **fixed** (result line; one folded witness line). 5 Fare Evasion witness loop: **unchanged**. 6 Leader walks to own muster: **better** (`gang_leader_19331` attends). 7 Pavement sleep at the Hideout door: **better** (not seen). 8 Preach never reaches a sermon: **better** (`cultist_26195 d21 19:00`). 9 Self-contract: not seen. 10 ExpansionCap kills: not seen.

## 5. Dull spots (mechanism, not a knob)

1. **The tag has no hunter's walk.** `gang_leader_19331` (`d22 18:00` LIVE) and `raider_70326` (`d24 18:00`) are paid out on sightings; the raider is "Tagged ... done by Garrick Singh" (`d26 23:00`) with no scene. The taker should appear at the door with an ambush and a result line.
2. **A betrayal that costs the betrayer.** A member who sells the leader's sighting is judged at court (a fine, a beating, thrown out).
3. **Shakedown that answers.** `purist_42201` takes the same 3c from Block#474 four days running; the block should escalate (a collector, a refusal like the Fight Pits' `gang_leader_71953 d21 00:09`).
4. **A raid that matters to both sides.** The Garage's 500c is a number; the Zetatech owner should post a locate on the leader (corps already post tags, `d22 00:06`).
5. **Deal with a face.** `dealer_17998 d21 00:00` 178 min for 1c; a named buyer who drinks at the club and can be a snitch.
6. **A scav find that sells.** Still "nothing worth selling" in every scav round read here.
7. **A runner that runs.** A Network contract with a payer, so JackedIn has a data event.
8. **Ronin Park.** Beaten out of the Unplugged (`purist_42201 d22 00:07`), beats a 70 y cultist 22 h later (`cultist_66947 d22 22:00`): the best cross-life thread. Give him a bed or a gang by the week's end.

## 6. Too much

- **Daily refusal lines at 00:07** ("stays off the trade", "sat out the raid", "chose the shift"): the same sentence 4-6 times per life. Fold repeats ("3rd day") or log on the first and on the change.
- **"Bury cooled ... N in a row"** (33, 11): the back-off counter is noise.
- **Jail days** read as sleep 8.0h, jail 16.0h in six diaries (`scav_48740` d22-d27, `dealer_41540` d21-d25, `purist_57388` d25-d27). Harsh, not broken; a cell meal or a cellmate line would carry them.
- Nothing reads as a broken city. The 30-day Crew and GrudgeFormed counters do not show as a wall in this group: GrudgeFormed is 4-5 single lines across the 16.
