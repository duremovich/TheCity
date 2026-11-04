#!/usr/bin/env python3
"""Make a cityview film readable: the mp4, contact sheets, a strip, a paths plot and the motion findings.

Usage: python tools/film.py DIR [--sheet N] [--grid CxR] [--width PX] [--crop [PX]] [--strip N] [--strip-at F]
                                [--no-video] [--k K] [--snap-m M] [--frozen-n N] [--slide-n N] [--rate-factor F]
                                [--stack-m M] [--stack-n N] [--z-jump M] [--yaw-deg D] [--worst N] [--bodies N]

DIR is a film's directory (cityview `film NAME SECS [FPS]` in a --play script, or `--film NAME:SECS[:FPS]`;
docs/TESTING.md "Filming motion"): frame_00001.png .., trace.csv (a row per frame per drawn body near the subject)
and meta.json. Writes into DIR:

  film.mp4      the frames at the film's fps (ffmpeg on PATH): true speed, the clock being a fixed step
  sheet_K.png   N contact sheets (default 1), each a CxR grid (4x3) of evenly spaced frames, labelled with the
                frame, the film time and the sim clock (day, game time, the lerp's tick pair and u); --crop [PX]
                crops each frame to a PX box (default 360) round the subject's screen position from the trace
  strip.png     --strip N: N consecutive frames cropped round the subject in one row (from --strip-at F, default
                the film's middle): frame-to-frame motion in one image
  paths.png     top-down: every traced body's drawn x/y as a polyline, the subject bright with its sim tile at each
                tick as dots, a marker at each finding's frame, a scale bar in metres
  findings.txt  the lines printed

The analysis prints one `FINDING film <check>: ...` line per defect class (counts, worst cases with body and frame
ids) and a per-body table. Every threshold is an argument; these are prints, never asserts. A step's expected length
is the walk at the play speed over a frame: walk_m_s x speed_x / fps (speed_x the frame's clock, x the walk-matched
rate). PIL draws the sheets and the plot (stdlib otherwise).
"""

import argparse
import csv
import json
import math
import os
import shutil
import statistics
import subprocess
import sys
from collections import defaultdict

FONT_PATHS = ["C:/Windows/Fonts/consola.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"]


def font(size):
    from PIL import ImageFont

    for p in FONT_PATHS:
        if os.path.exists(p):
            return ImageFont.truetype(p, size)
    return ImageFont.load_default()


def num(v, default=float("nan")):
    try:
        return float(v)
    except (TypeError, ValueError):
        return default


def load(dir_):
    meta_p = os.path.join(dir_, "meta.json")
    trace_p = os.path.join(dir_, "trace.csv")
    if not os.path.exists(meta_p) or not os.path.exists(trace_p):
        sys.exit(f"film.py: {dir_} has no meta.json / trace.csv (a film's directory?)")
    with open(meta_p, encoding="utf-8") as f:
        meta = json.load(f)
    rows = []
    with open(trace_p, encoding="utf-8", newline="") as f:
        for r in csv.DictReader(f):
            for k in ("frame", "body", "is_subject", "tick_from", "tick_to", "indoors", "aboard", "walking", "lying"):
                r[k] = int(float(r[k]))
            for k in ("film_t", "u", "view_tick", "speed_x", "x", "y", "z", "yaw_deg", "tile_x", "tile_y", "prev_x",
                      "prev_y", "floor", "clip_rate", "lean", "sx", "sy", "wall_s"):
                r[k] = num(r[k])
            rows.append(r)
    frames = sorted(int(n[6:11]) for n in os.listdir(dir_) if n.startswith("frame_") and n.endswith(".png"))
    return meta, rows, frames


def clock(view_tick):
    """A view tick (game minutes) as 'dD HH:MM:SS'."""
    if view_tick != view_tick:
        return "?"
    day, m = divmod(view_tick, 1440.0)
    h, m = divmod(m, 60.0)
    s = (m - int(m)) * 60.0
    return f"d{int(day)} {int(h):02d}:{int(m):02d}:{int(s):02d}"


# ---------------------------------------------------------------------------------------------------------------- video


