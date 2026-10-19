# Shadow V5: the crime trades (critic: crime)

Read: V4 `docs/shadow_v4/crime.md`; k20 (seed 42, d20-26): pimp x2, fixer x2, forger x2, gun x2, buyer x2 (plus street_worker Amara Sato, the target of one pimp's rounds); s43 (2k, d26-32): fixer, gun x2, buyer; s44 (2k, d90-96): gun x2, buyer. **No 2k pimp, no 2k forger (V4 had both), no arms dealer, no black doc in any window.** Citations `window/file dDD HH:MM`.

**Short version.** The pimp now has a night: he goes out and Collects from a named worker, and the cut is split with the gang. The owner Fixer visits his office to collect the week. The papers sale gained a walk-in "handed papers" event, but the coins still move at midnight, up to 37 h before the handover. The gun still hunts (StakeOut, PreconditionLost, retry). The buyer's hit now comes back on him: the city posts a locate on him and he is jailed for Conspiracy. Arms dealer and black doc are absent everywhere (OPEN-017, untouched by the milestone).

## Per archetype

**Pimp (k20 only): alive, thin on money.** V4: a title, the cut went to the gang, no worker met. Now both pimps run GangWork nights and `Collect->` a named worker in 61-minute blocks (Zoe Nightingale: 49 Collect actions in 7 days; Agathe Hawthorne: 5 before a 6-day Shakedown sentence). The cut and pass-up are visible:
`k20/pimp_10291 d22 00:00 [money] +5c (Tribute), purse 15c; flows: Tribute +4 from gang The Ash Wolves, StreetCut +2 from Matthias Sato#8944, StreetCut -1 to gang The Ash Wolves`.
Week: StreetCut in 6c, out 3c (half passed up), Tribute +20c. Zoe's night: Collect 00:54-01:55, Collect 01:57-02:58, `served Matthias Sato#8944, a regular`, then sleep on the street. Thin because the workers are broke: Agathe's three nights on Amara Sato (wallet 0-2c) yield no StreetCut row.

**Fixer: thin, one thing alive (the owner's collection).** V4: owners never entered the office. Now Ivo Stroud (owns Fixer#5167) gets a `Visit` goal ("a visit due"), walks 35 min, Rests 91 min in the office:
`k20/fixer_8586 d26 16:23 [event] Served: Ivo Stroud collected the week's cut at Fixer#5167 in person`.
2k: Juniper Park (owns Fixer#531, ClerkWork 467-533 min/day) posts a guard contract on herself (`s43/fixer_979 d27 00:00`, 48c), offers a captain a 50c bribe (refused), takes FixerCut +27c and +38c at midnights and, as foreman, hires a Forger. Tank Reyes (k20, a Vat Tech who owns Fixer#5412) never goes near it. No fixer in any diary meets a broker or a taker.

**Forger (k20 only): thin, better at the edges.** V4: papers sold at midnight to buyers nobody meets. Now there is a daytime handover inside the ClerkWork shift:
`k20/forger_25122 d21 12:03 [event] Served: Alaric Santoro handed Slate Achebe papers at Fixer#5167` (and `13:44` for Greta Adeyemi).
The coins are still `papers_daily`: `d21 00:00 +32c (Gray) from Slate Achebe#9236`, `d20 00:00 Gray +32 from Greta Adeyemi` (bug 1). Around the trade: joins and quits the Iron Knives in 4 days, takes up the Drowned Choir (d25), is a 4c-tipping "regular" of a Spire Lounge staffer (d23 20:23), and is arrested for Forgery twice (d22 report, d26 11:57, 2 days). Aldo Greaves (`forger_25746`) sold nothing; his papers are exposed at an unrelated arrest (d25 05:06).

**Gun: alive, same as V4, now fed from the Fixer board.** Contracts read "posted at Fixer#531". Hunting intact:
`s44/gun_1652 d90 18:44 ACTION StakeOut @ Freight Yard#538 (Vats), 181 min (ended) [target Amara Lindqvist#1991]` then `21:45 PLAN FAILED ... StakeOut: PreconditionLost`, replan, `d91 00:00 ContractFulfilled`.
`s43/gun_1050 d32`: AskAround > StakeOut at Bar#417, three 181-min stake-outs, two PreconditionLost. The 20k pair (Knox Fairweather, Aldo Ramirez) are Vat Techs who only took a tag at 18:00 on the last day. No gang-boss hunt in this sample, so V4's Zelda/Hollis lives are not re-tested.

**Buyer: thin, but with consequences now.** V4: a contract as a line in a memory. Now:
`s43/buyer_1708 d29 00:00 ContractPosted: the city posted a locate on Joss Hallow for 29`, four sold sightings of him (15c each, d30-32), `d32 12:07 Sentence: Joss Hallow sentenced to 23 days for Conspiracy`.
Best crime story in the set. The two 20k buyers (same home Block#5090, same Vat Farm, same target Smoke Oyelaran) are quiet: both beat contracts Expired and the escrow came back (Zed +25c `buyer_23014 d22 00:00`, Hesper +20c `buyer_12445 d20 00:00`); neither re-posts.

**Arms dealer: not present** (k20 log "only 0 distinct arms_dealer candidate(s)"; none on s43, s44). V4 had 5 and no weapon was sold; OPEN-017 (armoury surplus never forms on 0-180c treasuries).

**Black doc: not present** in any window (OPEN-017: `build_clinics` needs 700c).

## Bugs

1. **Papers still paid at midnight, now before the handover.** `k20/forger_25122`: `d21 00:00 +32c (Gray) from Slate Achebe`, then `d21 12:03 handed Slate Achebe papers`; Greta Adeyemi paid `d20 00:00`, handed `d21 13:44` (37 h later). The coin comes from the midnight `papers_daily`; the handover is a separate event. V4's midnight-Gray bug is unfixed and phase 5 decorates it. 1 of 2 forger diaries (the other sold nothing). Full tier.
2. **The pimp's Collect lands on a sleeping worker.** `k20/pimp_13211 d20 04:10-06:14` Collect at street (Mid West) targeting Amara Sato; her diary (`street_worker_16741 d20 00:01-06:00`) is `ACTION Sleep @ Block#2807 (Mid West), 357 min`, at home, yet logs `d20 05:11 served Agathe Hawthorne#13211` and `06:14 ... a regular`. The encounter counts without the worker at the pitch, and no coin moves (Agathe's wallet stays at 7-24c from other sources, Amara at 0-2c). Likely the encounter is created from the `pimp_of` link, not presence. 2 of 2 pimp diaries as far as I could check (Zoe's target Matthias Sato had no diary). Full tier.
3. **Both sides log "served" for one Collect, and the pimp becomes the worker's "regular".** Zoe: `served Matthias Sato#8944, a regular (valence -0.10)` (`pimp_10291 d21 01:55`); Agathe: `served Amara Sato#16741, a regular`; Amara: `served Agathe Hawthorne#13211, a regular`. The pimp is both server and the worker's regular client. A cut collector is not the worker's client (OPEN-110 shape, no coins). 2 of 2 pimp diaries. Full tier.
4. **Cuts settle at 00:00, not at the Collect.** Zoe collects 49 times but StreetCut lands only at midnight ticks (`d22 00:00`, `d24 00:00`, `d25 00:00`), so a night of Collect blocks shows no money. Same family as bug 1: the FixerCut also lands at 00:00 on 2k (`s43/fixer_979 d27 00:00 +27c`, `d29 00:00 +38c`). 3 of 3 owner/pimp diaries. Full tier.
5. **The owner is paid 6 h before he arrives.** `k20/fixer_8586 d26 08:35 +39c (FixerCut)` while asleep at Block#2100; `14:17 PLAN Visit on Fixer#5167`; `16:23 Served: collected the week's cut ... in person`. The "in person" line is narration over a settlement already posted. 1 of 1 collections. Full tier.
6. **Diary label: the school fee reads as "Contract".** `s44/gun_1652 d91 00:00 -2c (Contract) ... Contract -2 to the City` (also d93-d95) right after `ContractFulfilled`; `ages.rs:303` pays the school fee as `Flow::Contract`. A reader sees a gun charged for fulfilling a locate. Not a sim bug (V4 gap 13 family). Also no payout row follows either `ContractFulfilled` I read (Santiago `s43 d26 00:00`, Kuro `s44 d91 00:00`; the 32-38c prices never show as income). Weaker evidence: I did not trace the settlement path, it may pay elsewhere. 2 diaries.
7. **Gun Contract goal flaps at its cooling cap.** `s43/gun_1050 d28`: Contract wins, "could not plan or plan failed twice", cooled 2 h, Idle, repeat (43 such lines in 7 days) until the plan finally runs at `d32 04:32`. 1 of 6 gun diaries. Full tier.

Question, not a bug: the two 20k buyers share a home, job and target within 2 days (one household, two escrows for one grudge?).

## Gate items

- **Pimp's night (phase 4b): settled, with caveats.** Presence and round exist (`k20/pimp_10291`: GangWork goal 21:01 and 00:00, Collect blocks 00:46-02:58 at Mid 7). Cut and `pimp_pass_up`: StreetCut +2 from the worker, -1 to The Ash Wolves (`d22 00:00`, `d24 00:00`); week 6c in, 3c passed up. Caveats: the worker need not be present (bug 2), the cut lands at midnight (bug 4).
- **OPEN-110 (pimp pays as a client of his own pitch?): settled for coins, open for the label.** No flow goes from either pimp to a street worker; Zoe's StreetCut out (3c) goes only "to gang The Ash Wolves", Agathe has no StreetCut row, StreetDice is with bystanders. No circle on one pitch. But the pimp is recorded as the worker's "regular" served client (bug 3): decision for Dylan, should the served-memory skip a pimp collecting his own cut?
- **Fixer contracts and licences (phase 5): bug (weak).** The board works (every taker contract is "posted at Fixer#531/#5167") but the fixer meets no broker or taker in any of the three fixer diaries (Juniper Park: ClerkWork 467-533 min, no encounter lines; Ivo: only his own collection; Tank Reyes: never visits). No licence line appears in any crime diary (Flow::Licence is the civic clerk's, so not mine to find).
- **Fixer owner's weekly collection: settled** (`k20/fixer_8586 d26 16:23`), with bug 5. One sample, 20k.
- **The gun and the hit market: settled, unchanged (still the best of the pass).** StakeOut with PreconditionLost and retry (`s44/gun_1652 d90 18:44-21:46`, `s43/gun_1050 d32 08:01-22:04`), and the hit now bites the buyer (`s43/buyer_1708 d29 00:00`, `d32 12:07`). No gang-boss gun in the sample, so the V4 stand-outs are not re-tested.
- **Black doc and arms dealer: bug, still absent, not milestone work.** 0 candidates in all three windows (`k20_s42_d20.log`: "only 0 distinct arms_dealer candidate(s)", "no black_doc candidates"; same on both 2k seeds). OPEN-017 (V4 bug 16); the milestone changed neither.
