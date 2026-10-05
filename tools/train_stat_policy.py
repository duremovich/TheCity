"""Learned Statistical-tier policy experiment: fit the 24-row table, a coarse
extended table and 2-layer MLPs on `calibrate --rows-csv` agent-hours; report
losses, calibration and held-out regimes; export weights.

usage: python tools/train_stat_policy.py <dir with rows_*.csv> [--export out.toml --hidden 32]
       [--drop mood,z_spire,...] [--heldout]
Not stdlib-only: needs numpy.
"""
import glob
import json
import os
import sys
import time

import numpy as np

D = sys.argv[1]
ARGS = sys.argv[2:]
EXPORT = ARGS[ARGS.index("--export") + 1] if "--export" in ARGS else None
SEEDS_TRAIN = [1000, 1001, 1002, 1003, 1004]
SEED_VAL = 1005

FEAT = ["ph_morning", "ph_work", "ph_evening", "ph_night", "lawfulness", "hunger", "wealth", "employed",
        "housed", "married", "age", "mood", "z_spire", "z_civic", "z_vats", "z_mid", "z_sump"]
NF = len(FEAT)
HUNGER = FEAT.index("hunger")
DROP = set(ARGS[ARGS.index("--drop") + 1].split(",")) if "--drop" in ARGS else set()
OUT_IN = [i for i in range(NF) if FEAT[i] not in DROP]
ROLL_IN = [i for i in OUT_IN if i != HUNGER]
ROLLS = ["steal", "flirt", "robbed", "assaulted", "killed", "meet", "chat", "chat_home"]
EPS = 1e-7


def load():
    cache = os.path.join(D, "rows.npy")
    if os.path.exists(cache):
        return np.load(cache)
    parts = []
    for p in sorted(glob.glob(os.path.join(D, "rows_*.csv"))):
        parts.append(np.loadtxt(p, delimiter=",", skiprows=1, dtype=np.float32))
    a = np.concatenate(parts)
    np.save(cache, a)
    return a


A = load()
cols = ["seed", "tick", "agent", "row"] + FEAT + ["outcome", "courting", "steal", "flirt", "robbed", "assaulted",
                                                  "killed", "met", "chats", "chats_home"]
C = {c: i for i, c in enumerate(cols)}
X = A[:, 4:4 + NF].astype(np.float32)
seed = A[:, C["seed"]].astype(int)
row24 = A[:, C["row"]].astype(int)
outcome = A[:, C["outcome"]].astype(int)
courting = A[:, C["courting"]] > 0
chats = A[:, C["chats"]]
# Roll targets and masks (which rows each head is trained / scored on).
Y = np.stack([
    A[:, C["steal"]], A[:, C["flirt"]], A[:, C["robbed"]], A[:, C["assaulted"]], A[:, C["killed"]],
    np.minimum(A[:, C["met"]], 2) / 2.0, np.minimum(chats, 1),
    np.where(chats > 0, A[:, C["chats_home"]] / np.maximum(chats, 1), 0),
], 1).astype(np.float32)
M = np.stack([np.ones(len(A), bool), courting] + [np.ones(len(A), bool)] * 5 + [chats > 0], 1)


def report_data():
    n = len(A)
    print(f"rows {n} ({', '.join(str((seed == s).sum()) for s in sorted(set(seed)))} per seed)")
    names = ["eat", "work", "social", "sleep", "idle"]
    bc = np.bincount(outcome, minlength=5) / n
    print("outcome balance: " + ", ".join(f"{k} {v:.3f}" for k, v in zip(names, bc)))
    for j, r in enumerate(ROLLS):
        m = M[:, j]
        print(f"  {r:10} positives {Y[m, j].sum():9.0f} of {m.sum():8d} rows: rate {Y[m, j].mean():.5f}")
    print(f"  met>2 hours {(A[:, C['met']] > 2).sum()}, chats>1 hours {(chats > 1).sum()} (clipped targets)")
    for f in ["employed", "housed", "married"]:
        print(f"  {f} share {X[:, FEAT.index(f)].mean():.3f}")
    print("  zone shares " + ", ".join(f"{z} {X[:, FEAT.index(z)].mean():.3f}" for z in FEAT[12:]))
    w = X[:, FEAT.index("wealth")]
    print("  wealth quantiles (meals/20) " + str(np.round(np.quantile(w, [0.05, 0.25, 0.5, 0.75, 0.95]), 3)))
    print("  child share %.3f" % (X[:, FEAT.index("age")] < 0.18).mean())