def video(dir_, meta, frames):
    ff = shutil.which("ffmpeg")
    if not ff:
        print("film.py: no ffmpeg on PATH; film.mp4 skipped")
        return None
    out = os.path.join(dir_, "film.mp4")
    cmd = [ff, "-y", "-loglevel", "error", "-framerate", str(meta.get("fps", 10)), "-start_number", str(frames[0]),
           "-i", os.path.join(dir_, "frame_%05d.png"), "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20",
           "-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2", out]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        print(f"film.py: ffmpeg failed: {r.stderr.strip()[:400]}")
        return None
    return out


# ---------------------------------------------------------------------------------------------------------------- sheets


def frame_info(rows):
    """Per frame: the subject's row (or any row: the clock) by frame index."""
    by = {}
    for r in rows:
        f = r["frame"]
        if r["is_subject"] or f not in by:
            by[f] = r
    return by


def crop_box(r, img, px):
    w, h = img.size
    sx, sy = (r["sx"], r["sy"]) if not off_screen(r, (w, h)) else (w / 2, h / 2)
    half = px / 2
    x0 = min(max(sx - half, 0), max(w - px, 0))
    y0 = min(max(sy - half, 0), max(h - px, 0))
    return (int(x0), int(y0), int(x0 + min(px, w)), int(y0 + min(px, h)))


def off_screen(r, size):
    w, h = size
    return not r or not r["is_subject"] or r["sx"] != r["sx"] or not (0 <= r["sx"] < w and 0 <= r["sy"] < h)


def label(r, f, size):
    if not r:
        return f"#{f}"
    off = f"\nsubject OFF SCREEN at px ({r['sx']:.0f}, {r['sy']:.0f})" if off_screen(r, size) else ""
    return (f"#{f} t {r['film_t']:.2f}s  {clock(r['view_tick'])}\n"
            f"tick {r['tick_from']}->{r['tick_to']} u {r['u']:.2f}  {r['speed_x']:.0f}x{off}")


def tile_of(dir_, f, r, crop, tw, fnt):
    from PIL import Image, ImageDraw

    img = Image.open(os.path.join(dir_, f"frame_{f:05d}.png")).convert("RGB")
    size = img.size
    if crop:
        img = img.crop(crop_box(r, img, crop))
    th = int(img.size[1] * tw / img.size[0])
    img = img.resize((tw, th), Image.LANCZOS)
    d = ImageDraw.Draw(img)
    text = label(r, f, size)
    bb = d.multiline_textbbox((4, 4), text, font=fnt)
    d.rectangle((bb[0] - 3, bb[1] - 3, bb[2] + 3, bb[3] + 3), fill=(0, 0, 0))
    d.multiline_text((4, 4), text, fill=(255, 255, 120), font=fnt)
    return img


