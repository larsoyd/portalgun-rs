# Player movement reconstruction

The audited starting HEAD was `aa6d848744bbc91656773516f93b202feca7a64d`, with a clean worktree. This work targets the pinned installed **Linux Portal binaries**, not a separately archived 2007 retail build. The running original identifies build 9862575.

## Audit of the previous controller

| File | At the reviewed HEAD |
|---|---|
| `crates/portal/src/movement.rs` | Binary-derived command normalization, friction, acceleration, gravity halves, jump impulse/boost, Portal air cap/funneling/fling guard, velocity limits. Helpers omitted surface multipliers and duck-dependent jump branches. 150-unit player speed remained an inference from HL2 inheritance. |
| `crates/portal/src/surfaces.rs` | Game-data inheritance for sound/material only. Physical friction, speed and jump factors absent. |
| `src/walker.rs` | Partial dry FullWalkMove ordering around the binary-derived helpers. Axis-by-axis endpoint collision, push-out, step-up and top-surface snapping were chamber approximations. No ducking or complete single-player duck-jump state. |
| `src/player.rs` | Fixed interval correct at 0.015 s. Input sampled in Update after fixed simulation; `mem::take(jump)` falsely released held Space on subsequent fixed ticks. |
| `src/chamber.rs` | Custom axis-aligned static geometry, material surface-property lookup and gun ray casts. Not Source BSP collision or VPhysics. |
| `src/view.rs` | Binary-derived weapon bob/lag and look-down offsets. Uses Walker eye, but Walker had a fixed eye height. No complete portal eye interpolation or view-roll recovery. |
| `crates/portal/src/teleport.rs` | Verified velocity transform, floor-exit minimum and speed cap. Touch/crossing geometry is an adaptation; velocity tests do not establish complete portal collision fidelity. |
| `crates/portal/tests/movement.rs` | Useful helper-level binary constant/branch tests. Cannot establish controller sequencing or player state. |
| `tests/fling.rs` | Useful complete fling regressions, but old whole-jump tests only checked initial velocity and held input passed directly to Walker. |
| `tests/walker.rs` | Chamber behavior regressions; exact-contact assertions encoded the old collision convention. No ECS input or crouch coverage. |

The previous implementation did not contain a complete copied Source controller. The SDK and Portal-derived sources supplied names and comparison points, while the numerical helper behavior had binary evidence. HL2 inheritance alone did not establish the Portal player's runtime suit state.

## Implemented stages

1. **Fixed input:** production MovementPlugin separates simulation from audio/rendering; keys are sampled after Bevy InputSystems in PreUpdate. Held Space persists across every fixed tick; a separate pending press survives frames with zero ticks and explicit release/repress events. Scripts have persistent `jump-held` and `crouch` controls.
2. **Crouch and jump:** separate ducked, transitioning, in-duck-jump, button and timer state; standing/crouched hulls 72/36, eye offsets 64/28, half-width16; Source spline and strict .4/.2-second boundaries; airborne origin shift36; standing-hull clearance sweep; blocked recovery; automatic single-player jump crouching; duck-jump eye recovery; forced portal duck and stuck-crouch recovery. Camera, gun aiming, viewmodel and crossing geometry consume the same eye/hull state. Crouched footsteps use the lower run threshold, extra100ms interval and .65 volume multiplier.
3. **Collision:** swept Minkowski AABB traces with time, normal, startsolid/allsolid and 1/32 skin; four-bump multiple-plane sliding; collision-time clipping, ceiling/wall/corner contacts; alternate step path chosen by horizontal progress; swept ground categorization and StayOnGround. Portal velocity formulas remain unchanged. Crossing now sweeps the remaining transformed segment against exit geometry.
4. **Surfaces:** original parser fields, same-name extension and ordered base inheritance; effective player friction `min(physics_friction*1.25,1)` in braking/acceleration, retained Portal air friction including .25 ascent state; max-speed and jump factors.
5. **Reference comparison:** recorded original ProcessMovement outputs and replayed equivalent flat-ground scenarios against Rust at .015 s. No constants were tuned by feel.

