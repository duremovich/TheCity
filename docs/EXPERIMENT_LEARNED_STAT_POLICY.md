# Experiment: a learned Statistical policy (table vs MLP)

2026-10-05, on 787cab0 (M9–M11 complete). Question (`docs/VISION.md` › "Note: learned policies
vs explicit brains"): as M12–M13 add conditioning (district, class, wealth, chrome), would a small
neural network beat the Statistical tier's calibrated 24-row table, which grows combinatorially?

**Answer in one line: keep the table for now.** On the calibration data a 2-layer MLP beats it
by a lot: validation log-loss 0.84 vs 1.49 per agent-hour. Put in the sim with the same features,
though, it **fails parity by 2×**: thefts 4.86 vs Full 2.21, assaults 2.31 vs 1.05. The features that
carried its gain (mood and zone) don't mean the same thing for an off-screen agent as for a Full
one. With those two dropped, the MLP passes parity, but its crime heads are then no better than the
table and it costs about 12 % of the 2,000-city's throughput. Switch only when a new key both blows
up the table and is honestly simulated off screen (the trigger is in § 8).

## 1. Setup

- **Data**: `citysim-cli calibrate --rows-csv`. Seeds 1000–1005, 30 days, 500 agents, the
  `calibration_city` (gangless, rent-free, equal wallets, Full forced). Each process ran one seed
  (`--first-seed`). That took 4.3 s per seed. Each row is one Full agent-hour: the features at the
  hour's start, the dominant exec category (the table's outcome rule), whether the agent had a
  courtship candidate, and which of steal / flirt / robbed / assaulted / killed fired. It also
  records new non-co-worker edges (`met`), Chats begun and Chats with a housemate. These are the
  tallies `calibrate` already keeps, one row per agent-hour instead of summed per bucket.
- **Split**: train on seeds 1000–1004 (1.81 M rows), validate on seed 1005 (362,681 rows).
- **Rows**: 2,172,660 agent-hours in total.
- **Class balance** (outcome): eat 0.056, work 0.022, social 0.011, sleep 0.167, idle 0.745.
- **Rolls**, positives / rows:

  | Roll | Positives / rows | Rate | Note |
  |---|---|---|---|
  | steal | 2,169 / 2.17 M | 0.00100 | |
  | flirt | 2,987 / 410 k | 0.0073 | only hours with a candidate |
  | robbed | 1,918 | 0.00088 | |
  | assaulted | 998 | 0.00046 | |
  | killed | 47 | 0.00002 | |
  | meet | | 0.025 | target `min(met,2)/2` |
  | chat | | 0.017 | target `min(chats,1)` |
  | chat_home | | 0.756 | among the 37,006 chat hours |

  The clipped targets drop 16 k hours with `met > 2` and 1.6 k with `chats > 1`. The table keeps the
  raw counts, so the meet and chat heads run slightly low.
- **Coverage gaps the calibration city builds in**:
  - housed share is **1.000**, so no homeless agent is ever seen;
  - child share is 0;
  - employed share is 0.109;
  - the Vats zone holds 2.3 % of hours;
  - all 30 days fall in one season.

## 2. Feature set (`systems::stat_policy::features`, 17 inputs)

| # | Feature | Notes |
|---|---|---|
| 0–3 | phase one-hot (Morning, Work, Evening, Night) | the table's first key |
| 4 | lawfulness | table key, continuous here |
| 5 | hunger | table key, continuous here |
| 6 | wealth | coins ÷ local Market price, clamped to 20 meals, ÷ 20 |
| 7 | employed | has a `Job` |
| 8 | housed | `Household.home` is set (constant in the training data) |
| 9 | married | |
| 10 | age | years ÷ 100 (continuous instead of a band) |
| 11 | mood | `-1..=1` |
| 12–16 | zone one-hot | `Zone::ALL` order, from the agent's current tile |

Season and day-of-season are left out because a 30-day calibration city covers only one season.
To learn them, `calibrate` would need runs started in other seasons. The **roll** net does not see
hunger: the table pools its rolls over hunger for a reason (`calibrate`, M10). A Statistical agent
eats as soon as it is hungry, so rates conditioned on hunger don't transfer.

## 3. The four models

All losses are mean log-loss per agent-hour. "total" is the sum of the outcome cross-entropy and
each roll's BCE, with each roll scored only on its own mask.

