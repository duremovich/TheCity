# Lives at work: the counterpart, the evening and the household, careers and leaders, and the 20k city

Companion to `SHADOW_V4.md` (whose features 1-6 this answers) and its critiques in `shadow_v4/`, `JOBS_AND_LIVES.md` (the trades, the Pitch, vice, ages and the gray market this builds on), `LIFE_L2.md` (the evening, the spots, the venues), `SCALE_20K.md`, `TESTING.md`, `VISION.md`. Written 2026-10-17 against main `ce5afe6` (SAVE_VERSION 34), while a separate coder fixes the 23 V4 bugs; § 8 lists what this depends on. Identifiers marked *new* don't exist at HEAD. Every number is a *placeholder* unless it cites a measurement.

**Purpose.** Shadow V4 read 258 diaries and found the city alive wherever a gang, a debt, the law or the hit market touches a life, and thin nearly everywhere else. The trades have ~50 titles and almost every shift is one 358-540 minute `ClerkWork` with nobody on the other side of the counter: no patient, pupil, fare, crate, client, tenant, inmate or attendee. The evening is a far corner or a bed (120 of 369 HangOuts found nobody; 14 of 17 married picks never met their spouse in a week). The trades ignore age (15 of 50 labourers over 60, a 19-year-old teacher, a 19-year-old foreman), and 7 of 8 people running a gang ended the week jailed, downed or dead for crimes they did in person. At 20k the one Precinct puts guards on 56-88 h of travel a week and arrests witnessed crime at half the 2k rate. Dylan picked four groups for this milestone (2026-10-17): **A** work has a counterpart, **B** the evening and the household, **C** ages and leaders, **D** the 20k scale. The design answer to A is one shared mechanism, the **service encounter**, fed by the flows the city already has. Tone: systems-level, every event a seeded game abstraction; names and numbers live in config (the M17 rule).

---

## 0. Decisions

"Mine" = the spec author's, overturnable. "Dylan" = decided 2026-10-17.

| # | question | decision | why | whose |
|---|---|---|---|---|
| D1 | Scope | groups A-D from SHADOW_V4 features 1-6, in seven phases plus a probe phase and the gate | Dylan's pick | Dylan |
| D2 | One mechanism or twenty | one *new* `systems/service.rs`: an encounter is `(server, served, place, kind)`, made hourly from a place's **queue** and its **on-duty servers**; every trade is a `[[serves]]` config row naming its queue source and effects; no per-trade module | the critics asked for twenty counterparts; twenty bespoke passes is twenty parity problems | mine |
| D3 | The parity rule | **decide world-side, animate on screen.** Who serves whom, when and for how much is a tier-blind roll keyed by hashed purposes; a Full body only walks and stands the outcome. A Statistical server or served counts as present (the `tipped_on_duty`/`service_on_duty` pattern); a Full one must be in the building | it is the only rule under which twenty counterparts don't each need a stat-table column (JOBS D17) | mine |
| D4 | Money | encounters re-route **existing** flows to a named server (the visit price, the tip, the fare, the fee, the rent); new coins move only where a row says so (§ 2), never minted | the coin identity and the calibrated books stay put; the city's money circuit is not this milestone's subject | mine (Q1) |
| D5 | What an encounter writes | for bodies (Full and Coarse): one *new* `MemoryKind::Served` each side (mood weight 0, so parity-neutral); for every tier: the *new* `World::regulars` ledger; an `Acquaintance` edge only on the `regular_after`-th meeting within `regular_days`; an event only for the notable outcomes (§ 1.4) | memories are what the diaries read; the ledger keeps edges from flooding the save (the `stat_meet` lesson: 30 → 48 MB) | mine |
| D6 | Who is served first | the queue is FIFO; each server takes up to the row's `per_hour`; ties among servers by `hash(PURPOSE_SERVE, place, served, hour)`, never the lowest id | V4 bug 7 (every tip to the lowest id) is the same bug in the general case | mine |
| D7 | The evening at home | the Home becomes a **spot** (`SpotKind::Home`, *new*) in `leisure::best_spot`, scored by the existing kin term plus a household term; a friend's Home is a spot when its host's habit is there | the spot scorer already weighs kin and `last_met`; the home just wasn't a candidate | mine |
| D8 | Purposeful visits | one appended `GoalKind::Visit` (a scripted plan like `Rescue`): a kin who was hurt, jailed or taken, an elder parent; the pairs are chosen world-side at midnight (`household::visits_due`, *new*) | B's "parent's answer" and "a visit after hearing the spouse was hurt" are one shape | mine |
| D9 | Ages | age bands on the trade rows (`min_age`, `max_age`), seeded seniority on the `Job`, a `Profession` component that survives a layoff, retirement on savings or kin, and a frailty exit; the foreman by experience | V4 features 4 and the labour/care/vice critiques | mine (Q2, Q3) |
| D10 | Gang ranks | `GangMember.rank` 1 is the lieutenant (the field exists; only 0 and 2 are written today); the leader delegates the risky orders while the gang has `delegate_min_fit` fit members; the leader's and lieutenants' cut comes off the top of `leisure::payout` | V4 feature 5 | mine (Q6) |
| D11 | District stations | an appended `BuildingKind::Station`: a booking desk and a few cells, the city's, where a district's guards muster, book and hold; the Jail (the Precinct) keeps the sentences; a nightly transfer van moves the held | V4 feature 6; the single Precinct is bug 22's suspect, so the probe comes first (phase 0) | mine (Q5) |
| D12 | Spire homes at 20k | an exec's Home is drawn and relocated within the Hq's district tier; no eviction to make room | V4 corp critique; rides bug 5's `hq_of` fix | mine (Q7) |
| D13 | Switches | master `[lives] enabled` with `CITYSIM_LIVES_OFF=1`; one sub-switch per phase (§ 9); each off byte-identical on 42-44 × 30 | the JOBS_AND_LIVES pattern | mine |
| D14 | The stat table | no regen before the gate; one regen at the gate (`p_chat_home`, `p_chat`, `p_social` move with home evenings; `p_theft_caught` with stations), Murders checked against main | `thecity-calibration-noise`: a regen redraws the rare rates | mine (Q8) |

---

## 1. Shared machinery: the service encounter

### 1.1 Data

