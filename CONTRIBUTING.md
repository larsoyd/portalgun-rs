# Contributing

Contributors retain copyright in their work. Submit original contributions
under the PolyForm Noncommercial License 1.0.0 and only when you have authority
to grant those rights. No copyright assignment is requested.

Identify any third-party code, translations, adaptations or assets in your
contribution. Provide the source, revision, applicable terms and required
notices. Preserve existing attribution. Public availability or attribution
alone does not establish permission to copy or relicense an implementation.

Keep Portal binaries and assets out of the repository. The program must keep
reading and checking the user's original installation. Do not weaken those
checks or bundle extracted game data to make tests pass.

Use brief SPDX headers for original Rust and shader files, and document mixed
licensing in `THIRD_PARTY.md`. Update the dependency inventory and run the
checks in [license coverage](LICENSING.md) when dependencies or notices change.
Follow the formatting, Clippy and workspace test checks described there and
in the README. Keep changes focused and comments brief.
