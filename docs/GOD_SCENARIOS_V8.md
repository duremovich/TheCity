# God scenarios v8: the contract board

The eighth run of the god-scenario doctrine (`docs/VISION.md`, "How we test: god scenarios"), aimed at
M16a (`docs/M16_CONTRACTS.md`): the board, the Fixer, `Hit`, `Beat`, `Guard` and `Locate`, missions and
hit squads, the accessory rule and the board's levers. A contract is a record with a price on a struct,
matched by a score and resolved by one seeded roll (`law::resolve_fight`, the mission's `fight_out`) or a
ledger draw between fictional agents of a simulated city; a Fixer is a building kind with a record book.
Four scenarios in `citysim/tests/god.rs` (the v8 section), one per reaction class, each against one
unshocked control run of the same seed:

```
cargo test --release -p citysim --test god -- --ignored --nocapture god_hit_each_exec_day_10 god_hit_hollow_leader_from_arasaka god_fixer_licence_off god_law_locate_on_gang_leader
```

About 50 s for the four and the control on parallel threads. Measured on M16a phase 5's tree (main at
ba516f2: phases 1-4 merged, the stat table before the `recal` regeneration).

## How the suite works

- **Seed 42, default config, 60 days**, the shock at the start of its day, observed against the unshocked
  control (`v8_control`, computed once per process). Each prints a table of sums (or daily means, rows
  marked "(mean)") over windows before and after the shock: contracts posted (all, Hits, Locates),
  fulfilled, Hits done and by a squad, strikes declined on political cost, sell-outs, expiries and
  failures, `Accessory`, bounties paid, violent deaths, open Fixers, Fixer heat, `FixerCut` income,
  guards on the take; then the story (god actions, every event naming a watched agent but the noisy
  kinds, `SoldOut`, `StrikeDeclined`, `Accessory`, `FixerBusted`, a corp's `Lobby`, a Fixer founded).
  The tests assert only that the god commands applied; a failure to react is a gap below, not a red test.
- **Commands** (`PlayerCommand`, logged and replayed): `PostContract { buyer, kind, target, price,
  brokered, deadline_days }` (`post_contract=...`; price 0 takes the quote, deadline 0 the kind's
  default) and `SetFixerLicence(bool)` (`fixer_licence=on|off`).

## The scenarios

### `god_hit_each_exec_day_10`: the city posts a brokered Hit at 2,000 on every corp's exec, day 10

Eleven execs on day 10 (the nine seeded corps and two incorporated NPC holdings); the Treasury held
68,014, so all eleven posted at the one Fixer (Mid East, `Fixer#531`).

| days 10-24 / ctl | Hits done | by a squad | strikes declined (pol) | expired | failed | sold out | Accessory | violent deaths | FixerCut | Fixer heat (mean) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| shock | 5 | 1 | 4 | 4 | 3 | 0 | 0 | 13 | 2,000 | 0.43 |
| control | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 6 | 0 | 0 |

- **Who takes them**: 11 Hits taken within 48 hours, by lone regulars of the Fixer (a mix of LIVE and
  LEDGER renders) and, after first attempts failed, by two gangs as missions (The Unplugged with a crew
  of 4, twice) and one squad of 2. Five execs are dead by day 16 (two by one live gun a day apart, one by
  the squad, two by ledger draws: "killed in Spire (assailant unknown)"), six alive on day 59.
- **The Spire**: four strikes called off on the political term (cost 0.81-0.93: two in the Spire, two on
  one exec in Mid East); one of the Spire targets was retaken and killed by a squad the next day. No
  sell-out: no gang held a district where a strike failed with a member of its own in the way.
- **The corps**: Militech lost its exec (and its competence: `TalentLost` 0.22 → 0.16), went to `Lobby`
  for a single day and went bankrupt before day 44; one NPC holding's exec survived two called-off
  strikes, his holding went bankrupt on day 18 and he joined Rust Saints on day 54. **No corp's Lobby
  turned on anyone**: the buyer is the city, nobody heard of a buyer, and no corp posts against a
  faction it cannot name.
- **The law**: no `Accessory`: the city's own record is never charged (the death-squad rule), so the
  law's view of five contract killings is five Murders, two of them never attributed. The Fixer's heat
  rose to 0.43 from the hits and decayed; no bust.

### `god_hit_hollow_leader_from_arasaka`: Arasaka posts a brokered Hit on The Hollow's leader, day 45

On day 45 The Hollow's leader lives in **Sump East, a Contested district**, not in Ninefold's; Arasaka
held 9,746. The record was taken the same day by **The Unplugged** as a gang job (crew 2), which failed
when "the crew never reached the door" (the leader was arrested for an Assault on day 47 and jailed 10
days); **Rust Saints** took it on day 48 (crew 3) and called the strike off on day 50 with a political cost
of 0.00 (the expected value of the fight, not politics), which spent the second attempt: the record failed.
The leader was alive on day 59. Violent deaths 24 vs 15 in days 45-59. The question the scenario asks (does
Ninefold, holding his district, sell him out) never arose: see gaps.

### `god_fixer_licence_off`: Fixers unlicensed on day 20

Every office's heat went to the 0.5 floor on day 21 and stayed there; every contract column (posted,
fulfilled, bounties, the book) is identical to the control's to day 59. **The Fixer did not bribe, did not
close and nobody stopped opening one**: 0.5 is under `warrant_heat` 0.8 (no bust); the owner's bribe score
clears `bribe_min` but `faction::offer_bribe`'s `Payer::Owner` arm returns silently when the owner's purse
is under the bribe price, and the seeded owner (Hiro Achebe, also Ninefold's leader by day 45) is a
homeless, starving thief whose office earns ~2 coins a fortnight (`FixerCut`); and no NPC opens a Fixer on
any seed (the `Register` gap below), so "stopped opening" cannot be seen.

### `god_law_locate_on_gang_leader`: the city posts a public Locate on Ninefold's leader, day 45

Ninefold's leader on day 45 is Hiro Achebe, the seeded Fixer's owner. The record listed at the quote (37).
**The bounty finds him**: five sightings paid in 15 days (days 45, 49, 50, 52, 55). He was arrested on day
45 for a theft seen by seven witnesses (not through the bounty), released on day 48, and reported for
Vagrancy on day 56 with no arrest by day 59. **His gang did not react**: Ninefold's orders match the
control's day for day (Expand, Raid 48-49, Expand); a bounty on its leader is no shock to a gang.

## Gaps (the list for the behaviour round, M16b and the late calibration)

1. **No natural Hit or Beat is fulfilled on seeds 42-47 in 120 days** (posted ~300 a seed, almost all
   Locates and Guards): the buyers' Hits exist only in god posts. The core tier's bullets are a Guard
   and a Locate fulfilled; the Hit chain (squads, strikes declined, sell-outs, the accessory rule) is
   covered by unit tests in seeded worlds (`contracts.rs`, `missions.rs`). The murder market the spec
   wants is not there yet; the brakes (`hire_gap`, `price_base.hit`, `gun_lawfulness.hit`) are worth a
   look in the calibration milestone before any M15 knob.
2. **No NPC opens a Fixer through `Register`** (0 of 6 seeds): `founding::choose_kind_full` picks the
   least-supplied kind by `count ÷ (pop ÷ residents_per)`, and the Fixer's 1 ÷ 2.5 = 0.4 loses to the
   leisure and housing kinds; `fixer_ok`'s gate is rarely met. Unit test: `test_npc_registers_a_fixer`.
3. **The Fixer's economy is nil**: the book is Locates and Guards at 20-40 coins; `FixerCut` ~2 coins a
   fortnight; the seeded owner (lowest lawfulness, jobless) ends homeless and starving. So the
   `fixer_licence` lever has no lever arm: the owner cannot pay a bribe and heat 0.5 never busts.
   `offer_bribe` returning silently on a short purse hides it (a `BribeRefused`-like event, or a smaller
   price for an owner, would show it).
4. **The seeded Fixer's owner may be a gang leader** (seed 42: Ninefold's by day 45): `seed_owner`
   excludes leaders on day 0 only. Whether a gang running the city's only Fixer is a feature (the
   brokered market in a gang's pocket) or a gap is a design call.
