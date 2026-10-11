# Escape menu

Original behavior is the default. Esc opens a pause menu with Resume Game,
Options and Quit. Options contains six unchecked improvement toggles. Existing
`original_*` settings remain supported: `true` selects the original behavior.

Apply saves the draft and leaves Options open. OK saves and returns to the pause
menu. Cancel, including the Options close button, discards changes since the last
Apply. Esc hides GameUI and resumes play, retaining an open Options draft for
reopening. Quit has a confirmation dialog; canceling it keeps the game paused.

The application pauses virtual simulation time, stops movement/look/fire input,
releases the cursor and discards input across capture transitions. The frame
that resumes cannot turn a menu click or Space press into a shot or jump. Saving
writes a temporary sibling file and renames it only after a successful write;
failure leaves the active configuration and pending draft intact.

## Binary evidence

Read-only Ghidra MCP was used for the client input and VGUI methods. GameUI and
engine paths were traced through disassembly of the installed sibling binaries.
Addresses below are raw ELF virtual addresses unless prefixed `G`; client Ghidra
addresses have an image-base offset of `0x10000`.

| Original path | Evidence | Application behavior |
|---|---|---|
| Engine Escape dispatch | `engine.so 0x4edb30`, key `0x46`, `gameui_activate` / `gameui_hide` when a level is loaded | Open/hide the in-game UI |
| GameUI activation and hide | `GameUI.so 0x193600` / `0x192c70`, max clients `<= 1` sends `setpause` / `unpause` | Pause/unpause this single-player sandbox |
| Resume | `GameUI.so 0x175b0c` → `0x194810` → `gameui_hide` | Resume without closing the application |
| Mouse transition | Engine activation `0x4ec1e0`, surface cursor `2` at `0x4ec29c`; client MCP `G0x50bac0` clears accumulated deltas; `G0x50a7c0` / `G0x50a730` deactivate/activate input | Free/lock the Bevy cursor and suppress transition input |
| Options commands | `GameUI.so 0x2e80d0`: Apply invokes apply with argument `1` and stays open; OK uses `0` and closes on success; Cancel closes without apply; helper `0x2e7fa0` applies the sheet | Stage, apply, accept or discard changes |
| Options lifetime | `GameUI.so 0x16cb80` caches the dialog; `0x1c0690` forwards GameUIHidden to children without applying or closing it | Keep pending choices when hiding the UI |
| Quit | `GameUI.so 0x175a20` → `0x170280` QueryBox, confirmed command `QuitNoConfirm` at `0x170372` | Confirm before exiting |
| Presentation | Installed `portal/resource/gamemenu.res`, `platform/resource/sourceschemebase.res`, `hl2/resource/optionssubportal.res` | Resume/Options/Quit order, gray panel, orange selection, checkboxes and an OK/Cancel/Apply footer |

The Options draft/file transaction and clearing held Bevy keys are adaptations
of those contracts. They are not translations of Source's cvar persistence or
input data structures. The global Escape implementation follows the traced
engine path; exact PC VGUI event ordering has not been compared in a recording
of the running original.

Main-menu buttons activate on press; ordinary Options buttons activate on
release while still selected. Space and Enter follow the same timing. This comes
from GameUI `0x269fa0` / `0x26a180`, activation mode `0` set at `0x26a7a9`,
main-menu override `1` at `0x169da0`, and key handlers `0x26a500` / `0x269100`.
The rollover, press and release WAVs are read from the installed game; their
assignments appear at GameUI `0x169d71`, `0x169d7f` and `0x169d8f`.

## Audio and remaining differences

Source pauses selected channels, not the entire sound device. Engine
`0x26fca0` reads the pause state through `0x382110` → `0x2c5660` → `0x2cb890`;
`0x270070` filters channels whose byte at `+0x135` has bit `0x10` set.
The channel initializer at `0x2508a4` copies sound flag `0x80` into that bit.
Client wrappers `G0x2ae190`, `G0x2ae9f0`, `G0x2aed90` and `G0x2af050` start
with flags zero, and engine `0x379d60` passes caller flags through unchanged.
The pause predicate is a flag, not whether the sound is a loop or one-shot.

Existing unflagged application sounds can finish while simulation is paused.
Per-emitter pause flags have not been mapped for every gameplay sound; complete
Source audio pause behavior remains unverified. There is no blanket device pause.

The menu is a Bevy adaptation for this sandbox, not the entire VGUI interface.
It has no save/load, level selection, key binding editor, or display/audio tabs.
Source blur, focus/transition fades and exact font metrics are not reproduced.
The text uses Portal's installed DejaVu Sans fallback font. Display and mouse
settings remain available in the settings file. No game resources are bundled.

Tests cover defaults, independent toggles, atomic-save failure, Apply/Cancel,
Escape draft retention, quit confirmation, input capture transitions and live
body-shadow changes. Original and optional camera gates are checked at floor
entries with wall/floor/ceiling exits, standing/crouched and before/after crossing.
