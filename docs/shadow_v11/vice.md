# Shadow V11 critique: vice (club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke)

Window: 100k island, seed 42, d21-d27, two picks each (18 diaries). Cites are `file dDD HH:MM` under `target/shadow_v11/isle100k_s42_d21/`.
Method: day summaries and camera tables for all 18, event-tag counts, greps of `[event]` and `[law]`, full reads of owner_broke_7581 d21-d23, madam_104391 d22-d25, fighter_64359 d22-d24, club_staff_35900 d23.

## 1. Verdict

Clearly better than V10: the madam, the fighter and the joytoy now have a product (nightly bookings with a cut, a rough client barred, a Pit bout, a host who serves visitors), and one broke owner lives a whole arc (unpaid wage, quit, Hall garnishment, layoff, arrest, ten days in the cells, repossession).
The pace fix shows: club_staff_34383 walks 256 tiles in 173 min (0.68 min/tile; V10 was 1.8-2.6), so no one loses a night to a commute, though a few still spend 5-8 h a day travelling to far posts.
Stories start but are still dropped: the bouncers never open a door, the fighter's loss has no sequel, the barred client is never seen again, and d27 still drops every shift.

## 2. Per archetype

- **club_staff: thin (V10 thin/dead).** club_staff_34383 works 5.8 h daily but travels 7.7 h daily on a 256-tile commute to Club#4874 (D6); nothing else happens. club_staff_35900 is livelier: `d23 00:02 GoTo(Spot) ... cut at street (D3), 138 min, 276 tiles` for stims, `d23 02:20 GoTo(Hotel) 92 min`, `d23 07:49 GoTo(Home) 333 min, 325 tiles`: 11.1 h of travel in one day, 3.9 h sleep. It then witnesses five Dealing/Theft crimes in an afternoon (`d23 14:50-15:58`) and does nothing with them.
- **bouncer: dead (V10 one alive, one dead).** Both work 4.2-5.9 h at a Strip Club, idle 8-12 h, and have zero refusal, ejection or Brawl lines in 12 nights of door work. `bouncer_57664 d26 18:15 Marriage: Mina Dunmore married Percival Upton` is the only non-witness event. V10's door story is absent from this sample.
- **dancer: thin (V10 alive/thin).** dancer_74279 works 4.7-5.8 h, 52 camera lines, no tip or regular line; `d25 12:07 Treated: Echo Duarte took Therapy at Ripperdoc#5105` is the sole event. dancer_104644 travels 4.5-7.3 h a day (`d24: sleep 3.2h, work 1.8h, travel 7.3h`) and sees `d24 19:21 GangJoin: Ignatius Fletcher joined The Drowned Choir 7` plus four Fare Evasion witness lines at 19:28, with no reaction.
- **companion: alive / thin (V10 one alive, one dead).** companion_73001: `d21 21:00 Served: Hesper Oakes is now a regular of Clemence Ravensworth at Spire Lounge#4884`, more regulars d22 and d24. companion_87767 shows no Served line and 4.0 h and 5.6 h travel days; its commute is now 51 tiles, so V10's 866-minute walk is gone.
- **cook: alive, flat (V10 alive, flat).** cook_74736 `d26 00:07 LaidOff: Dmitri Thorne let Joss Quarry go ... (0c in hand against 9c a day in wages), owed 3c`, then `d26 00:11 Hire: Joss Quarry hired as Cook (picked by Bronwen Novak, the foreman)`. cook_34100 `d24 14:24 WitnessReacted: Sebastian Thorne saw Vale Dunmore's Murder and ran`, a good beat with no follow-up.
- **fighter: alive (V10 thin).** fighter_64359 is the best arc in the group: `d22 00:00 Skipped: Xia Park is off sick ... Anton Nightingale covers the Fighter's shift`, `d23 00:00 Diagnosed: vat-cancer`, `d23 13:38 Cured: ... (289c on the doctor's slate)`, `d24 13:24 Assault: Xia Park attacked Smoke Chrome`, `d24 15:52 Sentence: 10 days`, jailed d24-d27, `d27 13:46 Starving`. fighter_42709: `d26 23:00 Bout: Tess Duarte beat Ilse Fairweather at Fight Pit#2344 (bets paid 6)`; he is the loser and the next entry is ClerkWork at 23:09, with no injury, purse or grudge.
- **joytoy: alive / thin (V10 dead/thin).** joytoy_51830 serves visitors nightly: `d21 21:00 Served: Lorcan Underhill served Imogen Wexford (visit) at Dollhouse#5000` (x3), again d23-d25, but 3.2-4.7 h travel on a 76-tile commute. joytoy_87496 carries dread 0.99 (`d22 00:00 [rep] dread 0.99 ... heat 0.48`) decaying to 0.38 by d27 with no cause in the window, and one Served line (`d24 21:00`).
- **madam: alive (V10 thin).** madam_104391: `d25 00:00 Served: Ursula Inchbald booked 99 clients at Brothel#5491 tonight (Adela Tanner 23, ...), 771c through the house; 2 rough, thrown out`; `d24 18:30 ... Dex Coldwater roughed up Fenn Bartlett ...; threw him out and barred him from the house`. madam_47125 books 14-20 a night; `d26 23:34 Bex Mbeki roughed up Zed Novak ... barred`. Her own cut is small (`d22 00:00 +14c Wage` against 246c through the house).
- **owner_broke: alive (V10 alive/thin).** owner_broke_7581 (section 3). owner_broke_15514: `d22 00:07 UnderWater ... hangs on`, `d22 21:51 Quit: Adela Yates walked out ... (unpaid, owed 21c)`, `d26 10:00 Confronted: ... left with nothing (grudge 0.45)`: V10's confrontation again, still ending at nothing.

