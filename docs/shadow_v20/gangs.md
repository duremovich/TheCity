# Shadow V20: the gangs (critic report)

Group: gang_member `_94430`, gang_leader `_105912`, raider `_20130`, cultist `_17637`, jailed `_62226`, dealer `_33044`
(one pick each; V19 had two). 100k island, seed 42, d21-d27. Cites `<arch>_<id> dDD HH:MM` under
`target/v20/isle100k_s42_d21/`. Raid counts are from `target/v20/events.tsv` (30 days), by line, not by raid.

## 1. Verdict

- **Better as a city, still thin as a gang life.** Walkers wait for the green man, a raider climbs eight floors in her own body,
  a leader is let out early from a crowded cell, the yard has cards in it. The V7-V9 pieces all show in these diaries.
- **But the week is still the cells and the day job**: 2 of 6 are in a Precinct all week (`jailed`, `gang_leader`); the three free
  gang lives (member, raider, dealer) work 6-9 h a day and commit 0-1 h of crime. Hunt never acts (1 goal in 6 diaries, cooled).
- No minor harmed (below). Raid "hurt" lists now print ages: V19 bug 6 is fixed.

## 2. The exit's checks

**(a) A street crossed at the lights with a wait: seen, plentifully.**
- `gang_member_94430 d21 19:56`: "CROSS at the lights at (820, 728) (junction 821,731), 37 s at the kerb for the green man"; `20:02`
  71 s; `d22 06:24` "on the green man" (no wait). Waits vary 2-71 s. 21 kerb lines in that diary, 19 in the raider's.
- Jaywalking is not near-forbidden for everyone: `dealer_33044` 34 and `cultist_17637` 23 "CROSS jaywalking the carriageway ... no
  crossing" (`dealer_33044 d25 02:23`, `02:27`, `06:13`, `09:27`; 27 of the 34 on d25-d26); the member 7; the raider 0.
  No `footbridge` line in any of the six.

**(b) An El ride a person would take: seen twice, both poor.**
- `cultist_17637 d24 18:16`: "EL ride to Block#521: on foot to the stop at (823, 619) ..., the Hillside Line at 18:18, off at the stop at
  (558, 616) at 18:22 (1 leg, jumping the gate)"; aboard `18:28`, off `18:32`. The walk it replaces was 482 tiles, 29 min; he
  gets home about the same time, and `18:19` "Witness: 4 saw Cutter Vance (52 y) commit Fare Evasion".
- `dealer_33044 d26 06:14`: planned at a market, then a 27-min walk to the Vats stop (`06:21` into D6 The Vats), aboard `06:41`, off in
  D4 `06:48`, then the walk home. Door to door about 43 min against a 43 min walk (996 tiles): saves nothing and walks away first.
  Both are "jumping the gate": no one in these diaries pays a fare.

**(c) A raid climbed in a member's own body: seen.**
- `raider_20130 d21 00:03`: "The Black Ghosts Splinter 181's raiders (18) went up the stairs of Block#2636 to the Red Lanterns Hideout on
  floor 8 (the lift cut), the last up in 7 min"; her own `[climb]` "CLIMB up by the stairs in Block#2636: floor 0 to 8, 4 min in all";
  `00:07` now at floor 8. The chain: `00:00` GOAL Raid (muster called, courage 0.19->0.43), PLAN `[GoTo(MusterPoint) > Muster >
  GoTo(RaidTarget) > Brawl]`.
- It lands on nobody: `00:33` "walked in, nobody held it (20 raiders, 0 defenders) ... took 11c and 0 food". Back on the street `00:33`,
  Unwind `00:36`. The climb is real; the prize is 11 coins (bug 2).
