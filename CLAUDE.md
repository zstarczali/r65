# R65

KickAssembler compatible 6502/C64 cross assembler in Rust. The goal is to match
KickAss 5.25 on the same sources: the output bytes, the log text and the file
formats. The Cargo package and the program are named `r65`.

## Commands

```bash
cargo build
cargo build --release
cargo clippy
target/debug/r65 file.asm -showmem -vicesymbols
target/debug/r65 --help
target/debug/r65 --gui file.asm
```

`cargo test` runs the real program from `tests/`, each test in its own temporary
folder. `tests/language.rs`, `output.rs` and `cli.rs` check exact bytes, text and
exit codes. `tests/manuals.rs` assembles every ```` ```asm ```` block of both manuals
and checks the opcode tables, so an example that stops working fails the build.
`tests/kickass_manual.rs` holds the behaviour observed by running KickAss.jar, one topic
at a time. `tests/kickass_gaps.rs` holds what the original does and R65 doesn't do yet.
These tests are `#[ignore]`d and fail today: `cargo test --test kickass_gaps -- --ignored`.
Remove the `#[ignore]` when you fix one.

For a quick check, assemble a small `.asm` file and inspect the `.prg`, the log and
the exit code. Use the scratchpad for these, because outputs are written next to the
source. `-nooutput` writes nothing.

`F:\Development\c64_samples_kick` is a useful set of real KickAss sources, and
all 8 samples assemble. Run each from its own folder, because they import
`nv_c64_util` by relative path. Use `-nooutput` to keep that folder clean.

## Code map

The flow is `main` → `driver::assemble` → preprocess → lex → parse → passes → output.

| File | Job |
|---|---|
| `src/main.rs` | Arguments, window or command line mode, `r65.cfg` |
| `src/driver.rs` | Options, `HELP` option list, start-up banner, one whole assembly, error text |
| `src/help.rs` | `--help [topic]` texts |
| `src/asminfo.rs`, `src/asminfo_libraries.txt` | `-asminfo` sections |
| `src/preproc.rs` | Comments, `#define`, `#if`, `#import`; also `.import source` |
| `src/lexer.rs`, `src/parser.rs`, `src/ast.rs` | Tokens and syntax tree; `.directive` names are in `parser.rs` |
| `src/eval.rs` | `Asm` state, pass loop (`Asm::run`), scopes, labels, memory blocks |
| `src/exec.rs` | Running statements, instructions and addressing modes, macros, segments |
| `src/builtins.rs` | Expressions, built-in functions, methods of script values |
| `src/value.rs` | Script value types (`Val`), Java style number printing |
| `src/output.rs` | Program files, memory map, symbol files, byte dump, `.d64` images |
| `src/tables.rs`, `src/enc.rs` | Opcode tables per CPU, PETSCII and screen code tables |
| `src/jpeg.rs` | JPEG decoder that gives the same pixels as the original's Java runtime |
| `src/gui.rs` | Optional eframe/egui editor window |

## Compatibility rules

- `MECHANICS.md` explains each rule and its source in the original (class,
  method, bytecode offset). `mechanics.json` holds the same values as data.
  Read the relevant section before changing behaviour, and don't "correct" an
  odd behaviour there without checking: many are deliberate, such as
  `-maxaddr` not being enforced, Java `HashMap` label order in `.vs` files,
  Java number printing and CRLF in `.sym` files.
- When a rule changes or a new one is found, update both files. Say whether the
  value was observed by running the original or only estimated.
- Log and error text is part of the compatibility. Keep the wording, including
  the original's oddities such as `Made N asserts , M failed`.
- Exit code is 0 on success and 1 on any error.
- `KickAss.jar` isn't in this repository, but on this machine it's in
  `D:\Development\KickAss`. Run a copy from another folder, so its `KickAss.cfg` isn't read.
  Only claim a match with the original if it was actually compared. The original beats its
  manual: section 22 of `docs/R65-Manual.md` lists where the two disagree.

## When adding things

- A new command line option: add it to `Options`, `parse_args` and the `HELP`
  list in `driver.rs`. Add it to the `help.rs` topics if it needs explaining.
- A new directive, function or method: also update the matching topic in
  `help.rs` and both manuals in `docs/` (`R65-Manual.md`, `R65-Manual.hu.md`). The
  manuals must keep the same `asm` examples, apart from `//` comments, and describe
  what R65 does today. Their section 22 lists the differences from Kick Assembler.
- The version comes from `Cargo.toml`. The name and copyright are constants at the
  top of `driver.rs`.
- The icon sources are the two SVGs in `assets/`. After changing one, run
  `python assets/make_icons.py` to rebuild `r65.ico` (embedded in the Windows exe
  by `build.rs`) and `r65-icon-256.png` (the window icon, included in `gui.rs`).

## Builds and releases

`.github/workflows/build.yml` builds Windows, macOS and Linux on x64 and arm64,
and packages `.zip` or `.tar.gz` files; there is no MSI. It only runs when started
by hand (`workflow_dispatch`), and a run started on a `v*` tag also publishes a
GitHub release. It builds with `--locked`, so commit `Cargo.lock` whenever dependencies change. macOS x64 is
cross compiled and is the only target without a smoke test.

## Code style

There is no rustfmt config and many lines are longer than rustfmt's default, so
don't run `cargo fmt` over the whole project. `cargo clippy` already reports
warnings; don't add new ones.

## Known gaps

- Java plugins are not supported: `.plugin` and plugin modifiers report errors.
- The segment parameter `dest` is accepted but has no effect.
- The rest is in section 22 of `docs/R65-Manual.md`, with tests in `tests/kickass_gaps.rs`.
  The biggest ones: the original checks names before the first pass and lists all errors
  (`Got N errors and 0 warnings while executing:`), while R65 stops at the first one.
