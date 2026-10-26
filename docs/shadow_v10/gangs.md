# Shadow V10: the gangs (critic report)

Group: gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider (two picks each, 16 diaries). Main `33f9c55`, 100k island, seed 42, d21-d27. Cites are `<arch>_<id> dDD HH:MM` under `target/shadow_v10/isle100k_s42_d21/`. Counts are greps and awk over the 16 diaries. d27 is a rest day for job holders.

## 1. Verdict

- **Better than V9, still a day job with a badge.** Phase 5b's consequences fire and read like a harsh city: a missed watch costs the colours, a failed court names who did not come, release brings a grudge, a leader walks out and finds a church. Most of the 16 still hold a wage post and live a worker's life.
- **Stories start and a few get picked up.** Missed duty, beaten, expelled (`dealer_51168`) and jailed, grudge, left the gang, converted (`gang_leader_20414`) are real arcs. The raid, the deal and the scav round still produce nothing, and the walk still eats the day.
- **Noise has replaced emptiness in one place.** Fare Evasion is the whole criminal career of two people (`scav_31438`, `raider_85728` jailed four times each in 7 days).

## 2. Per archetype (V9 word in brackets)

- **gang_member: thin (thin).** `_12322` is a Builder 10 h a day, 17c; the gang is one refused extortion (`d21 02:24` "Block#519 would not pay Dorian Stroud"), then it walks away (`d27 00:05` GangLeave, `00:08` "took up Church of the Open Circuit"). `_6592` is a Vat Tech; one Assault on him at `d25 05:12`, flee home, no follow-up.
- **gang_leader: alive, one (thin).** `_20414` (Pale Dogs, 42 members): `d21 21:00` "court failed: nobody came (summoned Kasimir Ferreira (did not come)... 2 failed in 7 days)", `d22 05:43` Cells "learned who put him inside", `d22 15:43` released "to a changed life: a grudge on Gwendolyn Sallow", `d26 00:05` "left The Pale Dogs (walked away)", `d26 00:08` "took up Church of the Open Circuit (miserable, mood -0.33)". `_71747` (Glass Teeth, LieLow then Raid) is a night Vat Tech with a 638 min commute and 11-17 h of travel a day; she sleeps 783 min on a pavement (`d24 06:47`).
- **purist: thin (thin, one good beat).** `_85772` has the best crew line: `d21 00:07` "chose the shift as Sanitation over The Unplugged 3's watch (11% the gang's way)", `d22 00:07` "missed the watch (1 in 10 days)", `d22 20:00` Service "heard Roderick Singh preach... put 3c on the plate", `d23 00:40` arrested, `d23 07:57` "6 days for Grand Theft", jail to d27. `_25175` musters (below), no raid, 7 UseStim.
- **cultist: thin (alive as person, dead as cultist).** `_23650` has one sermon (`d22 22:00` Service, 1c), then `d25 00:05` GangLeave plus "GrudgeFormed: Ottilie Reyes swore against Gideon Ramirez (a creed left, 0.80)"; nobody acts on it by d27. `_57082` (Squat) is a 1-day Fare Evasion cell then a Scavenge loop; `d26 23:23` he throws a drunk out of a bar door.
- **scav: thin (dead).** The trade now acts: 43 Scavenge in `_31438`, 34 in `_63960`. It pays nothing visible: `_31438` has zero money lines in 7 days. `_63960` is really a Forger who owns Fixer#5410 and earns `d26 09:00` "+80c (Gray)... Served papers": the one alive trade here, and it is not the scav's.
- **dealer: thin (thin).** `_14497` has the story: `d21 10:00` Blackmail (a witness wants 14c, "went for her; Ottilie Singh won and went to the law"), `d21 15:10` second Blackmail, 5c, "Jocasta Sato won and Hesper Lindqvist will keep quiet", `d21 21:22` welcomed-back crew line, `d23 05:43` StealVehicle then Fence. The Deal itself is `d21 00:00` "Deal 7 min". `_51168` Gambles, Drinks, brawls (`d22 11:00` "started a brawl... miserable -0.62 and drunk"), then `d22 21:22` "lost the colours: 3 duties missed, beaten out by Lena Ashcombe".
- **runner: thin (alive/thin).** `_7044` is the arc: `d23 08:00` Skipped, same minute Fire "dismissed as Sanitation (absent 3 times in 14 days)", `d23 08:01` Contract, a 285 min walk to a Fixer, Network 31 min, then `d24 04:24` Pitch and Vend, Scavenge. `_31221` is a Vat Tech who hacks (3 JackIn) and pays nothing visible.
- **raider: thin (new).** Neither raids. `_63269` (Iron Brothers, order Raid, no job) hauls for the Vats by d24. `_85728` (71 y) Begs 10 times, Flirts 5, is jailed four times for Fare Evasion; `d21 20:31` "stood watch at Fight Pit#2585... (unpaid: the gang is broke)".

