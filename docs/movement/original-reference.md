# Original Portal movement reference, 2026-10-11

All binary addresses below are raw ELF virtual addresses (Ghidra server addresses add 0x10000). No original binaries or Ghidra database were edited. Static reads used objdump against pinned server; live Ghidra MCP was client.so. Installed game server SHA256 matches pinned binary: `9079f5dc98add02a0b10755f033353b32fa2d73c98645aca5df27a97a56f30fb`.

## Actual executable validation

Original `/home/lars/.local/share/Steam/steamapps/common/Portal/hl2_linux` successfully loaded `testchmb_a_00` and `testchmb_a_08`. Console says build9862575 and 66.7 tickrate. Simulation interval 0.015 seconds. Gravity log600.0. Initial direct sandbox launch failed159; escalated launch was approved, then missing 32-bit SDL resolved using installed SteamLinuxRuntime i386 libraries. A minimal FONTCONFIG_FILE in /tmp avoided old bundled fontconfig incompatibilities with host font configuration. The original renderer produced an actual playable chamber and Portal gun (see `/tmp/portal-original-game.png`).

Every writable game search path and game configuration was redirected through `/tmp/portal-original-reference/gameinfo.txt`; reads continued from original installed assets. `cl_cloud_settings 0` used on measurement runs. No user saves were intentionally loaded or changed. First isolated launch downloaded existing Steam Cloud cfg into /tmp; no explicit cloud-save command was issued. Launch logs are `/tmp/portal-original-launch2.log` and `/tmp/portal-reference-gdb.log`.

## Measurement method

`/tmp/portal-reference.gdb` sets a read-only breakpoint at actual runtime module base + server raw0x2ce200 (CPortalGameMovement::ProcessMovement), then a FinishBreakpoint records CMoveData and player fields at return. `/tmp/portal-measured-real-input.csv` contains keyboard-driven results. It has noisy X11 events and camera changes around some runs, so do not use those portions as controlled experiments.

`/tmp/portal-controlled.gdb` supplies explicit command buttons, forward/sidemove, view angles, and previous-command button state at the same ProcessMovement entry. It does not alter positions, velocities, collision geometry, gravity, acceleration, or duck timers. Therefore `/tmp/portal-measured-controlled.csv` is an observed original-engine movement reference under injected input, not an end-to-end original input-system test. It uses the original `testchmb_a_08` collision world; running paths cross a floor-height change. The reference input schedule is encoded in the script.

Sampled fields: player maxspeed+0xdd0, suit+0x8f9, ducked+0x8c0, ducking+0x8c1, inDuckJump+0x8c2, jumptime+0x8cc, duckjumptime+0x8c8, flags+0x118. CMoveData buttons+0x24, oldbuttons+0x28, origin+0x98, velocity+0x40.

## Observed speed and grounded acceleration

The actual Portal player has suit0 and maxspeed150.0 every measured tick. This upgrades the former no-suit inference to observed behavior in these installed maps.

Original forward command450 scales to maxspeed150. First nine ground velocities are22.4999981,38.9999962,55.4999924,71.9999924,88.4999924,104.9999924,121.1999969,136.4279938,150.0 (controlled ticks200..208). Sustained velocity150.0 through249. Running jump250 yields225.0 horizontal; backward jump400 yields-225.0. Both first vertical outputs146.5.

Static provenance: CPortal_Player Spawn raw0x906fc0 calls CHL2_Player Spawn0x601a70 at906fe0. CHL2 Spawn checks suit at601a97; no suit branch601bc8 calls StartWalking0x5fa100. Spawn calls maxspeed updater0x5fb4a0 at601b62. Updater reads suit at5fb4c9, selecting `hl2_normspeed` when equipped or `hl2_walkspeed` when absent. Its no-suit setting read is5fb528 and store5fb5f8/5fb784. Thus observed150 has both binary call path and runtime state evidence.

## Observed standing jump and gravity

Controlled baseline tick100 feet(-960,-128,146.5607147). Hold jump101..145.

| Relative tick | feet z | vz | ducked | inDuckJump | jump timer ms |
|---:|---:|---:|---:|---:|---:|
|0|146.5607147|0|0|0|0|
|1|148.8257141|146.5|0|1|510|
|2|186.9557190|137.5|1|1|495|
|17|202.7057190|2.5|1|1|270|
|18|202.6757202|-6.5|1|1|255|
|34|183.8357239|-150.5|1|1|15|
|35|146.5607147|0|0|0|0|

Held jump stays grounded35..45; no automatic second jump. Tick2 origin lift36 is automatic jump-duck hull repositioning even without Ctrl. Feet apex delta56.1450043 includes the hull-origin lift; physical ballistic displacement after subtracting36 is20.1450043. Initial physical displacement2.265 and vz146.5 confirm the existing160 impulse and600 gravity: first FullWalkMove half-step, impulse addition, CheckJumpButton half-step, movement, final half-step. Airborne velocity then decreases9 per tick. Landing in this measured original spot occurs relative tick35 (.525s), with grounded-position snap involved.

