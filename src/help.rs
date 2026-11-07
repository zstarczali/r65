//! The command line help: `rust65 --help [topic]`.
//! The option list itself lives in `driver::HELP`, next to the option parser.

use crate::driver;

const OVERVIEW: &str = "Usage:
  rust65 <file.asm> [options]     assemble a source file
  rust65 --gui [file.asm]         open the optional editor window
  rust65 --help [topic]           show this help, or one topic

Quick start:
  rust65 game.asm                 writes game.prg next to the source
  rust65 game.asm -showmem -vicesymbols -bytedump
  rust65 game.asm -o out.prg -odir build -define DEBUG

Exit code: 0 on success, 1 on any error.

Help topics (rust65 --help <topic>):
  options       every command line option
  syntax        labels, numbers, operators, addressing modes
  directives    all .directives
  preprocessor  #define, #if, #import ...
  functions     built-in functions and constants
  values        methods of strings, lists, hashtables, files, pictures ...
  segments      memory blocks, segments and output files
  disk          .disk: writing .d64 images
  encodings     .text character encodings
  cpu           supported CPUs and opcode sets
  output        which files are written and what they contain
  config        the rust65.cfg file
  examples      a complete sample program
  limits        what isn't supported
  all           everything above";

const SYNTAX: &str = "SYNTAX

