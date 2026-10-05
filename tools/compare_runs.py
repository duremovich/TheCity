#!/usr/bin/env python3
"""Side-by-side comparison of two per-day run reports.

Usage: python tools/compare_runs.py a.csv b.csv

Input: CSV from `citysim-cli run --report` (header line expected; columns are matched by
name, so a column present in only one file shows '-' for the other). Non-numeric columns
(e.g. season) are skipped. Prints the mean of each numeric column and the relative
difference (b - a) / |a|.
"""
import csv
import sys


def means(path):
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f))
    out = {}
    for k in rows[0]:
        try:
            out[k] = sum(float(r[k]) for r in rows) / len(rows)
        except ValueError:
            pass
    return out


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    a, b = means(sys.argv[1]), means(sys.argv[2])
    print(f"{'column':<26}{'A':>12}{'B':>12}{'rel diff':>10}")
    for k in list(a) + [k for k in b if k not in a]:
        x, y = a.get(k), b.get(k)
        rel = f"{(y - x) / abs(x):+.1%}" if x and y is not None else "-"
        fx = "-" if x is None else f"{x:.4g}"
        fy = "-" if y is None else f"{y:.4g}"
        print(f"{k:<26}{fx:>12}{fy:>12}{rel:>10}")


if __name__ == "__main__":
    main()
