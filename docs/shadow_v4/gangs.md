# Shadow V4: the gangs

Archetypes: gang member, gang leader, purist, cultist, scav, dealer, runner. The 2k findings come only from the Full-LOD regenerations, `k2_s43_d26_full` (12 picks, no runner) and `k2_s44_d90_full` (14 picks). The 20k findings come from `k20_s42_d20` (14 picks), which was unaffected. Citations take the form `run/file dN HH:MM`. Read against `docs/history/SHADOW_V3.md` and `docs/history/shadow_v3/{street_and_the_old,gray_and_faith}.md`.

Not flagged, because they are standing decisions: the harsh and shrinking city, no safety net, uncapped faith, placeholder names. Also not flagged: wounded in care who go unfed when nobody can pay. That is by design (`wounded.rs::care_daily`).

## The headline: in 2k, a gang life at Full LOD is a cell

Picks with 2 or more of their 7 days in jail:

- `k2_s43_d26_full`: **8 of 12**.
- `k2_s44_d90_full`: **7 of 14**.
- `k20_s42_d20`: **3 of 14**.

In the earlier mixed-LOD 2k run, where many pins sat at Coarse, the share was lower. The 2k arrests come fast and cost a lot:

- A 0-coin shakedown is reported and becomes a 6-day sentence the same afternoon (`k2_s43_d26_full/scav_2236 d26 09:26` to `15:18`).
- A 5-coin shakedown is reported within the minute and becomes 6 days (`k2_s44_d90_full/scav_2025 d90 11:12` to `16:28`).
- A dealer is in a cell 17 minutes into the run (`k2_s43_d26_full/dealer_1489 d26 00:17`).

The sentence mix across the 2k diaries: 15 × Assault 10 days, 13 × Fare Evasion 1, 7 × Shakedown 6, 6 × Grand Theft 6, 6 × Dealing 5.

A week of street crime in 2k ends inside at about twice the 20k rate. It is also higher at Full than in the Coarse-mixed run. That is a parity question, not harshness (gap 1).

## Gang member

Picks:

- **Lorcan Brandt**, 30, Ninefold dealer, married (`k2_s43_d26_full/gang_member_844`).
- **Freya Northcott**, 48, homeless Dreg, Ninefold (`k2_s43_d26_full/gang_member_947`).
- **Ulric Moreau**, 35, homeless Dreg, The Hollow (`k2_s44_d90_full/gang_member_2188`).
- **Juniper Kowalczyk**, 57, Ninefold (`k2_s44_d90_full/gang_member_621`).
- **Zara Volkov**, 27, Sanitation worker, The Unplugged 4 (`k20_s42_d20/gang_member_6285`).
- **Knox Varley**, 28, The Glass Teeth (`k20_s42_d20/gang_member_9830`).

The week:

- **Lorcan** beats a man (d26 07:19) and is beaten at Bar#417 (d27 18:41). He is put down bleeding by Rue Voss (d27 19:30). Fifty minutes later Delia Bellamy, a purist bartender who holds a beating grudge against him, takes a stim and finishes him on the floor (d27 20:19). His mother and his wife swear kin grudges against her. See gap 4.
- **Freya** squats (d26 01:36) and runs five shakedowns for 0, 5, 0, 3 and 5c. She is beaten four times between 01:41 and 07:14 on d30, collects five beating and robbery grudges, and strips a corpse for 13c (d32 17:01). She sleeps 4.9 h and spends 8.6 h a day walking.
- **Ulric** squats, scavenges for 9c, and picks four fights in 27 hours with enemies on his own corner (d90 08:02 to d91 11:13), and a fifth on d94. Each fight wins on "enemy near" over Unwind. It is a feud machine with no cost to him.
- **Juniper** is avenged on (d90 12:52) and does 5 days for Dealing (d91).
- **Zara** (20k) converts to the Church, steals 2 food and walks out of the gang (d21).
- **Knox** (20k) is murdered at 06:28 on day 20.

**Verdict: alive as street feud and hustle, thin as a gang.** The violence chains read like people, Lorcan's death above all. The gang's business is squats, small shakedowns and arrests.

## Gang leader

Picks:

