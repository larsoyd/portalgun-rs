# Player rig and Portal animation evidence

The visible primitive rig is adapted from Funny-Bones and contributors; see
[credits and license](../THIRD_PARTY.md). Its joints now follow Portal's
installed `models/player/chell_animations.mdl`, with the world portal gun on
`weapon_bone`. No extracted game data is committed.

## Binary evidence

Read-only Ghidra MCP analysis used `/home/lars/portal-decomp/bin/client.so`,
SHA256 `00b6e45eb84a93f0974c23d3e2656aa9e32e9ada64443f651899a78cde694aae`.
Addresses below are **Ghidra addresses**, raw ELF virtual addresses + `0x10000`.
Names describe vtable and call-graph identifications rather than exported symbols.

| Address | Observation used |
|---|---|
| `0x999f14` | `CPortalPlayerAnimState` vtable, identified through its RTTI at `0x999f00`. |
| `0x283140` | Primary and secondary fire events choose standing or crouched primary-fire gestures. |
| `0x282fb0` | Activities are translated by the active weapon. |
| `0x2830c0` | Ducking/ducked states select crouch idle below speed 0.5, crouch locomotion otherwise. |
| `0x283020` | Moving above speed 0.5 chooses running activity. |
| `0x2506d0` | Horizontal speed is treated as zero at squared speed <= 0.25. |
| `0x255f20` | Movement direction is relative to eye yaw. The nine-way blend normalizes cosine and negative sine by their largest absolute component, then scales by speed/sequence ground speed. |
| `0x250f80` | Maximum ground speed samples the movement blend with its direction normalized to the edge. |
| `0x256460`, `0x256370` | Feet yaw follows movement; stationary body yaw allows 45 degrees before turning. Convergence uses 720 degrees/sec with `clamp(abs(delta)/60, 0.01, 1)`. |
| `0x250670` | Body pitch pose parameter uses negative eye pitch. |
| `0x282dc0` | Jump/airwalk state and landing restart are distinct from crouch locomotion. The >300 upward-velocity airwalk path is identified but not separately reproduced. |
| `0x2536c0` | Main activity changes select a translated sequence and restart its cycle. |
| `0x2f5ef0` | Animation sections use descriptor-relative offsets. The final frame has a separate section at `numframes/sectionframes + 1`. |

The earlier base-player `move_yaw` path at `0x221530` was inspected but is not
the Portal multiplayer animation-state path used here.

## Installed model evidence

Chell's main model includes `models/player/Chell_animations.mdl`. That animation
model has 67 bones, 86 animation tracks and 17 sequences. Relevant sequences:

- `idle_PORTALGUN`, `idleCROUCH_PORTALGUN`: 161 frames at 30 fps, internally
  divided into 30-frame sections. Their automatic layers supply the aim pose.
- `Run_PORTALGUN`: a 3×3 directional grid, 31 frames at 30 fps. Forward root
  movement is about 190 Source units/cycle.
- `Crouch_walk_PORTALGUN`: a 3×3 grid, 26 frames at 30 fps. Forward movement is
  about 54.86 Source units/cycle.
- `portalgun_standing_jump`: 26 frames at 30 fps, non-looping.
- `stand_fire_PORTALGUN`, `crouch_fire_PORTALGUN`: additive firing tracks,
  26 frames at 30 fps, played once per accepted shot.

Transitions read the sequence fade-in duration (0.2 seconds for these clips).
The first frame of a new state retains the previous pose; subsequent frames
blend toward the advancing clip. Sparse additive animations start at zero
translation and identity rotation. Bone 255 terminates an empty track.

`w_portalgun.mdl` has one identity root bone. Its muzzle lies at local
`[0, 0, -21]`. Its vertices are attached directly to the animated player
`weapon_bone`; no estimated grip translation or weapon rotation is used.
The model's material uses the empty entry in its material search directories,
so the loader now searches every declared directory.

## Rendering adaptations and limits