| Model | Numbers | Train total | **Val total** | Val outcome | steal | flirt | robbed | assaulted | killed | meet | chat | chat_home |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| (a) 24-row table (refit on train) | 192 | 1.501 | **1.491** | 0.733 | 0.00855 | 0.0425 | 0.00784 | 0.00467 | 0.00020 | 0.1199 | 0.0855 | 0.489 |
| (b) coarse extended table | 207,360 | 0.998 | **1.029** | 0.543 | 0.00773 | 0.0383 | 0.00738 | 0.00451 | 0.00027 | 0.0990 | 0.0821 | 0.247 |
| (c) MLP 17→32→13 | 1,549 | 0.801 | **0.837** | 0.395 | 0.00730 | 0.0372 | 0.00688 | 0.00410 | 0.00020 | 0.0977 | 0.0822 | 0.206 |
| (d) MLP 17→16→13 | 781 | 0.819 | **0.851** | 0.408 | 0.00738 | 0.0378 | 0.00695 | 0.00414 | 0.00020 | 0.0982 | 0.0828 | 0.205 |
| (c′) MLP 32, no mood/zone ("safe") | 1,165 | 1.191 | **1.191** | 0.474 | 0.00830 | 0.0423 | 0.00768 | 0.00449 | 0.00020 | 0.1187 | 0.0850 | 0.451 |

- **(a)** follows the same rules as `calibrate`. The outcome is fitted per 24 rows. The rolls are
  pooled over hunger (12 cells). Flirt is conditioned on having a candidate, and chat_home on a Chat
  happening.
- **(b)** buckets everything:

  | Key | Buckets |
  |---|---|
  | phase | 4 |
  | lawfulness | 3 |
  | hunger | 2 |
  | wealth | 3 (<1, <5, ≥5 meals) |
  | employed | 2 |
  | housed | 2 |
  | married | 2 |
  | age | 3 (<18, <60, ≥60) |
  | mood | 3 (<-0.3, <0.3, ≥0.3) |
  | zone | 5 |

  That gives **25,920 outcome cells** (12,960 roll cells) × 13 numbers. Only **2,256 cells (8.7 %)**
  have any training hours. Each cell is shrunk toward (a) with 20 pseudo-counts, and an empty cell
  falls back to (a). Even so it trails the MLP and **overfits killed** (0.00027 vs 0.00020). This
  is the combinatorial explosion, measured.
- **(c) and (d)** are each two nets: an outcome net (all inputs → H → 5-way softmax over eat /
  work / social / sleep / idle) and a roll net (inputs minus hunger → H → 8 sigmoids). ReLU
  activation, Adam, batch 4,096, 12 epochs, best epoch kept by validation loss. Training takes
  40 s (H=32) and 29 s (H=16) in numpy. Halving the width costs 0.014 nats, so 16 hidden units is
  enough.
- **Where the gain comes from**:
  - **Outcome**: mostly continuous hunger. A Full agent doesn't eat once hunger is ≥ 0.7, which a
    two-bucket key can't express. `run_statistical` already hard-codes that gate, so in the sim
    much of this gain is redundant.
  - **Rolls**: mood and zone. Without them (c′) the roll losses are back at table level, e.g.
    steal 0.00830 vs 0.00855.

### Calibration (validation)

Each cell is the bucket-weighted mean |predicted − observed| over the 24 original buckets, divided by
the head's overall rate. Lower is better.

| Model | eat | work | social | sleep | steal | flirt | robbed | assaulted | killed | meet | chat | chat_home |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| (a) table | .053 | .176 | .033 | .053 | .270 | .192 | .187 | .332 | 1.51 | .081 | .038 | .029 |
| (b) ext table | .042 | .043 | .030 | .023 | .225 | .234 | .188 | .303 | 1.54 | .060 | .036 | .029 |
| (c) MLP 32 | .031 | .046 | .050 | .026 | .234 | .168 | .187 | .339 | 1.50 | .061 | .051 | .012 |
| (d) MLP 16 | .049 | .046 | .070 | .020 | .301 | .172 | .167 | .293 | 1.52 | .067 | .086 | .014 |
| (c′) safe | .034 | .082 | .057 | .043 | .283 | .279 | .201 | .284 | 1.53 | .082 | .036 | .019 |

Killed has 47 positives in all the data (about 8 in validation), so no model calibrates it.

The interesting buckets are the hungry ones. Observed / predicted steal rate on validation:

| Row | Hours | Observed | Table | MLP 32 |
|---|---|---|---|---|
| Morning law0 hunger0 | 243 | .0288 | .0012 | .0024 |
| Work law0 hunger0 | 654 | .0229 | .0016 | .0053 |
| Work law1 hunger0 | 1,566 | .0198 | .0016 | .0071 |
| Work law1 hunger1 | 80,238 | .0015 | .0016 | .0014 |

The table pools these buckets away by design. The MLP recovers part of the gap through the
correlates of hunger (wealth, mood), even though its roll net cannot see hunger itself.

## 4. Inference in the sim

The policy lives in `citysim/src/systems/stat_policy.rs`:

- `StatPolicy` trait with two impls, `TablePolicy` (the old `stat_row`) and `MlpPolicy`.
- `StatMlp` loads from `assets/stat_mlp.toml`. Feature standardisation is folded into `w1`/`b1`.
- `[lod] policy = "table" | "mlp"` (serde default `"table"`). The MLP is loaded only under
  `"mlp"`.
- `run_statistical` calls `stat_row` and nothing else changed. The same five-plus rolls are drawn
  in the same order from the agent's stream, so the policy changes probabilities, never the RNG
  stream.
- The two global numbers (`p_dole_day`, `p_theft_caught`) still come from the table.
- First-layer weights of inputs that were constant in training are zeroed at export, so `housed`
  carries no weight. Otherwise a homeless agent in the real city would have been read through
  untrained random weights.
- The Rust forward pass matches numpy to about 1e-6 on validation rows.

**Default is unchanged**: `run --days 10 --seed 42 --report` matches the 787cab0 binary byte for
byte except the wall-clock `ticks_per_sec` column.

**Cost** (`cargo test --release -p citysim --test lod bench_stat_policy_row -- --ignored --nocapture`,
2,000 city, 1,999 agents):

| Path | µs per agent-hour |
|---|---|
| table row | 0.003 |
| features only | 0.037 |
| MLP, features + both nets (H=32) | **0.31** |

The Statistical tier runs about 30 agent-hours per tick, so the MLP adds about 9 µs per tick.

`run --days 30 --seed 42 --report` at 2,000 agents, three alternating pairs, measured in wall
ticks/s:

| Rep | table | mlp (safe) |
|---|---|---|
| 1 | 12,152 | 10,424 |
| 2 | 11,635 | 10,177 |
| 3 | 15,530 | 13,724 |
| mean | 13,106 | 11,442 (**−12.7 %**) |

Part of the gap is trajectory: different draws make a different city. The forward pass is scalar
code with an index gather. With fixed-size contiguous inputs it could vectorise to roughly a
quarter of the cost, but none of the conclusions depend on that.

## 5. Parity (`test_full_vs_statistical_within_15pct`, `CITYSIM_STAT_POLICY=mlp`)

Full vs Statistical, per 100 agent-days. Seeds 2000–2005, 500 agents, 30 days.

| Metric | Full | Table | **MLP 32 (all features)** | MLP 32 safe (no mood/zone) | Tolerance |
|---|---|---|---|---|---|
| thefts | 2.214 | 2.054 ✓ | **4.864 ✗** | 2.037 ✓ | 0.50 |
| hunger-days | 0.118 | 0.014 ✓ | 0.030 ✓ | 0.012 ✓ | 0.50 |
| arrests | 0.969 | 0.667 ✓ | **1.751 ✗** | 0.650 ✓ | 0.50 |
| assaults | 1.047 | 1.083 ✓ | **2.311 ✗** | 1.169 ✓ | 0.50 |
| violent deaths | 0.046 | 0.061 ✓ | 0.113 ✓ | 0.054 ✓ | 0.50 |
| marriages | 0.879 | 0.707 ✓ | 0.723 ✓ | 0.660 ✓ | 0.50 |
| actor share law0 | 0.228 | 0.248 ✓ | 0.246 ✓ | 0.255 ✓ | 0.050 |
| actor share law1 | 0.542 | 0.542 ✓ | 0.558 ✓ | 0.524 ✓ | 0.081 |
| actor share law2 | 0.230 | 0.211 ✓ | 0.196 ✓ | 0.220 ✓ | 0.050 |
| **verdict** | | pass | **fail** | pass | |

**Why the full MLP fails: covariate shift on simulated state.** A temporary probe compared mean
features of the Statistical tier in a forced-Statistical calibration city against the Full
training rows:

