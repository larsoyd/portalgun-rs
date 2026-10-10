# portalgun-rs

This is Portal's handheld portal device. In Rust, on [Bevy](https://bevyengine.org).

Nobody tuned it by eye. The numbers come out of Portal's own binaries and
files, and where we couldn't find one, we didn't invent one. The gun, the
portals and the trip through them should behave the way they did in 2007.
That's the job.

You need your own copy of Portal. The program reads it at startup, and none of
it ends up in this repository. Not a texture, not a sound, not a line of the
game's code.

Fan project. Valve didn't make it and hasn't endorsed it.

## What's in the box

The `portal` crate. Zero dependencies. It does the following.

**It reads Portal's files:**

- VPK archives
- MDL models with their vertex and strip files. Animation tracks are decoded
  the way Source decodes them and come out as bone poses.
- VTF textures, converted to DDS, sprite sheets included
- KeyValues text and sound scripts
- binary DMX particle files (`.pcf`)

**It runs the particles the gun and the portals use.** Every initializer,
emitter, operator, force and constraint Portal's particle files ask for. Child
systems, sub-stepped simulation, sprite, trail and rope renderers,
SpriteCard sheets.
The random numbers come from the game's own 4096-entry table, read in the
order the game reads it.

**It draws the portal surface** the way Portal's shader does: the view
through the hole, the edge, and the static you see before the other end
exists.

**It runs the gun's glow sprites** and the fade between the gun's states.

**It does the portal maths:**

- the transform from one portal to its partner
- rays that pass through a pair
- fitting a portal onto a wall, including nudging a shot off an edge and
  snapping it down to the floor
- which side of a portal you're on, and how fast you come out the other one

Everything is in Portal's own units: Source units, z up. If you want metres,
convert at the door.

## Running it

The tests:

```
cargo test --workspace
```

To see what the crate pulls out of your install:

```
cargo run -p portal --example portal_inspect -- "/path/to/Steam/steamapps/common/Portal"
```
