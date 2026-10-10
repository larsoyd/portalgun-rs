# Pinned duck reconstruction, 2026-10-11

Raw ELF vaddr throughout; Ghidra = raw + 0x10000. server.so SHA256 9079f5dc98add02a0b10755f033353b32fa2d73c98645aca5df27a97a56f30fb; client.so 00b6e45eb84a93f0974c23d3e2656aa9e32e9ada64443f651899a78cde694aae.
Read-only Ghidra MCP client 8765 used for matching prediction functions, server disassembly to verify. Cached client decompiles in /tmp/portal-duck-decomp; server Duck disassembly /tmp/portal-duck.asm. Copied projects only in /tmp. Existing main server Ghidra has truncated PIC-thunk function bodies; later a previously repaired scratch project supplied complete server decompiles (see confirmation below).

## Addresses

| Function | server raw | client raw |
|---|---|---|
| Duck | 0x27b6e0 | 0x2314a0 |
| HandleDuckingSpeedCrop | 0x26de80 | 0x226ba0 |
| FinishUnDuck | 0x270540 | 0x226640 |
| FinishDuck | 0x279490 | 0x2303e0 |
| CanUnduck | 0x26fd90 | 0x228db0 |
| StartUnDuckJump | 0x27a3f0 | 0x230a20 |
| CanUnDuckJump | 0x27b220 | 0x231350 |
| FinishUnDuckJump | 0x279bc0 | 0x230770 |
| SetDuckedEyeOffset | 0x27a960 | 0x230c40 |
| UpdateDuckJumpEyeOffset | 0x27aa30 | 0x230d10 |
| ReduceTimers | 0x270e20 | 0x226080 |
| CPortal_Player::ForceDuckThisFrame | 0x908c60 | pending |

Player server offsets: flags 0x118 (FL_DUCKING=2); ducked byte 0x8c0 (determines hull), ducking byte 0x8c1 (transition), in_duck_jump byte 0x8c2; duck_timer_ms 0x8c4, duck_jump_eye_timer_ms 0x8c8, jump_timer_ms 0x8cc. Distinct booleans required: ducked/flag do not equal transition state. Client offsets respectively 0x33c, 0xe74, 0xe75, 0xe76, 0xe78, 0xe7c, 0xe80.

## Per-tick ordering

PlayerMove raw 0x27ab40: CheckParameters; clear outputs; ReduceTimers (call 0x27ac48); CategorizePosition; fall bookkeeping and sounds; UpdateDuckJumpEyeOffset (call 0x27acf1); Duck (virtual +0xbc at 0x27acfb); remaining movement including FullWalkMove and CheckJumpButton.
ReduceTimers subtracts frametime*1000 from EACH positive duck/duck-jump/jump timer and clamps negative to zero (0x270e20). Thus a freshly started grounded duck keeps timer=1000 on its first tick and fraction=0. Tick 2 has timer=985, etc. Full duck on tick 28 at 0.405s elapsed (strict >0.4, not >=). Full unduck after 14 intervals=0.210s, first release tick starts timer 1000 and zero elapsed.

CheckJumpButton success raw 0x27541b: SINGLEPLAYER writes jump_timer_ms=510, in_duck_jump=true. Therefore jump tick itself has original hull. Next tick ReduceTimers gives 495, Duck starts automatic shrink/foot elevation, even with Ctrl never pressed. Normal jump impulses/gravity occur AFTER Duck.

## Exact duck algorithm (normal alive player, no observer/vehicle)

Capture BEFORE modifying oldbuttons: changed = held ^ oldbuttons; pressed=changed&held; released=changed&oldbuttons. Snapshot in_air=(ground==null), in_duck=(flags&2)!=0, duck_jump=(jump_timer_ms>0), duck_jump_eye=(duck_jump_eye_timer_ms>0).
Update oldbuttons IN_DUCK bit from current held state; no clearing of jump bit here.
HandleDuckingSpeedCrop: only if not already speed-cropped this tick AND in_duck AND grounded, command f/s/up *= f32(1/3). Does NOT immediately clamp current velocity. Occurs before changing crouch state this tick. Grounded beginning of crouch is full speed; airborne crouch does not crop.