## Evidence and reference matrix

Raw ELF virtual addresses are used throughout; Ghidra addresses add0x10000 for the pinned client/server projects. Function names describe vtable/call-graph identifications, not claimed symbol-table exports. See [duck evidence](duck-evidence.md), [collision and engine evidence](collision-evidence.md), and [original runtime/parser evidence](original-reference.md) for addresses, offsets, hashes and method.

[record-original.gdb](record-original.gdb) contains the actual recorder and injected command schedule. It hooks server raw0x2ce200 and samples at return. It changes command inputs, not physics, position, velocity or timers. It requires the original game, an isolated writable game overlay and the local paths recorded in the script. The fixture CSVs are sampled numeric state, not game assets. Keyboard-driven measurements independently established the default binding and crouch timing.

| Scenario | Reference and result |
|---|---|
| Standing jump | Original ticks100..146; first rise2.265, post-tick vz146.5, next-tick hull-origin lift36; physical apex20.145; landing at tick35 (.525s). Every sampled position, velocity and duck/ground flag compared. |
| Acceleration | Original199..209; speeds22.5,39,55.5,72,88.5,105,121.2,136.428,150. Every sampled tick compared. |
| Running jump | Original250: vx225,vz146.5. Subsequent original path changes floor level, so airtime is not used as a level-ground reference. Formula tests cover boost/clamp. |
| Backward jump | Original400: vx-225,vz146.5. Formula tests cover sign and above-cap behavior. |
| Crouch jump | Original499..615; post-jump vz151 (assignment160, rather than addition to first gravity half-step), physical apex21.33; landing on35th jump tick; held duck and release recovery compared every tick. |
| Ground duck/unduck | Original650..780; duck completes after27 elapsed ticks (.405s), ordinary unduck after14 (.210s). State transitions compared every tick. |
| Crouch walking/stopping | Same original sequence; reaches50, then braking44,38,32,26,20,14,8,2,0. Every sampled position/velocity compared. |
| Falling known height | Binary midpoint integration, Rust500-unit start ->473 after20 ticks, vz-180; original initial falling outputs-9,-18,-27 also observed. Not a matched-height original/Rust landing experiment. |
| Walking off platform | Binary trace and step formulas plus chamber regressions. No measured original ledge comparison. |
| Low ceiling / blocked unduck | Binary clearance/clip rules and Rust chamber tests; not measured in original. |
| Angled wall / corners | Binary plane solver, engine skin, Rust wall/corner/normal/thin-wall tests; not measured in original. |
| Floor-wall / wall-floor flings | Preserved transform/minimum/cap tests, chamber end-to-end tests, forced-duck/recovery and same-tick exit obstacle tests. Rust graphical crossings observed; no matched original fling trajectory. |

The four included CSV comparisons check every position/velocity component within 0.003 units and exact ducked/ducking/inDuckJump/ground flags. Fixtures contain 303 recorded command outputs plus four initial states. The tolerance accommodates original float normalization and accumulated rounding; it is not a gameplay tuning parameter.

The speed path is `sv_maxspeed=320`, limited by the actual player's client maximum150,
then by the surface speed factor (and the default unconstrained factor1). Keyboard commands
request450 before vector-length normalization, so diagonals stay capped. `hl2_normspeed=190`
belongs to the equipped-suit branch, which was inactive in the measured Portal player.
Ground crouch crops commands by1/3 after that cap; it does not directly replace existing
velocity. Air crouch does not crop commands.