# ----------------------------------------------------------------- losses

def losses(P_out, P_roll, idx):
    """Mean log-loss on rows `idx`: outcome CE, and per roll BCE on its mask."""
    res = {"outcome": float(-np.log(np.clip(P_out[idx, outcome[idx]], EPS, 1)).mean())}
    for j, r in enumerate(ROLLS):
        m = idx[M[idx, j]]
        y, p = Y[m, j], np.clip(P_roll[m, j], EPS, 1 - EPS)
        res[r] = float(-(y * np.log(p) + (1 - y) * np.log(1 - p)).mean()) if len(m) else float("nan")
    res["total"] = res["outcome"] + sum(res[r] for r in ROLLS)
    return res


# ----------------------------------------------------------------- tables

def fit_table(idx, key_out, key_roll, prior=None, k=0.0):
    """Cell means. `key_out`/`key_roll` map rows to cell ids; `prior` (P_out,
    P_roll over all rows) shrinks sparse cells with `k` pseudo-counts."""
    n_out = key_out.max() + 1
    n_roll = key_roll.max() + 1
    P_out = np.zeros((len(A), 5), np.float32)
    P_roll = np.zeros((len(A), 8), np.float32)
    cnt = np.zeros((n_out, 5))
    np.add.at(cnt, (key_out[idx], outcome[idx]), 1)
    tot = cnt.sum(1, keepdims=True)
    if prior is not None:
        # Prior mean per cell = mean prior prediction of the training rows in it.
        pc = np.zeros((n_out, 5))
        np.add.at(pc, key_out[idx], prior[0][idx])
        pm = pc / np.maximum(tot, 1)
        cell_out = (cnt + k * pm) / (tot + k)
        P_out = np.where(tot[key_out] > 0, cell_out[key_out], prior[0])
    else:
        cell_out = cnt / np.maximum(tot, 1)
        P_out = cell_out[key_out]
    for j in range(8):
        m = idx[M[idx, j]]
        s = np.bincount(key_roll[m], weights=Y[m, j], minlength=n_roll)
        c = np.bincount(key_roll[m], minlength=n_roll).astype(float)
        if prior is not None:
            ps = np.bincount(key_roll[m], weights=prior[1][m, j], minlength=n_roll)
            pm = ps / np.maximum(c, 1)
            cell = (s + k * pm) / (c + k)
            P_roll[:, j] = np.where(c[key_roll] > 0, cell[key_roll], prior[1][:, j])
        else:
            P_roll[:, j] = (s / np.maximum(c, 1))[key_roll]
    return P_out.astype(np.float32), P_roll.astype(np.float32), (tot[:, 0] > 0).sum()