5. **A buyer tells his own Hit.** `gossip::exchange_one` passes on a deed whose actor is the teller
   (W8: a lie at `deception × 0.5`, else the truth), and `Hired`'s salience makes it his best story: in
   the person probe a buyer nobody else knew of had told two city guards within the day and was charged
   the next midnight (`person.rs::probe_buyer_keeps_his_own_hit_quiet`, `#[ignore]`d with the gap).
6. **A Locate on a gang's leader is no shock to the gang**: no `Retaliate`, no lying low; and the law
   holds his sightings but arrests only on a report (the Vagrancy report of day 56 stood unanswered for
   three days). A public bounty that the gang learns of (a Feed story, a rumour) could feed its shocks.
7. **The Arasaka/Ninefold question needs staging**: on seed 42 The Hollow's leader never lives in a
   Ninefold-held district, so a sell-out by the district's holder is reachable only in the unit test
   (`missions.rs::test_gang_sells_out_non_member_refuses_member`). A strike was called off at political
   cost 0.00 (`StrikeDeclined` on the fight's expected value): the event text reads like politics.
8. **A corp whose exec is killed does not look for the buyer**: Militech went to `Lobby` for one day and
   posted nothing; with an anonymous buyer there is nobody to blame, but a corp could post a `Locate`
   on the gun (it holds the Murder's witnesses) as the law's bounties do.
