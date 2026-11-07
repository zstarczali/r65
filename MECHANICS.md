# R65 – how it works and where the rules come from

This app is a new 6502/C64 cross assembler written in Rust. Its behaviour was
matched against the original Java assembler (KickAss.jar 5.25) by reading the
decompiled classes and by running both programs on the same sources. Class and
method names below point at the original program; "observed" means the value
was confirmed by running the original; "estimated" means we looked but could
not pin it down exactly.

All values are also in `mechanics.json`.

## Where the evidence comes from

The original is a Java program (a `.jar`), not native machine code, so the
native reverse-engineering tools (string search, function pseudocode at an
address) refuse it. Instead each method was read as Java bytecode with the
Java class/method readers. A Java method has no memory address; it is named
here as `Class.method` plus the bytecode offset of the instruction that holds
the value (for example `SegmentManager.<clinit> @0004 sipush 8192`).

| Rule | Original method (bytecode offset) |
|---|---|
| Banner width 54 | `KickAssembler.execute` @0038 `bipush 54` |
| Pass loop | `KickAssembler.assemble` @0402–046a |
| Exit code 1 on error | `KickAssembler.execute` @00cc `iconst_1` |
| Default start $2000 | `SegmentManager.<clinit>()V` @0004 `sipush 8192` |
| BASIC line at $0801 | `BasicUpstartSegmentModifier.createUpstart` @0016 `sipush 2049` |
| Zero page limit 255 | `_65xxMnemonicNode.resolveArgType` @0037 `sipush 255` |
| Number printing | `NumberValue.doubleToString` (`(int)v == v` → integer text) |
| Memory map | `SegmentManager.printMemoryBlocks` |
| Byte dump layout | `ByteDumpWriter.printSegment`, `printBytes`, `writeBytesLine` |
| VICE label order | `ViceSymbolFile.writeSymbols` (java.util.HashMap) |
| Label recording | `LabelDirective.executePass`, `LabelAssignmentDirective.executePass`, `ScopeManager.addResolvedSymbol` |
| Disk images | `D64Image.generateImage` |

## Command line

Like the original, this is a command line tool: `r65 file.asm [options]`.
Without a source file it prints the banner and "Error: No input file given."
and exits with code 1 (the original also exits with 1). The editor window is
optional and only opens with `--gui [file]` or `--screenshot <png>`.

## Passes and labels

- The source is preprocessed (`#define`, `#if/#elif/#else/#endif`, `#import`,
  `#importif`, `#importonce`), split into tokens and parsed once.
- The whole program is then executed again and again ("flex passes"). A label
  that is used before it is defined takes its value from the previous pass.
  The log prints `flex pass N` for each pass and `Output pass` at the end
  (source: `kickass.KickAssembler.assemble`).
- A name that is used but never declared anywhere is reported right after the
  first pass, before any `flex pass` line, like the original's pre-pass check.
- Pass loop (`KickAssembler.assemble` @0402–046a): print `flex pass N`, run the
  pass, report errors; repeat until the program is finished, then
  `Output pass`. If a pass makes no progress and the program isn't finished,
  one more strict pass runs (`setFailOnInvalidValue(true)` @044e) so the real
  error is reported. The original has no pass limit; ours stops after 12 as a
  safety net only (never reached by any example, pass counts match).

## Choosing the addressing mode (zero page rule)

Source: `_65xxMnemonicNode.resolveArgType`.

1. A `.a`/`.abs` suffix forces absolute.
2. The zero page form is used when the instruction has one and the value is
   known and 0–255, or a `.z`/`.zp` suffix is given, or the label comes from a
   `.zp { }` block and its value fits in 0–255.
3. Otherwise a branch instruction uses relative mode, and everything else the
   absolute form.
4. A label defined later than its use gets the absolute form, with the warning
   "Using absolute mode for zeropage argument. Try defining involved labels
   before this statement." (observed).
- Relative distance: −128…127 from the address after the instruction; error
  "relative address is illegal (jump distance is too far: N)".
- Argument type numbers (`AT_ABSOLUTE` = −1, `AT_ZEROPAGE` = 2,
  `AT_ABSOLUTEX` = −2, `AT_ABSOLUTEY` = −3, `AT_IMMEDIATE` = 1,
  `AT_INDIRECT` = −4, `AT_IZEROPAGEX` = 5, `AT_IZEROPAGEY` = 6, `AT_NONE` = 0)
  come from the original's constant table.
