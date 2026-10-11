# Fixed simulation and rendered movement

The simulation still advances by exactly 0.015 seconds. `Walker` remains the source
for collision, portal entry/exit, movement sounds, gun traces and HUD placement tests.
`VisualHistory` holds the last two completed fixed snapshots; `VisualPlayer` is the
once-per-render sample. Eye position (including duck height), hull centre/extents and
velocity interpolate with `Time<Fixed>::overstep_fraction()`. Mouse and scripted view
angles use the current player angles directly, including across the yaw wrap.

This follows the scheduling in Bevy's official
[physics_in_fixed_timestep example](https://github.com/bevyengine/bevy/blob/main/examples/movement/physics_in_fixed_timestep.rs).
The pinned Bevy 0.19.1 `bevy_app::main_schedule` and `bevy_time::fixed` implementations
were also checked locally: PreUpdate precedes RunFixedMainLoop, and the fixed loop
restores the generic clock to virtual time before AfterFixedMainLoop.

## Scheduling and consumers

- BeforeFixedMainLoop: read keys and mouse, or run due scripted controls. Input is
  consumed before simulation through an explicit movement-plugin schedule set.
- FixedUpdate: consume a pending jump edge once, retain held jump/movement/crouch,
  advance Walker, and update both snapshots. Every catch-up tick updates history.
- AfterFixedMainLoop: sample history, then place the main camera. Frames without fixed
  ticks still update interpolation and mouse look. A player reposition/reset detected
  outside simulation resets history even when no fixed tick runs.
- Update: firing consumes its own button edges once, using current Walker eye/forward.
  It does not wait for a simulation tick or use the visual eye. Script actions remain
  sampled per render frame against Time<Real>, as before; this is not a deterministic
  tick-indexed input replay format.
- PostUpdate, before TransformSystems::Propagate: sway uses visual velocity/angles and
  the accumulated portal rotation. Its bob clock advances each rendered frame. The
  viewmodel camera remains a child of the main camera. `faces::draw_views` reads that
  same main Transform for recursive views, frusta, clip planes, near-plane fixes and
  the doubler. `views::through` already applies a rigid matrix to that pose, so those
  two files need no separate interpolation or simulation-state reads.

Position interpolation has the standard one-tick (15 ms) phase offset relative to
virtual time, or 0–15 ms behind the latest completed authoritative snapshot. It adds
no smoothing spring, prediction correction, mouse-angle delay or delayed gameplay
trace. Eliminating that positional phase offset would require prediction/extrapolation
and different collision/teleport guarantees; this change deliberately does not do that.

## Teleport handling and original-client evidence

A successful teleport resets both endpoints to the final destination snapshot, after
forced duck and exit collision. Its rotation accumulates across fixed ticks until the
viewmodel consumes it, carrying the saved gun facing through each crossing without
restarting bob. External player resets separately rebase the gun facing and bob clock.
No interpolation segment connects the two rooms. Rejected/obstructed teleports remain
ordinary source-side motion.

The rendered eye is transformed through the linked portal while it lies behind the
opening and the hull straddles it. A ceiling exit also checks the feet-to-eye segment,
so unducking cannot leave the eye above the ceiling after the shorter hull clears it.
The main view, attached gun camera and recursive portal views share this corrected pose.

Ghidra MCP was accessible with `/home/lars/portal-decomp/bin/client.so` loaded.
SHA-256: `00b6e45eb84a93f0974c23d3e2656aa9e32e9ada64443f651899a78cde694aae`.
Addresses below are **Ghidra addresses** (subtract 0x10000 for raw ELF virtual
addresses in this project's convention). Function names are call-graph identifications,
not exported symbols. Decompiler output was inspected through the live bridge.

| Address | Observed behavior |
|---|---|
| `0x556e90` | Registers `EntityPortalled` with callback `0x554850`. |
| `0x554850` | Reads the portal matrix and entity interpolators. Compares current versus transformed positions to decide whether the newest sample is already transformed. Calls angle and position history transforms. The player path adjusts the position matrix around a 36-unit hull-centre offset before transforming history, then calls `0x551270`. |
| `0x28c3f0` | Walks timestamped position-history entries, optionally skipping the newest; applies the supplied affine matrix to samples older than client time, then re-interpolates. |
| `0x28c2b0` | Corresponding angle-history iteration and angle-transform call. |
| `0x551270` | Stores a pending teleport flag and portal matrix on the player. |
| `0x54d910` | Consumes the pending teleport, transforms two saved eye-position vectors, transforms/normalizes eye angles and resets their interpolator. Rotates each viewmodel's saved facing vector at offset `0x854`. |
| `0x5516e0` | Updates the client's saved eye positions and smooths their difference; includes a portal transformation path. |
| `0x552190`, `0x551ac0` | CalcView handles pending teleport before calculating the view. CalcPortalView checks eye distance behind the portal plane and portal-hole membership, with a ceiling/unduck special case, then transforms the eye and angles through the portal. |
| `0x28b200`, `0x28a950`, `0x28a870` | The viewmodel lag gate tests the player's world AABB against two triangles covering the linked, active portal surface, with zero tolerance. It does not use the 64-unit portal trigger volume. |
| `0x288a30`, `0x2debb0` | Builds the portal triangles from the origin, right/up vectors and 32/54 half extents, then tests triangle/AABB intersection. |

Intentional differences: Rust resets position history instead of carrying
Source's history and saved eye offsets through the portal matrix.
The chamber controller's forced duck and collision-adjusted destination do not always
form a rigid transform of the prior eye. Rust has no network prediction/interpolation
queue or demo interpolation. The eye correction uses the chamber's rectangular overlap
helper and a symmetric ceiling-opening test; Source checks its simulator's deeper hole
shape and has a one-sided ceiling check. Saved-eye recovery, portal force-no-draw state
and roll recovery are not reconstructed here. This does not claim exact visual parity
with the original client for every tilted-portal transition.

Gun turn lag remains active through portal overlaps. Source disables it while the
player intersects the portal surface. Reproducing that switch visibly snaps a swaying
gun to its centred pose on entry, even if hidden facing is reset to prevent an exit
jolt. The app deliberately removes that switch: the same lag integration runs before,
during and after a crossing, and the saved facing rotates through the portal. The
engine-free lag function retains the original zero-limit behavior for reference tests.

## Floor-portal entry

The old `carve` reduced every in-plane-rotated portal to an inscribed square. At 20°,
a player jumping over a point 25 units along the portal's long axis landed on the
floor instead of entering, although the whole hull fit inside the opening.

The chamber now subtracts the actual right/up rectangle as convex half-spaces and
sweeps the hull against the remaining solids. Collision skin, gravity, jump impulse,
fixed interval and exit-velocity rules are unchanged. Regressions cover entries at
0°, 20°, 45°, 70° and 90°, plus hulls overlapping the rim that must remain supported.
This remains a static chamber collision model, not Source's BSP/VPhysics portal simulator.

## Validation

Automated tests cover fractions 0/0.5/1, stationary/spawn/reset state, zero and multiple
fixed ticks, a teleport in an earlier catch-up tick, immediate wrapped angles, mouse
and scripted input ordering, held/quick jump presses, firing on a zero-tick frame from
the authoritative eye, viewmodel parenting, recursive portal transforms, accumulated
teleport rotations, ceiling/unduck eye correction, and continuous sway during entry,
crossing and exit.
At simulated 60/120/180 Hz, steady 150-unit/s motion advances by 150/Hz every rendered
frame while the timestep remains 15 ms. Existing recorded-original trajectory checks
and controller/fling/fuzz regressions are retained.

The initial interpolation validation ran **422 tests, 0 failed** with
`RUSTC_WRAPPER= cargo test --offline --workspace`.
Formatting and whitespace checks passed.
`RUSTC_WRAPPER= cargo clippy --offline --workspace --all-targets -j 2 -- -D warnings` passed.

Graphical smoke runs used the release application, Vulkan on an RTX 4070, 1280×720,
MSAA 4, and the existing 179.92 Hz DP-1 display. MangoHud capped rendering at the
requested rates; the desktop refresh mode was not changed. Each run lasted 15 seconds
with 10 seconds of local frame-time logging, using
[`smooth-motion.script`](../../tests/fixtures/smooth-motion.script). Measurements were
repeated after compilation finished to remove build-time CPU/memory contention.

| Render cap | Average FPS | Average frame time | 1% low FPS |
|---|---:|---:|---:|
| 60 | 59.9 | 16.7 ms | 56.5 |
| 120 | 119.4 | 8.4 ms | 108.6 |
| 180 | 178.5 | 5.6 ms | 154.2 |

All three runs exited successfully and exercised repeated floor/wall portal crossings;
the 180 run logged seven crossings. Captured rendered frames were inspected. These
short runs include occasional long frames (0.1% low about 15–19 FPS) and are not proof
of hitch-free rendering or a frame-by-frame perceptual assessment of every teleport.
The deterministic tests establish removal of the 66.67 Hz positional staircase.
The pre-existing missing `v_hands` texture warning remains.

The ceiling regression uses [`ceiling-loop.script`](../../tests/fixtures/ceiling-loop.script).
At 3440×1440, the original six-second capture contained a clear-grey band covering
about 22% of the frame. After the eye correction, all nine sampled captures across
repeated crossings rendered without that band; the application exited successfully.
Logs and captures are in `/tmp/portal-vertical/{before,after}`. Unit regressions separately
verify immediate turn response beyond the hull, continuous back-and-forth shaking
through the opening at 60/180 Hz, and preservation of an in-progress turn through
the portal rotation. A denser run saved
17 frames around the first crossing, with no clear-grey band; requests falling within
the same rendered frame were coalesced by the screenshot system. The complete suite
now passes 433 tests, plus Clippy with warnings denied, formatting and the release build.
The final [sway scenario](../../tests/fixtures/portal-sway.script) repeats alternating
mouse turns during ceiling loops. Its 3440×1440 release run exited successfully, with
nine sampled captures free of the reproduced clear-grey band. Artifacts are in
`/tmp/portal-release/graphics`; the entry-snap fix is also pinned by the sway regressions.

Local run logs, CSVs and captures are in `/tmp/portal-smooth-{60,120,180}`; use the
`verified.log` files and newest CSVs. The earlier 60/120 CSVs were contaminated by
parallel compilation and are excluded from this table. Example reproduction:

```sh
mkdir -p /tmp/portal-smooth-180 /tmp/portal-smooth-data
env XDG_DATA_HOME=/tmp/portal-smooth-data MANGOHUD=1 \
  MANGOHUD_CONFIG=fps_limit=180,autostart_log=1,log_duration=10,log_interval=5,output_folder=/tmp/portal-smooth-180 \
  target/release/portalgun --script tests/fixtures/smooth-motion.script --quit-after 15
```
