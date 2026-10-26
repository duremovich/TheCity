# Shadow V9: the 100k city, read the new way (2026-10-26)

The first shadow pass under the reset doctrine. One window: main `4108184`, the 100k island
(`isle_m_k2_100k`), seed 42, days 21-27, two picks per archetype (82 diaries), plus one 30-day counter run of the
same seed. Seven Sonnet critics read it (`docs/plans/shadow_v9.md`); their reports are `docs/shadow_v9/<group>.md`.
Runs in `target/shadow_v9/` (diaries `isle100k_s42_d21/`, counters `analyze100k.txt`). No device rows, no V8 tables.

## Verdict

The 100k city reads as a city of commuters with job titles. Seven critics of seven name the same first thing: a
followed character spends hours on legs that should take minutes, often inside one district, and arrives with
nothing to do. Under the walking, most trades are a title without a trade: no patients, no fares, no clients, no
door, no sermon, no raid result. The stories the sim does make (a revenge hit, a creed-leaver hunted, a CEO reported
for a shakedown, a dancer who steals a bike) start and then nothing picks them up.

## Bugs, ranked by what the player sees first

1. **Distance at 100k.** Legs of 4-11 h, same-district El rides of 4-9 h, a CEO starving with 500c because food is a
   329-min trip, a builder's 12-h moving day, a vendor's pitch two districts away, a sweeper's post across the island.
   Island-pass distance (V8) held at 20k; 100k breaks it. Every critic.
2. **Six trades never hired at 100k**: Office Worker, Teacher, Madam, Back-Room Doc, Contract Host, Fighter (0 hires
   in 30 days in `events100k.tsv`; 64k hires overall). The shadow found no candidate for seven archetypes.
3. **Quit "unpaid" with money in hand**, including owners quitting their own bar, clinic or shop (forger, cook,
   orderly, bartender-owner); partial wages; same-minute Quit and Hire.
4. **Gang verbs that no-op**: GangWork `[GoTo(Spot) > Preach]` planned 17 times, executed 0; extortion of 0 coins
   logged as success; "Collected 0 to 0 members" (`OPEN-118`); 17 `Protected` against 1,510 Extortion.
5. **The law's noise**: 641k Witness events in 30 days; a guard logs 113 fare evasions, arrests nobody, is arrested
   for one himself; 13 of 16 guard Arrest plans fail `PreconditionLost`; ReportCrime never completes (the walk).
6. **Care-seeking is missing**: diagnosed cancer rising for a week never plans a clinic trip; 158 `Treated` city-wide.
7. Smaller: Bury resolves the Cemetery to the burier's workplace; the super is a 538-min one-tile statue again; the
   cabbie drives 0 fares; the day labourer works 0 h; a leader sleeps 13 h on a pavement; an unexplained arrest.

## Dull spots worth a mechanism (the critics' best)

- **Consequence**: witnesses react (report, flee, gossip, blackmail); raids write a result (wounded, a smashed till,
  stolen stock, a grudge between crews); an expired hit is reposted higher or the buyer does it; jail has cellmates
  and contacts, release returns a member to the gang (welcome or a "you talked" beating), jail absences cost the job.
- **The faith on screen**: a creed's service at a Chapel at a set hour, a congregation, a tithe, conversion or
  walking out, the leaver hunted as a heretic.
- **Gang duty vs the day job**: an order puts a shift on the member's schedule with pay; missing it costs standing.
- **Work with incidents**: a per-trade incident table (a spill, a fight on the floor, a theft from stock, a
  customer's complaint); a dawn shape-up at the yard for the jobless; fares and clients sent to the cab, the dancer,
  the companion, the dealer, the vendor by bodies.
- **Care**: patients walk in through a queue; the sick choose between cost, the black doc, begging and pawning.

## What this does to the pass

`PRESSURE_PASS.md` gains fix phases F1 (distance at 100k), F2 (the missing trades and the gang/law no-ops), F3
(quits and wages, after phase 1 lands) and phase 5 (consequence), and phase 2 grows (cab fares, the shape-up,
care-seeking, the super's round, the dealer's and scav's day).