- Opcode tables: `kickass._65xx.cpus` (`CPU_6502NoIllegals`, `CPU_6502` with
  illegal opcodes – the default, `dtv`, `_65c02`). Opcode constants such as
  `LDA_IMM` use the default 6502 table.

## Memory, segments and files

- A program without `*=` starts at $2000 in the block "Default"; `*=` without
  a name makes a block called "Unnamed" (observed).
- The main file holds the Default segment. Gaps between blocks are filled with
  zero (`Segment.flatten` uses a fresh zeroed array; `-fillbyte` changes it).
- `-maxaddr` is accepted but, like the original 5.25, not enforced
  (`KickAssemblerParameters.maxMemoryAddress` is never read).
- Overlapping blocks are an error after the passes:
  "In segment 'X' the memoryblock 'A' ($a-$b) overlaps 'B' ($c-$d)" (observed).
- `-mbfiles` writes `<name>_<block>.prg` per block name
  (`C64OutputManager.postPassExecution`).
- Every written file is logged as "Writing prg file: name"
  (`C64OutputManager.writeC64File`), also with `-nooutput`.

## Built-in BASIC start macros

Our own macro text does the same job as the original's built-in include file
(`include/autoinclude.asm`): `BasicUpstart(address)` writes the BASIC line
`10 SYS address`; `BasicUpstart2(address)` places it at $0801 in a block named
"Basic" and then opens an empty block "Basic End" at $080e. The segment
modifier `modify="BasicUpstart"` builds the same line with the link address
$0801 + 8 + number of digits − 2.

## Text encodings

`ascii`, `petscii_mixed`, `petscii_upper`, `screencode_mixed` (default),
`screencode_upper`. The PETSCII tables are taken from `CharToPetsciiConverter`;
screen codes come from PETSCII with the usual rule (0–31 → +128, 64–95 → −64,
96–127 → −32, 128–159 → +64, 160–191 → −64, 192–254 → −128, 255 → 94).

## Script values

- Numbers print like Java doubles, whole numbers without ".0"
  (`NumberValue.doubleToString`): `0.3333333333333333`, `1.0E-4`, `Infinity`.
- `round` = Java `Math.round` (round half up: −2.5 → −2); `mod` = Java `%`.
- Structs print as `Name-struct` (`StructValue.getString`); command arguments
  as `Argument<IMMEDIATE>`; matrices as `[[a,b,c,d],...]`.
- `.enum`: entries without a value count 0, 1, 2…; a given value does not move
  the counter (`EnumDirective.executePass`).
- `RotationMatrix(aX,aY,aZ)`, `ScaleMatrix`, `MoveMatrix`,
  `PerspectiveMatrix(z)` follow the original's formulas; a matrix times a
  vector divides by w.

## Pictures

- `getSinglecolorByte(x,y)`: 8 pixels, bit = colour index & 1;
  `getMulticolorByte`: every second pixel, colour index & 3. Default colour
  table: black first, then the first new colours in pixel order (up to 4).
- JPEG files are decoded the way the original's Java runtime does it
  (classic reference decoder: slow integer IDCT, 13 constant bits, 2 pass
  bits, fixed point YCbCr → RGB with 1.402, 1.772, 0.71414, 0.34414).
  Confirmed: the pixel checksum of the dithering example matches exactly
  (observed). Progressive or colour-subsampled JPEGs use a general decoder and
  may differ slightly.

## SID files

Header fields: name at 22, author at 54, copyright at 86 (32 bytes each),
version at 4, data offset at 6, load address at 8 (0 = first two data bytes),
init at 10, play at 12, songs at 14, start song at 16, speed at 18 (4 bytes),
flags/startpage/pagelength only for version ≥ 2. Big-endian.

## Output files

- Symbol file (`.sym`, CRLF lines), observed with KickAss.jar 5.25: `SymbolFile.writeScope`
  goes over `getLocalDefinedEntities`, a Java `HashMap`, so each scope has the order of
  `java_bucket`, with namespaces among the labels. `.label name=$addr`, with
  `.namespace n {` / `}` and `.label name=$addr {` / `}` around inner scopes and a 2 space
  indent per level. `.var` and `.const` aren't written. An empty namespace is written. The
  body of a labelled macro call stays empty (`.label mc=$200b {`, `}`), because it has its
  own symbol table. Loop labels and `.lohifill` labels have no braces.
- VICE file (`.vs`): `al C:<hex> .<name>`, names used more than once get
  `__0`, `__1`…; `break <hex> <arg>`; `watch <arg> <a1> <a2> `
  (`ViceSymbolFile`).