```
if held_duck || ducking || in_duck || duck_jump:
  if held_duck || duck_jump:
    if pressed_duck && !in_duck && !duck_jump && !duck_jump_eye:
      duck_timer=1000; ducking=true
    if ducking && !duck_jump && !duck_jump_eye:
      seconds=max(0,1000-duck_timer)*0.001f
      if seconds>0.4 || in_duck || in_air: FinishDuck()
      else: SetDuckedEyeOffset(spline(seconds/0.4))
    if duck_jump:
      if !in_duck: StartUnDuckJump()
      else if !held_duck && CanUnDuckJump(trace):
        FinishUnDuckJump(trace)
        duck_jump_eye_timer=800 + 200*(1-trace.fraction)
  else:
    if in_duck_jump:
      if !held_duck:
        if CanUnDuckJump(trace):
          FinishUnDuckJump(trace)
          if fraction<1: duck_jump_eye_timer=800+200*(1-fraction)
      else: in_duck_jump=false
    if duck_jump_eye: return  # snapshot at entry, not recalculated
    if allow_auto_movement || in_air || ducking:
      if released_duck:
        if in_duck && !duck_jump: duck_timer=1000
        else if ducking && !ducked:
          duck_timer=800 + ((1000-duck_timer)/400)*200
      if CanUnduck():
        if ducking || ducked:
          seconds=max(0,1000-duck_timer)*0.001f
          if seconds>0.2 || (in_air && !duck_jump): FinishUnDuck()
          else:
            SetDuckedEyeOffset(spline(1-seconds/0.2)); ducking=true
      else:
        if duck_timer != 1000:
          SetDuckedEyeOffset(1); duck_timer=1000
          ducked=true; ducking=false; flags|=FL_DUCKING
```

The impossible-looking `else held_duck` in in_duck_jump handling follows original source structure; compiler removes it because outer branch already knows !held_duck. Preserve relevant branch semantics. If no outer duck activity, eye-height repair resets eye to standing when not dead, observing, in vehicle, or during jump-eye recovery and offset differs >0.1.

## Hull and camera operations

Spline = t*t*(3-2*t), with f32 polynomial. Duck seconds calculations f32 then cast to double for time comparison/division factors. Constants server raw 0xa81e44=1000f, 0xa81e6c=0.001f, 0xa83d10=0.4 double; client 0x7e4d88=0.4 double, 0x7e4d90=2.5 double; 0x7e29e8=0.2 double, 0x7e2e40=5 double. Crop f32 1/3 at server 0xa83c20.

SetDuckedEyeOffset(q): eye_z=(standing_mins.z + crouched_eye.z - crouched_mins.z)*q + standing_eye.z*(1-q). For standard mins both zero: eye=28*q+64*(1-q). Leaves eye x/y untouched. Camera transition does NOT continuously resize the hull.

FinishDuck: return if FL_DUCKING already set. Set flag, ducked=true, ducking=false, eye=crouched. If grounded feet += standing_mins - crouched_mins (zero); if airborne feet += standing_hull_size - crouched_hull_size (=36 vertically). FixPlayerCrouchStuck(moveup=true); CategorizePosition.

StartUnDuckJump: set flag, ducked=true, ducking=false, eye=crouched, feet += full hull size delta 36; fix crouch stuck upwards; CategorizePosition. Does not set in_duck_jump because CheckJumpButton did that already. Camera world eye preserved: +36 feet, -36 local eye.

CanUnduck: target feet unchanged on ground (hull min correction), feet-36 airborne. Temporarily use STANDING hull, trace from current feet to target feet, restore current hull selection. Require !trace.startsolid AND fraction==1. This is not just a stationary clearance test at the destination.

FinishUnDuck: apply same target feet calculation; ducked=false, flag clear, ducking=false, in_duck_jump=false, eye=standing, duck_timer=0; assign feet; reset interpolation latch; CategorizePosition.

CanUnDuckJump: trace CROUCHED hull 36 down; require fraction<1. Candidate feet=original feet -36*fraction. Temporarily use STANDING hull, stationary trace at candidate; restore current hull. Require !startsolid (no separate fraction test for stationary trace).

FinishUnDuckJump(trace): lower feet by 36*fraction; clear duck flag, ducked, ducking, in_duck_jump, ALL THREE timers; eye=64 -36*(1-fraction) =28+36*fraction. CategorizePosition. Caller then may set eye-recovery timer above.

UpdateDuckJumpEyeOffset: if eye timer !=0, seconds=max(0,1000-eye_timer)*0.001; if seconds>0.2: eye_timer=0; eye=standing. Else eye=spline(1-seconds/0.2) blend. Runs BEFORE Duck. On first recovery frame uses caller's fraction through timer. Later integrates exactly 15ms/tick.

Expiration recovery: when jump_timer reaches 0 but in_duck_jump remains true, Duck's release branch still attempts CanUnDuckJump. If no nearby ground, regular CanUnduck then expands in air by lowering feet 36 if clear. If Ctrl held, remain crouched until release. Low-ceiling clearance prevents expansion.

## ForceDuckThisFrame

server 0x908c60 confirms: IF not already ducked, set ducked=true, ForceButtons(IN_DUCK=4) via 0x89a690, flags|=2, SetVCollisionState(current origin,current velocity,VPHYS_CROUCH=1) via 0x8a37f0. This function itself does NOT move feet, update eye, set ducking, set timers or set in_duck_jump. Teleport caller makes additional changes and is documented in collision-evidence.md.

