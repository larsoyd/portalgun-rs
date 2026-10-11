# License coverage

Original work by larsoyd in both workspace crates, including source, tests,
shaders, documentation and supporting scripts, is offered under the
[PolyForm Noncommercial License 1.0.0](LICENSE). This grant covers only rights
the author has authority to grant. Third-party material retains its own terms.

The canonical license text is kept unchanged in `LICENSE`, from the
[official plaintext download](https://polyformproject.org/licenses/noncommercial/1.0.0.txt).
The project supplies its [required notice](NOTICE) separately. Copies of both
files are included in the `portal` crate so its source package stands alone.

## Third-party boundaries

`src/body.rs` combines original contributions with MIT-licensed rig geometry
and posing adapted from Funny-Bones and contributors. The upstream portions
retain their MIT grant; original contributions by larsoyd are under PolyForm.
The file and application package therefore declare
`PolyForm-Noncommercial-1.0.0 AND MIT`, not a choice between licenses for the
whole application. The dependency-free `portal` crate declares
`PolyForm-Noncommercial-1.0.0`.

Keep the rig's copyright and full MIT notice with copies containing that work.
The [third-party credits](THIRD_PARTY.md) identify its revision and scope.
Cargo dependencies retain their own licenses and are not relicensed by this
project. Using a dependency does not make the whole project available under
that dependency's license.

Portal and its files remain Valve's. No permission over game assets, Valve
code or trademarks is granted here. The application requires an original,
unmodified Portal installation and reads assets locally. Numerical observations
and format fixtures used by tests are not bundled game assets. References to
the Source SDK, Portal-Base or disassembly document implementation evidence;
they are not grants to redistribute or relicense those sources.

## Use and redistribution

Read the license for the operative terms. It permits noncommercial purposes,
specifies personal uses without anticipated commercial application, and lists
institutions whose use is permitted regardless of funding. Changes and new
works require a permitted purpose. A commercial integration generally needs
separate permission unless an express permitted-purpose provision applies.
Compiling the code or putting it in a library, plugin or game does not itself
create an exemption.

Distribution has its own license clause. Recipients must receive the terms or
their URL and supplied `Required Notice:` lines. Our source archives include
the full text, `NOTICE`, this coverage description and the rig's MIT notice.
Do not add restrictions to upstream components or imply exclusive authorship
over them. For a mixed-purpose or commercial distribution, assess the actual
license clauses rather than treating this summary as permission.

## Distribution policy

This project releases source only, without vendored dependencies or game data.
Both Cargo packages set `publish = false`. The application uses a local path
dependency; its package file list is not a promise of registry publishability.
Any future registry publication needs a separate packaging and policy review.

The [dependency inventory](docs/dependency-licenses.csv) records the locked
versions, declared licenses, selected alternatives, features and target
relationships audited on 2026-10-11. `registry/<crate>-<version>/...` evidence
paths refer to files inside the downloaded crate archive; URLs identify
upstream evidence. Inactive lock entries are distinguished from enabled
dependencies. License expressions alone do not capture every bundled asset.

Before distributing a binary or vendored dependency archive:

- Collect applicable dependency copyright notices, full license texts and
  NOTICE files, including generated output and statically included material.
- Preserve Fira and Cantarell font notices under OFL-1.1, and the separate
  shader notices in Bevy. Include the Zlib notice for Mikkelsen's tangent code,
  both licenses for `dpi`, and the Unicode notices.
- Establish the exact grants for Bevy's AgXc and Blender-derived lookup tables.
  Their provenance is not fully cleared by the crate's MIT/Apache metadata.
- Resolve the scope of LGPL-labelled Plasma protocol XML in generated bindings.
  Winit uses the blur protocol. The MIT wrapper declaration alone does not
  resolve the terms for the bundled protocol sources or generated output.
- Review LGPL conditions for native libraries, including ALSA and libudev.
  Retain replaceable shared-library linkage, provide required notices, and
  establish the necessary modification and debugging permissions. Do not
  silently change the canonical PolyForm text to address these conditions.
- Review Apple SDK-derived binding terms before Apple distributions. Do not
  bundle SDKs or operating-system components under the project's license.
- Complete missing dependency-specific notice details, including `constgebra`
  and `dispatch`. Preserve separately licensed examples, tests and images if
  those files are vendored. Disabled GPL data/font features are not blanket
  permission to redistribute their assets.

These are distribution gates, not findings that every listed component is
incompatible. Keep existing dependencies and features until a demonstrated
issue requires a change. Obtain authoritative clarification or professional
review where the applicable rights remain uncertain.

## Maintenance

Run `python3 tools/check_licenses.py` and
`cargo deny --locked --workspace --all-features check licenses` when changing
licenses or dependencies. The first checks text integrity, metadata, headers
and inventory coverage. The second checks allowed package expressions,
including build and development dependencies. Neither establishes authorship
or clears the distribution gates above.

Update the inventory for each dependency change using locked Cargo metadata,
the enabled all-target dependency tree, and the actual upstream notices.
Keep the distinction between license alternatives (`OR`) and cumulative
obligations (`AND`). New third-party material needs its own provenance review.

Before a source release, also run:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -j 4 -- -D warnings
cargo test --locked --workspace -j 4 -- --test-threads=2
```

Inspect both package file lists and the source archive for the license files,
notices and absence of game assets. Run graphical scenarios against the
original installation; missing game data or a failed renderer is not a pass.
