# R65 Assembly Language Manual

[Magyar változat](R65-Manual.hu.md)

R65 is a KickAssembler compatible 6502 cross assembler for the Commodore 64. This manual describes the
language that R65 accepts: how to write source files, every directive, the script library and the files that
R65 produces. It describes what R65 does today. Where that differs from the Kick Assembler reference manual, section
[22](#22-limitations) says so. For how the rules were derived from KickAss 5.25, see [MECHANICS.md](../MECHANICS.md).

All examples marked as assembly source in this manual are assembled by the automated tests.

## Contents

1. [A first program](#1-a-first-program)
2. [The command line](#2-the-command-line)
3. [Source format](#3-source-format)
4. [Values and expressions](#4-values-and-expressions)
5. [Instructions and addressing modes](#5-instructions-and-addressing-modes)
6. [Labels and the program counter](#6-labels-and-the-program-counter)
7. [Passes and forward references](#7-passes-and-forward-references)
8. [Data directives](#8-data-directives)
9. [Variables, constants, enums and structs](#9-variables-constants-enums-and-structs)
10. [Control flow](#10-control-flow)
11. [Functions, macros and pseudocommands](#11-functions-macros-and-pseudocommands)
12. [Scopes and namespaces](#12-scopes-and-namespaces)
13. [The preprocessor](#13-the-preprocessor)
14. [Script library](#14-script-library)
15. [Text encodings](#15-text-encodings)
16. [Memory blocks and segments](#16-memory-blocks-and-segments)
17. [Output files and disk images](#17-output-files-and-disk-images)
18. [Importing data](#18-importing-data)
19. [Debugging and testing](#19-debugging-and-testing)
20. [CPUs and opcode sets](#20-cpus-and-opcode-sets)
21. [Error messages](#21-error-messages)
22. [Limitations](#22-limitations)
23. [Directive index](#23-directive-index)

## 1. A first program

Save this as `border.asm`. It starts from BASIC with `RUN` and turns the border red:

```asm
BasicUpstart2(start)        // a BASIC line "10 SYS 2064" at $0801

.const BORDER = $d020

* = $0810 "Main"
start:
    lda #RED
    sta BORDER
    rts
```

Assemble it:

```bash
r65 border.asm -showmem
```

R65 writes `border.prg` next to the source and prints the memory map:

```text
Memory Map
----------
Default-segment:
  $0801-$080c Basic
  $080e-$080d Basic End
  $0810-$0815 Main
```

Load the program in VICE with `x64sc border.prg`, or start it automatically after every successful build:

```bash
r65 border.asm -execute x64sc
```

## 2. The command line

```text
r65 <file.asm> [options]
```

The exit code is 0 on success and 1 on any error. A failed `.assert` does not change the exit code.

| Option | Meaning |
|---|---|
| `-o <file>` | Name of the output file, relative to the current folder |
| `-odir <dir>` | Folder for the output files (default: the source file's folder) |
| `-binfile` | Write a raw `.bin` without the 2 byte start address |
| `-mbfiles` | Write one file per memory block |
| `-nooutput` | Log as usual but write no files |
| `-showmem` | Print the memory map |
| `-vicesymbols` | Write a VICE label file (`.vs`) |
| `-symbolfile` | Write an importable symbol file (`.sym`) |
| `-symbolfiledir <dir>` | The same, into this folder |
| `-bytedump` | Write `ByteDump.txt` with addresses, bytes and source |
| `-bytedumpfile <file>` | The same, with this file name |
| `-debugdump` | Write a `.dbg` file that maps bytes to source lines, for C64 Debugger |
| `-asminfo <sections>` | Write assembly information for editors (see below) |
| `-asminfofile <file>` | File for `-asminfo`; default `asminfo.txt` |
| `-asminfotostdout` | Print the `-asminfo` text instead of writing a file |
| `-define <name>` | Define a preprocessor symbol |
| `-libdir <dir>` | Extra folder to search for imported files (can be repeated) |
| `-fillbyte <n>` | Byte used for gaps between memory blocks (default 0) |
| `-maxaddr <n>` | Accepted but not enforced, like the original 5.25 |
| `-aom` | Allow memory blocks to overlap; later blocks win |
| `-afo` | Allow scripts to write files with `createFile` |
| `-excludeillegal` | Don't accept illegal 6502 opcodes |
| `-dtv` | Accept the DTV opcodes |
| `-cpu <name>` | `_6502NoIllegals`, `_6502`, `dtv` or `_65c02` |
| `-warningsoff` | Don't print warnings |
| `-noeval` | Only parse the source |
| `-time` | Print how long the assembly took |
| `-debug` | Accepted; it prints nothing extra, like the original in the cases tried |
| `-log <file>` | Also write the log to a file |
| `-execute <cmd>` | Run a program with the output file after a successful build |
| `-executelog <file>` | Send the output of the `-execute` program to this file. R65 doesn't wait for the program |
| `-cfgfile <file>` | Read more options from a file, relative to the source file (can be repeated). A missing file only gives a warning |
| `-pseudoc3x` | A semicolon separates pseudocommand arguments, as in Kick Assembler 3 |
| `-replacefile <a> <b>` | Use file `b` wherever file `a` is imported |
| `-maxdiag <n>` | How many warnings to list (default 5) |
| `:name=value` | A value for the script hashtable `cmdLineVars` |
| `--gui [file]` | Open the editor window |
| `--screenshot <png>` | Open the window, save a picture of it and quit |
| `-h`, `--help [topic]` | Help; `r65 --help` lists the topics |

Numbers in options can be written as `255`, `$ff` or `0xff`.

`-asminfo` takes section names joined with `|`: `version`, `libraries`, `directives`, `preprocessorDirectives`, `files`,
`syntax` and `errors`, or the groups `all`, `allPredefined` and `allSourceSpecific`. Each section starts with its name in
brackets, and its lines have fields separated by `;`. An unknown name gives the warning `Invalid asminfo type: x`. `[files]` only
names the files that the `errors` and `syntax` sections use. The `[version]` line has R65's version, and the directive texts are
R65's own. With `-nooutput` only `-asminfotostdout` gives a result.

Default options can be kept in `r65.cfg` next to the program, one or more per line. Lines starting with `#` or
`//` are skipped. The file is read before the command line and isn't used in window mode.

## 3. Source format

- **Statements** end at the end of the line or at a `;`. Unlike many assemblers, `;` is not a comment:

  ```asm
  lda #1; ldx #2; rts
  ```

- **Comments** are `// to the end of the line` and `/* a block */`. Block comments can span lines and can sit in
  the middle of a statement, as in `sta /* border */ $d020`.
- **Upper and lower case differ** everywhere. Mnemonics must be lower case: `lda` is an instruction, while
  `LDA` is reported as an unknown pseudocommand.
- **Identifiers** are made of letters, digits and `_`, and don't start with a digit.
- **Line breaks** mean nothing inside `( )` and `[ ]`. Everywhere else a line break ends the statement, also after a
  comma or an operator, so `.byte 1,` followed by a new line is a syntax error:

  ```asm
  .byte (1 +
         2), 3

  .macro drawBox(x, y,
                 width,
                 height)
  {
      .byte x, y, width, height
  }

  drawBox(1, 2,
          3, 4)
  ```

- **Bodies** of `.if`, `.for` and `.while` can be a `{ }` block or one statement. An opening `{` can be on
  the next line.

## 4. Values and expressions

### Numbers

| Form | Example | Value |
|---|---|---|
| Decimal | `123`, `1.5` | 123, 1.5 |
| Hexadecimal | `$ff` | 255 |
| Binary | `%1010` | 10 |

Exponent notation like `1e3` isn't accepted. All numbers are 64 bit floating point values and print like Java
doubles: whole numbers without a fraction, then forms such as `1.5`, `1.0E-4`, `Infinity` and `NaN`.

### When a value becomes bytes

`.byte`, `.word` and `.dword` drop any fraction and keep the low bits, without an error. So `.byte 256` gives `$00`,
`.byte -1` gives `$ff`, and `.byte 3/2` gives `$01`:

```asm
.byte 256, -1, 3/2, 255.9   // 00 ff 01 ff
.word 70000                 // 70 11
```

A string can't be written with `.byte`; use `.text`. `.number()` gives the number behind a character or the text of
a string: `'x'.number()`, `"12".number()`.

### Strings

| Form | Meaning |
|---|---|
| `"text"` | Plain string; a backslash has no special meaning |
| `@"text\n"` | String with escapes |

Escapes in `@"..."` strings: `\n`, `\t`, `\r`, `\b`, `\f`, `\\`, `\"` and `\$xx`, which gives the raw byte `xx`
regardless of the encoding.

`+` joins strings as soon as the **left** side is a string, converting the right side to text. A number on the left
is an error, so start with an empty string:

```asm
.var count = 3
.print "count=" + count     // count=3
.print "" + count + " items"
```

### Characters

`'a'` is a character. It takes its numeric value from the current [encoding](#15-text-encodings), and it prints as a
character. With the default encoding `screencode_mixed`, `'a'` is 1:

```asm
.byte 'a'                   // 01
lda #'a'
lda #'a'+1                  // a9 02: characters work as numbers
.text "World" + '!'         // and as text after a string
```

### Booleans, null and lists

`true`, `false` and `null` are values. `null` prints as `Null`. Conditions must be real booleans: `.if (1)` is an
error, because numbers aren't converted to booleans.

`[1, 2, 3]` is a list with at least two items, and `[x]` is only a grouping that gives `x`. Empty and one item lists are
made with `List()`.

### Parentheses

Both `( )` and `[ ]` group an expression, and they can be nested. Next to an instruction they differ, because `( )` also
selects an indirect mode:

```asm
* = $1000
lda #(2 + 5) * 2            // a9 0e
lda #[2 + 5] * 2            // a9 0e
jmp ($1000)                 // 6c 00 10: indirect
jmp [$1000]                 // 4c 00 10: absolute
```

### Operators

From the tightest to the loosest:

| Level | Operators | Notes |
|---|---|---|
| 1 | `.` `[]` `++` `--` | Member access, indexing, postfix increment |
| 2 | `-` `+` `!` `~` `++` `--` | Unary |
| 2 | `<` `>` | Low and high byte; see below |
| 3 | `*` `/` | `/` is a real division: `3/2` is `1.5` |
| 4 | `+` `-` | |
| 5 | `<<` `>>` | Whole numbers |
| 6 | `<` `<=` `>` `>=` | |
| 7 | `==` `!=` | |
| 8 | `&` | Whole numbers |
| 9 | `^` | Whole numbers |
| 10 | <code>&#124;</code> | Whole numbers |
| 11 | `&&` | Stops early |
| 12 | <code>&#124;&#124;</code> | Stops early |
| 13 | `? :` | |
| 14 | `=` `+=` `-=` `*=` `/=` | Assignment |

`<` and `>` as prefixes apply to the whole arithmetic expression after them, so `<$1234+1` is the low byte of `$1235`:

```asm
.byte <$1234+1, >$1234      // 35 12
.byte <($1234+1)            // the same, written clearly
```

Note that `&`, `|` and `^` bind looser than comparisons, as in C. Write `(a & 1) == 1`.

Comparing with `==`: numbers and characters compare by value, strings and characters by text. Two lists are equal only
if they are the same list. A number is never equal to a string.

### The `?:` expression

`condition ? a : b` gives one of two values and evaluates only that one. It can be used in operands:

```asm
* = $1000
.var debug = true
inc debug ? $d020 : $d013   // ee 20 d0
.var text = 1 > 2 ? "yes" : "no"
```

### Assignment outside `.eval`

An assignment returns its value, so `.eval x = y = 25` sets both. A postfix `x++` returns the value from before the
change. An assignment needs `.eval` to be a statement: a plain `x = 2`, `x++` or `list.add(1)` is a syntax error.

```asm
.var x = 1
.eval x = 2
.eval x++
.eval x += 3
.eval x *= 2
.print "x=" + x             // x=12
```

## 5. Instructions and addressing modes

| Mode | Syntax | Example |
|---|---|---|
| Implied, accumulator | | `nop`, `asl` |
| Immediate | `#value` | `lda #$10` |
| Zero page | `value` | `lda $10` |
| Zero page indexed | `value,x` `value,y` | `lda $10,x`, `ldx $10,y` |
| Absolute | `value` | `lda $1000` |
| Absolute indexed | `value,x` `value,y` | `lda $1000,x` |
| Indirect | `(value)` | `jmp ($1000)` |
| Indexed indirect | `(value,x)` | `lda ($10,x)` |
| Indirect indexed | `(value),y` | `lda ($10),y` |
| Relative | `label` | `bne loop` |

### Zero page or absolute

R65 uses the zero page form when the instruction has one and the value is known and between 0 and 255, or the
label comes from a `.zp { }` block and fits. A label defined **later** than its use gets the absolute form, with a warning:

```text
Warning: Using absolute mode for zeropage argument. Try defining involved labels before this statement.
```

Define zero page labels before they are used, mark them with `.zp`, or force the mode with a suffix. Labels in a `.zp`
block get the zero page form even when they're defined later:

```asm
* = $1000
    lda zpReg1      // a5 10
    sta zpReg2      // 85 11

* = $10 "Zero page" virtual
.zp {
zpReg1: .byte 0
zpReg2: .byte 0
}
```

### Forcing a mode

A suffix directly after the mnemonic selects the mode:

| Suffix | Mode | Suffix | Mode |
|---|---|---|---|
| `.z` `.zp` | Zero page | `.a` `.abs` | Absolute |
| `.zx` `.zpx` | Zero page,x | `.ax` `.absx` | Absolute,x |
| `.zy` `.zpy` | Zero page,y | `.ay` `.absy` | Absolute,y |
| `.im` `.imm` | Immediate | `.i` `.ind` | Indirect |
| `.izx` `.izpx` | `(zp,x)` | `.izy` `.izpy` | `(zp),y` |
| `.r` `.rel` | Relative | | |

```asm
lda.a $10       // ad 10 00
lda.z $10       // a5 10
```

`.z` keeps only the low byte of the value: `lda.z $1234` gives `a5 34`. On an instruction without a zero page form it's an
error: `jmp.z $1000` reports `'jmp' does not support mode 'zeropage'`.

### Branches

A branch reaches −128 to 127 bytes from the instruction after it. Further away it's an error:

```text
Error: relative address is illegal (jump distance is too far: 200).
```

## 6. Labels and the program counter

### Setting the address

```asm
* = $1000               // a block named "Unnamed"
* = $2000 "Main"        // a block with a name
*=$3000 "Vars" virtual  // reserves addresses; no bytes are written
.pc = $4000 "Same"      // the same as *=
```

Without any `*=` the code starts at `$2000` in a block named `Default`. `*` in an expression is the current address:

```asm
* = $1000
    bne *               // a branch to itself
.print toHexString(*)
```

### Labels

```asm
* = $1000
start:
    jmp start
.label border = $d020   // a label with a given value
    inc border
```

A label is visible in the scope where it's defined and in the scopes inside it. Defining the same label twice in
one scope is an error. `@name:` defines a label in the root scope from anywhere.

### Multi labels

`!name:` can be defined any number of times. `!name+` means the next one and `!name-` the previous one. The name can
be empty, and repeating the sign skips further:

```asm
* = $1000
!:  dex
    bne !-          // the previous !
    jmp !+          // the next !
    nop
!:  jmp !++         // the second ! from here
!:  nop
!:  nop
!loop:
    bne !loop-
```

### Label scopes

A label directly before a `{ }` block, a macro call, an `.if` or a loop names that scope, so its labels can be
reached from outside with a dot. For an `.if`, the labels come from the branch that was taken:

```asm
* = $1000
player: {
    x: .byte 0
    y: .byte 0
}
    lda player.x

.macro clear() {
    start: lda #0
}
first: clear()
    jmp first.start

.label io = $d000 {
    nop
}

    jmp chosen.entry
chosen: .if (true) {
    entry: lda #0
} else {
    entry: nop
}
```

The iterations of a `.for` or `.while` loop are reached by index, also from code before the loop:

```asm
* = $1000
rows: .for (var i = 0; i < 3; i++) {
    row: .byte i
}
    lda rows[1].row
```

### Operand labels

A label inside an instruction's operand gives the address of the operand bytes, which is useful for self-modifying
code:

```asm
* = $1000
    lda value:#0        // "value" is the address of the #0 byte
    inc value
    jmp target:($2000)
.word target
```

### `.pseudopc`

Code that will be copied and run elsewhere: labels inside get addresses at the target, while the bytes stay here.

```asm
* = $1000
.pseudopc $c000 {
copied:
    jmp copied      // 4c 00 c0
}
```

### `.align`

Moves to the next multiple of a number, writing zeros:

```asm
* = $1001
.align $100         // now at $1100
```

## 7. Passes and forward references

R65 runs the whole program repeatedly. The log shows `flex pass N` for each pass and `Output pass` at the end. In each
pass, a label used before its definition takes its value from the pass before. The passes end when all values are
known and stable.

- **Labels can be used anywhere in their scope**, also before their definition. **`.var` and `.const` can only be used
  after their declaration**; before it they report `Unknown symbol`.
- Addresses can depend on values defined later: `.fill later, 0` works when `later` is defined below.
- A name that's used but never declared anywhere is reported right after the first pass:
  `Unknown symbol 'name'`.
- **`.print` runs once**, in the output pass. **`.printnow` prints in every pass**, which helps to debug the passes.
- A program that never settles fails with `Made no progress and can't solve the program`.

## 8. Data directives

| Directive | Writes |
|---|---|
| `.byte a, b` (`.by`) | 8 bit values |
| `.word a, b` (`.wo`) | 16 bit values, low byte first |
| `.dword a, b` (`.dw`) | 32 bit values, low byte first |
| `.text "abc", 65` (`.te`) | Strings in the current encoding; numbers as raw bytes; `null` writes nothing |
| `.fill n, expr` | `n` values of `expr`, each 8 bit |
| `.fillword n, expr` | `n` values of `expr`, each 16 bit |
| `.lohifill n, expr` | A table of low bytes followed by a table of high bytes |

In `.fill`, `.fillword` and `.lohifill`, the expression can use `i`, which counts from 0 to `n`−1. In `.fill` and
`.fillword` it can also be a list, which writes all items for each `i`:

```asm
* = $1000
sine:   .fill 256, 127 + 127 * sin(toRadians(i * 360 / 256))
pair:   .fill 3, [1, 2]         // 01 02 01 02 01 02
words:  .fillword 2, $1000 + i  // 00 10 01 10
```

A label before `.lohifill` gets the fields `lo` and `hi`:

```asm
* = $1000
screen: .lohifill 25, $0400 + i * 40
    ldx #3
    lda screen.lo,x
    sta $fb
    lda screen.hi,x
    sta $fc
```

## 9. Variables, constants, enums and structs

```asm
.var counter = 0        // can change
.var empty              // starts as null
.const WIDTH = 40       // can't change
.eval counter = counter + 1
```

Assigning to a constant is an error (`Can't assign to 'WIDTH' since it isn't a variable`). The content of a list or
hashtable in a `.const` can still change. `.lock()` locks it, and changing a locked list then reports `You can't modify a locked value`. The original lets a locked hashtable change, and so does R65.

A name can be defined once in a scope, whether as a label, a `.var`, a `.const` or a parameter of the macro, function or
pseudocommand: a second definition reports `The symbol 'x' is already defined`. This is checked before the first pass, so it
also covers macros that are never called and false `.if` branches. A `.namespace` opened again shares its names with the first.

`.var` and `.const` are visible in their own scope and the scopes inside it. A variable declared inside a block hides
an outer one with the same name until the block ends. Without `.var`, an assignment changes the outer variable.

### `.label`

`.label name = value` defines a named address. Unlike a constant, it can be used before its definition, and it's written to
the symbol and VICE files.

### `.enum`

Entries without a value count 0, 1, 2 and so on. An entry with a value doesn't move the counter:

```asm
.enum { IDLE, RUNNING, DONE = 10, FAILED }
.byte IDLE, RUNNING, DONE, FAILED   // 00 01 0a 02
```

### `.struct`

```asm
.struct Point { x, y }
.var p = Point(1, 2)
.var q = Point()        // fields start as null
.eval p.x = 9
.byte p.x, p.y          // 09 02
.print p.getStructName() + " has " + p.getNoOfFields() + " fields"
.print p.getFieldNames().get(0) // x
.eval p.set("y", 3)             // or by index: p.set(1, 3)
.print p.get("y")               // or by index: p.get(1)
```

### `.define`

`.define a, b { ... }` runs a block in its own scope and makes only the listed names visible afterwards, as locked constants.
Bytes can't be written inside the block. It's a good place to build a list or hashtable step by step.

```asm
.define half, double {
    .var base = 10
    .var half = base / 2
    .var double = base * 2
}
.byte half, double      // 05 14
```

## 10. Control flow

### `.if` and `.else`

The parentheses are required. The condition must be a boolean.

```asm
.const DEBUG = false

.if (DEBUG) {
    inc $d020
} else .if (2 > 1) {
    sei
} else {
    cli
}

.if (!DEBUG) nop
```

Each branch is its own scope.

### `.for`

The first and last part of the header can be empty or hold several expressions:

```asm
.for (var i = 0, var j = 5; i < 2; i++, j--) {
    .byte i, j          // 00 05 01 04
}
```

A `var` in the header is local to the loop. Without `var`, the header uses an outer variable, which keeps its final value:

```asm
.var n = 0
.for (n = 0; n < 3; n++) { }
.byte n                 // 03
```

### `.while`

```asm
.var w = 0
.while (w < 2) {
    .byte w
    .eval w++
}
```

### Unrolling code

Loops can write instructions, which makes unrolled code short to write:

```asm
* = $1000
.for (var row = 0; row < 25; row++) {
    sta $0400 + row * 40,x
}
```

## 11. Functions, macros and pseudocommands

All three can be used before their definition. Macros and functions with the same name but a different number of
parameters are separate definitions. Macros and functions can call others and themselves, up to 10000 levels deep.

### Functions

A function computes a value. It can't write bytes. Without a `.return` it gives `null`.

```asm
.function square(n) {
    .return n * n
}
.function square(a, b) {
    .return a * b
}
.byte square(3), square(2, 5)   // 09 0a
```

### Macros

A macro writes code each time it's called. Each call has its own scope, so its labels never collide:

```asm
* = $1000
.macro wait(n) {
    ldx #n
loop:
    dex
    bne loop
}
wait(10)
wait(20)
```

Parameters are values and are evaluated once at the call. A macro from a label-named call exposes its labels as
described in [Label scopes](#label-scopes). If a name could be read as a label or a pseudocommand, `:name(...)` calls
the macro explicitly.

### Pseudocommands

A pseudocommand is called like an instruction, with arguments separated by `:`. Each argument keeps its addressing mode:

```asm
* = $1000
.pseudocommand mov src : dst {
    lda src
    sta dst
}
mov #1 : $d020
mov $10,x : $20,y
mov ($10),y : ($20,x)
```

Missing arguments have the type `AT_NONE`. Too many arguments are an error. A call must stay on one line. `:name` calls
the pseudocommand even when an instruction has the same name:

```asm
* = $1000
.pseudocommand adc a : b : target {
    clc
    lda a
    .byte $69, b.getValue()     // the real adc #
    sta target
}
adc #$10                        // the instruction
:adc $20 : #1 : $21             // the pseudocommand
```

Inside the body, an argument has two methods: `.getType()` and `.getValue()`. `CmdArgument(type, value)` builds a new
argument, for example to reach the next address:

```asm
* = $1000
.pseudocommand inc16 arg {
    inc arg
    bne !+
    inc CmdArgument(arg.getType(), arg.getValue() + 1)
!:
}
inc16 $1000
inc16 $2000,x
```

| Argument | `getType()` | Constant |
|---|---|---|
| none | 0 | `AT_NONE` |
| `#1` | 1 | `AT_IMMEDIATE` |
| `$10`, `$1000` | −1 | `AT_ABSOLUTE` |
| `$10,x`, `$1000,x` | −2 | `AT_ABSOLUTEX` |
| `$10,y`, `$1000,y` | −3 | `AT_ABSOLUTEY` |
| `($1000)` | −4 | `AT_INDIRECT` |
| `($10),y` | 6 | `AT_IZEROPAGEY` |
| `($10,x)` | −5 | No matching constant, see [Limitations](#22-limitations) |

The constants `AT_ZEROPAGE` (2) and `AT_IZEROPAGEX` (5) exist, but no argument reports them, so comparing `($10,x)` with
`AT_IZEROPAGEX` is false.

## 12. Scopes and namespaces

### Scopes

`{ }` opens a scope. Labels and variables inside aren't visible outside, except by a [named scope](#label-scopes):

```asm
* = $1000
{
    loop: jmp loop
}
{
    loop: jmp loop      // a different label
}
```

### Namespaces

```asm
* = $1000
.namespace sound {
    .label VOLUME = 15
    init: rts
    .function level() { .return VOLUME }
    .macro beep() { nop }
    beep()
}
    jsr sound.init
    lda #sound.VOLUME
```

- Labels, also those made with `.label`, and inner namespaces are reached from outside with `sound.name`. A `.const` or `.var`
  can't be: `sound.X` reports `Variable 'X' not present on value of type 'Namespace'`. The same holds for scopes of labels.
- **Functions and macros of a namespace can only be called inside it.** `sound.level()` reports `Unknown function : level`,
  and `sound.beep()` as a statement is a syntax error.
- Opening the same namespace again continues it.
- `.namespace name` without `{ }` and `.filenamespace name` apply to the rest of the file.

### The root scope

`@` puts a definition in the root scope, or reads a name from there: `@name` and `@name()` in expressions, `@name()` or `:@name()` for a
macro call. This publishes names from a namespace:

```asm
* = $1000
.namespace lib {
    .macro @publicMacro() { nop }
    .function @publicFunction() { .return 7 }
    .const @PUBLIC_CONST = 5
    .var @publicVar = 4
    .struct @PublicStruct { a }
    @publicLabel: nop
}
publicMacro()
.byte publicFunction(), PUBLIC_CONST, publicVar, <publicLabel
```

## 13. The preprocessor

These directives run first, line by line, before parsing. They start with `#`:

| Directive | Meaning |
|---|---|
| `#define NAME` | Define a symbol. Also from the command line with `-define NAME` |
| `#undef NAME` | Remove a symbol |
| `#if EXPR` | Keep the following lines if `EXPR` is true |
| `#elif EXPR` | Otherwise, if |
| `#else` | Otherwise |
| `#endif` | End of the condition |
| `#import "file"` | Include a source file |
| `#importif EXPR "file"` | Include it if `EXPR` is true |
| `#importonce` | Include the current file only once |

Expressions use symbols with `!`, `&&`, `||`, `==`, `!=` and parentheses. A symbol is true if it's defined. These symbols
have no value, so they can't be used in normal expressions. For values from the command line, use
[`cmdLineVars`](#values-from-the-command-line).

```asm
#define DEBUG

#if DEBUG && !RELEASE
    .print "debug build"
#elif RELEASE
    .print "release build"
#else
    .print "plain build"
#endif
```

Imported files are searched next to the importing file, in the main source file's folder, then in each `-libdir`.
`.import source "file"` and `.importonce` work like `#import` and `#importonce`, but the preprocessor forms are preferred.

```asm
#import "helper.asm"
#import "helper.asm"    // skipped if helper.asm has #importonce
```

## 14. Script library

### Math

`abs` `acos` `asin` `atan` `atan2` `cbrt` `ceil` `cos` `cosh` `exp` `expm1` `floor` `hypot` `IEEEremainder` `log`
`log10` `log1p` `max` `min` `mod` `pow` `random` `round` `signum` `sin` `sinh` `sqrt` `tan` `tanh` `toDegrees`
`toRadians`

- Angles are in radians.
- `log` is the natural logarithm.
- `max`, `min`, `atan2`, `hypot`, `pow`, `mod` and `IEEEremainder` take two arguments.
- `random()` gives a value from 0 up to 1. `setSeed(n)` sets its seed.
- `round` rounds halves up, so `round(2.5)` is 3 and `round(-2.5)` is −2.
- `mod` keeps the sign of the first argument, so `mod(-7, 3)` is −1.

Constants: `PI`, `E`.

### Numbers to text

| Function | Example | Result |
|---|---|---|
| `toIntString(n)` | `toIntString(7)` | `7` |
| `toIntString(n, width)` | `toIntString(7, 3)` | `  7` (padded with spaces) |
| `toHexString(n, width)` | `toHexString(255, 4)` | `00ff` (padded with zeros) |
| `toBinaryString(n, width)` | `toBinaryString(5, 8)` | `00000101` |
| `toOctalString(n, width)` | `toOctalString(8)` | `10` |

The width is optional in all of them.

### Strings

| Method | Result |
|---|---|
| `.size()` | Length |
| `.charAt(i)` | The character at `i` |
| `.substring(a, b)` | From `a` up to, but not including, `b` |
| `.toLowerCase()`, `.toUpperCase()` | Changed case |
| `.asNumber()`, `.number()` | The number in the text |
| `.asNumber(radix)` | The same in another base: `"f".asNumber(16)` is 15 |
| `.asBoolean()` | True if the text is `true` |
| `.string()` | Any value as text |

### Lists

`List()` is empty, and `List(n)` has `n` items set to `null`.

| Method | Result |
|---|---|
| `.add(a, b, ...)` | Adds items; returns the list |
| `.addAll(list)` | Adds all items of another list |
| `.get(i)`, `list[i]` | The item at `i` |
| `.set(i, v)` | Replaces the item at `i` |
| `.remove(i)` | Removes the item at `i` and returns it |
| `.size()` | Number of items |
| `.sort()`, `.reverse()`, `.shuffle()` | Reorders the list |
| `.lock()` | Prevents further changes |

A list has to hold only numbers for `.sort()`.

```asm
.var items = List().add(3, 1, 2)
.eval items.sort()
.print "" + items           // [1,2,3]
.print "" + items.get(0)    // 1
```

### Hashtables

| Method | Result |
|---|---|
| `.put(k, v, ...)` | Adds one or more pairs; returns the table |
| `.get(k)` | The value, or `null` |
| `.containsKey(k)` | True if the key exists |
| `.remove(k)` | Removes a key |
| `.keys()` | List of the keys |
| `.lock()` | Prevents further changes |

A key is compared by its text, so `get(1)` and `get("1")` find the same entry.

```asm
.var colors = Hashtable().put("sky", BLUE, "grass", GREEN)
.byte colors.get("sky")     // 06
```

### Values from the command line

`:name=value` options fill the hashtable `cmdLineVars`. The values are strings:

```asm
.var level = cmdLineVars.containsKey("level") ? cmdLineVars.get("level").asNumber() : 1
.print "level " + level
```

```bash
r65 game.asm :level=3
```

### Vectors and matrices

| Expression | Result |
|---|---|
| `Vector(x, y, z)` | A vector |
| `.get(i)`, `.getX()`, `.getY()`, `.getZ()` | A coordinate |
| `v + w`, `v - w`, `-v` | Vector arithmetic |
| `v * w` | Dot product |
| `v.X(w)` | Cross product |
| `v * n`, `v / n` | Scaling |
| `Matrix()` | The identity matrix |
| `RotationMatrix(aX, aY, aZ)` | Rotation; angles in radians |
| `ScaleMatrix(x, y, z)`, `MoveMatrix(x, y, z)` | Scale, move |
| `PerspectiveMatrix(z)` | Perspective |
| `.get(row, col)` | A cell |
| `.set(row, col, v)` | Changes this cell and returns the matrix |
| `m * n` | Matrix product |
| `m * v` | Transformed vector, divided by w |

```asm
.var turn = RotationMatrix(0, 0, toRadians(90))
.var moved = MoveMatrix(1, 0, 0) * Vector(1, 2, 3)
.print "" + moved           // (2.0,2.0,3.0)
```

### Source information

`getFilename()` is the current file's name, `getPath()` its full path, and `getNamespace()` the current namespace
(`<RootNS>` at the top).

`asmCommandSize(opcode)` gives the size of an instruction.

### Constants

- Colors: `BLACK` `WHITE` `RED` `CYAN` `PURPLE` `GREEN` `BLUE` `YELLOW` `ORANGE` `BROWN` `LIGHT_RED` `DARK_GRAY`
  `GRAY` `LIGHT_GREEN` `LIGHT_BLUE` `LIGHT_GRAY`. The `GREY` spellings also work.
- Opcodes of the default 6502 table: the mnemonic for implied instructions (`NOP`), otherwise the mnemonic and the mode:
  `_IMM` `_ZP` `_ZPX` `_ZPY` `_IZPX` `_IZPY` `_ABS` `_ABSX` `_ABSY` `_IND` `_REL`.

```asm
* = $1000
.byte LDA_IMM, 1            // a9 01
.byte NOP                   // ea
.print "" + asmCommandSize(LDA_ABS)     // 3
```

- Argument types: see [Pseudocommands](#pseudocommands).
- File formats: see [Binary files](#binary-files).

### Writing text files

`createFile(name)` makes a text file in the output folder, with the methods `.write(text)`, `.writeln(text)` and `.writeln()`.
It needs the `-afo` option; otherwise it reports `File output is blocked`.

## 15. Text encodings

`.encoding "name"` selects how `.text` and `'c'` characters turn into bytes. The default is `screencode_mixed`.

| Encoding | `"aA@1"` gives |
|---|---|
| `screencode_mixed` | `01 41 00 31` |
| `screencode_upper` | `61 01 00 31` |
| `petscii_mixed` | `41 c1 40 31` |
| `petscii_upper` | `61 41 40 31` |
| `ascii` | `61 41 40 31` |

```asm
.encoding "petscii_mixed"
.text "Hello"
.byte 0
.encoding "screencode_mixed"
.text @"raw byte: \$ff"
```

## 16. Memory blocks and segments

### Memory blocks

A memory block is a run of bytes with a start address. `* = addr "name"` starts one, and `.memblock "name"` starts one at
the current address. The main output file joins the blocks of the `Default` segment, and fills the gaps with `-fillbyte`.
Blocks must not overlap: R65 reports an error unless `-aom` or `allowOverlap` is given, and then later blocks win, with a
warning.

### Segments

A segment is a list of memory blocks, which lets code and data be written in any order and placed separately.

```asm
.segmentdef Code [start=$0810]
.segmentdef Data [startAfter="Code", align=$100]
.segmentdef Vars [start=$c000, virtual]

.file [name="game.prg", segments="Code,Data"]

.segment Code "Main"
start:
    lda table
    sta counter
    rts

.segment Data "Tables"
table: .byte 1, 2

.segment Vars "Variables"
counter: .byte 0
```

- `.segmentdef Name [params]` defines a segment. Defining one twice is an error.
- `.segment Name` switches to a segment. `.segment Name [params] "block"` also defines it the first time and opens a
  block. Back to the main program: `.segment Default`.
- The main output file is only written if the `Default` segment has bytes or no other segment exists.

### Segment parameters

| Parameter | Meaning |
|---|---|
| `start=addr` | Where the segment starts |
| `startAfter="name"` | Start right after another segment |
| `align=n` | Align the start |
| `min=addr`, `max=addr` | Allowed range, both ends included. A block outside it is an error, also a virtual one and in a segment that nothing writes |
| `fill` | Fill the whole `min` to `max` range; needs both |
| `fillByte=n` | The byte used for filling |
| `virtual` | Addresses without output |
| `allowOverlap` | Blocks may overlap; later blocks win |
| `hide` | Not shown in the memory map |
| `outPrg="file"` | Write the segment to a prg file. `%o` stands for the source file's name |
| `outBin="file"` | Write the segment to a raw file. `%o` stands for the source file's name |
| `segments="a,b"` | Include the blocks of other segments |
| `prgFiles="a,b"` | Include prg files, at their own addresses |
| `sidFiles="a,b"` | Include the music of SID files, at their own addresses |
| `modify="BasicUpstart"` | Add a BASIC line at `$0801`, with `_start=addr` (or `marg1=addr`) as its target |
| `dest="text"` | Accepted, has no effect |

`.file`, `.segmentout` and disk file entries pass their bytes through a temporary segment, so they accept `segments`,
`prgFiles`, `sidFiles`, `allowOverlap`, `fill`, `fillByte`, `min`, `max`, `hide`, `modify` and its arguments too. A parameter
written without a value, like `fill`, means `fill=true`. Overlaps are checked inside one segment, so two segments can use
the same addresses until they're combined.

A segment's blocks are written nowhere unless it's named by `outPrg`, `outBin`, `.file`, `.disk` or `.segmentout`. `-bytedump`
still shows them. The memory map also lists the blocks from `prgFiles` and `sidFiles`, under the file's name.

```asm
.segmentdef Rom [start=$e000, min=$e000, max=$e00f, fill, fillByte=$ea, outPrg="rom.prg"]
.segmentdef Raw [start=$2000, outBin="raw.bin"]
.segmentdef Boot [start=$0810, modify="BasicUpstart", _start=$0810, outPrg="boot.prg"]

.segment Rom
    .byte 1
.segment Raw
    .byte 2
.segment Boot
    rts
```

Patching a program works by combining a segment holding the original with one holding the changes, because later blocks
win:

```asm
.segmentdef Base [prgFiles="prog.prg"]
.segmentdef Patch []
.file [name="patched.prg", segments="Base,Patch", allowOverlap]

.segment Patch
* = $3001 "New code"
    lda #$ff
```

### `.segmentout`

Places the bytes of other segments, flattened, at the current address. Their labels keep the addresses of their own segment,
which is how code is prepared for copying elsewhere:

```asm
.segmentdef Packed [start=$2000]
.segment Packed
    .byte 1, 2, 3
.segment Default
* = $1000 "Copy"
    .segmentout [segments="Packed"]
```

## 17. Output files and disk images

| File | When |
|---|---|
| `name.prg` | By default: 2 byte start address, then the bytes |
| `name.bin` | With `-binfile` |
| `name_block.prg` | With `-mbfiles`, per block |
| `name.vs` | With `-vicesymbols`: VICE labels, breakpoints and watchpoints |
| `name.sym` | With `-symbolfile`: labels that another source can `#import` |
| `ByteDump.txt` | With `-bytedump`: addresses, bytes and source |

`name` is the source file name without its extension. Files go next to the source unless `-odir` is given.

A `.sym` file holds the labels of the root namespace, with nested label scopes. Namespaces appear in it, but empty. See
[Limitations](#22-limitations).

### `.file`

Writes selected segments to a file of their own. `%o` in the name stands for the source file's name.

```asm
.segmentdef Data [start=$2000]
.file [name="%o_data.prg", segments="Data"]
.file [name="data.bin", type="bin", segments="Data"]
.file [name="parts.prg", segments="Data", mbfiles]

.segment Data "Table"
    .byte 1, 2, 3
```

| Parameter | Meaning |
|---|---|
| `name="file"` | The file name; required |
| `type="prg"` or `"bin"` | With or without the start address; default `prg` |
| `mbfiles` | One file per block: `parts_Table.prg` |

### `.disk`

Writes a 35 track `.d64` image:

```asm
.segmentdef Code [start=$0810]
.segmentdef Data [start=$2000]

.disk [filename="game.d64", name="MY GAME", id="2A"] {
    [name="LOADER", type="prg", segments="Code"],
    [name="DATA",   type="seq", segments="Data"],
    [name="SECRET", type="prg", hide, segments="Data"],
}

.segment Code
    rts
.segment Data
    .byte 1
```

| Disk parameter | Meaning |
|---|---|
| `filename="x.d64"` | The image file; required. `%o` stands for the source file's name |
| `name="text"` | Disk name; default `UNNAMED` |
| `id="text"` | Disk id |
| `interleave=n` | Sector step; default 10 |
| `format="commodore"` | `commodore` has 35 tracks; `speeddos` and `dolphindos` have 40 |
| `dontSplitFilesOverDir` | A file that would have sectors on both sides of track 18 moves after it |
| `storeFilesInDir` | Files also use the free sectors of track 18, when they get there after track 17 |
| `showInfo` | Print the name, the size in sectors and the start track and sector of each file |

| File parameter | Meaning |
|---|---|
| `name="text"` | File name, up to 16 characters |
| `type="prg"` | `del`, `seq`, `prg`, `usr` or `rel`; a trailing `<` locks the file, as in `"prg<"` |
| `hide` | Stored on the disk but not listed in the directory |
| `noStartAddr` | Leave out the start address. By default only `prg` files have one; `noStartAddr=false` keeps it on the others |
| `interleave=n` | Sector step for this file |
| `segments`, `prgFiles`, `sidFiles` | Where the bytes come from |

A disk holds up to 152 visible files. A full disk reports `Disk is full!`.

## 18. Importing data

### `.import`

| Directive | Writes |
|---|---|
| `.import binary "file", offset, length` | The bytes of a file |
| `.import c64 "file", offset, length` | The same, skipping the 2 byte start address |
| `.import text "file", offset, length` | The file as text, in the current encoding |
| `.import source "file"` | Includes source, like `#import` |

`offset` and `length` are optional.

```asm
* = $1000
.import binary "data.bin"
.import binary "data.bin", 1, 2
.import c64 "data.bin"
```

### Binary files

`LoadBinary(file)` reads a file into a value, with the methods `.getSize()`, `.get(i)` (signed) and `.uget(i)` (unsigned).

```asm
* = $1000
.var data = LoadBinary("data.bin")
.fill data.getSize(), data.uget(i)
```

A second argument names the parts of the file, in the form `"C64FILE, name=offset, ..."`. `C64FILE` skips the 2 byte start
address. Each name gets its own methods:

```asm
* = $1000
.var file = LoadBinary("data.bin", "first=0, second=2")
.byte file.getfirstSize(), file.ugetsecond(0)
```

The constants `BF_C64FILE`, `BF_KOALA`, `BF_FLI`, `BF_BITMAP_SINGLECOLOR` and `BF_DOODLE` are templates for common formats:

| Constant | Parts |
|---|---|
| `BF_KOALA` | `Bitmap`, `ScreenRam`, `ColorRam`, `BackgroundColor` |
| `BF_FLI` | `ColorRam`, `ScreenRam`, `Bitmap` |
| `BF_BITMAP_SINGLECOLOR` | `Bitmap`, `ScreenRam` |
| `BF_DOODLE` | `ColorRam`, `Bitmap` |

So a Koala picture gives `.getBitmap(i)`, `.getBitmapSize()` and so on.

### SID files

`LoadSid(file)` reads a SID tune. The fields are `.header`, `.version`, `.location`, `.init`, `.play`, `.songs`,
`.startSong`, `.speed`, `.name`, `.author`, `.copyright`, `.size`, `.flags`, `.startpage` and `.pagelength`.
`.getData(i)` gives a byte of the music.

```asm
.var music = LoadSid("tune.sid")
* = music.location "Music"
.fill music.size, music.getData(i)
.print "init=$" + toHexString(music.init) + " play=$" + toHexString(music.play)
```

### Pictures

`LoadPicture(file)` reads a gif, jpg or png picture.

| Member | Result |
|---|---|
| `.width`, `.height` | Size in pixels |
| `.getPixel(x, y)` | Color as `$rrggbb` |
| `.getSinglecolorByte(x, y)` | 8 pixels from `x` (counted in bytes) and `y`; bit = color index & 1 |
| `.getMulticolorByte(x, y)` | 4 double pixels; 2 bits = color index & 3 |

The color index comes from the color table. Without a second argument, the table starts with black, followed by the first
new colors in pixel order, up to 4 colors. With `LoadPicture(file, list)`, the list gives the table:

```asm
* = $2000
.var logo = LoadPicture("picture.png", List().add($000000, $ffffff, $ff0000))
.fill logo.width / 8, logo.getSinglecolorByte(i, 0)
```

## 19. Debugging and testing

| Directive | Meaning |
|---|---|
| `.print expr` | Prints in the output pass |
| `.printnow expr` | Prints in every pass |
| `.error msg` | Stops with an error |
| `.errorif cond, msg` | Stops with an error if `cond` is true |
| `.assert "name", a, b` | Checks that `a` equals `b` |
| `.asserterror "name", expr` | Checks that `expr` gives an error |
| `.break [arg]` | VICE breakpoint at the next instruction (with `-vicesymbols`) |
| `.watch a [, [b] [, arg]]` | VICE watchpoint on an address or a range (with `-vicesymbols`) |

```asm
* = $1000
.assert "two", 1 + 1, 2
.asserterror "bad type", 1 + "a"
.errorif * > $2000, "the code is too long"

    nop
.break
    nop
.break "if A == 1"
.watch $d020
.watch $1000, $1010, "store"
.watch $d021, , "hex8"

    beq short
.errorif (>*) != (>short), "this branch crosses a page"
    nop
short:
```

Both also take code blocks. They are assembled on the side and add nothing to the program. `.assert` compares their bytes,
and `.asserterror` expects the block to give an error:

```asm
.assert "same code", { lda $1000 }, { lda $1000 }
.assert "with a loop", {
    .for (var i = 0; i < 2; i++)
        sta $0400 + i
}, {
    sta $0400
    sta $0401
}
.asserterror "must fail", { lda #"text" }
```

The log shows the bytes: `same code -- OK. | 2000:ad,00,10`, or `x -- FAILED! | 2000:ad,00,10 -- 2000:ae,00,10` when they differ.

The result is listed at the end of the log, as `Made 2 asserts , 0 failed`. **Failed asserts don't stop the assembly or
change the exit code**, so check the log, or use `.errorif` to make a build fail.

`-vicesymbols` writes these lines into the `.vs` file:

```text
al C:1000 .lbl
break 1001 if A == 1
watch d020
watch store 1000 1010
watch hex8 d021
```

`-bytedump` shows each statement's address, bytes and source, which is the quickest way to see what a macro produces.

## 20. CPUs and opcode sets

Choose with `-cpu <name>` or, in the source, `.cpu <name>`.

| Name | Instructions |
|---|---|
| `_6502` | Standard 6502 and illegal opcodes; the default |
| `_6502NoIllegals` | Standard 6502 only (`-excludeillegal`) |
| `dtv` | C64 DTV: also `sac`, `sir` and `bra` (`-dtv`) |
| `_65c02` | 65C02 |

The tables give the opcode in hexadecimal, per addressing mode. They were generated by running R65 on every combination.

### Standard 6502

| Instr. | impl | imm | zp | zp,x | zp,y | (zp,x) | (zp),y | abs | abs,x | abs,y | (ind) | rel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `adc` |  | 69 | 65 | 75 |  | 61 | 71 | 6D | 7D | 79 |  |  |
| `and` |  | 29 | 25 | 35 |  | 21 | 31 | 2D | 3D | 39 |  |  |
| `asl` | 0A |  | 06 | 16 |  |  |  | 0E | 1E |  |  |  |
| `bcc` |  |  |  |  |  |  |  |  |  |  |  | 90 |
| `bcs` |  |  |  |  |  |  |  |  |  |  |  | B0 |
| `beq` |  |  |  |  |  |  |  |  |  |  |  | F0 |
| `bit` |  |  | 24 |  |  |  |  | 2C |  |  |  |  |
| `bmi` |  |  |  |  |  |  |  |  |  |  |  | 30 |
| `bne` |  |  |  |  |  |  |  |  |  |  |  | D0 |
| `bpl` |  |  |  |  |  |  |  |  |  |  |  | 10 |
| `brk` | 00 |  |  |  |  |  |  |  |  |  |  |  |
| `bvc` |  |  |  |  |  |  |  |  |  |  |  | 50 |
| `bvs` |  |  |  |  |  |  |  |  |  |  |  | 70 |
| `clc` | 18 |  |  |  |  |  |  |  |  |  |  |  |
| `cld` | D8 |  |  |  |  |  |  |  |  |  |  |  |
| `cli` | 58 |  |  |  |  |  |  |  |  |  |  |  |
| `clv` | B8 |  |  |  |  |  |  |  |  |  |  |  |
| `cmp` |  | C9 | C5 | D5 |  | C1 | D1 | CD | DD | D9 |  |  |
| `cpx` |  | E0 | E4 |  |  |  |  | EC |  |  |  |  |
| `cpy` |  | C0 | C4 |  |  |  |  | CC |  |  |  |  |
| `dec` |  |  | C6 | D6 |  |  |  | CE | DE |  |  |  |
| `dex` | CA |  |  |  |  |  |  |  |  |  |  |  |
| `dey` | 88 |  |  |  |  |  |  |  |  |  |  |  |
| `eor` |  | 49 | 45 | 55 |  | 41 | 51 | 4D | 5D | 59 |  |  |
| `inc` |  |  | E6 | F6 |  |  |  | EE | FE |  |  |  |
| `inx` | E8 |  |  |  |  |  |  |  |  |  |  |  |
| `iny` | C8 |  |  |  |  |  |  |  |  |  |  |  |
| `jmp` |  |  |  |  |  |  |  | 4C |  |  | 6C |  |
| `jsr` |  |  |  |  |  |  |  | 20 |  |  |  |  |
| `lda` |  | A9 | A5 | B5 |  | A1 | B1 | AD | BD | B9 |  |  |
| `ldx` |  | A2 | A6 |  | B6 |  |  | AE |  | BE |  |  |
| `ldy` |  | A0 | A4 | B4 |  |  |  | AC | BC |  |  |  |
| `lsr` | 4A |  | 46 | 56 |  |  |  | 4E | 5E |  |  |  |
| `nop` | EA |  |  |  |  |  |  |  |  |  |  |  |
| `ora` |  | 09 | 05 | 15 |  | 01 | 11 | 0D | 1D | 19 |  |  |
| `pha` | 48 |  |  |  |  |  |  |  |  |  |  |  |
| `php` | 08 |  |  |  |  |  |  |  |  |  |  |  |
| `pla` | 68 |  |  |  |  |  |  |  |  |  |  |  |
| `plp` | 28 |  |  |  |  |  |  |  |  |  |  |  |
| `rol` | 2A |  | 26 | 36 |  |  |  | 2E | 3E |  |  |  |
| `ror` | 6A |  | 66 | 76 |  |  |  | 6E | 7E |  |  |  |
| `rti` | 40 |  |  |  |  |  |  |  |  |  |  |  |
| `rts` | 60 |  |  |  |  |  |  |  |  |  |  |  |
| `sbc` |  | E9 | E5 | F5 |  | E1 | F1 | ED | FD | F9 |  |  |
| `sec` | 38 |  |  |  |  |  |  |  |  |  |  |  |
| `sed` | F8 |  |  |  |  |  |  |  |  |  |  |  |
| `sei` | 78 |  |  |  |  |  |  |  |  |  |  |  |
| `sta` |  |  | 85 | 95 |  | 81 | 91 | 8D | 9D | 99 |  |  |
| `stx` |  |  | 86 |  | 96 |  |  | 8E |  |  |  |  |
| `sty` |  |  | 84 | 94 |  |  |  | 8C |  |  |  |  |
| `tax` | AA |  |  |  |  |  |  |  |  |  |  |  |
| `tay` | A8 |  |  |  |  |  |  |  |  |  |  |  |
| `tsx` | BA |  |  |  |  |  |  |  |  |  |  |  |
| `txa` | 8A |  |  |  |  |  |  |  |  |  |  |  |
| `txs` | 9A |  |  |  |  |  |  |  |  |  |  |  |
| `tya` | 98 |  |  |  |  |  |  |  |  |  |  |  |

### Illegal opcodes

The set `_6502` adds these to the standard table. Names in one row are aliases.

| Instr. | impl | imm | zp | zp,x | zp,y | (zp,x) | (zp),y | abs | abs,x | abs,y | (ind) | rel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `ahx`, `sha` |  |  |  |  |  |  | 93 |  |  | 9F |  |  |
| `alr`, `asr` |  | 4B |  |  |  |  |  |  |  |  |  |  |
| `anc` |  | 0B |  |  |  |  |  |  |  |  |  |  |
| `anc2` |  | 2B |  |  |  |  |  |  |  |  |  |  |
| `arr` |  | 6B |  |  |  |  |  |  |  |  |  |  |
| `axs`, `sbx` |  | CB |  |  |  |  |  |  |  |  |  |  |
| `dcp`, `dcm` |  |  | C7 | D7 |  | C3 | D3 | CF | DF | DB |  |  |
| `isc`, `ins`, `isb` |  |  | E7 | F7 |  | E3 | F3 | EF | FF | FB |  |  |
| `las`, `lae`, `lds` |  |  |  |  |  |  |  |  |  | BB |  |  |
| `lax`, `lxa` |  | AB | A7 |  | B7 | A3 | B3 | AF |  | BF |  |  |
| `nop` | EA | 80 | 04 | 14 |  |  |  | 0C | 1C |  |  |  |
| `rla` |  |  | 27 | 37 |  | 23 | 33 | 2F | 3F | 3B |  |  |
| `rra` |  |  | 67 | 77 |  | 63 | 73 | 6F | 7F | 7B |  |  |
| `sax` |  |  | 87 |  | 97 | 83 |  | 8F |  |  |  |  |
| `sbc2` |  | EB |  |  |  |  |  |  |  |  |  |  |
| `shx` |  |  |  |  |  |  |  |  |  | 9E |  |  |
| `shy` |  |  |  |  |  |  |  |  | 9C |  |  |  |
| `slo` |  |  | 07 | 17 |  | 03 | 13 | 0F | 1F | 1B |  |  |
| `sre` |  |  | 47 | 57 |  | 43 | 53 | 4F | 5F | 5B |  |  |
| `tas`, `shs` |  |  |  |  |  |  |  |  |  | 9B |  |  |
| `xaa`, `ane` |  | 8B |  |  |  |  |  |  |  |  |  |  |

### DTV

The set `dtv` adds these to the illegal set: `bra` (relative, 12), `sac` (immediate, 32) and `sir` (immediate, 42).

### 65C02

The set `_65c02` has no illegal opcodes. These rows are new or changed compared to the standard table; `(zp)` is
written `lda ($10)`, `zp,rel` is `bbr0 $10,label` and `(abs,x)` is `jmp ($1000,x)`.

| Instr. | impl | imm | zp | zp,x | zp,y | (zp,x) | (zp),y | abs | abs,x | abs,y | (ind) | rel | (zp) | zp,rel | (abs,x) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `adc` |  | 69 | 65 | 75 |  | 61 | 71 | 6D | 7D | 79 |  |  | 72 |  |  |
| `and` |  | 29 | 25 | 35 |  | 21 | 31 | 2D | 3D | 39 |  |  | 32 |  |  |
| `bbr0` |  |  |  |  |  |  |  |  |  |  |  |  |  | 0F |  |
| `bbr1` |  |  |  |  |  |  |  |  |  |  |  |  |  | 1F |  |
| `bbr2` |  |  |  |  |  |  |  |  |  |  |  |  |  | 2F |  |
| `bbr3` |  |  |  |  |  |  |  |  |  |  |  |  |  | 3F |  |
| `bbr4` |  |  |  |  |  |  |  |  |  |  |  |  |  | 4F |  |
| `bbr5` |  |  |  |  |  |  |  |  |  |  |  |  |  | 5F |  |
| `bbr6` |  |  |  |  |  |  |  |  |  |  |  |  |  | 6F |  |
| `bbr7` |  |  |  |  |  |  |  |  |  |  |  |  |  | 7F |  |
| `bbs0` |  |  |  |  |  |  |  |  |  |  |  |  |  | 8F |  |
| `bbs1` |  |  |  |  |  |  |  |  |  |  |  |  |  | 9F |  |
| `bbs2` |  |  |  |  |  |  |  |  |  |  |  |  |  | AF |  |
| `bbs3` |  |  |  |  |  |  |  |  |  |  |  |  |  | BF |  |
| `bbs4` |  |  |  |  |  |  |  |  |  |  |  |  |  | CF |  |
| `bbs5` |  |  |  |  |  |  |  |  |  |  |  |  |  | DF |  |
| `bbs6` |  |  |  |  |  |  |  |  |  |  |  |  |  | EF |  |
| `bbs7` |  |  |  |  |  |  |  |  |  |  |  |  |  | FF |  |
| `bit` |  | 89 | 24 | 34 |  |  |  | 2C | 3C |  |  |  |  |  |  |
| `bra` |  |  |  |  |  |  |  |  |  |  |  | 80 |  |  |  |
| `cmp` |  | C9 | C5 | D5 |  | C1 | D1 | CD | DD | D9 |  |  | D2 |  |  |
| `dec` | 3A |  | C6 | D6 |  |  |  | CE | DE |  |  |  |  |  |  |
| `eor` |  | 49 | 45 | 55 |  | 41 | 51 | 4D | 5D | 59 |  |  | 52 |  |  |
| `inc` | 1A |  | E6 | F6 |  |  |  | EE | FE |  |  |  |  |  |  |
| `jmp` |  |  |  |  |  |  |  | 4C |  |  | 6C |  |  |  | 7C |
| `lda` |  | A9 | A5 | B5 |  | A1 | B1 | AD | BD | B9 |  |  | B2 |  |  |
| `ora` |  | 09 | 05 | 15 |  | 01 | 11 | 0D | 1D | 19 |  |  | 12 |  |  |
| `phx` | DA |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `phy` | 5A |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `plx` | FA |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `ply` | 7A |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb0` |  |  | 07 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb1` |  |  | 17 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb2` |  |  | 27 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb3` |  |  | 37 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb4` |  |  | 47 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb5` |  |  | 57 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb6` |  |  | 67 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb7` |  |  | 77 |  |  |  |  |  |  |  |  |  |  |  |  |
| `sbc` |  | E9 | E5 | F5 |  | E1 | F1 | ED | FD | F9 |  |  | F2 |  |  |
| `smb0` |  |  | 87 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb1` |  |  | 97 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb2` |  |  | A7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb3` |  |  | B7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb4` |  |  | C7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb5` |  |  | D7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb6` |  |  | E7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb7` |  |  | F7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `sta` |  |  | 85 | 95 |  | 81 | 91 | 8D | 9D | 99 |  |  | 92 |  |  |
| `stp` | DB |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `stz` |  |  | 64 | 74 |  |  |  | 9C | 9E |  |  |  |  |  |  |
| `trb` |  |  | 14 |  |  |  |  | 1C |  |  |  |  |  |  |  |
| `tsb` |  |  | 04 |  |  |  |  | 0C |  |  |  |  |  |  |  |
| `wai` | CB |  |  |  |  |  |  |  |  |  |  |  |  |  |  |

### Example

```asm
.cpu _65c02
* = $1000
    phx
    stz $10
    lda ($10)
    inc
    bbr0 $10, *
    jmp ($1000,x)
    bra *
.cpu _6502
    lax $10
```

## 21. Error messages

An error shows up to three lines before the error line, a `^` under the column, the message, the position and, inside
macros and functions, where each call came from:

```text
.byte 256
lda nosuch
    ^

Error: Unknown symbol 'nosuch'
at line 2, column 5 in game.asm
```

| Message | Usual cause |
|---|---|
| `Unknown symbol 'x'` | A misspelled name, one that's defined in an inner scope, or a `.var` used before its declaration |
| `The symbol 'x' is already defined` | The same name twice in a scope |
| `Can't assign to 'x' since it isn't a variable` | Assigning to a label or `.const` |
| `Pseudo command 'x' not defined` | An upper case mnemonic, or a missing definition |
| `Macro command 'x' not defined` | A wrong name |
| `Invalid number of arguments for macro` | A wrong number of arguments |
| `Unknown function : +` | An operator or a method that the value doesn't have, like `"x"-1` |
| `Variable 'x' not present on value of type 'Namespace'` | Something other than a label behind the dot |
| `Missing start token`, `Missing end token` | A bracket without its pair |
| `'jmp' does not support mode 'zeropage'` | `.z` on an instruction without that mode |
| `In segment 'X' the memoryblock 'A' (...) is out of segment bounds (...).` | A block outside `min` and `max` |
| `invalid adressingmode. 'x' doesn't support ...` | The instruction has no such mode, like `sta #1` |
| `The mnemonic 'x' is not supported by the current cpu` | Change the CPU |
| `relative address is illegal (jump distance is too far: n).` | Use `jmp`, or invert the branch |
| `Can't get a double representation from a value of type String` | A number on the left of `+` and a string on the right, or a string in `.byte` |
| `Can't get a boolean representation from a value of type Number` | `.if (1)`; write a comparison |
| `Can't output bytes inside a function or a .define block` | Use a macro |
| `In segment 'X' the memoryblock 'A' (...) overlaps 'B' (...)` | Two blocks at the same address |
| `Can't open file: x`, `Can't open file 'x'` | Check the path and `-libdir` |
| `Too deep recursion` | A macro or function that never stops calling itself. R65's own text: the original stops with a Java `StackOverflowError` |

## 22. Limitations

- Java plugins aren't supported: `.plugin`, `.modify` and `.filemodify` report an error. The only segment modifier is
  `BasicUpstart`.
- `-maxaddr` is accepted but not enforced, like the original 5.25.
- The segment parameter `dest` has no effect.
- Progressive or subsampled JPEG pictures may differ slightly from the original.

### Differences from the Kick Assembler manual

R65 follows the original program, KickAss.jar 5.25, and was compared with it by running both. Where the original's manual says
something else, R65 does what the program does:

| Topic | The Kick Assembler manual | The program and R65 |
|---|---|---|
| Line breaks | An expression continues after an operator | That's a syntax error outside `( )` and `[ ]` |
| `jmp.z` | Allowed | `'jmp' does not support mode 'zeropage'` |
| `AT_IZEROPAGEX` | The type of `($10,x)` | `($10,x)` gives -5, and no argument gives 5 |
| `noStartAddr` | The default is false for every file type | Only `prg` has a start address by default |
| `.sym` and namespaces | `.label name = $2000` | `.label name=$2000`, indented inside `.namespace name { }` |

### Differences from the original program

- With several errors, the original lists them all first (`Got 2 errors and 0 warnings while executing:`). R65 stops at the first
  one, which is the one that the original describes in detail.
- The original checks names before the first pass, so `lda nosuch` in a macro that's never called is an error there. R65 only
  checks the code it runs, and an unknown function or a wrong number of arguments in other code isn't found either.
- A method called with too many arguments is an error in the original, while R65 ignores the extra ones.
- An endless recursion stops with R65's own error, and a recursion of 2000 levels works where the original runs out of stack.
- The number of `flex pass` lines can differ when a segment starts after another one, or when a label moves that nothing uses.
- `-asminfo`: `[version]` has R65's version, the texts of the directives are R65's own, the `syntax` entries are in another order
  and those of the built-in macros are left out. The built-in file is named `r65:autoinclude.asm`.
- A `//` line in a `-cfgfile` file is skipped, while the original warns about its words.

## 23. Directive index

| Directive | Section |
|---|---|
| `.align` | [6](#align) |
| `.assert`, `.asserterror` | [19](#19-debugging-and-testing) |
| `.break`, `.watch` | [19](#19-debugging-and-testing) |
| `.byte`, `.word`, `.dword`, `.text` | [8](#8-data-directives) |
| `.const`, `.var`, `.eval` | [9](#9-variables-constants-enums-and-structs) |
| `.cpu` | [20](#20-cpus-and-opcode-sets) |
| `.define` | [9](#define) |
| `.disk` | [17](#disk) |
| `.encoding` | [15](#15-text-encodings) |
| `.enum`, `.struct` | [9](#9-variables-constants-enums-and-structs) |
| `.error`, `.errorif` | [19](#19-debugging-and-testing) |
| `.file` | [17](#file) |
| `.fill`, `.fillword`, `.lohifill` | [8](#8-data-directives) |
| `.filemodify`, `.modify`, `.plugin` | [22](#22-limitations) |
| `.filenamespace`, `.namespace` | [12](#namespaces) |
| `.for`, `.if`, `.while` | [10](#10-control-flow) |
| `.function`, `.macro`, `.pseudocommand`, `.return` | [11](#11-functions-macros-and-pseudocommands) |
| `.import` | [18](#import) |
| `.label` | [9](#label) |
| `.memblock`, `.segment`, `.segmentdef`, `.segmentout` | [16](#16-memory-blocks-and-segments) |
| `.pc` | [6](#setting-the-address) |
| `.print`, `.printnow` | [19](#19-debugging-and-testing) |
| `.pseudopc` | [6](#pseudopc) |
| `.zp` | [5](#zero-page-or-absolute) |
| `.importonce` | [13](#13-the-preprocessor) |
| `BasicUpstart`, `BasicUpstart2` | Below |

The built-in macro `BasicUpstart(addr)` writes the BASIC line `10 SYS addr` at the current address.
`BasicUpstart2(addr)` does the same at `$0801` in a block named `Basic`, and then opens an empty block named `Basic End` at
`$080e`.
