# Critique: the gang at day 90, seed 42, d90-d96 (Life L2, a737870)

Five diaries from the day-90 window, where the gangs are full: The Hollow (60 members; order VirtRaid, then Squat, then Contest), The Unplugged (54 Purists), Ninefold (18). Day 90 is the Collect weekday, so every Hollow member gets the weekly pay-out in the first hours of the run. Read against `docs/SHADOW_V1.md` and `docs/shadow_v1/critique_gang.md`.

| Diary | Who | Role |
| --- | --- | --- |
| `gang_member_608` | Nikolai Ramirez, 25, Hollow, rank 0 | street dealer at Fight Pit#470 |
| `gang_leader_671` | Freya Bellamy, 39, leader of The Hollow | Collect, Call, Extort |
| `dealer_1953` | Delia Blackwood, 26, Unplugged | dealer at Club#439 (Spire), one Muster |
| `runner_1992` | Kieran Upton, 20, Hollow, hacking 0.70, deck tier 2, courage 0.00 | runner |
| `purist_2303` | Liesl Brandt, 27, Unplugged | five Preach, one Squat, one missed Muster |

## What changed since V1 and L1b

Reads right now, and did not before:

- **Walking fell.** These five walk 9.6-12.8 h a day (V1: 17 h), though that is still far above the 3-6 h of the non-gang rows.
- **Money moves.** Every Hollow member got a 21 c pay-out on d90 (the leader got 22). In V1 extortion took 0 c and income was the dole.
- **The Hideout is used.** Nikolai spends 1,302 minutes there and rests there for most of d95 (14.9 h idle). In V1 it was a bed.
- **Collect is a visible ritual.** Freya walks to two Fight Pits and the Hideout, then logs "collected 1263 to 60 members (the week's tribute and take 2603)". Best single line in the five diaries.
- **A gang can act together.** On d95 the Unplugged order flipped to BreakOut. Delia mustered for 184 min, marched to the Precinct, and brawled with about nine witnesses.
- **Witness spam is gone.** Nikolai logs 14 witnesses over a week of dealing; V1 logged 58 in four hours.
- **Fights and grudges have causes.** Neighbours rob each other, a spouse gossips, a friend's death becomes a grudge.

Still wrong, in order of damage: the walking (M1), the leader's solitude and the empty Call (L1-L3), dealing that sells nothing before dawn (M2, D1), the runner who flees and unwinds in a loop (R1), and a creed that never shows (P1).

## Answers to the open questions

- **Is "paid from gang The Hollow" a bug?** No, it is a flat commission. `systems/stims.rs:395` pays the dealer `dealer_cut = 1` c per dose out of the gang's pocket, after the buyer paid the gang `dose_price` (5 c). The buyer's side shows only in the buyer's diary (Freya `d91 14:18` "Stims -10 to gang The Hollow", Kieran `d90 13:37`, Nikolai himself `d92 15:01`). It reads as a stipend because the dealer's diary never shows who paid. The mechanism is fine; the economics and the presentation are not (M2, M3, M8).
- **Why do dealers sleep 2.6 h?** Stims. `stim_energy = 0.4` per dose for six hours (`assets/config.toml`, `[stims]`), and `needs::decay` restores energy only during Sleep, so each dose is a free half-night. Nikolai logs zero Sleep actions on d93-d96 and his energy sits at 0.6-1.0 (`d93 09:40` UseStim: 0.49 to 0.80). Delia does the same. Consistent and good colour, but it has no cost yet (M5).
- **Does the Hideout feel like home?** It is a room, not a crew. Nikolai rests among five people there and speaks to none (M4).
- **Did anybody mourn?** No (M7).

---

## 1. Gang member: Nikolai Ramirez (The Hollow, dealer)

### A day in the life

