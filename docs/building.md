# Building

`cargo build --profile fast` for everyday work: release speed, incremental,
finely split (`Cargo.toml`). Nightly toolchain (the workspace uses the
`profile-rustflags` cargo feature).

## Rebuild times (2026-10-20)

A one-line change (a constant), `fast` profile, 24-thread machine; median
of three:

| Edited | Before | rust-lld | + no ThinLTO in `game` |
|---|---|---|---|
| `combat/hunter.rs` | 8.2 s | 7.2 s | 5.0 s |
| `chasm.rs` | 10.8 s | 10.2 s | 6.4 s |
| `rig.rs` | 7.1 s | 5.5 s | 4.3 s |

Where a rebuild went (`cargo rustc -p game --bin game --profile fast --
-Z time-passes`, an edit in `hunter.rs`, before): thin local LTO 3.8 s,
linking (MSVC link.exe) 3.2 s, preparing the incremental session 1.1 s,
LLVM on the changed code 0.7 s; the front end (parsing, macros, type
checking) well under a second. So splitting the crate (to spare the front
end) or `-Zthreads` would gain little; the back end is the cost.

- **rust-lld** (`.cargo/config.toml`): linking 3.2 s to 1.6 s.
- **No thin local LTO in `game`** (`-Clto=off` for that package only):
  saves 2 to 4 s. Frame time with 30 stalkers (`--set bench on --set hunters
  on --set crowd 30`): 17.0 ms with it, 17.5 ms without (seven runs each,
  spread about 1 ms): within the noise, at most a few percent. Off for every
  crate (`lto = "off"`), 19.7 ms: 13% slower, so the dependencies keep it.
  For performance work, judge with it on: `rustflags = []` there.

Toggling these settings rebuilds the dependencies once (some 4 min), and
switching back and forth may leave a stale `game.exe` in place (cargo finds
both builds fresh): touch a source file to be sure.

## Not done

- Splitting the `game` crate (the chasm is 12k of its 33k lines and needs
  little of the rest): the front end is cheap, so little to gain now.
- Splitting the largest modules (`chasm.rs` 5k lines, `rig.rs`, `hunter.rs`):
  an edit re-optimises its module's codegen unit; `chasm.rs` edits are the
  slowest. A modest gain.
- Excluding `target/` from Windows Defender's real-time scanning: the
  incremental session's preparation varies from 0.3 to 1.1 s. A system
  setting, the user's to make.
- Bevy's `dynamic_linking`: Windows caveats; linking is now 1.6 s.
