# Portal movement collision evidence, 2026-10-11

Repository HEAD inspected: aa6d848744bbc91656773516f93b202feca7a64d.
Pinned server.so SHA256: 9079f5dc98add02a0b10755f033353b32fa2d73c98645aca5df27a97a56f30fb.
All addresses below are **raw ELF virtual addresses**. Ghidra addresses = raw + 0x10000.
Evidence is static binary tracing, not observed gameplay. Existing scratch decompiles read from
`/tmp/claude-1000/-home-lars-Projects-rust-rewrite-portalgun-rs/850edd9f-7f82-4888-a33f-6d7dfadd1146/scratchpad/dec*`.
New decompiles are `/tmp/portal-collision-decomp`; copied project `/tmp/portal-collision-ghidra`, opened read-only, no original project mutations.
Critical comparisons/constants independently checked with objdump and raw ELF data reads.

## Audit

At reviewed HEAD, Walker::travel performs axis-by-axis endpoint collision, not Source TryPlayerMove; push_out and ground_below are explicit approximations. Ground stays attached via broad top-surface selection, not hull sweeps. STEP=18 and GROUND_REACH=2 have Source precedent but their use does not reconstruct StepMove/StayOnGround. Surface parser retains only step sounds/material, dropping movement factors. These omissions make passing fling/walker tests insufficient fidelity evidence.

## ClipVelocity — raw 0x2762c0

- blocked flags: normal.z > 0 gives bit 1, normal.z == 0 gives bit 2.
- backoff = dot(input, normal) * overbounce.
- output = input - normal * backoff.
- adjust = dot(output, normal); if adjust < 0, output -= normal * adjust.
- No STOP_EPSILON per-component zeroing in this binary (unlike some older Source/Quake code).

## TryPlayerMove — raw 0x2763b0

- Four bumps, five stored collision planes maximum.
- Save primal velocity and current original velocity, time_left = frametime.
- Break if squared velocity length equals zero.
- Sweep from origin to origin + velocity*time_left. An existing identical first destination/trace may be reused with movement optimizations enabled.
- Add each trace fraction to allFraction. allsolid => velocity zero, return blocked=4.
- If fraction > 0: for fraction==1 run an extra zero-length trace at end; invalid destination (startsolid or fraction !=1) => stop. Otherwise move origin to end, reset original velocity to current velocity, clear accumulated planes.
- fraction==1 => done.
- Ground blocked bit uses normal.z > double 0.7, wall bit uses normal.z ==0.
- If already five planes, stop. Append normal.
- Special first-plane airborne walking branch: clip original velocity against first plane; overbounce 1 if normal.z >0.7, otherwise 1+sv_bounce*(1-surfaceFriction). sv_bounce default=0.
- General branch tries clipping original velocity against each plane with overbounce=1, accepting candidate only when its dot against every other plane >=0.
- If none accepted and exactly two planes: normalize cross product of planes and project current velocity onto that crease. More planes => stop.
- Stop if dot(result_velocity, primal_velocity)<=0.
- time_left *= 1-fraction and continue next bump.
- allFraction==0 => stop. Horizontal collision speed loss also triggers rough landing effects (not reconstructed here).

## StepMove — raw 0x26f150, vtable +0xd8

- Save origin/velocity; run ordinary TryPlayerMove using supplied first destination/trace.
- Save ordinary slide result. Restore starting origin/velocity.
- If allow-auto-movement (+0x900) then up destination += stepsize+0.03125. Double epsilon at raw 0xa83cf0.
- Sweep up; accept endpoint only when neither startsolid nor allsolid.
- TryPlayerMove from raised origin, then sweep down by stepsize+0.03125 when allowed.
- If down-trace normal.z < double 0.7, restore ordinary slide path.
- Otherwise accept safe down-trace endpoint and compare squared horizontal distances of both paths.
- If ordinary slide went farther, restore it. Ties choose stepped path.
- Stepped path retains **ordinary slide's z velocity**.
- Positive final height gain accumulates in outStepHeight (+0x64).

## StayOnGround — raw 0x274050

Trace origin to origin+(0,0,2), then from its end to original origin-(0,0,stepsize).
Snap only when 0 < fraction <1, !startsolid, normal.z >= double 0.7,
and abs(origin.z-end.z)>0.015625. Last tolerance is 1/64, raw float 0xa83c54.

## Ground surfaces — raw 0x2732c0 / SetGroundEntity raw 0x273390

CategorizeGroundSurface:
1. Trace surface index (+0x40 short) => player surface index +0xf44.
2. physics GetSurfaceData => player surface pointer +0xf48.
3. physics GetPhysicsProperties => player friction +0xf4c.
4. Multiply friction by 1.25 (raw float 0xa83c50), cap above at 1.0.
5. Material from surfacedata +0x48 => player +0xf50.

SetGroundEntity(non-null trace) calls CategorizeGroundSurface, clears waterjumptime, handles ground-touch notification, zeros move velocity.z. It adjusts base velocity on moving-ground changes, then calls CBaseEntity::SetGroundEntity raw 0x2a4550.