Program counter
  * = $1000             set the address (opens a block named \"Unnamed\")
  * = $1000 \"Main\"      the same, with a block name
  Without any *= the code starts at $2000 in the block \"Default\".

Labels
  name:                 a label
  !name:                a multi label, can be defined many times
  jmp !name+            the next !name
  bne !name-            the previous !name
  scope.name            a label inside a named scope

Numbers
  123  $ff  %1010       decimal, hexadecimal, binary

Strings and comments
  \"text\"                a string
  // line   /* block */ comments

Operators
  + - * /               arithmetic (a string on the left of + joins strings)
  << >> & | ^ ~         bit operations
  == != < > <= >=       comparison
  && || !               logic
  ? :                   condition ? a : b
  <value  >value        low byte, high byte
  = += -= *= /= ++ --   assignment, in .var, .eval and .for

Addressing modes
  nop                   implied
  lda #$10              immediate
  lda $10               zero page (value known and 0-255)
  lda $1000             absolute
  lda $10,x   lda $1000,y   indexed
  lda ($10,x) lda ($10),y   indirect indexed
  jmp ($1000)           indirect
  bne label             relative (-128..127)

Forcing an addressing mode with a suffix
  lda.z  lda.zp         zero page       lda.a   lda.abs   absolute
  lda.im lda.imm        immediate       jmp.i   jmp.ind   indirect
  lda.zx lda.zpx        zero page,x     lda.zy  lda.zpy   zero page,y
  lda.izx lda.izpx      (zp,x)          lda.izy lda.izpy  (zp),y
  lda.ax lda.absx       absolute,x      lda.ay  lda.absy  absolute,y
  bne.r  bne.rel        relative

A label defined later than its use gets the absolute form, with a warning.
Define zero page labels first, or put them in a .zp { } block.

Passes
  The program runs repeatedly (\"flex pass N\") until all values are known.
  .print output appears once, in the output pass. .printnow prints in every
  pass. Use it to debug the passes.";

const DIRECTIVES: &str = "DIRECTIVES

Data
  .byte a, b  (.by)            8 bit values
  .word a, b  (.wo)            16 bit values
  .dword a, b (.dw)            32 bit values
  .text \"abc\" (.te)            a string in the current encoding
  .fill n, expr                n bytes; expr can use i (0..n-1)
  .fillword n, expr            n words
  .lohifill n, expr            a low byte table, then a high byte table
  .align n                     move to the next multiple of n

Values
  .var name = v                a variable
  .const name = v              a constant
  .label name = v              a named address, also written to symbol files
  .eval expr                   evaluate, for example .eval x = x + 1
  .enum { a, b, c = 10 }       constants, counted from 0
  .struct Name { x, y }        a struct type: Name(1, 2).x
  .define a, b { ... }         hide values, compute them in the block

Flow
  .if (cond) { } else { }
  .for (var i = 0; i < 8; i++) { }
  .while (cond) { }
  .return value                leave a .function

Definitions
  .function name(a, b) { .return a + b }
  .macro name(a, b) { ... }             call: name(1, 2)
  .pseudocommand name a : b { ... }     call: name #1 : $d020

Scopes
  name: { ... }                a named scope, read with name.label
  .namespace name { ... }      a namespace
  .filenamespace name          a namespace until the end of the file
  .zp { ... }                  labels in here use zero page addressing

Memory
  .pc = $1000                  like * = $1000
  .pseudopc $c000 { ... }      code that will run at another address
  .memblock \"name\"             a new block at the current address
  .segment Name \"block\"        switch segment (see: --help segments)
  .segmentdef Name [params]    define a segment
  .segmentout [params]         place another segment's bytes here
  .file [params]               write segments to a file
  .disk [params] { [file], }   write a .d64 (see: --help disk)
  .modify Name(args) { ... }   only with plugins (see: --help limits)
  .filemodify Name(args)       only with plugins

Files
  .import source \"file.asm\"    the same as #import
  .import binary \"file\", offset, length
  .import c64 \"file.prg\", offset, length    skips the 2 byte start address
  .import text \"file\", offset, length

Other
  .encoding \"name\"             see: --help encodings
  .cpu name                    see: --help cpu
  .print expr                  print in the output pass
  .printnow expr               print in every pass
  .error msg                   stop with an error
  .errorif cond, msg           stop with an error if cond is true
  .assert \"text\", a, b         test that a equals b
  .asserterror \"text\", expr    test that expr gives an error
  .break [arg]                 VICE breakpoint (written with -vicesymbols)
  .watch a [, b, arg]          VICE watchpoint (written with -vicesymbols)

Built-in macros
  BasicUpstart(addr)           a BASIC line \"10 SYS addr\" at the current address
  BasicUpstart2(addr)          the same at $0801, in a block \"Basic\"";

const PREPROCESSOR: &str = "PREPROCESSOR

These run first, line by line, before the source is parsed.

  #define NAME                 define a symbol (also: -define NAME)
  #undef NAME                  remove a symbol
  #if EXPR                     conditional source; ! && || == != and ( ) work
  #elif EXPR
  #else
  #endif
  #import \"file.asm\"           include a source file
  #importif EXPR \"file.asm\"    include it when EXPR is true
  #importonce                  this file is included only once

Imported files are searched next to the importing file, then in each -libdir.
-replacefile <a> <b> uses file b wherever file a is imported.";

const FUNCTIONS: &str = "BUILT-IN FUNCTIONS AND CONSTANTS

Math
  abs acos asin atan atan2 cbrt ceil cos cosh exp expm1 floor hypot
  IEEEremainder log log10 log1p max min mod pow random round signum
  sin sinh sqrt tan tanh toDegrees toRadians
  setSeed(n)                   seed for random()

Numbers to text
  toIntString(n)  toBinaryString(n)  toOctalString(n)  toHexString(n)

Creating values
  List(n)  Hashtable()  Vector(x, y, z)
  Matrix()  IdentityMatrix()  RotationMatrix(aX, aY, aZ)  ScaleMatrix(x, y, z)
  MoveMatrix(x, y, z)  PerspectiveMatrix(z)
  CmdArgument(type, value)     a pseudocommand argument

Files
  LoadBinary(file [, format])  a binary file; see the BF_ formats below
  LoadSid(file)                a SID tune
  LoadPicture(file [, colors]) a gif, jpeg or png picture
  createFile(name)             a text file; needs the -afo option

Source
  getFilename()  getPath()  getNamespace()
  asmCommandSize(opcode)       size of an instruction, e.g. asmCommandSize(LDA_IMM)

Constants
  PI  E
  BLACK WHITE RED CYAN PURPLE GREEN BLUE YELLOW ORANGE BROWN LIGHT_RED
  DARK_GRAY GRAY LIGHT_GREEN LIGHT_BLUE LIGHT_GRAY  (GREY spellings also work)
  AT_NONE AT_IMMEDIATE AT_ABSOLUTE AT_ZEROPAGE AT_ABSOLUTEX AT_ABSOLUTEY
  AT_INDIRECT AT_IZEROPAGEX AT_IZEROPAGEY
  BF_C64FILE BF_KOALA BF_FLI BF_BITMAP_SINGLECOLOR BF_DOODLE
  LDA_IMM, STA_ABS ...         opcode values (default 6502 table)
  cmdLineVars                  hashtable of the :name=value options";

const VALUES: &str = "METHODS OF SCRIPT VALUES

Any value
  .string()

String
  .size() .charAt(i) .substring(a, b) .toLowerCase() .toUpperCase()
  .asNumber() .asBoolean()

List
  .get(i) .set(i, v) .add(a, b, ...) .addAll(list) .size() .remove(i)
  .shuffle() .reverse() .sort() .lock()

Hashtable
  .put(k, v) .get(k) .containsKey(k) .remove(k) .keys() .lock()

Struct
  .field .getStructName() .getNoOfFields() .getFieldNames() .get(i) .set(i, v)

Vector
  .get(i) .getX() .getY() .getZ() .X(other)

Matrix
  .get(row, col) .set(row, col, v)

Pseudocommand argument
  .getType() .getValue()

Binary file
  .getSize()  .get(i)  .uget(i)        get is signed, uget is unsigned
  With a format, each block has its own: .getBitmap(i) .ugetBitmap(i) .getBitmapSize()

Picture
  .width .height .getPixel(x, y) .getSinglecolorByte(x, y) .getMulticolorByte(x, y)

SID file
  .header .version .location .init .play .songs .startSong .speed
  .name .author .copyright .size .flags .startpage .pagelength";

const SEGMENTS: &str = "MEMORY BLOCKS, SEGMENTS AND OUTPUT FILES

A segment is a list of memory blocks. Code goes to the segment \"Default\"
unless you switch. The main output file holds the Default segment, with gaps
filled with zero (-fillbyte changes this). Overlapping blocks are an error
unless -aom or allowOverlap is given.

  .segmentdef Code [start=$0810]
  .segmentdef Data [startAfter=\"Code\"]
  .segment Code \"Main\"
      lda #0
  .segment Data \"Tables\"
      .byte 1, 2, 3
  .file [name=\"data.prg\", segments=\"Data\"]

Parameters of .segmentdef, .segment, .segmentout and .file
  start=addr            where the segment starts
  startAfter=\"name\"     start right after another segment
  align=n               align the start
  min=addr  max=addr    allowed address range
  fill                  fill up to max
  fillByte=n            the byte used for filling
  virtual               reserve addresses without output
  allowOverlap          blocks in this segment may overlap
  hide                  not shown in the memory map
  dest=\"text\"           accepted, has no effect
  outPrg=\"file\"         write this segment to a prg file
  outBin=\"file\"         write this segment to a raw file
  segments=\"a, b\"       take the bytes of these segments
  prgFiles=\"a, b\"       take the bytes of these prg files
  sidFiles=\"a, b\"       take the bytes of these SID files
  modify=\"BasicUpstart\" add a BASIC start line, _start=addr gives its target

Parameters of .file
  name=\"file\"           the file name
  type=\"prg\"|\"bin\"      the file type
  mbfiles               one file per memory block";

const DISK: &str = "DISK IMAGES

  .disk [filename=\"game.d64\", name=\"MY GAME\", id=\"2A\"] {
      [name=\"LOADER\", type=\"prg\", segments=\"Code\"],
      [name=\"DATA\",   type=\"prg\", segments=\"Data\"],
  }

Disk parameters
  filename=\"x.d64\"      the image file
  name=\"text\"           disk name (default UNNAMED)
  id=\"text\"             disk id
  interleave=n          sector step (default 10)

File parameters
  name=\"text\"           file name, up to 16 characters
  type=\"prg\"            the file type
  hide                  not listed in the directory
  interleave=n          sector step for this file
  noStartAddr           leave out the 2 byte start address
  segments, prgFiles, sidFiles and the other parameters of --help segments

Limits: 35 tracks, at most 152 visible files.";

const ENCODINGS: &str = "TEXT ENCODINGS

  .encoding \"screencode_mixed\"    used by .text; this one is the default

  ascii  petscii_mixed  petscii_upper  screencode_mixed  screencode_upper";

const CPU: &str = "CPUS

  _6502            standard 6502 with illegal opcodes (default)
  _6502NoIllegals  standard 6502 only (-excludeillegal)
  dtv              with the C64 DTV opcodes (-dtv)
  _65c02           65C02

Choose with -cpu <name> or in the source with .cpu <name>.";

const OUTPUT: &str = "OUTPUT FILES

  name.prg        program with a 2 byte start address (the default)
  name.bin        raw bytes, with -binfile
  name_block.prg  one file per block, with -mbfiles
  name.vs         VICE labels, breakpoints and watchpoints, with -vicesymbols
  name.sym        symbols to import in another source, with -symbolfile
  ByteDump.txt    addresses, bytes and source, with -bytedump
  *.d64           disk images from .disk

\"name\" is the source file without its extension. Files go next to the source
file unless -odir is given. -nooutput logs the same lines but writes nothing.

With -log <file> the log is also saved to a file. -showmem prints the memory
map, -time the time taken. -execute <cmd> runs a program with the output file
after a successful build, for example: -execute x64sc";

const CONFIG: &str = "CONFIG FILE

rust65.cfg next to the rust65 program holds default options, one or more per
line. Lines starting with # or // are skipped. They are read before the command
line, and aren't used in window mode.

  # rust65.cfg
  -showmem
  -libdir C:\\c64\\lib";

const EXAMPLES: &str = "EXAMPLE

  // border.asm: rust65 border.asm -showmem
  BasicUpstart2(start)

  .const BORDER = $d020

  .macro setBorder(color) {
      lda #color
      sta BORDER
  }

  * = $0810 \"Main\"
  start:
      setBorder(RED)
      ldx #0
  !loop:
      lda message,x
      beq !done+
      sta $0400,x
      inx
      jmp !loop-
  !done:
      rts

  message:
      .text \"HELLO\"
      .byte 0

Command lines
  rust65 border.asm                       assemble to border.prg
  rust65 border.asm -odir build           write into the build folder
  rust65 border.asm -define DEBUG         #if DEBUG becomes true
  rust65 border.asm :level=3              cmdLineVars.get(\"level\") is \"3\"
  rust65 border.asm -execute x64sc        start VICE after the build";

const LIMITS: &str = "LIMITS

- Java plugins aren't supported: .plugin, .modify and .filemodify report an
  error (only the BasicUpstart modifier of segments exists).
- -maxaddr is accepted but not enforced, like the original 5.25.
- Progressive or subsampled JPEG pictures may differ slightly from the original.";

const TOPICS: [(&str, &str); 13] = [
    ("syntax", SYNTAX),
    ("directives", DIRECTIVES),
    ("preprocessor", PREPROCESSOR),
    ("functions", FUNCTIONS),
    ("values", VALUES),
    ("segments", SEGMENTS),
    ("disk", DISK),
    ("encodings", ENCODINGS),
    ("cpu", CPU),
    ("output", OUTPUT),
    ("config", CONFIG),
    ("examples", EXAMPLES),
    ("limits", LIMITS),
];

/// The help text of a topic (none = overview), or None for an unknown topic.
pub fn text(topic: Option<&str>) -> Option<String> {
    let overview = format!("{}\n\n{}", driver::banner_lines().join("\n"), OVERVIEW);
    let Some(topic) = topic else { return Some(overview) };
    let t = topic.to_lowercase();
    match t.as_str() {
        "options" => Some(driver::HELP.to_string()),
        "all" => {
            let mut parts = vec![overview.as_str(), driver::HELP];
            parts.extend(TOPICS.iter().map(|(_, body)| *body));
            Some(parts.join("\n\n"))
        }
        _ => TOPICS.iter().find(|(name, _)| *name == t).map(|(_, body)| body.to_string()),
    }
}

/// The topic names, for the "unknown topic" message.
pub fn topic_names() -> String {
    let mut names = vec!["options"];
    names.extend(TOPICS.iter().map(|(name, _)| *name));
    names.push("all");
    names.join(", ")
}