def table24(idx):
    # Outcome by the 24 rows; rolls pooled over hunger (12 cells), as calibrate.
    return fit_table(idx, row24, row24 // 2)


def ext_keys():
    ph = X[:, :4].argmax(1)
    law = np.digitize(X[:, 4], [0.3, 0.7])
    hun = (X[:, 5] >= 0.4).astype(int)
    wl = np.digitize(X[:, 6], [0.05, 0.25])  # <1 meal, <5 meals, more
    emp, hou, mar = (X[:, 7] > 0).astype(int), (X[:, 8] > 0).astype(int), (X[:, 9] > 0).astype(int)
    age = np.digitize(X[:, 10], [0.18, 0.60])
    mood = np.digitize(X[:, 11], [-0.3, 0.3])
    zone = X[:, 12:].argmax(1)
    dims = [4, 3, 2, 3, 2, 2, 2, 3, 3, 5]
    parts = [ph, law, hun, wl, emp, hou, mar, age, mood, zone]
    key = np.zeros(len(A), int)
    for p, d in zip(parts, dims):
        key = key * d + p
    # Rolls pooled over hunger: drop the hunger digit.
    rk = np.zeros(len(A), int)
    for i, (p, d) in enumerate(zip(parts, dims)):
        if i != 2:
            rk = rk * d + p
    return key, rk, int(np.prod(dims))


# ----------------------------------------------------------------- MLP

class Net:
    def __init__(self, inputs, hidden, outputs, rng):
        self.inputs = inputs
        n = len(inputs)
        self.W1 = (rng.standard_normal((hidden, n)) * np.sqrt(2.0 / n)).astype(np.float32)
        self.b1 = np.zeros(hidden, np.float32)
        self.W2 = (rng.standard_normal((outputs, hidden)) * np.sqrt(1.0 / hidden)).astype(np.float32)
        self.b2 = np.zeros(outputs, np.float32)
        self.params = [self.W1, self.b1, self.W2, self.b2]
        self.m = [np.zeros_like(p) for p in self.params]
        self.v = [np.zeros_like(p) for p in self.params]
        self.t = 0

    def forward(self, Z):
        H = np.maximum(Z @ self.W1.T + self.b1, 0)
        return H, H @ self.W2.T + self.b2

    def step(self, Z, H, dO, lr, wd=1e-5):
        gW2 = dO.T @ H
        gb2 = dO.sum(0)
        dH = (dO @ self.W2) * (H > 0)
        gW1 = dH.T @ Z
        gb1 = dH.sum(0)
        self.t += 1
        for i, (p, g) in enumerate(zip(self.params, [gW1, gb1, gW2, gb2])):
            if i in (0, 2):
                g = g + wd * p
            self.m[i] = 0.9 * self.m[i] + 0.1 * g
            self.v[i] = 0.999 * self.v[i] + 0.001 * g * g
            mh = self.m[i] / (1 - 0.9 ** self.t)
            vh = self.v[i] / (1 - 0.999 ** self.t)
            p -= lr * mh / (np.sqrt(vh) + 1e-8)


def softmax(o):
    o = o - o.max(1, keepdims=True)
    e = np.exp(o)
    return e / e.sum(1, keepdims=True)


def sigmoid(o):
    return 1 / (1 + np.exp(-np.clip(o, -40, 40)))


class MLP:
    def __init__(self, hidden, train_idx, seed=0):
        rng = np.random.default_rng(seed)
        self.mu = X[train_idx].mean(0)
        sd = X[train_idx].std(0)
        self.sd = np.where(sd > 1e-6, sd, 1.0)
        self.out = Net(OUT_IN, hidden, 5, rng)
        self.rolls = Net(ROLL_IN, hidden, 8, rng)
        # Rare-event heads start at their base rate.
        base = np.array([Y[train_idx][M[train_idx, j], j].mean() for j in range(8)])
        self.rolls.b2[:] = np.log(np.clip(base, 1e-5, 1) / np.clip(1 - base, 1e-5, 1))
        self.out.b2[:] = np.log(np.bincount(outcome[train_idx], minlength=5) / len(train_idx) + 1e-6)
        self.unseen = sd <= 1e-6  # constant in training (a held-out one-hot)

    def z(self, idx, cols):
        return ((X[idx] - self.mu) / self.sd)[:, cols].astype(np.float32)

    def predict(self, idx):
        _, o = self.out.forward(self.z(idx, self.out.inputs))
        _, r = self.rolls.forward(self.z(idx, self.rolls.inputs))
        return softmax(o), sigmoid(r)

    def fit(self, train_idx, val_idx, epochs=12, batch=4096, lr=2e-3, verbose=True):
        rng = np.random.default_rng(1)
        best, best_state = 1e9, None
        for ep in range(epochs):
            perm = rng.permutation(train_idx)
            lr_ep = lr * (0.5 if ep >= epochs * 2 // 3 else 1.0)
            for s in range(0, len(perm), batch):
                b = perm[s:s + batch]
                n = len(b)
                Zo = self.z(b, self.out.inputs)
                H, o = self.out.forward(Zo)
                p = softmax(o)
                p[np.arange(n), outcome[b]] -= 1
                self.out.step(Zo, H, p / n, lr_ep)
                Zr = self.z(b, self.rolls.inputs)
                H, r = self.rolls.forward(Zr)
                g = (sigmoid(r) - Y[b]) * M[b]
                # Each head averaged over its own masked rows.
                g = g / np.maximum(M[b].sum(0), 1)
                self.rolls.step(Zr, H, g.astype(np.float32), lr_ep)
            if val_idx is not None and len(val_idx):
                Po, Pr = self.full_predict()
                lv = losses(Po, Pr, val_idx)["total"]
                if verbose:
                    print(f"    epoch {ep}: val total {lv:.5f}")
                if lv < best:
                    best = lv
                    best_state = [[p.copy() for p in net.params] for net in (self.out, self.rolls)]
        if best_state is not None:
            for net, st in zip((self.out, self.rolls), best_state):
                for p, q in zip(net.params, st):
                    p[...] = q

    def full_predict(self):
        idx = np.arange(len(A))
        Po, Pr = np.zeros((len(A), 5), np.float32), np.zeros((len(A), 8), np.float32)
        for s in range(0, len(A), 200000):
            a, b = self.predict(idx[s:s + 200000])
            Po[s:s + 200000], Pr[s:s + 200000] = a, b
        return Po, Pr

    def n_params(self):
        return sum(p.size for net in (self.out, self.rolls) for p in net.params)

    def export_net(self, net, zero_unseen=False):
        cols = net.inputs
        sd, mu = self.sd[cols], self.mu[cols]
        W1 = net.W1 / sd
        b1 = net.b1 - (net.W1 * (mu / sd)).sum(1)
        if zero_unseen:
            W1 = np.where(self.unseen[cols][None, :], 0.0, W1)
        return {"inputs": cols, "hidden": int(W1.shape[0]), "outputs": int(net.W2.shape[0]),
                "w1": W1.ravel().tolist(), "b1": b1.tolist(), "w2": net.W2.ravel().tolist(),
                "b2": net.b2.tolist()}


def toml_net(name, d):
    fl = lambda v: "[" + ", ".join(repr(float(np.float32(x))) for x in v) + "]"
    return (f"[{name}]\ninputs = [{', '.join(str(i) for i in d['inputs'])}]\nhidden = {d['hidden']}\n"
            f"outputs = {d['outputs']}\nw1 = {fl(d['w1'])}\nb1 = {fl(d['b1'])}\nw2 = {fl(d['w2'])}\n"
            f"b2 = {fl(d['b2'])}\n")


# ----------------------------------------------------------------- calibration

HEADS = ["eat", "work", "social", "sleep"] + ROLLS


def calib(P_out, P_roll, idx):
    """Per original 24-row bucket: predicted vs observed rate per head.
    Returns the row-weighted mean |pred - obs| / overall rate per head."""
    obs_o = np.eye(5)[outcome]
    out = {}
    for h, name in enumerate(HEADS):
        if h < 4:
            pred, obs, m = P_out[:, h], obs_o[:, h], np.ones(len(A), bool)
        else:
            j = h - 4
            pred, obs, m = P_roll[:, j], Y[:, j], M[:, j]
        sel = idx[m[idx]]
        err, wt = 0.0, 0
        for r in range(24):
            s = sel[row24[sel] == r]
            if len(s) == 0:
                continue
            err += len(s) * abs(pred[s].mean() - obs[s].mean())
            wt += len(s)
        out[name] = (err / max(wt, 1)) / max(obs[sel].mean(), 1e-9)
    return out


def bucket_table(P_out, P_roll, idx, heads=("eat", "steal", "robbed", "meet")):
    obs_o = np.eye(5)[outcome]
    lines = []
    for r in range(24):
        s = idx[row24[idx] == r]
        cells = []
        for name in heads:
            h = HEADS.index(name)
            if h < 4:
                cells.append(f"{obs_o[s, h].mean():.4f}/{P_out[s, h].mean():.4f}")
            else:
                j = h - 4
                ss = s[M[s, j]]
                cells.append(f"{Y[ss, j].mean():.4f}/{P_roll[ss, j].mean():.4f}" if len(ss) else "-")
        lines.append(f"| {r:2d} | {len(s):6d} | " + " | ".join(cells) + " |")
    return "\n".join(lines)


# ----------------------------------------------------------------- main

def main():
    report_data()
    tr = np.where(np.isin(seed, SEEDS_TRAIN))[0]
    va = np.where(seed == SEED_VAL)[0]
    results = {}
    t = time.time()
    Pa = table24(tr)
    results["a table24"] = (Pa, 24 * 4 + 12 * 8)
    key, rk, ncells = ext_keys()
    Pb = fit_table(tr, key, rk, prior=Pa[:2], k=20.0)
    results["b ext-table"] = (Pb, ncells * 4 + ncells // 2 * 8)
    print(f"ext table: {ncells} outcome cells ({ncells // 2} roll cells), populated in train: {Pb[2]}")
    for hidden in (32, 16):
        mlp = MLP(hidden, tr)
        print(f"  training MLP {hidden}")
        t0 = time.time()
        mlp.fit(tr, va)
        print(f"  trained in {time.time() - t0:.0f}s")
        # Inputs constant in training (housed: the calibration city houses
        # everyone) keep their random init weights: zero them, so an unseen
        # value (a homeless agent) reads as the training value.
        for net in (mlp.out, mlp.rolls):
            net.W1[:, mlp.unseen[net.inputs]] = 0.0
        results[f"{'c' if hidden == 32 else 'd'} mlp{hidden}"] = (mlp.full_predict(), mlp.n_params())
        if EXPORT and hidden == int(ARGS[ARGS.index("--hidden") + 1] if "--hidden" in ARGS else 32):
            text = "# stat_mlp: learned Statistical policy (docs/EXPERIMENT_LEARNED_STAT_POLICY.md), " \
                   f"hidden {hidden}, trained on calibrate seeds {SEEDS_TRAIN}\nversion = 1\n\n"
            text += toml_net("outcome", mlp.export_net(mlp.out)) + "\n" + toml_net("rolls", mlp.export_net(mlp.rolls))
            with open(EXPORT, "w", newline="\n") as f:
                f.write(text)
            print(f"  exported {EXPORT}")
            # A check vector for the Rust forward pass.
            Po, Pr = mlp.predict(va[:3])
            print("  check rows", va[:3].tolist(), np.round(Po, 6).tolist(), np.round(Pr, 6).tolist())
    print("\n## losses (mean log-loss per agent-hour; rolls on their own masks)")
    print("| model | params | train total | val total | val outcome | val steal | val flirt | val robbed |"
          " val assaulted | val killed | val meet | val chat | val chat_home |")
    for name, (P, npar) in results.items():
        lt, lv = losses(P[0], P[1], tr), losses(P[0], P[1], va)
        print(f"| {name} | {npar} | {lt['total']:.5f} | {lv['total']:.5f} | {lv['outcome']:.5f} | "
              + " | ".join(f"{lv[r]:.5f}" for r in ROLLS) + " |")
    print("\n## calibration (val): bucket-weighted mean |pred-obs| / overall rate, per head")
    print("| model | " + " | ".join(HEADS) + " |")
    for name, (P, _) in results.items():
        c = calib(P[0], P[1], va)
        print(f"| {name} | " + " | ".join(f"{c[h]:.3f}" for h in HEADS) + " |")
    for name in ("a table24", "c mlp32"):
        print(f"\n### {name}: per bucket, val observed/predicted")
        print("| row | hours | eat | steal | robbed | meet |")
        P = results[name][0]
        print(bucket_table(P[0], P[1], va))
    print(f"\n(done in {time.time() - t:.0f}s)")


def heldout():
    """Train on one regime, test on another (val seed's held-out rows; also
    train-seed held-out rows, which no model saw)."""
    regimes = {
        "homeless (train housed)": X[:, FEAT.index("housed")] > 0,
        "jobless (train employed)": X[:, FEAT.index("employed")] > 0,
        "broke <1 meal (train >=1 meal)": X[:, FEAT.index("wealth")] >= 0.05,
        "Sump (train other zones)": X[:, FEAT.index("z_sump")] == 0,
        "unhappy mood<-0.2 (train >=-0.2)": X[:, FEAT.index("mood")] >= -0.2,
    }
    allidx = np.arange(len(A))
    tr_seeds = np.isin(seed, SEEDS_TRAIN)
    print("\n## held-out regimes: test rows = the held-out regime on all six seeds (never seen in training)")
    print("| regime | test hours | model | test total | outcome | steal | robbed | assaulted | meet | chat |")
    rate_lines = []
    for name, keep in regimes.items():
        tr = allidx[tr_seeds & keep]
        te = allidx[~keep]
        va_in = allidx[(seed == SEED_VAL) & keep]
        oracle_tr = allidx[tr_seeds]
        te_val = allidx[(seed == SEED_VAL) & ~keep]
        if len(te) < 2000:
            print(f"| {name} | {len(te)} | (too few) |")
            continue
        models = {"table24": table24(tr)[:2]}
        mlp = MLP(32, tr, seed=0)
        mlp.fit(tr, va_in, epochs=8, verbose=False)
        models["mlp32"] = mlp.full_predict()
        if mlp.unseen.any():
            Wsave = [mlp.out.W1.copy(), mlp.rolls.W1.copy()]
            for net in (mlp.out, mlp.rolls):
                net.W1[:, mlp.unseen[net.inputs]] = 0.0
            models["mlp32 unseen-zeroed"] = mlp.full_predict()
            mlp.out.W1[...], mlp.rolls.W1[...] = Wsave
        # Oracle: trained on all regimes of the train seeds; scored on the val seed's held-out rows.
        orc = MLP(32, oracle_tr, seed=0)
        orc.fit(oracle_tr, allidx[seed == SEED_VAL], epochs=8, verbose=False)
        models["oracle mlp32 (val seed)"] = orc.full_predict()
        models["table24 (val seed)"] = models["table24"]
        models["mlp32 (val seed)"] = models["mlp32"]
        for mname, (Po, Pr) in models.items():
            idx = te_val if "val seed" in mname else te
            l = losses(Po, Pr, idx)
            print(f"| {name} | {len(idx)} | {mname} | {l['total']:.5f} | {l['outcome']:.5f} | {l['steal']:.5f} | "
                  f"{l['robbed']:.5f} | {l['assaulted']:.5f} | {l['meet']:.5f} | {l['chat']:.5f} |")
            if "val seed" in mname:
                continue
            obs_o = np.eye(5)[outcome]
            cells = []
            for h in ("eat", "sleep", "steal", "robbed", "assaulted", "meet", "chat"):
                k = HEADS.index(h)
                if k < 4:
                    cells.append(f"{obs_o[idx, k].mean():.4f}/{Po[idx, k].mean():.4f}")
                else:
                    j = k - 4
                    s = idx[M[idx, j]]
                    cells.append(f"{Y[s, j].mean():.4f}/{Pr[s, j].mean():.4f}")
            rate_lines.append(f"| {name} | {mname} | " + " | ".join(cells) + " |")
    print("\n## held-out regimes: observed/predicted rate on the held-out rows")
    print("| regime | model | eat | sleep | steal | robbed | assaulted | meet | chat |")
    print("\n".join(rate_lines))


if __name__ == "__main__":
    if "--heldout" in ARGS:
        report_data()
        heldout()
    else:
        main()
