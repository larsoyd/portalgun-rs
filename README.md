# portalgun-rs

A sandbox that recreates Portal's (2007) handheld portal device, written in
Rust on [Bevy](https://bevyengine.org) 0.19.

The gun, the portals and moving through them are read from Portal's own
binaries and files rather than tuned by eye. Where a value could not be found
in the game, it is marked as an estimate instead of being made up. There is a companion cube but it is in alpha at the moment and the physics do not work very well. 

Original behavior is the default. New behavior work starts with decompilation
and Ghidra evidence. Optional improvements stay off until you enable them in
**Esc → Options → Improvements**; incomplete ports remain documented as limitations.

This is a fan project for study. It is not affiliated with or endorsed by
Valve. Nothing from the game is in this repository, only code: to run it you
need your own copy of Portal, which the program reads at startup.

**NB:** The code within both workspace crates, including source, tests, shaders, documentation and supporting scripts are **strictly** non-commercial only. See: [license coverage](LICENSING.md)
and the [required notice](NOTICE).

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
- Walking bobs and sways the gun the way `C_BasePortalCombatWeapon` does: a
  0.45 second up and down cycle and a 0.9 second side to side one, growing with
  speed up to 320, plus the lag that trails the gun behind a turn and the push
  out of the way when you look down. Turn lag stops while the hull touches a
  portal, as in the game. Continuous sway is an optional improvement.

Portal placement now dispatches the original success and supported failure
particle graphs. Attachment glow sprites, pickup beams, holding animations and
the holding sound follow the gun's state, including portal removal. Model lighting
reads the installed normal, Phong exponent and lightwarp textures. The chamber
uses Bevy point lights and uniform ambient lighting in place of Source's per-entity
lights and baked ambient cube. Refraction children remain unsupported.

### The player model

A capsule-and-joint player rig adapted from
[Funny-Bones/ELDEN-RING-Combat-Rewrite](https://github.com/Funny-Bones/ELDEN-RING-Combat-Rewrite)
holds Portal's world gun model. Chell's installed animation data drives the
joints, folding hands, and weapon bone: idle, directional movement, crouching,
jumping, aiming and firing. The animation-state rules were traced in Ghidra;
clip rates, movement distances, blend grids, layers and transition durations
come from the model. Takeoff and landing blend from the previous pose.
The jump clip starts on an actual jump and holds until 0.2 seconds have passed
and you are on the ground, so walking off a ledge keeps the walk or idle clip.
Rising faster than 300 units a second, as out of a floor portal, is Portal's
airwalk state. Chell's model has no airwalk clip, and the game falls back to
its first sequence on the bind pose. This is the default here too; the Options
menu can enable holding the jump clip instead.
Movement and collision still use the existing Portal controller.
A grounded capsule rig dipping below its supporting floor does not create a
portal copy until the collision hull enters beyond the existing trace tolerance.

A portal that turns you about its normal leaves the view rolled. As in the
game, the roll eases back every frame, faster the more you are tipped and twice
as fast on the ground, and mouse look turns the other way while you hang
upside down.

The body and held gun render in both colours' recursive portal views, clipped
into matching halves while crossing an opening. Boots have soles and heels,
and the default feet and gait follow the authored animation. Floor fitting and
matching the walk cycle to footstep sounds are optional improvements. The direct
player body and its held gun cast no shadow by default; their portal copy still does.
The separate first-person gun and arms do not cast world shadows.

Capsule proportions, ground fitting and sound-synchronized gait are adaptations;
full Source foot IK and animation-layer history are not reproduced. See
[animation evidence and limits](docs/player-animation.md).

See [third-party credits](THIRD_PARTY.md) for the upstream revision and MIT
notice. No Elden Ring installation or data is needed.

### The portals

- Portals open on surfaces the gun accepts, fitted to the wall and nudged off
  edges and up off the floor.
- Each face draws the rim and static of Portal's `PortalRefract` stage 2 shader
  with the game's own noise and colour ramps, and opens over half a second.
  Moving one portal turns its partner to static.
- Each linked portal shows the view out of its partner, rendered by its own
  camera with the game's clip plane half a unit behind the exit. At the eye,
  the renderer holds it 0.01 units ahead to keep the projection invertible.
  Views nest two deep, `r_portal_stencil_depth`'s default, and the deepest level shows full
  static or, when the two portals face each other, the game's depth doubler,
  which repeats last frame's screen down the tunnel.
- A view is drawn only when the game would draw it: linked, not full static,
  you in front of it, and part of it on screen, or you within 54 units.
- Walking up to a portal, the part of the near plane inside the opening is
  filled with the view, as Portal's render fix does, so you never see past the
  wall as you step through.

- Persistent particles and edge trails come from the installed PCFs and each
  portal model's own attachments. They follow replacement and deactivation and
  render with separate geometry for each recursive camera. Closing and failure
  graphs run too, with their refraction children explicitly omitted.

See [particle integration and limits](docs/particles/README.md) for the effect
matrix, evidence and tests. TODO: opening/particle refraction and the cheaper
culling the game uses for nested views.

### Going through

- Walk into a linked portal and you come out of the other with your position,
  speed and view carried over. Exit speed is capped at 1000 and a floor exit
  pushes you up at least 300, as `CProp_Portal::TeleportTouchingEntity` does.
  A portal counts as a floor when its normal points up by more than 0.7071.
- The camera follows the eye through an opening independently of the body
  centre, including ceiling exits and crouch transitions.

### Companion Cube

The installed Companion Cube skin can be picked up with **E**, carried through
portals, dropped with **E** or the right mouse button, and thrown with the left
mouse button. Its position, orientation and velocities transform through the
portal pair; rendering clips it into two halves while crossing. Portal shots
pass through physics props, as the original gun's trace filter specifies.

Pickup reach, holding targets, mass limits and throw force follow the original
pickup controller.

The cube moves the way vphysics moves it. Its mass, inertia, damping and hull
come from `metal_box.phy`, its surface from `metalpanel`. Each step follows
IVP's own order: contacts held a quarter unit apart by a normal spring, static
friction that holds a contact until it slips, impacts at the moment a corner
arrives with the game's restitution and friction cone, damping, gravity and
the sleep test that lets a resting cube settle. Thrown cubes spin through their
own inertia.

Near a portal the cube only meets that portal's cut out wall and tube, and the
room on the far side while it is in the hole, as `CPortalSimulator` sets up.
Those pieces take the material of the surface behind the portal. When a portal
moves or closes with the cube still in its hole, the passable space search
pushes it back out. Moving the other portal leaves it alone.

Landings and hits play the cube's own impact sounds, hard or soft by the
surface it meets, and sliding plays its scrape loop, with the thresholds and
volumes of the game's collision and friction callbacks.

Two parts are still host versions rather than ports: several contacts are
solved together by Gauss-Seidel instead of the game's pivoting solver, and the
moment of impact is found by stepping rather than IVP's own search. A cube
that lands across a portal's edge can wedge in the opening more often than in
the original. The original contact-dependent velocity clearing on a normal
drop is not reproduced; releases use its non-contact speed limits.

**R** clears portals and pending shots and triggers the gun's fizzle response.
This is a sandbox shortcut for the game's cleanser behavior.

### Moving

Walking, falling and jumping follow `CGameMovement` and `CPortalGameMovement`
in `server.so`, one 15 ms tick at a time. Rendering interpolates movement between
ticks while mouse look stays immediate, so high refresh rates do not repeat the
same player position between simulation updates:

- On the ground: friction 4 with a stop speed of 100, acceleration 10 toward
  where you are heading, and a walking speed of 150, `hl2_walkspeed`.
- Gravity 600, applied half before and half after each move. Jumping also
  applies the original extra gravity half-step.
- Ctrl crouches with the original hull change, eye-height curve, clearance
  checks and ground movement reduction. Surfaces affect friction, speed and jumping.
- A jump of 160 plus Half-Life 2's single player forward boost. Holding the
  key does not jump again when you land. The complete duck-jump sequence,
  including the automatic airborne hull change, is reconstructed.
- You stand on the ground when it is within 2 units below your feet and you
  are not rising faster than 140. Step movement uses swept hull traces, an
  18-unit step height and Source's 1/32-unit collision margin.
- In the air Portal caps how much you can steer at 60 and accelerates with a
  fixed 15.
- Once you move faster than 300 across the ground, keys that push against
  your motion are ignored. A fling out of a portal keeps its speed whatever
  you press, until you land and friction slows you down.
- Falling onto a floor portal while looking down nudges you toward its
  middle, as `sv_player_funnel_into_portals` does.
- Near a portal you move through that portal's own collision, built the way
  `CPortalSimulator` builds it: the world in front cut off just before the
  plane, the wall behind cut open around the hole, a thin tube lining the
  opening and the far side's world carried through the hole. A portal takes
  you over while its box touches you, you pass through when your centre goes
  behind the plane inside the hole, and if you end up stuck the game's stuck
  nudges and closest-passable-space search move you out.

The 150-unit speed and several complete movement sequences have been measured
in the pinned installed game. See [movement evidence and validation](docs/movement/README.md)
for the audit, recorded references and remaining differences.

### Sound

Every sound Portal's manifest declares is read with its volume, pitch and
waves. The gun's fire sounds come from its weapon script and the portal opening
sounds from the game's sound scripts, each played at a volume and pitch drawn
from the script's range.

Going through a portal plays the enter sound when a box half your size first
meets the opening and the exit sound when a slightly larger one leaves it, both
pitched and turned up with your speed. The fling woosh fades in above 300 and
reaches full volume at 2300.

Footsteps follow `UpdateStepSound`: a step every 300 milliseconds running
and 400 walking, alternating feet, quieter when walking, with the sound of the
surface under you from its `$surfaceprop`. A jump plays a step at full volume,
a hard landing plays one by how fast you fell, and a long fall adds the long
fall boots' `PortalPlayer.FallRecover`. Within one tick the step comes first,
from how you were moving before the move, then the jump, then the landing.

Close and supported invalid-placement sounds use the game's scripts too.

### The crosshair

Portal's quick info crosshair from `CHUDQuickInfo`: the blue bracket on the
left and the orange on the right fill when a shot of that colour would land
where you are aiming, checked ten times a second as the server does, and a
small ring beside each fades in and out to show which colour you fired last.
The centre is the `Q` of Half-Life 2's crosshair font, read from your install.
Sizes, places, colours and fade speeds are the game's.

### The test chamber

A room of 768 by 512 by 384 units built from Portal's own wall, floor and
ceiling materials, with a non-portalable metal strip, a ledge and a cube. You
collide with it as boxes, with each portal's opening cut out of its wall, and
step up onto ledges of up to 18 units.

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
- The shot's projectile stream from `C_PortalBlast` in `client.so`: the
  game's stream particles follow its curved path and fade after it lands.
- The view through portals, its recursion, culling, static and depth doubler,
  and the render fix near a portal, from `client.so`.
- The crosshair from `CHUDQuickInfo` and `CHudCrosshair` in `client.so`, and
  the placement test behind it from `server.so`.
- The gun's bob, lag and look down push from `client.so`, and the footstep,
  jump and landing sounds from `server.so`.
- The view model's animation rates and fades, the sounds' volumes and pitches,
  the materials and the window icon, from the game's files.

Still stand-ins or estimates:

- World geometry remains an AABB chamber. Swept collision supports walls,
  corners, ceilings and steps; slopes, moving platforms, water and ladders
  are not represented.
- Portal collision covers the chamber's own geometry and the cube. The extra
  trace the game runs through a portal when nothing was hit is not reproduced.
- Where a portal fits: the fitting rules are ported but not yet checked against
  `VerifyPortalPlacement`.
- Sound is not positional yet, so the portals' ambient hum is left out and the
  cube's impacts and scrapes play at the same loudness wherever it is.

Deliberate deviations:

- Optional continuous gun sway keeps lag active through portals, avoiding the
  original's abrupt return to the centred pose while touching an opening.
- Movement rendering uses a one-tick interpolation buffer. Teleports reset its
  position endpoints while carrying the gun's facing through the portal. This
  does not reproduce Source's complete saved-eye or prediction history.

- Nested portal views are culled by the whole view instead of the game's
  narrower frustum through the portal. The picture is the same, a nested view
  is only drawn a little more often.
- No occlusion query: a view hidden behind something is still drawn. The game
  skips that query too when its material system runs threaded.

## Controls

| Input | Action |
|---|---|
| Mouse | Look |
| W A S D | Walk |
| Space | Jump |
| Ctrl | Crouch |
| E | Pick up or drop the cube |
| R | Clear portals and pending shots |
| Left mouse button | Fire blue, or throw the held cube |
| Right mouse button | Fire orange, or drop the held cube |
| F11 | Toggle fullscreen |
| Esc | Open or hide the pause menu |

`settings.toml` beside where you run it sets `sensitivity`, `fov`, `msaa`
(1, 2, 4 or 8), `fullscreen` and `vsync` (true or false), one `key = value`
per line. A missing file uses defaults; an unknown key or bad value stops
the program with the line number.
`--settings <file>` reads another file.

Press **Esc → Options** for the Improvements checkboxes. All are unchecked by
default. Changes are staged: **Apply** saves them and keeps Options open, **OK**
saves and closes Options, and **Cancel** discards changes since the last Apply.
**Use original behavior** unchecks all improvements without changing display or
mouse settings. Apply/OK write the selected settings file and take effect in the
current session. A save failure leaves the active settings unchanged.

Opening the menu pauses simulation and releases the cursor. Resume or Esc
returns to play without passing the menu click, jump key or mouse movement into
the game. Esc hides an open Options window while retaining its pending choices
for reopening. Quit asks for confirmation. The menu supports mouse, arrow keys,
Tab and Enter/Space. See [menu behavior and evidence](docs/menu.md).

The existing file keys remain compatible. Each now defaults to `true`; setting
one to `false` explicitly enables its corresponding improvement. Existing files
with explicit choices retain them. Missing keys use original behavior:

| Key | With `true` |
|---|---|
| `original_airwalk` | Rising out of a portal shows the model's bind pose, as the game does for its missing airwalk clip. |
| `original_gun_lag` | The view model stops lagging while your hull touches a portal. |
| `original_body_shadows` | Your body casts no shadow. Its held gun and its copy through a portal still do. |
| `original_feet` | No foot fitting: the feet keep the animation's pose, since the game turns IK off for the player. |
| `original_gait` | The walk cycle plays at the clip's own rate instead of following the footstep sounds. |
| `original_eye_gate` | The camera passes through a portal on the game's hole test, including its one-sided ceiling check. |

The app starts in borderless fullscreen at the desktop's resolution and refresh
rate. `fullscreen = false` starts in a window. VSync defaults to off, using
mailbox presentation when supported; `vsync = true` enables it. Actual frame
rate depends on rendering cost. The terminal reports physical resolution,
monitor refresh rate and frame timings under `[display]`.

`--script <file>` plays timed actions instead of the keyboard and mouse, one
per line as seconds, action and arguments: `fire1`, `fire2`, `use`, `fizzle`, `jump`,
`jump-held 1` / `jump-held 0`, `crouch 1` / `crouch 0`,
`look <yaw> <pitch>` and `move <forward> <side>`. With `--shot-at <seconds>`
and `--quit-after <seconds>` it takes screenshots and exits on its own.

To record a bug, run `cargo run --release -- --log-file portalgun.log` and
include the log with a screenshot. It records startup failures, effective settings,
GPU diagnostics, portal/body decisions, movement and exit status. The file appends
across runs and rotates at about 8 MiB, keeping one `.previous` file. `RUST_LOG`
can override verbosity; `RUST_LOG=info,portalgun=debug` keeps detailed app diagnostics.

## Setup

The program looks for Portal in this order:

1. `--install <dir>`
2. the `PORTAL_DIR` environment variable
3. Steam's default library for the executable's platform:
   - Linux: `~/.local/share/Steam/steamapps/common/Portal`
   - Windows: `%ProgramFiles(x86)%\Steam\steamapps\common\Portal`, normally
     `C:\Program Files (x86)\Steam\steamapps\common\Portal`

For another Steam library, pass `--install` or set `PORTAL_DIR`. A Linux executable
running inside WSL uses Linux paths. To read your Windows install from it, use
`--install "/mnt/c/Program Files (x86)/Steam/steamapps/common/Portal"`.

It checks the files it reads against known hashes. If one differs, it refuses
to start and names the file and both hashes, because the numbers were read
from that exact build of the game.

```
cargo run --release
```

### Build on Arch Linux

Install a current stable [Rust toolchain](https://rustup.rs/) (Rust 1.95 or newer)
and the [Bevy Linux dependencies](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md):

```sh
sudo pacman -S --needed base-devel pkgconf alsa-lib systemd-libs libx11 libxcursor libxrandr libxi wayland libxkbcommon vulkan-icd-loader
cargo build --locked --release -j 4
./target/release/portalgun
```

Your desktop also needs the Vulkan driver for your GPU. Keep an existing Rust
compiler cache configured when building.

### Build Windows from Ubuntu WSL

Install [Ubuntu under WSL](https://learn.microsoft.com/en-us/windows/wsl/install)
with `wsl --install -d Ubuntu` from an administrator PowerShell, then open Ubuntu.
Install Rust with [rustup](https://rustup.rs/) inside Ubuntu and keep the checkout
in its Linux filesystem, for example under `~/Projects`.

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends build-essential pkg-config gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64
rustup update stable
rustup target add x86_64-pc-windows-gnu
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++ \
cargo build --locked --release --target x86_64-pc-windows-gnu -j 4
```

This produces `target/x86_64-pc-windows-gnu/release/portalgun.exe`, a native
Windows executable. Copy it into a Windows folder and run it from PowerShell:

```powershell
.\portalgun.exe --install "C:\Program Files (x86)\Steam\steamapps\common\Portal"
```

The same cross-build commands work on ordinary Ubuntu. To build a Linux
executable in Ubuntu instead, install `libx11-dev libasound2-dev libudev-dev
libwayland-dev libxkbcommon-dev` and run `cargo build --locked --release -j 4`.

The Arch and Ubuntu-to-Windows workflows run on pushes to `main` that change
source, tests, dependencies or their build configuration. README-only edits do
not trigger builds. Cargo downloads and compiled dependencies are cached across
runs. CI builds the release executables only. Run the test suites locally with
your own Portal installation. No game assets or built executables are uploaded
by these workflows.

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
| `crates/portal/` | The engine-free crate: file readers, particles, portal maths, player movement. |
| `src/main.rs` | Startup. Finds and checks the install, then opens the window. |
| `src/args.rs`, `src/settings.rs`, `src/script.rs` | Flags, the settings file and timed scripts. |
| `src/options.rs`, `src/menu.rs` | Staged improvement settings and the Escape menu. |
| `src/units.rs` | Source units to Bevy metres, the only place that converts. |
| `src/chamber.rs`, `src/scene.rs` | The test chamber and its drawing. |
| `src/body.rs`, `src/bodyanim.rs` | The capsule rig, Portal animation clips and held world gun, visible in portals. |
| `src/walker.rs`, `src/player.rs` | Moving through the chamber, mouse look and crossing portals. |
| `src/gun.rs`, `src/firing.rs` | The gun's shots and what happens when they land. |
| `src/portals.rs`, `src/faces.rs`, `src/views.rs`, `src/shaders/` | The portal pair, the drawn faces and the views through them. |
| `src/effects.rs`, `src/impacts.rs`, `src/particle_material.rs` | PCF instances, impact dispatch and particle materials. |
| `src/hud.rs` | The crosshair. |
| `src/viewmodel.rs`, `src/viewanim.rs`, `src/view.rs` | The view model, its animation and its camera. |
| `src/sound.rs`, `src/texture.rs`, `src/icon.rs` | Sounds, textures and the window icon from the install. |
| `src/run.rs` | Timed screenshots and quitting for scripted runs. |

## A note on the data

No Portal binaries, models, textures, sounds, fonts or animation data are
bundled. The program reads them from your original, unmodified installation.
Numerical movement measurements and implementation evidence are documented
for testing. Please keep game assets out of forks too: share the code, and
let each person read the data from the copy of the game they own.

## License

Original work in both crates is under the
[PolyForm Noncommercial License 1.0.0](LICENSE). This is source-available
software with noncommercial use terms. See [license coverage](LICENSING.md)
and the [required notice](NOTICE).

Adapted rig code retains its
[upstream MIT notice](licenses/ELDEN-RING-Combat-Rewrite-MIT.txt). Dependencies
keep their own licenses; see [third-party credits](THIRD_PARTY.md).

Releases contain source only. Registry publishing is disabled for both crates.

The licence covers this project's code only. It grants nothing over Portal or
its files, which remain Valve's.