## 3. The pass's mechanisms seen at 100k

Fired and quoted:
- Crew duty vs day job: `purist_85772 d21 00:07`, `d22 00:07`; `dealer_51168 d21 00:07`, `d22 00:07` (40% the gang's way, 3 in 10 days).
- Skipped for misery and Fire: `dealer_51168 d22 00:07`, `runner_7044 d23 08:00`.
- Colours lost, beaten out: `dealer_51168 d22 21:22`.
- Court quorum failure with names: `gang_leader_20414 d21 21:00`.
- Release welcomed or with a grudge: `dealer_14497 d21 21:22` (welcomed), `gang_leader_20414 d22 15:43`, `cultist_57082 d21 14:42` (grudge on the reporter).
- Day off with the crew: `gang_leader_20414 d21 20:44` names four at the Hideout.
- Service with a plate: `purist_85772 d22 20:00`, `cultist_23650 d22 22:00`.
- Till smashed and a gang answering: `gang_leader_71747 d27 19:39` Punished x2 ("The Glass Teeth will answer The Iron Knives").
- Blackmail with a fight: `dealer_14497 d21 10:00`, `15:10`.

Expected, never seen:
- A Raid that reaches the target. Two Raid plans muster (`purist_25175 d24 18:03`, Muster 130 min at Chapel#5077; `dealer_51168 d21 18:04`, Muster 43 min) and both are cut by Sleep (`d25 03:05`, `d21 19:48`). Zero Brawl actions in the group, zero raid result lines.
- A Collected line (zero in 16 diaries; `gang_leader_20414 d21 15:16` runs Collect and logs no event). Also no Protected event, no gear sink, no loan, no UnderWater.
- A Preach action (0; Service is the only sermon, twice).

## 4. Bugs, ranked by what the player sees first

1. **Scavenge is a no-op loop.** `scav_63960 d21 02:48-06:01`: Scavenge "1 min (ended)" then "PLAN FAILED: ... PreconditionLost", four times in three hours, goal cooled, repeats all week (34 fails there, 17 in `scav_31438`; 52 Scavenge PreconditionLost in the three scav-like diaries). The 60 min Scavenge blocks yield no found, sold or hauled line. V9 "scav dead" is now "scav busy and unpaid". Likely cause: a Street Market bin with nothing in it fails the precondition on the first tick and the plan has no "next bin".
2. **Raid plans are cut by Sleep every time.** Both musters above. Muster is a 43-130 min wait, then the raid leg starts against an energy goal that outbids it (`dealer_51168 d21 19:48`: Sleep 0.615 vs Raid 0.579). The mechanism cannot finish in a normal day. V9 raid outcome: unchanged.
3. **The walk is still the day.** GoTo hours over 7 days: `gang_leader_71747` 71 h (6 of 20 legs over 240 min, a 638 min commute from D4 to a D6 night shift), `purist_25175` 60 h, `dealer_51168` 56 h, `cultist_57082` 55 h. Single days: `cultist_57082 d22` 16.0 h, `gang_leader_20414 d21` 12.9 h, `raider_63269 d23` 13.2 h. V9 bug 1: better for gang_member (`_12322` 47 h vs `_18708` 82 h), unchanged for leaders and raiders.
4. **Fare Evasion jails the poor on a loop.** `raider_85728` arrested d22, d24, d26; `scav_31438` d21, d25, d26, d27, each "1 days for Fare Evasion". It eats a third of their week and every Cells line reads the same. V9 saw it on `gang_leader_101227`; worse now.
5. **Leaders and the homeless sleep on pavements 8-13 h.** `gang_leader_71747 d24 06:47` 783 min, `d25 16:47` 708 min (she has Block#583); `gang_leader_20414 d22 15:44` 497 min, `d24 06:32` 731 min; `cultist_57082 d21 17:22` 697 min. V9 bug 5: unchanged.
6. **Court is a statue when it succeeds.** `gang_leader_20414 d21 19:50` "Court 70 min (ended)" and `d25 20:06` 54 min with no attendee line; the failure at `d21 21:00` comes 70 min later, so one night reads as court held and court failed. A second Court is a 1 min PreconditionLost at a street (`d23 08:34`). V9 bug 6: better (absentees named), success path blank.
7. **Zero ReportCrime.** One GoTo(Hall) in 16 diaries, no ReportCrime action. V9 bug 2: unchanged here, nothing to show for Phase 4 in this group.
8. **No Collected line.** The old "0 to 0 members" line is gone and nothing replaced it, so the weekly tribute cannot be confirmed from a diary. V9 bug 3: better by silence.
9. **Church hopping.** `gang_member_12322` takes up The Drowned Choir at `d26 00:08`, leaves the Cinder Widows at `d27 00:05`, takes up the Open Circuit at `d27 00:08`. Two faiths in 24 h at mood 0.07 reads as a rule firing, not a person. Probably the 00:05 daily pass.
10. **ExpansionCap kills Earn/Work/Bury plans.** 8 cases in 7 diaries (`cultist_23650 d21 20:11`, `d26 18:10`; `dealer_14497 d26 18:02`; `gang_leader_71747 d22 21:54`). V9 bug 10: unchanged.
11. **Friend edges at +1.00:** 169 `affinity +1.00` matches in the group; not traced to the gang tie. V9 bug 8: unchanged, unverified.
12. **Service names Hideout#5077; the same id is Chapel#5077** (`purist_85772 d22 20:00` vs its Where line). Cosmetic.

## 5. Dull spots (mechanism, not a knob)

1. **The raid never lands.** Once Muster completes, the plan holds a commitment (like a shift) until the leg ends or the member is wounded, and the muster is timed to the gang's night so Sleep does not win. Result lines (winner, stolen stock, wounded) go to the victim venue's rumour pool.
2. **The scav round pays.** Each bin has stock that regrows daily, a finder takes a visible item (food, parts, a deck chip) that sells at the market or feeds the gang tithe, and an empty bin moves the scav to the next bin. `_31438` then has a trade, and `StakeOut` and `Strip` a reason.
3. **Grudges that are never acted on.** `cultist_23650 d25 00:05` and `gang_leader_20414 d22 15:43` leave a grudge and nothing happens in 5 days. A creed-leaver or a jail-reporter is hunted once the duty dispute settles (HuntStarted was in V9 and is absent now).
4. **A leader's walk-out has no succession.** `gang_leader_20414` leaves a 42-member gang and no member takes the seat. The highest rank present takes the Hideout, calls court the next night, and a rival makes the rest an offer.
5. **Deals leave nothing.** `dealer_14497 d21 00:00` "Deal 7 min" with no buyer, coin or product. Deal moves stock for coin with a named buyer who then goes to a Club; blackmail then has a source.
6. **Runner without a contract.** `runner_7044` walks 285 min to a Fixer and Networks, then nothing. A Network yields a contract with a payer and a deadline.
7. **Raider with no raid.** Order Raid, 15 members, unpaid (`raider_85728 d21 20:31`). A raid draws a pool from the treasury and pays the watchers; "the gang is broke" is already the right hook.

## 6. Too much

- **Fare Evasion, 8 arrests in two lives.** Monotone, not harsh. A jailed-then-released poor man works the fare off at the station, or learns to dodge the checks.
- **Gang leaving.** Four GangLeave lines in 16 diaries in 7 days (`gang_leader_20414`, `gang_member_12322`, `cultist_23650`, `dealer_51168`), plus mood-driven conversions. A leader of 42 leaving the same week as three others reads as the roster dissolving. Check the 30-day GangLeave count at the next close; I do not have it.
- Nothing else reads broken: one Fire in 16 diaries, two Brawl event lines.
