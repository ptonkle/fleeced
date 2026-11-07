# Licenses

Every file in this repository that is not code written for it: its source, its author, its terms. The code is under the repository's license.

## Assets

| Path | Source | Terms |
| --- | --- | --- |

### The win tunes

Both melodies are traditional and in the public domain. No lyrics are used. The melody notes in `crates/core/src/tunes.rs` were written out by hand; no source file is in this repository. Scotland the Brave's gracenotes come from a rule in this repository: `grace_march` in that file applies general Great Highland Bagpipe conventions to the plain melody. They are not taken from any score.

| Tune | Status | Read from | Terms of what was read |
| --- | --- | --- | --- |
| Scotland the Brave | Traditional, about 1911. Cliff Hanley's later words are not used. | The melody notes and their lengths, from the pipe score `725_Scotland_the_Brave.bww` at Breizh Partitions (celticscores.com/sheet-music/366_Scotland_the_Brave), marked Traditional. Checked against thesession.org/tunes/4960. Nothing else is kept from it. | The site gives "Unknown license" for the file and says traditional scores are free to download. The melody is traditional. |
| Flowers of the Forest | Traditional. The tune is old (a form of it is in the Skene manuscript, about 1620). Jean Elliot's 1756 words are not used. | The melody line of Jim Paterson's solo arrangement at mfiles.co.uk/scores/flowers-of-the-forest-arranged.htm. The lift on E follows the pipe score `314_The_Flowers_of_the_Forest.jpg` at Breizh Partitions and thesession.org/tunes/2834 setting 31595. The embellishments and the free timing are this project's own. | The melody is traditional; the arrangement around it, not used here, is Music Files Ltd's. |

### Sound numbers

| What | From |
| --- | --- |
| The chanter's pitch and scale in `data/sound.json` | Ewan Macpherson, The Pitch and Scale of the Great Highland Bagpipe, 1998 (publish.uwo.ca/~emacphe3/pipes/acoustics/pipescale.html). Ratios are facts; none of the article's text is copied. |
| The chanter's spectrum in `data/sound.json` | Fitted to measurements of a solo piper, archive.org/details/Piping2232012. The recording was measured only; none of it is in this repository or in the game. |

## Vendored code

| Path | Project and version | Author | License | Checksum |
| --- | --- | --- | --- | --- |
| `web/net/vendor/trystero-nostr-0.25.4.js` | [Trystero](https://github.com/dmotz/trystero) 0.25.4, Nostr strategy, bundled with its dependencies (copied from Chromatic, which took it from Crabble) | Dan Motzenbecker | MIT | `web/net/vendor/SHA256SUMS` |

## Code taken from other repositories

| From | What | License |
| --- | --- | --- |
| sgilson7/chumworld at a644c57c, Sam Gilson | The method of the grass in `crates/core/src/scape/grass.rs` and `data/shaders/grass.*`: baked opaque prisms in distance tiers, sinking by distance, and the ground's own shading. Written anew from `notes/grass.md`. | MIT |

A file with no license, a likeness, a recording or any third-party picture is a worklist row in `SECOND-ORDER.md` for the person to decide, and stays out of git until they do.