| piece | what |
|---|---|
| `[[serves]]` rows (*new*, config) | `key` (a trade id or a legacy `Role` name, resolved through the same table `Config::unresolved_names` checks), `kind` (*new* `ServeKind`: `Visit`, `Patient`, `Pupil`, `Inmate`, `Tenant`, `Fare`, `Parcel`, `Crate`, `Client`, `Door`, `Staff`, `Buyer`, `Attendee`, `Source`, `Broker`), `per_hour`, `minutes` (the encounter's share of the hour, for the diary), `skill` (the `Skills` field a meeting trains), `skill_gain`, `regular_after`, `regular_days`, `valence` (the memory's), `event_on` (which outcomes print an event) |
| `ServeKind` | appended enum, the one match in `service.rs`; each arm names its queue source (§ 2) and its effect |
| `World::queues` (*new*, not saved) | `BTreeMap<EntityId (place), Vec<Waiting { served: EntityId, kind, since: Tick, coins: i64 }>>`, filled by the source hooks during the hour, drained by the hourly pass, cleared at its end (an hour's unserved are the "waited" outcome) |
| `World::regulars` (*new*, saved, `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`) | `BTreeMap<EntityId (server), SmallVec<[Regular { who, visits: u8, last_day: u16 }; 8]>>`; the oldest entry falls off; empty off, so the off save is byte-identical |
| `MemoryKind::Served` | appended; `subject` the other party; salience `0.15`, mood weight 0; written by `world.remember` (so a chip contract writes nothing: the dollhouse's D9 gap shows, as designed) |
| `EventKind::Served` | appended; pushed only for a row's `event_on` outcomes |
| Flows | none new in the core; rows that move new coins name theirs (§ 2) |

### 1.2 The pass

`service::hourly` (*new*, from `living::run` at the top of the hour, after `vice::hourly`): for each place with a queue, in id order, its servers on duty now (a `[[serves]]` row's staff on shift and not held; a Full or Coarse one inside the place, or at the stop for a round trade; a Statistical one counted present on `lod::shift_attended`), each taking up to `per_hour` from the queue's head; a served goes to the server `argmin hash(PURPOSE_SERVE, place, served, hour)` among those with room. Each pairing calls `service::serve(world, server, served, place, kind)`:

1. the row's arm (§ 2): the outcome (`Done`, `Refused`, `Ejected`, `Short`, `Waited`, …) and any re-routed coins;
2. `Served` memories on both, bodies only;
3. the regulars ledger (both tiers): `visits += 1`; at `regular_after` within `regular_days` and no edge, `social::first_meeting` at the v1 affinity (hashed, not the world stream); with an edge, `social::interacted`;
4. `skill_gain` to the server's row skill, and a day on its `Profession` (§ 4);
5. an `EventKind::Served` when the outcome is in `event_on`.

The queue is filled by **hooks in the flows that already run**: `leisure::on_complete` and `stat_visit` (venue visits), `assets` shop and install sales, `health::oncology`, `stims::detox`, `wounded::stabilise`, `streetwork::stat_cab`, `gray::papers_daily`, `vice::street_hour`, `ages::children`, `camp`'s roster, the Jail's held list, the super's and courier's rounds. A hook is one line: `service::arrive(world, served, place, kind, coins)`.

### 1.3 What a Full body does

Nothing new for a counter trade: the `ClerkWork` (or `BartendWork`, `FabWork`) runs as today and the hour's `Served` memories land in the diary inside it ("served Ines Vo, a regular, at Noodle Bar#560"). Round and fare trades walk (§ 2.3). No `ActionKind` is interrupted mid-shift: a mid-action insert is where V4 bug 12's class of failure lives.

### 1.4 Events

Events are for the moments a reader of the Feed would see: a refusal at a door, an ejection, an eviction notice served, a pilfered crate, a dodged fare, a buy-out, a break-out, a new regular of a vice worker (the leverage hook: a companion knows things). Every other encounter is a memory and a CSV count (`served_by_kind[ServeKind]`, *new*, in `stats.current.jobs`).

### 1.5 Scaling

The pass is O(queued + servers on duty) once an hour; the hooks are O(1); the ledger is bounded at 8 a server (~0.6 MB at 20k). No per-tick read, no per-agent scan.

---

## 2. A: the counterpart, trade by trade

Each row: the queue's source (what already happens that now finds a server), the encounter's effect, any new coins, and the Statistical mirror. "Present" means § 1.2's rule.

### 2.1 Venues and vice (phase 1)

| trade | served (source hook) | effect | coins | mirror |
|---|---|---|---|---|
| bartender, barkeep, club staff, croupier | a paid visit's `Enjoy`/`Drink`/`Gamble` completion at the place (`leisure::on_complete`, `stat_visit`) | `Served`; regulars; a croupier's table game names its dealer | unchanged | the visit already mirrors (`p_venue`); the encounter rides it |
| cook | an `EatOut` meal at a NoodleBar | the meal is the cook's (`cooked` count); **one staff meal a shift** from the bar's stock at shift end (V4: a noodle cook steals food) | the staff meal is stock, not coins (a `Flow::Wholesale`-free take, booked on the bar like spoilage) | the staff meal is a world-side shift-end rule for every tier (beside `exec::actions::end_shift`), so the diet rows move alike |
| dancer | a StripClub or Club visit while a dancer is on duty | the tip to the assigned dancer (Clubs get the tip arm V4 bug 7 adds) | the existing `Flow::Tip` | the visit mirrors; the tip is assigned by hash in both tiers |
| companion, brothel worker | a Lounge (High rung) or Brothel visit | as the dancer; a regular after `regular_after` (3) visits is an edge and an event ("a regular of …"), the M16b leverage hook | `Flow::Tip` | as above |
| chip host | a Dollhouse visit | the encounter runs; `remember` writes nothing under the chip; the diary's empty hours are the cost (V4 feature 12 asks for more; out of scope) | unchanged | as above |
| bouncer | an arrival at a door-kept venue (`leisure::door_ok`, which already reads `trades::door_staff`) | a refusal is `Refused`: an event, a memory each side, the turned-away's `Enjoy` fails at the door as today; **an `Assault` raised inside the venue on a door shift** is `Ejected` on `p_eject × (bouncer fighting − attacker fighting)`: the attacker's plan aborted, put at the door, the fight ended | none | the door check is world-side today; the ejection reads presence (a Statistical bouncer credited), the assault comes from either tier's fight path |
| madam | every shift of her Brothel's workers (`Staff`) | the madam is named as the house's foreman in its `Hire`/`Layoff` text (`industry::foreman_note`'s shape); `madam_cut` of each tip at the house goes to her | `madam_cut` of `Flow::Tip` (worker → madam), *new* coins between purses | the tip and its cut are world-side |
| fighter | a bout (`leisure::bouts`, after V4 bug 17's pairing fix) | the bettors are the bout's served: a winner's bet paid names the fighter | unchanged | `bouts` is world-side |

### 2.2 Care and civic (phases 1 and 3)

| trade | served (source hook) | effect | coins | mirror |
|---|---|---|---|---|
| ripperdoc (phase 1) | a Clinic sale or service: chrome `Shop`, `Install`, therapy, `Detox`, `Treat` (oncology), a `wounded::stabilise` | the doctor is named in the sale's text and memory; a `Patient` regular is an edge (the doctor knows the chrome; a gossip and leverage hook) | unchanged (the sale already pays the owner) | the sales already gate on `fixes::staff_present`; the encounter assigns among the present |
| orderly (phase 1) | a carried-in downed body (`Rescue`'s arrival), a stabilisation, a `Detox` | the orderly is the triage: named in the stabilise event, a memory; with no orderly on duty, `stabilise_no_orderly_mult` (0.9) on the save roll | none | the stabilise roll is world-side; presence as § 1.2 |
| teacher (phase 3) | the children whose routine today is `School` at this School (`ages::place_of`) | the roster: one encounter a pupil a day (children have no Brain: no memory on the child side, the ledger names them); **a School day's `edu_gain` needs a teacher present**, scaled by `0.5 + 0.5 × teacher knowledge`; a parent meets the teacher on the day's first encounter if the parent is a body at home (a chat at the door, no walk) | none (the fee exists) | children are Statistical-only; the teacher's presence as § 1.2 |
| camp warden (phase 3) | the Camp's children (`camp::children_of`) | the headcount (named children); `p_camp_beating` a child-day writes a `ChildTaken`-class grief to its parents (a memory, no body); `p_camp_escape` returns a child to its household (or the street if none); the parents' visits and buy-outs are served here (§ 3.5) | the buy-out (§ 3.5) | world-side daily rolls |
| jailer (phase 3) | the Jail's held (`law::held_daily`'s list) | the meal and the count are the jailer's; a jailer short already raises `factions::breach` odds; the jailer is named on a breakout | none | world-side |
| super (phase 3) | the tenants of each Block on the round (`Tenant`) at the stop | at each stop: a tenant in arrears gets `Notice` (an event: the super serves the eviction warning, the eviction itself stays `ownership`'s midnight rule); a tenant short today pays the super at the door if the purse now holds the due (`settle_rent` called early, the same coins); a repair logged against the Block's condition (`repairs_per_stop`, through the existing Block upkeep) | rent unchanged in amount; the door payment is the same `Flow::Rent`, earlier | the door payment is the midnight's settle moved earlier: the Statistical super's stops run as a world-side roll at the same hour (`shift_attended`), so the arrears clock moves alike |
| civic clerk (phase 5) | an ID or licence sale (`Flow::Licence`) | the clerk named; a bribe (M16a's on-the-take record) names the clerk | unchanged | world-side |

The super's round is also re-sited (phase 3): `trades::new_round` takes the `round_stops` Blocks **nearest the super's Home** within the employer's Blocks in the post's district (`round_near`, *new*), not the id-ordered rotation; the walk between stops counts as work (the shadow's `activity` reads a round leg as work-class); and a super whose employer owns a Block on the round is housed there rent-free (`tied_housing`, *new*: `rent_for` returns 0 for a resident super of the owner; a seeded super moves in at the midnight relocation, never evicting). V4: 63-79 h of travel a week; Bex evicted from the building she supervises.

### 2.3 The street and freight (phase 4)

| trade | served (source hook) | effect | coins | mirror |
|---|---|---|---|---|
| cab driver | a booked ride (`streetwork::stat_cab`, and the same hash roll for a Full commuter in `life::travel`, so a pin changes nothing) | the ride is assigned to an on-duty driver of the Garage; a Full driver drives it: a scripted `Fare` plan (*new* body for the existing duty) of `GoTo(rider's home door) → Drive(target: the rider's work door) → GoTo(Rank)`, at vehicle speed (M13 trips: crashes, `Manslaughter`); a Full rider rides beside him; `p_dodge` (lawless rider) is `Short`: an event and a grudge | the fare unchanged (`Flow::CabFare`) | the ride is a world-side roll; the driver's body animates |
| courier | a parcel (`streetwork::parcel_stops`) | a parcel is a named item (*new* `Parcel { from, to: EntityId, fee }` on `Jobs2State`, not an asset): the drop at the recipient building is an encounter with a staff member present there (or the Statistical staff credited); undelivered by shift end returns to the Yard | `parcel_fee` unchanged | the senders and recipients are world-side; the round already walks |
| loader | each `handling` unit crossing a Yard on a shift (`industry`'s handling fee) | the lift is the loader's (`Crate`, no person): `lifts` per shift in the memory line ("loaded 14 crates for Nutrix"); `p_pilfer` per lift moves one unit to the loader (a `Theft` the owner reports when the shift's count is short), the JAL spec's unbuilt pilfer | the pilfered unit is stock, not coins | world-side per crossing |
| street worker | a client (`vice::street_hour`'s `street_payer`) | the payer is a body **near the pitch** (within `client_tiles`, out walking in the evening) before the district hash; the encounter names the client; a lawless client may rob (the existing `street_assault`, now one of the client's outcomes) | unchanged | the hash draw stays for the off-screen pitch |
| pimp | each of his district's working street pitches, nightly (`Staff`) | a *new* `Collect`-style step: the pimp walks his pitches in pitch hours; presence within `pimp_tiles` is what "protected" means (the assault roll's pimp term reads presence, not freedom); the cut lands in his purse and he passes `pimp_pass_up` to the gang; a pimp absent `pimp_absent_days` loses the district (`name_pimps` re-assigns) | `pimp_cut` (exists) now pays the pimp; `pimp_pass_up` of it to the gang (`Flow::StreetCut`) | the presence roll credits a Statistical pimp on `shift_attended` |
| dealer | a buyer (`stims::buy_stims`, `stat_buy`) | a dealer stands a **corner** (a HangOut spot in a held district, one a dealer, `deal_corner`, *new*, instead of only a Bar) in deal hours; buyers are regulars (an edge after `regular_after`), and a regular buys before a stranger; a lookout (another member on the corner) halves the dealing witness roll | the dose price unchanged | the buy is world-side for the Statistical (`stat_buy`); the corner's presence as § 1.2 |
| vendor, rickshaw, tinker | a sale, a ride, a repair (exist) | named buyers already; they join the regulars ledger | unchanged | exists |

### 2.4 The tower and the brokers (phase 5)

| trade | served (source hook) | effect | coins | mirror |
|---|---|---|---|---|
| CEO and exec | the **Meeting** (exists, `ActionKind::Meeting`) | held at the Hq (bug 5's `hq_of`); the attendees are the corp's grade ≥ 1 staff present at the Hq that hour, up to `meeting_attendees` (6): encounters both ways (the exec meets an employee: V4 "a CEO never meets one of his employees"); **the decision**: when `corp_brain`'s two best orders score within `decision_margin`, the exec's personality picks between them (`courage` → the expansive, `loyalty` → the defensive, hashed), and the day's order, contracts posted and layoffs are pushed as the exec's (`EventKind::Served`, "Zara Hawthorne decided Vatra would Expand in Food; at the table: …"); attendees remember it and gossip it (a work deed, § 3.6) | none | `corp_brain` is world-side; the tilt is a hash of the exec's traits, the same in any tier |
| reporter | the day's crime deeds in the Feed's district (`Source`) | a `Round` duty with *new* stops: the scenes of yesterday's reported deeds in the Feed's district, `round_stops` of them; at a stop, a witness present is interviewed (an encounter); `news::daily` runs the best-covered story under the reporter's byline (`Story.byline`, *new*); a byline count raises `Reporter` standing (`[classpay]` grade 1 after `byline_promote`) | none | the stops and the byline are world-side; the walk is the body's |
| fixer, forger | a Fixer's client: a paper buyer (`gray::papers_daily`), a contract poster or taker (`contracts`) | the midnight paper sale becomes a **walk-in**: the buyer is queued for the Fixer's next open hours and served there by a fixer or forger present (a Full buyer walks, the `Commission` plan's shape; a Statistical one is credited); a contract's post or take at the Fixer is an encounter with the fixer on duty (edges, a first-hand `Hired`); the owner who holds no post **collects in person** once a week (§ 3.4's owner-operator) | the paper price and the FixerCut unchanged; the sale moves from 00:00 to the open hours | the queue is world-side; a Statistical buyer's sale happens at the same hour as a Full one's would |

### 2.5 What stays out

Rooms and furniture; a dialogue layer (M18's); any act at a vice venue (systems-level only); new goods; the owner's pay from profit and the struck-off back room (SHADOW_V4 features 9 and bug 16, not picked); a patient's choice between doctors (the queue is the place's).

---

## 3. B: the evening and the household (phase 2, the camp answer in phase 3)

### 3.1 Home as a spot

`leisure::spots_near` gains `SpotKind::Home(b)` (*new*) for the agent's own Home (keyed by the Home's outside door tile). `best_spot` scores it as any spot: the contacts' habit at `now + 60` (the spouse's building = the Home → p = 1), the existing kin term `kin_w × days since met`, plus `home_w × household members home now` and `child_w × children at home` (`Child.routine` not `Camp`/`Street`). The `HangOut` runs inside the Home; `hangout_done`'s partner is the household member present of highest affinity (`world.residents_of(home)` with a body inside, not `world.hangouts`), so the exchange, `last_met` and the street-dice analogue (`home_dice`, off by default) run as on a corner. Off shift and past `home_from` (18:00) the travel term makes the Home the near spot. V4: 14 of 17 married picks never met their spouse.

### 3.2 A friend's place

`spots_near` also offers `SpotKind::Home(h)` for up to `visit_hosts` (3) Friend or Family contacts whose habit at `now + 60` is their Home, within `spot_walk_cap_tiles`. A visitor arriving to an empty house leaves at once (the empty-spot rule, § 3.3); a visit is a HangOut with the host (`interacted`, gossip, `last_met`). No room cap (a visit is not a bed). The host may refuse an Enemy (`best_spot` already scores enemies down; a visit to a Rival's home is never offered).

### 3.3 Leaving an empty spot

A HangOut with nobody present after `empty_leave_ticks` (20) ends early as `Done` with the lone fun (`spot_lone_fun`); the spot goes into a *new* `Brain.empty_spots: SmallVec<[(TilePos, Tick); 4]>` and `best_spot` scores it × `empty_penalty` (0.3) for `empty_memory_hours` (6). The next pick runs at once (Unwind re-plans). V4: 120 of 369 HangOuts empty at 2k, 39 % at 20k; one pick re-chose the same empty spot five times.

### 3.4 The household

- **Reunite.** `household::daily` (*new*, at midnight, world-side): a married pair living apart (one homeless, in a hotel or a squat, the other housed) moves the unhoused spouse into the housed one's Home when it has room (a move as `ownership`'s midnight `rehouse` makes, the rent share split as today); else the pair joins the relocation queue together. `social::marry` moves in only at the wedding today.
- **New parents.** A Birth sets *new* `Household.newborn_until = birth + newborn_days` (30) on both parents. The carer (the parent without a Job, else the lower wage, else the mother; hashed on a tie) has every away spot and night goal (any goal but Work, Sleep, Eat and Flee from 21:00 to 06:00) scored × `newborn_away_mult` (0.3); both parents' energy decay × `newborn_energy_mult` (1.15); `newborn_cost_per_day` from the household (Food, through the pantry, as a child's). V4: a mother on a corner at 01:39 two nights after the birth.
- **Kids at home.** § 3.1's `child_w`; a School child at home after 16:00, a Chores child at the pitch.
- **The household's business.** `household::business_of(id)`: a building owned by the agent or the spouse. At the midnight search a jobless adult of the owner's household is offered the business's open post before the outside pool (`owner_operator`, *new*); an heir (`assets::settle_corpse`'s inheritance) with a household business takes no outside post the same tick unless the business has had no take for `abandon_days` (7). A household member employed there is never laid off before a non-member (`wages::shed_extra`'s order). V4: Rhoswen inherits her wife's Garage and leaves it for a Vat Tech job the same tick.

### 3.5 Visits with a purpose: `GoalKind::Visit` (appended)

`household::visits_due` (*new*, midnight, world-side, O(fresh kin memories)) posts at most one visit a day an agent, from:
- a kin or spouse Downed, Assaulted or Arrested (a first-hand or rumoured memory of the last day): to the Clinic, the kin's Home or the Station/Precinct (a visiting hour);
- an elder parent (over `frail_from_years`) not met for `elder_visit_days` (7): to the parent's Home or cot;
- a child taken to a Camp: the **parent's answer** (phase 3), chosen by hash with weights from the household's state: **visit** (weekly, the default; an encounter with the warden and a `last_met` with the child), **buy out** (the household holds `buyout_fee` (60) plus `buyout_food_days` of food: the fee to the Camp's owner, `Flow::Fine`-shaped *new* `Flow::Buyout`, and the child goes home), **break out** (lawfulness < 0.3, courage ≥ 0.6, or a gang member who can bring `breakout_crew`: a contest at the Camp door against the wardens present, M9's breach shape; won, the child goes home and a `Crime::Abduction` is filed on the parent; lost, the parent is arrested).
A Full visitor walks the scripted plan (`GoTo(place) → Wait(visit_ticks) → home`); a Statistical one is credited (`social::interacted`, `last_met`, belonging) and its buy-out or break-out resolved by the same roll at the same hour. A parent who never visits (no `last_met` with a Camp child in `neglect_days`) is a printed finding in phase 3, not a camp-take term (Q4).

### 3.6 Gossip beyond crime (a side effect, phases 2 and 5)

`gossip::post_deed` gains three work deeds, each a `Deed` appended with a salience below the crimes': `Hired`/`LaidOff` (the foreman's or madam's pick, § 4.4), `Wed`/`Born` (the household's events), and `Decided` (the exec's Meeting, § 2.4). Not the full SHADOW_V4 feature 8; the minimum that lets a diary hear about a life that isn't a crime.

---

## 4. C: ages, careers and leaders (phase 6)

### 4.1 Age bands per trade

`TradeCfg` gains `min_age` and `max_age` (`#[serde(default)]`, 0 = none); legacy `Role`s get the same keys in a `[roles.<name>]` table. Read by the hiring candidates (`demography`'s pick, `trades::top_up`, the dawn pick) as a hard gate on new hires; incumbents are not fired for crossing a band (the frailty exit does that). Placeholders: physical rows (`loader`, `builder`, `packer`, `day_labourer`, `fighter`, `bouncer`, `guard`) 18-60; `teacher`, `ripperdoc`, `foreman`-eligible 25+; `dancer`, `companion`, `chip_host` 18-45; `street_worker` 18-50; `camp_warden` 25-65. The seeded jobs respect the bands under the switch (the on city's seeds move once, as in JAL phase 7). V4: fighters 61-67, a 70-year-old host, wardens 74-75, 15 of 50 labourers over 60.

### 4.2 Seeded seniority and the foreman

`Job.seniority_days: u16` (*new*, `#[serde(default)]`): at seed, `min(years over the row's min_age, seniority_cap_years) × 120 × u` with `u = hash(PURPOSE_TENURE, id)`; zero for a hire after day 0. **Experience** = `seniority_days + days since hired_tick`. `industry::foreman` becomes the worker of highest experience (ties: the row's skill, then the lower id) aged at least the row's `foreman_min_age` (30); the foreman draws `foreman_premium` (1.25 × wage) and is the actor of the building's `Hire` and `Layoff` events (the foreman's own diary holds them; V4: "no foreman's own diary holds a Hire"). Layoffs by `wages::shed_extra` stay newest-first, by experience.

### 4.3 The profession

*New* component `Profession { trade: TradeKey, days: u32, last_day: u16 }` (one, the latest trade worked ≥ `profession_min_days` (14)), written at each worked shift and seeded from the seed job's seniority. It survives a layoff, a jail term and a move. Hiring ranks a matching profession first (`profession_w`), then skill; a laid-off loader returns to loading. A trade's skill floor (`min_skill`, V4 bug 6's fix) admits a holder of the profession below the floor (experience stands in for the test).

### 4.4 Retirement and the frailty exit

`ages::daily` (world-side, midnight): a worker over `retire_from_years` (62) retires (quits, an event "retired from …", `Trait::Retired` *new*: no Earn's job search, no hire) when the household's coins cover `retire_days` (60) of its costs, or an adult child with a Home takes them in (the JAL kin take-in, run first). Otherwise they work on, and when `ages::work_mult` falls under `frail_exit_mult` (0.6) the employer lets them go "too frail" (the `elder_fire_mult` path, an event, a `Profession` kept). No pension and no dole (the no-safety-net rule). A retired elder's day: the Home spot (§ 3.1), kin visits (§ 3.5), Beg when the money runs out (the `Retired` trait clears when coins fall under `unretire_coins`, and the street ladder takes over).

### 4.5 Leaders lead

- **Lieutenants.** `gang::recompute_leader` writes rank 1 to the top `1 + members / lieutenant_per` (8) of `leader_scores` after the leader (V4 bug 8's tenure weighting assumed).
- **Delegation.** With `fit_headcount ≥ delegate_min_fit` (5), the leader's `following_order` returns `None` for `Expand`, `Squat`, `Harvest`, `VirtRaid` and the march of a `Raid` (the leader keeps `Lead`: collect, call, parley, bribes, and fights in a defence or a `Contest` at the Hideout). A raid's `actor` (the one `raid::depart` and `resolve` key on) is the top lieutenant; `raid_min_members` calls off a short raid (V4 bug 9's fix). `lod::stat_extort_target` and the Statistical gang work read the same rule, so off-screen leaders stop extorting too.
- **The cut off the top.** `leisure::payout` takes `leader_cut` (0.2) of the pool for the leader and `lieutenant_cut` (0.1) shared by the lieutenants before the equal shares (`Flow::Tribute`, after V4 bug 14 books it); the Collect event prints the split. A rich leader is a target (the hit market's price reads coins already).
- **The ladder.** A lieutenant's raid record (`raids_led`, *new* on `GangMember`) enters `leader_scores` as `raids_won × raid_w`, so succession prefers the lieutenant who led and lived.

---

## 5. D: the 20k city (phase 7; the probe in phase 0)

### 5.1 The probe first (bug 22)

Phase 0 builds `law_probe` (*new*, `citysim-cli`, printed, never asserted): per witnessed crime, the report and the arrest, by the suspect's district's walk to the Precinct, by tier, at 2k (42-44 × 60) and 20k (42 × 30). V4 measured reports 0.18 vs 0.32-0.40 a diary and arrests 0.30 vs ~0.6 at the same witness rate. If the arrest share falls with the escort distance, § 5.2 is the fix; if it is flat, the cause is elsewhere (a per-capita cap, `crackdown_reports_per_1000`, the plan queue: V4 bug 20) and phase 7 builds § 5.2 for the guards' travel alone and files the rest.

### 5.2 Stations

- **Data.** `BuildingKind::Station` (*new*, appended; city-owned, Civic niche off; `[buildings.station]` capacity `station_cells` (6, per capita through `fit_to_map`), on a district's Civic or Mid Lot). `[law] stations = true`, `station_min_tiles` (a district whose Hall-to-Precinct walk is under it gets none: the 2k map gets 0-1, the 20k island ~6-8, measured at seed), `transfer_hour` (04:00), `station_desk_share` (0.25).
- **Guards.** `law::reconcile_guards` hires to the Station nearest the candidate's Home with fewer than its share (`guard_count × district residents ÷ city`), so a guard's post is near home; the law brain's `beats` give each guard a district of its Station. Desk days are at the Station.
- **Arrests.** The escort (`law::follow_guard`, `jail_suspect`) goes to the nearest of the Precinct and the Stations. A Station books the suspect: a fine (V4 bug 4's fine-when-able) or a one-night Vagrancy is served there; anything longer waits in the cells until `transfer_hour`, when the van moves every held body to the Precinct (bodiless, like a carried prisoner; the held keep their `Sentence` from booking). Full cells (`station_cells`) send the escort on to the Precinct.
- **The mirror.** Statistical arrests come through `p_theft_caught` (Full arrests per Full theft); stations change the Full rate, so the gate's regen re-learns it and the parity test prints the arrest rows. Corp guards stay corp guards (V4 bug 11).

### 5.3 Spire homes for execs

With bug 5's `hq_of`, a corp exec's Home at seed is drawn from the Homes in the Hq's district tier (`exec_home_tier`, *new*: the Spire for a corp with an Hq), and `life::relocate`'s "nearer work" for an exec considers only Homes of that tier or better; the 20k generator's Spire Block supply is checked at seed against `execs × exec_home_spare` (1.5), printed. No eviction to make room. V4: Arasaka's 20k head moved into Sump 3 and slept in a church shelter.

### 5.4 Near rounds and beds

The super's near round and tied housing (§ 2.2) and the station beats (§ 5.2) are the 20k distance fixes this milestone owns; the Hideout bed walk cap and the corp patrol radius are V4 bug 11's.

---

## 6. LOD parity, mechanism by mechanism

| mechanism | Full | Statistical | why they agree |
|---|---|---|---|
| encounter (§ 1) | server in the building, served from the hook | server counted present on `shift_attended`, served from the same hook | the pass is world-side and tier-blind; memories have mood weight 0 |
| tip and cut routing | hash among present | hash among counted | the coins already moved in both tiers |
| cook's staff meal | shift-end rule | shift-end rule | world-side |
| bouncer ejection | presence + fight | presence credited | the fight comes from either tier's path |
| teacher's School day | presence gates `edu_gain` | presence credited | children are Statistical-only |
| super's door rent | the stop's encounter | the same roll at the same hour | the midnight settle runs early for both |
| cab ride | the same hash roll; the driver's body drives | counted ride | `stat_cab`'s roll is the single source |
| parcel, crate, pilfer | the round's stops | credited at the stop's hour | world-side per unit |
| pimp's presence | body within `pimp_tiles` | credited on `shift_attended` | as the bouncer |
| dealer's corner | body on the corner | `stat_buy` | the buy is world-side |
| exec's decision | the Meeting | `stat` exec credited | `corp_brain` is world-side |
| fixer's walk-in | the client walks | credited at the same hour | the queue is world-side |
| home and friend's place | spot choice | `p_chat_home`, `p_chat`, `p_social` | **learned at the gate's regen** (D14) |
| empty-spot leave | spot choice | none (no spots off screen) | the fun gain is the free rung's; printed |
| reunite, newborn flags | midnight rule | midnight rule | world-side |
| `Visit` | the body walks | credited | `visits_due` is world-side |
| camp answers | walk + contest | same roll | world-side |
| ages, profession, retirement, foreman | midnight and hire rules | the same | world-side |
| lieutenants and the cut | `following_order` | `stat_extort_target` reads the same rule | world-side |
| stations | the escort walks to the nearest | `p_theft_caught` | **learned at the gate's regen** |

The parity test (`lod.rs::test_full_vs_statistical_within_15pct`) gains printed rows: encounters by kind per staffed shift, the home share of chats, arrests per theft by tier. Asserted only where the row is world-side by construction (encounters per shift, within 15 %).

---

## 7. Throughput

The 2k city keeps its 4,000 ticks/s floor (`core_throughput`) and 20k its printed 140 (`core_20k`). Per phase: the encounter pass O(queued + on duty) hourly; the hooks O(1); `best_spot` gains at most `1 + visit_hosts` candidates per pick and reads habits it already reads; `empty_spots` is four entries; `visits_due` and `household::daily` are O(fresh memories + married pairs) at midnight; the age and profession rules ride the existing midnight and hire passes; stations make the escort's target a min over ~8 doors. No new per-agent per-tick read. `prof` names each new site (`service`, `household`, `visits`); a phase whose 2k `core_throughput` drops more than 5 % is profiled before merge.

---

## 8. Dependencies on the V4 bug fixes (in flight, another coder)

Phase 0 starts on the main that holds them. The ones this spec reads:

| V4 bug | read by |
|---|---|
| 2 the night gate, Rest vs Sleep, the night grocery | § 3 (the evening and the newborn's night damping assume the Night gate) |
| 3 Fight proportion | § 2.1's ejection and § 3.5's break-out read the relative-strength term |
| 4 the sentence mix (a fine when able) | § 5.2's station booking |
| 5 `hq_of` and the exec leaks | § 2.4's Meeting, § 5.3 |
| 6 the wrong person in the post (skill floor, `quit_blocks`, the pitch spot) | § 4.1, § 4.3 |
| 7 tips (lowest id, Clubs) | § 2.1 (the encounter's assignment supersedes `tipped_on_duty`'s pick; the Club arm comes from the fix) |
| 8 leadership by tenure; the 1-day interim | § 4.5 |
| 9 lone raids, 0-coin shakedowns | § 4.5 (delegation assumes the call-off) |
| 11 corp guards, `holes_up_at` | § 5 |
| 12 the courier's PreconditionLost | § 2.3 (the courier's drop is at someone else's building) |
| 14 the gang's pay as `Flow`s | § 4.5 (the cut) |
| 17 bouts | § 2.1 |
| 18 staff leisure at their own workplace | § 2.1 (a worker is never served at its own post) |
| 20 the plan queue's aging at 20k | § 7 at 20k |
| 22 the 20k arrest rate | § 5.1's probe replaces "a probe before a fix" |
| 23 the shadow picks | § 9's diaries (day labourer, pimp, vendor) |

---

## 9. Phases

Each phase is one commit, merged by the orchestrator; each runs the trio, `core_year`, the behaviour tier, the gods, parity and `core_20k`, measures 42-47 × 120 at 2k and 42 × 30 at 20k against the phase before, and checks its off identity (42-44 × 30 byte for byte, events and the shared CSV columns). One solo coder, phase by phase (`plan-implementation-sop`), `/code-review high` at the end.

| # | phase | builds | switch (off = byte-identical) | why here |
|---|---|---|---|---|
| 0 | **Baselines** | `law_probe` (§ 5.1); a `lives_probe` printing, per archetype, idle hours, spouse meetings a week, empty HangOuts, `Served`-able shifts (shifts with a source event at the place) | none (tools only) | measure first: the money-circuit lesson; bug 22's probe |
| 1 | **The encounter, venues and the Clinic** | § 1 whole; § 2.1; § 2.2's ripperdoc and orderly; SAVE 35 | `[lives] enabled`, `[service] venues` | every source already exists as a visit or a sale: the smallest change that puts a person across every counter the critics named first |
| 2 | **The evening and the household** | § 3.1-3.4, § 3.5's kin and elder visits, § 3.6's `Hired`/`LaidOff`/`Wed`/`Born` deeds | `[household] enabled` | small, high-value, and independent of the counterparts |
| 3 | **Civic counterparts and the round** | § 2.2's teacher, warden, jailer, super (tenants, near round, tied housing); § 3.5's parent's answer | `[service] civic` | the camp answer needs the warden's encounter |
| 4 | **The street and freight** | § 2.3 | `[service] street` | needs bug 12; the cab driver's body is the largest new plan |
| 5 | **The tower and the brokers** | § 2.4; the civic clerk; § 3.6's `Decided` deed | `[service] tower` | needs bug 5; the decision touches `corp_brain` |
| 6 | **Ages, careers and leaders** | § 4 | `[careers] enabled`, `[gangs] lieutenants` | moves the seeded city (bands, seniority): after the counterparts so their diaries are read on the old ages first |
| 7 | **The 20k city** | § 5.2, § 5.3 | `[law] stations`, `[life] exec_home_tier` | read phase 0's probe; the largest map change |
| 8 | **The gate** | the one stat-table regen with the Murders check; `core_sanity` bullets; unit tests; probes; gods; Shadow V5 diaries and critics; `analyze_run.py` sections; "Implemented: deviations"; `/code-review high` and its fix commit | — | TESTING.md's cadence |

### 9.1 Gate bullets and diary acceptance per phase

Bullets are mechanism-existence candidates (measured on 42-47 first; on 2 or fewer of 6 seeds they become unit tests and printed `FINDING`s, never bullets). The diary acceptance is what a critic reading Shadow V5's diaries of those archetypes should now see; it is judged by reading, not asserted.

**Phase 1.**
- Bullets: a `Served` at a Bar, a NoodleBar, a StripClub, a Lounge, a Clinic; a regular made; a door refusal; a cook's staff meal; a tip at a Club; a madam's cut.
- Unit tests: the queue is FIFO and capped by `per_hour`; the server is a hash among the present, never the lowest id; a Statistical server is counted; an encounter writes no memory under a chip; the regulars ledger caps at 8; an off city writes no `regulars`.
- Diaries: the bartender, cook, dancer, companion, club staff, ripperdoc and orderly each name people served in a shift, some of them twice in the week; a bouncer turns someone away or ejects a fighter; a ripperdoc's week holds patients by name; the companion's idle hours are unchanged (that is § 3's), but the shift has faces.

**Phase 2.**
- Bullets: a HangOut at home with a spouse; a visit to a friend's Home; an empty spot left early; a reunited couple; a `Visit` to a hurt kin.
- Unit tests: `best_spot` picks the Home when the spouse is due home and the corner when not; an empty-spot memory scores the spot down for `empty_memory_hours`; the newborn's carer is damped and the other parent is not; `reunite` moves the homeless spouse in when there is room; an heir with a household business keeps it.
- Probes: married, off shift at 19:00 with the spouse due home: at home by 21:00 and a Chat logged (control: unmarried, the corner); a mother three days after a birth is home between 22:00 and 06:00 (control: the father on a night shift works).
- Diaries: married picks meet their spouses most evenings; a friend's place appears; no pick re-chooses an empty corner in the same evening; a new parent's nights are at home; empty HangOuts fall well under V4's 120 of 369.

**Phase 3.**
- Bullets: a School day with a teacher; a Camp headcount; a super's door payment; an arrears notice; a parent's Camp visit.
- Unit tests: no teacher, no `edu_gain`; a super's round takes the nearest Blocks; a tied super pays no rent; a buy-out returns the child and moves `buyout_fee`; a break-out is a contest at the door and files `Abduction` when won.
- Probes: a parent with 80 coins and food whose child was taken buys it out within 14 days (control: 10 coins, visits); a super walks fewer than 4 h a shift.
- Diaries: a teacher has named pupils; a warden counts named children and loses one to an escape now and then; a super collects, warns and repairs, and lives on the round; Cass's taken child gets a visit or a buy-out, not only a grudge.

**Phase 4.**
- Bullets: a cab fare driven by a named driver; a parcel delivered to a named recipient; a lift; a pimp's collection in his purse; a dealer's regular.
- Unit tests: a Full commuter rolls the same cab hash as a Statistical one; a pilfered unit is reported when the count is short; the street assault reads the pimp's presence, not his freedom.
- Diaries: a cabbie drives fares; a courier hands parcels to people; a loader loads crates; a pimp walks his pitches and keeps a cut; a dealer has a corner and repeat buyers; a street worker's night has a client.

**Phase 5.**
- Bullets: a Meeting with attendees; an exec's decision event; a byline story; a walk-in paper sale at the Fixer.
- Unit tests: the decision picks between the two best orders only within `decision_margin`; a paper sale lands in open hours, never at 00:00.
- Diaries: a CEO meets employees at the Hq and makes a call they gossip; a reporter interviews a witness and has a byline; a fixer meets clients and takers.

**Phase 6.**
- Bullets: a retirement; a frailty exit; a foreman's own `Hire`; a lieutenant-led raid; a leader's cut.
- Unit tests: a new hire outside a row's band is refused; seniority is seeded and experience orders the foreman; a profession survives a layoff and ranks the rehire; the leader's `following_order` is `None` for a raid with 5 fit members and `Some` with 4; the payout's cut comes first.
- Probes: a laid-off loader with a profession is rehired as a loader before a stranger; a 64-year-old with 600 coins retires (control: 20 coins, works on).
- Diaries: few workers over 60 in physical trades, none in their 70s; a foreman is the most experienced, over 30, and hires; leaders collect and parley, lieutenants raid and shake down; fewer than half the leaders end the week jailed, downed or dead.

**Phase 7.**
- Bullets (20k, `core_20k` mechanisms): a Station booking; a transfer at `transfer_hour`.
- Unit tests: the escort takes the nearest Station; a full Station sends it on; a guard is hired to the Station nearest home.
- Diaries (20k): guards travel well under V4's 56-88 h; execs live in the Spire; the arrest share per witnessed crime is printed beside the 2k one (bug 22's finding read again).

---

## 10. Acceptance (TESTING.md form)

Asserts catch broken; behaviour judges alive; numbers are reports.

- **`core_sanity`**: the candidates of § 9.1 that fire on 3 or more of 6 seeds; the coin identity and `escrow_leak` unchanged; the collapse bounds unchanged; the 4,000 tps floor.
- **`core_20k`**: the station bullets; ticks/s printed against 140.
- **Person probes** (`tests/person.rs`): § 9.1's; plus "a regular": a Full agent who visits the same Bar three evenings is an Acquaintance of its bartender (control: three different Bars).
- **God scenarios** (60 days against the control, one per reaction class):
  - `god_kill_all_teachers` (day 20): `edu_gain` stops, the School posts refill.
  - `god_child_camp_sweep` (every Street child taken on day 10): visits, buy-outs, a break-out; grudges against the Law.
  - `god_decapitate_lieutenants` (every rank 1 killed on day 30): the leaders lead raids again until new lieutenants rise.
  - `god_cut_the_precinct` (the Precinct's guards jailed 10 days, day 20, 20k): the Stations hold and transfer; arrests fall less than in a no-station control.
  - `god_retire_everyone_over_55` (day 10): the trades refill, the foreman's post moves, kin take-ins rise.
- **Shadow V5** (the milestone's diary read, 2k day 26 and 90 and 20k day 20, the V4 recipe): every archetype re-read; the bounds' `known/wk`, `walk_h/d` and `work_h/d` re-calibrated on the eight-run ensemble only if the rows move for these reasons.
- **Printed bands** (`tools/analyze_run.py`, never asserted): encounters by kind per staffed shift; regulars made; the home share of HangOuts and chats; spouse meetings a week; empty HangOuts; visits by purpose; buy-outs and break-outs; ages by trade (the over-60 share in physical rows); retirements and frailty exits; foreman's mean age and experience; leaders' and lieutenants' jail-days; the leader's cut; arrests per witnessed crime by escort distance; guards' travel hours; execs by home tier; ticks/s at 2k and 20k per phase.

---

## 11. Risks

| risk | what could go wrong | the answer |
|---|---|---|
| R1 parity drift | a counterpart that moves a Full body's day changes what the Statistical table learned | D3 by construction for every encounter; the home evening and stations are the two learned moves, re-learned once at the gate (D14); the parity test prints the new rows |
| R2 the save | regulars, memories and edges grow the save at 20k | the ledger is 8 a server; memories bodies-only; edges only at `regular_after`; the save size printed per phase (the 30 → 48 MB lesson) |
| R3 the city softens | home evenings and retirement take people off the corners; stations and lieutenants take leaders out of the cells | printed J24, J25, Murders per phase; restore tone by pressure (rents, cuts), never by broken days (`thecity-brutality-via-pressure`) |
| R4 the city hardens | break-outs, pilfering and buy-out debts add crime | each rare, capped per capita, and printed |
| R5 throughput | the evening spot pick runs often | O(1 + visit_hosts) extra per pick; the `prof` site; a 5 % rule per phase |
| R6 the seeded population moves | age bands and seniority change who holds what at day 0 | all under `[careers]`; the on city's seeds move once, in phase 6, and the gate re-reads |
| R7 the V4 fixes move first | the bug coder's merges shift the baseline mid-spec | phase 0 measures on the fixed main; this spec's numbers are V4's and are re-read there |
| R8 tone | encounters at vice venues | systems-level: a visit, a name, a tip, a regular; no act described (JAL R9) |

---

## 12. Questions for Dylan

1. **Money in encounters.** Re-route only (the visit, tip, fare, rent and fee named to a server), or new per-encounter fees (a doctor's cut per patient, a teacher's per pupil)? *Recommendation: re-route only, plus the three named new flows (the madam's cut of tips, the pimp's keep, the Camp buy-out); the money circuit is the calibration milestone's.*
2. **Retirement.** On savings or kin with a frailty exit for the rest, or only the frailty exit? *Recommendation: both; no pension. A savings retirement is the rich elder's life, the frailty exit the poor one's.*
3. **Age bands.** A hard gate on new hires, or a hiring penalty that still lets a 70-year-old load in a labour famine? *Recommendation: a hard gate on hires, never a firing; the frailty exit handles incumbents.*
4. **The taken child.** May a parent break a child out of a Camp (a crime, a fight with the wardens), and should a parent who never visits raise the service's odds of taking the next child? *Recommendation: break-outs yes, rare and lawless-only; neglect printed only in this milestone.*
5. **Stations.** City-run district stations with cells and a nightly van, or more Precincts (each a full Jail)? *Recommendation: stations; the Jail's 1,600 beds at 20k stay one place and one breakout target.*
6. **Leaders.** Should a leader never do street crime above `delegate_min_fit`, and is 20 % off the top right? *Recommendation: never on the march or a shakedown, always in a defence or a parley; 20 % for the leader, 10 % for the lieutenants, printed.*
7. **Spire homes.** Seat 20k execs in the Spire at seed and on relocation without evicting anyone? *Recommendation: yes; and if the 20k Spire is short, the generator adds Blocks rather than the sim evicting.*
8. **The one regen at the gate** (`p_chat_home`, `p_chat`, `p_social`, `p_theft_caught`). *Recommendation: once, at the gate, Murders checked against main; keep main's table if it drifts past the noise.*

## Dylan's answers

2026-10-17:

1. **Money:** re-route only, plus the three named flows (the madam's cut, the pimp's keep, the Camp buy-out).
2. **Retirement:** both: on savings or kin, and a frailty exit; no pension.
3. **Age bands:** a hard gate on new hires, never a firing.
4. **The taken child:** visit, buy-out, and a rare break-out for the lawless only; neglect printed only.
6. **Leaders:** never on the march or a shakedown above `delegate_min_fit`, always at a defence or a parley; 20 % off the top for the leader, 10 % for the lieutenants, printed.

Not yet asked: questions 5 (stations), 7 (Spire homes at 20k) and 8 (one regen at the gate). The recommendations stand as defaults unless Dylan overrides them before phase 7 and the gate.

