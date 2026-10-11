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

Original contributions by larsoyd are offered under PolyForm Noncommercial
1.0.0. The upstream portions retain their MIT grant. The combined `src/body.rs`
file declares `PolyForm-Noncommercial-1.0.0 AND MIT`; this does not offer an MIT
alternative for all of the project's original code. See [license coverage](LICENSING.md).

## Dependencies

Dependencies retain their own licenses. The [locked dependency inventory](docs/dependency-licenses.csv)
records exact versions, alternatives selected for compliance, obligations and
evidence. It includes build dependencies, platform dependencies and inactive
lock entries. Dependencies are downloaded by Cargo, not vendored here.

Crate metadata does not cover every embedded font, shader, lookup table,
protocol definition or test asset. See the [distribution policy](LICENSING.md#distribution-policy)
for the notices and unresolved questions that must be addressed before binary
or vendored distributions. No game data is included in source releases.

## Game files and reference implementations

Portal models, animations, textures, fonts, sounds and the particle random table
are read from the user's original installation. They remain Valve's property
and are not covered by the rig's MIT license or this project's PolyForm license.

The [movement](docs/movement/README.md) and [particle](docs/particles/README.md)
evidence identifies Source SDK and Portal-Base comparison sources. Those sources
have separate terms; a citation does not grant redistribution rights. The
project's grant covers its independently authored implementation and does not
relicense Valve's code or other third-party expression.
