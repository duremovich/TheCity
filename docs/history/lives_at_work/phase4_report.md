# Phase 4 report: the street and freight

## Status at the session close (2026-10-18)

**WIP sha `2c22096`** on `worktree-agent-aeb9f5f4cf9b9e4ee`, on top of main `1f37f78`; the tree compiles (lib, cli, tests) and is clean. To be squashed into the one phase commit. The worktree's `target/` is kept; main's reference binary is `scratchpad/p4/main_src/target/release/citysim-cli.exe` (built from `git archive 1f37f78`, run from `main_src`).

**Built and compiling (block A of the brief's order, plus the scaffolding):**
- The switch: `[service] street = true` with **`CITYSIM_LIVES_STREET_OFF=1`** (deviation: the brief's `CITYSIM_STREET_OFF` is Jobs & lives phase 2's `[jobs2] street` switch, `config.rs` "the street's A/B and identity device"; under it main hires no street trades, so the identity read 1,467 fewer events from day 0). `ServiceCfg` keys `cab_wait_ticks 30, client_tiles 12, pimp_tiles 12, pimp_pass_up 0.5, pimp_absent_days 3, p_short_report 0.5, witness_lookout_mult 0.5` (skip at default), `ServiceCfg::strip_street`, `Config::strip_street` (drops the `[[serves]]` rows flagged `street = true`, the keys, `[stims] deal_corner`), `ServesCfg.street`, `StimsCfg.deal_corner` (toml `true`), `Config::unresolved_names` accepts the gang-post keys `pimp` and `dealer`. Nine `[[serves]]` rows in `assets/config.toml` (cab_driver/fare, courier/parcel, loader/crate, street_worker/client, pimp/staff, dealer/buyer, vendor/buyer, rickshaw/fare, tinker/buyer).
- State: `Jobs2State.cab_rides: Vec<CabRide>`, `cab_rolled: BTreeSet<EntityId>`, `parcels: Vec<Parcel>` (all skip-empty, cleared at the top of `streetwork::daily`); `EventKind::FareShort` (ALL 220); 13 `Jobs2Cols` columns (`cab_rides_driven, cab_rides_unmanned, fares_short, parcels_delivered, parcels_returned, lifts, pilfer_reports, street_clients_near, pimp_collections, pimp_pass_up, corner_deals, dealer_regulars, lookout_halved`; cli header `+ 13`); SAVE_VERSION 38. Deviation: `cab_rides_unmanned` replaces the brief's `cab_waits_walked` (a rider never waits: with no driver's body within `cab_wait_ticks` the Garage's off-screen cab takes the ride, the leg timed).
- `streetwork.rs`: `Ride`/`RideBy`, `cab_roll` (the pure roll and pick, `stat_cab`'s logic lifted out unchanged), `pay_ride` (the coins and counters), `stat_cab = first hour + cab_roll + pay_ride`; `full_cab` (the Full/Coarse commuter's roll at the commute GoTo, once a day a rider via `cab_rolled`; the fare paid or dodged `Short` by `transit::pays_or_dodges` with a `FareShort` event and a `Robbed` grudge driver → rider; the rider's leg `GotoTimed` at the Car's `timed_mult`; a bodied on-duty driver within `cab_wait_ticks` by the lowest `hash(PURPOSE_FARE, rider, driver)` takes a `CabRide`; the `Fare` encounter at the pick-up via `service::serve(driver, rider, garage, Fare, 0)` or `service::encounter(puller, rider, None, district, Fare, 0)` for a rickshaw); `drivers_free`, `cab_ticks`, `ride_of`, `on_fare`, `fare_tile`, `fare_leg` (the driver's leg timed at cab speed), `fare_plan` (one leg a plan: stage 0 to the rider's door, 1 to the work door, read off the driver's position like `round_plan`; the M13 crash roll skipped: no vehicle asset for the cab, deviation).
- Hooks: `exec/mod.rs::start_step` (before the vehicle/El/walk: `fare_leg` then `full_cab`), `plan.rs` Work arm (`fare_plan` after `round_plan`), `leisure::spot_of` (a driver's leg tile), `service::encounter(server, served, place: Option, district, kind, coins)` with `serve` its placed wrapper (event text names the district with no place).
- `tests/street.rs` (3 tests): `test_full_commuter_rolls_the_same_cab_as_a_statistical_one` passes; `test_off_city_writes_nothing` passes; **`test_full_commuters_cab_is_timed_and_a_driver_drives_it` fails at "the fare paid"**: `full_cab` returned a timed leg, `pays_or_dodges` reads `Some(false)` (no dodge), yet the rider's coins stayed 500 → 500 — `pay_ride` did not move coins, or the ride's `by` branch paid nobody (read the printed `Jobs2Cols` tail: `cab_rides_driven/unmanned/rickshaw_trips`; the `eprintln!` is still in the test). Diagnose this first; the two other tests prove the roll and the identity.
- Off identity checked on seed 42 × 30 d against main's binary: events byte-identical, 977 shared CSV columns 0 differing cells (with the renamed switch; `p4/base` and `p4/off`). Not yet 43-44, not yet the saves.

**Half-built:** nothing beyond the failing test above; `Parcel` is a type only (`streetwork::Parcel`, saved on `Jobs2State.parcels`, never pushed).

**Not started (the brief's order):** B the parcel (`parcels()` to push `Parcel`s with a hashed sender and a recipient staff member; `parcel_stops` to return the recipients' buildings under `street_on`; the courier's stop in `trades::round_plan`'s arrival branch as the super's; the return at `service::shift_end`); C the lift (`service::lift(loader, yard, n)` at the loader's shift end from `yard_units`, the owner's `p_short_report` Theft report after `industry::pilfer`'s hit); D the client (`vice::street_hour`'s body-first pick with `world.agents_near`, the `Client` encounter, `street_assault` taking the client as actor); E the pimp (presence within `pimp_tiles`, the Statistical credit on `following_order`, the `Collect`-style round in `gang_work_target`, `street_cut` to the pimp with `pimp_pass_up`, `pimp_seen` and `name_pimps` dropping the absent); F the dealer (`stims::deal_corner`, `World.corner_dealers`, the `Buyer` encounter in `buy_stims`/`stat_buy`, the regular bought first, the lookout's `witness_mult` in `law::raise_crime_except`); G vendor/rickshaw/tinker joining the regulars (`stat_vendor_meal`, the Full vendor's `Vend` completion, `tinker_repair` → `encounter`; the rickshaw's is in `full_cab` already). Also: the cabbie probe and the pimp probe in `tests/person.rs`, the five printed bullets in `tests/core.rs`, the two `tests/lod.rs` rows, the measurements (42-47 × 120, lives probe rows, diaries, A/B), § 13/§ 14 and ARCHITECTURE § 6/§ 7, the commit message.

**Next three steps for whoever resumes:** (1) `git log --oneline -1` = `2c22096`; run `cargo test --release -p citysim --test street test_full_commuters_cab -- --nocapture`, read the printed counters and fix the fare payment (then remove the `eprintln!`); (2) build B and C (small: both are one hook each into `trades::round_plan`'s arrival branch and `service::shift_end`), run the off identity on 42-44 × 30 with saves against `p4/main_src`; (3) D-G in order, or hand back A-C with D-G named in § 14 if time is short (the brief allows it).


- Worktree `D:\TheCity\.claude\worktrees\agent-aeb9f5f4cf9b9e4ee`, branch `worktree-agent-aeb9f5f4cf9b9e4ee`, ff-merged to main `1f37f78` (confirmed). One squashed commit on top of `1f37f78`; never merged.
- Main's binary for the base runs, the off identity and the A/B: `git archive 1f37f78` into `scratchpad/p4/main_src` (its own target), run from that root.
- Final sha: FINAL_SHA

## Footers (final tree)

```
CHANGE_FOOTER
LINT_FOOTER
```

## The switch: `CITYSIM_LIVES_STREET_OFF=1` (deviation from the brief's name)

`CITYSIM_STREET_OFF` already exists: Jobs & lives phase 2's switch for `[jobs2] street` (`config.rs`, "the street's A/B and identity device"), the pitches and rounds themselves. Set, it drops every street trade (no cab drivers, couriers, vendors hired), so a phase 4 off run under it compared against main's shipped city read 1,467 fewer events from day 0 (the hires). The phase's switch is `[service] street` with **`CITYSIM_LIVES_STREET_OFF=1`**; `Config::strip_street` drops the street `[[serves]]` rows (flagged `street = true`), the `[service]` street keys and `[stims] deal_corner` whenever the switch is off by any path, as phase 3's `strip_civic`. The merge set's identity run must use the new name.

## Built

BUILT_P4

## Off identity (CITYSIM_LIVES_STREET_OFF=1 against main's binary, 42-44 x 30)

IDENTITY_P4

## Numbers against the base

NUMBERS_P4

## Lives probe

LIVES_P4

## Diaries

DIARIES_P4

## Test changes

TESTS_P4

## Left unbuilt (§ 14 names it)

LEFT_P4

## Open items

OPEN_P4
