#!/usr/bin/env python3
"""Generate assets/map.txt, the M10 v2 map (docs/M10_SCALE.md section 2).

256 x 192 tiles. Cells of 7 x 7 tiles between roads: column c = 0..31 covers x = 8c..8c+6, row
r = 0..22 covers y = 8r..8r+6. Vertical roads at x = 7 + 8k (x = 255 is the east edge road),
horizontal roads at y = 7 + 8k up to y = 183; y = 184..188 is open Sump ground and a 3-tile Water
strip runs along the south edge.

Zones (per tile, first match wins): x >= 208 Vats; y >= 112 Sump; y < 40 and x >= 64 Spire;
40 <= y < 80 and 96 <= x < 176 Civic; else Mid.

Buildings: 400 Blocks (Homes) at 5 residents (60 Spire tier 2, 140 Mid tier 1, 200 Sump tier 0),
12 Vat Farms in the Vats, 3 Street Markets and 3 Bars (one each Civic, two each Mid), the Precinct
(Jail) and the Civic Hall in Civic, the Recycler (Cemetery), Reserve Depot (Warehouse) and two
Security Offices in the Vats, two Hideouts in the SW and SE corners of the Sump, and 160 Lots
(open plots: no walls, one door) spread over every zone: the M10 60 plus Jobs and room J5's 100
on the free ground (Vats 40, 20 of them double-wide yards; Mid 30; Civic 12; Spire 10; Sump 8). Double-wide buildings overwrite the road
segment between their two cells; the BFS below proves every door is still reachable.

The old 96 x 64 map is assets/map_v1.txt (unit tests only).

Run:  python assets/gen_map.py > assets/map.txt
The output is committed; the loader never calls this script.
"""
import sys
from collections import deque

W, H = 256, 192
GROUND, WALL, ROAD, DOOR, FARM, WATER = ".", "#", "=", "D", "f", "~"
COLS, ROWS = 32, 23

grid = [[GROUND] * W for _ in range(H)]


def zone_of(x, y):
    if x >= 208:
        return "V"
    if y >= 112:
        return "U"
    if y < 40 and x >= 64:
        return "S"
    if 40 <= y < 80 and 96 <= x < 176:
        return "C"
    return "M"


zones = [[zone_of(x, y) for x in range(W)] for y in range(H)]

# Roads, then water.
for k in range(COLS):
    x = 7 + 8 * k
    for y in range(0, 189):
        grid[y][x] = ROAD
for k in range(ROWS):
    y = 7 + 8 * k
    for x in range(W):
        grid[y][x] = ROAD
for y in range(189, 192):
    for x in range(W):
        grid[y][x] = WATER

buildings = []  # (kind, x, y, w, h, door_x, door_y, tier)
used = set()  # cells taken


def take(cells):
    for cell in cells:
        assert cell not in used, ("cell used twice", cell)
        used.add(cell)


def place(kind, c, r, tier=1):
    """One building from its template at cell (c, r) (the top-left cell for multi-cell kinds)."""
    if kind == "Home":
        rect, door, cells = (8 * c + 1, 8 * r + 3, 5, 4), (8 * c + 3, 8 * r + 6), [(c, r)]
    elif kind in ("Hall", "Warehouse", "SecurityOffice", "Lot"):
        rect, door, cells = (8 * c, 8 * r + 1, 7, 6), (8 * c + 3, 8 * r + 6), [(c, r)]
    elif kind == "Cemetery":
        rect, door, cells = (8 * c, 8 * r, 7, 7), (8 * c + 3, 8 * r + 6), [(c, r)]
    elif kind in ("Market", "Bar", "Lot2"):
        rect, door, cells = (8 * c, 8 * r + 1, 15, 6), (8 * c + 7, 8 * r + 6), [(c, r), (c + 1, r)]
    elif kind == "Farm":
        rect, door, cells = (8 * c, 8 * r, 15, 7), (8 * c + 7, 8 * r + 6), [(c, r), (c + 1, r)]
    elif kind == "Hideout":
        rect, door, cells = (8 * c, 8 * r + 2, 15, 5), (8 * c + 7, 8 * r + 6), [(c, r), (c + 1, r)]
    elif kind == "Jail":
        rect, door = (8 * c, 8 * r, 15, 15), (8 * c + 7, 8 * r + 14)
        cells = [(c, r), (c + 1, r), (c, r + 1), (c + 1, r + 1)]
    else:
        raise ValueError(kind)
    take(cells)
    x, y, w, h = rect
    # Jobs and room J5: a double-wide Lot is a plain `B Lot` line 15 wide.
    buildings.append(("Lot" if kind == "Lot2" else kind, x, y, w, h, door[0], door[1], tier))


