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
| `0x283140` | Primary and secondary fire events choose standing or crouched primary-fire gestures. The crouched one needs `FL_DUCKING`, which client movement sets and clears together with `m_bDucked` (raw `0x230416`, `0x230a3c`, `0x231a55`, `0x2267bf`, `0x230942`), so a duck still in progress fires standing. |
| `0x282fb0` | Activities are translated by the active weapon. |
| `0x2830c0` | Ducking/ducked states select crouch idle below speed 0.5, crouch locomotion otherwise. |
| `0x283020` | Moving above speed 0.5 chooses running activity. |
| `0x2506d0` | Horizontal speed is treated as zero at squared speed <= 0.25. |
| `0x255f20` | Movement direction is relative to eye yaw. The nine-way blend normalizes cosine and negative sine by their largest absolute component, then scales by speed/sequence ground speed. |
| `0x250f80` | Maximum ground speed samples the movement blend with its direction normalized to the edge. |
| `0x256460`, `0x256370` | Feet yaw follows movement; stationary body yaw allows 45 degrees before turning. Convergence uses 720 degrees/sec with `clamp(abs(delta)/60, 0.01, 1)`. |
| `0x250670` | Body pitch pose parameter uses negative eye pitch. |
| `0x250530` | The main activity starts as `ACT_MP_STAND_IDLE` and asks jumping, duck, swim, dying and move in that order. |
| `0x250a30` | Event 6 sets jumping and its first frame, stores the jump start time and restarts the main sequence. The server raises it from Portal's `CheckJumpButton` override (server `0x2de350`) only when a jump happens. |
| `0x282dc0` | Upward speed above 300 (`0x7f3280`), or an airwalk already latched, is `ACT_MP_AIRWALK` until the ground. Otherwise a jump holds `ACT_MP_JUMP_START` until the player is on the ground more than 0.2 s (`0x7f2e0c`) after the event. Landing restarts the main sequence and asks for an `ACT_MP_JUMP_LAND` gesture. A fall without a jump keeps the ground activity. |
| `0xa50f40` | The portal weapon's activity table maps these to `*_PRIMARY`. Activity names compare without case (`0x5ed870`), so the model's `ACT_MP_JUMP_START_primary` matches. |
| `0x2536c0` | Main activity changes select a translated sequence and restart its cycle. A sequence changes only when its activity differs. An activity the model lacks returns -1 and falls back to sequence 0. |
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

## Crossing and shadow evidence

Further read-only analysis used client MCP port 8765 and the already analyzed
scratch server project on port 8766. A redundant fresh server analysis was
stopped; the valid existing scratch project supplied the server decompilation.

| Client address | Contract |
|---|---|
| `0x54d060`, `0x563890`, `0x5638b0` | The local direct body is hidden outside portal views and at recursion depth 1 while the camera is transformed through a portal. |
| `0x551ac0`, `0x552190` | Camera crossing sets/resets the transformed-camera state. |
| `0x5562a0`, `0x35c530`, `0x54c690` | Intersecting geometry receives an entrance clip plane and a separately clipped linked copy. Biases are 0.3 and 0.75 Source units; candidate centres below signed distance −5 are rejected. |
| `0x54bc50` | The linked copy transforms the original bone matrices rather than advancing a separate animation. |
| `0x54c4c0` | Local-body/held-weapon ghosts draw in the main view when the camera is not transformed, in the first recursive view when it is, and in deeper recursive views. |
| `0x340080` → `0x362790` → `0x361d00` | Live render origin follows the entity origin; selecting an animation does not subtract a different eye height. |
| `0x21f390`, `0x21f3b0` | First-person viewmodels neither cast shadows nor receive projected textures. |
| `0x54c830`, `0x344e70` | The direct Portal player casts no shadow; portal ghosts inherit the base animating shadow path. |

The Bevy renderer clips actual body, boot and gun fragments in both colour and
depth/shadow passes. It uses transformed bounds of every rendered mesh, including
the held gun, to find portal intersections. This substitutes the capsule meshes'
bounds for the original model-hitbox/simulator intersection. The two copies share
local transforms; the linked root applies the portal transform. Camera layers
implement the complementary direct/ghost visibility rules. Stale planes and the
linked copy are cleared when the body leaves the opening.

Direct player-body shadows are an optional rendering enhancement, off by default.
The held world gun and the portal ghost retain their shadows. The camera-attached gun and arms cast no shadow and receive none.
This avoids the duplicate gun/arm silhouettes without claiming identical Source
shadow rendering.

## Jumps and airwalk

