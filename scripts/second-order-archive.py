#!/usr/bin/env python3
"""Move closed rows (status done) from SECOND-ORDER.md to
notes/second-order-archive.md, so agents read only live rows. Rows keep their
full text and their order; open and person rows stay. Run it when rows close:
at each merge and progress report. `--check` only counts them.

After Ape Race's scripts/second-order-archive.py. The archive keeps the
notebook's table header, so scripts/notebook_check.py and
scripts/journal_check.py read it the same way (an archived number stays taken,
and a journal citation of an archived row still finds it).

    python3 scripts/second-order-archive.py [--check]
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from notebook_check import cells  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
LIVE = ROOT / "SECOND-ORDER.md"
ARCHIVE = ROOT / "notes" / "second-order-archive.md"
INTRO = (
    "# Second-order notebook: archive\n\n"
    "Closed rows (status *done*) moved here from `SECOND-ORDER.md` by "
    "`scripts/second-order-archive.py`, in their original order. Their numbers stay taken.\n\n"
)


def main(argv):
    lines = LIVE.read_text().split("\n")
    header = next(i for i, l in enumerate(lines) if l.startswith("|") and "Status" in cells(l))
    rule = header + 1
    assert lines[rule].startswith("| ---"), "SECOND-ORDER.md: no rule line under the table header"
    status = cells(lines[header]).index("Status")
    keep, moved, inside = lines[: rule + 1], [], True
    for line in lines[rule + 1 :]:
        inside = inside and line.lstrip().startswith("|")
        c = cells(line) if inside else []
        if inside and len(c) > status and c[status].strip("*_ ").lower() == "done":
            moved.append(line)
        else:
            keep.append(line)
    if "--check" in argv:
        print(f"{len(moved)} closed rows to archive")
        return 0
    if moved:
        if not ARCHIVE.exists():
            ARCHIVE.parent.mkdir(exist_ok=True)
            ARCHIVE.write_text(INTRO + lines[header] + "\n" + lines[rule] + "\n")
        with ARCHIVE.open("a") as f:
            f.write("\n".join(moved) + "\n")
        LIVE.write_text("\n".join(keep))
    print(f"archived {len(moved)} rows")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
