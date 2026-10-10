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

- The view model, `v_portalgun.mdl`, skinned and animated from the model's own
  sequences, on its own camera at the game's `viewmodel_fov` of 54. It draws
  when you start and settles into its idle, and plays the fire animation on
  every shot.
- Either button fires, then both wait half a second, as
  `CWeaponPortalgun::PrimaryAttack` and `SecondaryAttack` set it. The draw does
  not hold up the first shot, since `Deploy` clears both timers.
- The shot is traced from the eye along the view for 56755.84 units. The stream
  leaves 30 units ahead, 4 right and 5 down of the eye, and the portal opens
  when the shot arrives, at 3000 units a second.
- A shot at a surface marked `%noportal` is refused.

TODO: the glow sprites and beams, the stream and impact particles, the pickup
and hold animations, and the fizzle effects for shots that cannot land.

### The portals

- Portals open on surfaces the gun accepts, fitted to the wall and nudged off
  edges and up off the floor.
- Each face draws the rim and static of Portal's `PortalRefract` stage 2 shader
  with the game's own noise and colour ramps, and opens over half a second.
  Moving one portal turns its partner to static.

TODO: the view through a linked portal with recursion, the refraction while a
portal opens, closing and fizzling.

### Going through

- Walk into a linked portal and you come out of the other with your position,
  speed and view carried over. Exit speed is capped at 1000 and a floor exit
  pushes you up at least 300.

TODO: Portal's own crossing rules and collision behind a portal, falling
through floor portals, the view roll recovery, and objects that follow you.

### Sound

Every sound Portal's manifest declares is read with its volume, pitch and
waves. The gun's fire sounds come from its weapon script, the portal opening
sounds and the enter and exit sounds from the game's sound scripts, each
played at a volume and pitch drawn from the script's range.

TODO: positional sound with each script's sound level, the portals' ambient
loop, close and fizzle sounds.

### The test chamber

A room of 768 by 512 by 384 units built from Portal's own wall, floor and
ceiling materials, with a non-portalable metal strip, a ledge and a cube. Until
Portal's own player movement is ported, you walk it with a simple stand-in:
fixed walking speed, gravity, a jump and box collision.

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

Read from Portal's binaries and files:

- The field of view: `fov_desired` (default 75, range 75 to 120) and
  `viewmodel_fov` (54), both horizontal on a 4:3 screen and widened for wider
  screens the way the client does it.
- Mouse look: `sensitivity` 3 and `m_yaw` and `m_pitch` 0.022 as defaults.
- The gun's refire, draw, muzzle offset, trace range and shot speed, from
  `CWeaponPortalgun` in `server.so`.
- The stream's path from `C_PortalBlast` in `client.so`, ready for the stream
  particles.
- The view model's animation rates and fades, the sounds' volumes and pitches,
  the materials and the window icon, from the game's files.

Still stand-ins or estimates:

- Walking, jumping and collision. Portal's player movement comes next.
- Walking through the wall behind a portal. The game carves its collision, this
  sandbox skips the wall inside the opening.
- Where a portal fits: the fitting rules are ported but not yet checked against
  `VerifyPortalPlacement`.
- Sound is not positional yet.

There are no deliberate deviations from the original so far.

## Controls

| Input | Action |
|---|---|
| Mouse | Look |
| W A S D | Walk |
| Space | Jump |
| Left mouse button | Fire blue |
| Right mouse button | Fire orange |

`settings.toml` beside where you run it sets `sensitivity`, `fov` and `msaa`
(1, 2, 4 or 8), one `key = value` per line. A missing file means Portal's
defaults, an unknown key or bad value stops the program with the line number.
`--settings <file>` reads another file.

`--script <file>` plays timed actions instead of the keyboard and mouse, one
per line as seconds, action and arguments: `fire1`, `fire2`, `jump`,
`look <yaw> <pitch>` and `move <forward> <side>`. With `--shot-at <seconds>`
and `--quit-after <seconds>` it takes screenshots and exits on its own.

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

The window uses Portal's own icon, read from `portal/resource/game-icon.bmp` in
your install. On Linux, Wayland compositors take a window's icon from a desktop
entry rather than from the window, so the program also writes a hidden
`portalgun.desktop` and its icon under `$XDG_DATA_HOME` (by default
`~/.local/share`). Delete both to remove them.

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
| `src/args.rs`, `src/settings.rs`, `src/script.rs` | Flags, the settings file and timed scripts. |
| `src/units.rs` | Source units to Bevy metres, the only place that converts. |
| `src/chamber.rs`, `src/scene.rs` | The test chamber and its drawing. |
| `src/walker.rs`, `src/player.rs` | The stand-in walker, mouse look and crossing portals. |
| `src/gun.rs`, `src/firing.rs` | The gun's shots and what happens when they land. |
| `src/portals.rs`, `src/faces.rs`, `src/shaders/` | The portal pair and the drawn faces. |
| `src/viewmodel.rs`, `src/viewanim.rs`, `src/view.rs` | The view model, its animation and its camera. |
| `src/sound.rs`, `src/texture.rs`, `src/icon.rs` | Sounds, textures and the window icon from the install. |
| `src/run.rs` | Timed screenshots and quitting for scripted runs. |

## A note on the data

Nothing derived from Portal is committed or generated into the repository.
Everything is read from your install each time the program starts. Please keep
it that way in forks: share the code, and let each person read the data from
the copy of the game they own.

## License

The code is under the [MIT License](LICENSE).

The licence covers this project's code only. It grants nothing over Portal or
its files, which remain Valve's.
