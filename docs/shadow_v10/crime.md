# Shadow V10: the crime trades (critic: crime)

Window: main `33f9c55`, 100k island, seed 42, d21-27. Fourteen diaries: forger 14145 and 90429, fixer 23959 and 74482, gun 28759 and 28930, buyer 59896 and 104449, pimp 96574 (one pick only), arms_dealer 71504, black_doc 6844 and 13839, jailed 19749 and 93738. Cites are `<arch>_<index> dDD HH:MM`. A missing line is not evidence; counts are of these diaries only. d27 is a rest day for most (work 0 h).

## 1. Verdict

Against V9 the group moved from commuters to people with consequences. Walks are now mostly under an hour, crimes get reported, arrested, sentenced and released into a changed life, and one owner's collapse (black_doc_13839) is a full story that other people pick up.
The trades themselves are still thin. Clients are nearly all Statistical, the pimp never collects, the guns never hunt, and the black doc has no patients at all. The story-rich lives are the failing ones (broke owner, jailed, assaulted), not the working ones.
Jail is now the dominant state: 6 of 14 spend 4+ whole days in the Precinct. It reads harsh and consistent, but a 71-year-old getting 10 days for assault is the edge of monotone.

## 2. Per archetype

- **Forger: alive (V9 thin/dead).** `forger_90429` is the best trade chain in the group: `d24 02:32 Report ... for Forgery`, works on, `d24 16:39 Served ... handed Matthias Novak papers` (+40c), `d25 12:46 Arrest`, `d25 14:35 Sentence 2 days for Forgery`, then `d26 00:07 Cells: gave Marek Chrome a name in the cells: ask at Fixer#5897`. `forger_14145` serves two Full clients (`d22 16:56`, `d23 12:47`, +40c each) and hires staff (`d21 00:11 Hire`), but on `d27` takes a 31c cab to a Hideout (`d27 05:27`) instead of working.
- **Fixer: thin (V9 thin).** `fixer_74482` serves 13 times, nearly all Statistical clients on the 09:00 tick (`d21 09:00` three at once), and lays off a forger for revenue (`d26 00:07 LaidOff`). `fixer_23959` lost the shop to a loan shark and works a Vat Farm: `d23 00:07 Loan: missed a payment ... Repossessed: Mako Garland took Fixer#5202`. A real beat, but it ends there.
- **Gun: alive then jailed (V9 thin/dead).** `gun_28759` (71 y, Ninefold) is blackmailed (`d23 20:00 Blackmail: Verity Coldwater ... wants 5c`, she attacks first and wins), stands watch for pay (`d24 20:30 Crew ... paid 3c`), is reported, arrested `d24 21:12` and gets 10 days. `gun_28930` assaults a stranger `d23 16:25`, arrested `d23 22:42`, 10 days. Neither hunts: no Intel, AskAround or StakeOut in either, and `gun_28759` holds a live 1441c hit (#1496, Taken, Live) while brawling with strangers.
- **Buyer: alive (V9 dead).** `buyer_59896` escalates on one target: `d23 00:06` locate 40c plus tag 303c, `d24 00:06 posted a beat on Elspeth Fairweather for 486 at Fixer#5517 (culprit)`. He also brawls at a club door (`d21 20:07`) and gets five news Stories. `buyer_104449` is defrauded (`d26 00:09 learned the job ... sold for 40 never existed`), takes a 0.74 grudge, then does nothing for 15 h (`d27`).
- **Pimp: dead (V9 thin).** `pimp_96574` never collects. The one plan is a 364-min walk to the spot that is cut (`d21 00:00 GoTo(Spot) ... 584 tiles`), then 209 fare-evasion sightings and three arrests (`d21 22:11`, `d23 14:52`, `d24 12:27`, the last one 3 days for Theft), 0c throughout.
- **Arms dealer: alive but one note (not sampled in V9).** `arms_dealer_71504`: `d21 00:00` deal in a bar with 13+ witnesses, `Report`, `d21 01:11 Arrest`, `d21 02:06 6 days for Arms Dealing`, `d27 02:06 Release`, `d27 02:53 Marriage: Kira Duarte married Dante Stroud`, `d27 08:21 Crew: welcomed back ... 6 days served, nothing said inside`. Six dead days in between.
- **Black doc: one dead, one alive.** `black_doc_6844` sits 8.6 h a day in Back Room#874 doing only ClerkWork, Rest and HangOut, no patient, no event but 56 Witness lines. `black_doc_13839` is the story: see section 3.
- **Jailed: alive (new picks).** `jailed_93738`: `d21 00:07 Cells: missed the shift ... (absence 1 in 14 days)`, `d23 00:07 dismissed`, `d23 16:49 Release: came out to a changed life: the post as Vat Tech gone`, `d25 20:39 Starving`. `jailed_19749` cycles fare-evasion arrests into a 3-day Theft sentence (`d23 19:02`) and spends `d27` on 10.8 h of travel.

## 3. The pass's mechanisms seen at 100k

Fired and quoted:
- **Cells (jail consequence):** absences, dismissal, "changed life" (`jailed_93738 d21-d23`). Cell contact referral (`forger_90429 d26 00:07`).
- **Forgery as a crime with a client, a fee and a report** (`forger_90429 d24 02:32 ... d25 14:35`).
- **Owner collapse chain** (`black_doc_13839`): `d21 00:07 UnderWater: cut the hours ... 0c against 5c a day`, `d21 02:46 3 days for Theft`, `d23 00:07 Loan missed (129c owed)`, `d23 14:16 Quit: Bram Whitlock walked out ... unpaid, owed 21c`, `d25 16:00 Confronted: Sabine Thorne ... demanded the 10c in wages owed`, `d26 00:00 Evicted ... (arrears)`, `d26 16:00 Confronted ... Sabine Thorne swung; Dagny Lockridge won`, `d27 12:00` again with Rin Tanaka. Debts get collected by the people owed.
- **Repossession** (`fixer_23959 d23 00:07`), **Blackmail** (`gun_28759 d23 20:00`), **crew duty and release welcome** (`gun_28759 d24 20:30`, `arms_dealer_71504 d27 08:21`), **hit escalation** (`buyer_59896 d23-24`), **Defrauded** (`buyer_104449 d26 00:09`), **Arms Dealing crime** (`arms_dealer_71504 d21`).
- Counters, 30-day run `target/pp100k/analyze.txt` against V9: hits done 15 (V9 8), fixer_cut 68.6 a day (V9 20.6), HitEscalated 10. That run is one build earlier than 5b.

Expected, never seen in these diaries: Skipped, Punished, Service, a Protection line, raid results, court quorum or fold, a beaten-out-for-talking release, Bribe, Refused. Most are gang-side and belong to the gangs critic, but no crime pick touches a protection racket or a shake-down of a fixer shop.

## 4. Bugs, ranked by what the player sees first

1. **JoinGang walks that never arrive (unchanged).** `jailed_93738 d25 00:18 GoTo(Hideout) 300 min, 833 tiles`, then cut `d25 06:18 (327 tiles)`, `d25 23:38 (349 tiles)`; 11 JoinGang goals in 4 days, zero joins. Likely cause: the Hideout is picked across districts and the walk is cut by sleep or hunger, with no step toward a nearer Hideout.
2. **Long walks inside a district (better, not fixed).** `buyer_59896 d21 06:00 GoTo(Seller) 283 tiles, 252 min` and `d21 21:42 GoTo(Home) 244 min` around 6.8 h of meeting; also `d21 17:32 Bar#4372 59 min for 5 tiles`. `black_doc_13839` commutes 292 tiles, 280 min. `pimp_96574 d21 00:00 364 min`. V9's 350-570 min legs are mostly gone for fixers and forgers (3 to 48 min commutes).
3. **Fare-evasion arrest loop (unchanged).** `pimp_96574` three arrests in four days, `jailed_19749` three releases in seven. Release is always onto a D2 street with 0c (`pimp_96574 d24 03:37 Release ... Scavenge`). 209 fare-evasion sightings in one pimp diary is the loop's source.
4. **A mass relationship flip with no event.** `gun_28759 d21 14:46`: 51+ Friends become Enemy at -0.60 in one minute, then she assaults seven strangers in two hours (`d21 15:03-17:03`). No betrayal, occupy result or order line explains it; likely the squat target block's residents all went Enemy. Reads as a glitch, not a person.
5. **Assault sentence is long and flat.** 10 days to a 71-year-old gang member and to a 38-year-old (`gun_28759 d24 23:05`, `gun_28930 d24 02:58`). Both then vanish for the rest of the window, 24.0 h jail on four rows.
6. **Black doc with no patient (`black_doc_6844`).** 8.6 h of ClerkWork every day, no Treated line, net +26c down to 0c. The trade is a desk.
7. **Marriage 47 minutes after release at 02:53 (`arms_dealer_71504 d27`).** A stranger marries a man who just walked out of six days in the cells. Funny, but there is no courtship in the diary.
8. **Corp EXEC sleeps in capsule hotels (`buyer_104449 d23-27`)** with 451c and a home block, 117 min away; no eviction or reason line.

V9 bug status: 1 (commute walks) better; 2 (guard arrests nobody) not sampled; 3 (fare evasion as transport) unchanged; 4 (owner quits unpaid) better, now a visible chain (`black_doc_13839`); 5 (posted hit nobody takes) better, hits done 15 vs 8 and escalation fires; 6 (shops serve nobody in sight) better for forger and fixer, still invisible (Statistical clients) for `fixer_74482`.

## 5. Dull spots, ranked

1. **The pimp's round.** Mechanism: the worker is a Full body at a spot in the pimp's own district at pitch hour, so the Collect is under an hour and a missed one becomes a search or an argument. Still zero collects in seven days.
2. **The gun with a contract and no hunt.** `gun_28759` holds a 1441c hit and brawls strangers. Mechanism: a taken contract plans Intel, StakeOut and has a deadline; a gun who instead brawls loses the contract (`ContractLost`).
3. **The back-room patient.** `black_doc_6844`. Mechanism: a hurt night patient (from the brawls above) walks in; the doc's hour has a line, a fee and a risk.
4. **Six jail days with nothing.** `gun_28930`, `pimp_96574`, `arms_dealer_71504` have four to six rows of 24.0 h jail with only `MetInJail`. Mechanism: a cellmate offers a job, a debt or a fight (the forger's cell referral is the model), and the crew visits.
5. **The buyer defrauded and quiet.** `buyer_104449` has a 0.74 grudge and 451c. Mechanism: the buyer posts a revenge contract on the fraud seller, or is told who sold it at a Fixer counter.
6. **Statistical clients on the 09:00 tick.** `fixer_74482` three served at `d21 09:00`. Mechanism: the booked client walks in during the shift and is Full when the camera is on the shop.

## 6. Too much

- **Fare evasion and jailing.** 209 sightings and three arrests in one pimp diary; `pp100k` has 16,750 arrests in 30 days and mean 890 jailed, and jail fills 6 of 14 diaries for days. Harsh is accepted; the one-day sentence loop for a 1c fare reads as noise and the same face every day.
- **Assault witnesses.** `gun_28759 d21 15:03`: four Witness plus WitnessReacted lines per blow, seven blows in two hours. The diary is unreadable at that rate.
- **Blackmail, Confronted and Theft all landing on one broke owner** (`black_doc_13839`, four Confronted in three days) is dense but coherent; it reads as a story, not noise.
