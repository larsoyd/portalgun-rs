# Portal particles in the application

The application reads `particles/portals.pcf`, `particles/portalgun.pcf`, and
`particles/portal_projectile.pcf` through the installation's existing search path.
All 67 definitions pass the reconstructed runtime's function checks. Child indices
are rebased when the files are combined. No game data is embedded in this repository.

`EffectsPlugin` runs the existing `portal::particles::system::System` and submits
its existing sprite, trail and rope geometry. `PortalImpactsPlugin` separately
connects finite effects and close sounds. This separation allows persistent
rendering and impact dispatch to be tested independently.

## Implemented effects

| Effects (both colours) | Dispatch | Rendering |
|---|---|---|
| `portal_1/2_particles` | Placed portals; `particles_2` attachment | SpriteCard / UnlitGeneric, including vacuum children |
| `portal_1/2_edge` | Placed portals; `particlespin` attachment | Additive trails |
| `portal_1/2_success` | Successful shot arrival | Complete UnlitGeneric particle material path |
| `portal_1/2_nofit` | `CantFit` arrival | Parent and refraction child supported |
| `portal_1/2_badsurface` | `BadSurface` arrival | Parent and refraction child supported |
| `portal_1/2_overlap` | `Overlap` arrival; CP1 at the other portal | Parent and refraction child supported |
| `portal_1/2_close` | Replacement, relocation and deactivation | Parent and refraction child supported |
| `portal_1/2_near` | Explicit `PortalImpact` API, with target portal frame | Parent, children and refraction child supported |
| `portal_1/2_badvolume` | Explicit `PortalImpact` API | Parent and refraction child supported |

Misses and sky/pass-through hits emit no impact. A bumped successful placement is
still success; its quality score is not interpreted as a near failure. This
chamber has no near-portal rejection or forbidden-volume placement outcomes, so
those two effects are not automatically selected. Cleanser volumes, gun charging,
and projectile stream dispatch remain outside this integration.

`particle/warp2_warp.vmt` uses `Refract`. It draws `refract_ps20b` static combo 66:
the frame behind the sprite, offset by the `warp1_normal` normal map scaled by
`$refractamount` and vertex alpha, blurred over the shader's four taps, blended
over the sparks of its own effect. As in `client.so` (0x5ccbd0), the frame is
copied once before a refracting effect draws, so the warp bends the scene without
its own sparks. The port takes Bevy's transmission snapshot after the opaque pass
at full resolution; the game copies into a power-of-two buffer after earlier
translucent draws.

## Binary and asset evidence

Checked against the hashes in `portal::install::PINNED` on 2026-10-11. Addresses
in this table are **Ghidra addresses (raw ELF virtual address + 0x10000)**.

| Evidence | Finding |
|---|---|
| server `0x92abf0`, dispatches at `0x92ad93` and `0x92adbe` | Colour byte selects `portal_1/2_particles` on `particles_2`, attachment mode 4, reset flag 1; `portal_1/2_edge` on `particlespin`, mode 4, reset flag 0. Dispatch follows relocation. |
| server `0x92cd30` | Precaches both portal models and persistent, success, near, nofit, overlap, badvolume, badsurface, close and cleanser effect names. |
| server `0x926260` | Fizzle code 0 success, 1 nofit, 2 overlap, 3 badvolume, 4 badsurface, 5 killed/close, 6 cleanser, 7 close, 8/9 near, 10 none. |
| server `0x92b580` | Delayed placement dispatches its result. Success closes the old active portal with code 7 before relocating it. Revalidates overlap and forbidden volume before dispatch. |
| server `0x926f40` | Explicit fizzle dispatches killed/close (code 5). |
| client `0x5550a0` | Overlap effect names receive CP1 at the nearest other active portal. With only one linked pair, this is the other colour. |
| client `0x533510` | Particle DrawModel uses the current render camera, rejects shadow/SSAO passes, and has distance/CP-owner culling. It has no portal-recursion-depth rejection. |
| server `0x926260`, float at raw `0xa747a4` | Ordinary impact origin is portal origin + forward; near dispatch uses a 16-unit offset at the selected existing portal. Near's selected frame depends on hit portal colour and fired colour. |

The impact orientation deserves care: `AngleVectors`' second output is *right*,
despite the public source naming that local `vUp`. The following `VectorAngles`
uses that right vector as forward and the portal normal as up. `Frame::impact`
preserves this basis; it does not reuse the persistent attachment basis.

Both `models/portals/portal1.mdl` and `portal2.mdl` were read directly. Their sole
idle sequence has one frame. The sampled bone pose differs from the bind pose;
using the bind pose would orient the particle plane incorrectly. `particles_2`
ends up five units along the portal normal. `particlespin` has opposite 20-degree
local rotations in the blue and orange models. Each model is loaded separately.

The actual PCFs specify blue's `fire_particle_4` SpriteCard, orange's
`effects/portal_2_particle` trail material, both edge trail definitions, and the
vacuum children. The VMTs supply blend flags, vertex colour/alpha flags,
SpriteCard size limits, overbright and add-self parameters, and depth blend.
VTF sheet resources supply both sampled rectangles and their interpolation
weight. Texture addressing follows the VTF clamp flags.

