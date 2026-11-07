# R65

A KickAssembler compatible 6502 cross assembler for the Commodore 64, written in
Rust. It assembles KickAss 5.25 sources from the command line and aims to give
the same output bytes, log text and file formats as the original. No Java needed.

## Features

- KickAss language: macros, functions, pseudocommands, structs, scopes, namespaces,
  multi labels, multi-pass label resolving and a script language with lists,
  hashtables, vectors and matrices
- Preprocessor: `#define`, `#if`, `#import` and related directives
- CPUs: 6502 with or without illegal opcodes, C64 DTV and 65C02
- Memory blocks and segments, with `.prg` and raw `.bin` output and `.d64` disk images
- Loading of binary files, SID tunes and gif, jpeg or png pictures
- VICE label files, importable symbol files, byte dumps and memory maps
- An optional editor window

## Building

You need a [Rust toolchain](https://rustup.rs) that supports the 2024 edition.

```bash
cargo build --release
```

The program is written to `target/release/rust65` (`rust65.exe` on Windows).

## Usage

```bash
rust65 game.asm
```

This writes `game.prg` next to the source. The exit code is 0 on success and 1 on
any error.

Some common options:

| Option | What it does |
|---|---|
| `-o <file>` | Name of the output file |
| `-odir <dir>` | Folder for the output files |
| `-showmem` | Print the memory map |
| `-vicesymbols` | Write a VICE label file (`.vs`) |
| `-symbolfile` | Write an importable symbol file (`.sym`) |
| `-bytedump` | Write `ByteDump.txt` with addresses, bytes and source |
| `-define <name>` | Define a preprocessor symbol |
| `-libdir <dir>` | Extra folder to search for imported files |
| `-execute <cmd>` | Run a program with the output file after a successful build |
| `--gui [file]` | Open the editor window |

Default options can be kept in a `rust65.cfg` file next to the program.

## Help

The built-in help covers all options and the language:

```bash
rust65 --help
```

```bash
rust65 --help directives
```

The topics are `options`, `syntax`, `directives`, `preprocessor`, `functions`,
`values`, `segments`, `disk`, `encodings`, `cpu`, `output`, `config`, `examples`,
`limits` and `all`.

## Example

```asm
BasicUpstart2(start)

.const BORDER = $d020

.macro setBorder(color) {
    lda #color
    sta BORDER
}

* = $0810 "Main"
start:
    setBorder(RED)
    rts
```

```bash
rust65 border.asm -showmem
```

## Limitations

- Java plugins aren't supported, so `.plugin` and plugin modifiers report an error.
- Progressive or subsampled JPEG pictures may differ slightly from the original.
- There are no automated tests yet.

Run `rust65 --help limits` for the current list.

## Documentation

[MECHANICS.md](MECHANICS.md) explains how the assembler works and where each
compatibility rule comes from in the original program.
[mechanics.json](mechanics.json) holds the same values as data.

## Copyright

(C) 2026 Zsolt Tarczali