Raw0x275327 tests player `ducking`+0x8c1 OR FL_DUCKING in flags+0x118 to choose assigning impulse instead of adding to incoming vz. It does not test only fully-ducked+0x8c0. The sprint/crouch horizontal-boost percentage later uses fully ducked+0x8c0. Default movement optimizations byte raw0xd6b1e8 is1, selecting160 constant raw0xa83c58; fallback sqrt(gravity*42) is not active. Original initial ledge/free-fall velocities measured-9,-18,-27...

## Crouch timing and input

Original `portal/cfg/config_default.cfg` line13 binds Ctrl to+duck (Mac overrides differ). Keyboard run Ctrl begins tick1924; fully crouched1951 (27 further ticks, .405s). Release1959; fully standing1973 (14 further ticks, .210s). Grounded feet remain unchanged through both transitions. Precise controlled crouch-jump samples are in the accompanying controlled CSV (input schedule500..599 duck,550..599 jump).

## Surface parser actual dependency

Installed `bin/vphysics.so` SHA256 `2cf294207058fa31aed0a1bee93def3260b76b7c770da5d80181e78962cc5615`. Parser raw0x2b700 reads surface key/value tokens with case-insensitive comparisons. String refs and branches:

- friction stringraw0x157384 compared at0x2b9f1; parse at0x2c098 usingstrtod, convert tofloat, store surface+0.
- maxspeedfactor stringraw0x145a51 compared at0x2ba17; parse at0x2c0d0 and store surface+0x40.
- jumpfactor stringraw0x145a60 compared at0x2ba3d; parse at0x2c130 and store surface+0x44.
- base stringraw0x145a2f, handling0x2b8e0, copies referenced surface via0x2b310. Before token parsing it looks up an existing surface with the current name, otherwise default (string0x145a10), at0x2b87b..0x2b8c4. Copy routine0x2b310 copies0x60 bytes of properties; parser objects include a0x10-byte prefix before surface data.

Original loose hl2/scripts/surfaceproperties.txt default explicitly sets friction0.8,jumpfactor1,maxspeedfactor1. The physical-friction value is not directly the player multiplier: its ground-surface conversion must also be applied (server ground routine investigated by integration agent). Physics parser assembly: `/tmp/portal-reference-vphysics.asm`.

## Limits

Measurements establish these pinned Linux binaries, not an independently archived2007 retail executable. No measured floor/wall portal fling, low-ceiling, wall-slide, surface-factor override, water/ladder, or full matching Rust GUI trace was produced in this slice. A passing Rust unit test alone must not be described as observed fidelity. Root integration owns Rust tests and graphical comparison.

## Controlled crouch-jump and duck-walk results

Input Ctrl begins tick500 and fully ducked527. Standing baseline for this scenario is146.5607147216797. Ctrl+Space550..599:

- jump550: feet148.89321899414062 (delta2.33250427), vz151.0, ducked1, inDuckJump1, jump timer510.
- next551: feet151.09071350097656,vz142.0. Already crouched jump has no36-unit lift.
- apex567 (relative tick18): feet167.89071655273438 (delta21.33000183),vz-2.0.
- landing584 (relative tick35): baseline feet, vz0.0. Ctrl+Space remains held through599; no repeated jump.
- release both600: instantly standing, inDuckJump0, duckjumptime1000.0; timed eye recovery persists although hull stands. Timer reaches0 at614.

Ordinary crouch650 fully completes677. Ground duck-walk690..719: first vx7.499999523, second8.999999046, then increments1.5/tick under100-stopspeed friction until50.0 at719. This verifies both the1/3 command crop and unchanged stopspeed100. Release forward720 vx44.0, then decrements6/tick. Ordinary Ctrl release750 begins unduck; fully standing764.

These observed original samples can serve as independent numerical assertions for Rust movement tests. Relevant whole sequences should be copied from CSV, not recomputed from the Rust implementation. Automatic crouch-jump eye recovery on landing is a distinct path from ordinary .2-second unduck.

## Final snapshot and fixture suitability

Controlled CSV completed1246 ticks; all task-owned game/debugger processes exited (verified with pgrep). `/tmp/portal-measured-reference.json` extracts numerical scenarios. `/tmp/portal-measured-crouch-normalized.csv` is controlled original ticks650..780 normalized against pre-command tick649 origin and renumbered1..131. Every sample has fixed original feetz146.5607147216797 and y-128.0. Horizontal position change agrees with reportedvx*.015 within0.000029326units, consistent with unobstructed travel (contact traces themselves were not captured). This flat sequence is suitable for a normalized fixture. Input schedule relative: duck1..100; forward41..70; release forward71; release duck101; standing115. Velocity after releasing duck-walk is44,38,32,26,20,14,8,2,0 at relative71..79. No clean standing-walk braking-only scenario was recorded; running-jump landing/braking contains a floor-height transition and should not be presented as one.
