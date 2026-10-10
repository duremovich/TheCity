# M16: Contracts — the board, Fixers, hired guns, leverage, the wounded

Companion to `SPEC.md`, `M8_FACTIONS.md`, `M9_LAW.md`, `M10_SCALE.md`, `M11_OWNERSHIP.md`, `M12_DISTRICTS.md`, `M13_ASSETS.md`, `M14_VIRT.md`, `M15_WORD_AND_BLOOD.md`, `ROADMAP_POST_M14.md` and `VISION.md`. M15 gave the city knowledge as talk: who is feared, who is owed, who means to kill whom. M16 gives it **knowledge as a price**: what it costs to have a thing done, and who will do it. A Mid West clerk's friend is too weak to hunt the Hollow runner who killed her, so she takes her savings to the Fixer behind the Stack Bar; the Fixer passes the job to a Sump gun who drinks there; the gun waits outside the runner's Home on the habit the Bar told him, and the runner dies on the second night. Three weeks later the gun is picked up on an old Assault, gives up the Fixer's name under questioning, and the clerk's friend is charged as accessory. Meanwhile Habitat keeps three of Stackwell's tenants' children in a private prison until Stackwell's board sells two Blocks, and Stackwell's chair loses the next vote to the member Habitat has been blackmailing. Where this document and the earlier ones disagree, this one wins for M16.

The roadmap row (`ROADMAP_POST_M14.md`): *the contract entity, Fixers and hired guns, hits, leverage moves (hostage, extortion, blackmail, threats, fraud), bounties, the law's accessory rule; the open contracts are the quest board.* M16 also takes what the addenda and `VISION.md` assign to it: missions at three LODs (governance addendum), the board vote swung by leverage (governance), private prisons, hostages and freeing them as a mission, abduction as a leverage move feeding the scav economy, poaching by threat (addendum 2), hit squads gated on territory, relative power and political cost or the territory's faction hired, bounties for location streams, tracking tags and scanners (addendum 3), propaganda and fabricated stories as contracts (addendum 6; M15 deferred fabrication here), the `Wounded` state, Rescue by faction temperament, Clinics as treatment and Trauma Team as a subscription contract on a person (addendum 8), and `Heist` as a contract kind (M14 deferred it). Daemons, the outside world and the player stay out (§ 14).

Scale: 2,000 residents on the 256 × 192 map; every number assumes M10–M15 have landed. At the time of writing the tree holds M12 phase 2; M13's assets, M14's runs and `FactionDb`, and M15's deeds, reputation, grudges, Hunt and `social::resolve` are specified, not built, and M16 reads them as specified.

(revised 2026-10-08: M10-M15 are built and M15 is closed at `d738a07`; every identifier below was checked against that HEAD and corrected where it had drifted (the list is in "M16a and M16b", *Identifier corrections*). **Life pass L2 "the living city"** (`docs/LIFE_L2.md`, plan `~/.claude/plans/life-l2.md`) lands between M15 and M16 and is read here as planned: its wage economy, venues and gang fronts, the World account and conservation identity, off-screen faction violence with `Hole.source`/`Hole.faction`, the LOD budget with per-class quotas, bed and seat reservations, and `save::SAVE_VERSION` 2. Each M16 plan verifies L2's identifiers against HEAD before its first phase, adapting identifiers, never behaviour. M16 is now **two milestones, M16a and M16b** (roadmap addendum 16; `docs/BIG_PICTURE_2026-10-07.md` § 6); the section after the decisions table says which part owns what. As everywhere in this project, a "hit", a "hostage", "blackmail" or a "mission" is a seeded dice roll over Rust structs, counters and config keys between fictional agents of a simulated city, nothing more.)

Decisions taken by the drafting agent (overturnable, listed so they are cheap to overturn):

| Question | Decision |
| --- | --- |
| What a contract is | A struct in `World::contracts: BTreeMap<ContractId, Contract>`, not an entity: buyer, kind, target, terms, deadline, broker, taker, status, risk, political cost, and `known_by` (who knows the buyer is behind it). Hundreds at most (`max_open` 256 open). The existing `Corp.contracts` (Security guard contracts on buildings) stays as it is; M16 calls those *guard contracts* in prose and does not migrate them. |
| Jobs and leverage | Two structs, as the roadmap sketched: a **`Contract`** is a job someone is paid or coerced to do; a **`Coercion`** is a standing hold one party has over another (protection, blackmail, a hostage) with a demand. A coerced job is a `Contract` with `Terms::Coerced(CoercionId)`. (revised 2026-10-08: M16a ships `Contract` with `Terms::Pay` only and no `Coercion`; M16b adds the `Coercion` store and appends `Terms::Coerced` with `#[serde(default)]` on every new field, so a 16a save loads in 16b.) |
| Who may be a buyer | Any M11 owner: an agent, a gang, a corp, or `None` for the city and the law, paying through `World::purse` and `ownership::pay`, so the ledger and the conservation test cover every coin. |
| Escrow | **Brokered means escrowed.** A contract placed with a Fixer moves the price into `Contract.escrow` at posting (`Flow::Escrow`, counted by `ownership::total_coins`); a direct contract (no Fixer) is unescrowed and reaches only the buyer's edges and faction. Reneging and fraud live on the direct side, which is why Fixers earn their cut. |
| The Fixer | `BuildingKind::Fixer` (label "Fixer", letter `X`), foundable through `Register`, worked by its owner or one `Role::Fixer`. It keeps a book, a list of regulars, a cut and its own **heat**. Matching is **daily**, at `match_hour`, never hourly. The roadmap's `Den` is renamed to the kind it is. (revised 2026-10-08: the roadmap's sketch called this building a "Den"; that name now belongs to L2's Gambling Den, `BuildingKind::Den`, letter `D`, a leisure venue and often a gang front. The Fixer is `BuildingKind::Fixer` and is never called a Den; there is one `Den` in the code, L2's.) |
| Who takes jobs | Agents (hired guns, couriers, runners, talkers), gangs as factions (the tenth gang `Order::Job`), Security corps (Guard, Escort, Trauma), Feeds (Spin). The taker of record is one `EntityId`; a crew rides along in the mission. The player takes the same jobs in M18 through the same call. (revised 2026-10-08: M16a's takers are agents, gangs and Security corps for `Guard`; Escort and Trauma corps and Feeds arrive with M16b's kinds.) |
| Missions | A **mission is a `raid.rs` expedition with a contract as its source.** The muster, the march, `raiders_at`, `by_strength`, `fight_out`, crossfire and litter are factored to take a `Mission` as well as a `Gang`; gang raids keep their `Gang.raid_at` state unchanged. A hit squad is a raid on a person. (revised 2026-10-08: at HEAD `raid.rs` dispatches on an enum, `raid::Expedition { Gang(EntityId), Riot(u32) }` (M12 D31), not a trait; a mission is its third variant, `Expedition::Mission(ContractId)`, with the `Gang` and `Riot` arms moved into `match` arms token for token. `raiders_at` is private at HEAD; `gathered`, `by_strength`, `strength`, `fight_out`/`fight_out_until`, `crossfire`, `expedition_rival` and `muster_point` are the public pieces a mission calls.) |
| Three LODs | One contract, three renderers: **ledger** (both parties Statistical and unwatched: one draw at a hash-picked due tick), **live** (any party Full or Coarse, or the mission in view: executed by exec; streamed through M14's `Overwatch` when a crew member carries a deck), **played** (M18). The ledger draw uses the same strength estimate the live brawl would use. |
| Off-screen fulfilment | **A pre-bound hole.** A ledger Hit kills through `demography::kill` and opens a `HoleKind::Killed` hole with the new field `Hole.contract = Some(id)`; the binder skips the candidate draw and binds `Actor(taker)`, still rolling the witness from the hole's own stream. The 2-bit kind field stays full; the answer never changes. (revised 2026-10-08: the kill is `World::kill_by(target, DeathCause::Violence, Some(taker))` (`demography::kill` does not exist). No new `Hole` field: L2 gives `Hole` `source: Option<ViolenceSource>` and `faction: Option<EntityId>` and a binder filter that restricts the pool to `faction`'s members, with the episode precedent of `faction` holding one agent. M16a appends `ViolenceSource::Contract(ContractId)` and opens the hole with `source = Some(Contract(id))`, `faction = Some(taker)`; `bind::bind_in` on that source draws the Unknown roll and the candidate draw and discards both, binds `Bound::Actor(taker)`, then rolls the witness as today, so the stream offset, save/load and re-bind are unchanged.) |
| The accessory rule | Knowledge-driven, as M15 requires. A new deed `Hired` (actor buyer, object target) is known to the buyer, the Fixer and the crew, and travels by gossip like any deed. The law files `Crime::Conspiracy` against the buyer when an interrogation of an arrested taker succeeds or when a guard or a reporting witness holds `Hired` about the victim of an open Murder report. The system never names a buyer it has not been told. (revised 2026-10-08: `CrimeReport { crime, suspect, witness, tick, resolved }` has no victim and no id at HEAD, so the check is keyed on fulfilled contracts, not on reports: § 3, *Conspiracy*.) |
| Hit squads | A Hit whose solo `p_success` is below `squad_below` goes as a squad. The strike is **gated** on the target district's controller (M12), relative strength, and a political cost with that controller; when the cost exceeds the job's value and the controller is a gang that would sell, the job is offered to that gang instead (`Sold out`). |
| Bounties and tags | `Locate` pays per sighting delivered to the buyer's `FactionDb`; `Tag` plants an `AssetKind::Tag` (M13 asset, tier 1–3) on the target that writes one sighting an hour until its battery dies or a `Scanner` finds it. Both reuse M14/M15's `Sighting`. (revised 2026-10-08: the struct is M14's `virt::Sighting { who, tile, tick, confidence }` plus M15's `relayed` flag, kept in `World::db: BTreeMap<EntityId, FactionDb>` (`FactionDb { sightings }`) and relayed from members' and guards' eyes by `gossip::maybe_sight` under `gossip::wants_sighting`. `Locate` is M16a; `Tag` and the scanners are M16b.) |
| Extortion | Generalised to `leverage::coerce`, the Intimidate move of M15 with a demand. The gang Home shakedown is a one-shot coercion with the M8 claim counting unchanged on top; **protection** is the standing version, weekly, on owner-run businesses in held districts. (revised 2026-10-08: at HEAD the shakedown is `gang::extort(world, actor, home)`, whose M15 W27 Intimidate goes through `moves::resolve`; M16b routes it through `leverage::coerce` with the same `p`. Protection targets L2's venues too, never the gang's own fronts.) |
| Blackmail | A secret is a deed the holder knows about the target whose holder count is ≤ `secret_known_max`. It lapses when the deed's pool reach passes `secret_public_reach`. Holder counts are tallied by M15's daily reputation pass for the few keys referenced by secrets, so no extra memory scan. |
| Hostages | A `Captive` component on the held person (as `Sentence` holds a convict), at a Hideout or a corp `BuildingKind::Prison`. Taking one is M13's `Abduct` and its `Crime::Abduction`; the leverage is a `Coercion` over the captive's kin, employer or faction. Freeing one is a `Rescue` contract: a raid on the building. (revised 2026-10-08: M13 built the abduction as the Harvest's drag, `chrome::abduct(world, agent, victim)`: a fight with nearby members' fighting summed, a win sets `Brain.cuffed_by`/`Brain.abducted_by` on the victim and `Brain.escorting` on the abductor, and the Harvest plan ends in a Rip at the Hideout (`p_rip_kill`, else release with `MemoryKind::Abducted`). Off screen, `chrome::abduct_offscreen` kills the victim into an `Abducted` hole. A hostage reuses the drag and replaces the Rip with `HandOver`; a hostage taking is never off screen (it promotes). M16b.) |
| Fraud | Three forms only: a buyer who posts direct with no intent to pay (renege decided at posting from a hash), a fabricated story bought through Spin, and a fake fixer selling a job that does not exist. All go through M15's Deceive move and `Crime::Fraud`. |
| The wounded | `resolve_fight`'s death roll, when it hits, becomes `Wounded` with `p = wound_share` instead of a death; the wounded bleed out after `bleed_hours` unless carried to a Clinic. Deaths can only fall, never rise, from this rule; the Murder baseline is protected. |
| Rescue and temperament | `Temperament { Abandon, Loyal, Recover }` as a trait on gangs (Recover for corps and the law, fixed). Rescue is a Full/Coarse goal for the wounded's faction-mates and kin. Off screen it is in the StatTable, re-calibrated, except Trauma subscribers, who get a multiplier. |
| Trauma Team | `ContractKind::Trauma`: a standing weekly subscription a person buys from a Security corp. A wound writes a beacon sighting (conf 1.0) to the corp's `FactionDb`; the corp sends a crew from its Security Office to the last sighting, carries the client to a Clinic. |
| Governance | `Governance::Board { members }` gains a vote rule and a vote every `vote_days` or on the chair's death. Two seed corps get boards. Members vote by preference; a `Vote` coercion or a `Persuade` contract swings one member at a time. (revised 2026-10-08: at HEAD `Corp.governance` and `Gang.governance` are `Governance::Dictator` everywhere; the `Board { members }` variant exists (only a save test builds it) and nothing reads it. M16b.) |

## M16a and M16b (2026-10-08)

Dylan's decision (roadmap addendum 16, `BIG_PICTURE_2026-10-07.md` § 6): M16 is two milestones. **M16a** is the contract entity, the Fixer, `Hit`, `Beat`, `Guard` and `Locate`, missions and hit squads, the accessory rule and the board: the vision's "assassinations are popular" and "quests are real". **M16b** is leverage and coercion, hostages and private prisons, the wounded with Rescue and Trauma Team, boards and votes, tags and scanners, fraud and protection, and the contract kinds `Abduct`, `Rescue`, `Escort`, `Steal`, `Deliver`, `Tag`, `Persuade`, `Vote`, `Spin` and `Trauma`. Both land after Life pass L2 and read it as planned. Everything named here is a game abstraction: a contract is a struct with a price and a deadline, taken by a utility score and resolved by a seeded dice roll (`law::resolve_fight`, `moves::resolve`, `security::contest`, a keyed `SimRng::word` stream) between fictional agents; a hostage is a component and a counter, a vote a tally, a wound a timer.

**16a stands alone.** No 16a type, field, system or test names a 16b type. Where a 16a section mentions a 16b feature, 16a ships the part marked *a* and 16b **appends**: enum variants appended, every new field `#[serde(default)]`, every new config key behind a section whose `off()` is the 16a behaviour, so a 16a save loads in 16b and `--leverage-off` reproduces the 16a-closing city byte for byte. Concretely: 16a's `Terms` is `Pay { price }` only (16b appends `Coerced(CoercionId)`); 16a's `ContractKind` is `Hit, Beat, Guard, Locate` (16b appends the ten); `Target` is `Agent, Building` (16b appends `Node, Carry, Seat, Story`); `ContractStatus` has no `Standing` (16b appends it for Trauma); `Origin` is `Hunt, GangOrder(Order), CorpOrder(CorpOrder), Law, God` (16b appends `Court, Bury, Spin, Board, Subscription`); a reneging buyer in 16a gets the `Betrayed` deed and the grudge only (16b appends the `Crime::Fraud` filing); the shakedown stays M15's `gang::extort` roll in 16a (16b routes it through `leverage::coerce` with the same `p`). M17 may follow 16a directly if the outside is wanted sooner; the follow-ups at the end of this section say what M17 then waits for.

### Which part owns what

**Sections.**