The body layer is visible to both recursive portal-camera chains and chamber
lights. The main and first-person cameras exclude it. The depth doubler copies
the main view, whose portal surfaces already contain the body.

The rig uses interpolated movement and resets its facing on teleport revisions.
Automatic jump-duck raises collision feet by 36 units while preserving the eye.
The visual body origin follows the interpolated eye, with standing/crouched
eye height blended over the same transition as the pose. This also prevents
midair crouch presses and crouched landings from popping the rig by 36 units.
This is a rendering adaptation to this controller's hull representation, not
a claim of identical Source render-origin handling.

A seeded regression also exposed a movement bug: unducking can lower the hull
centre across a floor portal before travel starts. The crossing sweep now saves
the centre before duck processing so this crossing cannot be lost. The reduced
case starts a crouched hull at feet `[-250, 0, -15]` in a floor opening; releasing
crouch must teleport to the linked ceiling instead of falling below the room.

The capsule proportions and head remain a stylized stand-in. Full Source foot
IK, sequence-transition history, all gesture-slot rules, arbitrary included-model
remapping, external animation blocks and a split portal-straddling body are not
implemented. The decoded animation poses and attachment replace the previous
procedural stance; this does not claim a complete 1:1 Source animation renderer.

## Validation

`tests/body.rs` checks installed clip grids and section boundaries, aim and
firing, crouch height, both legs through takeoff and landing, the automatic
jump-duck and midair crouch body-origin continuity, mesh render layers, and relocation on a
teleport revision. `tests/portal_render.rs` checks all recursive cameras include
the body while the main and screen-copy views exclude it.

`tests/fixtures/player-body.script` places perpendicular portals, then crouches,
jumps, fires and walks through a portal for graphical checks. Screenshots are
runtime artifacts outside the repository.

### Seeded regression testing

`tests/body_fuzz.rs` runs 128 seeds by default. Each seed drives 600 movement and
animation steps through wall-to-wall, floor-to-wall or ceiling-to-floor portals,
with randomized jumping, crouching, aim, movement, fire and render time steps
from 1/240 second to 0.2 seconds. Assertions check chamber containment, bounded
stance-height changes, valid camera/bone transforms and plausible bone lengths.
A second test samples 300 random sequence/grid/cycle combinations per seed,
including clamped values outside the authored ranges. Missing game data fails
the tests rather than skipping them.

Replay a seed or run a larger corpus:

```sh
BODY_FUZZ_SEED=26 BODY_FUZZ_COUNT=1 cargo test --test body_fuzz -- --nocapture
BODY_FUZZ_COUNT=2048 cargo test --test body_fuzz -- --nocapture
```

The initial wall-only seed 1 reproduced a 36-unit rig pop when pressing crouch
in midair. Expanded seed 26 reproduced the missed unduck crossing described
above. Both were reproduced before fixes and have focused regression tests in
`tests/body.rs` and `tests/walker.rs`. Bind-pose bone lengths were initially
checked too strictly: valid authored tracks alter translation, so the fuzz
check uses a broad collapse/stretch bound rather than claiming rigid lengths.

The pre-release corpus of 2,048 seeds passed 1,228,800 movement/animation steps
and 614,400 clip samples. It exercised 802 wall-pair crossings, 814 floor/wall
crossings and 1,007 ceiling/floor crossings, with 21,106 takeoffs. These are
regression checks for the covered invariants, not proof of exact Source fidelity.

Four focused mutation checks also failed as expected: choosing the wrong final
animation section, advancing a new clip on its first frame, snapping stance
height, and capturing the portal sweep after duck processing. All four failures
were assertion failures in the relevant tests, not compilation failures.

The v0.8.0 release build completed the scripted portal/crouch/jump/fire/crossing
run with application exit 0. Captures show the rig and held gun in the recursive
views. The headless Gamescope wrapper crashed during shutdown after the app
exited; this is a harness limitation, not a clean wrapper teardown. These checks
do not establish pixel-identical animation or full Source foot placement.