- Muster break-ups: 20 "broke up at the muster" lines in events.tsv, e.g. day 8610 "The Unplugged 7's raid on Corp HQ#9069 broke up at
  the muster at Hideout#8451: 2 came (Alaric Oakes, Pixel Ramirez); 6 asleep, 20 in the cells". It names who came and where the rest were.
  None of the six diaries is one (the raider's has one `Muster` line).
- 30-day Raid file: 501 lines; 66 "walked in, nobody held it"; 96 "held"/"Lost"; 44 "went up the stairs"/lift/climb lines.

**(d) A cell stay that varies, ends by bail or early release: early release seen, bail not.**
- `gang_leader_105912`: arrested `d22 03:28` (Shakedown), 6 days at `04:00`; "let out early from Precinct#6407, 6 days short on her
  Shakedown: the cells crowded (2187 held, over 1200)" `d23 00:07`. Re-arrested at home `d24 04:02`, 6 days, Station -> Precinct
  `d25 04:00`, "let out early ... 5 days short ... (2177 held, over 1200)" `d26 00:07`, arrested again `d26 00:40`.
- Cell life: cards in the yard (`jailed_62226 d21 15:00` -2c, `d24 15:00` +2c), a yard walk "with Zara Haas (in for Dealing, out d27)"
  (`d22 15:00`), "out d29", "out d31", "out d32" for the other prisoners. No bail, buy-out, work detail or BreakOut line in the six.
- `jailed_62226` herself: d21-d27 each "sleep 8.0h, jail 16.0h", Precinct 1440m; the 15:00 yard line is the only variety.
- 30 days: 26,645 Arrest, 44,363 Release lines; jailed mean 1616, max 2912 (`analyze.txt`).

**(e) Any minor harmed: none found.** Events.tsv Raid, Murder, Arrest, Release, Assault lines: none with an age under 18 (filtered by
"(0-17 y)"); under-18 ages in that file are births, school and care lines only. Hurt lists print 26-47. All Arrest/Witness/Cells
lines in the six diaries name adults (guards Odile Hallow 27 y, Seraphine Draper 28 y). I did not read all 459 Murder lines.

**(f) Anyone starving: none in the six.** 46 `Starving` lines in 30 days at 100k, `deaths_starvation=2`, `starvation_deaths_per_week=0.47`
(`analyze.txt`). 20k: 13 Starving lines, 3 deaths, 0.7 a week (`analyze20k.txt`). OPEN-277's "hundreds a month" is not in these
counters; the street critic owns who the 46 are.

## 3. Per archetype (V19's word in brackets)

- **gang_member: thin (alive).** `_94430` Hilde Draper (41 y, Plant Operator, Ash Knives, 53 members): works `d23`-`d27`, order LieLow, 0 Raid,
  0 Deal, 17 Succession lines in the week (`d23 01:22` Aldo Kemp, `03:38` Clemence Stroud, `06:08` Rin Ito: three leaders in five hours).
- **gang_leader: alive (alive).** `_105912` Elspeth Sallow (Glass Rats, order Raid all week): the arrest / early release / re-arrest loop
  is the story; the order Raid never runs for her.
- **raider: alive (alive).** `_20130` (20 y, Orderly): the midnight raid `d21`, then a job at her own tower's Ripperdoc (commute 0),
  19 kerb lines, 0 jaywalks. A raider who is mostly an orderly.
- **cultist: thin (alive).** `_17637` Cutter Vance (52 y, Super, courage 0.08): 2 Preach actions (`d26 06:38`, `d27 02:15`, 31 min on the
  street); `d23`-`d24` idle 10-12 h, 4 h "other", 600-800 m on the street. Two `[el]` rides, 23 jaywalks.
- **jailed: dead (thin).** `_62226`: seven identical days.
- **dealer: thin (thin).** `_33044` Freya Sato (38 y Builder, Low Choir rank 1, treasury 800c): one Deal in 7 days (`d21 00:00` 194 min at
  Club#9021, with 16 witnesses of Nerys Ito's 6 sales), then a builder's week. `d25 07:23` GOAL Hunt "cooled until d25 09:23 (could not
  plan or plan failed twice)", then Unwind.

## 4. Bugs, ranked by what the player sees first

1. **The week is cells and the job: unchanged.** 2 of 6 all-week in a Precinct at 2187 held over a 1200 cap (`gang_leader_105912 d23
   00:07`); a raid crew of 28 has 20 in the cells (day 8610). V19 bug 1: unchanged.
2. **A raid climbs eight floors for 11 coins and nobody home** (`raider_20130 d21 00:03`-`00:33`). OPEN-278 lands, but the Hideout
   is still empty: 66 "nobody held it" lines in 501 Raid lines (16 at Hideouts). Likely cause: the defenders are in the cells or at work.
3. **The revolving door (new).** A Shakedown leader is let out a day in, re-arrested at home 25 h later for the same crime, let out a
   day in again, arrested 40 min after (`gang_leader_105912 d23 00:07`, `d24 04:02`, `d26 00:07`, `d26 00:40`). The early release
   works; nothing changes between release and the next arrest.
4. **The El: gate-jumping and a ride that goes the wrong way.** `cultist_17637 d24 18:16` and `dealer_33044 d26 06:14` both "jumping
   the gate" with Fare Evasion witnessed by 4; the dealer walks 27 min toward the Vats to ride back (saves about 0 min). Likely
   cause: the walk-or-ride compare does not count the walk away from home to reach the stop.
5. **Jaywalking is a habit for two of six**: `dealer_33044` 34 (27 on d25-d26), `cultist_17637` 23, mostly 2-min hops (`d25 02:23`,
   `02:27`). The brief says near-forbidden. Not found: why (cause not diagnosed); the rest of the six cross at the lights.
6. **Succession churn: unchanged.** 17 Succession lines in `gang_member_94430` in 7 days, 18 in `cultist_17637`, 13 dealer, 10 leader.
   V19 bug 2: unchanged.
7. **Hunt / Avenge / Revenge: unchanged.** 1 GOAL Hunt in 6 diaries (cooled); 5516 Hunt events in the 30-day file, so the mechanism fires
   elsewhere. V19 bug 3: unchanged here.
8. **Plan failures**: `PLAN FAILED` 30 (member), 25 (cultist), 16 (dealer), 16 (raider) in 7 days; `BuildingFull` 10 times for the dealer's
   GetHigh (`d24 18:15` the cultist's). Camera JUMP noise remains (`gang_leader_105912 d22 04:00`, "allowance 2").
9. **Archetype name versus life: unchanged** ("dealer" one Deal a week; "raider" an Orderly; "gang_member" LieLow).

V19 bugs in this group: 1 cells **unchanged**; 2 succession **unchanged**; 3 Hunt **unchanged**; 4 mismatched hit price **not seen**;
5 Bury loop **not seen**; 6 raid hurt lists without ages **fixed**; 7 long muster wait **better** (a short crew now breaks up naming
who came); 8 lifts no queue **not seen** (no lift line in the six); 9 camera JUMP **unchanged**; 10 archetype mismatch **unchanged**.

## 5. Dull spots and too-much

- Dull: `jailed_62226` d21-d27 (about 2 lines a day); the member's `d22`-`d27` idle 5-10 h (`d23` 9.8 h).
- Too much: 21 kerb lines in one diary; the "next: Expand/LieLow" tails on every raid line; "Witness: 16 saw ..." for one stim deal.
- Good: the muster break-up line (who came, who slept, who was in the cells); kerb waits that vary; yard cards with named neighbours.
