#!/usr/bin/env python3
"""Check a rendered win tune by analyzing the WAV, since nobody here can hear it.

    python3 tools/sound/verify.py
    python3 tools/sound/verify.py --bww PATH.bww --midi PATH.mid

The pitch track is blind: it reads only the WAV, with an autocorrelation over
the chanter's range, and names each frame by the nearest of the nine pipe notes.
It then compares that with:

- the score the core compiled (every melody note and gracenote, in place), and
- the compiled melody, as a blind transcription in order, or with --bww
  (Scotland the Brave) or --midi (Flowers of the Forest) the original source
  file, parsed here, so a slip in tunes.rs cannot hide.

It also measures each note's pitch in cents against its just ratio, the drones'
pitch and level, and the note lengths. It says nothing about how the pipes sound
to a person.
"""
import argparse
import difflib
import json
import math
import operator
import pathlib
import statistics
import struct
import subprocess
import sys
import wave

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import render  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[2]
HOP = 0.005
WINDOW = 0.016
LOW, HIGH = 380.0, 1100.0
NAMES = ["lg", "la", "b", "c", "d", "e", "f", "hg", "ha"]


def read_wav(path):
    with wave.open(str(path)) as w:
        rate = w.getframerate()
        raw = w.readframes(w.getnframes())
    return [s / 32768 for s in struct.unpack(f"<{len(raw) // 2}h", raw)], rate


def bandlimit(signal):
    """Take the drones out as far as a filter can: the chanter's fundamental
    starts at 416 Hz and the drones' strongest partials sit below that."""
    out = list(signal)
    for _ in range(3):
        render.biquad(out, {"type": "highpass", "freq": [{"v": 360.0}], "q": -3.0, "gain": 0.0})
    render.biquad(out, {"type": "lowpass", "freq": [{"v": 2400.0}], "q": -3.0, "gain": 0.0})
    return out


def track(signal, rate):
    """(t, hz, clarity) per frame."""
    hop = round(HOP * rate)
    size = round(WINDOW * rate)
    first = int(rate / HIGH)
    last = int(rate / LOW) + 1
    frames = []
    mul = operator.mul
    for at in range(0, len(signal) - size - last - 1, hop):
        a = signal[at:at + size]
        energy = sum(map(mul, a, a))
        if energy < 1e-5:
            frames.append((at / rate, 0.0, 0.0))
            continue
        corr = []
        for lag in range(first - 1, last + 2):
            b = signal[at + lag:at + lag + size]
            corr.append(sum(map(mul, a, b)) / math.sqrt(energy * sum(map(mul, b, b)) + 1e-12))
        inner = range(1, len(corr) - 1)
        peak = max(inner, key=corr.__getitem__)
        top = corr[peak]
        # the shortest lag near the top: an octave down correlates as well
        k = next((i for i in inner if corr[i] >= 0.93 * top and corr[i] >= corr[i - 1] and corr[i] >= corr[i + 1]), peak)
        y0, y1, y2 = corr[k - 1], corr[k], corr[k + 1]
        d = y0 - 2 * y1 + y2
        lag = k + first - 1 + (0.5 * (y0 - y2) / d if d else 0.0)
        frames.append((at / rate, rate / lag, y1))
    return frames


def nearest(hz, table):
    best, cents = None, 1e9
    for name, ref in table.items():
        c = 1200 * math.log2(hz / ref)
        if abs(c) < abs(cents):
            best, cents = name, c
    return best, cents


def segments(frames, table, blip):
    """The blind transcription: runs of one note, [name, start, end]."""
    runs = []
    for t, hz, clear in frames:
        name = None
        if hz and clear > 0.6:
            n, c = nearest(hz, table)
            name = n if abs(c) < 45 else None
        if name is None:
            continue
        if runs and runs[-1][0] == name and t - runs[-1][2] < 0.02:
            runs[-1][2] = t
        else:
            runs.append([name, t, t])
    return [r for r in runs if r[2] - r[1] >= blip]


def goertzel_level(signal, rate, hz):
    w = 2 * math.pi * hz / rate
    c = 2 * math.cos(w)
    s1 = s2 = 0.0
    for x in signal:
        s1, s2 = x + c * s1 - s2, s1
    return 2 * math.sqrt(max(0.0, s1 * s1 + s2 * s2 - c * s1 * s2)) / len(signal)


def bww_melody(path):
    """Melody notes of a Bagpipe Music Writer file: (name, beats). A word such
    as Bl_16 is a note; a word such as 'la dots the note before it."""
    out = []
    for word in pathlib.Path(path).read_text(errors="replace").split():
        if word.startswith("'") and out:
            out[-1] = (out[-1][0], out[-1][1] * 1.5)
            continue
        head, _, length = word.partition("_")
        if not length.isdigit():
            continue
        name = head.rstrip("lr").lower()
        name = {"la": "la", "lg": "lg", "b": "b", "c": "c", "d": "d", "e": "e", "f": "f", "hg": "hg", "ha": "ha"}.get(name)
        if name:
            out.append((name, 4 / int(length)))
    return out


