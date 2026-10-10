# portalgun-rs

Portal's handheld portal device, rebuilt in Rust for [Bevy](https://bevyengine.org).

The aim is a sandbox where the gun, the portals and moving through them behave
as they do in Portal (2007). Behaviour comes from the game's own files and
binaries rather than from tuning by eye.

This is a fan project for study. It is not affiliated with or endorsed by
Valve. Nothing from the game is in this repository, only code. To run it you
need your own copy of Portal, which it reads at startup.

## What is in it

The `portal` crate has no dependencies. It contains:

- **Readers for Portal's files**:
  - VPK archives
  - MDL models with their vertex and strip files, plus animations sampled into
    bone poses
  - VTF textures, converted to DDS, with sprite sheet resources
  - KeyValues text and sound scripts
  - binary DMX particle files (`.pcf`)
- **The particle runtime the gun and portals use**:
  - every initializer, emitter, operator, force and constraint that Portal's
    particle files call for
  - system start-up, sub-stepped simulation and child systems
  - sprite, trail and rope renderers, with sprite sheets and SpriteCard cards
  - the game's own 4096-entry random table, in the order the game reads it
- **The portal surface**: how the Portal shader blends the view through the
  portal, the edge and the static overlay.
- **The gun's effects**: the glow sprites and how they fade as the gun's
  state changes.
- **Portal maths**:
  - the pair transform
  - rays passing through a pair
  - fitting a portal onto a surface, including bumping a shot away from edges
    and snapping it to the floor
  - which side of a portal a mover is on, and its exit speed

## Running the tests

```
cargo test --workspace
```

To see what the crate reads from your install:

```
cargo run -p portal --example portal_inspect -- "/path/to/Steam/steamapps/common/Portal"
```
