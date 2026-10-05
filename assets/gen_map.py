#!/usr/bin/env python3
"""Generate assets/map.txt for the Living City Simulator (spec: World model > Map file format).

Layout: a road grid every 8 tiles (vertical roads at x = 7 + 8k for k in 0..9, horizontal roads
at y = 7 + 8k for k in 0..6, horizontal roads stop at x = 79), 5x4 Homes bottom-aligned to the
road below each block, the two Farms and the Warehouse on the east edge facing the x = 79 road,
Market and Hall at the central crossing (44, 31) / (54, 31), the Bar below the Market, the Jail
below the Hall, the Cemetery in the north-west corner, the Hideouts in the south-east and south-west corners, and
a 3-tile Water strip along the south edge.

Run:  python assets/gen_map.py > assets/map.txt
The output is committed; the loader never calls this script.
"""
import sys
from collections import deque

W, H = 96, 64
GROUND, WALL, ROAD, DOOR, FARM, WATER = ".", "#", "=", "D", "f", "~"

grid = [[GROUND] * W for _ in range(H)]

# Water strip along the south edge.
for y in range(61, 64):
    for x in range(W):
        grid[y][x] = WATER

# Road grid.
V_ROADS = [7 + 8 * k for k in range(10)]  # 7..79
H_ROADS = [7 + 8 * k for k in range(7)]   # 7..55
for x in V_ROADS:
    for y in range(0, 61):
        grid[y][x] = ROAD
for y in H_ROADS:
    for x in range(0, 80):
        grid[y][x] = ROAD

buildings = []  # (kind, x, y, w, h, door_x, door_y)


def stamp(kind, x, y, w, h, door):
    dx, dy = door
    for yy in range(y, y + h):
        for xx in range(x, x + w):
            on_perimeter = yy in (y, y + h - 1) or xx in (x, x + w - 1)
            if on_perimeter:
                grid[yy][xx] = WALL
            else:
                grid[yy][xx] = FARM if kind == "Farm" else GROUND
    assert dy in (y, y + h - 1) or dx in (x, x + w - 1), (kind, door)
    grid[dy][dx] = DOOR
    buildings.append((kind, x, y, w, h, dx, dy))


# Special buildings first so Home slots can be excluded where they overlap.
specials = [
    ("Cemetery", 8, 0, 8, 8, (8, 3)),        # door west onto road x=7
    ("Market", 40, 25, 8, 6, (44, 30)),      # door south onto road y=31
    ("Hall", 49, 25, 10, 6, (54, 30)),       # door south onto road y=31
    ("Bar", 40, 32, 8, 6, (44, 32)),         # door north onto road y=31
    ("Jail", 49, 32, 6, 6, (52, 32)),        # door north onto road y=31
    ("Farm", 80, 2, 12, 9, (80, 6)),         # door west onto road x=79
    ("Farm", 80, 14, 12, 9, (80, 18)),
    ("Warehouse", 80, 26, 8, 6, (80, 28)),
    ("Hideout", 80, 56, 7, 5, (80, 58)),
    ("Hideout", 0, 56, 7, 5, (3, 56)),       # door north onto road y=55 (x=7 is a road, so x=0 not x=1)
]


def rect_overlaps(ax, ay, aw, ah, bx, by, bw, bh):
    return ax < bx + bw and bx < ax + aw and ay < by + bh and by < ay + ah


# Home slots: column blocks c = 0..9 (x = 8c .. 8c+6), row blocks r = 0..6 (y = 8r .. 8r+6).
# Each Home is 5x4 at (8c+1, 8r+3) with its door at the bottom centre facing the road at y = 8r+7.
home_slots = []
for r in range(7):
    for c in range(10):
        hx, hy = 8 * c + 1, 8 * r + 3
        if any(rect_overlaps(hx, hy, 5, 4, sx, sy, sw, sh) for (_, sx, sy, sw, sh, _) in specials):
            continue
        home_slots.append((hx, hy))
assert len(home_slots) >= 60, len(home_slots)
home_slots = home_slots[:60]

for (hx, hy) in home_slots:
    stamp("Home", hx, hy, 5, 4, (hx + 2, hy + 3))
for (kind, x, y, w, h, door) in specials:
    stamp(kind, x, y, w, h, door)

# Order buildings exactly as the spec's count table lists kinds, Homes first.
KIND_ORDER = ["Home", "Farm", "Market", "Bar", "Jail", "Cemetery", "Hall", "Hideout", "Warehouse"]
buildings.sort(key=lambda b: (KIND_ORDER.index(b[0]), b[2], b[1]))

# --- validation ---------------------------------------------------------------
for (kind, x, y, w, h, dx, dy) in buildings:
    doors = 0
    for yy in range(y, y + h):
        for xx in range(x, x + w):
            on_perimeter = yy in (y, y + h - 1) or xx in (x, x + w - 1)
            ch = grid[yy][xx]
            if on_perimeter:
                assert ch in (WALL, DOOR), (kind, xx, yy, ch)
                doors += ch == DOOR
            else:
                assert ch == (FARM if kind == "Farm" else GROUND), (kind, xx, yy, ch)
    assert doors == 1, (kind, doors)
    # the tile outside the door must be Road
    outside = [(dx - 1, dy), (dx + 1, dy), (dx, dy - 1), (dx, dy + 1)]
    outside = [(ox, oy) for (ox, oy) in outside if not (x <= ox < x + w and y <= oy < y + h)]
    assert any(0 <= ox < W and 0 <= oy < H and grid[oy][ox] == ROAD for (ox, oy) in outside), (kind, dx, dy)
    if kind == "Farm":
        assert sum(row[x:x + w].count(FARM) for row in grid[y:y + h]) >= 60

# Every door must be reachable from every other door over walkable tiles.
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
for (kind, x, y, w, h, dx, dy) in buildings:
    assert (dx, dy) in seen, ("unreachable door", kind, dx, dy)

counts = {k: sum(1 for b in buildings if b[0] == k) for k in KIND_ORDER}
assert counts == {"Home": 60, "Farm": 2, "Market": 1, "Bar": 1, "Jail": 1, "Cemetery": 1,
                  "Hall": 1, "Hideout": 2, "Warehouse": 1}, counts

# --- output -------------------------------------------------------------------
out = sys.stdout
for row in grid:
    out.write("".join(row) + "\n")
out.write("\n")
for (kind, x, y, w, h, dx, dy) in buildings:
    out.write(f"B {kind} {x} {y} {w} {h}\n")