## 3. Round 2's mechanisms seen at 100k

Fired:
- **R6a madam's bookings and the rough client barred:** both madams, six nights each (lines above).
- **R6a Dollhouse visits:** joytoy_51830 d21-d25.
- **R6a bets on the bout:** fighter_42709 `d26 23:00`, "bets paid 6".
- **R3 the sick covered, the doctor's slate:** fighter_64359 d22-d23. madam_104391 `d26 10:53 Treated ... Therapy`.
- **Hall garnishment, arrest, cells, repossession:** owner_broke_7581 `d22 00:07 Confronted: the Hall took 4c from Mei Haas for Imogen Ellery's wages (21c still owed)`, repeated d23 (17c owed); `d23 01:43 Arrest`, `03:08 Sentence ... 10 days`; `d23 13:00 Imani Adeyemi ... reported Mei Haas to the Hall for the wages`; `d26 00:07 Cells: Quill Wozniak gave Mei Haas a name in the cells: ask at Fixer#5120`; `d27 00:07 Repossessed: Sable Marlow took Bar#4355 from Mei Haas for the 557c still owed`. That delivers V10's ask for a sequel to the confrontation.
- **Deck purchases on finance:** fighter_42709 `d26 00:32`, then `01:21 JackedIn`; madam_104391 `d23 10:07`.

Expected, never seen: any bouncer door line; a tip or protection/tribute line for dancers and club staff; the madam's raid or crew tribute; a fighter's purse or injury; a Pit bout for fighter_64359 (jailed); an R1 fare ladder or El ban line for the long walkers; the Hq-as-office or landlord desk (R6b) for either broke owner; gun hunts (none in this group).

## 4. Bugs, ranked by what the player sees first