Portal CategorizePosition raw 0x2d06d0:
- No base implementation's unconditional friction=1 reset! Base raw 0x278f80 has that reset; Portal override omits it.
- Water checks; observer early exit; trace down 2 unless velocity.z>140 or moving up on ladder.
- Ground normal must >= double 0.7, with quadrant ground checks when initial trace is nonwalkable.
- On failure clear ground; if velocity.z>0 and not noclip set surfaceFriction=0.25.
- On success SetGroundEntity refreshes friction; when waterlevel<2 and trace neither solid nor zero fraction, snap origin to trace end.
- Therefore carried surface friction and 0.25 air-friction state can persist in air; do not insert SDK base reset into Portal's override.

CheckParameters raw 0x273970:
maxspeed=min(serverMax,clientMax) when clientMax !=0; multiply by min(surface.maxSpeedFactor(+0x40),constraint factor); cap combined command length. surface.jumpFactor is +0x44 (CheckJumpButton raw 0x2750e0). Physics friction affects braking and ground/air acceleration, not just speed.

## Portal hull trace — raw 0x2ce890

Constructs swept Ray_t from dynamic GetPlayerMins/GetPlayerMaxs (vtable +0x6c/+0x70), collision group 8, mask usually 0x201400b. Resolve portal environment player+0x1370; call raw 0x2dcce0 (Ghidra 0x2ecce0). That wrapper uses portal simulation trace raw 0x2dc8e0 only when environment and linked simulation are active, otherwise engine TraceRay vslot +0x10.
If initial fraction==1, helper raw 0x2dc5a0 succeeds, and boolean cvar enabled, run raw 0x2dd520 against player collision model; replace initial result only with a nearer result that is neither startsolid nor allsolid.

This is **more than removing the portal's parent wall**: it relies on portal simulation/linked collision state and supplementary collision models. A Rust AABB-chamber aperture trace is a host adaptation, not a complete translation of this dependency graph. Tilted portal rim/dynamic brush/contact cases remain open unless implemented and tested separately.

## Teleport/force-duck corrections to existing movement.md

Prior note's “No direct call to the ground entity setter” is incorrect:
- TeleportTouchingEntity raw 0x912ff0 contains call **0x9136be -> 0x2a4550(entity,NULL)**, unconditionally after velocity transform/clamp and touch bookkeeping.
- Additional ground clear at call 0x91330b on floor/ceiling-to-nonvertical teleport branch.

Force-duck condition in teleport raw 0x91329c..0x913328:
abs(entry.forward.z)>0 AND
(abs(abs(entry.forward.z)-1)>=0.01 OR abs(abs(exit.forward.z)-1)>=0.01).
Tolerance raw 0xa7481c = float 0.01.
When condition holds and player not yet ducked, call raw **0x908c60**, then set m_bInDuckJump (+0x8c2)=true. Adjust local eye/center z offset by -16 if entry.z>0, +16 otherwise (raw float 0xa747a4=16).

ForceDuckThisFrame raw 0x908c60:
- Return immediately if already ducked (+0x8c0).
- Set ducked=true, AddButtons(IN_DUCK=4) via raw 0x89a690, AddFlag(FL_DUCKING=2) via raw 0x2378f0.
- Refresh collision state via raw 0x8a37f0 using origin, velocity, ducked argument true.
- Camera interpolation is not implemented in this helper; recovery runs through movement duck state.

## Proposed independent boundary scenarios

- Clip into ceiling, oblique wall, two perpendicular walls, three-plane corner; preserve tangential velocity and reject penetration.
- Sweep at high velocity across thin AABB; starting contact moving away must not collide; actual startsolid and allsolid separate.
- Step obstacle heights at 18, 18+1/32 boundaries; low overhead clearance; stair alternative farther/shorter/tied than ordinary slide.
- StayOnGround differences at exactly 1/64 vs just above, fraction=0 vs positive, 0.7 normal threshold.
- Surface friction raw 0.4 -> effective0.5; raw0.8 ->1; >0.8 capped. Air acceleration after slow ascent must retain0.25 until landing restores surface coefficient.
- Teleport clears ground; floor-to-wall forces crouched hull; wall-to-floor no force solely from exit orientation; floor-to-floor does not force crouch.

No gameplay observation or engine/vphysics full trace-equivalence measurement was performed by this investigation.

## Engine collision skin — additional dependency traced

Installed Portal engine.so path `/home/lars/.local/share/Steam/steamapps/common/Portal/bin/engine.so`.
SHA256 `e3c5b0c567a0a4fae765a572b983d8aae5100acb68ae18322c32a67cbb1bdd7a`.
Raw engine disassembly cached `/tmp/portal-engine.asm`.

String anchor `CM_TraceToLeaf` raw0x841275, source path `/data/src/engine/cmodel.cpp` raw0x841284.
Function raw0x322720 refers to both (profiling source line0x5c3), calls brush clip raw0x3223a0 at0x32294f.
Brush clip calls optimized AABB clip raw0x31e480 at0x3226b9 when brush sides marker==0xffff.

