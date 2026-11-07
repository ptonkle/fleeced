#!/usr/bin/env python3
"""Turn playtest files into one Markdown table for the design journal.

    python3 tools/playtest_readout.py FILE.json [FILE.json ...]

Each file is what a player's "Send a playtest file" saved (save.rs's kit,
playtest.rs): counts of named events and the tick each milestone was first
reached. One row per file; a column per event and milestone seen in any of
them. It reads only those fields, and refuses a file of any other kind.
"""

import json
import sys
from pathlib import Path


def main(paths):
    rows, events, firsts = [], set(), set()
    for p in paths:
        d = json.loads(Path(p).read_text())
        if d.get("kind") != "playtest":
            print(f"{p}: not a playtest file", file=sys.stderr)
            return 1
        rows.append((Path(p).name, d))
        events |= set(d.get("counts", {}))
        firsts |= set(d.get("firsts", {}))
    events, firsts = sorted(events), sorted(firsts)
    head = ["file", "save version", "sessions", "ticks"] + events + [f"first {m} (tick)" for m in firsts]
    print("| " + " | ".join(head) + " |")
    print("|" + " --- |" * len(head))
    for name, d in rows:
        cells = [name, str(d.get("save_version", "")), str(d.get("sessions", "")), str(d.get("ticks", ""))]
        cells += [str(d.get("counts", {}).get(e, 0)) for e in events]
        cells += [str(d.get("firsts", {}).get(m, "-")) for m in firsts]
        print("| " + " | ".join(cells) + " |")
    return 0


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(2)
    sys.exit(main(sys.argv[1:]))