1. **d27 drops every shift (V10 bug 2: unchanged).** Work hours on d27 are 0.0-0.6 for all picks (camera rows for cook_74736, dancer_74279, bouncer_92827, joytoy_51830: work 0.2/0.1/0.1/0.1), while the second camera table shows the coarse column at 5.9-18.2 h (bouncer_92827 18.2, cook_74736 15.3, joytoy_51830 9.4). The pick drops to coarse LOD on the last day with no diary line; likely the last-day slice, but the player sees a closed city. `madam_104391 d27 00:02 GOAL Idle (was Work)` is the same.
2. **Distant stim run by a salaried attendant (new).** `club_staff_35900 d23 00:02 GoTo(Spot) ... 138 min, 276 tiles` to D3, then 92 min to a hotel, then 333 min home: 11.1 h travel, 3.9 h sleep. At 14:15 the same person buys stims at Club#5182, 18 tiles away. Likely the stim source choice at 00:00 ignores distance when the near one is closed.
3. **Far posts still eat 7.7 h a day (V10 bug 1: better).** `club_staff_34383`: 256 tiles, travel 7.7 h on d22-d26 (7.7/7.7/7.7/7.7 to the decimal), work 5.8 h. Pace is no longer the fault; the job match is.
4. **Bouncers have no door (V10 bug "door runs once a fortnight": worse in this sample).** 0 door lines in 12 bouncer-nights vs 1 in 14 last time.
5. **Dread 0.99 with no deed (new).** `joytoy_87496 d22 00:00 [rep] dread 0.99 standing 0.40 heat 0.48`, header `known by 172`, decaying daily. The only rumour (`d23 00:01 Assaulted by corp Kessler 6 on Rue Garland#9347`, from d14) names someone else. Flag for the engine.
6. **Fired and rehired in four minutes (new).** `cook_74736 d26 00:07 LaidOff ... d26 00:11 Hire: Joss Quarry hired as Cook`. Reads as a bug, not a story.
7. **Gossip flood on the madam (new).** madam_104391 has 176 `GOSSIP ABOUT ME` lines, mostly the same d21 assault (`d21 01:02`, `02:29`, `02:35`). Noise.
8. **No food in the cells (new).** `fighter_64359 d27 13:46 Starving` while jailed 24 h a day since d24. Harsh is fine; reads as an omission four days into a ten-day sentence.
9. **Fare Evasion witness spam (V10 regular spam: different form).** `dancer_104644 d24 19:28` four identical witness lines in one minute; `companion_73001 d21 12:21`, `d27 07:09`. Seen, never fined or chased.

V10 bug status: commutes of hours: better; d27 skip: unchanged; sick bouncer invisible, ExpansionCap loop, Idle/Eat flip, regular spam, robbed-gossip with no event: not seen in this sample (not evidence of a fix).

## 5. Dull spots, ranked

1. **The door.** A bouncer should resolve every arrival against drunk, armed, colours and grudge; the turned-away person waits outside and returns with a friend.
2. **The bout has no consequence.** The loser needs an injury, a purse split, a rematch grudge the Pit posts. fighter_42709 loses and clerks on.
3. **The barred client is never seen again.** `madam_104391 d24 18:30` Dex Coldwater is thrown out; give him a grudge, a friend with a knife, or a bribe to the doorman that a later night shows.
4. **Dancers and club staff earn nothing visible.** No tip or regular line for either dancer or club_staff_34383. Reuse the companion regulars mechanism.
5. **The quiet owner gets no sequel.** owner_broke_15514 hangs on at `0c in hand against 3c a day`; give it the 7581 chain (Hall garnish, arrest) or let a fixer buy the debt.
6. **Witness lines lead nowhere.** Murder, Dealing, Grand Theft: five an afternoon for club_staff_35900. A club hand who sees Dealing should call the Hall, tell the gang, or be threatened.
7. **Joytoy dread without a cause** (bug 5): if real, give it an incident.

## 6. Too much

- Nothing reads as a broken city. The 99-client night (`madam_104391 d25 00:00`, 771c) is a spike against 62, 53, 58, 56 but plausible.
- Noise: the gossip flood (bug 7) and witness spam (bug 9) bury the good lines. Not harshness.
- Monotony: club_staff_34383's identical 7.7 h travel five days running reads as a metronome, not a life.
