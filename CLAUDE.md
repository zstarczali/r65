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

`cargo test` runs but the project has no tests. To check a change, assemble a
small `.asm` file and inspect the `.prg`, the log and the exit code. Use the
scratchpad for these, because outputs are written next to the source. `-nooutput`
writes nothing.

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
- `KickAss.jar` isn't in this repository. Only claim a match with the original if
  it was actually compared.

## When adding things

- A new command line option: add it to `Options`, `parse_args` and the `HELP`
  list in `driver.rs`. Add it to the `help.rs` topics if it needs explaining.
- A new directive, function or method: also update the matching topic in
  `help.rs`.
- The version comes from `Cargo.toml`. The name and copyright are constants at the
  top of `driver.rs`.

## Code style

There is no rustfmt config and many lines are longer than rustfmt's default, so
don't run `cargo fmt` over the whole project. `cargo clippy` already reports
warnings; don't add new ones.

## Known gaps

- A pseudocommand call must stay on one line (`mov #1 :` followed by a line break
  is a syntax error). Definitions may span several lines.
- Java plugins are not supported: `.plugin` and plugin modifiers report errors.
- The segment parameter `dest` is accepted but has no effect.
