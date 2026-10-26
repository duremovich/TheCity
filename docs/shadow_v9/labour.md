# Shadow V9: labour critic

Group: worker, worker_friday, night_shift, fabber, loader, builder, foreman, courier, cabbie, day_labourer (two each, 20 diaries). Window: 100k island, seed 42,
d21-27, `target/shadow_v9/isle100k_s42_d21/`. Method: `camera.tsv` rolled up per person (work, move, home hours, long legs), then full days read for
builder_55127 d22 d27, builder_65590 d23, cabbie_51481 d23, courier_61240 d24, day_labourer_13629 d24, day_labourer_78609 d26-27, fabber_30662 d24 d27,
foreman_79015, loader_87774 d23, night_shift_30238 d22, worker_45057 d22, worker_friday_18794 d21-22, worker_friday_93234 d22. Cites are `file dDD HH:MM`. A missing line is not evidence.

## 1. Verdict

The fixed-building trades (night_shift, worker, foreman, fabber, courier) read as people with a shift and a bed: 8-9 h of work, 1-4 h of travel, and a weekly splurge
(a Legs implant on finance, a motorcycle on finance, a foreman losing his purse at the Gambling Den). Inside the shift the day is a metronome, and four things break the illusion:
builders still walk 6-8 h a day to a random "site" (the V8 bug, unchanged), cabbies drive no one, couriers carry no parcels, and the day labourer has no day labour.
The harsh note is real where it shows (an owner-bartender quits his own bar unpaid, a shakedown and a fight at a day labourer's door), but almost none of it comes from the job.

Pooled over the 20 people, d21-27: roughly 5.7 h work, 4.4 h moving, 12.5 h at home a day (d27 is a rest day for most).

## 2. Per archetype

| archetype | state | excerpt |
| --- | --- | --- |
| night_shift | alive | `night_shift_30238 d26 17:30` -45c "bought a Legs implant T1 at Ripperdoc#5946 for 60 on finance"; 0.9 h moving, 56-90 Unloads inside the own Street Market, a 27-min Break on the street every night (`d22 01:52`). |
| foreman | alive, thin day | `foreman_79015 d23 19:37` loses 10c at Gambling Den#5637, wins 10c at `d25 06:17`; `foreman_10841` lays off one Builder every midnight (`d21-27 00:07`, "revenue, a shift past full staff"). The builders are never on screen. |
| worker (Vat Tech, Courier; labels drift) | alive, flat | `worker_45057 d22 18:02` pay, Club#6072 91 min (-5c), a Tip to a Statistical dancer, bed. Reads like a job, but see courier. |
| fabber | thin | `fabber_29680` FabWork 537 min on one tile every day, 0.6 h of movement, no breaks (`d21-26`). `fabber_30662` commutes 222 min each way D1 to D6 (`d24 16:34`, `d27 06:02`): 7.6 h travel, 5.7 h sleep. |
| loader | thin | `loader_87774` works 2.3 h and walks 8.4 h inside one district: Freight Yard to Fixer#6028 82 min, back 81 min (`d23 03:16`, `d23 04:56`), then the shift closes mid-ClerkWork and he drinks at 06:57. |
| courier | thin | `courier_61240 d24 09:02-11:40`: GoTo(Station), ClerkWork 16, GoTo(Station), Unload 16, on repeat, all at Freight Yard#6026. Zero "parcel" lines in either courier or worker_45057. |
| builder | broken | bug 1 |
| cabbie | dead | bug 2 |
| day_labourer | dead as a trade, alive as a person | bug 3, dull spot 1 |
| worker_friday | mixed | `_18794` is a Bartender who owns Bar#4467 and quits it unpaid (bug 4); `_93234` is a Recycler Tech who loses a shift and 18 h to a corpse (bug 5). |

## 3. Bugs, ranked by what the player sees first

1. **The builder's site day is still a hike (V8 bug 1, unfixed).** `builder_55127 d22 07:43-18:02`: home D4 to the Yard in D3 (126 min), `GoTo(TargetHome)` 290 min to Garage#1749 in D1,
   Unload 16 min, ClerkWork 160 min, then 370 min home. Pick average d21-27: 11.8 h moving, 3.7 h working, net +1c to +2c a day. `builder_65590 d23 09:21` is 333 min to Noodle Bar#1432 (D1) as "TargetHome";
   ClerkWork there fails twice with PreconditionLost (`15:11`, `15:29`), the Work goal cools, he eats and wanders. There is no site to see: the "site" is a random garage or noodle bar and nothing is built.
   Likely cause: the site target is picked by building type, not by distance from the Yard, and ClerkWork wants the Yard (its target shows `Construction Yard#5590` while he stands in the bar).
2. **Cabbies drive nobody.** `cabbie_51481`, `cabbie_69861`, d21-27: zero fares (V8 had 8), zero pickup legs, zero "as a fare" lines. 51 of 144 actions in `cabbie_69861` are `Rest` at the Garage, which the summary books as 8.2 h of work
   (`cabbie_51481 d23 15:47` and `17:20`, Rest 91 min each). The wage lands anyway (`d23 00:00` +6c). A dead trade that pays.
3. **The day labourer never labours.** `day_labourer_13629`, `_78609`: 0.0 h of work on 14 of 14 days. Earn is always `Wander > Scavenge` (97 Scavenge actions in one diary), then "goal Earn cooled ... could not plan or plan failed twice"
   (`13629 d21 09:50`, `d22 08:14`). Net 0c on 5 of 7 days; food comes from the home pantry (`EatAtHome`). Nothing in the labour market reaches a jobless adult.
4. **An owner quits his own bar "unpaid".** `worker_friday_18794`, Bartender, `Owns: Bar#4467`: wages go out of his till to Statistical staff (`d22 17:35` -4c to Nadia Crane#56975), then `d22 18:00` "Quit: Wren Oakes quit as Bartender (unpaid)".
   The place owner flips between "corp Habitat 9" and "Wren Oakes#18794" from line to line (`d21 20:28` vs `d22 04:57`). After the quit he still receives Drink income at home (`d23 21:00` +5c), misses rent every night
   (`RentShort d22-d26`), and a notice is served `d26 09:53`. Likely cause: the owner's own post is paid last from a till that also pays other bartenders, and the bar has two owner records.
5. **Civic duty eats a shift and a day, and the burial fails.** `worker_friday_93234 d22 02:09`: goal Bury, a 620-min leg to a corpse in D6, carry it, then 449 min to "Cemetery" which resolves to his own workplace Recycler#4486 (D4); `20:19` BuryCorpse then
   `PLAN FAILED ... PreconditionLost`. 18.9 h of travel, 0.4 h of work, no wage, corpse unburied. He had slept among 23 corpse sightings (`d22 00:10`). 415 violent deaths in 30 days at 100k and this man is the only undertaker.
6. **The shift ends under the worker's feet.** `loader_87774 d23 06:20` ClerkWork 1 min then PreconditionLost: the 21:00-06:00 shift closed while he was still walking 81 min to the Yard. The legs of that shift were for nothing.
7. **Unlabelled money.** `foreman_79015 d22 00:09` +26c "(other)" with no flow: a bribe, a hire fee, or a leak. The player cannot tell which.
8. **Visible jump.** `fabber_30662 d23-26 07:34` "JUMP (a Full body) 4 tiles in 1 min" on the transit commute (Fare -1c). Small, daily.

## 4. Dull spots, ranked (mechanism, not a knob)

1. **The jobless have no market.** Two of two day labourers idle 22 h a day at home or fail at Scavenge for a week. Mechanism: the shape-up. At dawn a foreman's yard, a Fixer or a Street Market posts "N hands wanted today";
   jobless adults within reach walk there, the foreman picks by strength and by who he knows (a friend or a grudge decides), pays the day's wage in scrip, and the ones turned away are a scene: a fight at the gate,
   a scab shift against a rival's gang. It gives the day_labourer a day and the nightly `LaidOff` events of `foreman_10841` a face (a named Builder laid off every midnight, never met).
2. **Every shift is a loop of Unload and ClerkWork on one tile.** Courier, foreman, loader, night_shift, worker: about 65 Unloads and 72 ClerkWork a day, 16 min each, same building, `GoTo(Station)` between. Mechanism: the work day
   carries an incident table keyed to the trade (a short count at the dock, a parcel marked for a gang stash that the courier may open or deliver, a crate that is not what the manifest says, an inspector, a fall). One incident a day
   per worker leaves a memory and a grudge or a coin and changes the next day's plan. The courier carries a parcel from a named Block to a named Block and the diary says who was waiting.
3. **The cab has no fares.** Mechanism: the cabbie takes real bodies on screen. A body that needs to cross districts (injured, wanted, late for a shift, rich) hails the nearest cab and the ride has a destination; the fare is an event
   the passenger remembers; some passengers want to go where the cabbie should not (a hideout, a clinic that asks no questions).
4. **The commute is the character, and it is empty.** Fabber D1-D6 (222 min), builder D4-D1 (333 min), loader 80-min hops. Mechanism: the long leg meets people: a regular fellow rider, a fare dodger
   (`Witness Fare Evasion` already fires, `builder_55127 d22 12:00`), a mugger on the long leg, a stall that sells a meal. A three-hour walk that never meets anyone should not exist; one that does is a story.
5. **The rest day is ten hours of `Rest`.** `builder_55127 d27 11:12-22:00`, `foreman_10841 d27`, `worker_45057 d27`: 90-min Idle Rest blocks at home; the Free-rung HangOut finds "nobody there" every time (`fabber_30662 d27`, 7 of 7).
   Mechanism: the day off pulls toward a weekly ritual with people in it (the church, the Den, a family visit, the other builder in the same Block: `builder_55127 d27 08:45` Chat with Tristan Zhou#65590 is the one scene in the day and it is lovely).
6. **Wages are 5-8c and nothing is planned.** Net +1c to +7c a day, with a lump buy (implant, motorcycle) as the only drama. Mechanism: a wage advance or a site loan from the foreman or the corp, with a
   collector and a missed-payment chain, so the lump purchase (`night_shift_30238 d26 17:30`, `cabbie_51481 d27 14:08`) has a creditor who turns up.

## 5. Evidence for PRESSURE_PASS targets

- **Phase 1, misery acts.** `worker_friday_18794` is the case Phase 1 wants: unpaid, missing rent every night, notice served, a `Quit` event already exists to reuse for the `[misery]` quit. No employed person in the 12 here skipped a shift
  d21-27, so absenteeism has no home yet. `day_labourer_13629 d24 08:00-08:56` shows a shakedown, an Intimidate (-2c), a Fight and a 283-min ReportCrime walk inside an hour, so a broke body already reaches Fight when provoked.
- **Phase 2, the dead trades.** The cabbie (bug 2) and the day_labourer (bug 3) are dead trades of the same kind; add them beside the bouncer and the joytoy.
- **Phase 4, reach.** `worker_friday_93234 d22 02:09` (620-min Bury leg), `day_labourer_13629 d24 08:56` (283-min ReportCrime to the Hall), the builder's 333-min site leg and the fabber's 222-min commute are all reach failures:
  Bury, ReportCrime and the site target need a distance cap. The builder site day is not in the pass at all and should be a fix phase.
