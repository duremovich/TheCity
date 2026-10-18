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

*Implemented* (phase 0, 2026-10-18): the probe is `citysim/tests/law_probe.rs` (`law_probe_2k`, `law_probe_20k`, `tier: diag`), not a `citysim-cli` subcommand, because the V4 sweep had already put it there; `python tools/test.py diag --only law_probe_2k`. The distance is Manhattan tiles from the district's centroid (its mean street tile; districts have no Hall) to the Jail's door; the suspect's district is its `Position` up to an hour after the act; the bands are quartiles of the districts' distances (the 2k map's 8 districts fall 3/2/2/1).

**Measured, phase 0 (2026-10-18, main `23a161d` plus the phase 0 probes).** Share of witnessed acts arrested within 3 days (reported within 1 day in brackets), by the suspect's district's walk to the Precinct; the tier rows where the band has ≥ 20 acts of the tier.

| city | band (tiles to the Precinct; districts) | acts | arrested (reported) | Coarse | Full | Statistical |
|---|---|---|---|---|---|---|
| 2k, 42-44 × 60 d pooled | 1: 2-45 (3) | 2,609 | 0.82 (0.69) | 0.80 | 0.85 | 0.91 |
| | 2: 108-122 (2) | 705 | 0.68 (0.54) | 0.68 | 0.69 | — |
| | 3: 126-144 (2) | 1,245 | 0.63 (0.29) | 0.64 | 0.55 | 0.77 |
| | 4: 189 (1) | 405 | 0.50 (0.28) | 0.47 | 0.72 | — |
| | all | 4,964 | 0.73 (0.53); per seed 0.71 / 0.69 / 0.78 | 0.71 | 0.77 | 0.84 |
| 20k, 42 × 30 d | 1: 2-128 (7) | 3,328 | 0.83 (0.76) | 0.81 | 0.89 | 0.82 |
| | 2: 144-324 (7) | 1,564 | 0.70 (0.52) | 0.67 | 0.78 | 0.75 |
| | 3: 376-520 (6) | 1,017 | 0.43 (0.29) | 0.41 | 0.58 | 0.64 |
| | 4: 572-692 (6) | 1,310 | 0.37 (0.21) | 0.35 | 0.48 | 0.67 |
| | all | 7,219 | 0.66 (0.54) | 0.61 | 0.84 | 0.78 |

Reading: **the arrest share falls with the distance** at both scales, 0.82 → 0.50 over 2-189 tiles at 2k and 0.83 → 0.37 over 2-692 tiles at 20k, and the fall is in the Coarse tier (0.81 → 0.35 at 20k) more than in Full (0.89 → 0.48, few acts) or Statistical (flat ~0.7-0.8: no escort). The report share falls with it (0.76 → 0.21 at 20k), so the witnesses' walk is part of it, not the escort alone. No decision here; phase 7 reads this. The 20k Precinct went full 34 times in 30 days (`UNPUNISHED`); 2k never. Per-district rows: `law_probe_20k`'s output (`target/test-logs/`).

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
| 0 | **Baselines** | `law_probe` (§ 5.1); a `lives_probe` printing, per archetype, idle hours, spouse meetings a week, empty HangOuts, `Served`-able shifts (shifts with a source event at the place). *Implemented* (2026-10-18): `law_probe` is `citysim/tests/law_probe.rs` (a diag-tier test, not a `citysim-cli` subcommand: it already existed there), `lives_probe` is `citysim-cli shadow --lives` (§ 13) | none (tools only) | measure first: the money-circuit lesson; bug 22's probe |
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

---

## 13. Phase 0 baselines

Measured 2026-10-18 on main `23a161d` plus the phase 0 probes, before any phase 1-7 change, with `citysim-cli shadow --lives` (the `--assert` recipe over all 48 pinnable archetypes: day 19, 7 days, seeds 42-44 pooled, up to 30 free agents per archetype and seed, each archetype in its own clone) and `shadow --lives --population 20000` (seed 42, day 20, 7 days, 228 picks of 48 archetypes pinned together in one world, 5 each: V4's 20k recipe). Printed, never asserted; rows with ≥ 5 agents and the pooled `all` (an agent picked for two archetypes counts in both; `ceo` and `exec` are the same 27 people at 2k). The columns: idle-class hours an agent-day; married picks (a spouse at the window's start), of them those that never *named* the spouse (no social move with, no HangOut beside) and never *co-located* with it awake (an hourly sample: same building, or both outdoors within 2 tiles, neither asleep); named hours a married-agent-week; co-located hours a married-agent-week (in brackets the share of those hours whose spouse was Statistical, standing where its demotion left it); HangOuts a week and the share with nobody else at the spot; whole free workday shifts at a building, the share *Served-able* (a non-staff body inside the workplace at an hourly sample, or a source event there: `AssetBought`, `Installed`, `Treated`, `Rescued`, `Cured`, `LicenceSold`, `Gambled`, `Bout`, `MissionServed`, `Donated`, `TraumaSave`), the share with any signal (the venue's `visits_today` tally added, the one place a Statistical visit shows), and the mean non-staff bodies per shift hour (the guard's are the Jail's inmates, the CEO's the Hq's meeting crowd; a Statistical visitor moves no body and writes no event, so a counter trade's 0.00 is a floor, not the truth; phase 1 makes the encounter itself visible).

**Beside V4** (`SHADOW_V4.md`, 258 diaries read by hand): V4 read 120 of 369 HangOuts empty (33 %); the probe reads 29 % at 2k (7,591 of 26,630) and 47 % at 20k (1,766 of 3,719). V4 read 14 of 17 married picks never meeting the spouse in a week; the probe reads 997 of 1,427 never naming the spouse at 2k (70 %) and 148 of 159 at 20k (93 %), but only 460 of 1,427 (32 %) and 70 of 159 (44 %) never co-located awake: couples share a roof for hours and do nothing together there, which is § 3's home-spot gap exactly. V4 read "almost every shift with nobody on the other side of the counter"; the probe reads 28 % of 8,426 whole shifts Served-able at 2k (32 % with the venue tally) and 33 % of 906 at 20k, and by trade: cooks, teachers, couriers, loaders, sweepers, builders, reporters 0.00-0.06; night porters 0.80 (hotel guests), bouncers 0.76, club staff 0.51, dancers 0.50 (bets and bouts), orderlies 0.43 and ripperdocs 0.44 (installs and treatments), guards 0.68 (inmates); the lives probe's 3.9 idle hours a day sits where V4 read 24-29 idle hours a week on the jobless street picks (`day_labourer` 6.2, `homeless` 5.7, `companion` 8.6 here).

### 2k, seeds 42-44 pooled, day 19, 7 days

| archetype | n | idle h/d | married | never named | never co-located | named/wk | co-located h/wk (Stat spouse) | HangOuts/wk | empty | shifts | Served-able | any signal | bodies/h |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `bouncer` | 50 | 4.50 | 42 | 19 | 4 | 4.05 | 31.93 (0.02) | 19.26 | 0.32 (307/963) | 233 | 0.76 | 1.00 | 0.51 |
| `builder` | 90 | 3.26 | 89 | 45 | 13 | 1.79 | 38.44 (0.02) | 13.37 | 0.07 (82/1194) | 524 | 0.00 | 0.00 | 0.00 |
| `cabbie` | 17 | 2.71 | 9 | 7 | 4 | 0.67 | 18.67 (0.14) | 18.88 | 0.46 (147/321) | 98 | 0.19 | 0.19 | 0.01 |
| `camp_warden` | 6 | 3.75 | 5 | 1 | 0 | 3.20 | 53.60 (0.01) | 11.17 | 0.19 (13/67) | 35 | 0.06 | 0.06 | 0.01 |
| `ceo` | 27 | 5.60 | 14 | 14 | 6 | 0.00 | 1.14 (0.94) | 1.33 | 0.72 (26/36) | 6 | 1.00 | 1.00 | 42.98 |
| `club_staff` | 90 | 4.13 | 82 | 60 | 19 | 1.89 | 18.87 (0.06) | 17.49 | 0.25 (385/1562) | 432 | 0.51 | 0.77 | 0.51 |
| `companion` | 6 | 8.57 | 4 | 4 | 1 | 0.00 | 39.25 (0.02) | 4.50 | 0.52 (14/27) | 30 | 0.00 | 0.80 | 0.00 |
| `cook` | 90 | 3.54 | 77 | 40 | 9 | 4.62 | 38.46 (0.04) | 18.56 | 0.23 (387/1664) | 505 | 0.04 | 0.04 | 0.01 |
| `courier` | 36 | 3.60 | 34 | 2 | 0 | 2.76 | 59.03 (0.00) | 11.06 | 0.11 (44/398) | 216 | 0.06 | 0.06 | 0.01 |
| `cultist` | 19 | 3.37 | 13 | 10 | 6 | 0.47 | 16.21 (0.01) | 16.73 | 0.33 (104/316) | 43 | 0.67 | 0.67 | 0.29 |
| `dancer` | 43 | 5.14 | 40 | 19 | 5 | 4.93 | 35.08 (0.03) | 16.16 | 0.29 (201/695) | 204 | 0.50 | 1.00 | 0.40 |
| `day_labourer` | 53 | 6.21 | 35 | 29 | 21 | 0.31 | 8.83 (0.30) | 19.19 | 0.35 (361/1017) | 70 | 0.33 | 0.33 | 0.31 |
| `elder` | 90 | 4.14 | 68 | 61 | 21 | 0.25 | 9.85 (0.35) | 16.42 | 0.38 (560/1478) | 362 | 0.20 | 0.21 | 1.81 |
| `exec` | 27 | 5.60 | 14 | 14 | 6 | 0.00 | 1.14 (0.94) | 1.33 | 0.72 (26/36) | 6 | 1.00 | 1.00 | 42.98 |
| `fabber` | 90 | 3.23 | 85 | 57 | 19 | 1.60 | 30.28 (0.02) | 11.02 | 0.29 (284/984) | 513 | 0.15 | 0.15 | 0.02 |
| `fighter` | 24 | 4.68 | 22 | 10 | 3 | 5.13 | 29.97 (0.04) | 22.56 | 0.33 (173/520) | 115 | 1.00 | 1.00 | 0.08 |
| `gang_leader` | 12 | 2.61 | 7 | 7 | 5 | 0.00 | 2.00 (0.57) | 10.17 | 0.54 (66/122) | 41 | 0.41 | 0.44 | 0.25 |
| `gang_member` | 74 | 3.95 | 51 | 44 | 21 | 0.20 | 15.89 (0.03) | 16.92 | 0.42 (512/1224) | 140 | 0.37 | 0.40 | 0.25 |
| `guard` | 90 | 3.91 | 54 | 37 | 22 | 0.93 | 5.62 (0.02) | 14.69 | 0.29 (385/1312) | 536 | 0.68 | 0.68 | 16.13 |
| `gun` | 56 | 3.44 | 36 | 31 | 21 | 0.17 | 2.34 (0.45) | 12.13 | 0.28 (191/677) | 168 | 0.26 | 0.26 | 2.15 |
| `homeless` | 54 | 5.74 | 22 | 8 | 7 | 1.64 | 11.27 (0.02) | 18.94 | 0.36 (367/1023) | 50 | 0.38 | 0.38 | 0.40 |
| `joytoy` | 9 | 5.84 | 7 | 5 | 5 | 0.29 | 10.29 (0.00) | 15.11 | 0.54 (73/136) | 45 | 0.00 | 0.00 | 0.00 |
| `loader` | 52 | 2.99 | 51 | 23 | 5 | 2.25 | 45.86 (0.01) | 16.27 | 0.17 (143/846) | 311 | 0.06 | 0.06 | 0.01 |
| `night_shift` | 90 | 2.82 | 84 | 25 | 12 | 7.12 | 46.42 (0.01) | 23.20 | 0.07 (150/2088) | 517 | 0.80 | 0.80 | 0.53 |
| `office_worker` | 90 | 4.21 | 83 | 78 | 35 | 0.24 | 29.08 (0.01) | 4.91 | 0.24 (105/442) | 531 | 0.07 | 0.07 | 0.02 |
| `orderly` | 24 | 4.13 | 15 | 8 | 1 | 1.80 | 29.20 (0.04) | 14.75 | 0.32 (115/354) | 127 | 0.43 | 0.43 | 0.12 |
| `purist` | 23 | 3.91 | 19 | 18 | 13 | 0.21 | 4.05 (0.10) | 12.26 | 0.46 (130/282) | 67 | 0.31 | 0.34 | 0.24 |
| `reporter` | 12 | 4.23 | 10 | 6 | 2 | 6.10 | 33.70 (0.03) | 20.25 | 0.15 (36/243) | 71 | 0.00 | 0.00 | 0.00 |
| `ripperdoc` | 25 | 3.97 | 19 | 16 | 9 | 0.79 | 21.00 (0.03) | 8.36 | 0.42 (87/209) | 123 | 0.44 | 0.44 | 0.19 |
| `scav` | 15 | 3.74 | 8 | 6 | 3 | 0.75 | 5.25 (0.29) | 19.27 | 0.50 (144/289) | 10 | 0.60 | 0.60 | 0.71 |
| `sick` | 31 | 3.55 | 20 | 17 | 9 | 0.30 | 6.70 (0.86) | 18.35 | 0.48 (273/569) | 100 | 0.22 | 0.22 | 2.80 |
| `super` | 89 | 3.35 | 46 | 45 | 15 | 0.26 | 2.39 (0.85) | 18.05 | 0.28 (454/1595) | 519 | 0.19 | 0.19 | 0.14 |
| `sweeper` | 90 | 2.97 | 79 | 66 | 57 | 0.54 | 8.05 (0.02) | 9.62 | 0.25 (215/866) | 522 | 0.00 | 0.00 | 0.00 |
| `teacher` | 12 | 3.61 | 9 | 4 | 1 | 3.00 | 33.89 (0.02) | 11.08 | 0.17 (23/133) | 72 | 0.00 | 0.00 | 0.00 |
| `vendor` | 14 | 2.32 | 5 | 5 | 3 | 0.00 | 3.20 (1.00) | 18.86 | 0.56 (147/264) | 4 | 0.25 | 0.25 | 0.22 |
| `worker` | 90 | 3.01 | 75 | 73 | 37 | 0.17 | 2.32 (0.41) | 13.41 | 0.38 (460/1207) | 511 | 0.18 | 0.18 | 0.09 |
| `worker_friday` | 90 | 3.92 | 81 | 72 | 33 | 0.43 | 8.28 (0.20) | 14.40 | 0.27 (355/1296) | 497 | 0.29 | 0.35 | 0.62 |
| `all` | 1821 | 3.86 | 1427 | 997 | 460 | 1.78 | 22.06 (0.05) | 14.68 | 0.29 (7591/26630) | 8426 | 0.28 | 0.32 | 1.43 |

### 2k after phase 2 (the phase 2 tree, 2026-10-18; the same recipe)

Lives at work phases 1 and 2 on, the own Home gated on company (the fix round). Main `70e9589` read on the same day (phase 1 on, phase 2 off) is the "main" column of § 14's phase 2 table and reads, pooled: idle 4.05, married 1,489, never named 1,016, never co-located 473, named/wk 1.87, co-located 22.94 (0.06), HangOuts/wk 14.32, empty 0.27 (7,591/26,630). A home HangOut's company is the household present (bodies), so `empty` counts an empty flat as an empty spot.

| archetype | n | idle h/d | married | never named | never co-located | named/wk | co-located h/wk (Stat spouse) | HangOuts/wk | empty | shifts | Served-able | any signal | bodies/h |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `bouncer` | 53 | 5.34 | 43 | 18 | 3 | 7.38 | 35.60 (0.01) | 18.41 | 0.21 (206/965) | 243 | 0.71 | 0.98 | 0.47 |
| `builder` | 90 | 3.66 | 89 | 51 | 12 | 2.48 | 41.90 (0.02) | 13.78 | 0.08 (99/1240) | 538 | 0.00 | 0.00 | 0.00 |
| `cabbie` | 17 | 3.04 | 9 | 5 | 5 | 2.89 | 19.22 (0.00) | 15.12 | 0.44 (114/257) | 100 | 0.26 | 0.26 | 0.02 |
| `camp_warden` | 6 | 4.23 | 5 | 1 | 0 | 6.20 | 61.20 (0.01) | 13.33 | 0.36 (29/80) | 35 | 0.03 | 0.03 | 0.00 |
| `ceo` | 28 | 6.17 | 13 | 13 | 5 | 0.00 | 1.31 (0.94) | 3.39 | 0.35 (33/95) | 4 | 1.00 | 1.00 | 31.28 |
| `club_staff` | 90 | 4.62 | 82 | 47 | 12 | 6.18 | 30.82 (0.04) | 19.42 | 0.19 (332/1748) | 446 | 0.59 | 0.80 | 0.66 |
| `companion` | 6 | 8.86 | 4 | 2 | 1 | 7.25 | 39.25 (0.01) | 8.33 | 0.12 (6/50) | 30 | 0.07 | 1.00 | 0.01 |
| `cook` | 90 | 4.21 | 82 | 46 | 18 | 6.66 | 39.33 (0.02) | 19.08 | 0.18 (317/1717) | 523 | 0.04 | 0.04 | 0.01 |
| `courier` | 36 | 4.23 | 32 | 10 | 0 | 3.22 | 64.19 (0.00) | 15.44 | 0.09 (49/556) | 216 | 0.06 | 0.06 | 0.01 |
| `cultist` | 54 | 3.63 | 51 | 19 | 10 | 3.62 | 26.58 (0.00) | 14.91 | 0.14 (106/773) | 160 | 0.78 | 0.79 | 0.63 |
| `dancer` | 45 | 5.59 | 43 | 25 | 9 | 5.40 | 34.12 (0.03) | 16.49 | 0.23 (174/742) | 210 | 0.44 | 1.00 | 0.39 |
| `day_labourer` | 56 | 5.80 | 36 | 26 | 21 | 2.67 | 11.44 (0.27) | 18.45 | 0.29 (304/1033) | 122 | 0.11 | 0.11 | 0.08 |
| `elder` | 90 | 4.38 | 71 | 64 | 32 | 1.11 | 8.16 (0.58) | 17.52 | 0.41 (638/1563) | 356 | 0.21 | 0.22 | 1.19 |
| `exec` | 28 | 6.17 | 13 | 13 | 5 | 0.00 | 1.31 (0.94) | 3.39 | 0.35 (33/95) | 4 | 1.00 | 1.00 | 31.28 |
| `fabber` | 90 | 3.49 | 83 | 51 | 25 | 3.88 | 29.18 (0.02) | 14.78 | 0.18 (242/1330) | 531 | 0.09 | 0.09 | 0.01 |
| `fighter` | 24 | 4.64 | 19 | 4 | 0 | 6.54 | 38.83 (0.03) | 15.91 | 0.30 (115/381) | 110 | 0.99 | 0.99 | 0.14 |
| `gang_leader` | 12 | 3.12 | 10 | 7 | 6 | 2.20 | 4.20 (0.00) | 11.58 | 0.35 (49/139) | 40 | 0.50 | 0.50 | 0.29 |
| `gang_member` | 87 | 3.74 | 76 | 33 | 25 | 4.22 | 21.78 (0.00) | 15.05 | 0.20 (254/1295) | 273 | 0.73 | 0.73 | 0.58 |
| `guard` | 90 | 4.10 | 70 | 51 | 29 | 1.50 | 5.20 (0.04) | 13.82 | 0.27 (328/1233) | 534 | 0.72 | 0.72 | 19.61 |
| `gun` | 74 | 3.85 | 56 | 41 | 20 | 2.11 | 8.36 (0.15) | 12.11 | 0.32 (290/896) | 286 | 0.22 | 0.25 | 0.97 |
| `homeless` | 43 | 5.23 | 14 | 10 | 7 | 0.36 | 2.00 (0.36) | 16.93 | 0.40 (292/728) | 61 | 0.44 | 0.44 | 0.33 |
| `joytoy` | 9 | 6.26 | 6 | 4 | 4 | 10.10 | 38.46 (0.00) | 22.58 | 0.07 (12/184) | 39 | 0.05 | 0.05 | 0.01 |
| `loader` | 55 | 3.30 | 54 | 19 | 7 | 5.02 | 48.31 (0.01) | 16.24 | 0.12 (109/893) | 324 | 0.06 | 0.06 | 0.01 |
| `night_shift` | 90 | 3.11 | 86 | 32 | 18 | 7.71 | 41.01 (0.01) | 22.99 | 0.09 (194/2054) | 520 | 0.85 | 0.85 | 0.52 |
| `office_worker` | 90 | 3.80 | 77 | 53 | 31 | 3.77 | 18.65 (0.02) | 15.32 | 0.15 (203/1379) | 525 | 0.30 | 0.30 | 0.10 |
| `orderly` | 20 | 3.69 | 16 | 7 | 4 | 6.94 | 28.00 (0.03) | 16.20 | 0.28 (90/324) | 113 | 0.54 | 0.54 | 0.17 |
| `purist` | 16 | 3.54 | 15 | 7 | 4 | 5.00 | 12.07 (0.01) | 13.38 | 0.08 (17/214) | 59 | 0.69 | 0.69 | 0.62 |
| `reporter` | 12 | 4.67 | 10 | 6 | 1 | 6.50 | 33.90 (0.04) | 20.50 | 0.18 (44/246) | 68 | 0.04 | 0.04 | 0.00 |
| `ripperdoc` | 16 | 4.22 | 14 | 7 | 1 | 2.44 | 27.52 (0.05) | 8.60 | 0.29 (38/130) | 87 | 0.39 | 0.39 | 0.23 |
| `scav` | 25 | 4.54 | 16 | 9 | 5 | 5.19 | 21.44 (0.03) | 18.24 | 0.36 (166/456) | 59 | 0.64 | 0.64 | 0.45 |
| `sick` | 35 | 3.93 | 25 | 22 | 11 | 0.92 | 10.24 (0.64) | 15.83 | 0.44 (243/554) | 107 | 0.36 | 0.36 | 2.44 |
| `super` | 90 | 3.67 | 50 | 48 | 17 | 0.36 | 2.90 (0.76) | 16.99 | 0.29 (444/1529) | 523 | 0.22 | 0.22 | 0.20 |
| `sweeper` | 90 | 3.31 | 83 | 64 | 53 | 1.52 | 15.34 (0.00) | 10.78 | 0.24 (232/970) | 532 | 0.11 | 0.11 | 0.02 |
| `teacher` | 12 | 4.26 | 10 | 5 | 1 | 4.10 | 40.20 (0.02) | 9.25 | 0.20 (22/111) | 68 | 0.00 | 0.00 | 0.00 |
| `vendor` | 25 | 3.14 | 12 | 9 | 3 | 3.58 | 15.00 (0.53) | 13.92 | 0.33 (116/348) | 16 | 0.44 | 0.44 | 0.26 |
| `worker` | 90 | 3.59 | 79 | 76 | 36 | 0.27 | 4.37 (0.26) | 14.31 | 0.38 (488/1286) | 515 | 0.16 | 0.16 | 0.84 |
| `worker_friday` | 90 | 4.40 | 78 | 58 | 37 | 2.14 | 15.11 (0.04) | 13.87 | 0.28 (343/1236) | 480 | 0.28 | 0.41 | 0.16 |
| `all` | 1896 | 4.12 | 1547 | 973 | 483 | 3.55 | 23.80 (0.04) | 15.38 | 0.24 (6831/29029) | 8933 | 0.33 | 0.37 | 1.57 |

### 2k after phase 3: the civic rows (the phase 3 tree, 2026-10-18; the same recipe)

Main `899acc2` (phases 1 and 2 on) read on the same day beside the phase 3 tree, the four civic archetypes and the guards (no `jailer` archetype exists in the probe; the Jail's row shows through the guards'), with the phase 1 columns (`met`: the share of shifts with an encounter; encounters a shift).

| archetype | city | n | idle h/d | married | never named | never co-located | named/wk | co-located h/wk (Stat spouse) | HangOuts/wk | empty | shifts | Served-able | any signal | bodies/h | met | enc/shift |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `teacher` | main `899acc2` | 12 | 4.26 | 10 | 5 | 1 | 4.10 | 40.20 (0.02) | 9.25 | 0.20 (22/111) | 68 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 |
| `teacher` | phase 3 | 12 | 3.84 | 10 | 6 | 2 | 1.08 | 19.60 (0.05) | 8.95 | 0.36 (36/101) | 65 | 0.03 | 0.03 | 0.00 | 0.45 | 0.60 |
| `camp_warden` | main `899acc2` | 6 | 4.23 | 5 | 1 | 0 | 6.20 | 61.20 (0.01) | 13.33 | 0.36 (29/80) | 35 | 0.03 | 0.03 | 0.00 | 0.00 | 0.00 |
| `camp_warden` | phase 3 | 6 | 3.51 | 6 | 1 | 0 | 5.00 | 44.50 (0.01) | 11.00 | 0.15 (10/66) | 33 | 0.09 | 0.09 | 0.01 | 0.79 | 1.36 |
| `super` | main `899acc2` | 90 | 3.67 | 50 | 48 | 17 | 0.36 | 2.90 (0.76) | 16.99 | 0.29 (444/1529) | 523 | 0.22 | 0.22 | 0.20 | 0.00 | 0.00 |
| `super` | phase 3 | 88 | 4.40 | 59 | 53 | 25 | 0.34 | 2.61 (0.65) | 16.58 | 0.30 (438/1455) | 516 | 0.26 | 0.26 | 0.22 | 0.97 | 27.80 |
| `guard` | main `899acc2` | 90 | 4.10 | 70 | 51 | 29 | 1.50 | 5.20 (0.04) | 13.82 | 0.27 (328/1233) | 534 | 0.72 | 0.72 | 19.61 | 0.01 | 0.01 |
| `guard` | phase 3 | 90 | 4.15 | 66 | 41 | 22 | 2.04 | 9.41 (0.04) | 13.76 | 0.25 (312/1228) | 522 | 0.73 | 0.73 | 23.25 | 0.14 | 0.17 |
| `all` | main `899acc2` | 1896 | 4.12 | 1547 | 973 | 483 | 3.55 | 23.80 (0.04) | 15.38 | 0.24 (6831/29029) | 8933 | 0.33 | 0.37 | 1.57 | 0.07 | 0.15 |
| `all` | phase 3 | 1806 | 4.24 | 1473 | 959 | 473 | 3.29 | 23.85 (0.04) | 15.09 | 0.24 (6432/27186) | 8492 | 0.29 | 0.34 | 2.02 | 0.32 | 2.22 |

### 2k after phase 4a: the street rows (the phase 4a tree, 2026-10-18; `tools/phase_measure.py`)

Main `1300584` against the phase 4a tree, `shadow --lives` 2k, seeds 42-44 pooled, day 19, 7 days (the measure's columns only; the full recipe at the gate):

| archetype | n | idle h/d | shifts | Served-able | met | enc/shift |
|---|---|---|---|---|---|---|
| `cabbie` main / 4a | 18 / 18 | 2.62 / 3.52 | 93 / 91 | 0.25 / 0.11 | 0.12 / 0.14 | 0.24 / 0.14 |
| `courier` main / 4a | 36 / 36 | 3.26 / 3.61 | 215 / 216 | 0.05 / 0.00 | 0.32 / 0.49 | 0.33 / 1.50 |
| `loader` main / 4a | 52 / 52 | 3.20 / 2.31 | 292 / 303 | 0.22 / 0.06 | 0.00 / 0.81 | 0.00 / 0.81 |
| `vendor` main / 4a | 20 / 16 | 2.72 / 2.73 | 14 / 4 | 0.14 / 1.00 | 0.21 / 0.25 | 0.21 / 0.50 |
| `homeless` main / 4a | 27 / 34 | 5.39 / 5.58 | 57 / 30 | 0.47 / 0.57 | 0.26 / 0.13 | 0.32 / 0.17 |
| `day_labourer` main / 4a | 59 / 65 | 6.66 / 5.87 | 91 / 98 | 0.30 / 0.20 | 0.43 / 0.35 | 1.37 / 0.62 |
| `all` main / 4a | 1806 / 1852 | 4.24 / 4.08 | 8492 / 8639 | 0.29 / 0.30 | 0.32 / 0.38 | 2.22 / 2.26 |

The cabbie's idle hours are his stand-by at the rank read as work only while `standing_by` holds (a Rest in the shift's post hours or off the rank reads idle); his Served-able fell with the rush shifts (06-16, 16-24: fewer bodies in the Garage). The vendor, homeless and day-labourer rows are not 4a's (the vendor's regulars are 4b's); their moves are the city's.

### 20k, seed 42, day 20, 7 days (5 picks an archetype in one world)

| archetype | n | idle h/d | married | never named | never co-located | named/wk | co-located h/wk (Stat spouse) | HangOuts/wk | empty | shifts | Served-able | any signal | bodies/h |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `arms_dealer` | 5 | 2.56 | 4 | 4 | 2 | 0.00 | 2.50 (1.00) | 15.00 | 0.44 (33/75) | 9 | 0.67 | 0.67 | 127.80 |
| `bouncer` | 5 | 3.11 | 5 | 5 | 0 | 0.00 | 25.60 (0.05) | 27.60 | 0.59 (82/138) | 20 | 0.90 | 1.00 | 0.35 |
| `builder` | 5 | 4.20 | 5 | 5 | 4 | 0.00 | 0.40 (1.00) | 22.80 | 0.44 (50/114) | 20 | 0.05 | 0.05 | 0.02 |
| `buyer` | 5 | 4.20 | 1 | 1 | 1 | 0.00 | 0.00 (0.00) | 5.60 | 0.36 (10/28) | 2 | 0.50 | 0.50 | 0.00 |
| `cabbie` | 5 | 4.10 | 3 | 2 | 2 | 0.33 | 1.33 (0.00) | 15.00 | 0.44 (33/75) | 27 | 0.11 | 0.11 | 0.04 |
| `ceo` | 5 | 5.33 | 2 | 2 | 1 | 0.00 | 0.50 (1.00) | 1.60 | 0.25 (2/8) | 6 | 1.00 | 1.00 | 323.06 |
| `club_staff` | 5 | 4.67 | 5 | 5 | 1 | 0.00 | 5.00 (0.32) | 27.60 | 0.45 (62/138) | 25 | 0.80 | 0.88 | 1.48 |
| `companion` | 5 | 4.96 | 2 | 2 | 0 | 0.00 | 2.00 (1.00) | 25.20 | 0.66 (83/126) | 25 | 0.16 | 0.52 | 0.05 |
| `cook` | 5 | 2.01 | 4 | 4 | 3 | 0.00 | 0.50 (1.00) | 30.40 | 0.66 (101/152) | 26 | 0.00 | 0.00 | 0.00 |
| `courier` | 5 | 1.98 | 5 | 5 | 2 | 0.00 | 1.80 (1.00) | 15.00 | 0.23 (17/75) | 30 | 0.00 | 0.00 | 0.00 |
| `cultist` | 5 | 2.22 | 4 | 3 | 1 | 0.65 | 28.25 (0.00) | 16.67 | 0.63 (43/68) | 18 | 0.61 | 0.61 | 0.25 |
| `dancer` | 5 | 4.71 | 5 | 5 | 1 | 0.00 | 18.00 (0.03) | 21.00 | 0.52 (55/105) | 23 | 0.65 | 1.00 | 1.83 |
| `day_labourer` | 5 | 8.63 | 4 | 4 | 0 | 0.00 | 14.00 (1.00) | 21.60 | 0.54 (58/108) | 2 | 0.00 | 0.00 | 0.00 |
| `dealer` | 5 | 3.92 | 4 | 4 | 2 | 0.00 | 0.50 (1.00) | 9.20 | 0.67 (31/46) | 7 | 0.29 | 0.29 | 0.27 |
| `elder` | 5 | 4.09 | 2 | 2 | 0 | 0.00 | 2.50 (1.00) | 25.40 | 0.53 (67/127) | 12 | 0.00 | 0.00 | 0.00 |
| `exec` | 5 | 7.86 | 1 | 1 | 1 | 0.00 | 0.00 (0.00) | 8.60 | 0.56 (24/43) | 0 | 0.00 | 0.00 | 0.00 |
| `fabber` | 5 | 3.52 | 5 | 5 | 1 | 0.00 | 2.60 (0.46) | 13.00 | 0.57 (37/65) | 26 | 0.19 | 0.19 | 0.04 |
| `fighter` | 5 | 6.15 | 3 | 2 | 1 | 2.33 | 13.33 (0.05) | 5.80 | 0.17 (5/29) | 21 | 1.00 | 1.00 | 1.82 |
| `fixer` | 5 | 2.59 | 4 | 4 | 3 | 0.00 | 0.50 (1.00) | 13.00 | 0.45 (29/65) | 28 | 0.39 | 0.39 | 0.10 |
| `foreman` | 5 | 2.54 | 5 | 5 | 3 | 0.00 | 0.80 (1.00) | 14.80 | 0.34 (25/74) | 30 | 0.03 | 0.03 | 0.00 |
| `forger` | 5 | 3.92 | 2 | 2 | 1 | 0.00 | 0.50 (1.00) | 4.40 | 0.36 (8/22) | 30 | 0.03 | 0.03 | 0.01 |
| `gang_leader` | 5 | 0.88 | 5 | 4 | 0 | 0.40 | 6.20 (0.48) | 12.00 | 0.67 (40/60) | 9 | 0.33 | 0.33 | 0.60 |
| `gang_member` | 5 | 5.45 | 3 | 2 | 1 | 2.33 | 10.67 (0.06) | 12.60 | 0.25 (16/63) | 15 | 0.07 | 0.07 | 0.01 |
| `guard` | 5 | 3.97 | 3 | 3 | 3 | 0.00 | 0.00 (0.00) | 9.40 | 0.04 (2/47) | 29 | 1.00 | 1.00 | 325.75 |
| `gun` | 5 | 6.22 | 4 | 4 | 3 | 0.00 | 2.25 (1.00) | 18.80 | 0.43 (40/94) | 14 | 0.07 | 0.07 | 0.02 |
| `homeless` | 5 | 7.47 | 1 | 1 | 0 | 0.00 | 7.00 (1.00) | 17.80 | 0.42 (37/89) | 0 | 0.00 | 0.00 | 0.00 |
| `joytoy` | 5 | 4.29 | 5 | 3 | 2 | 0.80 | 6.80 (0.09) | 26.40 | 0.42 (55/132) | 25 | 0.00 | 0.00 | 0.00 |
| `loader` | 5 | 2.53 | 4 | 2 | 2 | 6.50 | 36.00 (0.00) | 14.00 | 0.21 (15/70) | 30 | 0.00 | 0.00 | 0.00 |
| `madam` | 5 | 2.44 | 1 | 1 | 0 | 0.00 | 72.00 (0.00) | 15.00 | 0.63 (47/75) | 22 | 0.14 | 0.68 | 0.45 |
| `night_shift` | 5 | 2.36 | 5 | 5 | 4 | 0.00 | 0.60 (1.00) | 34.40 | 0.62 (107/172) | 30 | 0.70 | 0.70 | 0.47 |
| `office_worker` | 5 | 2.48 | 3 | 3 | 1 | 0.00 | 2.00 (1.00) | 18.00 | 0.66 (59/90) | 30 | 0.30 | 0.30 | 0.30 |
| `orderly` | 5 | 2.65 | 3 | 3 | 2 | 0.00 | 1.00 (1.00) | 17.20 | 0.29 (25/86) | 30 | 0.47 | 0.47 | 0.02 |
| `purist` | 5 | 5.71 | 4 | 4 | 3 | 0.00 | 0.25 (0.00) | 18.00 | 0.56 (50/90) | 6 | 0.00 | 0.00 | 0.00 |
| `reporter` | 5 | 3.25 | 5 | 4 | 2 | 0.40 | 2.60 (0.38) | 22.80 | 0.39 (44/114) | 30 | 0.20 | 0.20 | 0.20 |
| `ripperdoc` | 5 | 3.72 | 5 | 5 | 2 | 0.00 | 1.60 (1.00) | 5.00 | 0.28 (7/25) | 30 | 0.33 | 0.33 | 0.25 |
| `runner` | 5 | 6.11 | 1 | 1 | 0 | 0.00 | 51.00 (0.00) | 7.54 | 0.24 (8/33) | 19 | 0.53 | 0.53 | 130.03 |
| `scav` | 5 | 3.59 | 2 | 2 | 1 | 0.00 | 5.00 (0.00) | 10.80 | 0.13 (7/54) | 17 | 0.35 | 0.35 | 114.71 |
| `sick` | 5 | 3.77 | 4 | 4 | 1 | 0.00 | 9.25 (1.00) | 18.40 | 0.52 (48/92) | 18 | 0.61 | 0.61 | 0.41 |
| `street_worker` | 5 | 6.20 | 3 | 3 | 0 | 0.00 | 22.33 (1.00) | 28.60 | 0.57 (82/143) | 0 | 0.00 | 0.00 | 0.00 |
| `super` | 5 | 3.06 | 2 | 2 | 0 | 0.00 | 3.00 (1.00) | 20.40 | 0.31 (32/102) | 30 | 0.37 | 0.37 | 0.19 |
| `sweeper` | 5 | 1.65 | 4 | 4 | 4 | 0.00 | 0.00 (0.00) | 7.40 | 0.19 (7/37) | 30 | 0.83 | 0.83 | 0.20 |
| `teacher` | 5 | 3.16 | 4 | 3 | 1 | 0.75 | 19.25 (0.05) | 14.80 | 0.55 (41/74) | 30 | 0.00 | 0.00 | 0.00 |
| `vendor` | 5 | 2.37 | 2 | 2 | 1 | 0.00 | 3.50 (1.00) | 24.60 | 0.67 (83/123) | 3 | 0.00 | 0.00 | 0.00 |
| `worker` | 5 | 2.20 | 4 | 4 | 1 | 0.00 | 2.25 (1.00) | 15.20 | 0.32 (24/76) | 28 | 0.11 | 0.11 | 0.04 |
| `worker_friday` | 5 | 2.18 | 4 | 4 | 4 | 0.00 | 0.00 (0.00) | 13.80 | 0.33 (23/69) | 26 | 0.54 | 0.54 | 0.21 |
| `all` | 228 | 3.88 | 159 | 148 | 70 | 0.34 | 7.05 (0.28) | 16.42 | 0.47 (1766/3719) | 906 | 0.33 | 0.37 | 19.71 |

---

## 14. Implemented: deviations

### Phase 1 (2026-10-18): the encounter, venues and the Clinic

Built as § 1, § 2.1 and § 2.2's ripperdoc and orderly, under `[lives] enabled` (master, `CITYSIM_LIVES_OFF=1`) and `[service] venues`; `systems/service.rs`; SAVE_VERSION 35. Deviations from the spec text:

- **The CSV home.** The counters live in `Jobs2Cols` (`served_<kind>` for every `ServeKind`, `served_waited`, `serve_shifts`, `cooked`, `staff_meals`, `regulars_made`, `door_refusals`, `ejections`, `flow_madam_cut`), not `stats.current.jobs`: the Jobs & lives columns are that group.
- **The encounter resolves at the arrival's minute**, not at an hourly drain of a queue. `World::queues` is replaced by `World::service_room` (unsaved): each server's encounters taken this hour against its row's `per_hour`; `arrive` picks, among the servers on duty with room, the lowest `hash(PURPOSE_SERVE, place, served, hour)` (D6) and serves at once, FIFO by arrival; nobody on duty or every counter full is the "waited" outcome (`served_waited`, no event, no memory). Why: the first build drained hourly and lost 62 % of visits as waited, half of them at shift boundaries (a 17:40 drink read at 18:00 against a 09-18 bartender) and the Statistical evening's one-hour batch (21:00), and with them 63 % of the tips (D4: the city's money circuit is not this milestone's subject). Presence is now read when the visitor is there, as `tipped_for` read it. `service::hourly` only prunes the room ledger and queues the madam's `Staff` encounters.
- **The tip** is paid in `service::serve` to the server named when its trade row is `tipped` (dancer, companion, brothel worker); when the hash named an untipped server (a Club's Host, a StripClub's barkeep), the tip goes to the tipped worker on duty the hash names (`PURPOSE_TIP`, room not counted): the Host serves the visit, the dancer is tipped, and the flow main paid is re-routed, not removed (D4). The first build paid the tip only to the encounter's server and lost 62 % of the tips (a Club's 10 Hosts against 3 dancers). A visit at an hour with nobody tipped on duty pays no tip, as `tipped_for` paid none; a rest day or a Statistical no-show (`present`) now withholds it where `tipped_for` did not. **The madam's cut** is `round(0.3 × tip)`: at the 2k Brothel's 1-coin tips it rounds to 0, so `flow_madam_cut` reads 0 over 42-47 × 120 days (the mechanism is unit-tested at a 20-coin price); a Spire brothel's 2-coin tip is the first that pays her. Printed, open for the gate.
- **Who is behind the counter.** `tests/service.rs::service_probe` (diag) prints per venue kind the share of building-hours with a server on duty and why not: the Bar's Bartender and the Arcade's Attendant hold `[world] shift_day` (09-18) while the drinking and the Statistical evening are 18-24 (`demography::hire` gives the evening shift to Hosts, Fighters, Croupiers and Concierges only), and Clubs trade by day with evening Hosts. Not phase 1's to move (a shift change moves wages, leisure and the table); printed, open for the gate.
- **The madam's `Staff` encounter** fires at the first hour of each worker's shift (the hourly pass has no shift-start hook and both shifts end together), not "every shift" at its end. The madam is not yet named in the house's `Hire`/`Layoff` text (`industry::foreman_note` reads the industries' foreman; the Brothel is a venue): left to phase 6's foreman work.
- **`stabilise_no_orderly_mult` is not built**: `wounded::on_stabilise` has no save roll to multiply (a carried-in body is always stabilised at the Clinic); the orderly is the triage by presence only (the `Patient` queue, a memory, the regulars). The key is not in config (nothing would read it).
- **The regular's first-meeting affinity** is a hashed `0.1 + 0.3 × u` (`PURPOSE_REGULAR`), not `colocation`'s `first_affinity_with` (which draws the world stream).
- **The door refusal** is read at the exec's failed start of an `Enjoy`/`EatOut`/`Gamble` inside the venue against `door_ok` (a pure read), not inside `door_ok` itself (which the venue scorer also calls). The scorer already filters by `door_ok`, so refusals are rare (a state change between the pick and the arrival).
- **The ejection** runs after the fight resolves (the Assault is raised first): the attacker's plan is aborted and the attacker put at the door; the fight is not interrupted mid-way.
- **The chip host's encounter** runs and writes the guest's memory; the host's is dropped by `World::remember` (D5, as designed).
- **A Clinic's over-the-counter sale** (`assets::buy_noted` at a Clinic, a non-implant) queues a `Patient`; an implant's install queues once, in `chrome::installed_now` (the gang's install included).
- **The parity row** (`tests/lod.rs`): encounters per staffed shift of a serving row, pooled over the six seeds, is a **printed FINDING, not the assert § 6 planned**: it read Full 0.707 against Statistical 0.877 (27,684 / 26,694 staffed shifts; 24 % apart). The pass is world-side, but § 1.2's presence rule is not symmetric: a Full server must be inside the building, so its meal, walk or late commute loses the hour's arrivals, where a Statistical server is credited by `lod::shift_attended`. Demoted with the reason (CLAUDE.md rule 2), not widened; the gate decides whether a Full server on shift is credited while momentarily out (which would make the row assertable). The test's `actor share law0/law1` rows were already failing on main (SHADOW_V4 "Left open" 1).

The Statistical mirror of each mechanism is § 6's row, unchanged: the hooks fire from `stat_visit`, `stat_shop` (through `assets::buy`), the Statistical detox/treat paths and `lod::stat_work`'s shift end as from the bodies' paths; a Statistical server is counted present on `lod::shift_attended`.

**Measured, phase 1 against main `e5525ee`** (42-47 × 120 days, means a day from day 30; 20k seed 42 × 30 days from day 5; the phase 1 report has the per-seed totals):

| | main 2k | phase 1 2k | 20k main | 20k phase 1 |
|---|---|---|---|---|
| encounters served a day (`served_*`) | — | 126 visits, 6.3 patients, 4.2 staff, 0 attendees | — | 1,190 / 43 / 51 / 0 |
| waited (nobody on duty, or every counter full) | — | 214 | — | 2,140 |
| staffed shifts of a serving row | — | 183 | — | 1,570 |
| regulars made | — | 0.87 | — | 43 |
| staff meals (cooks) | — | 39 | — | 340 |
| door refusals / ejections | — | 0.013 / 0.004 | — | 0.16 / 0 |
| madam's cut | — | 0 (rounds to 0 at a 1-coin tip) | — | 0 |
| tips | 104 | 89 (−15 %: `present` withholds a rest day's and a no-show's tip; `tipped_for` did not) | 874 | 781 (−11 %) |
| strip / brothel / companion visits | 8.7 / 16.1 / 3.4 | 8.6 / 16.3 / 3.2 | 105 / 106 / 25 | 110 / 113 / 25 |
| Murders (120 d, per seed 42-47) | 94, 61, 112, 59, 111, 28 (mean 78) | 85, 89, 197, 148, 19, 94 (mean 105) | — | — |
| deaths by violence a day | 1.14 | 1.45 | 5.5 | 4.1 |
| J24 employed share / J25 homeless share (seed 42) | 0.770 / 0.020 | 0.764 / 0.023 | — | — |
| thefts / arrests a day | 2.86 / 25.0 | 2.72 / 24.0 | 39.4 / 125 | 37.6 / 119 |
| ticks/s, alternating A/B, seed 42 × 30 d, 3 rounds each | 4,735 / 5,008 / 5,300 | 6,383 / 4,358 / 6,301 (no regression; the batch runs' −24 % was six runs in parallel) | 223 | 225 |
| save size, 30 d seed 42 (`--save-at 43200`, no report) | 12,557,114 B | 12,398,418 B on (the `regulars` ledger is 8 entries a server; the rest is the city's own drift); off 12,557,114 B, the version token aside | | |

Readings. The Murders swing both ways by seed (46: 111 → 19; 44: 112 → 197) with the mean up a third: the encounter's edges (a regular is an Acquaintance, every repeat visit `interacted`) and the re-routed tips move a chaotic city's trajectory; no mechanism of the phase raises violence directly; printed, not retuned (the harsh-city decision stands). 63 % of arrivals wait because nobody is on duty: the Bar's Bartender and the Arcade's Attendant hold the day shift (09-18) while the drinking and the Statistical evening are 18-24, the NoodleBar's Cook likewise (`cooked` 0.2 a day against 2-3 meals out), and Clubs trade by day with evening Hosts (`service_probe`); the gate decides whether the venue shifts move. No bout has a bettor, so the fighter's `Attendee` row never fires; the madam's cut rounds to 0 at the 2k Brothel's 1-coin tips. The lives probe's new column (`met`: shifts with an encounter) reads 0.40 for club staff, 0.61 dancers, 0.86 companions, 0.73 madams, 0.22 orderlies, 0.14 ripperdocs, 0.01 cooks, 0 bouncers and fighters (`shadow --lives`, 2k, day 19).

### Phase 2 (2026-10-18): the evening and the household

Built as § 3.1-3.4, § 3.5's kin and elder visits and § 3.6's work and household deeds, under `[household] enabled` (`CITYSIM_HOUSEHOLD_OFF=1`, under the `[lives]` master); `systems/household.rs`; SAVE_VERSION 36. Deviations from the spec text:

- **The deed names.** `Deed::Married` already existed (posted by `social::marry`), so "Wed" is done; `Deed::Hired` is M16a's contract record, so the work deeds are `Deed::Employed` (posted in `demography::hire`) and `Deed::LaidOff` (in `economy::dismiss_as` for a `LaidOff` event); `Deed::Born` is posted by `household::on_birth` (actor the mother, object the child). Their `reach0`/`deed_sal` rows are 0.2-0.3 and `deed_sev` 0 (no grudge); the three keys are left out of a save at 0 and zeroed by the off switches so the off save is byte-identical.
- **The home spot** (`SpotKind::Home(b)`) is added per agent in `leisure::spots_near`, never in `World::spots`; the agent's own Home is offered at any distance (the travel term weighs the walk; a Home past `spot_walk_cap_tiles` was never offered in the first build, so a worker far from home never saw it) **and only with company to be had there** (`household::company_due`: a household body inside or a child at home, or a resident whose habit within the hour is the Home; the first measured build offered the own Home unconditionally and 140 of its 180 home HangOuts a day were 21-minute checks of an empty flat, 06:22 before the shift, a planner gap fixed before the gate's regen rather than learned into the table), a friend's within the cap under the same rule. The HangOut's plan walks to the Home (`GoTo(Home)`, or `GoTo(TargetHome)` for a friend's) and runs inside it; `UnwindPick.venue` marks a home HangOut; `can_start(HangOut)` allows it inside that Home. `hangout_done`'s company inside is the household present (bodies), not the street registry; the lives probe (`shadow --lives`) reads that company too.
- **No `home_from` key exists**; the spec's "past 18:00 the travel term makes the Home near" is left to the travel term, as the brief asked.
- **After an empty leave, company only.** § 3.3's "the next pick runs at once" re-planned a lonely corner at once: the first build doubled the city's HangOuts (355 → 775 a day) with three in four left empty, and the lives probe read the empty share 0.27 → 0.73 (nobody waited, so nobody met). Now, for `empty_memory_hours` after leaving a spot empty, `best_spot` offers only spots with company now (a body hanging out there, a resident inside the Home) and, with none, no HangOut (`household::company_only`, `company_at`); the empty-spot memory's penalty stands beside it.
- **Throughput**: the household's business is read as one set a midnight pass (`household::business_holders`, one building scan) for the hiring pool, not a building scan per candidate (the first build halved the 20k ticks/s); friends' Homes are offered only to the scored pick (`best_spot_kind`), since each reads a contact's 14-day habit.
- **Reunite** moves the unhoused spouse in when the housed one's Home has room, at midnight before `ownership`'s `rehouse`; with no room nothing is queued (`rehouse` has no queue; it already moves a homeless pair together).
- **The carer's damping** multiplies the scored goals after `score_goal` (every goal but Work, Sleep, Eat, Flee, `night_from`..`night_to`), not the considerations; the carer's away spots are × `newborn_away_mult` in `best_spot`. The newborn's keep goes on the Home's `child_food_debt` at midnight (the child's own draw).
- **The household's business**: `business_of` scans the buildings at midnight (no owner index exists; scan-ok); the owner's household takes the business's open post first in `job_search_pass_with` (before the graded or pooled pick); an adult whose household business took coins in the last `abandon_days` is out of the labour pool (`HirePool::pick`, `pick_candidate`); the layoff order (`past_cap_pass`, `newest_hire`) puts the owner's household after every non-member. "The same tick" of the heir is read as "while the business takes".
- **Visits** (`GoalKind::Visit`, appended; `World::visits` the day's pending visits): the trigger memories are the visitor's own `SawCrime` (Assault, the kin the object) and heard `Assaulted`/`Arrested` rumours about the kin in the last day, plus `World::wounded` for the downed; the walk is `GoTo(Victim)` (the kin's building: the Clinic's ward, the Home, the Precinct) and the visiting hour is a `Rest` there (no `Wait` action exists), its completion read in `actions::on_complete`'s `Rest` arm by the plan's goal; the hour of the day is hashed in `[visit_hour_from, visit_hour_to)`; a Statistical visitor is credited at that hour. An `EventKind::Visited` (appended) marks each visit. The Camp answer is phase 3's.
- **The CSV home** is `Jobs2Cols` again (`home_hangouts`, `friend_hangouts`, `home_with_spouse`, `empty_leaves`, `home_empty_leaves`, `reunited`, `kin_visits`, `elder_visits`, `carers`).
- **The spot tests** live in `tests/household.rs` with the rest of the phase (not `tests/leisure.rs`); the two probes in `tests/person.rs`.

The Statistical mirror (§ 6): reunite, the newborn flags and `visits_due` are midnight rules for every tier; a Statistical visitor is credited at the hour; the home evening moves the Full tier's chat and social rates, learned at the gate's one regen (D14, no regen here).

Measured (phase 2 against main `70e9589`, the same binary recipe as phase 1's table; `scratchpad/p2/phase2_report.md` has the per-seed rows and the first build's numbers). The first measured build offered the own Home unconditionally and read never-named 1,016 → 988, empty 0.27 → 0.30, idle 4.05 → 4.54 and 140 of its 180 home HangOuts a day left empty: the fix round below ("company to be had") is the tree that ships.

| | 2k main | 2k phase 2 | 20k main | 20k phase 2 (first build) |
|---|---|---|---|---|
| HangOuts a day | 355 | 361 (+1.5 %) | 3,190 | 3,430 (+8 %) |
| of them at a Home / at a friend's / at home with the spouse | — | 124 / 44 / 25 | — | 1,920 / 371 / 548 |
| HangOut company (mean others at the spot) | 1.10 | 1.27 | 1.12 | 2.27 |
| empty spots left early a day, of them the own Home | — | 127, of them 22.6 (18 % of the home HangOuts) | — | 1,010 / — |
| couples reunited (over the run) | — | 98 (42-47) | — | 42 |
| kin visits / elder visits a day | — | 7.3 / 30.7 | — | 6.1 / 370 |
| newborns' carers (over the run) | — | 178 (42-47) | — | 77 |
| births a day | 0.224 | 0.254 | — | — |
| homeless a day | 39.1 | 41.0 (+5 %) | 320 | 309 |
| evictions / thefts / arrests a day | 2.38 / 2.72 / 24.0 | 2.64 / 2.84 / 24.4 | — / 37.6 / 119 | — |
| Murders (120 d, per seed 42-47) | 85, 89, 197, 148, 19, 94 (mean 105) | 74, 115, 132, 132, 29, 196 (mean 113) | — | — |
| deaths by violence a day | 1.45 | 1.60 | 4.12 | 3.84 |
| J24 employed share / J25 homeless share (seed 42) | 0.764 / 0.023 | 0.759 / 0.025 | — | — |
| tips a day | 88.7 | 87.5 | — | — |
| ticks/s, alternating A/B, seed 42 × 30 d, 6 rounds each (first build) | 6,373 / 6,182 / 6,287 / 6,312 / 6,587 / 6,252 | 5,663 / 5,948 / 6,238 / 5,970 / 5,372 / 5,242 (−9 % on the means; the phase 2 binary with the household *off* read 6,338 / 5,706 / 5,133 in the same rounds, so the box drifted −19 % under the same binary and config: the A/B is noise-bound; see the profile) | 251 (10 d) | 239 (−5 %) |
| `--features prof`, seed 42 days 10-30, household on against off on the phase 2 binary (mean of 2; first build) | 5.05 s | 5.20 s (+2.9 %: `tick::exec` +0.14 s, the HangOut's household scan and the extra HangOuts; `household` midnight 0.049 s over 20 days; `tick::living` +0.04 s, `household::hourly`); the fix adds one habit read per bodied resident at the pick | | |
| save size, 30 d seed 42 (`--save-at 43200`, no report) | 12,398,418 B | 12,777,695 B on (+3 %: `visits`, `empty_spots`, the newborn flags and the city's own drift); off 12,398,418 B, one byte (the version) aside | | |

The lives probe (`shadow --lives`, 2k, seeds 42-44 pooled, day 19, 7 days; main → first build → the fix): all picks idle 4.05 → 4.54 → 4.12 h/d; never named the spouse 1,016 → 988 → 973 (of 1,489 / 1,489 / 1,547 married), never co-located awake 473 → 477 → 483; named 1.87 → 3.34 → 3.55 h a married-agent-week; co-located 22.9 → 23.6 → 23.8 h/wk; HangOuts 14.3 → 16.2 → 15.4 a week, empty 0.27 → 0.30 → **0.24** (6,831/29,029). By archetype (main → fix): office workers 5.0 → 15.3 HangOuts a week, empty 0.23 → 0.15, named 0.30 → 3.77; fabbers named 0.81 → 3.88, empty 0.33 → 0.18; cooks empty 0.22 → 0.18, named 5.27 → 6.66; gang members empty 0.37 → 0.20 and named 1.17 → 4.22; guards named 0.37 → 1.50, co-located 2.4 → 5.2 h; elders idle 4.60 → 4.38, named 0.07 → 1.11; workers named 0.09 → 0.27 but empty 0.30 → 0.38 (the day worker's spouse is out when the evening pick lands; the 21-minute checks that remain are theirs). The after-rows are § 13's second 2k table.

Readings. The device moved where the spec aimed it once the own Home was gated on company: the spouse is *named* twice as often a week, the lonely-corner share fell below main's (0.27 → 0.24) with the "company only" window and the gated Home together, and the idle hours came back to main's. The never-co-located count did not move: a couple's hours under one roof were already there (the probe counts them awake in the same building), and the home spot gives them something to do in them. What is left at home: 22.6 empty leaves a day at 2k against 124 home HangOuts (the eight fix-round diaries: 5 empty of 29 at a Home, 35 of 73 on the street), the misses of the "due within the hour" clause — `hunt::habit` says "home" from 20:00 for anyone who slept at home 7 of 14 nights, so at 19:00 on day 19 it read 317 bodied residents due home and 39 were inside an hour later (the first fix build counted a Statistical resident's habit too: 1,702 "due", 35 inside; a Statistical `Position` is a demotion snapshot and never arrives). Whether a Statistical spouse counts as home by the clock is the mirror question for the gate (§ 6): with it, the Full tier's home evening would see the 95 % of households whose other members are Statistical. Murders swing by seed both ways again (47: 94 → 196; 44: 197 → 132), mean +8 %, printed. Homeless (+5 %) and evictions (+11 %) are a late-run drift: by 30-day window the fix/main ratio of evictions reads 1.01, 1.11, 1.02, 1.24 and of homeless 0.96, 0.97, 0.97, 1.28 while rent paid, wages, employment and prices hold (0.98-1.00); the first mover is hotel nights (×0.77 from day 0: an evening at home is not an evening at a hotel), then wallets (×0.90 at days 60-90, ×0.85 at 90-120) beside deaths by violence ×1.39 in the same window; no mechanism of the phase touches rent; printed, not retuned.

Test changes (each pins the city a pre-phase rule was written for, as phase 1's vice tests did): `tests/leisure.rs::test_unwind_skips_paid_rungs_when_broke` lets a Free pick carry a Home as its venue (never a paid one); `tests/person.rs::probe_night_porter_on_shift_at_two_is_paid_by_eight`, `probe_worker_sleeps_the_night_before_a_shift` and `probe_gun_never_takes_a_hit_on_kin` run with `household.enabled = false` (with it on, the day-2 city picks another porter, one founding a business at the Hall at midnight and walking home before the shift; one farmer of twenty spends the evening at a far Braindance Parlour, walks five hours home and naps at 08:23; the gun probe's married civilian is killed on day 3 before the hit resolves: no night touches a household mechanism; traced in the phase 2 report); `tests/succession.rs::test_bug8_leader_by_tenure_and_no_succession_for_a_night` likewise (on the household's day-12 city no gang of four has a leader and two free members for its pick); `tests/utility.rs` counts 32 goals; the cli header gains 9 columns.

Parity (`tests/lod.rs::test_full_vs_statistical_within_15pct`, milestone tier, run once on the first build, 234 s, no fix): `paid venue visits Full 12.475 Statistical 16.391 (tolerance 1.87)`, `paid venue coins 54.724 / 72.461 (8.21)` and `hunger-days 1.269 / 1.779 (0.50)` are out, where phase 1's run had them in: the Full tier now spends evenings at home and eats there, while the Statistical table still carries main's venue and meal rates; this is § 6's stated mirror gap, learned at the gate's one regen (D14), so the rows stand RED until it. `actor share law0 0.899 / 0.729` and `law1 0.087 / 0.219` fail as on main; `encounters/shift 0.541 / 0.887` is phase 1's printed FINDING; thefts, arrests, assaults, violent deaths, marriages, bin meals, malnutrition, the rides and law2 pass.

### Phase 3 (2026-10-18): the civic counterparts and the round

Built as § 2.2's teacher, camp warden, jailer and super (tenants, the near round, tied housing) and § 3.5's parent's answer to a taken child, under `[service] civic` (`CITYSIM_CIVIC_OFF=1`, under the `[lives]` master; `service::civic_on`); SAVE_VERSION 37 (`Child.edu_taught`). Deviations from the spec text:

- **One `ServeKind` for the Camp and the Jail.** The spec names no kind for the camp warden; the Camp's children are `Inmate` as the Jail's held are, one `served_inmate` column pooling both (the `service_probe` and the diaries tell them by place). No new kind.
- **The civic rows are stripped off.** A `[[serves]]` row carries `civic = true` (teacher, camp_warden, jailer, super); `Config::load` drops those rows, the `[service]` civic keys and the trade rows' `round_near`/`tied_housing` whenever `[service] civic` is false by any path (the toml, the env), so the off save matches main byte for byte; a test that flips the switch after the load calls `Config::strip_civic` itself. No regulars among children or prisoners (`regular_after = 0` on the civic rows: a child has no edge table).
- **The taught day is a new field.** `edu_days` stays the midnight attendance count (unchanged off and on); `Child.edu_taught` gains `teacher_floor + (1 − teacher_floor) × the teacher's knowledge` on each `Pupil` encounter; `ages::on_mature` reads `edu_taught` under civic, `edu_days` off. A School day with no teacher on duty teaches nothing: that is the gate. The roster runs in `service::hourly` at the first hour of a teacher's shift (one pass over the children a School shift). The parent at the door is a body at home (`Position.building == home`, not Statistical): a first meeting at a hashed `0.1 + 0.3u` or `interacted`, a `Served` memory each side, no walk.
- **No `repairs_per_stop`.** `Building` has no condition field and the Block upkeep is a flat charge; `super_stops` counts the stops and nothing is repaired. Not built rather than a condition model invented.
- **The super serves his own round.** `service::arrive` resolves a server among the place's on-duty staff, but a super's employer is one Block and his stops are others; `service::super_stop` serves each stop's adult tenants by the super himself (his row's queue is the round). A `Notice` once a tenant a day (`World::notices_today`, saved while it holds anything so a mid-day resume serves the same notices, cleared at midnight); the door rent is `ownership::settle_rent(midnight = false)` (the same `Flow::Rent`, earlier; a full payment clears the arrears as the midnight one does; a short one adds no day). The Statistical super runs the same stop list in `lod::stat_work` at the shift's first hour when `shift_attended`.
- **The near round** (`[[trades]] round_near`): `trades::new_round` takes the `round_stops` Blocks of the pool nearest the super's Home door (the post's when homeless), ties by id; the nearest-first walk after it unchanged; a Builder's and a courier's rounds untouched. The shadow tool's `activity` reads a round leg on shift as work.
- **Tied housing** (`[[trades]] tied_housing`): `ownership::collect_rent` frees a resident whose tied trade's employer belongs to the Home's owner as it frees an owner living in their own Home; `settle_rent` likewise at the door. The move-in is `household::tied_housing` at midnight in `ownership::run` after `reunite` and before `rehouse`: the super with his co-resident spouse and children into the landlord's standing Block in the post's district nearest the post with room for them all, never evicting; `Housed` as `rehouse` posts it.
- **The jailer's meal and the breakout.** `law::jail_upkeep`'s fed prisoner is an `Inmate` arrival at the Jail (the night shift holds at midnight); `raid::breach` names the first jailer on duty in the `Jailbreak` text (", past <jailer>") and each on duty keeps a `Served` memory at −0.3 (`service::jailers_passed`).
- **The warden's rolls** are in `camp::daily` at midnight (`p_camp_beating`: a `ChildTaken` memory 0.5/−0.6 to each living parent; `p_camp_escape`: `release(Escaped)`, two new `ReleaseWhy` variants beside `Adult`/`Rescue`), hashed on (child, day).
- **A returning child needs no room.** The brief's "a parent's Home with room" sent every bought-out, broken-out or escaped child to the street: most pairs live in a flat of two and the taken child was its third. `camp::send_home` houses the child in a parent's standing Home as `demography::spawn_child` houses a newborn, without the capacity check; the street only with no parent housed.
- **The parent's answer** is an arm of `household::visits_due` (the day of the week hashed on (parent, week), `PURPOSE_ANSWER`; a parent with two taken children answers for the lower id; the hurt-kin visit outranks it on the day). The `Visit` goal carries it as `VisitWhy::Camp(CampAnswer)` on the pending visit (saved while pending); the walk is phase 2's `GoTo(Victim) → Rest` to the Camp (a child has a `Position` inside it), the answer resolved at the `Rest`'s end (`visit_done`) or, for a Statistical parent, at the hour (`hourly`). The visit is **not queued at the warden's counter** (his row serves children): a `Served` memory with the warden on duty, the `last_met` and the belonging written by the arm (a child has no edge table: no `interacted`).
- **The break-out's arrest path.** `law::arrest` takes any arrester adjacent or in the same building, so a lost break-out by a body is an arrest by the strongest warden standing (a warden has no escort duty: the cuffed parent waits for the Law as any `cuffed_by` does); a Statistical parent has no body at the door, so the Abduction is filed with the parent as suspect (`law::file_report`) and the warrant runs. A won break-out files the Abduction too (with the first warden present as witness) beside `raise_crime_on`. The raiders are the parent plus his gang's free members within `raid_gather_radius` of the Camp door (a Statistical parent's: the first `breakout_crew`), no `Raid` goal asked of them (a local version of `raid::raiders_at`); the defenders the wardens present and the Camp's robots; `raid::fight_out` as M9's breach. With no warden on duty the door is open and the child walks out.
- **The buy-out's refund.** The purse can move between midnight's answer and the hour: a fee paid short is refunded (`ownership::refund`) and the parent visits instead.
- **The CSV home** is `Jobs2Cols` again, 13 columns: `camp_beatings`, `camp_escapes`, `camp_headcounts` (one a Camp shift with children; the brief's bullet), `camp_visits`, `camp_buyouts`, `camp_breakouts`, `camp_neglect` (a daily tally of parents), `notices`, `door_rent`, `super_stops`, `tied_housed`, `flow_buyout`, `taught_days`.
- **The tests** are `tests/civic.rs` (10; the super's with them, there being no `tests/trades.rs`) and two probes in `tests/person.rs`; `tests/lod.rs` prints the door rent share by tier beside the encounters row.

The Statistical mirror (§ 6): the teacher's and the warden's rosters, the jailer's meal and the warden's rolls are world-side (a Statistical server is credited by `present`); the super's door rent moves at the shift's first hour for both tiers (`stat_work`); the camp answers are resolved by the same midnight roll, a Statistical parent's at the hour; tied housing and the near round are midnight and shift-start rules for every tier. No stat-table regen (D14).

Measured (phase 3 against main `899acc2`'s binary, built from `git archive` and run from its own root; 2k seeds 42-47 × 120 d, days 30+, mean a day; `scratchpad/phase3_report.md` has the per-seed rows):

| | main | phase 3 |
|---|---|---|
| taught School days (= `Pupil` encounters) a day | — | 1.09 (624 over the six runs; the 2k city has few children: 1.1 School days a day on main) |
| Camp headcounts a day (a Camp shift with children) | — | 0.86 |
| Jail meals as `Inmate` encounters a day | — | 59.7 |
| super's stops / tenants served / notices a day | — | 117 / 790 / 10.5 |
| door rent a day (of rent paid) | — | 143 coins (21 % of 676) |
| rent paid / rent short a day | 676 / 58.7 | 676 / 53.3 (−9 %) |
| evictions / homeless a day | 2.64 / 41.0 | 2.45 (−7 %) / 35.4 (−14 %) |
| tied move-ins | — | 172 over the six runs (28-31 a run; the first build's two bouncing supers, below, read 136 on seed 42 alone) |
| super's rounds walked a day | 2.43 | 2.85 (+17 %) |
| children taken / Camp children a day | 0.085 / 6.25 | 0.135 (62 → 90 over the six runs) / 5.48 |
| Camp beatings / escapes (over the six runs) | — | 82 / 17 |
| parents' Camp visits / buy-outs / break-outs (over the six runs) | — | 621 / 1 (seed 45, 60 coins) / 24 |
| neglecting parent-days a day | — | 1.84 |
| the hour's unserved arrivals a day | 212 | 223 (+5 %) |
| Murders (120 d, per seed 42-47) | 74, 115, 132, 132, 29, 196 (mean 113) | 55, 117, 225, 138, 64, 97 (mean 116) |
| deaths by violence / thefts / arrests a day | 1.60 / 2.84 / 24.4 | 1.58 / 2.93 / 24.2 |
| J24 employed share / J25 homeless share (seed 42) | 0.759 / 0.025 | 0.768 / 0.021 |
| HangOuts a day; employed share | 361; 0.773 | 349; 0.775 |
| ticks/s, alternating A/B, seed 42 × 30 d, 3 rounds each | 4,317 / 5,268 / 5,644 | 6,097 / 6,433 / 6,295 (the first main round a cold start; no regression readable on this box) |
| save size, 30 d seed 42 (`--save-at 43200`, no report) | 12,777,695 B | 12,945,486 B on (+1.3 %); off 12,777,695 B, one byte (the version) aside |

The lives probe (`shadow --lives`, 2k, seeds 42-44 pooled, day 19, 7 days; main → phase 3): teachers Served-able 0.00 → 0.03, `met` 0.00 → 0.45 of shifts, 0.60 encounters a shift (the one or two pupils of a 2k School); camp wardens Served-able 0.03 → 0.09, `met` 0.00 → 0.79, 1.36 encounters a shift; supers Served-able 0.22 → 0.26, `met` 0.00 → 0.97, 27.8 encounters a shift (six Blocks' tenants), idle 3.67 → 4.40 h/d (the round done, the post holds the rest of the shift); guards `met` 0.01 → 0.14 (the jailers' encounters among the Jail's staff); all 0.07 → 0.32. No `jailer` archetype exists in the probe or the diaries (the Jail's row shows through the guards'). The supers' travel: main's two diaries 6.7-13.3 h a day (V4's 63-79 h a week), phase 3's 0-7.7 h (median 3.3) with 8-10.6 h read as work (the round legs); the probe's super 311 → 165 minutes of a 540-minute shift.

Readings. The rounds are the device that moved: the super's shift is work now (stops, notices, door rent), his travel a third of V4's, and he lives in the landlord's Block (evictions −7 %, homeless −14 %, rent short −9 %: the door rent collects 21 % of the rent before midnight, so fewer tenants fall a day behind). The first build's tied housing bounced two households every midnight (a super's spouse relocated nearer her own work and the super with her; two supers of two landlords married each other): the relocation pass now leaves a landlord-housed household where it is and a worker housed by a co-resident's landlord stays (28-31 move-ins a run, mostly day 0). The Camp arms fire as rare mechanisms at 2k (few children: 62-90 takes in six runs; one buy-out, 24 break-outs, 17 escapes), as the brief expected: unit-tested, printed, not bullets. `children_taken` rises 59 %: the Camp's children go home (escapes, break-outs) and are taken again (seed 42's Lavinia Reyes slipped out twice); printed. Murders swing by seed both ways (44: 132 → 225; 47: 196 → 97), mean +3 %. Hunger-days, tips and venue visits unchanged; the hour's unserved arrivals +5 % (the Camp's children when no warden has room). No parity row asserted; `tests/lod.rs` prints the door rent share by tier beside the encounters row (not run this phase: the milestone tier; the first build's rows stand from phase 2).

Test changes (each pins the city a pre-phase rule was written for, as phases 1 and 2 did): `tests/camp.rs::test_release_at_adulthood_with_camp_raised` runs with the civic switch stripped (on, a parent's weekly answer took the child home in the four days before its coming of age, the mechanism the test does not ask for); `tests/person.rs::probe_married_pick_is_home_by_nine_with_the_spouse`, `probe_far_worker_with_one_rent_moves_near_work` and `probe_worker_makes_one_grocery_run_to_the_nearest_detour_market` likewise (tied housing moves the seeded supers' households on day 0, and the picks on the moved city are a Wanderer at 21:00, a far Farmer without a trade partner and a worker with two grocery runs). `tests/god.rs::god_flood_guns` picks the weakest gang **with a crew to march** (`raid_min_members` free members besides a leader who stays back, D10): on the civic city the weakest gang had three members and never drew the crate (`drawn > 0` failed at the merge tier); the pick's precondition, not the bound (the merge-tier god suite GREEN after it). Added: `tests/civic.rs` (10), two probes (the buy-out probe tries the first pairs until a parent lives the fortnight: the harsh city abducted, harvested and killed seed 42's first pick on day 13) and six printed bullets; the cli header +13.

### Phase 4a (2026-10-18): the cab, the parcel, the lift, the client

Built as § 2.3's cab driver, courier, loader and street worker under `[service] street` (**`CITYSIM_LIVES_STREET_OFF=1`**, under the `[lives]` master; `service::street_on`); SAVE_VERSION 38 (`Jobs2State.cab_rides`, `cab_rolled`, `parcels`, all skip-empty). The pimp, the dealer and the street trades' regulars (§ 2.3's last three rows) are phase 4b's. Deviations from the spec text and from `phase4_brief.md`:

- **The switch's name.** `CITYSIM_STREET_OFF` is Jobs & lives phase 2's `[jobs2] street` (the pitches and rounds themselves; under it main hires no street trade). This phase's is `[service] street` with `CITYSIM_LIVES_STREET_OFF=1`; `Config::strip_street` drops the `[[serves]]` rows flagged `street = true`, the `[service]` street keys and `[stims] deal_corner` whenever it is off by any path, as `strip_civic`.
- **One cab roll, two payers.** `streetwork::stat_cab`'s inline roll is `cab_roll` (the pure roll and pick) plus `pay_ride` (the coins), unchanged for the Statistical tier; a Full or Coarse commuter rolls the same hash at the commute's `GoTo` to work (`full_cab`, from `exec::start_step` before the vehicle, the El and the walk), once a day a rider (`cab_rolled`). A Full rider `transit::pays_or_dodges` names a dodger rides `Short` (no coins, `FareShort`, a `Robbed` grudge driver → rider); `stat_cab` has no dodge (a Full-only outcome, counted in `fares_short`).
- **The rider never waits.** The spec's rider waits at the door up to `cab_wait_ticks` and then walks; instead the ride goes to the standing-by driver who can reach the rider within `cab_wait_ticks` at cab speed and the lowest `hash(PURPOSE_FARE, rider, driver)` (D6), and with none the Garage's off-screen cab takes it (`cab_rides_unmanned`, replacing the brief's `cab_waits_walked`). The rider's leg is `GotoTimed` at the Car's `timed_mult` either way: the shift matters more than the ride. **A Statistical rider's cab is booked the same way** (`stat_cab` → `book_cab`, from his home door at the hour; D3: the ride is world-side, a body driver animates it): with only the Full and Coarse riders booking, a pinned driver drove nothing (2k has 200 bodies of 2,000).
- **The cab drivers' rush shifts.** `split` resolves to the day shift (09-18) for an odd id and the night (21-06) for an even one, on duty at neither commute (day workers leave before 09:00, night workers before 21:00): every ride went unmanned. Under the street switch a `split` fare trade takes `[service] cab_shifts` (`[[360, 960], [960, 1440]]`, one a driver in turn by id); off, the `split` rule.
- **The driver's body stands by.** On shift with no ride the driver's Work plan is `GoTo(Garage) → Rest`, the Rest running on while `streetwork::standing_by` holds and ending when a ride is his (`exec::actions`' early-end rule, false off); with a ride, one `GoTo(Spot)` leg a plan (the rider's door, then the work door, timed at cab speed by `fare_leg`), read off his position as `round_plan` reads a stop; the shift's last `ROUND_CUTOFF` is the post's (the work step, the wage), as the round's. No work action is cut short (§ 1.3). The M13 trip roll (crashes, `Manslaughter`) is **skipped**: the Garage posts no vehicle asset for the cab, and none is invented.
- **The parcel's stops are its recipients.** At midnight each parcel is named (`streetwork::name_parcels`): a sender among the corp's standing buildings, a recipient building among those within `site_reach` of the Yard with staff (not the sender) and a staff member of it, each a hashed draw (`PURPOSE_PARCEL`); `parcel_stops` holds the recipient buildings (the senders' off). The fee and `parcels` are unchanged. A Yard's couriers split the stops (each round starts `round_stops` further on by the courier's rank; one rotation for all of them before). At the stop (`trades::round_plan`'s arrival, beside the super's) `courier_stop` hands each parcel to its recipient when `service::present`, else to a present staff member (hashed); none present, it stays. At the courier's shift end (`service::shift_end`, both tiers) the parcels for his round's stops go back (`parcels_returned`); one no round reached is counted returned at midnight.
- **The lift is a count, not a person, at the shift's last hour.** `service::hourly`'s `lift_hour` (one world-side pass, both tiers through `present`) credits each loader present at his Yard in his shift's last hour with `industry::shift_lifts` (the Yard's units in the day to now, today's plus yesterday's share of the hours not yet past, over its handling staff on shift, one at least; the brief's "today's units" read 0 at a night shift's 05:00) through `service::lift`: `served[Crate]`, `lifts`, one `Served` memory with no subject (the diary: "loaded the shift's crates for <owner>"; the count is the column's, the memory has no count field). Not at `end_shift`: a memory written as the shift closes never lands on it (the loader's `met` read 0). **The pilfer was built already** (`industry::pilfer`, Jobs & lives phase 2); the owner's report is added after its hit: `hash(PURPOSE_SHORT, loader, day) < p_short_report` → `law::file_report(Theft, loader, None)`, `pilfer_reports`.
- **The client.** `vice::street_hour`'s visit roll reads a body first (`client_near`: `world.agents_near(spot, client_tiles)`, Full or Coarse, in no building, an adult, free, `street_payer`'s wants and purse, the lowest `hash(PURPOSE_CLIENT, worker, client, hour)`), the district's hashed draw after; the paid visit is a `Client` encounter through `service::encounter(worker, client, None, district, …)` (the pitch is a tile: the event names the district). A lawless (`lawfulness < 0.3`) client is the `street_assault` actor on the existing roll; else its own hashed pick.
- **A Pitch owner's row.** A street worker and a rickshaw puller hold no `Job`, so `service::row_of_agent` (and so `encounter`) found no row and dropped their encounters silently; under the street switch it reads the row keyed by the pitch's trade id.
- **The WIP's test file.** The first coder's `tests/street.rs` replaced the base's 21 M12 tests (litter, hotels, derelicts); they are restored and the phase's tests sit in its `lives_street` module.

The Statistical mirror (§ 6): the cab roll is one function for both tiers (asserted in `tests/street.rs`); a Statistical rider pays at the hour (`stat_cab`), a Full one at the commute's GoTo, both booked to a standing-by driver's body by one rule (`book_cab`) with the same `Fare` encounter (`ride_encounter`; the rickshaw puller's too); the parcels are named world-side at midnight, a Statistical courier hands them at his shift's first hour when `shift_attended` (the super's rule, `lod::stat_work`), a Statistical recipient credited by `present`; the lift and the pilfer's report are shift-end and night rolls for every tier; the client near is bodies only (a Statistical client has no tile), the hashed draw stays for the rest. `tests/lod.rs` prints cab rides per 100 commutes by tier. No stat-table regen (D14).

Measured (`tools/phase_measure.py`, phase 4a against main `1300584`'s binary, 2k seeds 42-44 × 120 d, mean of the three; off identity on seed 42 × 30: IDENTICAL, events, CSV and save):

| | main | phase 4a |
|---|---|---|
| Murders | 132 (55, 117, 225) | 81 (30, 88, 126), −39 %: a FINDING for the gate's 42-47. The fall is the off-screen roll ("killed in <district> (assailant unknown)": seed 44 179 → 68), which comes in storms on both sides (seed 43: main's 80 in days 20-60, 4a's 45 in days 100-120): a storm moved, not a level; no 4a mechanism touches the murder roll |
| J24 employed / J25 homeless | 0.770 / 0.019 | 0.751 / 0.021 |
| cab trips / fares | 54 / 405 | 66 / 504 (the Full commuters now ride) |
| cab rides driven / unmanned / short | — | 0 / 67 / 1 (unpinned drivers are Statistical: the off-screen cab; a pinned driver drives, `probe_cabbie_drives_a_fare_on_his_shift`) |
| parcels / delivered / returned | 16,864 | 17,172 / 3,736 / 799 |
| lifts / pilfered / pilfer reports | — / 48 | 57,235 / 51 / 27 |
| street visits / clients near / assaults | 19 / — / 0.3 | 20 / 7 / 1.3 |

**Open:** phase 2's `probe_new_parent_is_home_at_night` is `#[ignore = "gap: OPEN-100"]`: in the 4a city the carer missed an evening meal (a diverged Market's stock), went to bed hungry with an empty pantry and walked to a Market at 02:00 (4 of 8 night hours home); the § 3.4 damping does not hold a hungry carer's Eat. Not a 4a mechanism (the coins and states match the off city until her 18:53 purchase on day 4); a behaviour fix for § 3.4's owner.

Left for 4b: the pimp's round and presence, the dealer's corner and regulars, the lookout, and the vendor/rickshaw/tinker regulars (§ 2.3's last three rows); the `[service]` keys `pimp_tiles`, `pimp_pass_up`, `pimp_absent_days`, `witness_lookout_mult` and `[stims] deal_corner` are in the toml, unread.