- **mood** is about **0.02–0.04 off screen vs 0.18–0.28 for Full agents**. The off-screen hour
  doesn't reproduce mood's inputs: no co-location, the outcome mix is different, and victim
  memories pile up.
- **zone** means "the tile I'm on" for a Full agent (Civic holds 11.7 % of Full hours: work, the
  Hall) but "my home door" for a Statistical one (Civic 0 %).

The MLP correctly learned that unhappy agents steal and fight about 10× more (§ 6, mood row) and was
then fed a city of agents who all look unhappy. Its mean predicted night p_steal was about 0.0010–
0.0015 against the table's 0.00015. The table is immune only because it never looks at mood. The
crime metrics also feed back on themselves: robbed → mood drops → more crime.

**Rule this teaches:** a learned policy may condition only on state that the tier it drives
reproduces with the same distribution as Full. Anything else needs a drift check first.

**120 days, seed 42, the real 2,000 city** (gangs, rent, corps; one seed, so ±10 % is noise):

| | thefts | arrests | marriages | starvation | violent deaths | of which off screen | births | final pop |
|---|---|---|---|---|---|---|---|---|
| table | 6,105 | 2,338 | 626 | 2 | 190 | 106 | 89 | 2,009 |
| MLP safe | 7,011 (+15 %) | 2,442 (+4 %) | 631 | 4 | 224 | 140 | 55 | 1,929 |
| MLP all features | 10,926 (+79 %) | 3,646 (+56 %) | 622 | 1 | 306 | 208 | 75 | 1,870 |

The safe MLP drifts +15 % on thefts in the real city. Its wealth input sees unequal wallets that
the equal-wallet calibration city never had, which is a mild form of the same shift. The
all-features MLP shows the shift at full scale.

## 6. Held-out regimes: does the MLP generalise where the table cannot?

Each test trains on one regime and tests on rows from the other regime across all six seeds, which
no model saw. The "oracle" is an MLP trained on every regime of the train seeds and scored on the
validation seed's held-out rows: the upper bound. The table is the 24-row table fitted on the same
in-regime rows.

**Log-loss** (total, all heads) on the held-out rows:

| Held out (train → test) | Test hours | table | MLP 32 | oracle | MLP verdict |
|---|---|---|---|---|---|
| ≥1 meal of coins → **broke (<1 meal)** | 180 k | 1.339 | **0.808** | 0.786 | generalises: 96 % of the way to oracle |
| mood ≥ −0.2 → **unhappy (< −0.2)** | 17 k | 2.127 | **1.431** | 1.394 | generalises the trend, overshoots the rates |
| employed → **jobless** | 1.94 M | 1.978 | 1.301 | 0.821 | better loss, but assaulted predicted 20× too high |
| other zones → **Sump** | 807 k | 1.334 | 1.232 | 0.560 | barely: a one-hot can't extrapolate |
| housed → homeless | 0 | n/a | n/a | n/a | the calibration city houses everyone |

**Rates** on the held-out rows, observed / predicted:

| Regime | Model | eat | sleep | steal | robbed | assaulted | meet |
|---|---|---|---|---|---|---|---|
| broke | table | .038/.054 | .119/.171 | .0001/.0010 | .0008/.0009 | .0000/.0005 | .019/.025 |
| broke | MLP | .038/.028 | .119/.106 | .0001/.0004 | .0008/.0007 | .0000/.0002 | .019/.017 |
| unhappy | table | .067/.070 | .205/.179 | .0090/.0008 | .0043/.0007 | .0040/.0004 | .086/.023 |
| unhappy | MLP | .067/.092 | .205/.186 | .0090/.0195 | .0043/.0072 | .0040/.0065 | .086/.116 |
| jobless | table | .057/.028 | .161/.230 | .0009/.0021 | .0009/.0004 | .0004/.0011 | .024/.029 |
| jobless | MLP | .057/.016 | .161/.217 | .0009/.0007 | .0009/.0007 | .0004/.0085 | .024/.028 |
| Sump | table | .036/.060 | .230/.122 | .0005/.0013 | .0005/.0012 | .0002/.0006 | .006/.036 |
| Sump | MLP | .036/.031 | .230/.052 | .0005/.0011 | .0005/.0008 | .0002/.0004 | .006/.045 |

**Reading**:

- **Continuous axes generalise.** On wealth and mood the MLP extrapolates the direction correctly
  and lands near the oracle loss. Off the edge of the data, the table is 10× wrong on unhappy-agent
  crime in the other direction.
- **Binary flips and unseen categories do not.** Jobless and Sump show it. With one training value
  the net has nothing to interpolate. The jobless assault rate came out 20× over. Sump sleep came
  out 4× under, worse than the table's zone-blind average.
- New M12–M13 keys (district, class, chrome tier) are categorical. The MLP helps there only if they
  are encoded as **continuous properties of the category**, such as district crime, rent level or
  class wealth (M12's aggregates are exactly this), and not as one-hots.

## 7. What it costs to adopt

- An offline numpy step (`tools/train_stat_policy.py`) runs after `calibrate`. A Rust SGD would
  remove the Python dependency, but it doesn't change the answer.
- About 1.2–1.5 k weights in a TOML asset that nobody can read. The table's 24 rows are
  hand-auditable, and its "Work law1 hunger1 p_steal 0.0016" has been debugged by people.
- About 12 % of 2,000-city throughput (§ 4), recoverable with a vectorised forward pass.
- A new failure mode: covariate shift through simulated state (§ 5). It needs a standing drift
  check (Statistical vs Full feature means), which the parity test catches only indirectly.

## 8. Recommendation

**Keep the table. Move to the MLP only when conditioning grows, and only through "faithful"
features.** Concretely, switch when both of these hold:

1. **The table can't hold the key.** A new key (district, class, chrome) multiplies the table past
   about **100 rows**, or the sparsest row of a 6-seed `calibrate` drops under **500 agent-hours**.
   Today's sparsest row has 158, with the hungry rows already thin. That is the point where (b)
   above starts to overfit and an additive model wins.
2. **The key's input is reproduced off screen.** It passes a drift check: under a forced-Statistical
   calibration city, the input's mean and quantiles sit within about 10 % of the Full rows.
   - Wealth, employment, marriage and age pass, or nearly.
   - Mood and current zone fail today. To use them, either simulate them off screen
     (`stat_social` would have to move mood), or replace them with what the Statistical tier does
     keep: home zone, home district's M12 aggregates.

If M12 lands districts as one-hot zone keys only, the right move is still the table keyed by
district, about 24 × 6 rows with hunger pooling. The MLP's edge is continuous district properties
plus wealth and class, at the moment the table's cells run dry. Before that moment, it is a
slower, opaque table that is easy to break.

Kept from the experiment (all opt-in, default unchanged):

- the `StatPolicy` trait and `[lod] policy`;
- `calibrate --rows-csv/--first-seed`;
- `run --stat-policy`;
- `CITYSIM_STAT_POLICY` in the parity test;
- `bench_stat_policy_row`;
- `assets/stat_mlp.toml`, the safe 17→32 net without mood and zone, which passes parity.

## 9. How to reproduce

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release -p citysim-cli
# 1. data: six seeds in parallel, one CSV each (about 5 s total on 24 cores)
for s in 1000 1001 1002 1003 1004 1005; do
  ./target/release/citysim-cli calibrate --seeds 1 --first-seed $s \
    --rows-csv distil/rows_$s.csv --out distil/table_$s.toml &
done; wait
# 2. models, losses, calibration; export (needs numpy)
python tools/train_stat_policy.py distil --export distil/stat_mlp32.toml --hidden 32
python tools/train_stat_policy.py distil --export assets/stat_mlp.toml --hidden 32 \
  --drop mood,z_spire,z_civic,z_vats,z_mid,z_sump      # the shipped "safe" net
python tools/train_stat_policy.py distil --heldout     # § 6
# 3. parity, table vs mlp
cargo test --release -p citysim --test lod -- --ignored --nocapture test_full_vs_statistical
CITYSIM_STAT_POLICY=mlp cargo test --release -p citysim --test lod -- --ignored --nocapture test_full_vs_statistical
# 4. cost and throughput
cargo test --release -p citysim --test lod bench_stat_policy_row -- --ignored --nocapture
./target/release/citysim-cli run --days 30 --seed 42 --report --stat-policy table > t.csv
./target/release/citysim-cli run --days 30 --seed 42 --report --stat-policy mlp > m.csv
# 5. 120 days (marriages from the event stream)
./target/release/citysim-cli run --days 120 --seed 42 --report --events --stat-policy mlp > r.csv 2> ev.tsv
grep -c $'\tMarriage\t' ev.tsv
```
