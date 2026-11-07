# Brief

Stage 1, written by the person. `C1 Frame the problem and set the rules`, `B1 Establish context`.

## The description, word for word

> Alright time for a new game here's the brief:
>
> Name: {still trying to figure this out, suggest some things based on the brief}
>
> Overview: The game is a versus game where one team is sheepdogs and one team is wolves. The goal of the sheepdogs is to herd the sheep to a specific area, and the goal of the wolves is to eat a certain number of sheep. The game should be able to  support up to 8 players in any combination of roles as long as there is at least one member on each team (i.e. 1 sheepdog vs 7 wolves is ok as well as 1 wolf vs 7 dogs).
>
> Inspirations:
>  - Look at the other games we've created for inspiration on the menus, version numbers, notes, online, unit testing strategies, anything that seems generally applicable.
>
> Graphics:
> 3d, simplish polygons. Angular can be good but since this game involves sheep which are soft might want to use more spheres. ALso having the wolf blend in with the sheep is important so take that into account. 3rd person camera from above and behind.
>
> Map:
> Let's just do one map for now, set on a scottish hillside. I can include some images for inspiration, but general motifs should be short green grass, stacked stone walls, sparse, standalone large trees. Players cannot leave stone walled play area. Would be nice to have some grass rendering but it might run into performance issues, see grass rendering in sgilson7's Chumworld for inspiration, but don't go too deep down the rabbit hole. Cairns or other small things for the sheepdog to climb on to try to get a view.
> Generally the maps should need to have one or more choke points to herd the sheep through. Eventually the  goal of the sheep dog is to herd 75% of the sheep into the final pen.
> There should be holes that are underground tunnels that the wolf can go into to pop out at another hole. THis is one of their escape methods. Holes have cooldown on how oftne you can go into them though to preven tplayers from just abusig them and popping between them.
> Let's have random seeded maps, but generally the vague shape and size of the map should stay the same. tunnels, trees, etc are what change most significantly.
>
> Sheep:
> Let's go for 100 sheep by default. Use shetland for the modelling but we might want to add different types later. Vary them in size a bit too, and have lambs as well. They should be able to collide and clip a bit since they are fluffy. Overall we're still going for a bit of a cartoony vibe.
> sheep begin game not super well grouped, maybe some small groups of 5 or less but generally spread out. Sheep ai should keep them reasonably grouped once a dog groups them, they are herd animals after all.
>
> Sheepdog:
> The sheepdog naturally scares away the sheep that are near it. It can bark to scare them away even more effectively. Sheep are not scared away when the sheepdog is crouched. Sheepdog should be basically the same size of the sheep, maybe slightly smaller than the largest of sheep. Model after a border collie.
> Controls:
> Left click to kill targeted wolf instantly. sheepdog cannot target sheep. you do not get an outline like the wolf does when targeting something.
> E: bark
>
> Wolf:
> Wolf is slighly bigger than sheep so they need to crouch to avoid as being seen as larger. Sheep are not scared of the wolf.
> wolf is slightly slower than sheepdog.
> wolf has cooldown on how often it can eat, let's start with 20 seconds. need a ui element to make this clear, a countdown dial
> Wolf cannot access the pen, invisible wall, show message if they try to cross into the pen. Pen location is marked by boucing, floating downward arrow above it
> Controls:
> Left click to eat. there should be a small crosshair to indicate where your screen is pointed and you eat the one on that crosshair. Highlight them with a red outline to help indicate which one you will eat.
> e to enter hole while mousing over it
>
> Common Controls:
> mouse to look around
> shift to sprint (you have a stamina bar that replenish over time)
> space to jump
> alt to crouch. crouched movement is slower
> wolf and sheepdog can push around the sheep physically a little bit but not too much.
> E to dig at highlighted mound for powerup
> Right click: Activate current powerup
> f: discard powerup
>
>
> Information:
> Some information is global but some is onesided in the spirit of the game.
> Dogs can always see outline of other dogs
> wolves and always see outline of other wolves and dogs (I'm atualy not sure they should be able to see outline of all dogs, make it easy to roll this back)
> wolves always see the number of sheep eaten
> When a corpse (pile of bones with small amount of blood) is discovered eaten count can be seen by dogs, increments with each corpse found
> Number of sheep in the pen is global
>
>
> Powerups:
> power ups are unique per role. They are obtained by digging at the base of trees or cairns or similar things. Mousing over the mound that hides the power up should highlight it. mound changes to appear dug up after a player uses it. Power up mound can only be claimed by one player.
> Power ups should be denoted with an icon and text.
>
> Wolf powerups:
> 1. Super appetite: doubles eat cooldown speed for 30 seconds
> 2. invisibility cloak: go invisible for 5 seconds. breaks instantly upon eating.
> 3. Sheep suit: Appear more like a sheep for 30 seconds, i.e. wear a little sheep costume. Breaks instantly upon eating.
> 4. Bone: create a decoy power up mound that occupies the dog after they dig it up for 10 seconds. They are indistinguishable to the dogs. The dog plays with the bone in an unbreakable animation.
> 5. Howl (icon is 3 wolf moon): instantly changes map to night for 1 minute. WOuld be nice if the it was a quick but fluid change, with like the sun disappearing behind the horizon and the moon coming up to replce it, maybe takes 3 second to do that. Visibility during night should be a bit worse
> 6.
>
> Dog powerups:
> 1. carrot on a stick: attract sheep. THis one has like a usage meter so holding right click will bring out the carrot, releasing will put it away, sheep only follow while you're holding it out. 10 seconds of usage
> 2. super jump: single very high jump upon right click, let's start with 5x the height of the sheep.
> 3. Zoomies: infinite stamina and faster sprint speed for 10 seconds
> 4. Dognoculars: get a zoomed in view through a spyglass, default 3x but can zoom up to 10x with mouse wheel up and zoom back out with mouse wheel down. 10 second usage meter.
> 5. Sniff Snoff: temporarily show rising scent aura over carcasses and wolf (keep it vague for the wolf one though, don't want it to be too powerful)
> 6.
>
>
>
>
> Settings:
> Match length (default 8 min)
> number of sheep (default 100)
> number of eats required (default 10) (not sure how well this will scale with player number, let's not overdesign for now though)
> turn off powerups checkbox (default powerups on)
> global settings:
> Dog mode - applies dog colorblindness to the player's game
> sound slider
>
> Sound:
> basic intermittent baaing and shuffling noises
> howl noise when that power is actived
> bark noise when dog barks
> player running and sprinting noises and panting noises
> Basic video game noises for like pick up power up
> noise on wolf kill sheep
> noise on dog kill wolf
> when the dogs win it should be a synthesized bagpipe version of the song Scotland the Brave
> when wolves win it should play the same but Flowers of the Forest ist he song instead
>
>
> Misc:
> Wolves win if they eat the required number of sheep within the time limit
> Dogs win if they herd the required number of sheep into the pen or time runs out.
> Some of the wording in this is rough, establish glossary and ask questiosn about terms before beginning.
> Let's have a random role option in the lobby too.
> Unsure of if killing the wolf should end the game instantly or just put them on like a 30 second death timer. I think death timer might work better, game over instantly is probably too difficult for wolves. Let's have a sudden death lobby setting, default it to off (not sudden death).
> Let's try to recreate the spinning character icons from ape race
>
>
> Open questions:
> HOw do we adjust the number of sheep that need to get to the final pen if the number of sheep and the number of eats is variable?
>
>
> Other ideas for later:
> If there are at least 3 players there should also be a separate team available for a player that wants to play as a sheep and their goal is to appear as similar to the nonplayer sheeps to avoid getting found by the wolf.
> Would be interseting to  have a first person mode too but let's just do 3rd person for now
> Add bloodsplatter to adjacent sheep when the wolf eats
> other ideas for powerups?
>
>
>
> Ask any questions you have up front now.

And, during the first questions:

> oh also there is some new stuff in builder so make sure to get the updates for that

> also parallelize as much as possible and frontload work

> if any of this seems excessive or impossible let me know, I was just spitballing numbers

> since the woles and dogs are less interesting looking it's fine to show them all at the same time in the lobby, just make it obvious which one is the current player's election

> be careful about sheep getting stuck on the edges btw

> also the sheep should wander naturally a bit

## The tool

Fleeced: a 3D online versus game for up to 8 players, dogs against wolves over a flock of sheep on a Scottish hillside. Played in a desktop browser with mouse and keyboard.

## Decisions so far

Answers given in the session, 2026-11-06. `DECISIONS.md` holds the record; this list is the short form every agent reads. The person's numbers were spitballed: every balance number lives in one data table so it can move.

- **Name:** Fleeced (repository `ptonkle/fleeced`, folder `~/slop/fleeced`).
- **Bots:** simple bot dogs and bot wolves in any empty seat, online and in solo practice.
- **Menu and HUD look:** tartan and tweed: dark tartan panels in heather and moss, tweed texture, brass buttons, serif titles.
- **Pen target:** 75% of the starting sheep, rounded up, capped at sheep minus eats required plus 1, fixed at match start.
- **Eat scaling:** the eat cooldown stays a flat 20 s whatever the wolf count. The person: "well the danger of eating is it get you notcied I don't think the right way to balkance this would be to change the eat cooldown, i think it would be to change the number of eats necessary, and sheep in total if posisble". So the lobby's defaults follow the wolf count: Eats required = 10 per wolf; Number of sheep = 100, plus 50 per wolf after the first (1 wolf 100/10, 2 wolves 150/20, 4 wolves 250/40, 7 wolves 400/70). The lobby refills these when teams change until the host edits one by hand. Random roles count as resolved at match start, so that is when the defaults settle.
- **Dog attack:** every left click is a short committed lunge, reach about 2.5 m. A hit kills instantly. A miss means no lunge for 6 s and nearby sheep scatter.
- **Sudden death:** a killed wolf is out for the match and spectates; dogs win when the last wolf dies. With it off, a killed wolf returns after 30 s from a random hole away from dogs.
- **Penned sheep:** crossing the gate is final; a penned sheep ambles to the back and the pen count only rises.
- **Holes:** fixed pairs. Each hole leads to one other, always the same. 30 s cooldown per wolf to start.
- **Inner walls:** dogs and wolves can jump them; sheep use the gaps. The outer wall stops everyone, with an invisible barrier above it.
- **Digging:** hold E for 2 s, interruptible. About 12 mounds at the start and about one more a minute. A player already holding a powerup must use or discard it first. E digs or enters a hole only while the crosshair is on one; otherwise a dog's E barks.
- **Eating and corpses:** the wolf is locked in place for 1 s while eating. Nearby sheep flinch but don't flee. A dog finds a corpse by putting their crosshair on it with a clear line of sight; dogs get no other notice, beyond hearing the sound if close.
- **Words:** the person chose Sheepdogs and Wolves (never Dogs on screen, though the Dog mode setting keeps its name), Fold (not pen), Carcass (not corpse), Tunnel (not hole; each has two mouths), powerup as one word, and Mound. `GLOSSARY.md` holds them. Where this brief's word-for-word description says otherwise, the glossary wins.
- **Sixth Wolf powerup, Stampede:** a snarl that sends sheep within about 15 m bolting away. The Wolves' only way to break up a herd.
- **Sixth Sheepdog powerup, Bell:** hung on a sheep. It triggers once when a Wolf comes within 20 m of that sheep, rings for everyone for about 10 s, and is used up. No eating needed. The person, on reach: "I think it should be way farther, like 20m at least. But only ring for like 10s". The first wording: "alarm bell is a good idea but just call it bell, and instead it just makes the noise for everyone if the wolf is near that sheep, you don't have to eat. it's the dog's trap tool".
- **Bell, who sees it:** only Sheepdogs can see which sheep wears a Bell, so Wolves find out when it rings.
- **Lobby characters:** every seat's character turns at once in the lobby, with the current player's made obvious. The person: "since the woles and dogs are less interesting looking it's fine to show them all at the same time in the lobby, just make it obvious which one is the current player's election".
- **Reference pictures:** the agent pulls its own real photos now; the person may add theirs at any time.
- **Smaller rules, all the person's:** a bark every 3 s. Lambs can be eaten and count as one sheep. A Lunge kills a disguised Wolf like any other. Crouch is Alt or C. Sheepdogs start near the Fold and Wolves at tunnel mouths, and every Wolf's first eat waits out a full cooldown. A player who leaves is replaced by a bot. Sniff Snoff lasts 15 s. A mound gives one of six powerups at equal odds. Wolves can tell a Bone's fake mound and can't dig it.
- **Win tunes:** only the main recognizable parts of each tune, about 20 to 30 seconds. The person: "you don't need to do the whole song for these two just the main recognizable parts".
- **Edges:** the person: "be careful about sheep getting stuck on the edges btw". Sheep steer clear of walls and props, slide along them, and a Sheepdog can always get one out of a corner.
- **Gracenotes:** rewritten by rule from standard pipe conventions, so nothing comes from a score with unclear terms (the person's choice, R1.501).
- **Howl's night:** darker for everyone, with Wolves seeing clearly further than Sheepdogs.
- **Deploy:** local build first, then the agent asks "deploy?", then creates `ptonkle/fleeced` and publishes. Later fixes from the person's play go out unasked.
- **Online (agent's call, D23):** rollback over the kit's WebRTC transport. Every browser runs the same deterministic world, including sheep and bots; only inputs cross the wire. Measured before it is trusted.
- **Sheep look:** B Cloud, one soft fleece, plain face, stubby legs. Picked by the person from four prototypes (2026-11-06).
- **Flock colors:** a same-y flock: about 85% one cream-white coat with slight tone differences, about 15% odd sheep, and the Wolf in the main coat. The person: "make sheep more samesy, that was my inital idea? I feel like you don't often see a bunch of different colors of sheep in the same flock." (2026-11-06).

## Done means

- Two to eight people can play a full match online, from a room code, any mix of dogs and wolves with at least one of each. Empty seats can be bots.
- A person alone can play a full match against bots.
- 100 sheep hold 60 frames a second at 1920×1080 on the person's machine.
- Every feature in the description above is in, or has a row saying why not.

## Decisions that stay with the person

Names, the look of anything named in the brief, balance numbers they have set, and when to deploy a major version.

## Accessibility

Mouse and keyboard on a desktop browser. Team and target information is carried by outline and shape as well as color; the Dog mode setting must keep every HUD signal readable.

## Style

Not Builder's default style pack. World: simple polygons with soft rounded forms, cartoony, short green grass, dry stone walls, lone large trees. UI: tartan and tweed (see Decisions so far).

## References

- **Ape Race** (../ape-race): spinning character icons, menu machinery, online lobby conveniences, What's New, quick check and tags, browser engines setup.
- **Crabble** (../crabble): the version button, patch notes and `scripts/bump.sh`.
- **Honey Hole** (../honey-hole): WebGL page, audio, settings store, invite links.
- **Mandible** (../mandible): the online transport's origin and its testing.
- **Chromatic** (../chromatic): the newest copy of the shared scripts.

## Words

The person's own terms are in the description. `GLOSSARY.md` settles them; see `DECISIONS.md` for the agreed choices.

## Autonomy

The plan is approved in advance: the person said to frontload work and parallelize as much as possible. Where this brief is silent, the agent decides, records the decision in `SECOND-ORDER.md`, and continues. The agent may not loosen a Builder requirement or a boundary test, and may not add a server, an account or analytics.

- Deploys: `on my word`. Word given 2026-11-07: deploy once the todos are done (`CLAUDE.md` FL-12).
- Subagents: as many as the machine bears, about five at once, each in its own worktree on one or two related items. The person: "also parallelize as much as possible and frontload work". Subagents do not run the three-engine browser check; the coordinator does after merging.

## Journal

`default`

## Allowed core dependencies

`serde`, `serde_json` (reference default).