- **d90.** Asleep at home in Mid East when the pay-out lands (03:11, +21 c). Wakes 07:16 and walks 51 min to Fight Pit#478 in Sump West to Enjoy (4 c, paid to his own gang). At 09:40 GangWork becomes a plan: Hideout, pick up 10 stims, Fight Pit#470. The Hideout is 251 min away, the Pit another 172. He sells 2 doses at 17:09 to three friends. Hungry, he starts for a market, flips to Sleep and walks 187 min home.
- **d91.** A neighbour, Ulysses Nakamura, shakes him down for 3 c in his sleep (03:26). At 05:00 he beats Ulysses with his spouse watching. 08:01 he steals a meal in Sump East. 15:25 deals 4 h and sells 3 doses.
- **d92.** Deals 4 h, sells 4 doses, runs out, buys 2 back for 10 c. Walks 147 min to the Hideout and rests. Freya walks in at 20:17. No words.
- **d93-d96.** Four pre-dawn Deal shifts sell nothing. Between them he Wanders a Mid West street in 30-minute loops (8.9 h on d93, 9.1 h on d94) and rests at the Hideout most of d95. He logs two grudges for friends' deaths and does nothing about either.
- **Week.** Sleep 20 h, travel 67 h, idle 42 h, deal-shift time 25 h. Net -6 c. Dealing: 9 doses sold for 9 c commission; he paid 20 c for stims himself.

### Findings

**M1. Distance is still the day (distorts the day).** Home is Block#189 in Mid East, the Pit and Hideout are in Sump West, the market is in Mid West. GoTo steps: `d91 11:38` 227 min, `d92 07:54` 179 min, `d92 21:38` 150 min, `d94 08:08` 166 min, `d96 15:10` 171 min. Total 67 h of walking (12 h on d90, d91 and d96). Cause: the travel term exists ("travel 3.25->0.38" in the GangWork line) but it scores a plan; it never moves the home. Hideout, Pit and market are three districts apart. Fix: house gang members in or near the Hideout's district (the distance cap that now applies to jobs also applies to gang membership), and a food source at the Hideout (the gang kitchen `cook_*` already models) so a meal is not a 3 h trip.

**M2. Pre-dawn Deal shifts sell nothing (blocks the role).** Three daytime shifts sold 9 doses (`d90 16:49`, `d91 15:25`, `d92 10:53`). Four shifts that start 00:57-05:37 sold 0 (`d93 05:37`, `d94 03:13`, `d95 00:57`, `d96 02:59`, 241 min each). Cause: `ActionKind::Deal` only registers the dealer at the bar (`stims::start_deal`). A sale happens when some other agent's GetHigh, or the Statistical `stat_buy`, happens to come by, and nobody shops at 03:00. GangWork picks the shift start without looking at foot traffic. Fix: a Deal plan starts only in the venue's busy hours (Fight Pit: around the bout; Club: evening), or the Deal action rolls ambient customers per hour from the district's street density.

**M3. Dealing loses money (distorts the day).** 9 doses sold for 9 c commission; 4 doses bought for 20 c (`d92 15:01`, `d96 18:07`, each "Stims -10 to gang The Hollow"). Cause: stock is free at PickUp (`d90 13:57`, `d93 02:45`, 10 doses each) but there is no consignment ledger, so unsold stock is simply his supply (he used 6 of 20), and when he runs dry he pays full retail to his own gang. Fix: consignment. Stock is owed at `dose_price`, the commission is the margin, unsold stock is returned or charged. A dealer who uses his own product pays for it.

**M4. The Hideout is a room, not a crew (distorts the day).** d95: 14.9 h idle, 885 min at the Hideout in 91-minute `Rest` blocks, five sightings (Raven Coldwater, Anselm Ferreira, Tomas Kaneda, Freya Singh, Zane Bartlett), three new Acquaintance edges, no conversation. His only Socialise plans (`d92 21:38`, `d93 10:08`) are `[GoTo(Market) > Chat->X]`, a 150-minute walk away from the people beside him, and both were preempted. At `d95 19:41` he HangOuts on a street tile with "nobody there" while the Hideout, a few tiles away, holds people. His top contacts are outsiders: Lavinia Osgood 7, the neighbour who robbed him 5, the man who beat him 4. Cause: Chat plans route through a Market; Socialise does not prefer a co-present contact; the Hideout spot gets a bonus only during a Call (see L3). Fix: Socialise tries a contact within a few tiles first and chats in place; the Hideout spot carries a standing positive weight for its own members.