- **Hana Mbeki**, 71, The Drowned Choir (`k2_s43_d26_full/gang_leader_1455`).
- **Kieran Achebe**, 27, The Unplugged (`k2_s43_d26_full/gang_leader_1594`).
- **Liesl Volkov**, 52, homeless Dreg, Ninefold, 47 members (`k2_s44_d90_full/gang_leader_1757`).
- **Undine Ambrose**, 45, The Hollow (`k2_s44_d90_full/gang_leader_823`).
- **Ursula Lockridge**, 18, The Unplugged 5, 4 members (`k20_s42_d20/gang_leader_10287`).
- **Zed Underhill**, 34, Builder and Militech super, The Static Rats (`k20_s42_d20/gang_leader_21120`).
- Also read: Vivienne Bartlett, 31, who leads The Drowned Choir on day 90 (`k2_s44_d90_full/cultist_1233`).

The week:

- **Hana** leads a raid on Vatra's market herself and loses, 2 against 11 (d26 23:17). She goes down bleeding, and gang-mate Bram Haas carries her to a ripperdoc in time (d27 00:42). She starves there, is left maimed (d30 00:42), and is jailed for 10 days for the raid's assault the same day.
- **Kieran** collects "0 to 0 members" on a take of 40 (d27 06:15). Two corps tag him. A Fare Evasion arrest hands the gang to Ursula Blackwood for a night (d29 23:44). On release he goes hunting (d30 09:45).
- **Liesl** parleys peace from The Unplugged for 30 a week and collects 0 to 0 on a take of 229 (d90). Her gang offers a captain 62c and is refused (d91). She jacks in herself from the Hideout, Arasaka's ICE traces her (d91 07:21-07:37), and she gets 5 days for Data Theft (d92).
- **Undine** takes a Vat Tech job (d90), avenges herself on Ursula Ito (also a pick, d91 01:39), loses her bike twice, and does 6 days for a shakedown (d94).
- **Vivienne** steals a bike, avenges herself, and leans on a witness until the report is withdrawn (d91 00:00). She coerces a beating onto an apostate, pays 202c to 48 members out of a take of 405 (d91 03:02), hunts the apostate and beats him (d92 22:46). Then she takes an office job (d94) and does 6 days for a shakedown (d96).
- **Ursula** (20k) raids The Neon Dogs, 1 against 5 (d26 20:26). **Zed** (20k) walks his super's round and does 2 days for forgery.

**Verdict: alive as drama, not alive as a boss.** Seven of the eight people running a gang this week are jailed, downed or dead, almost always for a crime they committed in person. They are poor (Hana 7c, Kieran 14c, Ursula 2c, Vivienne 1c; Liesl 67c while sleeping rough). Vivienne's coercion and payout are the best "boss" material in the set.

## Purist (The Unplugged)

Picks:

- **Delia Bellamy**, 46, Bartender (`k2_s43_d26_full/purist_801`).
- **Bertram Brandt**, 40 (`k2_s43_d26_full/purist_1610`).
- **Sprocket Pembroke**, 59 (`k2_s44_d90_full/purist_1790`).
- **Ursula Ito**, 28 (`k2_s44_d90_full/purist_2015`).
- **Lavinia Sato**, 32, Loader (`k20_s42_d20/purist_21433`).
- **Ronin Northcott**, 40, homeless (`k20_s42_d20/purist_8820`).

The week:

- **Delia** preaches twice (d26 19:15, d27 14:19), then commits the murder above and gets 30 days, with 22 grudges sworn against her.
- **Bertram** is evicted, hired, married and out of the gang within two days (d26-d28). He then works 51 h and lives with 26 contacts.
- **Sprocket** and **Ursula** (s44) loop between the Chapel Hideout, the corner and the squat: 14-15 h a day of idle and HangOut, 2.4-4.6 h of sleep. Ursula is beaten three times in the week (d91 01:39, d96 18:14, 18:25).
- **Lavinia** and **Ronin** (20k) get jobs, a home and a marriage, and Lavinia walks away from the gang.

**Verdict: thin.** The creed shows only as Preach. One real thing: purists leave the gang when they get a job or a marriage, 3 of 6 within days.

## Cultist (The Drowned Choir)

Picks:

- **Yorick Hallow**, 30, Croupier (`k2_s43_d26_full/cultist_1058`).
- **Bram Haas**, 63 (`k2_s43_d26_full/cultist_1560`).
- **Vivienne Bartlett**, 31, leader (above).
- **Mateo Bartlett**, 63 (`k2_s44_d90_full/cultist_1320`).
- **Bram Coldwater**, 66, leads Drowned Choir 3 (`k20_s42_d20/cultist_17446`).
- **Nikolai Kemp**, 36 (`k20_s42_d20/cultist_17874`).

The week:

- **Yorick** abducts someone and gets 20 days (d27 14:59-17:56).
- **Bram Haas** rescues his leader (d27 00:42), is hired as a Croupier (d28) and does 5 days for Dealing that afternoon (d28 15:06).
- **Mateo** shakes down the same Block twice for 5c and does 6 days (d91).
- **Bram Coldwater** (20k) is diagnosed with cancer, marches 513 minutes alone, raids an Arasaka Security Office 1 against 6 and bleeds out (d25 02:06-13:55).
- **Nikolai** (20k) leaves; the cult posts a revenge beating on him (d23 20:17).

**Verdict: a gang with loyalty and apostate hunts, no faith.** The rescue and the hunts read true. There are 0 Preach actions across the 6 diaries (V3 gap 9 unchanged); tithes are a ledger line.

## Scav (organ-typed gangs)

Picks:

- **Kira Kimura**, 68, The Hollow (`k2_s43_d26_full/scav_1517`).
- **Nikolai Park**, 47 (`k2_s43_d26_full/scav_2236`).
- **Ada Hallow**, 23 (`k2_s44_d90_full/scav_2025`).
- **Freya Park**, 48, Night Stocker (`k2_s44_d90_full/scav_2203`).
- **Bronwen Osgood**, 29, Vat Tech (`k20_s42_d20/scav_15805`).
- **Piers Ambrose**, 70, Drowned Choir 8 (`k20_s42_d20/scav_15850`).

The week:

- **Kira** does 5 days for Dealing (d26 11:27) and comes out as acting leader while Raven Yates sits inside (d32 18:00).
- **Nikolai** gets 6 days for a 0-coin shakedown, **Ada** 6 days for 5c, and **Freya** 6 days for a stolen bike (d91).
- **Bronwen** (20k) leaves her gang and steals a bike.
- **Piers** (20k) does 6 days for 5c and makes the news 11 times (gap 13).

**Verdict: absent as scavs.** Nobody in the six Full picks harvests a body in the week. "Scav" is the gang's type, not the member's life. V3 gap 6 (the killer stays poor) cannot be judged from this pass.

## Dealer

Picks:

- **Priya Stroud** (`k2_s43_d26_full/dealer_1489`).
- **Ines Northcott** (`k2_s43_d26_full/dealer_2195`).
- **Rook Blackwood** (`k2_s44_d90_full/dealer_1170`).
- **Anselm Blackwood**, Vat Tech (`k2_s44_d90_full/dealer_952`).
- **Hale Jarrow** (`k20_s42_d20/dealer_14956`).
- **Bex Achebe** (`k20_s42_d20/dealer_16807`).

The week:

- **Priya** is jailed at 00:17 on day 26, released on d31, fined for sleeping rough that night and hired as a Bartender on d32.
- **Ines** is hired as a Bartender, quits when unpaid (d29) and does 5 days for Dealing (d30).
- **Rook** spends 152 h of the week inside for an earlier Assault.
- **Anselm** works his vats, beats a man (d90) and does 10 days (d95).
- **Hale** (20k) survives a hit and does 5 days for Dealing.
- **Bex** (20k) runs five shakedowns for 0c and falls behind on rent twice.

**Verdict: not alive.** Five of the six spend 1.5 days or more inside. Sales run 0-2 a week, worth 0-3c.

## Runner

Picks:

- **Maren Park**, 31, Precinct Guard (`k2_s44_d90_full/runner_2453`).
- **Silas Wozniak**, 57, Researcher (`k2_s44_d90_full/runner_956`).
- **Zara Sallow**, 41, Vat Farm Manager (`k20_s42_d20/runner_15041`).
- **Bertram Garland**, 27, Researcher (`k20_s42_d20/runner_21210`).

The week:

- **Maren** arrests four people on her day shift (d91, d93). Off shift she jacks in twice, lifts 20 Data from two Labs and sells three lots to Militech for 90c (d93-d96). She buys an arms implant on finance.
- **Silas** lifts Data once (d91 07:25). His bike is stolen, he beats the thief (d94 20:46) and does 10 days.
- The 20k pair barely run: one jack-in, and no runs at all.

**Verdict: alive at 2k day 90, where a cop who steals data is the best double life in the set; thin at 20k.**

**Why there is no runner on seed 43 day 26.** The pick needs a `JackedIn` event in the past 7 days (`citysim-cli/src/shadow.rs::candidates`). Nobody owns a deck yet: decks come only on finance at 60c, and the first purchases land on d31. The s43 diaries show 0 JackedIn and 0 Hack goals all week.

## 2k vs 20k, day 26 vs day 90

- **Jail:** 15 of 26 picks in 2k against 3 of 14 in 20k (the headline).
- **20k gangs are small numbered clones** (The Unplugged 4/5/8/10, Drowned Choir 3) of 3-15 members. They launch lone raids after marches of up to 513 minutes.
- **20k members mostly hold day jobs.** 5 of the 6 non-leader 20k picks are employed.
- **Day 90 gangs are bigger and richer**: Ninefold 47, Hollow 37, and the Choir paid 202c to 48 members. Politics appears: a parley, bribes to the captain, a hit market on rival leaders, coerced witnesses. On day 26 the gangs are still raiding markets and losing.

## Interactions

- **Picks meet each other, and it reads true.** Delia (purist) kills Lorcan (Ninefold), and Lorcan's wife and mother swear kin grudges. Undine (Hollow leader) beats Ursula Ito (purist). Bram Haas carries his leader to a ripperdoc.
- **Families are just header lines.** No married pick (Kieran, Undine, Yorick, Ada) shares a Chat or HangOut with their spouse in 7 days. The spouses appear 1-4 times, only as gossip or sightings.
- **Gossip is a crime blotter**: Assaulted 86, Robbed 17, Evicted 14, Killed 13 of the items heard at 2k.
- **26 % of HangOuts find nobody there** (59 of 224).

## Retracted from the first draft (Coarse-only evidence)

These came from pins that had fallen to Coarse in the old 2k runs and do not appear at Full:

- **Phantom killings.** Off-screen holes were bound to picks who were elsewhere. There are 0 `Attributed` events in the Full diaries. The code risk stays for Coarse bodies: `bind.rs::candidates_in` admits any member of the faction who is not jailed that day.
- **Raid food in pockets.** 237 stolen food in one inventory, and the dealer's PickUp loop that followed (31 × StockGone). The Full raid lost and took nothing. `raid.rs::share_food` still has no capacity cap, worth a unit test.
- **Fired too sick and rehired the same minute.** Not in the Full set.

## Gaps, ranked by how much they distort a life

1. **2k Full-LOD street crime ends in a cell within a day or two** `[bug]`, likely an LOD-parity issue.
   - Evidence: 15 of 26 Full 2k picks spend 2 or more days inside, against 3 of 14 at 20k and fewer in the Coarse-mixed run. A shakedown that took 0 coins gets 6 days.
   - Likely cause: the Full witness-report-arrest path in `law.rs` (file_report from on-screen witnesses, with guards close by at 2k density) runs far hotter than what the stat table models for the same act.
   - Fix: a probe of arrests per Extort/Deal act at Full vs Statistical; scale the Shakedown sentence by the take (0 coins: a fine).
2. **Leaders do their own street crime** `[feature]`. Seven of eight are jailed, downed or dead for raids, shakedowns, data theft or fare evasion.
   - Proposal: a lieutenant takes the risky orders (raids, shakedowns, runs go to rank 1 while the gang has 5 or more fit members), and the leader takes a cut in `leisure.rs::payout` before the equal shares.
3. **Lone raids** `[bug]` (20k). A 66-year-old with cancer goes 1 against 6 (`k20_s42_d20/cultist_17446 d25 10:55`); another raid is 1 against 5 (`gang_leader_10287 d26 20:26`).
   - Cause: `raid.rs::wait_for_crew` stops waiting in the last hour of the march window, and `brawl`/`corp_brawl` then fight with whoever stands at the door.
   - Fix: when fewer than `raid_min_members` are at the door, call the raid off (an event, then `faction::rethink`).
