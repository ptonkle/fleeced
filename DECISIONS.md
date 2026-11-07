# Decisions

Dated entries. Each quotes the person's request word for word, then says what the agent decided and where it is recorded. Grants of autonomy go here too ("you may publish"), with their scope: this run only, or until withdrawn.

## 2026-11-06 (opening)

> also parallelize as much as possible and frontload work

The agent decided: `BRIEF.md`'s Autonomy approves the plan in advance and names subagents, about five at a time, each on one or two items in a worktree. Work the open questions did not gate started while questions were being asked. Deploys stay on the person's word (FL-Q20).

> oh also there is some new stuff in builder so make sure to get the updates for that

The agent decided: the project starts from Builder at 54ed6df0 (0.3.0), newer than every sibling game. It takes the rollback session (D23), `kits/realtime`, `kits/online` and `kits/saves`, and names subagents as D21 asks. D20's answer chips are not followed for asking: the person's own rule says inline questions, so the 23 questions were asked inline and recorded in `decisions.json` afterwards (FL-8).

> if any of this seems excessive or impossible let me know, I was just spitballing numbers

The agent's reading of the numbers: none is impossible. One broke: a flat 20 s cooldown ends a 7-wolf match in about 40 s at 10 eats. The person chose to scale eats and sheep rather than the cooldown (FL-Q5). The agent's defaults, not the person's: 10 eats per wolf, and 100 sheep plus 50 per extra wolf.

The agent decided alone, recorded here for the person to overturn (the person then answered these as FL-Q24 to FL-Q35, keeping all but the Bell, which became 20 m and a single 10 s ring):

- Rollback (D23) rather than a host-run flock, because 100 sheep then add no network traffic. Measured before it is trusted.
- 60 ticks a second, a map of about 260 m by 140 m, and the sizes in `notes/contract.md`.
- E digs or enters only while the crosshair is on a mound or tunnel mouth; otherwise a Sheepdog's E barks.
- C works as well as Alt for crouch, since Alt can steal focus in a browser.
- A bark has a 3 s cooldown. Lambs are edible and count as one sheep. A Lunge kills a Wolf in a Sheep suit or Invisibility cloak like any other.
- Sheepdogs start near the Fold and Wolves at tunnel mouths, and every Wolf's first eat waits out a full cooldown.
- A player who leaves mid-match is replaced by a bot.