| Section | M16a | M16b |
| --- | --- | --- |
| Decisions table | the rows marked *a* below | the rows marked *b* |
| Goals and acceptance (the original bullets) | superseded for gating by *M16a goals* below | superseded by *M16b goals* below |
| § 1 The contract entity | `Contract`, `ContractId`, `MissionId`, the 16a variants listed above, `Render`, price, risk and posting, escrow, the *Who posts what* rows marked *a* | `CoercionId`, the appended variants, the rows marked *b* |
| § 2 Fixers, the board, hired guns | all of it for the four 16a kinds; Fixer-fed run orders (added below, L2 § 7's deferral) | the plans, prices, deadlines and `gun_lawfulness` keys of the 16b kinds; Feeds and Escort/Trauma corps as candidates |
| § 3 Missions, hit squads and the law | the mission, the strike decision, squads, sell out, `Order::Job`, Locate bounties, the ledger resolution of Hit, Beat and Locate, the law's view (`Hired`, interrogation, Conspiracy, Fixer heat and bribes), guard corruption (added below, L2 § 7's deferral) | Tags and scanners with the Purist Patrol (added below, L2 § 7's deferral); the ledger rows of Steal, Persuade and Vote; Rescue and Trauma missions reuse 16a's mission |
| § 4 Leverage | none | all of it, plus Parley (added below, L2 § 7's deferral) |
| § 5 The wounded, Rescue, Trauma Team | none | all of it |
| § 6 Governance and the vote | none | all of it |
| § 7 The Statistical tier and LOD | the rows marked *a*; the quota rows below | the rows marked *b* |
| § 8 Player levers and god commands | `SetFixerLicence`, `SetAccessoryMult`, `PublicBounties`; `PostContract`, `TakeContract` | `SetPrisonPermit`; `Hostage`, `Wound`, `Subscribe`, `TagAgent`, `Coerce`, `CallVote`, `ExposeSecret` |
| § 9 UI, events, CSV | as split there | as split there |
| § 10 Save compatibility | `save::SAVE_VERSION` 3 | `save::SAVE_VERSION` 4 |
| § 11 Testing and calibration | the 16a unit tests, `test_m16a_contracts_seed_42`, the 16a god scenarios | the rest, `test_m16b_leverage_seed_42` |
| § 12 Phases | superseded by *M16a phases* below | superseded by *M16b phases* below |
| § 13 Risks | the murder market; the raid refactor; ledger consistency | the wounded split |
| § 14 Out of scope | both | both |
| Addendum: attention and distraction | 16a's strike defenders and Guard contractors read the existing stub `faction::alertness_mult` (1.0 at HEAD) | 16b fills it (and `virt::alertness_mult`, the second stub) |

**Decisions-table rows.**

| Row | Part | Note |
| --- | --- | --- |
| What a contract is | a | |
| Jobs and leverage | a (`Contract`), b (`Coercion`, `Terms::Coerced`) | |
| Who may be a buyer | a | |
| Escrow | a | |
| The Fixer | a | one `Den`, L2's |
| Who takes jobs | a (agents, gangs, Security corps for `Guard`), b (Escort and Trauma corps, Feeds) | |
| Missions | a | Rescue and Trauma missions in b reuse it |
| Three LODs | a | |
| Off-screen fulfilment | a | via L2's `Hole.source`/`Hole.faction` |
| The accessory rule | a | |
| Hit squads | a | |
| Bounties and tags | a (`Locate`), b (`Tag`, scanners) | |
| Extortion | b | |
| Blackmail | b | |
| Hostages | b | |
| Fraud | a (reneging as a deed and a grudge), b (`Crime::Fraud`, fabricated stories, the fake job) | |
| The wounded | b | |
| Rescue and temperament | b | |
| Trauma Team | b | |
| Governance | b | |

**The original acceptance bullets.**

| Bullet | Part |
| --- | --- |
| contracts: posted, fulfilled across kinds | a for the four 16a kinds; b for "≥ 8 kinds posted, ≥ 4 kinds fulfilled" over all fourteen |
| contracts: ≥ 2 Fixers in business, ≥ 1 NPC-founded; Fixer income vs a Bar owner's; ≥ 1 `Reneged` with a grudge | a |
| hits (every clause, including the attributed hit and clearance) | a |
| leverage (hostage taken and rescued, blackmail and lapse, protection, poach by threat, fabricated and exposed) | b |
| bounties: `BountyPaid` | a |
| bounties: tag planted, tag found by a scanner | b |
| the wounded (every clause) | b |
| governance (every clause) | b |
| Assault, Murders, starvation, population, earlier gates | both, with the bounds in each part's goals |
| throughput | both, printed |

### M16a goals and acceptance

(2026-10-09: how these bullets are judged is docs/TESTING.md: existence bullets, sanity bounds, person probes, behaviour bounds and god reactions; every band, majority or six-seed device below is a printed report, not an assert. See *Acceptance, revised 2026-10-09* under the testing section.)

The city should read as a place where a death can be bought: a price on a name that the buyer thinks nobody knows, a Fixer who knows everyone's, a gun who takes it for two months' wages, a gang that will sell out a stranger in its own street and not its own member, and a law that reaches the buyer only through what someone tells it. The target spiral is the original one, all of it 16a: **a Mid West clerk is killed by a Hollow runner → her friend's grudge cannot win a fight, so she posts a Hit with the Stack Bar Fixer → a Sump gun takes it, stakes out the runner's Home on the Bar's intel and kills him → the runner's brother hunts the gun (M15) while the law files a Murder → the gun is arrested on another charge, interrogated, names the Fixer and the buyer → Conspiracy against the friend, the Fixer's heat closes the Stack Bar office for a week → The Hollow posts a Locate bounty on the friend.** On seeds 42-44 (majority) and 42-47 (means and existence), 2,000 residents, 120 days, L2 on, gate `test_m16a_contracts_seed_42`, judged as `docs/HANDOFF.md` "How the gates judge now" says, per-seed data in comments:

**Asserted (mechanism and existence):**

- the board: `Hit`, `Beat`, `Guard` and `Locate` each posted on every seed of 42-44; fulfilled contracts in ≥ 3 of the four kinds (majority); ≥ 2 Fixers in business on day 120 (open, a contract in the book within 7 days; majority); ≥ 1 Fixer founded by an NPC through `Register` (existence over 42-47); ≥ 1 `Reneged` followed by a `Betrayed` grudge on the buyer (existence over 42-47);
- money: L2's identity `ownership::total_coins + Σ outside treasuries − outside.minted` holds every day with escrow counted inside `total_coins`; a contract closes only by paying or refunding its escrow (probe `escrow_leak == 0`);
- hits: ≥ 1 fulfilled Hit on every seed of 42-44; over 42-47: ≥ 1 with a gang or corp buyer, ≥ 1 by a squad of ≥ 2, ≥ 1 `StrikeDeclined` on political cost, ≥ 1 `SoldOut`, and **≥ 1 fulfilled Hit later attributed** (a `Conspiracy` filed against its placing agent ≥ 1 day after the killing);
- the ledger and the law: every ledger Hit's hole binds to its taker (probe `contract_hole_wrong == 0`); a placing agent is never charged without a guard or witness holding `Hired` about the target (unit-tested; the probe `accessory_unfounded` was retired in the M16a review: it re-ran the filing's own check); `Hired` reaches ≥ 1 kin of a target and forms a `GrudgeCause::Hired` grudge (existence over 42-47);
- bounties: ≥ 1 `BountyPaid` (majority); ≥ 1 public Locate posted by the law (existence over 42-47);
- the deferred role features (L2 § 7): ≥ 1 Fixer-fed run order run to a `SellData` (existence over 42-47); ≥ 1 city guard on the take (existence over 42-47);
- LOD: live contract parties ≤ `contract_quota` and Fixer bodies ≤ `fixer_quota` on every sampled hour; L2's bound `tier_coarse ≤ max_coarse + pinned + outside_rank` still holds;
- sanity: Assault events per day ≤ 42.7; Murders per 120 days ≤ 1.25 × the L2-closing run per seed (majority; contract killings included); starvation and population within the scaled v1 bounds; ≥ 5 of the 9 seeded corps alive on day 120 (majority); the v1, M8-M15 and L2 gates, the forced-tier parity test and L2's `test_faction_violence_parity` pass; `--contracts-off` reproduces the L2-closing city byte for byte (a 15-day CSV and the calibration city); a version-2 save loads;
- throughput is printed (run mean and last-10-day mean) against the 4,000 ticks/s floor, judged only by alternating A/B.

**Printed findings (calibration bands, not asserted):** contracts posted per 120 days 30-150 (the four kinds; the original 60-300 covered fourteen); fulfilled share of closed contracts 35-70 %; Hits fulfilled 4-20; contract killings as a share of Murders 5-25 %; clearance on contract killings (taker arrested within 30 days) 20-60 %; a Fixer's weekly income ÷ the median agent-owned Bar's weekly net 0.5-2.0, both measured in the same L2 run (priced on L2's wage economy, below); squads as a share of fulfilled Hits; Locate sightings paid per bounty; guards on the take per week.

### M16b goals and acceptance

The city should read as a place where people are held over each other: a corp that keeps people in cells for leverage, a board that votes the way its secrets say, a gang that collects from every bar on its street, and fights that leave people bleeding on the pavement for whoever cares to carry them. The target spiral: **Habitat's Acquire offer is refused twice by Stackwell's exec → a crew Habitat hires through a Fixer abducts the exec's daughter into Habitat's Private Prison → a `Coercion` over the exec demands `Sell`, re-rolled daily → a Stackwell board member who holds a secret on the chair blackmails a swing vote, the vote swings and the new chair sells two Blocks → the daughter walks out of the Prison door; on another seed Stackwell's allies post a `Rescue`, the mission breaks the Prison door, a Habitat guard is wounded in the breach and Habitat's Trauma crew carries him to a Clinic before he bleeds out.** Same seeds and judging, gate `test_m16b_leverage_seed_42`, L2 and M16a on:

**Asserted (mechanism and existence):**

- the kinds: contracts posted across ≥ 8 of the fourteen kinds (majority) and fulfilled in ≥ 4 of the ten 16b kinds (majority);
- leverage: coercions of each leverage (Threat, Secret, Hostage, Debt) made (existence over 42-47); ≥ 1 protection coercion paid for ≥ 3 weeks (majority); ≥ 1 blackmail `Lapsed` when its secret went public, ≥ 1 poach by threat, ≥ 1 fabricated story and ≥ 1 `Exposed`, ≥ 1 Parley (existence over 42-47 each); gang income within ±10 % of the M16a-closing run per seed (majority: the shakedown through `coerce` is the same roll);
- captives: ≥ 1 hostage taken and **≥ 1 freed by a Rescue mission** (existence over 42-47); no captive starves; captives ≤ `captive_quota` every sampled hour; `SetPrisonPermit(false)` admits no new captive (a unit test);
- tags: ≥ 1 tag planted, ≥ 1 found by a scanner, ≥ 1 Purist Patrol scan (existence over 42-47);
- the wounded: fight deaths with `wound_share > 0` never exceed the same seed's with it 0 (a unit test over the fight path, and per seed the gate's fight deaths ≤ the M16a-closing run); ≥ 1 rescue by a gang member, **≥ 1 `TraumaSave`**, ≥ 1 `BledOut` (existence over 42-47); every bleed-out is counted a Murder (exactly);
- governance: ≥ 2 board votes held on every seed of 42-44; **≥ 1 `VoteSwung`** (existence over 42-47);
- attention: on a day a faction has a live raid or riot against it, its alertness is below its pool (mechanism), and alertness never falls below `alert_floor`;
- LOD: captives, the wounded and live parties within their quotas on every sampled hour; L2's tier bound holds;
- sanity: Assault ≤ 42.7 a day; Murders per 120 days ≤ 1.10 × the M16a-closing run and ≤ 1.25 × the L2-closing run per seed (majority); starvation, population and ≥ 5 of 9 corps as in 16a; the v1, M8-M15, L2 and M16a gates and the parity tests pass; `--leverage-off` reproduces the M16a-closing city and `--contracts-off` the L2-closing city byte for byte; a version-3 save loads;
- throughput printed as in 16a.

**Printed findings:** contracts posted per 120 days (all kinds) 60-300; active coercions on day 120 10-60; hostages held at once, max 1-6; wounded ÷ (wounded + fight deaths) 20-50 %; wounded saved (rescued or Trauma) 30-70 %; Trauma subscribers on day 120 2-8 % of adults; board votes with a swung member ≤ 50 %; Murders against the M15-closing run (the original "≤ +25 %" bound, now a print).

### M16a phases

1. **The board and money.** § 1 for the four kinds (`Contract` with `Terms::Pay`, posting, escrow and the 16a flows inside L2's conservation identity), § 2 (`BuildingKind::Fixer` `X`, `Role::Fixer`, seeding after L2's `jobs::seed_venues` with a refit fallback, regulars and `Network`, visibility, daily matching, the `Contract` goal as a scripted plan through the `plan::plan_for` bypass for solo Hit, Beat, Locate and Guard, settlement, renege), Hunt hiring, the ledger resolution with pre-bound holes on L2's `Hole.source`/`Hole.faction`, `PostContract`/`TakeContract`, the CSV's 16a columns, `SAVE_VERSION` 3, the `--contracts-off` identity. Commit.
2. **Missions.** § 3's `Expedition::Mission` factoring of `raid.rs`, landed alone first with the gang and riot paths byte-identical over 120 days; then squads, the strike decision, political cost, `Shock::Trespass`/`CorpShock::Trespass`, sell out and `Order::Job`, the `faction::alertness_mult` read. Commit.
3. **The law and the street.** The `Hired` deed and the Fixer's talk, interrogation, `law::accessory_check` and `Crime::Conspiracy`, Fixer heat, the bust and bribes (`Payer::Owner`), guard corruption, the Locate stream and the law's public bounties, the brain postings of § 1 marked *a*, Fixer-fed run orders. Commit. The first `Accessory` on seed 42 is the first visible M16a.
4. **LOD, levers, UI.** § 7's 16a rows with the quotas below, `calibrate` re-run only if the parity test fails (L2's precedent), `SetFixerLicence`, `SetAccessoryMult`, `PublicBounties`, the Board panel and overlay on `C` (the spec's `J` is M15's overlay at HEAD and L2 takes `H`; verify `C` is free in `citysim-app/src/input.rs`), the Mission panel, the Fixer's Building panel, the Inspector's contracts, the City panel's Contracts section, the overlay. Commit.
5. **Gate, god scenarios, docs.** `test_m16a_contracts_seed_42`, the 16a god scenarios of § 11 (`GOD_SCENARIOS_V7.md`), findings printed, README, SPEC and this file's "Implemented: deviations", then `/code-review high <base>..HEAD` and a fix commit.

### M16b phases