**M5. Four days without Sleep, and no cost (distorts the day).** The table says sleep 0.0 h on d93-d96. Energy stays up because a dose gives +0.4. He is an addict (addiction 0.94-1.00) and the stim is his bed. Missing: the crash. `withdrawal_energy` bites only after 24 h without a dose, so a user who always has stock never collapses. Fix: a dose defers tiredness as a debt, and a long stimmed run ends in a collapse sleep of 10 h or more.

**M6. The order is wallpaper (cosmetic).** VirtRaid on d90, Squat from 14:49, Contest from d92. Whatever it says, he Deals ("following gang order Contest", then PLAN GangWork [PickUp > Deal]). The Hollow's VirtRaid order has one runner and a dead one. Fix: the order decides who is a worker. Dealers stay on the Pit and thugs go; a member who stays reads it as "holding the Pit".

**M7. No heat, no law, no funeral (distorts the day).** A week of public dealing, heat pinned at 0.60, zero guard contact. His friend Yuki Oyelaran is killed (STATS: d92 05:56); `d95 00:00` and `d96 00:00` he logs `GrudgeFormed ... killed a friend` against two people and does nothing about either. A fellow dealer, Juno Ramirez, overdoses on d95 at 16:31 while Nikolai rests at the Hideout. The only corpse he logs is a runner who died on d90, six days later (`d96 00:00 SawCorpse of Volt Santoro`). Cause: grudges feed Fight only when "enemy near"; there is no hunt, wake or burial goal. Fix: a Mourn/Bury goal for gang-mates (a gang spend), and a grudge that sends the gang's fighters after a named target.

**M8. His friends pay for his sales (distorts the day).** The three buyers at `d90 17:09` are Friends and each drops 1.00 to 0.80; same at `d91 15:59`, `d92 11:35`, `d92 12:05`. Customers are recorded as witnesses of Dealing, so the dealer's own crew loses affinity to him for buying from him. Fix: a buyer is not a witness; only bystanders are. Gang-mates get stock from the Hideout, dealers serve outsiders.

**M9. A tool label (cosmetic).** `d93 09:00` "+4c (stims sale)" is the dole, labelled by the action in progress (Deal). STATS counts it, so the "dealing earned" column is inflated by one flow.

### What reads right

- Neighbours who rob, beat and gossip about each other; a spouse (Mags Nakamura) who tells his story seven times.
- The pay-out lands while he sleeps and equals five dole days.
- Witness lines are down to a plausible handful.
- Sales are tiny and thin, as `dealer_cut` says.

**Verdict.** Reads as a man who deals on the side of a life spent walking and wandering. Better than V1 (extortion gone, pay arrives, the Hideout is used), but not yet a Hollow member. Fix M1, M2 and M4 and it starts to be one.

---

## 2. Gang leader: Freya Bellamy (The Hollow, 60 members)

### A day in the life

- **d90.** Collect round from 00:11: Fight Pit#470, Fight Pit#478, the Hideout (45, 36 and 66 min of walking, 11 min at each). 03:11: "collected 1263 to 60 members", her own share 22 c. Then Unwind at a Civic club (129 min), breakfast, a 21-minute Extort at a Mid West Block for 0 c, a 169-minute walk to the Spire to sleep. At 18:41 she starts a 330-minute walk to the Hideout for a "Lead" step.
- **d91.** Arrives in Sump West at 23:57. The Lead plan is dropped at 00:11 and she walks 155 min to a Braindance Parlour. Sleeps in a Capsule Hotel (3 c). Buys 2 stims (10 c) at her own gang's Pit.
- **d92.** Extort in Mid West (302 min away) for 5 c. Rests at the Hideout. At 18:11 she Collects and Calls: "Freya Bellamy called The Hollow to the Hideout". Rests.
- **d93.** Extort refused and fought back (10:26). HangOuts in a Spire street (an Enemy, then nobody).
- **d94.** Six Extort plans, none completed. A stim run to the Spire Club fails (BuyStims "PreconditionLost", no dealer). At 22:54 an Unplugged enemy attacks her.
- **d95.** Extorts a Block in Sump Central for 5 c ("took it from The Unplugged"). Capsule Hotel.
- **d96.** Extort 0 c, sets off for the Spire Club for stims, eats instead, sleeps, HangOuts with nobody.
- **Week.** Travel 90 h (53% of the week), sleep 39.5 h, leisure 16.6 h, "work" 0.7 h. Net +4 c. Purse 4 c; treasury 1,272 c.