def cell_zone(c, r):
    return zones[8 * r][8 * c]


# --- Civic ---------------------------------------------------------------------
place("Jail", 16, 6)
place("Hall", 14, 7)
place("Market", 12, 7)
place("Bar", 18, 7)
for c in range(12, 22):
    place("Lot", c, 5)

# --- Mid -----------------------------------------------------------------------
place("Market", 2, 7)
place("Bar", 4, 7)
place("Market", 16, 11)
place("Bar", 18, 11)
for c in range(0, 8):
    place("Lot", c, 0)
for c in (8, 9, 22, 23, 24, 25):
    place("Lot", c, 5)

# --- Spire ---------------------------------------------------------------------
for c in range(8, 18):
    place("Lot", c, 0)

# --- Vats ----------------------------------------------------------------------
for c in (26, 28, 30):
    for r in range(1, 5):
        place("Farm", c, r)
place("Warehouse", 26, 6)
place("Cemetery", 28, 6)
place("SecurityOffice", 30, 6)
place("SecurityOffice", 26, 8)
for c in range(27, 32):
    place("Lot", c, 8)
for c in range(26, 31):
    place("Lot", c, 10)

# --- Sump ----------------------------------------------------------------------
place("Hideout", 0, 22)
place("Hideout", 24, 22)
for c in range(0, 16):
    place("Lot", c, 14)