## Remaining view scope

Client portal-specific view interpolation beyond the reconstructed movement eye offset remains incomplete. Forced-input removal is verified below.


## Confirmed dimensions and input

Portal gamerules vtable raw 0xcb5dfc slot32 GetViewVectors raw0x27d6a0 returns raw0xdf4260. Static initializer raw0x1bae62..0x1baeed copies 96 bytes from raw0xa84110. Decoder values:
- raw0xa84110 standing eye (0,0,64)
- raw0xa8411c standing mins (-16,-16,0)
- raw0xa84128 standing maxs (16,16,72)
- raw0xa84134 crouched mins (-16,-16,0)
- raw0xa84140 crouched maxs (16,16,36)
- raw0xa8414c crouched eye (0,0,28)
- subsequent observer hull mins/maxs +/-10.
These are pinned Portal values, not just SDK defaults. Scale factor server player+0x370 (client+0x608) multiplies all hull vectors; normal player scale 1.

Installed /home/lars/.local/share/Steam/steamapps/common/Portal/portal/cfg/config_default.cfg lines12-13 bind SPACE +jump, CTRL +duck. Mac-specific override ALT +duck lines39-43.

## Confirmed server/client parity and additional details

Recovered usable pre-existing scratch Ghidra server project, copied to /tmp/portal-duck-server-good/Server, and decompiled all eleven server functions read-only. Full server decompiles now /tmp/portal-duck-decomp/FUN_<ghidra-address>_<ghidra-address>.c. Server Duck and helpers confirm client pseudocode above (server-only network dirty bookkeeping omitted).

FixPlayerCrouchStuck raw server0x279370/client0x2302c0 tests current player hull; if stuck, increment feet.z by one up to 36 attempts; keep first non-stuck location; restore initial origin if all attempts fail. Both callers pass true (up). Client decompile confirms literal loop count0x24 and displacement1.

ForceButtons raw0x89a690 ORs bits into player+0xaa8; UnforceButtons raw0x89a6b0 clears bits. Portal ProcessMovement raw0x2ce200 calls PlayerMove at0x2ce2c7, FinishMove0x273db0 at0x2ce2cd, then UnforceButtons(IN_DUCK=4) at0x2ce2e2 and UnforceButtons(IN_JUMP=2) at0x2ce2ee. Thus these forced bits are command-scoped. Teleport calls ForceDuck and separately sets in_duck_jump=true; caller center offset correction is distinct from ordinary duck foot-preservation rule.

CheckJumpButton refuses when ducking && FL_DUCKING (uncrouch transition), and when duck_jump_eye_timer>0. It allows entering crouch (ducking=true but flag clear), and stable crouching. Jump impulse REPLACES vz with160*jumpFactor if ducking OR FL_DUCKING; only fully standing with no crouch transition ADDS it to vz after first gravity half-step (raw275327,27533a). SP success sets jump_timer510 and in_duck_jump whether already crouched or standing.

Derived flat-floor standing-jump numbers (binary formulas, NOT gameplay measurement; assuming grounded zero-vz start and default jumpFactor1): tick1 feet2.265, end-vz146.5, standing72/eye64. Tick2 feet40.395 including automatic +36 hull shrink, end-vz137.5, crouched36/eye28. Maximum world eye tick17=84.145 (standing eye64 plus20.145), feet56.145. Sweep/categorize and duck-jump ground trace must also be included before asserting total airtime.


## Initial categorization guard

PlayerMove raw0x27ade0: for MOVETYPE_WALK=2, if !mv.gameCodeMovedPlayer (mv byte0 mask2) && sv_optimizedmovement true, skip initial CategorizePosition; clear ground only if vz>250f (raw0xa74800). MOVETYPE_WALK with game-code displacement or optimization disabled goes through initial CategorizePosition. Other movement types also categorize. Source's spawn/teleport game-code movement state matters: a newly constructed host Walker may need one initial categorization as equivalent initialization, not unconditional per-tick categorization.

Implementation review of src/walker.rs: current main duck transitions match binary branch ordering. Required missing helper was FixPlayerCrouchStuck before finish_duck recategorization. For bit-level arithmetic, uncrouch interpolation computes t=(float)(1.0double-double(seconds)*5.0double), then f32 spline, unlike two separate f32 arithmetic steps.

sv_optimizedmovement ConVar raw0xdf3de0, registration0x1bacc1..0x1bacdd, default string raw0xb19e4e="1", flags0x2002; name string raw0xa83ab1. Client PlayerMove raw0x230df0 confirms ReduceTimers virtual +0xb0 then UpdateDuckJumpEyeOffset0x230d10 at0x230fa1 then Duck virtual+0xbc at0x230fab.