### Findings

**L1. She walks 330 minutes to the Hideout and drops the Lead on the doorstep (blocks the role).** `d90 18:41` PLAN Lead [GoTo(Seller) > Collect->Hideout] (a Call was due: the order changed at 14:49 and it was Evening). `d90 23:57` she reaches Sump West. `d91 00:11` "plan interrupted: Lead -> Unwind". Cause: `leisure::call_due` requires `DayPhase::Evening`; she arrives at night, so Lead is "satisfied" and Unwind (0.455) takes over. The Lead plan was never committed. Fix: once started, a Lead plan holds until its Collect step runs or fails, `call_due` stays true for the plan's life, and a leader far from the Hideout Calls on arrival or the order change waits for her.

**L2. She has no retinue, no escort, and not much fear around her (blocks the role).** Sixty members and she walks alone for 90 hours. Her Extorts are a thug's: 5 c, 0 c, refused and beaten. She lives in the Spire, 5.5 h from the Hideout, and owns a Motorcycle (T1) she never uses. Dread falls 0.64 to 0.31 while the two papers print "Zed Greaves assaulted Freya Bellamy" (d90) and "Gwendolyn Brandt assaulted Freya Bellamy" (d95), and over a hundred gossip lines are about her being hit. At `d95 14:18` her Extort squeezes Lena Varley, her own dealer (#673): "Lena Varley is now Enemy (was Friend)". Cause: a leader's GangWork is the member menu; Extort picks any Block in reach, including a crew member's. Fix: leader GangWork is delegation (Lead steps that send two or three named members and a retinue that travels with her by vehicle), and the Extort target filter skips gang-mates' homes.

**L3. Collect pays; Call does nothing visible (distorts the day).** `leisure::payout` credits every member wherever they stand, in equal shares, so there is no ceremony. Nikolai and Kieran both receive it asleep. The `d92 18:22` Call sets `call_until` and adds `call_w = 2.0` to one Unwind spot score, and only within `call_tiles = 120` of the Hideout. Nikolai reaches the Hideout at 19:47 for his own reasons and talks to no one; nobody turns up for a speech and nobody is told anything. Fix: a Call pulls loyal members to the Hideout as a plan (a Muster-lite with a short Briefing step where the leader announces the order), and the pay-out is handed over there, so those present collect in person and the absent get theirs later, docked.

**L4. The leader is paid like a rookie (distorts the day).** 22 c against 21 c; she buys 12 c meals from a purse of 5-30 c while the treasury holds 1,272 c after the pay-out. `payout` splits `tribute_share x week` equally; rank only orders the rounding remainder. Fix: a leader's cut off the top, a lieutenant tier, and a treasury that is spent (stash, bribes, a vehicle, bodyguards). A treasury nobody touches reads as a counter.

**L5. Sleep is solved; travel is not (distorts the day).** She sleeps at a Capsule Hotel when it is nearest (`d91 05:00`, `d95 19:03`), which is the V1 sleep-at-the-nearest-bed fix working. But Home is in the Spire and everything else is 3-5 h away: `d92 00:11` 302 min, `d93 05:50` 255 min, `d95 09:23` 274 min, `d96 01:23` 260 min. Fix: as M1; a leader lives at or beside the Hideout.

**L6. Stims from a rival's club (cosmetic).** `d94 08:36` BuyStims at Club#439 fails after a 115-minute walk. Club#439 is an Unplugged dealer's site. Fix: GetHigh's stim source prefers an own-gang Pit or the Hideout and treats a rival's dealer as a non-source.

### What reads right

- Collect is a real, readable ritual with real sums and the fronts as stops.
- The Call event exists and shows in Notable events.
- Hotels and Hideout Rest instead of 12-hour walks home to sleep.
- A leader hunted by other gangs, in the papers: right tone.

