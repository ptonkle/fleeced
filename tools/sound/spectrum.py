#!/usr/bin/env python3
"""Measure a pipe recording or a render, so the two can be compared by numbers.

    python3 tools/sound/spectrum.py WAV [WAV ...] [--low-a HZ] [--start S] [--seconds S]

For each file it prints the tuning (where the chanter's notes sit, in cents over
low A), the chanter's partials per note in dB against the strongest, the drones'
level against the chanter, the long-term spectrum in thirds of an octave, and
how long the shortest notes (the gracenotes) last. Plain Python, no installs.
"""
import argparse
import cmath
import math
import pathlib
import statistics
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import verify  # noqa: E402


def fft(x):
    """Radix-2, in place on a list of complex numbers."""
    n = len(x)
    j = 0
    for i in range(1, n):
        bit = n >> 1
        while j & bit:
            j ^= bit
            bit >>= 1
        j |= bit
        if i < j:
            x[i], x[j] = x[j], x[i]
    size = 2
    while size <= n:
        w = [cmath.exp(-2j * math.pi * k / size) for k in range(size // 2)]
        for start in range(0, n, size):
            for k in range(size // 2):
                a = x[start + k]
                b = x[start + k + size // 2] * w[k]
                x[start + k] = a + b
                x[start + k + size // 2] = a - b
        size *= 2
    return x


def hann(n):
    return [0.5 - 0.5 * math.cos(2 * math.pi * i / n) for i in range(n)]


def power(signal, at, size, window):
    frame = [complex(a * b) for a, b in zip(signal[at:at + size], window)]
    return [abs(v) ** 2 for v in fft(frame)[:size // 2]]


def db(x):
    return 10 * math.log10(max(x, 1e-14))


def thirds(signal, rate, seconds=None):
    """Long-term average spectrum in third-octave bands, dB against the top."""
    size = 4096
    window = hann(size)
    hop = max(size, (len(signal) - size) // 180)
    total = [0.0] * (size // 2)
    count = 0
    for at in range(0, len(signal) - size, hop):
        p = power(signal, at, size, window)
        total = [a + b for a, b in zip(total, p)]
        count += 1
    bands = []
    center = 100.0
    while center < 16000:
        lo, hi = center / 2 ** (1 / 6), center * 2 ** (1 / 6)
        a, b = int(lo * size / rate), int(hi * size / rate) + 1
        bands.append((center, sum(total[a:b])))
        center *= 2 ** (1 / 3)
    top = max(v for _, v in bands)
    return [(c, db(v / top)) for c, v in bands]


def partials(signal, rate, hz, at, size, window, count):
    """dB of each harmonic of hz in one frame, by the peak near each harmonic."""
    p = power(signal, at, size, window)
    out = []
    for n in range(1, count + 1):
        k = n * hz * size / rate
        if k >= size // 2 - 3:
            break
        c = round(k)
        out.append(sum(p[c - 2:c + 3]))
    return out


def measure(path, low_a, start, seconds):
    signal, rate = verify.read_wav(path)
    if seconds:
        signal = signal[int(start * rate):int((start + seconds) * rate)]
    elif start:
        signal = signal[int(start * rate):]
    print(f"== {path}: {len(signal) / rate:.1f} s, rms {math.sqrt(sum(x * x for x in signal) / len(signal)):.3f}")
    filtered = verify.bandlimit(signal)
    # a slower, longer track for the tuning and the note map
    frames = verify.track(filtered, rate)
    good = [(t, hz) for t, hz, clear in frames if hz and clear > 0.75]
    if not low_a:
        # the fullest bin of the fundamentals between 440 and 510 Hz
        bins = {}
        for _, hz in good:
            if 440 < hz < 510:
                bins[round(hz)] = bins.get(round(hz), 0) + 1
        low_a = max(bins, key=bins.get)
        near = [hz for _, hz in good if abs(hz - low_a) < 6]
        low_a = statistics.median(near)
    print(f"low A: {low_a:.1f} Hz")
    ratios = {"lg": 7 / 8, "la": 1, "b": 9 / 8, "c": 5 / 4, "d": 4 / 3, "e": 3 / 2, "f": 5 / 3, "hg": 7 / 4, "ha": 2}
    names = {k: low_a * v for k, v in ratios.items()}
    groups = {k: [] for k in ratios}
    for t, hz in good:
        n, c = verify.nearest(hz, names)
        if abs(c) < 60:
            groups[n].append((t, hz, c))
    print("note  frames  Hz       cents over low A  (just)")
    for k in ratios:
        if len(groups[k]) > 3:
            m = statistics.median(g[1] for g in groups[k])
            print(f"{k:>3} {len(groups[k]):6d}  {m:7.1f}  {1200 * math.log2(m / low_a):8.1f}  ({1200 * math.log2(ratios[k]):7.1f})")

    # gracenote lengths: runs of one note under 120 ms between longer ones
    runs = []
    for t, hz, clear in frames:
        name = None
        if hz and clear > 0.6:
            n, c = verify.nearest(hz, names)
            if abs(c) < 50:
                name = n
        if runs and runs[-1][0] == name:
            runs[-1][2] = t
        else:
            runs.append([name, t, t])
    short = [(r[2] - r[1] + verify.HOP) * 1000 for r in runs if r[0] and r[2] - r[1] < 0.12]
    if short:
        short.sort()
        print(f"short notes: {len(short)}, median {statistics.median(short):.0f} ms, quartiles {short[len(short) // 4]:.0f} to {short[3 * len(short) // 4]:.0f} ms")

    # partials of held notes, median in dB against each note's strongest
    size = 8192
    window = hann(size)
    print("held note partials, dB against the strongest (1st to 16th):")
    for k in ("la", "c", "e", "ha"):
        spans = []
        run = []
        for t, hz, c in groups[k]:
            if run and t - run[-1] > 0.02:
                if run[-1] - run[0] > 0.2:
                    spans.append((run[0], run[-1]))
                run = []
            run.append(t)
        spans = sorted(spans, key=lambda s: s[0] - s[1])[:12]
        rows = []
        for a, b in spans:
            at = int((a + b) / 2 * rate) - size // 2
            if at < 0 or at + size >= len(signal):
                continue
            hz = statistics.median(g[1] for g in groups[k] if a <= g[0] <= b)
            rows.append(partials(signal, rate, hz, at, size, window, 16))
        if not rows:
            continue
        count = min(len(r) for r in rows)
        mean = [statistics.median(r[n] for r in rows) for n in range(count)]
        top = max(mean)
        print(f"{k:>3} ({len(rows)} notes): " + " ".join(f"{db(v / top):4.0f}" for v in mean))

    print("drones against the chanter (dB, long-term):")
    band = thirds(signal, rate)
    for c, v in band:
        print(f"  {c:7.0f} Hz {v:6.1f} " + "#" * max(0, int(40 + v) // 2))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("wavs", nargs="+")
    ap.add_argument("--low-a", type=float)
    ap.add_argument("--start", type=float, default=0.0)
    ap.add_argument("--seconds", type=float)
    args = ap.parse_args()
    for w in args.wavs:
        measure(w, args.low_a, args.start, args.seconds)


if __name__ == "__main__":
    main()