The related [SDK movement source](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/game/shared/gamemovement.cpp)
and [Portal-derived movement source](https://github.com/SonicEraZoR/Portal-Base/blob/master/sp/src/game/shared/portal/portal_gamemovement.cpp)
were comparison aids. Portal-specific air friction, duck-jump state and constants were checked
against the pinned binaries rather than accepted from a related Source version.

## Test and graphical validation

TDD regressions first reproduced six jumps from held Space at three fixed ticks per frame, tunneling through a one-unit wall, a step beneath an insufficient ceiling, missing duck state/eye changes, absent surface properties and lost same-name surface inheritance. Additional independent corner, clearance, ceiling and exit-geometry cases cover integration.

Real Bevy KeyboardInput messages run through InputPlugin and the production simulation schedule. Held keys, short taps, release/repress and Ctrl are tested with zero/one/multiple fixed ticks; complete traces are identical at30,60 and144 render FPS. Original fling and random-portal containment scenarios remain present. Old exact-contact expectations were corrected for the verified collision skin; one exact18-unit downward boundary now includes the skin and strict StayOnGround endpoint rule. The nearby below-step and above-step scenarios are retained.

Graphical release validation loaded the locally installed models, materials and sounds, captured crouched/standing/portal screenshots, fired both portals and logged four crossings, including floor-to-wall and wall-to-floor. The viewer exited0. The first direct sandbox launch could not reach Wayland; an isolated gamescope display then succeeded. Audio emitted repeated ALSA poll errors, and gamescope crashed during shutdown after the viewer exited. These host issues prevent calling the graphical run wholly clean. Screenshots establish rendered camera/gun/portal state, not subjective motion quality or frame-by-frame equivalence to the original. Local artifacts are under `.claude/play/movement-reconstruction/`.

Final verification on the completed code:

| Check | Result |
|---|---|
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets` | Passed, no warnings |
| `cargo test --workspace` | 409 passed, zero failed |
| `cargo run --release` | Built and ran against the installed assets; final graphical replay exited0 |
| Targeted binary-derived mutations | 9/9 caught; the initial8/9 result exposed missing held-jump-during-unduck coverage, which was added |
| Existing repository ledger | Passed; movement20 green,2 open,1 not applicable |
| Movement ledger | Passed;7 green,2 open,1 not applicable |

`RUSTC_WRAPPER=` was used because the sandbox rejected the configured sccache process.
A final graphical attempt timed out because its limit included compilation; repeating with
the cached build completed all twelve seconds and four crossings. The final replay suppresses
repeated audio-backend log lines; that is not evidence that the audio issue is resolved.
No subjective smoothness or universal2007-build fidelity claim follows from these checks.
The user reports that gamescope normally crashes on exit on this host. A subsequent
`coredumpctl info gamescope` check found no stored coredumps to inspect.

## Remaining scope and discrepancies

- Portal traces still adapt the chamber by carving portal apertures and sweeping after crossing. They do not reproduce CPortalSimulator's complete linked-world collision models before crossing. Tilted openings use a conservative inscribed aperture; moving/closing portals and exact remote-contact timing remain incomplete. A blocked exit conservatively rejects the crossing; this prevents penetration but is not a reconstruction of all linked-world contacts. Existing and new fling regressions guard momentum and common crossings.
- Geometry is static AABB. Slopes, displacement surfaces, moving platforms/basevelocity, water/swimming/water-jumps, ladders, vehicles, noclip, stuck-spawn search, object carrying and multiplayer prediction are outside this chamber's represented systems. No claim of equivalence is made for those branches.
- Ordinary duck eye interpolation is reconstructed, but portal eye interpolation, prediction smoothing, stair-camera smoothing and view-roll recovery remain incomplete. There is no visible full-body player mesh in this viewer; the collision body, eye and gun share state.
- Step/skin boundary behavior is binary-derived, not independently measured on identical original geometry. Surface overrides are binary/parser-derived and tested with synthetic properties; only normal ground has matching gameplay recordings.
- The fixtures validate this pinned Linux build and recorded maps. They do not establish that every shipping2007 platform/build has identical behavior.