**Verdict.** Freya collects but does not lead. She does a thug's work, alone, at a thug's pay, and her one leader's act can be walked away from on the doorstep. Needs a committed Lead, a retinue, an audience for the Call and a leader's cut.

---

## 3. Dealer: Delia Blackwood (The Unplugged, Club#439 in the Spire)

### A day in the life

- **d90.** Pay-out of 3 c at 00:29 (169 c split among 50). At 18:33 she sets off on a 281-minute walk to Club#439. Deals 23:14-03:15, sells nothing.
- **d91.** HangOuts, then walks home to sleep, is dragged off by hunger mid-walk and logs 0 h of sleep. Wanders the Mid West street for 12.9 h (17 Wander actions in a row).
- **d92.** Another pre-dawn shift (02:23), another 12.8 h idle. No sales.
- **d93.** A third shift (01:40). Steals a meal at 07:11 and is reported. Fights Wren Tanaka at 15:33, loses, flees.
- **d94.** Walks 5.8 h to the Chapel for stims, 154 min toward the Club, is pulled back by Unwind (195-minute walk home). 16.6 h of travel.
- **d95.** 17:19 the order is BreakOut, 17:33 Raid. Musters 184 min in the Spire, marches 111 min, brawls at the Precinct at 23:36, flees.
- **d96.** 250 min to the Club, deals 166 min, 99 min back.
- **Week.** Sleep 15.5 h, travel 74 h, idle 33 h, Deal 19 h. Net +1 c. Sales: at most two.

### Findings

**D1. A dealer's shift is a registration and nobody shops (blocks the role).** Five Deal shifts (`d90 23:14`, `d92 02:23`, `d93 01:40`, `d95 12:44`, `d96 17:17`). The three pre-dawn ones show no sale. The d95 afternoon shift is probably the `+1 Stims` credited at `d96 00:00`; the d96 evening shift shows none. Same cause as M2. Fix: the shop-hours fix, and club dealers work the evening.

**D2. A dealer who uses up her stock (distorts the day).** She starts with 9 doses and uses all 9 herself in four days (`d90 18:19` through `d93 15:50`) with craving at 0.10-0.15, because GetHigh scores 0.10-0.19 on "dose or source 1.00" and mood. Holding stock counts as having a source. Fix: consigned stock (M3), and a non-hooked dealer does not dose from stock more than once a day.