def midi_melody(path):
    """Notes of a type 1 MIDI file: (name, beats), by the chanter's written
    pitches (G4 is low G)."""
    d = pathlib.Path(path).read_bytes()
    _, tracks, div = struct.unpack(">HHH", d[8:14])
    p = 14
    by_note = {67: "lg", 69: "la", 71: "b", 73: "c", 74: "d", 76: "e", 78: "f", 79: "hg", 81: "ha"}
    by_note = {k - 12 + 12: v for k, v in by_note.items()}
    notes = []
    for _ in range(tracks):
        size = struct.unpack(">I", d[p + 4:p + 8])[0]
        q, end = p + 8, p + 8 + size
        p = end
        t, run, on = 0, 0, {}

        def varlen():
            nonlocal q
            v = 0
            while True:
                b = d[q]
                q += 1
                v = (v << 7) | (b & 127)
                if b < 128:
                    return v

        while q < end:
            t += varlen()
            b = d[q]
            if b == 0xFF:
                q += 2
                skip = varlen()
                q += skip
                continue
            if b in (0xF0, 0xF7):
                q += 1
                skip = varlen()
                q += skip
                continue
            if b & 0x80:
                run = b
                q += 1
            kind = run & 0xF0
            if kind in (0xC0, 0xD0):
                q += 1
                continue
            key, vel = d[q], d[q + 1]
            q += 2
            if kind == 0x90 and vel > 0:
                on[key] = t
            elif kind in (0x80, 0x90) and key in on:
                notes.append((on.pop(key), t - 0, key))
    notes.sort()
    return [(by_note[k], (e - s) / div) for s, e, k in notes]


def compare(label, heard, written):
    match = difflib.SequenceMatcher(None, heard, written, autojunk=False)
    same = sum(b.size for b in match.get_matching_blocks())
    print(f"  {label}: {same} of {len(written)} source notes heard in order, {len(heard)} heard")
    for tag, a0, a1, b0, b1 in match.get_opcodes():
        if tag != "equal":
            print(f"    {tag}: heard {heard[a0:a1]} / source {written[b0:b1]} (source note {b0})")
    return same == len(written) == len(heard)


def check(path, score, args):
    signal, rate = read_wav(path)
    filtered = bandlimit(signal)
    frames = track(filtered, rate)
    steps = score["steps"]
    end = score["end"]
    table = {}
    for s in steps:
        table[s["note"]] = s["hz"]
    print(f"{path.name}: {len(signal) / rate:.1f} s, {len(steps)} chanter steps")

    def named(a, b):
        got = []
        for t, hz, clear in frames:
            if a <= t and t + WINDOW <= b and hz and clear > 0.6:
                got.append(nearest(hz, table))
        return got

    ok = {False: [0, 0], True: [0, 0]}
    cents = {}
    worst = []
    for s, nxt in zip(steps, steps[1:] + [{"t": end}]):
        a, b = s["t"] + 0.002, nxt["t"] - 0.002
        got = named(a, b)
        grace = s["grace"]
        ok[grace][1] += 1
        if not got:
            worst.append((s["t"], s["note"], "no frame"))
            continue
        names = [g[0] for g in got]
        top = max(set(names), key=names.count)
        if top == s["note"]:
            ok[grace][0] += 1
            if not grace:
                cents.setdefault(s["note"], []).extend(g[1] for g in got if g[0] == s["note"])
        else:
            worst.append((s["t"], s["note"], top))
    print(f"  against the compiled score: melody notes {ok[False][0]} of {ok[False][1]}, gracenotes {ok[True][0]} of {ok[True][1]}")
    for w in worst[:12]:
        print(f"    at {w[0]:.3f} s expected {w[1]}, heard {w[2]}")
    print("  pitch against the just ratio (median cents, spread):")
    for name in NAMES:
        if name in cents:
            v = cents[name]
            print(f"    {name:>2} {table[name]:8.2f} Hz  {statistics.median(v):+5.1f}  +/-{statistics.pstdev(v):.1f}")

    mid = signal[int(rate * 4):int(rate * 8)]
    levels = {hz: goertzel_level(mid, rate, hz) for hz in (119.0, 238.0, 476.0)}
    print("  drone partials at 4-8 s:", ", ".join(f"{k:.0f} Hz {v:.3f}" for k, v in levels.items()))

    # longer than any gracenote and shorter than any melody note; the drones'
    # tail after the last note is not melody
    blip = 1.5 * score["grace"]
    tune_at = steps[1]["t"]
    runs = [r for r in segments([f for f in frames if f[0] < end], table, blip) if r[2] >= tune_at]
    heard = [r[0] for r in runs]
    good = ok[False][0] == ok[False][1] and ok[True][0] == ok[True][1]
    if score["id"].startswith("scotland") and args.bww:
        # the game plays the lift and first line, then the score's last line
        # (24 notes), which closes the tune; the score's lift is the E the
        # chanter strikes in on, so they are one
        written = [n for n, _ in bww_melody(args.bww)]
        return compare("blind transcription against the BWW source", heard, written[:29] + written[-24:]) and good
    if score["id"].startswith("flowers") and args.midi:
        # the game plays the strain's second time through, which closes on D:
        # the file's notes from 35. The pipe scores lift on E, where this piano
        # arrangement lifts on B, so the lift is left out of the comparison.
        written = [n for n, _ in midi_melody(args.midi)]
        return compare("blind transcription against the MIDI source", heard[1:], written[35:35 + len(heard) - 1]) and good
    melody = [s["note"] for s in steps[1:] if not s["grace"]]
    return compare("blind transcription against the compiled melody", heard, melody) and good


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default="notes/audio")
    ap.add_argument("--bww")
    ap.add_argument("--midi")
    args = ap.parse_args()
    run = subprocess.run(
        ["cargo", "run", "--quiet", "-p", "tool_core", "--example", "sound_dump"],
        cwd=ROOT, capture_output=True, text=True,
    )
    if run.returncode != 0:
        sys.exit(run.stderr)
    data = json.loads(run.stdout)
    good = True
    for score in data["scores"]:
        good &= check(ROOT / args.dir / f"{score['id']}.wav", score, args)
    print("PASS" if good else "FAIL")
    sys.exit(0 if good else 1)


if __name__ == "__main__":
    main()