Chell's animation model has `ACT_MP_JUMP_START_primary` but no airwalk, float or
land sequence. A landing gesture therefore plays nothing. Airwalk selects
sequence 0, `head_rot_z`, a one-frame head delta with no activity, on the bind
pose. The game shows that pose while rising faster than 300 out of a portal. By
default the app uses that pose. The Options menu can enable holding the jump
clip instead (`original_airwalk = false`). The initial `head_rot_z` value and
the address of the bind-pose setup are still open. At value 0 the delta is identity.

## Feet, crouch jumps and cadence

The body renders at the interpolated entity origin, the bottom of the collision
hull, as `0x340080` does. An airborne duck lifts the hull bottom by 36 units, and
the body lifts with it, as in the game. Feet probes use the controller's carved
collision brushes: a linked floor opening is not treated as solid ground.

Each foot has a sole and heel. The toe bases are mirrored in the installed model,
so the two boot orientations differ. A two-bone leg solve preserves segment
lengths while fitting a low or planted sole to its support surface when the
optional floor fitting is enabled. It is off by default. This is a
capsule-rendering adaptation, not Source's full IK solver, foot locking, compressed
IK error tracks or terrain-slope alignment. Ground probes are per foot and respect
portal openings.

The movement audio keeps Portal's existing 400/300 ms timer, plus 100 ms while
crouched. Each emitted walking footstep records its side, time and interval. The
optional synchronized gait follows those events. Each directional clip is phase-mapped separately
before grid blending: its contact marker is the descending toe entry into the
three-unit support band after the swing peak. Short contact constraints keep the
sounding foot supported during stance transitions. These synchronization rules
are deliberate adaptations; the installed sequences contain no footstep events.

The earlier extra speed/ground-speed playback multiplier was removed. Helper
`0x250740` exists, but its use by the inspected Portal update path was not
established. `0x2572d0` writes playback 1 in an ordinary local-draw branch;
that branch alone does not prove the cadence of first-person portal views.
The authored movement blend scaling at `0x255f20` remains in use.

The capsule proportions and head remain stylized. Full Source IK, gesture-slot history, arbitrary included-model remapping and
external animation blocks remain outside this implementation. Installed poses and
attachments are decoded; this is not a claim of a complete 1:1 Source renderer.

## Regression coverage

Tests cover actual boot mesh contact, crouched takeoff/landing, both directional
feet against real audio timers, stop/start and stance changes, and first-person
shadow components. Portal tests exercise real scheduled floor-to-wall,
floor-to-floor and floor-to-ceiling traversal at 240/60 Hz and 45 ms frames,
standing/crouched, with 150/700-unit initial fall speeds. They check both original
and linked-copy camera layers in the same frame as movement.

Split-body tests cover complementary planes, original bias boundaries, every body
and held-gun mesh, shared pose transforms, and clearing stale clipping. The GPU
pixel test renders geometry behind the clipped body and checks both colour and
depth clipping in either direction; finite matrices alone cannot prove this.

The seeded movement corpus uses a real 15 ms simulation accumulator with render
steps from 240 Hz through 200 ms hitches. Five scenarios cover wall pairs,
floor/wall, ceiling/floor, floor/ceiling and floor/floor. Assertions include
portal-aware sole/support clearance, chamber containment and valid bone transforms.
A separate corpus seeks random animation sections, grids and cycles. Missing game
data or a failed GPU renderer is a hard failure.

```sh
BODY_FUZZ_SEED=26 BODY_FUZZ_COUNT=1 cargo test -j 2 --test body_fuzz -- --test-threads=1 --nocapture
BODY_FUZZ_COUNT=2048 cargo test -j 2 --test body_fuzz -- --test-threads=1 --nocapture
```

The 2,048-seed run passed with crossings `[497, 506, 597, 419, 610]` across those
five scenarios and 20,369 takeoffs, plus 614,400 clip samples. Finite-transform-only
fuzz checks missed visible defects; these results establish the
covered invariants, not absence of every possible regression.

`tests/fixtures/player-floor-body.script` exercises an open floor entry, reverse
crossing and crouched jump in the rendered game. `player-body.script` exercises
wall views, walking, firing and stance changes. Runtime screenshots remain outside
the repository. The headless app exits successfully; Gamescope still reports a
shutdown crash after the app exits, so wrapper teardown is not a clean pass.

Fourteen targeted mutation checks were caught by assertion failures, including
wrong clip biases, camera gates, shadow participation, gait timing, planted-foot
constraints and disabled GPU colour/depth clipping. No compilation failure or
timeout was counted as a successful detection.