def stride(cells, n):
    assert len(cells) >= n, (len(cells), n)
    return [cells[i * len(cells) // n] for i in range(n)]


def free_cells(zone, rows=range(ROWS), cols=range(COLS)):
    return [(c, r) for r in rows for c in cols if (c, r) not in used and cell_zone(c, r) == zone]


spire = free_cells("S")
mid = free_cells("M", cols=range(26))
sump = free_cells("U", rows=range(15, 23), cols=range(26))
for (c, r) in stride(spire, 60):
    place("Home", c, r, 2)
for (c, r) in stride(mid, 140):
    place("Home", c, r, 1)
for (c, r) in stride(sump, 200):
    place("Home", c, r, 0)

# --- Jobs and room J5: Lots on the free ground ----------------------------------
# docs/JOBS_V2.md § 2.2: +100 Lots by zone quota (placeholders), chosen by `stride` as the Homes are.
# The Vats take 20 double-wide yards (two adjacent free cells in a row, greedy row-major) and 20
# single Lots; ~127 free cells stay open ground.
NEW_LOTS = {"V": 40, "M": 30, "C": 12, "S": 10, "U": 8}
VATS_DOUBLE = 20
vats_free = free_cells("V")
pairs, paired = [], set()
for (c, r) in vats_free:
    if (c, r) in paired or (c + 1, r) in paired or (c + 1, r) not in vats_free:
        continue
    pairs.append((c, r))
    paired.update({(c, r), (c + 1, r)})
for (c, r) in stride(pairs, VATS_DOUBLE):
    place("Lot2", c, r)
for zone in "VMCSU":
    singles = NEW_LOTS[zone] - (VATS_DOUBLE if zone == "V" else 0)
    for (c, r) in stride(free_cells(zone), singles):
        place("Lot", c, r)

# --- stamp ---------------------------------------------------------------------
for (kind, x, y, w, h, dx, dy, tier) in buildings:
    for yy in range(y, y + h):
        for xx in range(x, x + w):
            on_perimeter = yy in (y, y + h - 1) or xx in (x, x + w - 1)
            if kind == "Lot":
                grid[yy][xx] = GROUND
            elif on_perimeter:
                grid[yy][xx] = WALL
            else:
                grid[yy][xx] = FARM if kind == "Farm" else GROUND
    assert dy in (y, y + h - 1) or dx in (x, x + w - 1), (kind, dx, dy)
    grid[dy][dx] = DOOR

KIND_ORDER = ["Home", "Farm", "Market", "Bar", "Jail", "Cemetery", "Hall", "Hideout", "Warehouse",
              "SecurityOffice", "Lot"]
buildings.sort(key=lambda b: (KIND_ORDER.index(b[0]), b[2], b[1]))

# --- validation ---------------------------------------------------------------
for (kind, x, y, w, h, dx, dy, tier) in buildings:
    assert x + w <= 255 and y + h <= 184, ("rect reaches the east road or the south band", kind, x, y)
    doors = 0
    for yy in range(y, y + h):
        for xx in range(x, x + w):
            on_perimeter = yy in (y, y + h - 1) or xx in (x, x + w - 1)
            ch = grid[yy][xx]
            if kind == "Lot":
                assert ch in (GROUND, DOOR), (kind, xx, yy, ch)
                doors += ch == DOOR
                assert ch == GROUND or on_perimeter, (kind, xx, yy, "interior door")
            elif on_perimeter:
                assert ch in (WALL, DOOR), (kind, xx, yy, ch)
                doors += ch == DOOR
            else:
                assert ch == (FARM if kind == "Farm" else GROUND), (kind, xx, yy, ch)
    assert doors == 1, (kind, x, y, doors)
    outside = [(dx - 1, dy), (dx + 1, dy), (dx, dy - 1), (dx, dy + 1)]
    outside = [(ox, oy) for (ox, oy) in outside if not (x <= ox < x + w and y <= oy < y + h)]
    assert any(0 <= ox < W and 0 <= oy < H and grid[oy][ox] == ROAD for (ox, oy) in outside), (kind, dx, dy)
    if kind == "Farm":
        assert sum(row[x:x + w].count(FARM) for row in grid[y:y + h]) >= 60

walk = {GROUND, ROAD, DOOR, FARM}
start = (buildings[0][5], buildings[0][6])
seen = {start}
q = deque([start])
while q:
    cx, cy = q.popleft()
    for nx, ny in ((cx - 1, cy), (cx + 1, cy), (cx, cy - 1), (cx, cy + 1)):
        if 0 <= nx < W and 0 <= ny < H and grid[ny][nx] in walk and (nx, ny) not in seen:
            seen.add((nx, ny))
            q.append((nx, ny))
for (kind, x, y, w, h, dx, dy, tier) in buildings:
    assert (dx, dy) in seen, ("unreachable door", kind, dx, dy)

counts = {k: sum(1 for b in buildings if b[0] == k) for k in KIND_ORDER}
assert counts == {"Home": 400, "Farm": 12, "Market": 3, "Bar": 3, "Jail": 1, "Cemetery": 1, "Hall": 1,
                  "Hideout": 2, "Warehouse": 1, "SecurityOffice": 2, "Lot": 160}, counts
lots_by_zone = {}
homes_by_zone = {}
for (kind, x, y, w, h, dx, dy, tier) in buildings:
    z = zones[dy][dx]
    if kind == "Lot":
        lots_by_zone[z] = lots_by_zone.get(z, 0) + 1
    if kind == "Home":
        homes_by_zone[z] = homes_by_zone.get(z, 0) + 1
        assert tier == {"S": 2, "M": 1, "U": 0}[z], (x, y, z, tier)
assert all(lots_by_zone.get(z, 0) >= 8 for z in "SCVMU"), lots_by_zone
assert lots_by_zone == {"S": 20, "C": 22, "V": 50, "M": 44, "U": 24}, lots_by_zone
assert sum(1 for b in buildings if b[0] == "Lot" and b[3] == 15) == VATS_DOUBLE
assert homes_by_zone == {"S": 60, "M": 140, "U": 200}, homes_by_zone

# --- output -------------------------------------------------------------------
out = []
out.append(f"{W} {H}\n")
for row in grid:
    out.append("".join(row) + "\n")
out.append("\n")
for row in zones:
    out.append("".join(row) + "\n")
out.append("\n")
for (kind, x, y, w, h, dx, dy, tier) in buildings:
    out.append(f"B {kind} {x} {y} {w} {h} {tier}\n")
sys.stdout.buffer.write("".join(out).encode("ascii"))