Public source was used as corroboration:
[server dispatch](https://github.com/SonicEraZoR/Portal-Base/blob/master/sp/src/game/server/portal/prop_portal.cpp),
[weapon near checks](https://github.com/SonicEraZoR/Portal-Base/blob/master/sp/src/game/server/portal/weapon_portalgun.cpp),
and [particle drawing](https://github.com/SonicEraZoR/Portal-Base/blob/master/sp/src/game/client/particles_new.cpp).
In that weapon source, near rejection concerns firing into a linked portal near
the eye; it does not mean an ordinary shot landed near another portal. The exact
near-detection distances and branch boundaries have not been ported into this
application's placement algorithm.

## Runtime, ownership and rendering

- Simulation remains in Source units and axes. It runs once per rendered game
  frame using virtual time; `System::simulate` retains its reconstructed PCF
  substeps, first-frame handling and timestep cap. Rendering additional cameras
  never advances simulation or the random stream.
- Definitions are owned by running systems; the immutable original random table
  uses shared ownership. There are no leaked static allocations or self-references.
  A deterministic sequence seed and one shared SIMD stream make identical event
  and timestep sequences reproducible. This does not claim identical seeds or
  frame timing to a separate original-game session.
- Per-colour revisions detect re-placement even at identical coordinates. Old
  persistent graphs and their draw entities are removed on replacement. Close
  graphs stay at the old frame and expire through the original operators.
- Mesh handles are reused across frames. Finished graphs and removed cameras
  release their entities and mesh assets. Materials and textures are loaded
  once. Existing sound playback uses cached waves and `PlaybackSettings::DESPAWN`.
- Empty batches are hidden without uploading an empty mesh. Bevy 0.19.1's mesh
  allocator skips empty allocations but attempts their upload, which otherwise
  logs an unallocated-key error during finite effects. This workaround keeps the
  last allocation only for the lifetime of the graph and has a regression test.
- Source positions are converted only while constructing render meshes; each
  camera's position and basis are converted back at that boundary. Billboards and
  distance sorting are rebuilt for that camera, with no duplicate simulation.
- Main, four recursive views and the depth-doubler copy each have a separate
  particle layer (32, 35–38, and 33 respectively). No particle layer is assigned
  to the viewmodel camera. Portal cameras retain their exit clipping plane.
- The shader implements additive (ONE/ONE and SRC_ALPHA/ONE), alpha-over and
  premultiplied blending. SpriteCard and UnlitGeneric vertex colours are
  linearized once with Source's power 2.2. SpriteCard sheet interpolation,
  alpha test, overbright, add-self and size fades are supported.

## Verification and remaining limits

Tests use the user's pinned installation and fail if it is unavailable. They
cover selection, blue/orange tint, both models' attachment planes and opposite
spins, child CP propagation, overlap CP1, deterministic simulation, finite graph
expiry, same-position replacement, deactivation, actual camera-layer wiring,
per-camera billboard differences, reused mesh handles, empty batches, and entity
and asset cleanup over repeated replacement. Existing renderer tests still cover
trails and ropes; a new test checks the second sheet rectangle and blend weight.

For asset inspection:

```sh
cargo run -p portal --example portal_inspect -- "$PORTAL_DIR" --effects
```

For repeatable graphical checks:

```sh
cargo run --release -- --script tests/fixtures/portal-particles.script \
  --shot-at 5 --shot-at 7 --quit-after 8 --shot-dir /tmp/portal-particles
```

Use `tests/fixtures/portal-particles-recursive.script` for facing portals. The
recursive scenario also passed with 4× MSAA, with outer trails and nested portal
views visible. Its capture is `/tmp/portal-particle-final-shots/shot-007.00.png`.

Scripted release runs exercised both persistent colours, linked views,
same-position replacement and rejected metal surfaces. After fixing empty mesh
uploads, the repeated-fire run exited successfully without renderer errors.
The host's gamescope harness sometimes segfaults during teardown *after* reporting
the application's exit 0; this is distinct from the application's render checks.

Final validation: all 444 workspace tests passed, `cargo fmt --all -- --check`
passed, `cargo clippy --workspace --all-targets -- -D warnings` passed, and the
release build passed. The repeated-fire and recursive release runs exited 0 with
no renderer errors.

Original-game reference captures succeeded in the pinned installed game in
`testchmb_a_08`, with its writable game/config paths redirected into `/tmp`.
The portal gun was explicitly selected; `sv_portal_placement_never_fail 1` was
used to isolate effect appearance from placement validation. The captures show
blue's static face with outer trails and orange's linked face with trails visible
through successive portal views. They confirm the qualitative effect layout and
recursive visibility, not pixel-identical brightness, random phase or timing.
The reference lighting, world geometry and exposure differ from the app chamber.
Successful reference log: `/tmp/portal-particle-original9.log`; screenshots:
`/tmp/portal-original-reference/screenshots/particles_reference_blue.jpg` and
`particles_reference_orange.jpg`. Earlier capture attempts were menu/wrong-weapon
frames and are not reference evidence. Original binaries/assets were not changed.

Remaining fidelity limits:

- The refraction warp samples a full resolution snapshot taken after opaque
  geometry, not the game's power-of-two copy taken after earlier translucents.
  A new refraction pipeline can miss its first frames while Bevy compiles it.
- Soft-particle depth uses Bevy's opaque depth prepass, reconstructed in Source
  units. It is not the original dest-alpha depth snapshot. Intersections with
  translucent portal faces and the original HDR/depth-snapshot behavior are not
  claimed pixel-identical.
- The legacy `Sprite` shader used by projectile ropes is drawable, but its original
  gamma-space framebuffer blending cannot be reproduced exactly by a linear HDR
  material pass. Projectile streams are loaded, not dispatched here.
- Original sleep/PVS behavior and distance-culling optimization are not ported;
  hidden world systems continue simulating, including when portal views are inactive.
- Near and bad-volume automatic triggers, cleanser volumes, and killed-versus-close
  sound distinctions need gameplay outcomes beyond the current chamber model.
