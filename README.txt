R65
===

A KickAssembler compatible 6502 cross assembler for the Commodore 64, written in
Rust. It assembles KickAss 5.25 sources from the command line and aims to give
the same output bytes, log text and file formats as the original. No Java needed.

Home page: https://github.com/zstarczali/R65
Releases:  https://github.com/zstarczali/R65/releases


Features
--------

- KickAss language: macros, functions, pseudocommands, structs, scopes, namespaces,
  multi labels, multi-pass label resolving and a script language with lists,
  hashtables, vectors and matrices
- Preprocessor: #define, #if, #import and related directives
- CPUs: 6502 with or without illegal opcodes, C64 DTV and 65C02
- Memory blocks and segments, with .prg and raw .bin output and .d64 disk images
- Loading of binary files, SID tunes and gif, jpeg or png pictures
- VICE label files, importable symbol files, byte dumps and memory maps
- An optional editor window


Usage
-----

    r65 game.asm

This writes game.prg next to the source. The exit code is 0 on success and 1 on
any error.

Some common options:

    -o <file>        Name of the output file
    -odir <dir>      Folder for the output files
    -showmem         Print the memory map
    -vicesymbols     Write a VICE label file (.vs)
    -symbolfile      Write an importable symbol file (.sym)
    -bytedump        Write ByteDump.txt with addresses, bytes and source
    -define <name>   Define a preprocessor symbol
    -libdir <dir>    Extra folder to search for imported files
    -execute <cmd>   Run a program with the output file after a successful build
    --gui [file]     Open the editor window

Default options can be kept in a r65.cfg file next to the program.


Help
----

The built-in help covers all options and the language:

    r65 --help
    r65 --help directives

The topics are options, syntax, directives, preprocessor, functions, values,
segments, disk, encodings, cpu, output, config, examples, limits and all.


Example
-------

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

    r65 border.asm -showmem


Limitations
-----------

- Java plugins aren't supported, so .plugin and plugin modifiers report an error.
- Progressive or subsampled JPEG pictures may differ slightly from the original.

Run "r65 --help limits" for the current list. The manual lists every known
difference from the Kick Assembler reference manual, in section 22.


Documentation
-------------

The assembly language manual describes the whole language. It is included
as PDF in English (R65-Manual.pdf) and in Hungarian (R65-Manual.hu.pdf).


Copyright
---------

(C) 2026 Zsolt Tarczali


License
-------

MIT, see the LICENSE file.
