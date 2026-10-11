# Third-party code

## Player rig

The primitive player rig in `src/body.rs` adapts the segment layout, joint
radii, materials, head geometry, and segment posing from
[Funny-Bones/ELDEN-RING-Combat-Rewrite](https://github.com/Funny-Bones/ELDEN-RING-Combat-Rewrite),
`src/rig.rs` at commit `177a3b5ec14a9bde39b9ab458a3400b4038224e3`.

Copyright (c) 2026 Funny-Bones and contributors. Used under the MIT License;
the full notice is preserved in
[licenses/ELDEN-RING-Combat-Rewrite-MIT.txt](licenses/ELDEN-RING-Combat-Rewrite-MIT.txt).

This adaptation draws the primitive rig on Chell's skeleton and animation clips
read from the user's Portal installation. It retains the segment geometry,
head, materials, and folding mitten approach, with Portal bone-name mappings.
The animation controller and held gun are integrated with Portal movement.
It does not include Elden Ring skeleton or animation data.

The held gun is Portal's `w_portalgun`, attached to the animated `weapon_bone`.
Neither Valve assets nor derived animation data are committed. The upstream
MIT license covers the adapted code, not Valve's assets.