**D3. Sleep is a long walk that never reaches a bed (distorts the day).** `d91 04:23` "Sleep [FleeToHome > Sleep]" 160 min, interrupted at 07:03 by Eat, day logs 0 h. `d96 03:45` Sleep [GoTo(Hideout) > Sleep] from the Club to the Chapel, interrupted by Eat at 06:03. Cause: home (Mid West), job (Spire) and Hideout (Sump Central) are three districts apart, Sleep targets Home or the Hideout, and Eat preempts it. Fix: sleep at the nearest usable bed (a Spire capsule hotel, the Club's staff room).

**D4. Idle is 12 hours of 30-minute Wanders (distorts the day).** `d91 09:25-23:42` is 17 consecutive "Idle [Wander]"; d92 repeats it. Nothing else scores above 0.05. Fix: when Idle wins three plans running, send her to the Hideout (Rest, Chat) or a venue. A street is not a place to stand for twelve hours.

**D5. The Muster is the best event in the week, and then it dissolves (distorts the day).** `d95 17:33` Raid [GoTo(MusterPoint) > Muster > GoTo(RaidTarget) > Brawl]: Muster 184 min, 111-minute march, `d95 23:36` two assaults at the Precinct with about nine witnesses, `d95 23:51` "Brawl 1 min", `d95 23:52` "PLAN FAILED: PreconditionLost", `d95 23:53` Flee. Others in the fight (Jocasta Mercer, Yorick Kaneda) appear to be fellow raiders. What is missing is an outcome: was the prisoner freed? The order flips to Expand at 23:36 as if it had been. Fix: a Brawl with a result (rescue succeeded, rescue failed, someone fell) and a diary line saying who came out.

**D6. Hated by half the city and always in the open (distorts the day).** Dread 0.90, known by 63, an Enemy of Freya Bellamy, beaten by Benedikt Moreau at `d90 15:06`. Nothing changes her habits: 19 street HangOuts, mostly the same Mid West street. Fix: as R1, a HangOut with a hostile present is penalised, and a venue of a recent assault is avoided for a day.

### What reads right

- The Muster and the march are exactly what a gang does with an order: a purpose, a place, a crowd.
- 1 c on a 5 c dose is a believable commission.
- A theft seen and reported (`d93 07:11`) is the right chain.

**Verdict.** Not yet a business. A dealer's week should be stock from the Hideout, a shift when the buyers are out, a few sales a night, a cut, a count. Hers is walking to a Club to stand in it, then wandering a street for twelve hours. The Muster saves it from being empty.

---

## 4. Runner: Kieran Upton (The Hollow, hacking 0.70, deck tier 2)

### A day in the life

- **d90.** 03:11 +21 c. Walks 222 min to extort in Sump West, is refused and fought back, buys 2 stim doses (10 c) at a Hollow Pit, scavenges about 7 h across the day.
- **d91.** HangOut in Mid West at 11:31; an enemy attacks at 11:54. From 12:12 to 18:12 he Flees home and Unwinds back to the same street six times.
- **d92.** Extort in Mid East (235-minute walk, 0 c). Assaulted at 19:02, flees.
- **d93.** Walks 194 min to the Spire, extorts 5 c at 09:35. At 09:58 "GOAL Hack". 84 min to Bar#434, jacks in at 11:22 (3 c terminal fee), "a ledger on the Treasury". The Treasury's ICE fries him at 11:45. At 12:47 he sleeps 8 h on the Spire street.
- **d94.** Beaten at 10:40, then seven more Flee/Unwind cycles by 19:12.
- **d95.** Beaten at 10:26, five more cycles. Sleeps 11.4 h.
- **d96.** Extort at 09:12 (5 c). Sees Delia Hawthorne murder Edric Whitlock at 09:37, strips the body (10 c), is beaten by the killer at 10:07, flees, sleeps.
- **Week.** Beaten 6 times, extorted 3 times (5 c, 5 c, 0 c), one run. Net +27 c. Sleep 56 h, travel 80 h, work 0.

### Findings

**R1. The Flee / Unwind loop (blocks the role).** Courage 0.00 plus an enemy on his street gives Flee about 1.15. FleeToHome runs 30 minutes and ends; Unwind (0.17-0.26) picks a HangOut at the same Mid West street, where the same enemies stand; Flee fires again. `d91 12:12-18:12` (6 flees), `d94 10:42-19:12` (7), `d95 10:27-16:12` (5): 18 loops in three days. This is V1's finding 14 ("Flee ends where it started") unchanged. Cause: FleeToHome is chunked and replanned, the spot score adds `affinity x presence` and does not exclude an Enemy-held spot, and no avoid memory is written. Fix: Flee ends only when the hostile has been out of sight for 30 minutes and writes an avoid-spot memory; spot scoring subtracts a strong constant per Enemy present.

**R2. A runner by tool, not by errand (blocks the role).** The only run is his own impulse: `d93 09:58 GOAL Hack (was GangWork): can hack, U(EV), 1-lawfulness, greed, 1-p_fry x (1-courage)`. Nobody sent him. The Hack goal was cooled from before the window until d93 08:41, so the VirtRaid day (d90) produced no run, and the run came when Contest was the order. The target is the city's Treasury ledger. There is no `SellData` in the whole week. Fix: Hack gets a client: the Hollow's VirtRaid order, a fixer contract, or a buyer on the Virt. A run begins with an order and ends in a sale or a failed contract.

**R3. A fried runner goes to sleep in the street (distorts the day).** `d93 11:45` Fried, `d93 12:17` Sleep 0.92, `d93 12:47` "Sleep @ street (Spire), 481 min". No injury, no loss of deck or sanity, no ripperdoc. The only consequence is `Hack cooled until d103`. Fix: Fried damages something (deck tier, sanity, a medical bill that sends him to a Clinic) and he sleeps in a bed.

**R4. The runner extorts for a living (distorts the day).** Four of seven days run an Extort GangWork (`d90 10:39`, `d92 13:49`, `d93 09:35`, `d96 09:12`), each after a 3-4 hour walk. Courage 0.00, intimidation 0.03, and the first one gets him fought back (`d90 11:00`). Cause: GangWork is one menu, and a runner without a run takes what is on it. Fix: runner-specific GangWork: with no run, work the terminal (probe a node, sell a lead) or wait at the Hideout.

**R5. Six beatings and he never leaves the corner (distorts the day).** Lena Lindqvist, Ghost Singh, Clemence Ito, Gwendolyn Brandt, Kira Ironside and Delia Hawthorne all hit him around the same Mid West street. Each HangOut lists three to five known Enemies present (`d91 22:21`, `d94 18:56`, `d95 08:21`). Same cause and fix as R1.

**R6. Looting is a good chain (reads right).** `d96 09:37` he sees a murder, strips the body five minutes later (`Stripped ... 10 coins`), and the killer beats him ten minutes after. A cause, a witness, a profit and a consequence.

### What reads right

- The ICE exists, bites, and the fry is logged.
- R6.
- He sleeps 56 h, the first Hollow diary where the body gets a normal night.

**Verdict.** A kid with a deck and no reason to use it. One impulsive run, no client, no pay, and a week lost in a street he cannot leave. The Flee loop is the worst thing in the five diaries.

---

## 5. Purist: Liesl Brandt (The Unplugged)

### A day in the life

- **d90.** Pay-out 3 c. At 06:16 she walks 217 min toward a derelict, Block#261 in Sump Central, Occupies it at 11:28 (the occupants fight back; she loses), then walks 258 min home. "The Unplugged took derelict Block#261 as a squat" and nobody stays.
- **d91.** 06:47 Preach on a Mid West street, 31 min. At 07:18 "preached to Jocasta Marlow" (Persuade, p=0.35, success).
- **d92.** Flee / GangWork loop in the small hours (four times). 09:51 Preach in Mid East after a 208-minute walk; "preached to Jinx Oakes" (p=0.42).
- **d93.** Two long walks toward Preach spots (240 and 300 min), both abandoned. Sleeps in the Chapel hideout in Sump Central.
- **d94.** Beats Joss Nakamura before ten witnesses, flees, scavenges 3 c, a 4-minute Preach in the evening, then 6 hours of Flee / Unwind.
- **d95.** 11:06 Preach, fails on Odessa Wexford (p=0.63). 16:53 sets off to loot the body of a Hollow dealer who just overdosed. 17:23 the order is BreakOut and she sets out for the Muster: 291 min.
- **d96.** 247 min to the Spire for the day's Preach (15:30), HangOuts, assaults Lavinia Osgood at 23:08 (witnessed, reported).
- **Week.** Sleep 46 h, travel 79 h, leisure 26 h, work 0. Net +7 c. Five Preach (two successes), one Squat, one missed Muster.

### Findings

**P1. Nothing about her is a Purist (blocks the role).** No chrome anywhere. She is never shown reacting to a chromed person, a clinic or an implant. Her Preach is GangWork with the same considerations as any thug (`U(wealth)`, `greed 0.79->0.86`, `order`) and wins on greed. The speech has no content ("Liesl Brandt preached to Jocasta Marlow"), the stake is "Join the gang", and the listener is the non-gang co-hanger of highest affinity. Cause: `leisure::preacher` is a gate (`is_purist`, `order == Expand`, `id.index % 100 < preach_share`) on an otherwise generic Persuade. Fix: give Purists motives. Preach scores on creed and loyalty and reads the listener's chrome (a chromed listener is a heckler); the Preached event says what was said; add a Purist goal that targets a chrome shop or clinic.

**P2. A 4-5 hour commute for a 31-minute sermon (distorts the day).** `d92 06:23` 208 min, `d93 00:23` 240 min, `d93 06:23` 300 min, `d96 11:23` 247 min. Both d93 walks were abandoned (`d93 04:23`, `d93 11:23`), so no Preach happened on d93. Cause: `busiest_spot` takes the busiest spot of a held district, and she lives in Mid West. Fix: preach where the crowd already is unless a better spot is within a 30-minute walk.

**P3. Her gang's leader is her Enemy (distorts the day).** Lux Fairweather sits at -0.56 on her list; the `d90 00:00` grudge is "Assaulted", weight 0.39. At `d92 10:23` and `d94 03:42` they share a HangOut tile and exchange nothing. A leader who beats a member can be plausible, but the diary never says why and nothing settles it. Fix: a discipline event that names itself ("Lux Fairweather disciplined Liesl Brandt for ...") and a reconciliation or defection path.

**P4. The Muster is missed by 14 minutes after a 5-hour walk (distorts the day).** `d95 17:23` GoTo(MusterPoint) 291 min; `d95 22:14` Muster; `d95 22:15` "PLAN FAILED: Muster: PreconditionLost". Delia left the same point at 22:00. The window (184 min for Delia, who was close) is shorter than the commute of a member in Mid West. Liesl then walks home (337 min) after the order has flipped. Fix: a Muster window that scales with the farthest called member's walk, or a call that goes by distance; a missed Muster says who left.

**P5. Believer's loops (distorts the day).** `d92 00:19-03:18` Flee, GangWork, Flee, GangWork, Flee, GangWork in 30-minute steps; `d94 19:23-23:23` Unwind/Flee six times. Same chunked-Flee cause as R1.

**P6. The Chapel is where the believers are (reads right, barely).** `d93 11:23` Sleep [GoTo(Hideout) > Sleep]; she sleeps in the Chapel 13:26-19:47 and meets three people there (Cato Stroud, Jasper Marlow, Beatrix Moreau) who become Acquaintances. It is the one place she is among her own, and she never goes back.

### What reads right

- A real Squat attempt with a real fight at the door (`d90 11:28`; she "Lost").
- A Preach that does something to the listener (`Persuaded`, +0.05 to the leader's edge) and a failure that is honest (`d95 11:37`).
- Going to the Hideout to sleep when exhausted.

**Verdict.** A gang-aligned street fighter with a habit of walking across the city to speak to strangers. The creed is a label and a gate, not a motive. Not a believer yet.

---

## Cross-cutting causes, in the order to fix them

1. **Distance.** Every diary spends 67-90 h walking because home, Hideout, work and market sit in different districts (M1, L5, D3, P2). Housing near the Hideout, a kitchen at the Hideout, nearest-bed sleep, and a travel gate on GangWork targets, not only a score term.
2. **Chunked Flee and spot choice.** One fix serves R1, R5, P5 and D6: Flee persists and writes an avoid memory, and spot scoring penalises present Enemies.
3. **Dealing demand.** A Deal shift is a registration (M2, D1). Shifts at busy hours, ambient customers, consigned stock, buyers who are not witnesses (M3, M8).
4. **The leader's role.** Lead is interruptible, Call is a nudge, the pay-out has no audience, the leader has no cut and no escort (L1-L4).
5. **Roles without motives.** A runner without a client (R2), a Purist without chrome (P1). The hooks exist (VirtRaid, Preach); the content does not.
6. **The dead.** Corpses lie for days and grudges send nobody (M7).

## Top 8 changes, ranked

1. Flee persists with an avoid memory; spot scoring penalises present Enemies (R1, R5, P5, D6).
2. Deal shifts at busy hours, with ambient customers (M2, D1).
3. Gang housing near the Hideout, a Hideout kitchen, sleep at the nearest bed (M1, L5, D3).
4. Lead is committed; Call becomes a Muster-lite with a Briefing; the pay-out is handed over at the Collect (L1, L3).
5. A leader's cut, a retinue, delegated GangWork, and an Extort filter that skips the gang's own homes (L2, L4).
6. Dealer consignment and a ledger; buyers are not witnesses (M3, M8).
7. Motives for the Purist and the runner: chrome in Preach, a client and a sale for Hack (P1, R2).
8. Mourning and revenge goals for gang deaths (M7).
