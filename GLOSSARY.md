# Glossary

The source of truth for what each word in this tool means. Strings, tests, the plan and the README use a term in the sense given here. The page shows a definition when a reader hovers the term.

Write each entry as a `### Term` heading, then one or two plain sentences, then an optional `Not:` line listing words that must not be used for it. A number a rule decides is a `{placeholder}` the core fills, so the glossary cannot disagree with the rules. Keep entries in alphabetical order. `crates/core/tests/copy.rs` checks the format, the placeholders and the `Not:` words; `TONE.md` governs the prose.

<!--
An example entry. It sits inside this comment, so it does not count.

### Spout

Where slush comes out. Each line has up to {line.spouts} spouts.

Not: tap, dispenser, nozzle
-->

### Bark

A Sheepdog's call. It scares sheep further and harder than the Sheepdog's presence does.

### Bell

A Sheepdog powerup hung on a sheep, which Sheepdogs alone can see. The first Wolf to come near sets it ringing for everyone, once.

### Bone

A Wolf powerup that plants a decoy mound. The Sheepdog who digs it plays with the bone and can do nothing else until it's over.

### Bot

A computer player in a seat no person holds, either team.

Not: AI player, CPU

### Cairn

A stepped pile of stones. A Sheepdog can climb one to see further, and a mound may sit at its base.

### Carcass

The small pile of bones and a little blood a Wolf leaves after eating a sheep. Sheepdogs count the ones they find.

Not: corpse, body, kill

### Carrot on a stick

A Sheepdog powerup. Sheep follow it while it's held out, until its meter runs out.

### Crouch

Moving low and slow. A crouched Sheepdog doesn't scare sheep, and a crouched Wolf stands as tall as a sheep.

Not: sneak, duck

### Defaults

A lobby checkbox, on at first. While on, Number of sheep, Eats required and Map size follow the players seated, and show ??? until Random roles settle. Off, the host types them.

### Dig

Holding E on a mound to get what's in it.

### Dog mode

A setting that shows the game in the colors a dog sees.

### Dognoculars

A Sheepdog powerup: a zoomed view through a spyglass, until its meter runs out.

### Eat

A Wolf taking the sheep under the crosshair. The Wolf is stuck in place briefly, and the sheep becomes a carcass.

Not: kill, bite, devour

### Eat cooldown

The wait between a Wolf's eats, shown as a countdown dial.

### Eats required

How many sheep the Wolves must eat to win. A match setting.

Not: quota, kill count

### Fold

The stone enclosure at the top of the map where the Sheepdogs herd the sheep. A sheep inside stays inside, and no Wolf can enter.

Not: pen, corral, goal, fank, enclosure

### Fold target

How many sheep must reach the Fold for the Sheepdogs to win.

### Gap

An opening in a stone wall. Sheep can only change fields through a gap.

Not: choke, choke point, gate

### Howl

A Wolf powerup that turns day to night for a while.

### Invisibility cloak

A Wolf powerup that hides the Wolf for a few seconds. Eating ends it at once.

### Invite code

The long code a host sends a player to meet with no room code. The player sends a reply code back.

### Lamb

A young sheep that follows its ewe. It counts as one sheep.

### Lobby

Where players gather before a match. Each picks a role, the host sets the match, and everyone presses Ready.

### Lunge

A Sheepdog's left-click attack: a long committed leap. It kills a Wolf it reaches. After a miss the Sheepdog must wait, and nearby sheep scatter.

Not: bite, strike, kill shot

### Match length

How long a match runs. A match setting.

### Mound

A small heap of dirt at the base of a tree or cairn. Digging it gives a powerup, once, to one player.

Not: pile, dirt pile, dig spot

### Map size

How much hillside a match is played on: Small, Medium or Big. A match setting, and the Fold is the same size on all three.

### Mouth

Either end of a tunnel.

### Number of sheep

How many sheep start the match. A match setting.

### Powerup

A one-off ability dug from a mound. Each team has its own, and a player holds one at a time.

Not: power up, power-up, item, pickup

### Reply code

The long code a player makes from an invite code and sends back to the host.

### Room code

The short code that brings players into one room. The host shares it, or a link that carries it.

### Seat

One place in a match, held by a person or a bot, or empty.

### Sheep

The flock the match is fought over. Sheep are computer-run, and always count one each, lamb or ram.

### Sheep suit

A Wolf powerup: a little sheep costume. Eating ends it at once.

### Sheepdog

A player on the team that herds sheep into the Fold. Modeled on a border collie.

Not: hound, shepherd, herder

### Sniff Snoff

A Sheepdog powerup that shows a rising scent over carcasses, and a vague one near Wolves, for a while.

### Stamina

What sprinting uses up. It refills over time.

Not: energy

### Stampede

A Wolf powerup: a snarl that sends nearby sheep bolting away.

### Sudden death

A match setting. A killed Wolf stays out, and the Sheepdogs win when the last Wolf dies.

### Super appetite

A Wolf powerup that runs the eat cooldown at double speed for a while.

### Super jump

A Sheepdog powerup: one very high jump.

### Tunnel

A Wolf-only passage underground. Each tunnel has two mouths, and a Wolf goes in at one and comes out at the other.

Not: hole, burrow, warren

### Tunnel cooldown

The wait before a Wolf can enter a tunnel again.

### Wolf

A player on the team that eats sheep, hiding among them by crouching.

### Zoomies

A Sheepdog powerup: endless stamina and a faster sprint for a while.