- VICE label order (`ViceSymbolFile.writeSymbols`): names are grouped in a
  Java `HashMap` and written in its iteration order. We reproduce it: the
  table starts at 16 slots and doubles while names > 0.75 × slots; a name's
  slot is `(h ^ (h >>> 16)) & (slots − 1)` with the Java string hash
  `h = 31·h + char`; names in the same slot keep the order they were added.
- Labels are added in the order they are placed, and only in the first pass
  where their address is known (`LabelDirective.executePass` returns early
  while the memory position is unknown). So labels after a `.text` whose
  length depends on a not-yet-known value (like the BASIC line of
  `BasicUpstart2(start)` with `start` defined later) are added a pass later.
- Byte dump (`ByteDumpWriter`): a header 80 wide per segment
  (`pad = max(4, 80 − title length)`, half left, rest right), `[block]` per
  block (`Unnamed` without a name; empty and virtual blocks are not listed).
  Each entry is split into lines of 3 bytes (entry ≤ 3 bytes), 16 bytes
  (entry ≤ 64 bytes) or 32 bytes (longer). A line is `%04x: ` plus one 3-char
  slot per byte (`%02x ` or three spaces past the end). Only the first line
  gets ` - ` + labels (`a,b:` padded to 8 with `%-8s`) + the source text from
  the statement's column. A label pointing inside the entry shows as `x-3`.
- Disk images (`D64Image.generateImage`, `C1541DiskWriter`): 35 tracks,
  default name "UNNAMED", id "   2A", file interleave 10. At most
  8 × 19 = 152 visible files ("Too many visible files. Got N, max is M.").
  Directory sectors 18/1, 18/2, … are used in order, 8 entries each; the first
  entry of a sector holds the link to the next one (18, s+1) or (0, $ff).
  Files, hidden ones too, are placed from track 1 sector 0: the next free
  sector is searched from the current sector modulo the track size, a new
  track starts at sector 0, track 18 is skipped, "Disk is full!" after track
  35; after each sector the position moves by the interleave. Names are padded
  to 16 with $a0. Checked byte for byte against the original on a 10-file disk.

## Error and log text

- Error text: up to three lines before the error line, the line itself, a `^`
  under the column, a blank line, `Error: msg`, `at line L, column C in file`
  and one `called at …` line per call, innermost first
  (`StackTraceErrorPrinter`). Errors inside built-in functions and operators
  get one `called at` line with their own position.
- Warnings: "Got 0 errors and N warnings while executing:" with at most 5
  entries (`KickAssembler.printErrorsAndTerminate`,
  `KickAssemblerParameters.showMaxNoOfDiagnostics`).
- Asserts: "Made N asserts , M failed" or "… M FAILED!".

## Not supported

Java plugins (`.plugin`, `.modify`, `.filemodify` with plugin modifiers) need a
Java runtime and are reported as errors.

## Observed by running KickAss.jar 5.25

These were all compared by running the original (`D:\Development\KickAss\KickAss.jar`) and
R65 on the same sources, also where the original's manual says something else. See section 22
of the manual for what is still different.

- Line breaks: inside `( )` and `[ ]` they mean nothing. Elsewhere they end the statement, also
  after an operator or a comma (`Syntax error`).
- Statement start: `Invalid token`; `#` gives `Token must be an identifer` at the next token.
  A plain `x = 2`, `x++`, `x += 2`, `l.add(1)` or `s.m()` is a `Syntax error` at the operator.
  `l[0]` is a call of the pseudocommand `l`.
- Brackets are checked per source file. `Missing start token` at a closing bracket without a
  pair or of another kind, `Missing end token` at the opening one that's left at the end.
- Names: `The symbol 'x' is already defined`, found before the first pass (no `flex pass 1`),
  at the column of the name. Label, `.var`, `.const`, `.label`, `.enum` item, `.define` name,
  operand label and parameter share one set per scope, also in macros that are never called and
  in false `.if` branches. Reopened namespaces share a set, and the namespace's own name is in
  the outer set. `.file`, `.zp` and `.pseudopc` have no set of their own. `@x` shows as `'@x'`.
- Member access: only labels (also `.label`) and namespaces are reached with a dot; `.lohifill`
  has `lo` and `hi`. Otherwise `Variable 'x' not present on value of type 'T'`, with T =
  `LabelValue`, `Namespace`, `ScopeValue` (`l[0]`), `number` or `struct`. A method there is
  `Unknown function : f`.
