# portalgun-rs

A sandbox that recreates Portal's (2007) handheld portal device, written in
Rust on [Bevy](https://bevyengine.org) 0.19.

The gun, the portals and moving through them are read from Portal's own
binaries and files rather than tuned by eye. Where a value could not be found
in the game, it is marked as an estimate instead of being made up.

This is a fan project for study. It is not affiliated with or endorsed by
Valve. Nothing from the game is in this repository, only code: to run it you
need your own copy of Portal, which the program reads at startup.

## What is in it

### The gun

TODO: the view model with its draw, idle, fire and pickup animations, the
glow sprites and beams, the fire rate, and what happens when a shot cannot
land.

### The portals

TODO: placement on portalable surfaces, the faces, the view through them with
recursion, the edge and the static, the open and close animations, and
fizzling.

### Going through

TODO: crossing a portal with your position, speed and view carried over,
floor portals, falling through, and objects that follow you.

### Sound

TODO: the gun's sounds, each portal's open, close and ambient loop, and the
enter and exit sounds, all read from Portal's sound scripts with their volume
and pitch.

### The test chamber

TODO: the greybox room with portalable and non-portalable panels, a ledge and
cubes. Until Portal's own player movement is ported, you walk it with a simple
stand-in.

### The portal crate

The `portal` crate holds everything that does not need an engine. It has no
dependencies and does not use Bevy.

**It reads Portal's files:**

- VPK archives
- MDL models with their vertex and strip files. Animation tracks are decoded
  the way Source decodes them and come out as bone poses.
- VTF textures, converted to DDS, sprite sheets included
- KeyValues text and sound scripts
- binary DMX particle files (`.pcf`)

**It runs the particles the gun and the portals use:** every initializer,
emitter, operator, force and constraint Portal's particle files ask for, with
child systems, sub-stepped simulation, sprite, trail and rope renderers, and
SpriteCard sheets. The random numbers come from the game's own
4096-entry table, read in the order the game reads it.

**It draws the portal surface** the way Portal's shader does: the view
through the hole, the edge, and the static you see before the other end
exists.

**It runs the gun's glow sprites** and the fade between the gun's states.

**It does the portal maths:**

- the transform from one portal to its partner
- rays that pass through a pair
- fitting a portal onto a wall, including nudging a shot off an edge and
  snapping it down to the floor
- which side of a portal you are on, and how fast you come out of the other one

Everything is in Portal's own units (Source units, z up). The app converts to
Bevy's metres in one place, `src/units.rs`.

## How faithful it is

TODO: every behaviour read from Portal's client.so and server.so, listed with
the game's own symbol. Then what is still estimated, and each deliberate
deviation from the original with the setting that turns it back.

## Controls

TODO: move, look, jump, fire blue, fire orange, and the settings file.

## Setup

The program looks for Portal in this order:

1. `--install <dir>`
2. the `PORTAL_DIR` environment variable
3. Steam's default library, `~/.local/share/Steam/steamapps/common/Portal`

It checks the files it reads against known hashes. If one differs, it refuses
to start and names the file and both hashes, because the numbers were read
from that exact build of the game.

```
cargo run --release
```

The tests:

```
cargo test --workspace
```

To see what the crate pulls out of your install:

```
cargo run -p portal --example portal_inspect -- "/path/to/Steam/steamapps/common/Portal"
```

## Layout

| Path | What it is |
|---|---|
| `crates/portal/` | The engine-free crate: file readers, particles, portal maths. |
| `src/main.rs` | Startup. Finds and checks the install, then opens the window. |
| `src/args.rs` | Command-line flags. |
| `src/units.rs` | Source units to Bevy metres, the only place that converts. |
| `src/run.rs` | Timed screenshots and quitting for scripted runs. |

TODO: the chamber, the walker, the gun, the portals and the sound modules as
they land.

## A note on the data

Nothing derived from Portal is committed or generated into the repository.
Everything is read from your install each time the program starts. Please keep
it that way in forks: share the code, and let each person read the data from
the copy of the game they own.

## License

MIT or Apache-2.0, at your option.

TODO: the licence files.

The licence covers this project's code only. It grants nothing over Portal or
its files, which remain Valve's.