4. **A grudge fight executes a man who is already down** `[bug]`. Delia attacks Lorcan 50 minutes after he went down bleeding, and it becomes a Murder with 22 grudges (`k2_s43_d26_full/purist_801 d27 20:04`).
   - Cause: `utility/goals.rs::fresh_hostile_near` does not skip `world.wounded` targets.
   - Fix: skip downed enemies there; leave killing a downed enemy to Hunt, which requires a kill-level grudge.
5. **The gang's pay is invisible, and Collect pays nothing** `[bug]`. Collect pays "0 to 0" on takes of 40 and 229 (`gang_leader_1594 d27 06:15`, `gang_leader_1757 d90 08:06`). It works when the treasury holds money (Choir: 202 to 48).
   - Cause: the 4c daily stipend and the territory tribute are direct `Wallet` writes in `gang.rs::daily_economy`, with no `Flow`. They drain the treasury before the Collect and show in the diaries only as "daily pass".
   - Fix: route both through `ownership::pay` (`Flow::Stipend`, `Flow::Tribute`).
6. **Shakedowns of the broke** `[bug]`. 9 of 17 extortions take 0 coins (2k 3 of 9, 20k 6 of 8), yet each one still files a crime, advances a claim, and gives every occupant a `WasRobbed` memory and an Enemy edge.
   - Cause: the `gang.rs` extort loop runs `remember` and `robbed_by` even when `share == 0`, and the daily territory tribute has already emptied the wallets.
   - Fix: no memory or edge on a 0 share; target occupied Blocks with coins.
7. **Cults never preach** `[bug]` (V3 gap 9, unchanged). There are 0 Preach actions across 6 cultist diaries, while purists preach.
   - Cause: `leisure.rs::preacher` requires `Order::Expand`, and the Choir sits at LieLow, Squat or Harvest.
   - Fix: let a cult preach under any order but a raid, or hold a weekly service at the Hideout.
8. **Age and sickness never weigh in a fight** `[feature]` (V3 gap 4, unchanged). A 71-year-old leads a raid; a 66-year-old with cancer raids alone.
   - Cause: `raid.rs::strength` is `fighting + 0.25·courage`; `ages.rs` scales only work.
   - Fix: apply the `ages::work_mult` frailty factor and a health factor to `strength` and to the abduction roll.
9. **Interim succession for a night in the cells** `[bug]`. The Unplugged pass to a deputy over a 1-day Fare Evasion stint (`k2_s43_d26_full/gang_leader_1594 d29 23:44`; `k20_s42_d20/cultist_17446 d21 10:36`).
   - Cause: `succession.rs::decide` exempts only Vagrancy.
   - Fix: skip the interim when the `Sentence` is under 2 days.
10. **Dealing has no corner, no customers and no lookout** `[feature]`. Five of six dealers are jailed; sales are 0-2 a week.
    - Proposal: per-dealer corners spread across held districts, repeat buyers as edges, and a lookout that reads the beat. Revisit after gap 1.
11. **Families exist only on paper** `[feature]`. Spouses never Chat or eat together.
    - Proposal: a household evening at home when both are home and off shift.
12. **Gossip is only crimes** `[feature]`.
    - Proposal: add jobs, marriages, sickness, gang orders and who got paid as topics carried on edges.
13. **One assault reported by 11 outlets** `[bug]` (20k). The numbered corp clones each run "Nutrix N Now" (`k20_s42_d20/scav_15850 d25 00:00`).
    - Fix: dedupe a story per franchise name.
14. **Cult typed as scav** `[bug]`, low priority (V3 gap 7). `gray.rs::assign_types` ignores creed: Drowned Choir 8 is a scav gang on 20k. No harvest or expulsion appeared this pass.
15. **Short sleep** `[bug]`, to verify. Some jobless picks sleep 2.4-3.4 h on free days (`purist_2015`, `purist_801`, `cultist_1560`) while they idle 10-15 h. Check the Sleep vs Rest accounting in `shadow.rs` before treating it as their lives.

Also checked: V3 gap 1 (idle hours) is unchanged for jobless members (purists at 14-15 h a day). V3 gap 3 (distance) is unchanged (4-11 h of travel a day) and worse for 20k raids. V3 gap 11 (the robbed do not arm) did not show in these picks.