- `.z`: `'jmp' does not support mode 'zeropage'` (also `zeropageX`, `zeropageY`), at the mnemonic.
- `getType` of `($10,x)` is -5. No argument reports `AT_IZEROPAGEX` (5).
- Operators: a left value without the operator gives `Unknown function : -`, and so do unary
  ones (at the operator). A wrong right value gives `Can't get a double representation from a
  value of type boolean` (lower case `boolean`). `Vector+1` gives `Can't get a vector from value of type number`.
- `.byte`, `.word`, `.dword`: a value that can't be written is reported at the directive.
- Calls: `Macro command 'x' not defined`, `Pseudo command 'x' not defined`, `Invalid number of
  arguments for macro`, `Too many arguments for pseudo command`, `Invalid number of arguments for
  function 'f'`, `Invalid number of arguments : size` (methods). Definitions: `The function 'f'
  is already defined.`, `The macro 'f'  is already defined`, `The pseudo command 'f'  is already
  defined` (two spaces). These come before the first pass.
- Recursion: the original has no limit, and a run out of Java stack ends with exit code 1.
  `sum(500)` and macro depth 300 work, `sum(2000)` doesn't. R65's limit of 10000 calls and its
  `Too deep recursion` text are its own (estimated, not from the original).
- Locked values: `You can't modify a locked value`. `Hashtable().lock()` then `.put` is allowed.
- `.const` doesn't lock a value, `.define` does.
- Asserts with code: `name -- OK. | 2000:ad,00,10`, `name -- FAILED! | 2000:ad,00,10  -- 2000:ae,00,10`
  (two spaces), blocks joined by `; `. Labels in the blocks appear in the `.vs` file.
  `.asserterror` without an error: `name -- FAILED!` for code, `name -- ERROR IN ASSERTION!` for a value.
- Missing files: `Can't open file: x` for `#import` (at the string) and `.import source`
  (at the directive); `Can't open file 'x'` for binary files; `Can't locate file 'x'` for
  `LoadPicture`. `prgFiles` and `sidFiles` are read after `flex pass 1`, also for unused
  segments, and reported at the parameter.
- Segment limits: `In segment 'D' the memoryblock 'D' ($c000-$d7ff) is out of segment bounds
  ($c000-$cfff).`, without a position, after `Output pass`. `min` defaults to `$0000`, a missing
  `max` prints as `($1000- )`. All segments count, also unwritten ones, virtual blocks and the
  blocks from `segments=` and `prgFiles`. Names of the others: `<<.file:o.prg>>`,
  `<<.diskfile-0>>`, `<<.segmentout:0>>`.
- `.segmentout` needs one more flex pass, and a shorter instruction doesn't by itself.
- Disk images: only `prg` has a start address by default. BAM of tracks 36-40 in sector 18/0:
  speeddos at `$b0`, dolphindos at `$c4`. An empty directory sector starts `00 00`. With
  `storeFilesInDir`, track 18 comes in track order after 17. `showInfo`: `Info for disk 'X'` and
  `  "A", sectors=1, startTS=1:0`. Bad format: `Unknown litteral 'cbm'` at the parameter. Errors
  while generating: `Writing d64 file: x.d64`, then `Error while generating 'x.d64': ...`
  without a position, and the file is left with 0 bytes. No filename: `Paremter 'filename' is missing.`
- Command line: a second source name gives `Warning. Already have an inputfile. Won't use 'x'`.
  `-cfgfile` with a missing file gives `Warning. Can't find configfile : x`. `-debug` prints
  nothing extra in the cases tried. `-execute` doesn't wait, also with `-executelog`, which is
  created even if the program can't start (`Error while executing external program: ...`, exit 0).
- `-asminfo`: sections in the order version, libraries, directives, ppdirectives, errors,
  syntax, files; CRLF lines. Bad selection: `Invalid asminfo type: x`. `[files]` only names what
  errors and syntax use: first the file of the error, then the main source, its imports, the
  built-in macros last. `-asminfotostdout` prints a blank line and `### ASMINFO START ###` ... `### ASMINFO END ###`.
  The 352 lines of `[libraries]` are in `src/asminfo_libraries.txt`.
- `-debugdump` (`<name>.dbg`): 3 space indent levels, `<Sources>`, a `<Segment>` per segment,
  entries `$1000,$1000,1,2,8,2,10` that end at the mnemonic or directive name, `<Labels>` with the
  code labels, `<Breakpoints>`, `<Watchpoints>`, blank lines between sections.