1. **Leverage.** § 4 without hostages: `Coercion`, `leverage::coerce` on `moves::resolve_with`, `Terms::Coerced` appended, the shakedown routed through it, protection (L2's venues included), threats and Silence, Debt, poaching by threat, Parley, blackmail with secret tallies and lapsing, fraud (`Crime::Fraud` on renege, the fake job), `Persuade` contracts, `SAVE_VERSION` 4, the `--leverage-off` identity. Commit.
2. **Captives and the new kinds.** `Captive`, `BuildingKind::Prison` (`Z`), the `Abduct` kind on M13's `chrome::abduct` drag with `HandOver`, hostage coercions, `Rescue` missions on 16a's `Expedition::Mission`, `Escort`, `Deliver`, `Steal` (a building and, through a `RunOrder` with `RunWhy::Contract`, a Node), `Spin` with fabricated stories and exposure. Commit.
3. **The wounded.** § 5: `Wounded`, `ExecState::Down`, bleed-out, `Stabilise`, `Temperament` and the Rescue goal, `Trauma` subscriptions and calls, the StatTable's `p_killed` re-calibrated once against a Full run, the parity tests. Commit. Watch it live: fight deaths must not rise.
4. **Boards, tags and attention.** § 6 (boards, votes, the campaign, the `Vote` kind), § 3's tags and scanners with the Purist Patrol, the attention addendum (filling both `alertness_mult` stubs), `SetPrisonPermit`, the 16b god commands, the 16b panels and overlay glyphs. Commit.
5. **Gate, god scenarios, docs.** `test_m16b_leverage_seed_42`, the 16b god scenarios, findings, docs, `/code-review high` and a fix commit.

### The LOD budget (L2's rule applied)

L2 makes `max_full` and `max_coarse` real caps on ranked bodies and gives each forced class a quota inside them; held agents cost no body. M16's forced classes rank inside the budget (never "outside the rank"), so L2's asserted bound holds unchanged:

| Class | Part | `lod::class_with` rank | Quota (`[lod]`, placeholders) | Over the quota |
| --- | --- | --- | --- | --- |
| live contract parties (taker, crew, target of a `Render::Live` contract) | a | 4, with runners and hunters | `contract_quota` 24 bodies | the contract stays `Taken` and queued by price (the `max_missions` queue), promoted when bodies free, until its deadline |
| Fixer owners, from `match_hour − 2` to midnight | a | 2 | `fixer_quota` 4 | the farthest Fixer ranks as a civilian (matching runs at any tier; only the Drink talk and walk-in `Network` need a body) |
| city guards on the take | a | the watch's 3 while on shift (L2) | none new | |
| captives | b | 4 | `captive_quota` 6 | a hostage taking is not posted |
| the wounded, Down | b | 4 | `wounded_quota` 12 | the would-be wound stays a death (deaths can only fall), counted `wounds_capped` |
| Rescue and Trauma crews | b | 4 | inside `contract_quota` | queued as above; a Trauma call that cannot muster is not a save |
| Prison guards | b | L2's private guards (2) | inside L2's `private_quota` 12 | |
| board members | b | none (votes run at any tier) | | |

At most 28 bodies in 16a and 46 with 16b, ranked above gang members, so at `max_coarse` the farthest gang member over L2's `gang_quota` falls first. A gang under `Order::Job` puts its crew in `contract_quota`; its other members keep L2's Statistical GangWork day, rolled against the `(Order(Expand), district, Member)` cell (L2's ledger has no `Job` cells, and `fviolence::rebuild_active` gives `Order::Job` no touched district: the job's violence is the contract's own, live or ledger). On-screen contract killings are not counted in L2's `order_rates` (a `Contract` goal is not `GangWork` or `Raid`, the Hunt precedent), so the ledger never doubles them off screen.

### Streams

Every new roll uses `SimRng::word(WordNs, a, b)` or a pure `splitmix64` hash, never `rng.world()` or `rng.agent()`. The namespace field is bits 52-55: HEAD uses 1-8 (`Exchange … Story`) and L2's plan appends 9-14 (`Leisure, Gamble, Bout, FViolence, GangHour, Held`), so M16 appends **one** variant, `WordNs::Contract` (15 if L2 lands as planned; verify at L2's merge), and the nibble is then full. Every M16 draw shares it with a purpose byte in `a`'s top bits (the `moves::roll` idiom, `kind << 56`): `a = purpose << 56 | id`, `b` the tick, day or agent index.

| Roll | Part | Purpose | `a` low bits, `b` |
| --- | --- | --- | --- |
| renege at posting | a | 0 | contract id, 0 |
| ledger `due` tick | a | 1 | contract id, 0 |
| ledger outcome | a | 2 | contract id, attempt |
| ledger taker death | a | 3 | contract id, attempt |
| Statistical regular day | a | none: `splitmix64(seed ^ index ^ day) % 7` | |
| interrogation, coercion, Parley, blackmail, Persuade and Vote | a, b | none: the move's own `WordNs::Move` draw through `moves::resolve_with` | |
| hostage kill on expiry | b | 16 | coercion id, day |
| wound instead of death | b | 17 | loser index, tick |
| tag plant, scan | b | 18, 19 | tag id, tick |
| Trauma subscription | b | 20 | agent index, day |
| fabricated-story exposure | b | 21 | story id, holder index |
| fake job | b | 22 | faker index, day |

### Pricing on L2's wage economy

The spec's prices were set in a dole city. L2 makes wages the economy (Σ wages > Σ dole by day 60, mean wage ≈ 6.5 a day, Bars staffed at 5, leisure venues and gang fronts competing for the same evenings). `price_base` is unchanged: a Hit at 400 is about nine weeks of a mean wage and fourteen weeks of the dole, which is the intended "a price a friend has to save for". The Contract goal's `U(price ÷ week_wage)` reads L2's wages, with `week_wage = max(7 × wage, 50)`: the floor of 50 is about one week of L2's mean wage, so a jobless gun reads a job as a mean worker would. Gang buyers' `treasury ≥ price` reads treasuries that now hold L2's front take and pay out `Collect`, so the gang rows of *Who posts what* fire at L2's gang incomes, not M15's. The Fixer band compares a Fixer's weekly `FixerCut` with the median agent-owned Bar's weekly revenue less wages and upkeep in the same run, both on L2's economy, and stays a printed finding; prices move only in the late calibration milestone's reset, never in M16.

### The L2 deferrals

| L2 § 7 feature | Received by | Where |
| --- | --- | --- |
| Runner archetype with Fixer-fed run orders | M16a | § 2, *Fixer-fed run orders* |
| Guard corruption (a bribe to a guard is a contract on the law) | M16a | § 3, *Guard corruption* |
| Gang leader: Parley, delegated violence | M16b (Parley); M16a (delegated violence: the Hit posted under Raid/Retaliate) | § 4, the moves table; § 1, *Who posts what* |
| Creed: the Purist Patrol | M16b | § 3, *Bounties, tags and scanners* |

### Identifier corrections (against `d738a07`)

Each correction is also marked "(revised 2026-10-08: …)" where the text uses it.

| The spec said | HEAD (or L2 as planned) has |
| --- | --- |
| `social::resolve`, "M15's resolver" | `moves::resolve(world, &SocialMove) -> MoveOutcome` and `moves::resolve_with(world, &SocialMove, bonus)` in `citysim/src/systems/moves.rs`; `SocialMove { actor, target, kind: MoveKind, stake: Stake }`, `Stake::{Coins, Info, Job, Join}` in `word.rs`; one draw on `WordNs::Move` |
| `demography::kill` | `World::kill_by(id, DeathCause, killer)` |
| `Hole.contract` (new field) | none: L2's `Hole.source: Option<ViolenceSource>` and `Hole.faction: Option<EntityId>`, with `ViolenceSource::Contract(ContractId)` appended by M16a |
| the binder | `bind::candidates_in`/`bind_in` (private), `bind::open_hole`; `HoleKind { Robbed, Assaulted, Killed, Abducted }` fills the 2-bit field |
| `Sighting` "reuse M14/M15's" | `virt::Sighting { who, tile, tick, confidence, relayed }`, `World::db: BTreeMap<EntityId, FactionDb>`, the relay `gossip::maybe_sight` under `gossip::wants_sighting` |
| `law.last_seen` | `World::last_seen: BTreeMap<EntityId, (TilePos, Tick)>` |
| the Expedition trait | `raid::Expedition { Gang(EntityId), Riot(u32) }`; a mission is `Expedition::Mission(ContractId)`; `raiders_at` private, `gathered`, `by_strength`, `strength` (`law::fighting + 0.25 × law::courage`), `fight_out`/`fight_out_until`, `expedition_rival`, `muster_point` public |
| "the M15 daily pass" for Statistical hunters | `hunt::stat_pass`; Hunt eligibility `hunt::heaviest_eligible`, `hunt::might_gap` |
| the Hunt chain | `hunt::plan` builds `GoTo(Intel) → [AskAround → GoTo(Intel)] → StakeOut → Attack` (target in `Plan.target`) through the `plan::plan_for` bypass with `HuntState` in `World::hunts`; the `Contract` goal copies that shape (`HuntWhy` gains no variant: a contract keeps its own state) |
| a "first-hand deed memory" | a `MemoryKind::Rumour` entry (`deed`, `object`, `hops`, `conf`) in `Memory.heard`, written by `memory::hear_entry`; deeds post to pools by `gossip::post_deed`; the deed is `word::Deed` (15 variants) |
| M15's reputation | `World::reputation` (`word::Reputation { dread, standing, honour, heat, known_by, press }`), read by `reputation::rep`; `World::regard: BTreeMap<(EntityId, EntityId), Regard { value, fear }>` |
| grudges and vendettas | `word::Grudge`, `GrudgeCause` (8 variants, `Betrayed` among them); `World::vendettas: Vec<Vendetta>`; `grudges::declare(world, a, b, w)`, `Vendetta.declared`, `[grudges] declared_days` (the M15 review); `grudges::member_of` |
| `Governance::Board { members }` "the M11 D49 hook" | exists on `Corp.governance` and `Gang.governance`, always `Dictator`; nothing reads `Board` |
| the attention hook | `faction::alertness_mult(world, Option<EntityId>) -> f32` and `virt::alertness_mult` are stubs returning 1.0, read by M12's corp-raid and riot responses, `street.rs`'s sweeps and M14's node and camera rolls |
| M13's `Abduct` | `ActionKind::Abduct` (Harvest only: `ctx.in_gang`, serves `GangWork`), `chrome::abduct` (the drag, `Brain.cuffed_by`/`abducted_by`/`escorting`), the Rip at the Hideout, `chrome::abduction_daily` and `chrome::abduct_offscreen` (the victim dies into an `Abducted` hole; L2 folds the daily roll into `fviolence::daily`), `Crime::Abduction` |
| guard contracts | `Corp.contracts: Vec<(EntityId, Tick)>` (client building, until), `Flow::Contract`, renewed weekly by `corps::renew_contracts` choosing by `corps::honour_seller` (`contract_price × (1.5 − honour)`, M15 W33) |
| `CorpOrder::Spin` | does not exist (`CorpOrder` has 9 variants); Spin is M15 W40's side spend while `Corp.spin_since` is set (`news::spin_considerations`) |
| `Payer` | `faction::Payer { Gang, Corp }`; M16a appends `Owner(EntityId)`; `faction::offer_bribe(world, Payer, BribeAsk, score)` |
| the controller | `District.control: Controller { Contested, City, Gang, Corp }`, `District.control_share` |
| report ids, report victims | `CrimeReport { crime, suspect, witness, tick, resolved }`: neither exists |
| `Role` "plus `Fixer`" | `Role::ALL` is 10 at HEAD, 17 after L2; `Fixer` is the 18th |
| `BuildingKind` "plus `Fixer`, `Prison`" | 16 at HEAD (letters `H F M B J C T G W O L N R V Q P`), 23 after L2 (`K A S I D U E`); `Fixer` `X` 24th (16a), `Prison` `Z` 25th (16b; the spec's `Q` is M14's Lab) |
| `GoalKind`, `GOAL_ORDER` | 25 at HEAD, 27 after L2 (`Unwind`, `Lead`); `Contract` (16a) after `Hunt`, `Rescue` (16b) after `Contract` |
| `Order::Job` "tenth" | still the tenth (`Order::ALL` is 9; L2 adds none) |
| `WorldState` flags | 13 of `MAX_FLAGS` 52 free at HEAD, none taken by L2 |
| `SimRng::contract`, keyed streams | no such fn: `SimRng::word(WordNs::Contract, a, b)` (Streams, above) |
| the CSV | columns go before `ticks_per_sec`, after L2's `LivingCols` and `BudgetCols`; 16a's then 16b's |
| `save::SAVE_VERSION` | 1 at HEAD; L2 takes 2; M16a 3, M16b 4 |
| the conservation test | L2's identity `total_coins + Σ outside treasuries − outside.minted`, the World account at id 1069 |

### Follow-ups in `M17_OUTSIDE.md` and `M18_PLAYER.md`

Those files are left as they are; these lines need a word changed when they are next revised.

**M17** (decision on `Abroad`: **it stays M17's own kind**, appended by M17 after M16b's ten, because its target, `Target::Outside { on: OutsideId, asset }`, and the `Away` component are M17 types; it needs only 16a's contract entity, board and settlement, so it also works if M17 follows 16a):

- line 3 and line 7: "M16" → "M16a and M16b"; line 7's "M13-M16 are specified, not built" is stale (M13-M15 built; L2 between).
- line 16, line 392, line 435: the board, the buyer of record, the accessory rule and the Board panel are M16a.
- line 20 and line 190: the `Retake` mission is "M16a § 3"; its forced sale `leverage::coerce(Threat, Sell(b))` is M16b § 4, so if M17 follows 16a directly the Retake ends at the cleared building and the forced sale waits for 16b (or buys back at `value`).
- line 40, line 441, line 478, line 486: "the M16 run", "byte-identical to M16", "M8-M16" → the M16b-closing run (or M16a's if M17 follows 16a).
- line 191: the funded `Guard` contract is M16a.
- line 238: `[contracts]` additions extend M16a's section.
- line 257 and line 266: `Steal { Node(uplink) }` is an M16b kind.
- line 263 and line 264: protection rackets and `Sell` coercions are M16b.
- line 357: `SimRng::contract(id)` → `SimRng::word(WordNs::Contract, …)` with its own purpose byte (the namespace nibble is full after M16: M17 shares `Contract`); `Flow::Payout` and "the crew paid as M16" are M16a.
- line 452: `PostContract Hit` is M16a.

**M18:**

- line 5: "M16 § 6" (the board challenger) is M16b.
- line 21, line 160, line 162, line 164, line 361, line 404: the board, `contracts::visible`, `contracts::accept`/`post`, the strike estimate `p_win` and the ask's price are M16a; `social::odds` → `moves` (M15's resolver is `moves::resolve`); the ask's Court and Bury origins are M16b (`Origin::Court`, `Origin::Bury`), Hunt is M16a.
- line 84, line 105, line 299: `wound_share`, `Wounded`, Trauma Team are M16b.
- line 185, line 200, line 433, line 481: `Coerce`, `leverage::coerce`, the secret rule, `VoteSwung` and coercing a majordomo are M16b.
- line 273: Fixer offers are M16a; tags on the character's assets are M16b.
- line 290: `Render::Played`, `max_missions` and "the M16 chain" are M16a.
- line 371: god `TakeContract` is M16a, `Coerce` M16b.
- line 468: "the fixed M16 price" is M16a.
- line 480: the `Expedition` "M16 uses" is M16a's `Expedition::Mission`.

## Goals and acceptance

The city should read as a place where anything can be bought: a price on a name that the buyer thinks nobody knows, a Fixer who knows everyone's, a gang that will sell out a stranger in its own street and not its own member, a corp that keeps people in cells for leverage, and a board that votes the way its secrets say. The target spiral: **a Mid West clerk is killed by a Hollow runner → her friend's grudge cannot win a fight, so she posts a Hit with the Stack Bar Fixer → a Sump gun takes it, stakes out the runner's Home on the Bar's intel and kills him → the runner's brother hunts the gun (M15) while the law files a Murder → the gun is arrested on another charge, interrogated, names the Fixer and the buyer → Conspiracy against the friend, the Fixer's heat closes the Stack Bar office for a week → The Hollow posts a Locate bounty on the friend.** On seed 42, 2,000 residents, 120 days:

- contracts: ≥ 60 posted across ≥ 8 kinds; ≥ 25 fulfilled across **≥ 4 kinds**; ≥ 2 Fixers in business on day 120, ≥ 1 founded by an NPC; a Fixer's weekly income within 0.5–2× a Bar owner's; ≥ 1 `Reneged` followed by a grudge against the buyer;
- hits: ≥ 4 Hits fulfilled, ≥ 1 with a gang or corp buyer, ≥ 1 by a squad of ≥ 2, ≥ 1 squad strike declined on political cost, ≥ 1 job sold to the territory's gang; **≥ 1 fulfilled hit later attributed**: a `Conspiracy` filed against its buyer ≥ 1 day after the killing; clearance on contract killings (taker arrested within 30 days) 20–60 %;
- leverage: ≥ 1 hostage taken and **≥ 1 hostage rescued**; ≥ 3 blackmail coercions with ≥ 1 lapsed when its secret went public; ≥ 1 protection coercion paid for ≥ 3 weeks; ≥ 1 poach by threat; ≥ 1 fabricated story and ≥ 1 exposed;
- bounties: **≥ 1 `BountyPaid`**, ≥ 1 tag planted and ≥ 1 found by a scanner;
- the wounded: wounded are 20–50 % of would-be fight deaths; ≥ 1 rescue by a gang member, **≥ 1 `TraumaSave`**, ≥ 1 `BledOut`;
- governance: ≥ 2 board votes held, **≥ 1 `VoteSwung`** (a member whose vote under coercion differs from their own preference);
- Assault events per day stay ≤ 42.7, **Murders per 120 days rise by ≤ 25 % over the M15 run** on the same seed (contract killings included), starvation deaths and population stay within the scaled v1 bounds, and the v1, M8–M15 and Full-vs-Statistical parity gates still pass;
- throughput is reported per gate; the shared floor is 4,000 ticks/s (2026-10-06: relaxed from 8,000 while systems are still being built; optimisation is a later pass).

(revised 2026-10-08: these bullets are the whole of M16 as first drafted and are kept as its design target. For gating they are superseded by *M16a goals and acceptance* and *M16b goals and acceptance* above, which assign each bullet to a part, recast it in the gate doctrine's shape (mechanism and existence asserted, calibration bands printed as findings, judged over seeds 42-44 and 42-47) and measure Murders against the L2-closing and M16a-closing runs, since the M15 run is two milestones back.)

## 1. The contract entity

### Data model

```rust
pub type ContractId = u64;     // World::next_contract, monotonic, saved
pub type CoercionId = u64;     // World::next_coercion
pub type MissionId = ContractId; // a mission is keyed by its contract

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum ContractKind { Hit, Beat, Abduct, Rescue, Escort, Steal, Deliver, Locate, Tag, Persuade, Vote, Spin, Guard, Trauma }
// (revised 2026-10-08) M16a declares `Hit, Beat, Guard, Locate`; M16b appends
// `Abduct, Rescue, Escort, Steal, Deliver, Tag, Persuade, Vote, Spin, Trauma` (RON names variants, so order is free).

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Target {
    Agent(EntityId),
    Building(EntityId),
    Node(NodeId),                                  // M14 Data: a Steal is a heist
    Carry { what: Cargo, to: EntityId },           // Deliver: a corpse, a captive, Parts, Stims, Data
    Seat { corp: EntityId, member: EntityId, for_: EntityId }, // Vote
    Story { deed: Deed, actor: EntityId, object: Option<EntityId>, fabricated: bool }, // Spin
}   // (revised 2026-10-08) M16a: `Agent`, `Building`; M16b appends `Node`, `Carry`, `Seat`, `Story`

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Terms { Pay { price: i64 }, Coerced(CoercionId) }   // (revised 2026-10-08) M16a: `Pay` only; M16b appends `Coerced`

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ContractStatus { Open, Taken, Standing, Fulfilled, Failed, Expired, Reneged, Cancelled }   // (revised 2026-10-08) `Standing` (Trauma) appended by M16b

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Origin { Hunt, Court, Bury, GangOrder(Order), CorpOrder(CorpOrder), Law, Board, Subscription, God }
// (revised 2026-10-08) M16a: `Hunt, GangOrder, CorpOrder, Law, God`; M16b appends `Court, Bury, Spin, Board, Subscription`
// (`Spin` because M15's Spin is a side spend while `Corp.spin_since` is set, not a `CorpOrder`)

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Render { #[default] Ledger, Live, Played }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contract {
    pub id: ContractId,
    pub buyer: Option<EntityId>,                   // an M11 owner; None = the city or the law
    pub agent: Option<EntityId>,                   // the person who placed it (the buyer, an exec, a leader, the captain)
    pub kind: ContractKind,
    pub target: Target,
    pub terms: Terms,
    pub escrow: i64,                               // coins held; 0 when direct
    pub broker: Option<EntityId>,                  // the Fixer building
    pub posted: Tick, pub deadline: Tick,
    pub origin: Origin,
    pub status: ContractStatus,
    pub taker: Option<EntityId>,                   // an agent, a gang, a corp or a Feed
    pub crew: SmallVec<[EntityId; 4]>,
    pub attempts: u8,
    pub risk: f32,                                 // 1 − p_success for the median gun, at posting
    pub political: f32,                            // § 3, at posting, refreshed at the strike
    pub renege: bool,                              // decided at posting, hidden (§ 4 fraud)
    pub due: Option<Tick>,                         // the ledger resolution tick
    pub render: Render,
    pub known_by: SmallVec<[EntityId; 8]>,         // who knows the buyer is behind it
}
```

`World::contracts`, `World::coercions: BTreeMap<CoercionId, Coercion>` (§ 4), `World::missions: BTreeMap<MissionId, Mission>` (§ 3), `World::next_contract`, `World::next_coercion` are saved. (revised 2026-10-08: `coercions` and `next_coercion` are M16b's, `#[serde(default)]`.) Closed contracts stay `closed_keep_days` for the biography and the Board panel's history, then drop; `World::contract_log` keeps a ring of 512 one-line outcomes.

### Price, risk and posting

`contracts::post(world, Posting) -> Result<ContractId, Refusal>` is the one entry point for brains, agents, god commands and, later, the player. The price is `price = price_base[kind] × (1 + risk_w × risk) × (1 + standing_w × standing(target)) × price_level_b`, where `risk` comes from the same estimate the strike uses (§ 3) against a median gun (fighting 0.5, Kit 0), `standing` is M15's, and `price_level_b` is the broker's (1.0 direct). The buyer must hold `price` in its purse; a brokered post moves it to `escrow` (`Flow::Escrow`), a direct post moves nothing. `Hired` is posted as a first-hand deed memory in the placing agent (and in the Fixer on a brokered post), so the knowledge exists from the first tick. (revised 2026-10-08: the purse is `World::purse(owner)` and payments go through `ownership::pay`/`charge`; a first-hand deed memory at HEAD is a `MemoryKind::Rumour` entry with `deed: Some(Hired)`, `object: Some(target)`, `hops: 0`, `conf: 1.0` in `Memory.heard`, written by `memory::hear_entry`; escrow is counted by `ownership::total_coins`, so L2's identity `total_coins + Σ outside treasuries − outside.minted` covers it; `standing` is `reputation::rep(world, target).standing`.) `renege = hash(seed, id) < renege_base × (1 − lawfulness) × (1 − honour)` of the placing agent on direct posts only. (revised 2026-10-08: the hash is a `SimRng::word(WordNs::Contract, …)` draw, purpose 0, per the Streams table.)

### Who posts what

Brains' orders and personal goals become contracts only where this table says; everything else keeps its M8–M15 rule. (revised 2026-10-08: the last column marks the part. A gang row fires at the faction level, at the gang's daily rescore, at any tier: L2's Statistical GangWork day for the gang's other members is untouched, and a Job crew is promoted under `contract_quota`.)

| Source | Trigger | Contract | Part |
| --- | --- | --- | --- |
| Hunt (M15) | an eligible grudge whose Hunt scores with `might_gap < hire_gap`, holder's coins ≥ price; Statistical holders in the M15 daily pass (revised 2026-10-08: `hunt::heaviest_eligible`, `hunt::might_gap`; Statistical holders in `hunt::stat_pass`; `[hunt] lethal_min`) | `Hit` when `weight ≥ lethal_min`, else `Beat`, target the grudge target | a |
| Court | a suitor rejected twice (`Rejected` memories) with coins ≥ price and sociability < 0.4 | `Persuade` (Charm on the courted, stake: affinity toward the buyer) | b |
| Bury | a kin corpse unburied 24 h, no living adult kin at Full or Coarse, coins ≥ price | `Deliver { Corpse → Recycler }` | b |
| Gang Raid / Retaliate | `ratio(own, rival) < hire_ratio` and treasury ≥ price | `Hit` on the rival leader (direct to a lieutenant's edges, or brokered when a Fixer is in a held district) | a (revised 2026-10-08: the treasury now holds L2's front take; this is the delegated violence L2 § 7 left to M16) |
| Gang BreakOut | the same deficit | `Rescue { the boss at the Precinct }`: the crew joins the breach as raiders | b (revised 2026-10-08: dropped by the M16 plan, C42: a hired crew inside the gang breach would enter the `Expedition::Gang` arm, which must stay byte-identical) |
| Gang Harvest (M13) | fit members < 3 | `Abduct` of the best visible-chrome target, price `abduct_share × chrome_value` | b (revised 2026-10-08: the target is `chrome::gang_harvest_target`) |
| Gang vendetta (M15) | an open vendetta with no fresh sighting of the rival leader | `Locate` bounty on the rival leader | a (revised 2026-10-08: "no fresh sighting" = no `virt::Sighting` of the leader younger than `fresh_sighting_hours` in the gang's `World::db` entry; vendettas are `World::vendettas`) |
| Corp Secure | a building with a loss in 7 days, no affordable guard contract | `Guard` on the building; `Locate` on the culprit when one is named | a |
| Corp Lobby | exec lawfulness < `dirty_lobby` | `Beat` (or `Hit` below `dirty_lobby / 2`) on the culprit gang's leader instead of the bribe | a |
| Corp Acquire | the weakest rival refuses the offer (M15 honour or not bankrupt) | a `Coercion` over its exec (§ 4), demand `Sell(building)` | b |
| Corp Research (M14) | no own runner, a rival Lab in `focus` with Data | `Steal { Node }`: a heist by a freelance runner, payload to the buyer | b |
| Corp Spin (M15) | while held (revised 2026-10-08: while `Corp.spin_since` is set, M15 W40's side spend beside the corp-wide order; there is no `CorpOrder::Spin`) | `Spin` (planting or burying), taken by the Feed; `fabricated` per § 4 | b |
| Law | a wanted suspect unlocated ≥ `law_bounty_days`, or an escaped convict | public `Locate` (buyer `None`, visible to everyone); a captain with lawfulness < `death_squad_lawfulness` posts `Hit` on the highest-heat gang leader | a (revised 2026-10-08: a public bounty sets `World::last_seen`) |
| Board | § 6 campaign | `Vote` and `Persuade` | b |
| Any adult | § 5 subscription rule | `Trauma` | b |

## 2. Fixers, the board, hired guns

### The Fixer

`BuildingKind::Fixer` (appended; label "Fixer", letter `X`), `[buildings] fixer = { capacity = 8, stock_cap = 0, staff = 1 }`, foundable through `Register` (`found_cost.fixer` 400, `value` 500, upkeep 8), hired `Role::Fixer` (label "Fixer") sorted by persuasion + knowledge. An agent founds one through M11's `Found` goal when `lawfulness < fixer_lawfulness` and `persuasion + knowledge ≥ fixer_skill`; two are seeded by `founding::build_on_lot` on Lots in Mid East and Sump Central, owned by the jobless adult with the lowest lawfulness living nearest, as M11 dealt Bars. (revised 2026-10-08, M16a: `BuildingKind::Fixer` is appended after L2's seven kinds (the 24th; letter `X`, free at HEAD and after L2's `K A S I D U E`), joins `FOUNDABLE` after L2's twelve, and its staff role `Role::Fixer` is the 18th after L2's seven, its shift `ClerkWork` at its employer (the L2 and M15 precedent) at `[economy] wage_fixer` 6, on L2's wage scale. Seeding runs in `World::new` after L2's `jobs::seed_venues` (which takes Lots first); with no vacant Lot in the district it takes one in an adjacent district, else refits a derelict through L2's `founding::refit` at no charge (seeded, as L2 seeds venues), else is skipped and logged. A Fixer often stands behind a Bar or an L2 Den; it is never itself a Den.)

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Broker {                            // on the Fixer building
    pub book: Vec<ContractId>,                 // open and taken, sorted
    pub regulars: BTreeMap<EntityId, Tick>,    // last Network visit
    pub cut: f32,                              // fixer_cut at founding
    pub heat: f32,                             // 0..=1
    pub closed_until: Option<Tick>,
    pub income: VecDeque<i64>,                 // per day, last 14
}
```

**Regulars.** `Network` (new `ActionKind`, dur 30, at a Fixer) marks the agent a regular for `regular_days`. Earn gains the option `GoTo(Fixer) → Network` for adults who are jobless or under-paid and have fighting, stealth, hacking, persuasion or intimidation ≥ `gun_skill_min`, with `1 − lawfulness` → Linear{0.6,0.4}; a gang member networks for their gang. Statistical regulars are written by the daily pass for adults whose Trace shows them in the Fixer's district and who pass the same gate, on one hash-picked day in seven.

**Fixer-fed run orders** (added 2026-10-08, M16a; L2 § 7 deferred the runner archetype here). At matching, after its book, an open Fixer hands at most `run_offers_per_day` (1) run order to a regular with hacking ≥ `gun_skill_min`, a deck in `Kit` or a public terminal within reach, and no standing `RunOrder`: M14's own `virt::RunOrder { patron: None, purpose: Purpose::Data { wipe: false }, target, chair, not_before: now, expires: now + 1 day, why: RunWhy::Fixer, mode }`, with `target`, `chair` and `mode` taken from M14's freelance scorer (`systems::virt::hack_offer`) for that runner, and `RunWhy::Fixer` appended to `RunWhy`. The run is M14's, unchanged, and `systems::virt::hack_plan` keeps a non-freelance order as it keeps a standing one. At the runner's `SellData` the Fixer's owner takes `fixer_cut` of the sale (`Flow::FixerCut`, taxed), and the runner stays a regular for `regular_days` without a `Network` visit. No `ContractKind` is involved: the Fixer brokers M14's Data demand to a freelance runner. A buyer-escrowed heist on a named node is M16b's `Steal { Node }`, which reuses the same `RunOrder` with `RunWhy::Contract`. A runner holding a Fixer order ranks in class 4 as M14 V13 runners do (L2's quota for that class: seated runners, bounded by chairs).

**Visibility: the board.** `contracts::visible(world, viewer) -> Vec<ContractId>` is what the Board panel, the Contract goal and M18's quest log read. A brokered contract is visible to the regulars of its Fixer and to gangs holding or adjacent to the Fixer's district; a direct contract to the buyer's edges (any `RelKind` but Enemy), the buyer's gang or corp staff; a public contract (buyer `None`) to everyone; `Guard`, `Escort` and `Trauma` to Security corps; `Spin` to Feeds. God mode sees all. (revised 2026-10-08: M16a has `Guard` for Security corps; `Escort`, `Trauma` and `Spin` arrive with M16b. A direct `Guard` posted by a suspect is also visible to on-shift city guards of the buyer's district: *Guard corruption*, § 3.)

### Matching (daily, at `match_hour`)

Each open Fixer ranks its book by `price × (1 + age_days ÷ deadline_days)` and makes up to `offers_per_day` offers: for each contract, the best eligible taker by `fit = skill[kind] × (1 − risk_for(taker)) × (0.5 + 0.5 × honour(taker))`, where `skill[kind]` is fighting (Hit, Beat, Abduct, Rescue, Escort, Guard), stealth (Steal, Tag, Locate), hacking (Steal on a Node), persuasion or intimidation (Persuade, Vote), and `risk_for` the strike estimate with the taker's own `fi`. (revised 2026-10-08: M16a's rows are Hit, Beat, Guard (fighting) and Locate (stealth); the others arrive with their M16b kinds. `fi` is `law::fighting`, which already folds `Kit.fighting`.) Eligibility: a regular, not the target, not the target's Spouse, Family or Friend, `lawfulness ≤ gun_lawfulness[kind]` for an illegal kind, free and not jailed, not already holding a contract. The taker **accepts** when the Contract goal's score (below) ≥ `accept_min`; a refusal moves the offer to the next candidate. A gang (through its leader's brain) and a Security corp are candidates too, scored by their best member. On acceptance: `status = Taken`, `taker`, a crew of up to `crew_max` of the taker's gang members or Friends who are regulars when `risk > squad_below`, `known_by` += taker and crew, `ContractTaken` event. The Fixer's cut is paid at fulfilment, never at acceptance.

### The Contract goal

| Goal | Considerations | Plan |
| --- | --- | --- |
| Contract | `Can(holds a Taken contract ∧ free)` → GATE; `U(price ÷ week_wage)` with `week_wage = max(7 × wage, 50)` (revised 2026-10-08: `wage` is L2's, mean ≈ 6.5, so the floor of 50 is about a week of the mean wage; see *Pricing on L2's wage economy*) → Logistic{8,0.3}; `1 − risk_for(self)` → Linear{0.7,0.3}; buyer honour (direct only) → Linear{0.5,0.5}; `1 − lawfulness` (illegal kinds) → Linear{0.6,0.4}; `courage` (violent kinds) → Linear{0.5,0.5}; urgency `1 − time_left ÷ total` → Linear{0.5,0.5}; flat `contract_flat` | per kind, below |

`GoalKind::Contract` sits after `Hunt` in `GOAL_ORDER`. Plans reuse existing chains: (revised 2026-10-08, M16a: `GOAL_ORDER` is 25 at HEAD and 27 after L2's `Unwind` and `Lead`; `Contract` makes 28. Its plans are **scripted**, built through the `plan::plan_for` bypass exactly as `GoalKind::Hunt`'s `hunt::plan` (`GoTo(Intel) → [AskAround → GoTo(Intel)] → StakeOut → Attack` with the target in `Plan.target`) and `GuardBody`'s, never searched by A*, with the contract's own intel state beside `World::hunts`'s `HuntState` shape; the last column of the table is the part.)

| Kind | Plan | Part |
| --- | --- | --- |
| Hit, Beat | M15's Hunt chain without a grudge: `[AskAround] → GoTo(Intel) → StakeOut → Attack(target)`, lethal per kind; a squad goes as a mission (§ 3) | a |
| Abduct | `GoTo(Intel) → StakeOut → Abduct` (M13) `→ GoTo(Holding) → HandOver` | b (revised 2026-10-08: M13's `ActionKind::Abduct` is gated to Harvest GangWork at HEAD; 16b widens `allowed` to the `Contract` goal and runs `chrome::abduct`'s drag, ending in `HandOver` instead of the Rip) |
| Rescue | a mission on the holding building (§ 3); the freed captive follows the crew home | b |
| Escort, Guard | `GoTo(Client or Building) → Guard(hours)`: stand within 2 tiles, defend in any brawl or assault on the client (the defender list of `fight_out` and `Attack` gains guards on contract) | a (Guard), b (Escort) |
| Steal | a building: M13's theft family on the target; a Node: M14's Hack with `patron = buyer` and the payload delivered to the buyer's store | b |
| Deliver | `GoTo(Cargo) → PickUp / CarryCorpse → GoTo(To) → HandOver` | b |
| Locate, Tag | `[AskAround] → GoTo(Intel) → StakeOut` writes the sighting; Tag adds `PlantTag` | a (Locate), b (Tag) |
| Persuade, Vote | `GoTo(Intel) → Coerce` (§ 4) with the contract's stake | b |

**Settlement** (`contracts::settle`): Fulfilled pays `escrow × (1 − cut)` to the taker (`Flow::Payout`, split equally with the crew) and `escrow × cut` to the Fixer's owner (`Flow::FixerCut`). A direct contract pays from the buyer's purse unless `renege`, in which case it is `Reneged`: a `Betrayed` deed (actor buyer) posted at `reach0.betrayed`, and the taker gains `Grudge { cause: Betrayed, weight: renege_grudge }`, which M15's Hunt reads. A taker who dies, is jailed, or loses the strike fails the attempt: back to `Open` with `attempts + 1` until `max_attempts`, then `Failed`; at `deadline` `Expired`. Both refund escrow (`Flow::Escrow`, reversed). `ContractPosted`, `ContractTaken`, `ContractFulfilled`, `ContractFailed`, `ContractExpired`, `Reneged` events. (revised 2026-10-08: the grudge goes in through M15's `Grudges` store (`word::Grudge { target, cause, weight, since, chain, … }`, `GrudgeCause::Betrayed` exists); the deed is posted with `gossip::post_deed`. M16a ends a renege there; M16b appends the `Crime::Fraud` filing of § 4.)

### Hired guns at the Statistical tier

A Statistical taker is offered and accepts exactly as a Full one (the score reads Personality, Skills and purse). If the target or any crew member is Full or Coarse, pinned, or in view, the contract is `Render::Live` and the taker is promoted to Coarse with Contract pinned, bounded by `max_missions` (queued by price). (revised 2026-10-08: and by L2's budget: live parties rank in class 4 inside `[lod] contract_quota` bodies, the *LOD budget* table above.) Otherwise it is `Render::Ledger` with `due = posted + hash(seed, id) % (deadline − posted) ÷ 2`. Promotion of either party before `due` turns a ledger contract live.

```toml
[contracts]
enabled = true
max_open = 256
closed_keep_days = 30
price_base = { hit = 400, beat = 120, abduct = 300, rescue = 350, escort = 80, steal = 150, deliver = 40, locate = 20, tag = 150, persuade = 100, vote = 250, spin = 120, guard = 25, trauma = 35 }   # (revised 2026-10-08) M16a reads hit, beat, guard, locate; the other keys arrive with M16b (serde default)
deadline_days = { hit = 10, beat = 7, abduct = 10, rescue = 7, escort = 3, steal = 7, deliver = 3, locate = 14, tag = 10, persuade = 7, vote = 14, spin = 5, guard = 7, trauma = 7 }
risk_w = 1.5
standing_w = 1.0
max_attempts = 2
accept_min = 0.35
contract_flat = 0.0
renege_base = 0.3
renege_grudge = 0.6
hire_gap = -0.1
hire_ratio = 0.7
abduct_share = 0.3
dirty_lobby = 0.3
law_bounty_days = 3
death_squad_lawfulness = 0.2
[fixers]
fixer_cut = 0.2
fixer_lawfulness = 0.35
fixer_skill = 0.8
regular_days = 14
gun_skill_min = 0.45
gun_lawfulness = { hit = 0.3, beat = 0.4, abduct = 0.3, steal = 0.4, tag = 0.5, persuade = 1.0, vote = 0.6, rescue = 0.7 }   # M16a: hit, beat
run_offers_per_day = 1             # (added 2026-10-08, M16a) Fixer-fed run orders
match_hour = 18
offers_per_day = 4
crew_max = 4
[buildings]                        # addition
fixer = { capacity = 8, stock_cap = 0, staff = 1 }
[economy]                          # addition (2026-10-08, M16a; L2's wage scale)
wage_fixer = 6
[lod]                              # additions (2026-10-08, M16a; L2's budget)
contract_quota = 24
fixer_quota = 4
[corps]                            # additions
found_cost = { fixer = 400 }
value = { fixer = 500 }
upkeep = { fixer = 8 }
```

## 3. Missions, hit squads and the law

### The mission

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mission {
    pub contract: ContractId,
    pub crew: SmallVec<[EntityId; 6]>,      // the taker first
    pub muster: TilePos,                     // the crew's nearest held Home, squat or the Fixer
    pub door: TilePos,                       // the target's intel tile or the building's door
    pub raid_at: Tick,
    pub stream_by: Option<EntityId>,         // M14 Overwatch
    pub defenders_hint: u8,                  // allies expected at the door, for the panel
}
```

`raid.rs` is factored, not forked: `Expedition` (a trait over `&Gang` and `&Mission` with `door`, `raid_at`, `crew`, `is_live`) feeds `raiders_at`, `depart`, and a `fight_out` that already takes two lists. The gang path keeps its code and calibration. A mission's plan is `GoTo(MusterPoint) → Muster → GoTo(MissionDoor) → Brawl`, the same actions with `LocationKey::MissionDoor`. **Defenders** at a person: the target, their Escort and Guard contractors within `raid_gather_radius`, gang members and Friends within it whose courage ≥ 0.5, the building's powered robots and private guards when the door is a building's. A Rescue's defenders are the holding gang's members inside or the Prison's guards and robots. M14's door, robot and camera runs apply to a mission's building door as to a raid. Crossfire (M12) applies. (revised 2026-10-08, M16a: at HEAD `raid.rs` dispatches on the enum `raid::Expedition { Gang(EntityId), Riot(u32) }` through `expedition_of`, `departure`, `raid_pending`, `raid_done`, `mustered`, `target_tile`, `muster_point`, `depart`, `resolve`, `wait_for_crew` and the private `raiders_at`; the mission is a third variant, `Expedition::Mission(ContractId)`, each fn gaining a `Mission` arm with the `Gang` and `Riot` arms moved into `match` arms token for token. Defenders at a building read `raid::private_guards_of`, `raid::posted_guards` and `raid::robots_first`; the brawl is `raid::fight_out`/`fight_out_until`, whose pairing order is `raid::by_strength`. The first commit of 16a's phase 2 is this factoring alone, with a byte-identical 120-day gang and riot CSV.)

### The strike decision

At `Muster` the leader of the crew re-reads the freshest intel (M15: own and buyer's `FactionDb` sightings, Locate streams, tags) and decides: (revised 2026-10-08: the intel is `virt::Sighting`s in `World::db` for the buyer's faction and the taker's own gang, plus `hunt::fresh_intel`/`hunt::habit`; tags are M16b's.)

- `p_win` from the brawl estimate: crew `Σ strength` against the expected defenders' `Σ strength` (the same `strength = fi + 0.25 × courage` as `by_strength`), `p_win = logistic(strike_k × (S_crew ÷ max(S_def, 0.1) − 1))`.
- The **controller** `C` of the door's district (M12). `political = 0` when `C` is the buyer, the taker's gang, `City` or `Contested`; else `political = pol_w × control_share × (0.5 + 0.5 × Regard(C, buyer).fear) × (1 + max(0, Regard(C, buyer).value))`, M15's Regard. A target who is a member or employee of `C` doubles it. (revised 2026-10-08: `C` is `District.control: Controller { Contested, City, Gang(id), Corp(id) }` and `control_share` is `District.control_share`; `Regard` is `World::regard[(C, buyer_faction)]`, `Regard { value, fear }`. M16a multiplies the defenders' `Σ strength` by `faction::alertness_mult(world, Some(C))` (or `None` for the City's law), the attention addendum's hook, a stub returning 1.0 at HEAD that M16b fills.)
- **Strike** when `p_win × price − (1 − p_win) × price × loss_w − political × price ≥ 0`; else **hold** a day (re-read intel) up to `hold_days`, then **sell out** or fail.
- **Sell out.** When `C` is a gang, the target is not its member, and `Regard(C, target's faction).value < sell_regard`, the contract is offered to `C` at `price × sell_premium` (the buyer tops up escrow or the offer lapses); `C`'s leader accepts with `L.greed` → Linear{0.6,0.4}, `1 − L.lawfulness` → Linear{0.5,0.5}, `fear` of the buyer → Linear{0.5,0.5} ≥ `accept_min`, and `C` executes it under `Order::Job`. `SoldOut` event.

A strike in another faction's district pushes `Shock::Trespass { by }` (0.4) on a gang `C` or `CorpShock::Trespass` (0.3) on a corp `C`, and a `Raided` deed (actor the buyer's faction) into the district pool: the political cost is paid in Regard and vendetta weight, through M15.

`Order::Job` (tenth gang `Order`, `is_raid()` true when the job is a mission): `Can(a Taken contract held by the gang)` → GATE; `price ÷ hoard_heat` → Logistic{8,0.3}; `L.greed` → Linear{0.5,0.5}; `1 − target_cover` → Linear{0.8,0.2}; flat `order_flat.job` (0.0). It holds until the contract settles. (revised 2026-10-08: `Order::ALL` is 9 at HEAD and L2 adds none, so `Job` is still the tenth. Following the M16 plan (C26), `is_raid()` is **false** for `Job`: the job's march is its own `Expedition::Mission`, so no gang-raid path sees it. L2: `fviolence::rebuild_active` gives `Order::Job` no touched district, and members outside the crew roll L2's Statistical GangWork day against the `Order(Expand)` cell.)

### Bounties, tags and scanners

`Locate` is a stream: every `Sighting` of the target written by a viewer of the contract (§ 2 visibility) is copied to the buyer's `FactionDb`, and the writer is paid `per_sighting` from escrow, at most one paid sighting per `min_gap_hours` and `cap_sightings` in all (`BountyPaid` event; `Flow::Payout`). A public bounty by the law is paid by the Treasury and sets `law.last_seen`. (revised 2026-10-08, M16a: the sighting writers at HEAD are the relay `gossip::maybe_sight` (members' and guards' eyes, gated by `gossip::wants_sighting`), M14's camera writes, the law's private `law::sightings` pass and a `StakeOut`'s success; the hook is one call where each pushes into `FactionDb.sightings`. The law's public bounty is paid from the Treasury (`World::purse(None)`) and sets `World::last_seen`. A Locate is never taken: it stays `Open` until `cap_sightings` are paid or its deadline, the M16 plan's C27.)

`Tag` plants `AssetKind::Tag` (M13 asset, tier 1–3, sold by Security Offices at `tag_price[tier]`) with `PlantTag` (new `ActionKind`, dur 2, adjacent): `security::contest(stealth tier of the taker, sight tier of the target)` where the target's tier is `1 + Eyes tier`. On success the tag is an `Asset` with `loc = On(target)`, owner the buyer, and writes `Sighting { conf: tag_conf[tier] }` to the buyer's db once an hour for `tag_battery_days`. `AssetKind::Scanner` (tier 1–3, `scanner_price`) is posted at Clinics and Security Offices; `Scan` (new `ActionKind`, dur 10, `scan_price`) at a building with a scanner contests `scanner tier` against `tag tier`, a win destroys every tag on the agent (`TagFound` event, a `Tagged` memory naming the tag's owner when the contest margin is ≥ 1). Adults with `heat ≥ scan_heat` or a hunter on them add Scan to the Shop goal. Tags are capped at `max_tags` city-wide; the hourly write is O(tags). (revised 2026-10-08: M16b. `AssetKind` ends with M14's `Camera` at HEAD; `Tag` and `Scanner` are appended after it and `AssetLoc::On(EntityId)` after `Limbo(HoleId)`; the contest is `security::contest(attacker_tier, defender_tier, step, &mut rng)` on the Contract stream (purposes 18 and 19); a Kit's sight tier is `Kit.sight`.)

**The Purist Patrol** (added 2026-10-08, M16b; L2 § 7 deferred the creed's anti-chrome vigilantes here because a patrol reads chrome the way a scanner reads tags). A Purist gang (M15's creed; The Unplugged, `creeds::seed_unplugged`) under Expand sends `patrol_share` of its GangWork to a Patrol: `GoTo(the busiest of L2's `leisure::spots` in a held district) → Scan` with the gang's tier-1 scanner (bought once as a Secure corp buys a camera, from its treasury, `Flow::Asset`), contesting the scanner's tier against each co-located adult's chrome visibility (`Kit.visible`, a tier 0-3 read as the defender's tier) instead of a tag's; a find destroys any tag on that adult and starts the member's `Attack` on that adult (an Assault under the normal witness, report and arrest rules), not a contract. `PatrolScan` events and a `patrols` counter. `[leverage] patrol_share = 0.3` (placeholder).

### The ledger resolution

At `due` a ledger contract draws once from `SimRng::contract(id)` with the strike's `p_win` (defenders from the target's Trace habit: Home at night, else the workplace). Hit: a win kills the target (`demography::kill`, `DeathCause::Violence`) and opens a pre-bound `HoleKind::Killed` hole with `contract = Some(id)`; Beat opens `Assaulted`; Steal on a building opens `Robbed` with `loot` = the take; Locate pays `cap_sightings ÷ 2` sightings; Persuade and Vote resolve the social move at any tier (no body needed, as M15 poaching). A loss is a failed attempt; when the target's might wins by > 0.3 the taker dies (`p_win < ledger_death_p` of the draw), opening a `Killed` hole on the taker pre-bound to the target. Abduct, Rescue, Escort, Guard, Trauma and squads are **never** ledger: they promote. (revised 2026-10-08: the draw is `SimRng::word(WordNs::Contract, …)` purpose 2 (there is no `SimRng::contract`); the kill is `World::kill_by(target, DeathCause::Violence, Some(taker))`; the pre-bound hole is opened through `bind::open_hole` with L2's fields `source = Some(ViolenceSource::Contract(id))`, `faction = Some(taker)` (the taker's death: `faction = Some(target)`), and `bind_in` on that source discards the Unknown and candidate draws, binds the one agent and rolls the witness as today. M16a resolves Hit, Beat and Locate here; Steal, Persuade and Vote are M16b's rows; the `due` tick is `taken + …`, not `posted + …`, as the M16 plan's C20 corrected. The defenders' habit is `hunt::habit(world, target, due)`.)

### The law's view

A fulfilled Hit is a `Murder` by the taker under the normal report, witness and arrest rules; a Beat an `Assault`. Nothing about the contract is visible to a witness.

- **The `Hired` deed** (appended to M15's `Deed`; `reach0 = 0.3`, `deed_sal = 0.7`, `deed_sev = 0.7`, `heat_w = 0.4`, `honour_w = −0.2`) is known first-hand to `known_by`. It travels by gossip; a Fixer speaking at Drink passes `Hired` with `p = fixer_talk × (1 − Fixer heat)` only to other regulars. Kin of the target who learn `Hired` form a grudge on the buyer (`GrudgeCause::Hired`), so revenge can reach the buyer. (revised 2026-10-08, M16a: `word::Deed` has 15 variants at HEAD; `Hired` is the 16th, and every `DeedTable` (`reach0`, `deed_sal`, `deed_sev` in `[gossip]`, `dread_w`, `honour_w`, `heat_w` in `[reputation]`) gains a `hired` field with a serde default; the knowledge is a `MemoryKind::Rumour` entry with `deed: Some(Hired)` in `Memory.heard`; `GrudgeCause::Hired` is appended to the 8 at HEAD.)
- **Interrogation.** When a taker of a fulfilled Hit or Beat within `interrogate_days` is jailed for any crime, the arresting guard makes an Intimidate move (M15 `resolve`, guard `skill_a = intimidation + 0.3 × Law.competence`, `bias = interrogate_bias`) against the taker; success puts the buyer and the Fixer in the guard's memory as `Hired` at `hops = 1, conf = 1`. (revised 2026-10-08: the move is `moves::resolve_with(world, &SocialMove { actor: guard, target: taker, kind: MoveKind::Intimidate, stake: Stake::Info { about: buyer } }, 0.3 × Law.competence)`, run in `law::jail_suspect` after the sentence; its draw is the move's own `WordNs::Move` stream; the M15 move has no `bias` argument, so `interrogate_bias` is added to the bonus.)
- **Conspiracy.** `law::accessory_check`, daily: for every open Murder or Assault report whose victim was the target of a fulfilled contract, if any guard or the report's witness holds `Hired` about that victim with `conf ≥ accessory_conf`, file `Crime::Conspiracy` (label "Conspiracy", severity just below Murder, sentence `accessory_mult × sentence(Murder)`) against the placing agent (`Contract.agent`; a corp's exec, a gang's leader, the captain when the law posted it). `Accessory` event. M15's street silence applies to witnesses, not to guards. (revised 2026-10-08: `CrimeReport { crime, suspect, witness, tick, resolved }` has no victim at HEAD, so the check is **keyed on contracts**, as the M16 plan's C31 decided: for each `Fulfilled` Hit or Beat closed within `accessory_days` whose placing agent is alive, free and not yet charged for it, if any city guard, or any witness holding a `SawCrime` of the strike, holds a `Hired` rumour with object the target and actor the buyer (a gang or corp buyer maps to the placing agent) at `conf ≥ accessory_conf`, then `law::file_report(world, Crime::Conspiracy, agent, Some(holder))`. `Crime::Conspiracy` is appended to `Crime` and ordered by `Crime::severity()` just below Murder.)
- **The Fixer's heat** rises `heat_per_hit` per fulfilled Hit and `heat_per_named` each time an interrogation names the Fixer, decays `heat_decay` a day. At `warrant_heat` the owner is a Conspiracy suspect and the office closes for `close_days` (`closed_until`, `FixerBusted`), with its book's open contracts refunded. A Fixer may bribe: M9's `offer_bribe` with `Payer::Owner`, scored with heat → Linear{0.8,0.2}, lowers its heat by `bribe_cool` on a take. (revised 2026-10-08: the bribe is `faction::offer_bribe(world, Payer::Owner(owner), BribeAsk::LookAway, score)` with `Payer::Owner(EntityId)` appended to `faction::Payer { Gang, Corp }`, paid from the owner's wallet as `Flow::Bribe`.)
- **Guard corruption** (added 2026-10-08, M16a; L2 § 7 deferred it here as "a bribe to a guard is a contract on the law"). A Fixer owner with heat ≥ `corrupt_heat`, a gang leader under a Crackdown, or a suspect with an open report and coins ≥ the price may post a **direct `Guard` contract on itself** (`Target::Agent(buyer)`, or its Fixer office as `Target::Building`), visible to the on-shift city guards whose district is the buyer's (§ 2) as well as the usual direct viewers. A city guard is eligible when its lawfulness < `corrupt_lawfulness` and it is not the buyer's arresting guard on a live arrest; it accepts by the Contract goal's score like any taker. While the contract is `Taken` the guard is **on the take**: `law::chaseable` skips the buyer (and, for a gang buyer, its members) for that guard, the guard's own witness memories of the buyer's crimes are not reported, and a Fixer buyer's heat falls by `bribe_cool` once. Nothing else changes: the guard still patrols, the captain's posture and every other guard are untouched, and the take is known to `known_by` as a `Hired` rumour with actor the buyer and object the guard, so interrogation and gossip can expose it; M16a charges no crime for it (a `Bribery` crime is a finding for the late milestone). `GuardTaken` event when the taker is a city guard; `guards_on_take` counter. A Security corp taking a `Guard` contract is converted, as before, into an M11 guard contract (`Corp.contracts`, `Building.secured_by`, renewed weekly by `corps::renew_contracts`).

```toml
[missions]
max_missions = 12
squad_below = 0.6
strike_k = 3.0
loss_w = 0.5
pol_w = 1.0
hold_days = 2
sell_premium = 1.5
sell_regard = 0.0
ledger_death_p = 0.15
[bounty]                           # (revised 2026-10-08) M16a: per_sighting, cap_sightings, min_gap_hours; the tag and scanner keys are M16b's
per_sighting = 15
cap_sightings = 10
min_gap_hours = 6
tag_price = [0, 150, 400, 900]
tag_conf = [0.0, 0.7, 0.85, 0.95]
tag_battery_days = 10
max_tags = 32
scanner_price = [0, 300, 800, 2000]
scan_price = 40
scan_heat = 0.4
[law]                              # additions
interrogate_days = 30
interrogate_bias = 0.0
accessory_conf = 0.5
accessory_mult = 0.75
heat_per_hit = 0.15
heat_per_named = 0.3
heat_decay = 0.02
warrant_heat = 0.8
close_days = 7
bribe_cool = 0.3
fixer_talk = 0.3
accessory_days = 60                # (added 2026-10-08, M16a) the plan's C31 window
corrupt_lawfulness = 0.3           # (added 2026-10-08, M16a) guard corruption
corrupt_heat = 0.3
[gangs.order_flat]                 # addition
job = 0.0
```

## 4. Leverage

(revised 2026-10-08: all of § 4 is **M16b**. M16a ships none of it: no `Coercion`, no `Terms::Coerced`, and the shakedown stays M15's `gang::extort` roll.)

### Data model

```rust
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Leverage {
    Threat,                                         // credibility = might_gap and dread, M15
    Secret { deed: Deed, actor: EntityId, object: Option<EntityId>, day: u32 },
    Hostage { captive: EntityId },
    Debt { amount: i64 },                           // an Edge.debt called in
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Demand {
    Coins { amount: i64, weekly: bool },            // a shakedown, protection, hush money, a ransom
    Do(ContractId),                                 // a coerced job (Terms::Coerced)
    Sell(EntityId),                                 // a building to the holder at value
    Join { building: EntityId, wage: i64 },         // poaching by threat
    Vote { corp: EntityId, for_: EntityId },
    Release(EntityId),                              // a captive or a convict (via the captain)
    Silence { report: u64 },                        // withdraw testimony: the report loses its witness
                                                    // (revised 2026-10-08: reports have no id at HEAD: `Silence { suspect: EntityId, tick: Tick }` names the report)
    Access(EntityId),                               // a door: DoorOpen for door_hours (M14's effect)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Coercion {
    pub id: CoercionId,
    pub holder: Option<EntityId>,                   // an M11 owner
    pub agent: EntityId,                            // who made the move
    pub over: EntityId,                             // the coerced agent (an exec or leader for a faction)
    pub leverage: Leverage,
    pub demand: Demand,
    pub since: Tick, pub until: Tick, pub next_due: Tick,
    pub paid: u16,
    pub status: CoercionStatus,                     // Active, Complied, Refused, Broken, Lapsed, Exposed
}
```

### The move

`leverage::coerce(world, agent, over, leverage, demand) -> CoercionOutcome` is M15's `social::resolve` with `MoveKind::Intimidate` and two additions to the logit: `+ lev_bias[leverage]` and `− demand_cost`, where `demand_cost = demand_w × value(demand) ÷ max(purse(over) + 7 × wage(over), 1)` (a ransom a family cannot pay is refused). A Secret adds `secret_w × deed_sev × (1 − reach)`; a Hostage adds `hostage_w × rel_w(over, captive)` (M15's relation weights). Success makes the demand happen (coins move with `Flow::Coerced`; `Do` sets the coerced taker; `Sell` transfers at `value`, `Acquired` event; `Join` is M15's poach; `Vote` pins the member's vote; `Silence` marks the report `withdrawn`, so it no longer counts toward the warrant; `Access` sets `DoorOpen`). Failure is M15's backlash, plus: the over files a report (`Crime::Extortion` for coins and threats, `Crime::Blackmail` for a secret) with `p = backlash_report × lawfulness`, and a holder of a weekly coercion that is refused loses it (`Refused`). `Coerced`, `CoercionBroken` events; the deeds `Extorted` and, new, `Blackmailed` (`reach0 = 0.2`, `deed_sev = 0.4`, `honour_w = −0.4`) go to the pools at the usual reach. (revised 2026-10-08: M15's resolver is `moves::resolve` in `citysim/src/systems/moves.rs`; `coerce` builds a `SocialMove { actor: agent, target: over, kind: MoveKind::Intimidate, stake }` (`Stake::Coins` for coin demands, `Stake::Job` for `Join`, `Stake::Info` otherwise) and calls `moves::resolve_with(world, &m, extra)` with the leverage and demand terms as `extra`, which `resolve_with` adds to the actor's skill term; if the phase-1 coder finds the terms must enter the logit after the skill weighting, it adds a `moves::resolve_logit` beside `resolve_with` and leaves `resolve`'s numbers unchanged. The outcome type is M15's `MoveOutcome { success, p, refused, backlash }`; the draw is the move's own `WordNs::Move` stream. A failure's report is `law::file_report(world, Crime::Extortion | Crime::Blackmail, agent, Some(over))`; `Crime::Blackmail` and `Crime::Fraud` are appended and ordered by `Crime::severity()`. Coins move through `ownership::pay` with `Flow::Coerced`.)

| Move | Who uses it, when |
| --- | --- |
| **Shakedown** (the M8 Home extortion) | `gang::extort` calls `coerce(Threat, Coins { extort_amount, weekly: false })` against the strongest occupant; the claim counting runs on success as today. The M15 Intimidate roll is this roll. (revised 2026-10-08: at HEAD `gang::extort(world, actor, home)` makes M15 W27's Intimidate through `moves::resolve` against the strongest occupant; routed through `coerce` with no extra term for a non-weekly demand up to twice `extort_amount`, it is the same roll and the same `p`, so gang income holds; L2's Statistical GangWork day calls `gang::extort` with no occupant present and is untouched.) |
| **Protection** | a gang under Expand or Contest, once a week per held district, targets the agent- or corp-owned Bar, Market, Hotel, Clinic or Fixer with the highest revenue in it: `Coins { protection_weekly, weekly: true }`. (revised 2026-10-08: the candidates include L2's Club, Arcade, NoodleBar, FightPit, Den and Lounge, never a front of the same gang (`Venue.front_of`); a rival gang's front is a candidate, and refusing protection there is a Contest by other means. L2 left protection on others' businesses to this row.) Paid weekly from the owner's purse until refused, the gang loses the district, or the agent is jailed. A corp owner pays only when its Security cover of that building is 0. |
| **Threat** | a coerced job: a gang leader or exec who cannot afford a contract's price makes it `Terms::Coerced` on a weaker regular; a suspect with an open report threatens its witness with `Silence` when `lawfulness < silence_lawfulness`. |
| **Blackmail** | any adult with `lawfulness < blackmail_lawfulness` holding a secret on someone richer (`standing_t > standing_a + 0.2`) makes it once a month: `Coins { blackmail_weekly, weekly: true }`, or `Vote` and `Sell` from a board campaign or Acquire. A secret is a deed with `deed_sev ≥ 0.4` or `honour_w < 0` held first-hand or at `hops ≤ 1` whose holder count (M15's daily pass tallies the keys referenced by active secrets) is ≤ `secret_known_max`. It **lapses** (`Lapsed`, no backlash) when any pool entry of that deed reaches `secret_public_reach`: the secret is out. |
| **Poaching by threat** | M15's poaching step (revised 2026-10-08: `competence::poach_daily`/`competence::try_poach`), for a corp with exec lawfulness < `poach_threat_lawfulness`, uses `Threat` or a Secret with `Join` instead of the wage premium. `Poached` deed as M15. |
| **Debt** | a lender (M15 `Edge.debt`) whose debtor has missed `debt_days` calls the debt as `Coins { amount: debt }`; failure is backlash, success settles the edge. (revised 2026-10-08: `Edge.debt: i32` and `Edge.debt_since` exist.) |
| **Parley** (added 2026-10-08; L2 § 7 deferred the gang leader's Parley here) | once a week, a gang leader whose gang is in an open vendetta (`World::vendettas`) or under Contest against a rival whose strength ratio exceeds its own by `parley_ratio` makes a Persuade move (`moves::resolve_with`, `Stake::Coins(parley_tribute)`) on the rival leader at the rival's Hideout door, or at any tier from the daily pass when either leader is Statistical. Success: a `Coercion { holder: rival gang, over: leader, leverage: Threat, demand: Coins { parley_tribute, weekly: true } }` made already `Complied` (tribute for peace, paid weekly with `Flow::Coerced` until refused), the vendetta closed (its `declared` cleared, both weights × 0.5) and `Regard.value` +0.2 both ways; `Parleyed` event. Failure: M15's backlash and `grudges::declare(world, a, b, w)` on the pair for `[grudges] declared_days`. The leader's other tool, delegated violence, is M16a's Hit posted under Raid or Retaliate. |

### Fraud

- **Reneging** (§ 2) is fraud. The street does not go to the law, it hunts: a Reneged taker files `Crime::Fraud` (label "Fraud", severity just above Theft, `sentence_hours.fraud` 48) against the placing agent only when the taker's lawfulness ≥ 0.5, and otherwise keeps the grudge. (revised 2026-10-08: M16a ends a renege at the `Betrayed` deed and grudge; M16b appends this filing.)
- **Fabricated stories.** A Spin contract with `fabricated = true` (posted when the buyer's exec or leader has `lawfulness < fabricate_lawfulness` and no true rival misdeed scores, price × `fabricate_mult`) plants a deed that never happened. The Feed's reporters contest it with a Deceive move by the Fixer or exec against their mean knowledge; a Feed that sees through it refuses and the attempt is a `Betrayed` deed against the buyer. A planted fabrication carries `Story.fabricated` (hidden); each day each holder of it whose own first-hand memories contradict it (they were the object, or a witness of the object elsewhere that day) exposes it with `expose_p`. **Exposed**: `Exposed` event, a `Betrayed` deed against the buyer at `reach0 = 1.0`, `Crime::Fraud` on the placing agent, and the Feed's reach × 0.8 for 14 days. (revised 2026-10-08: the M15 story is `word::Story { id, feed, deed, actor, source, object, tick, slant, paid_by }`; `fabricated: bool` is appended with `#[serde(default)]`. The Spin that posts it is M15 W40's side spend (`Corp.spin_since`, `news::spin_considerations`), not a `CorpOrder`; the Deceive is `moves::resolve` with `MoveKind::Deceive`; the exposure roll is the Contract stream's purpose 21.)
- **The fake job.** A regular with deception ≥ 0.6 and lawfulness < 0.3 may sell a non-existent contract to another regular for `fake_fee` (a Deceive move; success takes the fee, `Flow::Coerced`; the victim learns it on the deadline: `Betrayed` deed, grudge `Betrayed`).

### Hostages and the private prison

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Captive { pub holder: Option<EntityId>, pub held_at: EntityId, pub since: Tick, pub coercion: Option<CoercionId>, pub until: Tick }
```

`Captive` on the held agent (Full or Coarse, never demoted while held) pins `ServeTime` at the holding building as `Sentence` does, counts against its capacity, and costs the holder `hostage_food` coins a day (`Flow::Upkeep`). Holding buildings: a gang's Hideout, a squat, and the new **`BuildingKind::Prison`** (label "Private Prison", letter `Q` (revised 2026-10-08: **`Z`**; `Q` is M14's Lab; the Prison is the 25th kind, after L2's seven and M16a's Fixer), `[buildings] prison = { capacity = 20, stock_cap = 100, staff = 4 }`, Security niche, `found_cost.prison` 800, `value` 1200, upkeep 20, staffed by `Role::Guard` with the corp's Security Office as employer, never foundable by an agent). Taking a hostage is an `Abduct` (M13: `resolve_fight` with abductors summed, a carry, `Crime::Abduction`, `Abducted` event) by the holder's members or by an Abduct contract (revised 2026-10-08: `chrome::abduct(world, agent, victim)`, the drag through `Brain.cuffed_by`/`abducted_by`/`escorting`; the hostage plan ends at `HandOver` instead of the Harvest's Rip; a captive ranks in class 4 inside `[lod] captive_quota` 6, and a taking that would pass the quota is not made); on `HandOver` the `Captive` is added and a `Coercion { leverage: Hostage, over }` is made at once, `over` chosen as the captive's employer's exec, gang leader, or the living kin with the largest purse, in that order, with the holder's demand. The coercion is re-rolled daily until `until = since + hostage_days`; on `Complied` the captive is released at the door (`HostageFreed`); on expiry the holder releases or kills with `p_hostage_kill × (1 − holder lawfulness)` (`HostageKilled`, a Murder by the holder's agent).

**Who takes hostages.** A gang holding Retaliate or Job may take the rival leader's Spouse or child instead of raiding when `ratio < hire_ratio`, demand `Release` of its convicts (the captain complies when `Law.competence < 0.4` and lawfulness < 0.5) or `Coins`. A corp under Acquire whose seller refused twice, exec lawfulness < `dirty_lobby`, with a Prison or a Hideout-holding gang under contract, abducts the seller exec's kin, demand `Sell`. Corps build a Prison under Secure when holding a hostage with nowhere to put it and cash ≥ 0.8.

**Rescue.** The `over` of a hostage coercion, or the captive's gang, posts `Rescue` (or the gang raids directly: M8 Raid with the holding building as `raid_target`, as M12's corp raids). The mission's win frees every captive of that holder inside (`HostageFreed`, `Rescued` deed with the crew as actors, honour +), and the holder's members inside are beaten per `fight_out`. A captive whose `until` passes during a live Rescue's muster waits for it.

**Abduction for the factory.** `Deliver { Captive → building }` to an M13 Hideout under Harvest is the scav economy's input today; the goods milestone's Soylent factory reads the same contract kind and needs no change here. (revised 2026-10-08: at HEAD the Harvest drags its victim to the gang's Hideout and rips it there (`p_rip_kill`); off screen `chrome::abduct_offscreen` kills the victim into an `Abducted` hole, which L2 folds into `fviolence::daily`'s `Order(Harvest) × Abducted` cell. Nothing here changes either.)

```toml
[leverage]
max_coercions = 128
lev_bias = { threat = 0.0, secret = 0.3, hostage = 0.5, debt = 0.2 }
demand_w = 1.0
secret_w = 1.0
hostage_w = 1.5
backlash_report = 0.5
protection_weekly = 40
blackmail_weekly = 30
blackmail_lawfulness = 0.35
secret_known_max = 5
secret_public_reach = 0.3
silence_lawfulness = 0.3
poach_threat_lawfulness = 0.3
debt_days = 14
fabricate_lawfulness = 0.35
fabricate_mult = 2.0
expose_p = 0.05
fake_fee = 40
parley_ratio = 0.3                 # (added 2026-10-08) Parley
parley_tribute = 30
patrol_share = 0.3                 # (added 2026-10-08) the Purist Patrol (§ 3)
[hostage]
hostage_days = 7
hostage_food = 3
p_hostage_kill = 0.3
[lod]                              # additions (2026-10-08, M16b; L2's budget)
captive_quota = 6
wounded_quota = 12
[buildings]                        # addition
prison = { capacity = 20, stock_cap = 100, staff = 4 }
[corps]                            # additions
found_cost = { prison = 800 }
value = { prison = 1200 }
upkeep = { prison = 20 }
[law.sentence_hours]               # additions
blackmail = 72
fraud = 48
```

## 5. The wounded, Rescue, Trauma Team

(revised 2026-10-08: all of § 5 is **M16b**.)

### Wounded

`law::resolve_fight` keeps its formula and M13's chrome terms. When the death roll hits, a second draw makes it a wound with `p = wound_share × (1 − K.armour_loser)` instead; a robot is never wounded. The wounded agent gets (revised 2026-10-08: every fight at HEAD goes through `law::resolve_fight_mods(world, a, b, FightMods { kill_mult, a_bonus, kill_p })` (`resolve_fight` and `resolve_fight_with` wrap it); the wound draw is a branch there, on the Contract stream (purpose 17, keyed by the loser and the tick), drawn only when `wound_share > 0`, so with it 0 the function is byte-identical. `K.armour_loser` is `Kit.armour`. When the wounded class is at `[lod] wounded_quota`, the would-be wound stays a death (`wounds_capped`), so deaths still only fall.)

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wounded { pub by: Option<EntityId>, pub since: Tick, pub bleed_until: Tick, pub tile: TilePos, pub stable: bool }
```

pins `ExecState::Down` (no action, cannot be robbed of anything but its Wallet, carried as a corpse is carried: M13's `exec::follow_bearer`), is at least Coarse while down, and writes a `Sighting` of itself at its tile to its gang's and employer's `FactionDb` and, if it holds a Trauma subscription, to the Security corp's (conf 1.0, the beacon). At `bleed_until` an unstabilised wounded dies by `World::kill_by(…, Violence, by)`: the report becomes a Murder, the Murder count is unchanged from the M15 rule, and `BledOut` fires. `Stabilise` (new `ActionKind`, dur 30, at a Clinic with a Ripperdoc inside, `stabilise_price` paid by the carrier's faction, the wounded or the subscription; `Flow::Treatment`) sets `stable`, and the agent rises after `recover_days` with energy and safety at 0.2 (`Recovered` event). The witness, report and memory rules are the M12 Assault rules at the wound and the Murder rules at a bleed-out; a wound is never reported twice. (revised 2026-10-08: `ExecState::Down` is appended to `exec::ExecState` (whose last variants at HEAD are M13's `Fly` and M14's `JackedIn`); the carry reuses the private corpse-carry `exec::follow_bearer` and M9's `law::follow_guard` drag, generalised once; the beacon writes a `virt::Sighting` into `World::db` for the gang, the employer corp and the Trauma corp; a Down agent ranks in class 4 inside `wounded_quota`.)

### Rescue

`Temperament { Abandon, Loyal, Recover }` on `Gang` (`#[serde(default)]` Loyal; The Unplugged and any gang whose leader's loyalty < 0.3 seed Abandon); corps and the law are `Recover`. **Rescue** (Full and Coarse goal): `Can(a Down agent within rescue_radius who is a Spouse, Family, Friend, fellow member, colleague, or a guard's arrestee, unstable ∧ a Clinic reachable before bleed_until)` → GATE; `rel_w` or faction term (Recover 1.0, Loyal `loyalty`, Abandon 0) → Linear{0.8,0.2}; `courage` → Linear{0.4,0.6}; `1 − danger` (an attacker within 4 tiles) → Linear{0.5,0.5}. Plan `GoTo(Wounded) → CarryWounded → GoTo(Clinic) → Stabilise`. A gang member with `loyalty ≥ rescue_loyalty` rescues under Loyal. City guards carry their own and their arrestees. `Rescued` event and deed (honour +0.3 via `honour_w.rescued`). (revised 2026-10-08: The Unplugged exist at HEAD, seeded by `creeds::seed_unplugged`; `GoalKind::Rescue` follows M16a's `Contract` in `GOAL_ORDER` (29th after L2's two and M16a's one), a scripted plan through the `plan::plan_for` bypass.)

### Trauma Team

`ContractKind::Trauma` is a standing contract (`Standing`) between a client (buyer, the person) and a Security corp (taker), renewed weekly at `fee_weekly × price_level[Security]` (`Flow::Trauma`, taxed as owner revenue); unpaid, it lapses. Subscribers: every corp exec and board member (paid by the corp), and once a day a Full or Coarse adult (a Statistical one in the daily pass) with coins ≥ `subscribe_coins` and `heat ≥ subscribe_heat`, or an open hunt or contract against them, subscribes with `p_subscribe`. Clients choose the Security corp minimising `price_level × (1.5 − honour)` (M15's rule for guard contracts). **The call**: on a client's wound, the beacon sighting goes to the corp's `FactionDb`; the corp sends `trauma_crew` on-shift guards from its nearest Security Office as a mission whose door is the **last sighting** of the client (not the true tile: a client carried off by an abductor or a rescuer is met where the db says), plan `GoTo(Door) → CarryWounded → GoTo(Clinic) → Stabilise`, defenders fought through `fight_out` if the attacker is still there. A client stabilised before `bleed_until` is a `TraumaSave`; the corp pays `stabilise_price` (no extra fee). Off screen a subscriber's StatTable `p_killed` is multiplied by `1 − trauma_stat_save`; nothing else changes the table. (revised 2026-10-08: "M15's rule for guard contracts" is `corps::honour_seller`, which minimises `corps::contract_price × (1.5 − honour)` with honour from `reputation::rep`; Trauma reuses it with `fee_weekly × price_level` as the price. The subscription roll is the Contract stream's purpose 20. Trauma crews are M16a missions and ride inside `contract_quota`.)

```toml
[wounded]
wound_share = 0.6
bleed_hours = 3
stabilise_price = 80
recover_days = 3
rescue_radius = 12
rescue_loyalty = 0.5
[trauma]
fee_weekly = 35
trauma_crew = 2
subscribe_coins = 400
subscribe_heat = 0.3
p_subscribe = 0.2
trauma_stat_save = 0.5
[reputation]                       # additions (M15 tables)
honour_w = { rescued = 0.3, hired = -0.2, blackmailed = -0.4 }
heat_w = { hired = 0.4 }
dread_w = { hired = 0.3 }
```

## 6. Governance and the vote

(revised 2026-10-08: all of § 6 is **M16b**. At HEAD `Governance { Dictator, Board { members } }` sits on `Corp.governance` and `Gang.governance`; every faction is `Dictator`, only a save test builds a `Board`, and nothing reads it. 16b grows the `Board` variant with `#[serde(default)]` fields as below, and `corp_brain`'s inputs read the board's mean Personality for a board corp. The vote's preference reads `reputation::rep(…).standing` and `.honour`; affinity is `Edge.affinity`. Corp Personality for a Dictator is unchanged.)

`Governance::Board { members }` (the M11 D49 hook) becomes `Board { members: Vec<EntityId>, chair: EntityId, last_vote: Tick, votes: SmallVec<[(EntityId, EntityId); 8]> }`. `[governance] boards` names the seed corps with boards (Habitat and Nutrix): the exec as chair plus the `board_size − 1` employees with the highest standing; a dead or departed member is replaced by the next. A board's brain reads the **mean Personality** of its members, weighted 2 for the chair, instead of the exec's.

**The vote** runs every `vote_days` and at once on the chair's death. Candidates are members with `greed × standing ≥ ambition_min` plus the chair. Each member votes for the candidate maximising `pref = 0.4 × affinity + 0.3 × standing + 0.3 × (honour − 0.5) × 2` (M15) unless a `Vote` coercion pins them. Ties go to the chair. A new chair becomes `Corp.exec`; `VoteHeld` (with the tally) and, per pinned member whose `pref` choice differs, `VoteSwung`.

**The campaign.** `campaign_days` before a vote, the strongest challenger whose projected tally loses by ≤ 2 targets the swing members (closest `pref` margin) one a day: a `Persuade` move with `Stake::Coins(vote_bribe)` when lawfulness ≥ 0.5, else a Blackmail or Threat coercion with `Demand::Vote`, or posts a `Vote` contract for a regular with persuasion or intimidation. A rival corp under Acquire may back a challenger the same way. M18's player is one more challenger.

```toml
[governance]
boards = ["Habitat", "Nutrix"]
board_size = 5
vote_days = 45
campaign_days = 7
ambition_min = 0.4
vote_bribe = 150
```

## 7. The Statistical tier and LOD

(revised 2026-10-08: the last column is the part; the quotas for the forced classes are the *LOD budget* table in "M16a and M16b", under L2's rule that every forced class has a quota inside `max_coarse` and held agents cost no body.)

| Data and behaviour | Full | Coarse | Statistical | Part |
| --- | --- | --- | --- | --- |
| Posting (Hunt hires, Court, Bury) | event | event | the M15 daily pass (`hunt::stat_pass` for Hunt hires) | a (Hunt), b (Court, Bury) |
| Regular at a Fixer | `Network` | `Network` | the daily pass, one hash-picked day in seven | a |
| Taking a contract | the Contract goal | same | accepted by score at matching; a live contract promotes to Coarse (inside `contract_quota`) | a |
| Executing | exec | exec | the ledger draw at `due`, pre-bound holes (L2's `Hole.source`/`Hole.faction`) | a |
| Coercions | the move at the encounter | same | resolved at any tier (no body needed); hostages are always Coarse (inside `captive_quota`) | b |
| Wounded, Rescue | exec | exec | the StatTable `p_killed`, re-calibrated; Trauma multiplier | b |
| Tags, bounties | hourly sighting writes | same | tags write regardless of tier (a tag is the target's eyes on them, not theirs) | a (Locate), b (tags) |
| Board votes | any tier | any tier | any tier | b |

The StatTable is unchanged in shape; `calibrate` is re-run for `Network`, `Stabilise`, `CarryWounded`, `PlantTag`, `Scan` and the Wounded split of `p_killed`. (revised 2026-10-08: L2 keeps `calibrate` on its gangless calibration city with L2 off and re-runs the table only if a parity test fails; M16a follows that rule for `Network` and `Guard` (none of its Full actions changes the 500-agent city), and the one deliberate re-calibration is M16b's, for the Wounded split of `p_killed`, in its phase 3. L2's `test_faction_violence_parity` must still pass after each part.)

## 8. Player levers and god commands

| Command | Effect | Part |
| --- | --- | --- |
| `SetFixerLicence(bool)` | Off: Fixers are illegal; every open office has `heat` floored at 0.5 and new ones cannot `Register`. | a |
| `SetAccessoryMult(f32)` | The Conspiracy sentence multiplier. | a |
| `PublicBounties(bool)` | The law posts `Locate` on every wanted suspect after one day instead of `law_bounty_days`. | a |
| `SetPrisonPermit(bool)` | Off: corps cannot build Prisons; existing ones hold no new captives. | b |

CLI: `--lever day:SetFixerLicence:false`. **God commands**: `PostContract { buyer, kind, target, price, brokered, deadline_days }` (logged; `Origin::God`); `TakeContract { contract, taker }` (forces acceptance; M18's `PlayerCommand::TakeContract` is the same call); `Hostage { captive, holder, at, demand }`; `Wound { agent, by }` (the Wounded state at once, bleed timer running); `Subscribe { agent, corp }`; `TagAgent { agent, owner, tier }`; `Coerce { agent, over, leverage, demand }` (forced success); `CallVote(corp)`; `ExposeSecret(coercion)` (posts the deed at reach 1.0). (revised 2026-10-08: the CLI grammar at HEAD is `--lever day=D:name=value` (M12 D42), so `day=10:fixer_licence=off`. M16a: `PostContract`, `TakeContract` and the three a-levers; M16b: `SetPrisonPermit`, `Hostage`, `Wound`, `Subscribe`, `TagAgent`, `Coerce`, `CallVote`, `ExposeSecret`. Each is a `PlayerCommand` (the life-path addendum's rule), and M18's player reaches the same calls.)

## 9. UI, events, CSV

New `EventKind`s (amber): `ContractPosted`, `ContractTaken`, `ContractFulfilled`, `ContractFailed`, `ContractExpired`, `Reneged`, `SoldOut`, `StrikeDeclined`, `BountyPaid`, `Tagged`, `TagFound`, `Accessory`, `FixerBusted`, `Coerced`, `CoercionBroken`, `Lapsed`, `Exposed`, `HostageTaken`, `HostageFreed`, `HostageKilled`, `Wounded`, `Rescued`, `BledOut`, `Recovered`, `TraumaCall`, `TraumaSave`, `VoteHeld`, `VoteSwung`. New `Deed`s `Hired`, `Blackmailed`, `Rescued`; `GrudgeCause::Hired`; `GoalKind`s `Contract`, `Rescue`; `ActionKind`s `Network`, `HandOver`, `Guard`, `PlantTag`, `Scan`, `Coerce`, `CarryWounded`, `Stabilise`; `Order::Job`; `Shock::Trespass`, `CorpShock::Trespass`; `Crime`s `Conspiracy`, `Blackmail`, `Fraud`; `BuildingKind`s `Fixer`, `Prison`; `Role::Fixer`; `AssetKind`s `Tag`, `Scanner`; `ExecState::Down`; `Flow`s `Escrow`, `Payout`, `FixerCut`, `Coerced`, `Trauma`. (revised 2026-10-08: **M16a**: `ContractPosted`, `ContractTaken`, `ContractFulfilled`, `ContractFailed`, `ContractExpired`, `Reneged`, `SoldOut`, `StrikeDeclined`, `BountyPaid`, `Accessory`, `FixerBusted`, `GuardTaken`; the deed `Hired`; `GrudgeCause::Hired`; `GoalKind::Contract`; `ActionKind`s `Network`, `Guard`; `Order::Job`; `Shock::Trespass`, `CorpShock::Trespass`; `Crime::Conspiracy`; `BuildingKind::Fixer`; `Role::Fixer`; `RunWhy::Fixer`; `ViolenceSource::Contract`; `faction::Payer::Owner`; `WordNs::Contract`; `Flow`s `Escrow`, `Payout`, `FixerCut`. **M16b**: every other name in this paragraph, plus `Parleyed`, `PatrolScan` and `RunWhy::Contract`. Amber is L2's `AMBER` constant `#E0A030` in `citysim-app/src/ui/log.rs`.)

- **Board panel** (new, `J`; revised 2026-10-08: `C`, since `J` is M15's overlay at HEAD and L2 takes `H`; M16a): open, taken and standing contracts with kind, buyer (or "anonymous" unless the viewer is in `known_by`), target, price, broker, deadline, taker, render tag (LEDGER, LIVE, and the M14 stream); filters by viewer (god, a selected agent's `visible` set: the M18 quest log). History from `contract_log`.
- **Mission panel** (M16a): the crew, the door, the strike decision's terms (`p_win`, controller, political cost), the live brawl through M14's `MissionView` (revised 2026-10-08: the app's `citysim-app/src/mission.rs` and `ui/mission.rs`, fed by `Gang.stream_by`; a mission gets the same field).
- **Inspector** (M16a contracts; M16b the rest): an agent's contracts on both sides, coercions held and suffered, Captive and Wounded blocks, Trauma subscription, tags on them (god only).
- **Building panel** (M16a Fixer; M16b Prison): a Fixer's book, regulars, cut, heat, income; a Prison's captives.
- **Corp panel** (M16b): board, chair, last tally, campaign.
- **City panel** (M16a section and levers; M16b's counts appended): a Contracts section (open by kind, fulfilled today, hits, hostages held, wounded down, Fixers and heat) and the levers.
- **Map overlay** (`J`; revised 2026-10-08: `C`; M16a Fixers and mission doors, M16b Down agents and captives): a glyph on each Fixer sized by book, a crosshair on each live mission door, a pulsing cross on each Down agent, a lock on held captives.
- **CSV** (`--report`): `contracts_open`, `contracts_posted`, `contracts_fulfilled`, `contracts_failed`, `contracts_expired`, `reneged`, per kind `k_{kind}_posted` and `k_{kind}_done`, `hits_done`, `hits_squad`, `strikes_declined_pol`, `sold_out`, `contract_murders`, `contract_cleared`, `accessory`, `bounties_paid`, `tags_live`, `tags_found`, `coercions_active`, `protection_paid`, `blackmail_active`, `secrets_lapsed`, `fabricated`, `exposed`, `hostages_held`, `hostages_freed`, `hostages_killed`, `wounded`, `bled_out`, `rescued`, `trauma_subs`, `trauma_saves`, `votes`, `votes_swung`, per Fixer `f{i}_heat`, `f{i}_income`, ledger columns `flow_escrow`, `flow_payout`, `flow_fixer_cut`, `flow_coerced`, `flow_trauma`. (revised 2026-10-08: the columns go before `ticks_per_sec`, after L2's `LivingCols` and `BudgetCols` (HEAD's header ends `…,law_competence,flow_ads,flow_plant,ticks_per_sec`), in two groups: **M16a's** `contracts_*`, `reneged`, the per-kind columns for its four kinds, `hits_done`, `hits_squad`, `strikes_declined_pol`, `sold_out`, `contract_murders`, `contract_cleared`, `accessory`, `bounties_paid`, `guards_on_take`, `fixer_runs`, `live_parties`, the per-Fixer columns (four slots), `flow_escrow`, `flow_payout`, `flow_fixer_cut`; then **M16b's** per-kind columns for its ten kinds and the rest of the list, plus `parleys`, `patrols`, `wounds_capped`, `flow_coerced`, `flow_trauma`. `citysim-cli/tests/cli.rs`'s header test moves in each part.)

## 10. Save compatibility

Every new field is `#[serde(default)]`: `Hole.contract`, `Gang.temperament`, the board fields of `Governance::Board`, `Building.broker`; `World::contracts`, `coercions`, `missions`, `contract_log`, `next_contract`, `next_coercion` default empty. `Wounded`, `Captive` and `Broker` are new component stores. Every enum variant above is appended (`Order::Job` tenth, `Crime`s ordered by `severity()` as today, not by declaration). A pre-M16 save gets two seeded Fixers on the first day when Lots exist, no contracts, no boards until the first midnight (built from `[governance]`), and Loyal for every gang. Escrow is counted by `ownership::total_coins`, so the conservation test holds across save and load mid-contract. `Config::v1_profile` sets `[contracts] enabled = false`, which turns off posting, Fixers, missions, coercions beyond the shakedown roll (extortion as M15 left it), the Wounded split (`wound_share = 0`), Trauma and votes, so the v1, M8 and M9 tests run unchanged. (revised 2026-10-08: there is no `Hole.contract`: M16a uses L2's `Hole.source`/`Hole.faction` with `ViolenceSource::Contract` appended. `save::SAVE_VERSION` is 1 at HEAD and L2 takes 2; **M16a sets 3** (a version-2 save gets its Fixers seeded on the first midnight when Lots or derelicts exist, as L2 defers its venues) and **M16b sets 4** (a version-3 save gets no boards until the first midnight, Loyal gangs, no coercions); each migration step runs in `save::from_ron` after L2's `jobs::migrate`. M16a's sections (`[contracts]`, `[fixers]`, `[missions]`, `[bounty]`'s Locate keys) and M16b's (`[leverage]`, `[hostage]`, `[wounded]`, `[trauma]`, `[governance]`, `[bounty]`'s tag keys) each deserialize `off()` from an older config. `Config::v1_profile` and `calibration_city` turn both parts off; the CLI gains `--contracts-off` (both parts) and `--leverage-off` (16b's sections only), the identity devices of the two gates. Escrow inside `total_coins` keeps L2's identity across save and load mid-contract.)

## 11. Testing and calibration

**Unit tests** (`citysim/tests/contracts.rs`, `missions.rs`, `leverage.rs`, `wounded.rs`, `governance.rs`): a brokered post moves the price to escrow and `total_coins` is unchanged; a fulfilled contract pays taker and Fixer at the cut; an expired one refunds; a direct contract with `renege` set posts `Betrayed` and a grudge; `visible` hides a brokered job from a non-regular and shows a direct one to the buyer's Friend; matching never offers a Hit to the target's kin; the Hunt hires when `might_gap < hire_gap`; a ledger Hit opens a pre-bound hole that binds to the taker with the same witness on save, load and re-bind; a mission's `fight_out` matches a gang raid's result for the same lists and seed; a strike in a rival-held district computes the political term and pushes `Trespass`; a gang sells out a non-member and refuses a member; a Locate pays at most once per `min_gap_hours`; a tag writes hourly and a scanner of higher tier removes it; interrogation names the buyer and the next `accessory_check` files Conspiracy; a buyer nobody has heard of is never charged; the Fixer closes at `warrant_heat`; `coerce` is order-independent; a ransom above the purse is refused; protection pays weekly and stops when the district is lost; a secret lapses when its pool reach passes the bound; a fabricated story is exposed by its object; a hostage is fed by the holder and freed by a Rescue win; a wound bleeds out at `bleed_hours` and is a Murder, a stabilised one recovers; deaths with `wound_share > 0` never exceed deaths with it 0 for the same seed; a Trauma beacon routes the crew to the last sighting, not the true tile; a vote under a pinned member records `VoteSwung`; a pre-M16 save loads. (revised 2026-10-08: M16a owns `contracts.rs` and `missions.rs` and the tests on posting, escrow, settlement, renege, visibility, matching, Hunt hiring, the pre-bound hole (now on `Hole.source`/`Hole.faction`), the mission's `fight_out` parity with a gang raid, the political term, selling out, the Locate gap, interrogation, Conspiracy, the unfounded buyer, the Fixer's closing, plus Fixer-fed run orders, guard corruption, quotas and a version-2 save; M16b owns `leverage.rs`, `wounded.rs`, `governance.rs` and the rest, plus Parley, the Patrol and a version-3 save.)

**Acceptance, revised 2026-10-09 (docs/TESTING.md; supersedes the scenario gate below and the bands, majority, six-seed and pinned-run devices in the Goals bullets).** No milestone scenario gate: `tests/scenario.rs` is gone. The Goals bullets map onto the tiers: (1) every mechanism that must exist is a `core_sanity` bullet (`citysim/tests/core.rs`, `mechanisms()`: fired on at least one of seeds 42-44 in 120 days), or, when it fires on 2 or fewer of seeds 42-47, a unit test in a seeded world; (2) the sanity bounds (coin identity, population, starvation, assaults a day, Treasury, corps alive, held prisoners fed) are `core_sanity`'s and `core_year`'s and need nothing new; (3) every bullet about what a person does becomes a person probe (`citysim/tests/person.rs`: one pinned agent, one stimulus) or a `shadow --assert` bound on an archetype (`citysim-cli/src/shadow.rs`, a new archetype where the milestone adds one, its bounds measured with a margin); (4) each shock question is one god scenario per reaction class, 60 days, asserting that the world reacted. Calibration bands, Murders against a pinned run and per-seed trajectories are printed by `tools/analyze_run.py`, never asserted. For M16a the `core_sanity` bullets are a contract fulfilled (each of the four kinds where it fires on 3 or more seeds), a Fixer in business and a `BountyPaid`; the person probes a taker who refuses a Hit on kin and a buyer who is never charged unheard; a `fixer` archetype in the behaviour tier.

*Superseded:* **Scenario** (`citysim/tests/scenario.rs`, `#[ignore]` `test_m16_contracts_seed_42`): the bullets under *Goals and acceptance*, in particular ≥ 4 fulfilled kinds, a Fixer in business, an attributed hit with the buyer as accessory, a hostage and a rescue, a Trauma save, a bounty paid, a board vote swung, and Murders within 25 % of M15. **God scenarios** (`tests/god.rs`): `PostContract Hit` on each corp exec on day 10 at 2,000 coins (who takes it, does the squad strike in the Spire or sell out, does the corp's Lobby turn on the buyer); `PostContract Hit` on The Hollow's leader from Arasaka (does Ninefold, holding the district, sell him out); `Hostage` of the captain's Spouse by Ninefold demanding `Release` (does the law comply, Crush, or post a Rescue); `SetFixerLicence false` (do Fixers bribe, close, or move to the Sump); `Subscribe` every Corp-class adult and run M13's `ChromeEveryone 3` (do Trauma crews keep up); `CallVote Habitat` after `Coerce` on two members (does the chair fall, does the brain's order change with the board's Personality). (revised 2026-10-08: two gates, `test_m16a_contracts_seed_42` and `test_m16b_leverage_seed_42`, each over seeds 42-44 and 42-47 with the bullets of its part above. God scenarios: **M16a** the exec Hits, the Arasaka Hit in Ninefold's district and `SetFixerLicence false`, plus `PostContract Locate` on a gang leader by the law (does the bounty find him, does his gang Retaliate); **M16b** the captain's Spouse as a hostage, `Subscribe` with `ChromeEveryone 3`, `CallVote Habitat`, plus a Parley between the two strongest gangs in a vendetta. They go into `GOD_SCENARIOS_V7.md` (16a) and `V8` (16b), with a gaps list each.)

**Calibration**, tuned via `[contracts]`, `[fixers]`, `[missions]`, `[bounty]`, `[leverage]`, `[hostage]`, `[wounded]`, `[trauma]` and `[governance]` only: (revised 2026-10-08: under the gate doctrine every band below is a printed `// FINDING (calibration, not asserted)` line with a wide sanity assert beside it, split between the two gates as *M16a goals* and *M16b goals* say; no M8-M15 or L2 knob is tuned to reach one.)

| Target | Band |
| --- | --- |
| Contracts posted per 120 days | 60–300 |
| Fulfilled share of closed contracts | 35–70 % |
| Hits fulfilled per 120 days | 4–20 |
| Contract killings as a share of Murders | 5–25 % |
| Clearance on contract killings | 20–60 % |
| Fixer weekly income ÷ a Bar owner's | 0.5–2.0 |
| Active coercions on day 120 | 10–60 |
| Hostages held at once, max | 1–6 |
| Wounded ÷ (wounded + fight deaths) | 20–50 % |
| Wounded saved (rescued or Trauma) | 30–70 % |
| Trauma subscribers on day 120 | 2–8 % of adults |
| Board votes with a swung member | ≥ 1 in 120 days, ≤ 50 % of votes |

**Throughput.** Nothing runs per agent per tick. Matching is daily over ≤ 256 contracts × the Fixers' regulars (≤ 2k pairs); visibility is computed on demand for the Board panel and the Contract goal's gate; missions run on the raid machinery for ≤ 12 promoted crews; ledger contracts are one draw at `due`; coercions are ≤ 128 rolled weekly or daily; tags write ≤ 32 sightings an hour; the accessory check reads open Murder reports and the guards' memories (≤ 40 × 24) daily; secret holder counts piggyback on M15's daily reputation pass; votes are O(board²) every 45 days. The new Coarse loads (crews, hostages, the wounded, ≤ 60 at once) fit inside `max_coarse`. (revised 2026-10-08: the Coarse loads are now L2 quotas: 28 bodies at most in 16a and 46 with 16b, inside `max_coarse`, ranked above gang members; throughput is printed against the 4,000 floor and judged only by A/B.)

## 12. Phases

(revised 2026-10-08: superseded by *M16a phases* and *M16b phases* in "M16a and M16b"; the five phases below are the single-milestone plan, kept for the record.)

1. **The board and money**: § 1 and § 2 (`Contract`, posting, escrow and the flows with conservation, the Fixer building, regulars, visibility, daily matching, the Contract goal for Hit, Beat, Steal, Deliver and Locate on existing chains, settlement, renege), Hunt hiring, the ledger resolution with pre-bound holes, `PostContract` and `TakeContract`, the Board panel and CSV. A 10-day CSV matches M15 outside the new columns. Commit.
2. **Missions and the law**: § 3 (the `Expedition` factoring of `raid.rs` with the gang path byte-identical, squads, the strike decision, political cost, `Trespass`, selling out and `Order::Job`, tags and scanners), the `Hired` deed, interrogation, Conspiracy, Fixer heat and bribery, and the brain postings of § 1. Commit. The first `Accessory` on seed 42 is the first visible M16.
3. **Leverage**: § 4 (`Coercion`, `coerce`, shakedowns through it with gang income within 10 % of M15, protection, threats and silence, blackmail with lapsing, poaching by threat, fraud and fabricated Spin, hostages, `Captive`, the Prison, Rescue missions), § 6 boards and votes. Commit.
4. **The wounded**: § 5 (`Wounded`, `Stabilise`, `Temperament` and Rescue, Trauma subscriptions and calls), the StatTable re-calibration, § 8 levers and god commands, § 9 panels and overlay. Commit. Watch it live: fight deaths must not rise.
5. **Gate and polish**: the M16 scenario and god scenarios, calibration, README and docs, then `/code-review high <base>..HEAD` and a fix commit, then push.

## 13. Risks

- **The murder market runs away.** Every M15 grudge too weak to hunt is now a buyer, every hit makes kin, and kin can buy too: price, `hire_gap` and `gun_lawfulness.hit` are the brakes, with the 25 % Murder bound as the gate. Calibrate on contract killings' share of Murders before touching the revenge rules M15 tuned. (revised 2026-10-08: M16a; the bound is now ≤ 1.25 × the L2-closing run, and L2's off-screen faction killings are already inside that baseline.)
- **The raid refactor moves calibrated gang behaviour.** `raid.rs` drives M8–M12's raids, breaches and riots. Phase 2 lands the `Expedition` factoring alone first, with a byte-identical 120-day gang CSV, before missions call it. (revised 2026-10-08: M16a phase 2's first commit, on the `raid::Expedition` enum; the riot arm must stay byte-identical too.)
- **Off-screen consistency and the Wounded split.** A ledger contract must give the same answer however it is inspected, and the wound rule touches every fight. Pre-bound holes keep the binder's guarantees; `wound_share` only ever replaces deaths, and the StatTable is re-calibrated once, in phase 4, against a Full run. (revised 2026-10-08: the ledger half is M16a's, the Wounded split M16b's phase 3.)
- **The LOD budget** (added 2026-10-08). M16's forced classes rank above gang members inside `max_coarse`; if `contract_quota` sits full, gang bodies at Contest frontiers fall to Statistical and L2's ledger measures less on-screen gang violence. Watch L2's `gang_bodies` and the order-rate cells in M16a phase 4; shrink `contract_quota` before touching L2's quotas.
- **The stream namespace is full** (added 2026-10-08). `WordNs` holds 15 namespaces in bits 52-55; after L2 and `WordNs::Contract` none is left, so M17 and later share `Contract` with a purpose byte or widen the field, which moves every word stream and needs its own identity check.

## 14. Out of scope (M17 and later)

Contracts across the city boundary, a parent's `Retake` posted as contracts, outside combat maps and their transient casts (M17); the player taking and posting jobs in person, haggling over price as a dialogue render, the quest log beyond the Board panel's viewer filter, story LOD for contract parties (M18); daemons as contractors without a body (the candidate; `Contract.taker` is an `EntityId` so a daemon entity can hold one later), the Blackwall; Power and Water as goods, the Soylent factory as a building (it reads `Deliver { Captive }`), map layers, interiors and room-level prisons; courts and trials, insurance, outsourcing the law's convicts to private prisons, bounty hunters as a profession beyond Locate, and a contract market in Data beyond Steal and Deliver. (revised 2026-10-08: and, from L2's side, leisure economics beyond protection on its venues; M16a out of scope: everything assigned to M16b above; M16b out of scope: a crime for corruption and bribery, a guard-corruption market beyond the direct `Guard` contract, Parley between corps.)

## Addendum (2026-10-05, Dylan): attention and distraction

Spec amendment; decisions tabled as overturnable and to be read by the M16 plan and its coders, with hooks in M14 (detection rolls) and M9/M12 (the law's sightings and responses). (revised 2026-10-08: **M16b** builds this addendum, in its phase 4. At HEAD the hook already exists twice as a stub returning 1.0: `faction::alertness_mult(world, Option<EntityId>)` (read by M12's corp-raid response through `raid::posted_guards`, the riot response's `answering_guards` and `street.rs`'s sweeps) and `virt::alertness_mult` (read by M14's node contests and camera identification). 16b fills the first and points the second at it. M16a's strike defenders and Guard contractors call the stub, so 16a's numbers do not move when 16b fills it except through attention itself.)

**The idea.** Factions and NPCs have only so much attention. A faction fighting a physical war, a riot or a large run may not notice one lone runner stealing something quietly, or one sneaky infiltrator while a tank is outside blowing things up. If you can manipulate a group's attention and threat assessment you can be ignored long enough to get in and out. Sparking a war between the target and another faction is a tactic.

**Attention as a faction resource.** `Faction.attention: f32` (0..1) is a daily pool sized by the faction's management competence (M15) and headcount, consumed by active **threats** in order of assessed severity: a raid or breach in progress, a riot in a district it controls, a loud run against its nodes, a strike, a hostile acquisition, a vendetta, a sanction. Each threat holds a share for its duration plus `attention_linger_days`. What remains is the faction's **alertness**, and every detection and response roll the faction makes scales by it: guard sightings and witness rolls (M9, M12 stances), sweeps and ICE contests on its nodes (M14), camera identification (M14), riot and raid response strength (M12), the Security corp's contract response. Below `alert_floor` (0.2) the faction is **distracted**: quiet runs and infiltrations against it roll at the floor, and the law's reports from its guards slow.

**Threat assessment is manipulable.** The faction ranks threats by what it *knows* (M15: rumours, sightings, the trace), not by what is true. So: a war started through a contract or propaganda (M15 `Spin`, M16 `Hit` on a rival's member framed through a hop chain), a loud decoy run (M14 addendum) that is real but pointless, a riot sparked in its district (M12), a strike bought (M16 coercion of a union-shaped group), a sanction (M17) all consume its attention. The brains read attention too: a distracted faction's own orders tilt to `Hunker`/`Garrison`/`LieLow`.

| Decision | Call |
| --- | --- |
| Where it lives | `attention` on `Gang`, `Corp`, `Law` and `OutsideFaction`; recomputed daily and on each threat event; saved. |
| Pool | `pool = clamp(0.4 + 0.4 × competence + 0.2 × min(headcount ÷ 20, 1), 0.3, 1.0)`; threat shares from a table (raid 0.5, riot 0.4, loud run 0.3, strike 0.2, acquisition 0.15, vendetta 0.15, sanction 0.2), summed and clamped; `alertness = max(pool − Σ shares, alert_floor)`; `attention_linger_days` 2. |
| Detection hook | One multiplier `alertness_mult(faction)` read by: `law::sightings` and the witness roll for the law; M14 sweeps, ICE contests, camera id; M12 riot/raid response; M16 hit-squad and bodyguard reaction; M13 robot sensors. Default 1.0 when the field is absent (old saves). |
| Brains | `OrderInputs.attention`; a consideration `1 − alertness` on Hunker/Garrison/LieLow (Linear{0.5, 0.5}); the law's posture brain reads it on Garrison. |
| Player and NPC tactics | No new action: a distraction is any existing threat the actor can cause; the Quest view (M18) labels a contract's side effect "distracts X" when it would consume X's attention. |
| Bound | Attention is O(factions) daily; no per-agent cost. |

## Implemented: deviations (M16a)

Where the build departs from the text above. The numbered rows are the plan's Decisions table (`~/.claude/plans/m16a-contracts.md`, C1-C44; where the plan and this spec disagree the plan wins); the rest are the phase commits' recorded deviations (phase 1 1c228d6, phase 2 fa336a1 + 7649e3f, phase 3 ac9e9db, phase 4 2ebaea3) and phase 5's acceptance. One line each. Every contract, hit, beating, bounty and charge here is an abstract event between fictional agents: a record with a price on a struct, a utility score, one seeded dice roll or ledger draw.

### Decisions that changed the spec

- **C1:** the types live in `citysim/src/contract.rs` (types only, saved); `MissionId = ContractId`; 16a's variants only (C44).
- **C2:** `World::{contracts, missions, contract_runs, contract_log (cap 512), next_contract}` plus the derived indices (`by_party`, `ledger_due`, `locate_targets`, `on_take`, `contract_guards`, `mission_of`) rebuilt on load; ids never reused; closed records kept `closed_keep_days` (a fulfilled Hit or Beat `max(closed_keep_days, accessory_days)`).
- **C3:** built as `[contracts] enabled` with `contracts_off()` in `v1_profile`, `calibration_city` and `living_off`, and `contracts::on` also requiring `[living] enabled`; **retired 2026-10-09** with the off switches (docs/TESTING.md): contracts are always on, `v1_profile` and `calibration_city` included.
- **C4:** a fresh stream key, `rng::CONTRACT_KEY` on bits 63|62 with `ContractNs` in bits 56-59 (`Renege, Due, Outcome, TakerDeath, Talk`), not `WordNs`; `rng::stream_id_*` factored with a disjointness test.
- **C5:** escrow is a purse inside `ownership::total_coins` through one writer pair (`escrow_in`/`escrow_out`); `flow_escrow` is the day's net into escrow; probes `escrow_leak` and (review fix) `escrow_stuck`, both 0 on every seed; `escrow_leak == 0` is in `core_sanity`'s sanity list (phase 5).
- **C6:** direct records purse-check and pay at fulfilment; the renege draw on `ContractNs::Renege`; `Betrayed` is posted in the taker's Home district; a buyer short at settlement is reneged.
- **C7:** the quote as written (the worked 711 asserted); price 0 = quoted, days 0 = the kind's default; a Building target's standing is its owner's (0.3 for the city).
- **C8:** `BuildingKind::Fixer` (`X`) and `Role::Fixer` as written; NPC founding through `founding::choose_kind_with` + `fixer_ok` (phase 4: `fixer_licence` gates it).
- **C9:** **one** seeded Fixer (Mid East): two filled the per-capita cap and took Sump Central's only vacant Lot; `refit_with` for the fallback; no seed coins; the owner rule as written (a gang leader is excluded on day 0 only: on seed 42 the owner leads Ninefold by day 45).
- **C10:** `Network` scripted, not in `PLANNABLE`; Mode B's "1 − U(wealth)" read as `urgency(needs.wealth)`; Statistical regulars stamped one hash-picked day in seven.
- **C11:** visibility on demand, never cached, as written.
- **C12:** matching daily at `match_hour`, deterministic; phase 1 agents only, phase 2 gangs and squads, phase 3 a Security corp first (`corp_take`); guards and the target's kin never eligible.
- **C13:** `GoalKind::Contract` through `plan_for`'s scripted bypass; phase 2: a mission's crew scores it only inside `raid_gather_hours` before the muster, at urgency 1 (the deadline urgency never mustered a crew); a Hold cools the crew until 3 h before the new muster.
- **C14:** the Hunt's chase factored (`hunt::{Chase, chase, set_chase}`, `chased_by` rebuilt in `reindex`), identity-checked; contract runs live in `World::contract_runs`.
- **C15:** a Hit strike won without a death returns to asking with the record Taken; the Locate contact pays and re-asks; the Guard post cooled `24 − guard_hours`; phase 2: Hold at StakeOut re-asks and cools a day.
- **C16:** `set_status` the one writer; Cancelled logs `ContractFailed`; reneged Hits count in `hits_done` and `contract_murders`; no `law::sentence → on_jailed` hook (a jailed or cuffed taker fails through the per-tick validity pass and the ledger's free check); `World::remove_agent` calls `on_death(None)`.
- **C17:** `max_missions` live runs, the rest queued by price; phase 4: admission enforces the quota (a queued record holds no bodies), `start_queued` passes over a record that cannot start; a run's target ranks class 4 from the run's start.
- **C18:** the ledger draw at a hash-picked `due` as written; a ledger Hit settles before `stat_hit`; phase 2: the strike decision first, a Hold sets `due + 1 day`.
- **C19:** `ViolenceSource::Contract` appended; a pre-bound hole binds a living actor not jailed that day, else Unknown; the Unknown and candidate draws are discarded so the witness offset matches (tested); `contract_hole_wrong` 0.
- **C20:** `try_hire` in `hunt::adopt` and a daily `hire_pass`; phase 1 measured 1-8 holders a day failing only the coins test at a ~700 quote (no Hunt hire posted in 120 days on 42-44).
- **C21:** the Fixer's talk in a `gossip::exchange` wrapper at `Venue::Drink`; the listener's copy written as the exchange writes one; no `Hired` for a Guard.
- **C22:** `Expedition::Mission` landed alone (fa336a1, byte-identical 30 and 120 days); a mission still standing at `raid_at + RAID_MARCH_TICKS` fails the attempt; the brawl line goes to `contract_log` and the settlement text (no new `EventKind`).
- **C23:** a building door adds its posted guards and robots as defenders; the brawl waits for an absent target until the window's last hour, then fails.
- **C24:** `StrikeDeclined` on every Fail (`strikes_declined_pol` counts the political ones: a strike called off on the fight's expected value logs "cost 0.00"); `S_def` includes Guard takers over the target.
- **C25:** squads for Hit and Beat; the crew from the taker's gang, else Friends who are regulars of the Fixer; squads only at matching; `hits_squad` counts fulfilled squad records (phase 2's message said people; the code counts records, asserted in phase 5).
- **C26:** gangs offered Hit and Beat only; one acceptance rule (the sell-out leader rule, price not part of it); `OrderInputs.job = (id, price ÷ hoard_heat, cover)`, the Job row scored only with a job; `gang_eligible` excludes the buyer's and target's gangs, a gang mustering a raid or under a strike pin, and a leader tied to the target; a short sell-out top-up returns only its own take.
- **C27:** the law posts one public Locate a day (the most severe open crime, then the longest unseen) only while the Treasury covers every open public bounty's cap; phase 4: `public_bounties` shortens the delay to one day.
- **C28:** interrogation from `law::jail_suspect` as written (`moves::resolve_with` Intimidate, bonus `0.3 × competence + interrogate_bias`).
- **C29:** the Conspiracy sentence implemented in phase 1 (`sentence_days` would index out of range); reneged Hits and Beats count as done; the placing agent must not be cuffed; the law never charges its own records (buyer `None` or `Origin::Law`); memory only, never `known_by`.
- **C30:** no bribe while `hardened_until`; a bust only on an open office; `Payer::Owner` pays from the owner's purse (an owner under the bribe price offers nothing, silently).
- **C31:** only a corrupt city guard takes a corruption record, posted below `corrupt_lawfulness` with `Origin::Law`; the take's `Hired` only to the placer; an arrest by the guard on the take fails the record; agents only (the Fixer-office branch of `is_corruption` is unused in 16a).
- **C32:** a Security corp accepts a building Guard with room and price ≥ `contract_price` and converts it to an M11 guard contract.
- **C33:** gang Hits against rival gangs only; `Secure` posts a Guard instead of `buy_contract` only for loss buildings; the culprit Locate at the Fixer nearest the first lost building; `Lobby` falls back to the bribe with no open Fixer; vendetta Locates target the other side's leader or exec; the death squad needs gang heat > 0 and the Treasury's cover.
- **C34:** a Fixer's run order on the scorer's best Data target (`virt::best_data_target`).
- **C35:** `BudgetSets.{contract, fixer}` as written (Fixer owners class 2 from `match_hour − 2` to midnight, live parties class 4, whole records by price ≤ `contract_quota` 24); `Contract.started` stamps a real start, so a Guard never started expires at its deadline (buyer refunded, taker unpaid).
- **C36:** the tick slot as written (`…, living, contracts, demography, stats`); no `on_jailed` hook (C16).
- **C37:** the enums appended as listed; `EventKind` 12 amber kinds.
- **C38:** `ContractCols` as written plus `escrow_stuck` and (phase 3) `contracts_refused_full`; read by `tools/analyze_run.py`'s M16a section.
- **C39:** the levers and god commands as written; `SetAccessoryMult` clamps to 0..5 and refuses a non-finite value; the CLI's gang grammar for `take_contract` in phase 4.
- **C40:** `SAVE_VERSION` 3 in phase 1; superseded: the legacy migrations retired 2026-10-09 (only the current format loads, 6 at this writing).
- **C41:** the `--contracts-off` identity devices ran in phases 1-2 (byte-identical CSVs and events, `calibrate` identical); **retired 2026-10-09** with the switch (docs/TESTING.md: a phase diffs a default 30-day run against its base).
- **C42:** `C` the overlay (its own file, `contract_overlay.rs`), Shift+C the Board as a floating window; the M14 stream for a mission deferred; app flags `--overlay contracts`, `--board`, `--select-mission`.
- **C43:** as written; ticks printed, judged by A/B only.
- **C44:** held: no 16b type; `faction::alertness_mult` and `virt::alertness_mult` stay stubs.

### Phase deviations and findings

- **Phase 1:** the gun gate's cheapest test first and no per-think allocations (behaviour-identical); `World::median_wage` saved; an M13 gate device moved to existence over 42-53 (since retired with `scenario.rs`).
- **Phase 2:** `live_job_on` covers the crew; `lod.rs` touched for the crew's class; the shipped city posted no contract, so the phase was byte-identical on 42-44 and the squad path is unit-tested end to end.
- **Phase 3:** brain postings made contracts post naturally (273-334 a seed in 120 days, mostly Locates and Guards; ~700-820 Treasury-paid bounties); the behaviour tier re-measured on pick changes.
- **Phase 4:** `charity.rs::test_endowed_mission_serves_over_days` re-pins its guests broke and hungry daily; the default city moves (the Fixer owner holds a class-2 body evenings; on seed 42 a fourth gang forms on day 26).
- **Phase 5, acceptance (the TESTING.md pattern; the old gate `test_m16a_contracts_seed_42` and its majority, six-seed, Murders-ratio and identity devices superseded):** measured on seeds 42-47 × 120 days (ba516f2). `core_sanity` bullets for what fired on 6 of 6: a Guard fulfilled [11, 4, 12, 16, 11, 8], a Locate fulfilled [167, 166, 155, 145, 137, 133], a Fixer in business (open, a record brokered there within 7 days; days [111, 107, 112, 106, 111, 109]), a `Reneged` [22, 6, 12, 23, 6, 12], with phase 3's `BountyPaid` and city guard on the take. Unit tests in a seeded world for what fired on 2 or fewer: a Hit and a Beat fulfilled (0 of 6), a squad Hit (0; `missions.rs::test_squad_when_solo_estimate_below_squad_below` now settles it), a `SoldOut` and a `StrikeDeclined` on political cost (0; `test_gang_sells_out_non_member_refuses_member`), the Hit later attributed (0; `contracts.rs::test_interrogation_names_buyer_then_conspiracy_filed` now asserts the day), an NPC-founded Fixer (0; new `test_npc_registers_a_fixer`), a Fixer run to `SellData` (2 of 6; `test_fixer_run_order_runs_to_selldata_with_cut`). Person probes: a gun never takes a Hit on his wife (with a stranger control he takes), a buyer nobody heard of is never charged before somebody holds the deed, a broke hand with a sellable skill networks at the Fixer within a week; one gap probe `#[ignore]`d (a buyer tells his own Hit to guards). Shadow archetypes `fixer` and `gun` (bounds after the behaviour tier's rework). God scenarios v8 in `docs/GOD_SCENARIOS_V8.md`. **Finding: no natural Hit or Beat is fulfilled on any of 42-47**: the murder market the spec's goals describe exists only in god posts and unit tests.