def sheets(dir_, rows, frames, n_sheets, grid, width, crop):
    from PIL import Image

    cols, rws = grid
    per = cols * rws
    info = frame_info(rows)
    fnt = font(15 if width / cols >= 380 else 12)
    tw = width // cols
    total = per * n_sheets
    if len(frames) <= total:
        picks = frames
    else:
        picks = [frames[round(i * (len(frames) - 1) / (total - 1))] for i in range(total)]
    out = []
    for k in range(n_sheets):
        chunk = picks[k * per:(k + 1) * per]
        if not chunk:
            break
        tiles = [tile_of(dir_, f, info.get(f), crop, tw, fnt) for f in chunk]
        th = max(t.size[1] for t in tiles)
        sheet = Image.new("RGB", (tw * cols, th * math.ceil(len(tiles) / cols)), (20, 20, 20))
        for i, t in enumerate(tiles):
            sheet.paste(t, ((i % cols) * tw, (i // cols) * th))
        p = os.path.join(dir_, f"sheet_{k + 1}.png")
        sheet.save(p)
        out.append(p)
    return out


def strip(dir_, rows, frames, n, at, crop, width):
    from PIL import Image

    info = frame_info(rows)
    if at is None:
        at = frames[len(frames) // 2] - n // 2
    chunk = [f for f in frames if f >= at][:n]
    if not chunk:
        return None
    tw = max(240, min(width // len(chunk), crop))
    fnt = font(11)
    tiles = [tile_of(dir_, f, info.get(f), crop, tw, fnt) for f in chunk]
    th = max(t.size[1] for t in tiles)
    img = Image.new("RGB", (tw * len(tiles), th), (20, 20, 20))
    for i, t in enumerate(tiles):
        img.paste(t, (i * tw, 0))
    p = os.path.join(dir_, "strip.png")
    img.save(p)
    return p


# -------------------------------------------------------------------------------------------------------------- analysis


def wrap_deg(d):
    return (d + 180.0) % 360.0 - 180.0


def analyse(meta, rows, a):
    fps = float(meta.get("fps", 10))
    walk = float(meta.get("walk_m_s", 1.4))
    human = float(meta.get("human_speed_x", 1.0))
    by_body = defaultdict(list)
    for r in rows:
        by_body[r["body"]].append(r)
    for b in by_body.values():
        b.sort(key=lambda r: r["frame"])

    def expected(r):
        s = r["speed_x"] if r["speed_x"] > 0 else human
        return walk * s / fps

    found = defaultdict(list)  # check -> [(severity, body, frame, text)]
    marks = []  # (check, body, frame, x, y)

    def hit(check, sev, r, text):
        found[check].append((sev, r["body"], r["frame"], text))
        marks.append((check, r["body"], r["frame"], r["x"], r["y"]))

    summary = []
    for body, rs in by_body.items():
        speeds = []
        heading = []  # the recent steps (dx, dy)
        still_run = []  # frames the drawn body stood while the sim moved it
        walk_still = []
        idle_move = []
        mism = []
        for prev, r in zip(rs, rs[1:]):
            if r["frame"] != prev["frame"] + 1:
                heading, still_run, walk_still, idle_move = [], [], [], []
                continue
            dx, dy, dz = r["x"] - prev["x"], r["y"] - prev["y"], r["z"] - prev["z"]
            step = math.hypot(dx, dy)
            e = expected(r)
            v = step * fps
            speeds.append(v)
            sim_moves = (r["tile_x"], r["tile_y"]) != (r["prev_x"], r["prev_y"])
            # teleport: a drawn step far past the walk a frame covers at this speed
            if step > a.k * max(e, 0.05):
                hit("teleport", step / max(e, 1e-6), r, f"body {body} frame {r['frame']}: {step:.1f} m in a frame "
                    f"(walk {e:.2f} m; tile {prev['tile_x']:.0f},{prev['tile_y']:.0f} -> {r['tile_x']:.0f},{r['tile_y']:.0f}, "
                    f"tick {r['tick_from']}->{r['tick_to']} u {prev['u']:.2f}->{r['u']:.2f})")
            # back-snap: a step against the body's own recent heading
            hx, hy = sum(s[0] for s in heading), sum(s[1] for s in heading)
            hl = math.hypot(hx, hy)
            if hl > 0.5 * max(e, 0.02) * len(heading) and heading:
                back = -(dx * hx + dy * hy) / hl
                if back > a.snap_m:
                    hit("back_snap", back, r, f"body {body} frame {r['frame']}: {back:.2f} m back against its heading "
                        f"(tick {r['tick_from']}->{r['tick_to']} u {prev['u']:.2f}->{r['u']:.2f})")
            heading = (heading + [(dx, dy)])[-3:]
            # frozen walker: the sim moves it (its pair's tiles differ) but the drawn body stands
            if sim_moves and step < 0.02 and not r["indoors"] and not r["aboard"]:
                still_run.append(r)
            else:
                if len(still_run) >= a.frozen_n:
                    r0 = still_run[0]
                    hit("frozen", len(still_run), r0, f"body {body} frames {r0['frame']}-{still_run[-1]['frame']}: "
                        f"stood {len(still_run)} frames while the sim walked it ({r0['prev_x']:.0f},{r0['prev_y']:.0f} -> "
                        f"{r0['tile_x']:.0f},{r0['tile_y']:.0f}, u {r0['u']:.2f})")
                still_run = []
            # slide: the walk clip on a still body, the idle on a moving one, the clip's rate off the ground speed
            walk_clip = r["clip"] in ("walk", "walk_pose")
            if walk_clip and v < 0.2 * walk:
                walk_still.append(r)
            else:
                if len(walk_still) >= a.slide_n:
                    r0 = walk_still[0]
                    hit("slide_walk_in_place", len(walk_still), r0, f"body {body} frames {r0['frame']}-"
                        f"{walk_still[-1]['frame']}: the {r0['clip']} clip under a body moving < {0.2 * walk:.2f} m/s")
                walk_still = []
            if r["clip"] in ("idle", "idle_pose") and v > 0.5 * walk:
                idle_move.append(r)
            else:
                if len(idle_move) >= a.slide_n:
                    r0 = idle_move[0]
                    hit("slide_idle_moving", len(idle_move), r0, f"body {body} frames {r0['frame']}-{idle_move[-1]['frame']}:"
                        f" idling while drawn moving (from {v:.1f} m/s)")
                idle_move = []
            if r["clip"] == "walk" and v > 0.2 * walk and r["clip_rate"] > 0:
                ratio = v / (walk * r["clip_rate"])
                if ratio > a.rate_factor or ratio < 1.0 / a.rate_factor:
                    mism.append(ratio)
            # floor pop: a z jump in one frame (a lift or a stair climb moves z a little a frame)
            if abs(dz) > a.z_jump:
                hit("floor_pop", abs(dz), r, f"body {body} frame {r['frame']}: z {prev['z']:.1f} -> {r['z']:.1f} "
                    f"(floor {prev['floor']:.1f} -> {r['floor']:.1f}, indoors {prev['indoors']}->{r['indoors']}, "
                    f"aboard {prev['aboard']}->{r['aboard']})")
            # facing snap
            dyaw = abs(wrap_deg(r["yaw_deg"] - prev["yaw_deg"]))
            if dyaw > a.yaw_deg:
                hit("facing_snap", dyaw, r, f"body {body} frame {r['frame']}: yaw turned {dyaw:.0f} deg in a frame")
        if mism:
            r0 = rs[0]
            found["slide_rate"].append((len(mism), body, r0["frame"], f"body {body}: {len(mism)} walking frames with "
                                        f"ground speed / clip speed out by more than {a.rate_factor:g}x "
                                        f"(median {statistics.median(mism):.1f}x)"))
        exp = [walk * (r["speed_x"] if r["speed_x"] > 0 else human) for r in rs]
        moving = [s for s in speeds if s > 0.05]
        summary.append({
            "body": body, "subject": rs[0]["is_subject"], "frames": len(rs),
            "tier": max(set(r["tier"] for r in rs), key=[r["tier"] for r in rs].count),
            "lod": max(set(r["lod"] for r in rs), key=[r["lod"] for r in rs].count),
            "moving": len(moving) / max(len(speeds), 1),
            "mean": statistics.mean(moving) if moving else 0.0,
            "p95": sorted(moving)[int(0.95 * (len(moving) - 1))] if moving else 0.0,
            "expected": statistics.mean(exp) if exp else 0.0,
        })

    # stack: two bodies within stack_m for stack_n frames running
    by_frame = defaultdict(list)
    for r in rows:
        if not r["indoors"] and not r["aboard"]:
            by_frame[r["frame"]].append(r)
    runs = defaultdict(int)
    best = {}
    for f in sorted(by_frame):
        rs = by_frame[f]
        now = set()
        for i in range(len(rs)):
            for j in range(i + 1, len(rs)):
                p, q = rs[i], rs[j]
                if abs(p["z"] - q["z"]) < 1.0 and math.hypot(p["x"] - q["x"], p["y"] - q["y"]) < a.stack_m:
                    key = (min(p["body"], q["body"]), max(p["body"], q["body"]))
                    now.add(key)
                    runs[key] += 1
                    if runs[key] >= a.stack_n and (key not in best or runs[key] > best[key][0]):
                        best[key] = (runs[key], p)
        for key in list(runs):
            if key not in now:
                runs[key] = 0
    for key, (n, p) in best.items():
        hit("stack", n, p, f"bodies {key[0]} and {key[1]}: within {a.stack_m} m for {n} frames (to frame {p['frame']})")

    # the clock: the view's tick a frame against the play speed's (a stall then a jump reads as a stutter)
    info = frame_info(rows)
    fr = sorted(info)
    rate = float(meta.get("rate_1x_ticks_s", 0.1))
    clock_dev = []
    stalls = 0
    for f0, f1 in zip(fr, fr[1:]):
        r0, r1 = info[f0], info[f1]
        if f1 != f0 + 1:
            continue
        want = r1["speed_x"] * rate / fps
        got = r1["view_tick"] - r0["view_tick"]
        if want > 0:
            clock_dev.append(got / want)
            if got <= 1e-6:
                stalls += 1
    # the subject on the screen (the camera's follow): its biggest jump a frame, px
    subj = [info[f] for f in fr if info[f]["is_subject"] and info[f]["sx"] == info[f]["sx"]]
    sjump = max((math.hypot(b["sx"] - a_["sx"], b["sy"] - a_["sy"]) for a_, b in zip(subj, subj[1:])
                 if b["frame"] == a_["frame"] + 1), default=0.0)
    # bodies appearing or vanishing well inside the trace's ring (a pop; a door is a legit one)
    pops = 0
    ring = float(meta.get("trace_m", 40.0))
    first, last = min(fr) if fr else 0, max(fr) if fr else 0
    subj_at = {r["frame"]: r for r in rows if r["is_subject"]}
    pop_ex = []
    for body, rs in by_body.items():
        for r, edge in ((rs[0], rs[0]["frame"] > first), (rs[-1], rs[-1]["frame"] < last)):
            s = subj_at.get(r["frame"])
            if edge and s and math.hypot(r["x"] - s["x"], r["y"] - s["y"]) < 0.75 * ring:
                pops += 1
                if len(pop_ex) < a.worst:
                    pop_ex.append(f"body {body} {'in' if r is rs[0] else 'out'} at frame {r['frame']} "
                                  f"({math.hypot(r['x'] - s['x'], r['y'] - s['y']):.0f} m from the subject)")
    lines = []
    for check in ("teleport", "back_snap", "frozen", "slide_walk_in_place", "slide_idle_moving", "slide_rate", "stack",
                  "floor_pop", "facing_snap"):
        hs = sorted(found.get(check, []), key=lambda h: -h[0])
        bodies = len(set(h[1] for h in hs))
        worst = "; ".join(h[3] for h in hs[:a.worst])
        lines.append(f"FINDING film {check}: {len(hs)} ({bodies} bodies)" + (f"; worst: {worst}" if worst else ""))
    if clock_dev:
        lines.append(f"FINDING film clock: view tick a frame / the speed's: median {statistics.median(clock_dev):.2f}, "
                     f"min {min(clock_dev):.2f}, max {max(clock_dev):.2f}, stalled frames {stalls} of {len(clock_dev)}")
    else:
        lines.append("FINDING film clock: no frame pair at a running speed")
    lines.append(f"FINDING film pop: {pops} bodies appeared or vanished inside {0.75 * ring:.0f} m of the subject"
                 + (f"; e.g. {'; '.join(pop_ex)}" if pop_ex else ""))
    lines.append(f"FINDING film camera: the subject's biggest screen jump a frame {sjump:.0f} px")
    summary.sort(key=lambda s: (-s["subject"], -s["frames"]))
    table = [f"{'body':>8} {'subj':>4} {'frames':>6} {'tier':>6} {'lod':>8} {'moving':>6} {'mean m/s':>8} "
             f"{'p95 m/s':>8} {'expect':>7}"]
    for s in summary[:a.bodies]:
        table.append(f"{s['body']:>8} {s['subject']:>4} {s['frames']:>6} {s['tier']:>6} {s['lod']:>8} "
                     f"{s['moving']:>6.0%} {s['mean']:>8.2f} {s['p95']:>8.2f} {s['expected']:>7.2f}")
    table.append(f"({len(summary)} bodies traced; 'expect' is walk_m_s x the play speed, a walker's drawn ground speed)")
    return lines, table, marks


# ----------------------------------------------------------------------------------------------------------------- paths


CHECK_RGB = {"teleport": (255, 40, 40), "back_snap": (255, 140, 0), "frozen": (80, 160, 255), "stack": (200, 80, 255),
             "floor_pop": (255, 255, 0), "facing_snap": (0, 230, 200), "slide_walk_in_place": (255, 120, 200),
             "slide_idle_moving": (160, 255, 120)}


def paths(dir_, meta, rows, marks, size=1400):
    from PIL import Image, ImageDraw

    pts = [(r["x"], r["y"]) for r in rows if r["x"] == r["x"]]
    if not pts:
        return None
    x0, x1 = min(p[0] for p in pts), max(p[0] for p in pts)
    y0, y1 = min(p[1] for p in pts), max(p[1] for p in pts)
    span = max(x1 - x0, y1 - y0, 10.0) * 1.08
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    pad = 40
    sc = (size - 2 * pad) / span

    def px(x, y):  # city metres (y north) -> pixels (y down)
        return (pad + (x - cx + span / 2) * sc, pad + (cy + span / 2 - y) * sc)

    img = Image.new("RGB", (size, size + 70), (16, 16, 22))
    d = ImageDraw.Draw(img)
    fnt = font(14)
    tile_m = float(meta.get("tile_m", 4.0))
    # the tile grid, faint (the sim's 4 m squares)
    if span / tile_m < 120:
        gx = math.floor((cx - span / 2) / tile_m) * tile_m
        while gx < cx + span / 2:
            d.line([px(gx, cy - span / 2), px(gx, cy + span / 2)], fill=(30, 30, 38))
            gx += tile_m
        gy = math.floor((cy - span / 2) / tile_m) * tile_m
        while gy < cy + span / 2:
            d.line([px(cx - span / 2, gy), px(cx + span / 2, gy)], fill=(30, 30, 38))
            gy += tile_m
    by_body = defaultdict(list)
    for r in rows:
        by_body[r["body"]].append(r)
    subject = None
    for body, rs in by_body.items():
        rs.sort(key=lambda r: r["frame"])
        if rs[0]["is_subject"]:
            subject = rs
            continue
        seg = []
        for r in rs:
            if seg and r["frame"] != seg[-1]["frame"] + 1:
                if len(seg) > 1:
                    d.line([px(q["x"], q["y"]) for q in seg], fill=(110, 110, 130), width=1)
                seg = []
            seg.append(r)
        if len(seg) > 1:
            d.line([px(q["x"], q["y"]) for q in seg], fill=(110, 110, 130), width=1)
        elif seg:
            x, y = px(seg[0]["x"], seg[0]["y"])
            d.point((x, y), fill=(110, 110, 130))
    if subject:
        d.line([px(r["x"], r["y"]) for r in subject], fill=(0, 255, 120), width=3)
        # the sim's tile at each tick (the pair's newer tile's centre), a dot labelled with the tick
        ox, oy = meta.get("tile_x0_m"), meta.get("tile_y1_m")
        last_to = None
        for r in subject:
            if r["tick_to"] == last_to or ox is None or r["tile_x"] != r["tile_x"]:
                continue
            last_to = r["tick_to"]
            x, y = px(ox + (r["tile_x"] + 0.5) * tile_m, oy - (r["tile_y"] + 0.5) * tile_m)
            d.ellipse((x - 5, y - 5, x + 5, y + 5), outline=(255, 255, 255), width=2)
            d.text((x + 7, y - 7), str(r["tick_to"]), fill=(200, 200, 200), font=font(11))
        x, y = px(subject[0]["x"], subject[0]["y"])
        d.rectangle((x - 6, y - 6, x + 6, y + 6), outline=(0, 255, 120), width=2)
        d.text((x + 8, y + 4), "start", fill=(0, 255, 120), font=fnt)
    for check, body, frame, x, y in marks:
        c = CHECK_RGB.get(check, (255, 255, 255))
        p = px(x, y)
        d.line([(p[0] - 6, p[1] - 6), (p[0] + 6, p[1] + 6)], fill=c, width=2)
        d.line([(p[0] - 6, p[1] + 6), (p[0] + 6, p[1] - 6)], fill=c, width=2)
    # scale bar
    bar = 10 ** math.floor(math.log10(span / 4))
    for m in (5, 2):
        if bar * m <= span / 4:
            bar *= m
            break
    bx, by = pad, size + 20
    d.line([(bx, by), (bx + bar * sc, by)], fill=(255, 255, 255), width=3)
    d.text((bx + bar * sc + 8, by - 8), f"{bar:g} m", fill=(255, 255, 255), font=fnt)
    d.text((bx + bar * sc + 80, by - 8), "subject green (its sim tile each tick: o), other bodies grey; x a finding:",
           fill=(180, 180, 180), font=font(12))
    lx = bx
    for k, c in CHECK_RGB.items():
        d.text((lx, by + 14), f"x {k}", fill=c, font=font(12))
        lx += 12 + d.textlength(f"x {k}", font=font(12))
    p = os.path.join(dir_, "paths.png")
    img.save(p)
    return p


# ------------------------------------------------------------------------------------------------------------------ main


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir")
    ap.add_argument("--sheet", type=int, default=1, help="contact sheets to write (default 1)")
    ap.add_argument("--grid", default="4x3", help="a sheet's columns x rows (default 4x3)")
    ap.add_argument("--width", type=int, default=1600, help="a sheet's width, px (default 1600)")
    ap.add_argument("--crop", type=int, nargs="?", const=360, default=0, help="crop to a PX box round the subject")
    ap.add_argument("--strip", type=int, default=0, help="N consecutive cropped frames in one row (strip.png)")
    ap.add_argument("--strip-at", type=int, default=None, help="the strip's first frame (default: the middle)")
    ap.add_argument("--no-video", action="store_true")
    ap.add_argument("--k", type=float, default=3.0, help="teleport: a step past k x the walk a frame covers (3)")
    ap.add_argument("--snap-m", type=float, default=0.3, help="back-snap: metres back against the heading (0.3)")
    ap.add_argument("--frozen-n", type=int, default=5, help="frozen: frames still while the sim walks it (5)")
    ap.add_argument("--slide-n", type=int, default=5, help="slide: frames of a clip against the motion (5)")
    ap.add_argument("--rate-factor", type=float, default=2.0, help="slide: ground / clip speed off by this (2)")
    ap.add_argument("--stack-m", type=float, default=0.3, help="stack: bodies this near (0.3 m) ...")
    ap.add_argument("--stack-n", type=int, default=5, help="... for this many frames (5)")
    ap.add_argument("--z-jump", type=float, default=1.0, help="floor pop: a z jump past this in a frame (1 m)")
    ap.add_argument("--yaw-deg", type=float, default=90.0, help="facing snap: a turn past this in a frame (90)")
    ap.add_argument("--worst", type=int, default=3, help="worst cases a finding lists (3)")
    ap.add_argument("--bodies", type=int, default=20, help="the per-body table's rows (20)")
    a = ap.parse_args()
    meta, rows, frames = load(a.dir)
    if not frames:
        sys.exit(f"film.py: {a.dir} has no frame_*.png")
    out = []
    if not a.no_video:
        v = video(a.dir, meta, frames)
        if v:
            out.append(v)
    cols, rws = (int(x) for x in a.grid.lower().split("x"))
    lines, table, marks = analyse(meta, rows, a)
    out += sheets(a.dir, rows, frames, a.sheet, (cols, rws), a.width, a.crop)
    if a.strip:
        s = strip(a.dir, rows, frames, a.strip, a.strip_at, a.crop or 360, a.width)
        if s:
            out.append(s)
    p = paths(a.dir, meta, rows, marks)
    if p:
        out.append(p)
    head = (f"film {meta.get('name')}: {len(frames)} frames at {meta.get('fps')} fps ({meta.get('clock')} clock, "
            f"{meta.get('frames_late', 0)} late), speed x {meta.get('speed_x')}, walk {meta.get('walk_m_s')} m/s, "
            f"ticks {meta.get('tick_first')}-{meta.get('tick_last')}, {len(rows)} trace rows")
    text = [head] + lines + [""] + table + [""] + [f"wrote {o}" for o in out]
    with open(os.path.join(a.dir, "findings.txt"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(text) + "\n")
    print("\n".join(text))


if __name__ == "__main__":
    main()