General brush entering-plane path raw0x322440..0x322454:
- d1,d2 computed from plane distance at sweep start/end, around0x322477..0x3224cf.
- epsilon .03125 read from rawfloat0x829f88 at0x322417.
- entering fraction = max(0,d1-epsilon)/(d1-d2).
- leaving fraction uses (d1+epsilon)/(d1-d2), rawdouble0x841320.

Optimized AABB path raw0x31e480:
- First computes slab rejection using exact distances.
- At0x31e5e8 adds vector[-.03125]*4 (raw0x8412f0) to lower slab distances.
- At0x31e5ef adds vector[+.03125]*4 (raw0x841300) to upper distances.
- Multiplies by inverse delta then reduces slabs to entry/exit times.
- Stores accepted entry fraction into trace work+0x80 at0x31e6d7.

This confirms **1/32-unit collision skin** in actual engine dependency. It is not a reason to loosen StayOnGround's strict fraction<1 test. A hull moving toward wall should normally stop1/32 before ideal plane; an exact endpoint floor collision can report fraction<1 and finish1/32 above floor. Existing viewer tests requiring exactly x=-16/z=floor pin its old idealized trace convention, not engine trace fidelity. Initial overlap classification should still use actual geometry; expanding every AABB before all tests is not equivalent to the engine's two-stage rejection/TOI calculation.

## Forced-duck teleport origin formula: exact reconstruction excerpt

The origin/center distinction is critical. Portal player's WorldSpaceCenter override raw0x901a60
(vtable raw0xd4dd40 +0x258, with ELF data segment file offset -0x1000) returns abs origin plus
scaled half-standing/half-ducked hull height; it does not return eye position.
Teleport obtains that center **before** forcing duck.

Binary-derived pseudocode for player branch:

```text
old_origin = player.abs_origin
transfer_center = player.WorldSpaceCenter()  # pre-force hull center
if abs(entry.forward.z) > 0:
    aligned_pair = abs(abs(entry.forward.z)-1) < 0.01
                && abs(abs(exit.forward.z)-1) < 0.01
    if !aligned_pair:
        player.SetGroundEntity(NULL)
        if !player.ducked:
            player.ForceDuckThisFrame()
            player.in_duck_jump = true
            transfer_center.z += (-16 if entry.forward.z > 0 else 16)
new_origin = ThisToLinked.TransformPoint(transfer_center)
           + old_origin - transfer_center
new_velocity = ThisToLinked.Rotate(old_velocity)
# existing exit velocity clamp rules follow
player.SetGroundEntity(NULL)
player.Teleport(new_origin, transformed_angles, new_velocity)
```

Raw branch anchors0x91329c..0x913328; ForceDuck call0x914984;
subtract16 at0x9149f5, add16 branch at0x914ed8; transformed-origin arithmetic0x91332e..0x913418.
**Do not subtract the new crouched half-height (18) from the transformed transfer_center.**
For a previously standing player falling into a floor portal, old center is feet+36, adjusted transfer center is feet+20; the post-transform world-z origin offset is -20. Ceiling entry yields transfer center feet+52 and offset -52. Already-ducked players do not receive +/-16 adjustment and use their initial crouched center.

Corroboration (not the sole authority):
https://github.com/SonicEraZoR/Portal-Base/blob/master/sp/src/game/server/portal/prop_portal.cpp
lines939-984 matches the disassembled branch and origin formula. ForceDuck itself does not adjust eye/feet. Portal ProcessMovement removes forced IN_DUCK after PlayerMove; next tick ordinary recovery applies.

## Review of work-in-progress Rust collision (read-only, 2026-10-11)

1. Walker::travel must only run step-path alternative when initial horizontal sweep blocks. Source WalkMove fraction==1 path sets origin and StayOnGround immediately. Unconditional StepMove may mount unrelated overhead platforms.
2. StepMove upward trace updates origin only when neither startsolid nor allsolid; otherwise start alternative at original origin. Descend rejection must cover both flags.
3. Trace skin needs recomputation of all three expanded entry/exit slabs after exact geometric hit, not just subtraction from original winning plane. Different velocity magnitudes can change winning plane after expansion.
4. TryPlayerMove fraction==1 validates zero-length hull at end before accepting it; missing in current slide.
5. Source special first-plane airborne branch overwrites original velocity with clipped output even when fraction==0. With orthogonal AABB planes/default bounce zero this is mostly equivalent; retain as documented simplification if not represented.
6. Corner crease calculation in Source uses the output from final failed clip candidate, not the incoming velocity. For exact orthogonal AABB planes crease projection agrees, but general normal support must retain that distinction.
7. Current AABB chamber does not provide sloped brush normals/quadrant support, physics objects, or linked portal simulation clone shapes. Do not claim these unsupported cases reconstructed.

## Implementation review disposition

The review corrections above were applied: StepMove only follows a blocked horizontal trace,
all skin-adjusted entry planes participate in normal selection, unobstructed slide endpoints
are validated, solid step traces are rejected, and teleport preserves the verified adjusted
origin-to-center offset. The destination remainder is now swept. An initially obstructed exit
conservatively rejects the crossing. This is still a chamber adaptation, not complete
CPortalSimulator equivalence.
