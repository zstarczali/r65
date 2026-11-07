//! The assembly language: each test checks the exact bytes or printed text.
mod common;
use common::{bytes, fails, prg, prints, Project};

// ---------------------------------------------------------------- source format

#[test]
fn semicolon_separates_statements() {
    assert_eq!(bytes("lda #1; ldx #2; rts"), [0xa9, 1, 0xa2, 2, 0x60]);
}

#[test]
fn comments() {
    assert_eq!(bytes("sta /* border */ $d020 // end\n/* a\nb */ nop"), [0x8d, 0x20, 0xd0, 0xea]);
}

#[test]
fn mnemonics_are_lower_case() {
    fails("LDA #1", "Pseudo command 'LDA' not defined");
}

#[test]
fn line_breaks_in_lists_and_calls() {
    // only inside brackets: a data list ends with its line
    fails(".byte 1, 2,\n 3", "Syntax error");
    assert_eq!(bytes(".macro m(a,\n b) { .byte a, b }\nm(1,\n 2)"), [1, 2]);
    assert_eq!(bytes(".function f(a,\n b) { .return a+b }\n.byte f(1,\n 2)"), [3]);
    assert_eq!(bytes(".pseudocommand p a :\n b { .byte a.getValue(), b.getValue() }\np #1 : #2"), [1, 2]);
}

#[test]
fn opening_brace_on_the_next_line() {
    assert_eq!(bytes(".if (true)\n{\n nop\n}\nelse\n{\n sei\n}"), [0xea]);
    assert_eq!(bytes(".for (var i=0;i<2;i++)\n{\n .byte i\n}"), [0, 1]);
}

// ---------------------------------------------------------------- numbers

#[test]
fn number_formats() {
    assert_eq!(bytes(".byte 10, $ff, %1010"), [10, 255, 10]);
}

#[test]
fn data_is_cut_without_an_error() {
    assert_eq!(bytes(".byte 256, -1, 3/2, 255.9"), [0, 0xff, 1, 0xff]);
    assert_eq!(bytes(".word 70000"), [0x70, 0x11]);
    assert_eq!(bytes(".dword $12345678"), [0x78, 0x56, 0x34, 0x12]);
    assert_eq!(bytes(".by 1\n.wo 2\n.dw 3"), [1, 2, 0, 3, 0, 0, 0]);
}

#[test]
fn numbers_print_like_java() {
    prints(
        ".print 3/2\n.print 1000000\n.print 0.0001\n.print 0.1\n.print 1/0\n.print -1/0\n.print 0/0",
        &["1.5", "1000000", "1.0E-4", "0.1", "Infinity", "-Infinity", "NaN"],
    );
}

#[test]
fn bit_operators_use_whole_numbers() {
    prints(".print 1.9|0\n.print -1.5&$ff\n.print 3^1\n.print 1<<4\n.print 16>>2", &["1", "255", "2", "16", "4"]);
}

#[test]
fn low_and_high_byte_take_the_whole_sum() {
    assert_eq!(bytes(".byte <$1234+1, >$1234, <($1234+1)"), [0x35, 0x12, 0x35]);
}

#[test]
fn operator_precedence() {
    prints(
        ".print 2+3*4\n.print (2+3)*4\n.print 1+1==2\n.print (6&3)==2\n.print true||false&&false\n.print 1<<2+1",
        &["14", "20", "true", "true", "true", "8"],
    );
}

#[test]
fn brackets_group_without_making_an_indirect_mode() {
    assert_eq!(bytes("*=$1000\nlda #[2+5]*2\nlda #(2+5)*2\njmp [$1000]\njmp ($1000)"), [0xa9, 14, 0xa9, 14, 0x4c, 0, 0x10, 0x6c, 0, 0x10]);
    prints(".print [7]", &["7"]);
}

#[test]
fn conditional_expression() {
    assert_eq!(bytes(".var debug = true\ninc debug ? $d020 : $d013"), [0xee, 0x20, 0xd0]);
    prints(".print 1>2 ? \"yes\" : \"no\"", &["no"]);
}

#[test]
fn conditions_need_booleans() {
    fails(".if (1) nop", "Can't get a boolean representation from a value of type Number");
}

#[test]
fn logical_operators_stop_early() {
    prints(".var n=1\n.print false && n.x\n.print true || n.x", &["false", "true"]);
}

// ---------------------------------------------------------------- text

#[test]
fn joining_text() {
    prints(".print \"x\"+1+true+null", &["x1trueNull"]);
    prints(".var n = 3\n.print \"\"+n+\" items\"", &["3 items"]);
    fails(".print 1+\"x\"", "Can't get a double representation");
}

#[test]
fn plain_strings_keep_backslashes() {
    prints(".print \"c:\\new\"", &["c:\\new"]);
}

#[test]
fn escape_codes() {
    assert_eq!(bytes(".encoding \"ascii\"\n.text @\"a\\n\\t\\\\\\\"\\$ff\""), [0x61, 10, 9, 0x5c, 0x22, 0xff]);
}

#[test]
fn characters_follow_the_encoding() {
    assert_eq!(bytes(".byte 'a'"), [1]);
    assert_eq!(bytes(".encoding \"ascii\"\n.byte 'a'"), [0x61]);
    assert_eq!(bytes("lda #'a'+1"), [0xa9, 2]);
    assert_eq!(bytes(".text \"a\"+'b'"), [1, 2]);
    prints(".print 'a'\n.print \"a\"=='a'", &["a", "true"]);
}

#[test]
fn encodings() {
    for (name, expected) in [
        ("screencode_mixed", [0x01, 0x41, 0x00, 0x31]),
        ("screencode_upper", [0x61, 0x01, 0x00, 0x31]),
        ("petscii_mixed", [0x41, 0xc1, 0x40, 0x31]),
        ("petscii_upper", [0x61, 0x41, 0x40, 0x31]),
        ("ascii", [0x61, 0x41, 0x40, 0x31]),
    ] {
        assert_eq!(bytes(&format!(".encoding \"{}\"\n.text \"aA@1\"", name)), expected, "{}", name);
    }
    fails(".encoding \"nope\"", "Unknown encoding 'nope'");
}

#[test]
fn text_directive_mixes_strings_numbers_and_null() {
    assert_eq!(bytes(".text \"a\", null, 66\n.te \"b\""), [1, 66, 2]);
}

#[test]
fn byte_directive_refuses_strings() {
    fails(".byte \"a\"", "Can't get a double representation");
}

// ---------------------------------------------------------------- instructions

#[test]
fn addressing_modes() {
    let src = "nop\nasl\nlda #1\nlda $10\nlda $10,x\nldx $10,y\nlda $1000\nlda $1000,x\nlda $1000,y\nlda ($10,x)\nlda ($10),y\njmp ($1000)\nbne *";
    assert_eq!(
        bytes(src),
        [0xea, 0x0a, 0xa9, 1, 0xa5, 0x10, 0xb5, 0x10, 0xb6, 0x10, 0xad, 0, 0x10, 0xbd, 0, 0x10, 0xb9, 0, 0x10, 0xa1, 0x10, 0xb1, 0x10, 0x6c, 0, 0x10, 0xd0, 0xfe]
    );
}

#[test]
fn mode_suffixes() {
    assert_eq!(bytes("lda.a $10\nlda.abs $10\nlda.z $10\nlda.zp $1234"), [0xad, 0x10, 0, 0xad, 0x10, 0, 0xa5, 0x10, 0xa5, 0x34]);
    assert_eq!(bytes("lda.zx $10\nldx.zy $10\nlda.ax $10\nlda.ay $10"), [0xb5, 0x10, 0xb6, 0x10, 0xbd, 0x10, 0, 0xb9, 0x10, 0]);
    assert_eq!(bytes("lda.im 5\nlda.izx $10\nlda.izy $10\njmp.i $1000"), [0xa9, 5, 0xa1, 0x10, 0xb1, 0x10, 0x6c, 0, 0x10]);
    assert_eq!(bytes("*=$1000\nbne.r *"), [0xd0, 0xfe]);
    fails("lda.q 1", "Unknown extension: q");
}

#[test]
fn zero_page_rule() {
    // known and small: zero page
    assert_eq!(bytes(".label z = $10\nlda z"), [0xa5, 0x10]);
    // too big for zero page
    assert_eq!(bytes(".label a = $100\nlda a"), [0xad, 0, 1]);
    // from a .zp block: zero page even when defined later
    assert_eq!(bytes("*=$1000\nlda r\n*=$10 virtual\n.zp {\nr: .byte 0\n}"), [0xa5, 0x10]);
}

#[test]
fn a_later_zero_page_label_gets_the_absolute_form_and_a_warning() {
    let p = Project::new();
    let r = p.asm("lda z\n.label z = $10", &[]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(p.read("t.prg")[2..], [0xad, 0x10, 0]);
    assert!(r.out.contains("Using absolute mode for zeropage argument. Try defining involved labels before this statement."), "{}", r.out);
    assert!(!p.asm("lda z\n.label z = $10", &["-warningsoff"]).out.contains("Using absolute"));
}

#[test]
fn wrong_modes_and_branches() {
    fails("sta #1", "invalid adressingmode. 'sta' doesn't support IMMEDIATE mode");
    fails("*=$1000\nbne $1100", "relative address is illegal (jump distance is too far:");
    assert_eq!(bytes("*=$1000\nbne $1081"), [0xd0, 0x7f]);
    assert_eq!(bytes("*=$1080\nbne $1002"), [0xd0, 0x80]);
}

#[test]
fn cpu_selection() {
    assert_eq!(bytes("lax $10"), [0xa7, 0x10]);
    fails(".cpu _6502NoIllegals\nlax $10", "The mnemonic 'lax' is not supported by the current cpu");
    assert_eq!(bytes(".cpu dtv\n*=$1000\nsac #1\nsir #2\nbra *"), [0x32, 1, 0x42, 2, 0x12, 0xfe]);
    assert_eq!(bytes(".cpu _65c02\nphx\nstz $10\nlda ($10)\ninc\ndec"), [0xda, 0x64, 0x10, 0xb2, 0x10, 0x1a, 0x3a]);
    assert_eq!(bytes(".cpu _65c02\n*=$1000\nbbr0 $10,*\njmp ($1000,x)"), [0x0f, 0x10, 0xfd, 0x7c, 0, 0x10]);
    fails(".cpu _65c02\nlax $10", "The mnemonic 'lax' is not supported");
    assert_eq!(bytes(".cpu _65c02\nnop\n.cpu _6502\nlax $10"), [0xea, 0xa7, 0x10]);
}

#[test]
fn opcode_constants() {
    assert_eq!(bytes(".byte LDA_IMM, NOP, JMP_IND, BNE_REL, LDA_IZPY"), [0xa9, 0xea, 0x6c, 0xd0, 0xb1]);
    prints(".print asmCommandSize(RTS)\n.print asmCommandSize(LDA_IMM)\n.print asmCommandSize(LDA_ABS)", &["1", "2", "3"]);
}

#[test]
fn colour_constants() {
    assert_eq!(bytes(".byte BLACK, WHITE, RED, LIGHT_GRAY, LIGHT_GREY, DARK_GREY, GRAY"), [0, 1, 2, 15, 15, 11, 12]);
}

// ---------------------------------------------------------------- labels and memory

#[test]
fn default_start_and_set_address() {
    assert_eq!(prg("nop")[..2], [0x00, 0x20]);
    assert_eq!(prg("*=$1000\nnop")[..2], [0x00, 0x10]);
    assert_eq!(prg(".pc=$3000 \"X\"\nnop")[..2], [0x00, 0x30]);
    prints("*=$1000\n.print toHexString(*)", &["1000"]);
}

#[test]
fn multi_labels() {
    assert_eq!(bytes("*=$1000\n!: dex\nbne !-"), [0xca, 0xd0, 0xfd]);
    assert_eq!(bytes("*=$1000\njmp !+\nnop\n!: nop"), [0x4c, 4, 0x10, 0xea, 0xea]);
    assert_eq!(bytes("*=$1000\njmp !++\n!: nop\n!: nop"), [0x4c, 4, 0x10, 0xea, 0xea]);
    assert_eq!(bytes("*=$1000\n!a: nop\njmp !a-\n!a: jmp !a-"), [0xea, 0x4c, 0, 0x10, 0x4c, 4, 0x10]);
    fails("!: nop\njmp !--", "Unknown symbol '!--'");
}

#[test]
fn label_scopes() {
    assert_eq!(bytes("*=$1000\ns: {\nx: nop\n}\njmp s.x"), [0xea, 0x4c, 0, 0x10]);
    assert_eq!(bytes("*=$1000\n.macro m() {\nz: nop\n}\nc: m()\njmp c.z"), [0xea, 0x4c, 0, 0x10]);
    assert_eq!(bytes("*=$1000\nl: .for (var i=0;i<2;i++) {\nq: nop\n}\njmp l[1].q"), [0xea, 0xea, 0x4c, 1, 0x10]);
    assert_eq!(bytes("*=$1000\n.var w=0\nl: .while (w<2) {\nq: nop\n.eval w++\n}\njmp l[1].q"), [0xea, 0xea, 0x4c, 1, 0x10]);
    assert_eq!(bytes("*=$1000\njmp i.e\ni: .if (true) {\nnop\ne: sei\n} else {\ne: cli\n}"), [0x4c, 4, 0x10, 0xea, 0x78]);
    prints(".label a = $1000 {\n.label b = 5\n}\n.print a.b", &["5"]);
}

#[test]
fn inner_labels_stay_inside() {
    assert_eq!(bytes("*=$1000\n{\nl: jmp l\n}\n{\nl: jmp l\n}"), [0x4c, 0, 0x10, 0x4c, 3, 0x10]);
    fails("{\nl: nop\n}\njmp l", "Unknown symbol 'l'");
    fails("l: nop\nl: nop", "The symbol 'l' is already defined");
}

#[test]
fn operand_labels() {
    assert_eq!(bytes("*=$1000\nlda v:#0\ninc v"), [0xa9, 0, 0xee, 1, 0x10]);
    assert_eq!(bytes("*=$1000\njmp t:($2000)\n.word t"), [0x6c, 0, 0x20, 1, 0x10]);
}

#[test]
fn root_labels() {
    assert_eq!(bytes("*=$1000\n{\n@r: nop\n}\njmp r"), [0xea, 0x4c, 0, 0x10]);
    prints(".label x = 1\n{\n.label x = 2\n.print x\n.print @x\n}", &["2", "1"]);
}

#[test]
fn pseudopc() {
    assert_eq!(bytes("*=$1000\n.pseudopc $c000 {\nc: jmp c\n}\nd: jmp d"), [0x4c, 0, 0xc0, 0x4c, 3, 0x10]);
}

#[test]
fn align_pads_with_zero() {
    assert_eq!(bytes("*=$10fe\nnop\n.align 4\nsei"), [0xea, 0, 0x78]);
    fails(".align 0", "Page size must be positive.");
}

#[test]
fn labels_work_before_their_definition_but_constants_do_not() {
    assert_eq!(bytes("inc b\n.label b = $d020"), [0xee, 0x20, 0xd0]);
    fails("inc b\n.const b = $d020", "Reference to not yet defined symbol 'b'");
    fails(".print v\n.var v = 1", "Reference to not yet defined symbol 'v'");
}

#[test]
fn values_defined_later_are_found_by_the_passes() {
    assert_eq!(bytes("*=$1000\n.fill n, 7\n.label n = 2"), [7, 7]);
    assert_eq!(bytes("*=$1000\n.if (n==2) nop\n.label n = 2"), [0xea]);
}

// ---------------------------------------------------------------- data

#[test]
fn fill() {
    assert_eq!(bytes(".fill 3, i*2"), [0, 2, 4]);
    assert_eq!(bytes(".fill 3, [1,2]"), [1, 2, 1, 2, 1, 2]);
    assert_eq!(bytes(".fill 2, [i,i*$10]"), [0, 0, 1, 0x10]);
    assert_eq!(bytes(".fillword 2, $1000+i"), [0, 0x10, 1, 0x10]);
    assert_eq!(bytes(".fillword 2, [$100,0]"), [0, 1, 0, 0, 0, 1, 0, 0]);
}

#[test]
fn lohifill() {
    assert_eq!(
        bytes("*=$1000\nt: .lohifill 2, $1234+i\nlda t.lo,x\nlda t.hi,x"),
        [0x34, 0x35, 0x12, 0x12, 0xbd, 0, 0x10, 0xbd, 2, 0x10]
    );
}

#[test]
fn imports() {
    assert_eq!(bytes(".import binary \"data.bin\""), [1, 2, 3, 4, 5]);
    assert_eq!(bytes(".import binary \"data.bin\", 1, 2"), [2, 3]);
    assert_eq!(bytes(".import binary \"data.bin\", 3"), [4, 5]);
    assert_eq!(bytes(".import c64 \"prog.prg\""), [9, 8, 7]);
    assert_eq!(bytes(".import c64 \"prog.prg\", 1, 1"), [8]);
    assert_eq!(bytes(".encoding \"ascii\"\n.import text \"scroll.txt\", 1"), [0x62, 0x63]);
    fails(".import binary \"nosuch.bin\"", "Can't open file 'nosuch.bin'");
}

// ---------------------------------------------------------------- variables

#[test]
fn variables() {
    prints(".var v\n.print v", &["Null"]);
    prints(".var x=1\n.eval x=2\n.eval x++\n.eval x+=3\n.eval x*=2\n.eval x-=1\n.eval x/=11\n.print x", &["1"]);
}

#[test]
fn assignment_returns_a_value() {
    prints(".var x=0\n.var y=0\n.eval x=y=25\n.print x+y", &["50"]);
    prints(".var x=0\n.var y=x++\n.print x\n.print y", &["1", "0"]);
}

#[test]
fn constants_cannot_change() {
    fails(".const c = 1\n.eval c = 2", "Can't assign to 'c' since it isn't a variable");
    fails(".const x = 1\n.const x = 2", "The symbol 'x' is already defined");
    fails("x: nop\n.label x = 1", "The symbol 'x' is already defined");
    fails(".var l = List().lock()\n.eval l.add(2)", "You can't modify a locked value");
}

#[test]
fn variable_scopes() {
    prints(".var x=1\n{\n.var x=2\n.print x\n}\n.print x", &["2", "1"]);
    prints(".var o=1\n{\n.eval o=5\n}\n.print o", &["5"]);
    fails("{\n.var x=1\n}\n.print x", "Unknown symbol 'x'");
}

#[test]
fn enums() {
    assert_eq!(bytes(".enum { A, B, C=10, D }\n.byte A, B, C, D"), [0, 1, 10, 2]);
}

#[test]
fn structs() {
    assert_eq!(bytes(".struct P {x,y}\n.var p=P(1,2)\n.eval p.x=9\n.byte p.x, p.y"), [9, 2]);
    prints(
        ".struct P {x,y}\n.var p=P(1,2)\n.var q=P()\n.print q.x\n.print p\n.print p.getStructName()\n.print p.getNoOfFields()\n.print p.getFieldNames().get(1)\n.print p.get(0)\n.print p.get(\"y\")\n.eval p.set(0,7)\n.eval p.set(\"y\",8)\n.print p.x+p.y",
        &["Null", "P-struct", "P", "2", "y", "1", "2", "15"],
    );
}

#[test]
fn define_blocks() {
    assert_eq!(bytes(".define a {\n.var b=10\n.var a=b/2\n}\n.byte a"), [5]);
    fails(".define a {\n.var b=1\n.var a=b\n}\n.print b", "Unknown symbol 'b'");
    fails(".define a {\n.var a=List()\n}\n.eval a.add(1)", "You can't modify a locked value");
    fails(".define a {\nnop\n.var a=1\n}", "Invalid token");
}

// ---------------------------------------------------------------- control flow

#[test]
fn if_forms() {
    assert_eq!(bytes(".if (true) nop\nelse sei"), [0xea]);
    assert_eq!(bytes(".if (false) nop\nelse sei"), [0x78]);
    assert_eq!(bytes(".if (false) { nop } else sei"), [0x78]);
    // on one line `else` is the argument of the instruction
    fails(".if (true) nop else sei", "If without body");
    assert_eq!(bytes(".if (false) nop\nelse .if (true) sei\nelse cli"), [0x78]);
    assert_eq!(bytes(".if (false) { nop }"), Vec::<u8>::new());
}

#[test]
fn for_forms() {
    assert_eq!(bytes(".for (var i=0, var j=5; i<2; i++, j--) .byte i, j"), [0, 5, 1, 4]);
    assert_eq!(bytes(".var n=0\n.for (n=0;n<3;n++) { }\n.byte n"), [3]);
    assert_eq!(bytes(".var i=0\n.for (;i<2;) {\n.byte i\n.eval i++\n}"), [0, 1]);
    assert_eq!(bytes(".for (var i=0;i<2;i++) .for (var j=0;j<2;j++) .byte i*2+j"), [0, 1, 2, 3]);
}

#[test]
fn while_loop() {
    assert_eq!(bytes(".var w=0\n.while (w<2) {\n.byte w\n.eval w++\n}"), [0, 1]);
}

// ---------------------------------------------------------------- functions, macros, pseudocommands

#[test]
fn functions() {
    assert_eq!(bytes(".function f(a) { .return a*a }\n.function f(a,b) { .return a*b }\n.byte f(3), f(2,5)"), [9, 10]);
    assert_eq!(bytes(".byte later(2)\n.function later(a) { .return a+1 }"), [3]);
    prints(".function n() { }\n.print n()\n.function r() { .return }\n.print r()", &["Null", "Null"]);
    fails(".function f() { .return 1 }\n.function f() { .return 2 }", "The function 'f' is already defined.");
    fails(".function f(a) { .return a }\n.print f()", "Invalid number of arguments for function 'f'");
    fails(".function f() { nop }\n.print f()", "Invalid token");
}

#[test]
fn macros() {
    assert_eq!(bytes("m(1)\n.macro m(a) { .byte a }\n.macro m(a,b) { .byte a,b }\nm(2,3)"), [1, 2, 3]);
    assert_eq!(bytes("*=$1000\n.macro w() {\nl: jmp l\n}\nw()\nw()"), [0x4c, 0, 0x10, 0x4c, 3, 0x10]);
    assert_eq!(bytes(".macro r(n) {\n.byte n\n.if (n>0) r(n-1)\n}\nr(2)"), [2, 1, 0]);
    assert_eq!(bytes(".macro m() { nop }\n:m()\nm()"), [0xea, 0xea]);
    fails(".macro m() { nop }\nm", "Pseudo command 'm' not defined");
    fails(".macro m(a) { }\nm()", "Invalid number of arguments for macro");
    fails(".macro m(n) { m(n+1) }\nm(0)", "Too deep recursion");
}

#[test]
fn pseudocommands_keep_the_addressing_mode() {
    let def = ".pseudocommand mov s : d {\nlda s\nsta d\n}\n";
    assert_eq!(bytes(&format!("{}mov #1 : $d020", def)), [0xa9, 1, 0x8d, 0x20, 0xd0]);
    assert_eq!(bytes(&format!("{}mov $10,x : $20,y", def)), [0xb5, 0x10, 0x99, 0x20, 0]);
    assert_eq!(bytes(&format!("{}mov ($10),y : ($20,x)", def)), [0xb1, 0x10, 0x81, 0x20]);
    fails(&format!("{}mov #1 : $10 : $20", def), "");
}

#[test]
fn pseudocommand_can_share_a_name_with_an_instruction() {
    assert_eq!(
        bytes(".pseudocommand adc a : b {\nlda a\nsta b\n}\nadc #$10\n:adc #1 : $20"),
        [0x69, 0x10, 0xa9, 1, 0x85, 0x20]
    );
}

#[test]
fn argument_types() {
    prints(
        ".pseudocommand t a { .print \"\" + a.getType() + \" \" + a.getValue() }\nt\nt #1\nt $10\nt $1000\nt $10,x\nt $10,y\nt ($1000)\nt ($10),y\nt ($10,x)",
        &["1 1", "-1 16", "-1 4096", "-2 16", "-3 16", "-4 4096", "6 16", "-5 16"],
    );
    prints(
        ".print AT_NONE\n.print AT_IMMEDIATE\n.print AT_ABSOLUTE\n.print AT_ABSOLUTEX\n.print AT_ABSOLUTEY\n.print AT_INDIRECT\n.print AT_IZEROPAGEX\n.print AT_IZEROPAGEY",
        &["0", "1", "-1", "-2", "-3", "-4", "5", "6"],
    );
}

#[test]
fn new_arguments() {
    assert_eq!(bytes(".var a = CmdArgument(AT_IMMEDIATE, 100)\nlda a"), [0xa9, 100]);
    let src = ".pseudocommand inc16 a {\ninc a\nbne !+\ninc CmdArgument(a.getType(), a.getValue()+1)\n!:\n}\n*=$1000\ninc16 $1000";
    assert_eq!(bytes(src), [0xee, 0, 0x10, 0xd0, 3, 0xee, 1, 0x10]);
}

#[test]
fn missing_argument_is_at_none() {
    let src = ".pseudocommand p a : t {\n.if (t.getType()==AT_NONE) .eval t=a\nsta t\n}\np $10\np $10 : $20";
    assert_eq!(bytes(src), [0x85, 0x10, 0x85, 0x20]);
}

// ---------------------------------------------------------------- namespaces

#[test]
fn namespaces() {
    assert_eq!(bytes("*=$1000\n.namespace n {\nl: nop\n.label K = 5\n}\njmp n.l\n.byte n.K"), [0xea, 0x4c, 0, 0x10, 5]);
    assert_eq!(bytes("*=$1000\n.namespace n {\na: nop\n}\n.namespace n {\njmp a\n}"), [0xea, 0x4c, 0, 0x10]);
    prints(
        ".print getNamespace()\n.namespace A {\n.print getNamespace()\n.namespace B {\n.print getNamespace()\n}\n}",
        &["<RootNS>", "A", "A.B"],
    );
}

#[test]
fn namespace_functions_stay_inside() {
    fails(".namespace n {\n.macro m() { }\n}\nm()", "Macro command 'm' not defined");
    fails(".namespace n {\n.function f() { .return 1 }\n}\n.print n.f()", "Unknown function : f");
}

#[test]
fn publishing_to_the_root() {
    let src = ".namespace n {\n.macro @m() { nop }\n.function @f() { .return 7 }\n.const @K = 5\n.var @V = 4\n.struct @S { a }\n@l: sei\n.pseudocommand @p { cli }\n}\nm()\np\n.byte f(), K, V, S(9).a";
    assert_eq!(bytes(src), [0x78, 0xea, 0x58, 7, 5, 4, 9]);
}

#[test]
fn calling_the_root_version() {
    prints(
        ".function f() { .return \"root\" }\n.namespace n {\n.function f() { .return \"inner\" }\n.print f()\n.print @f()\n}",
        &["inner", "root"],
    );
    assert_eq!(bytes(".macro m() { nop }\n.namespace n {\n.macro m() { sei }\nm()\n:@m()\n}"), [0x78, 0xea]);
}

#[test]
fn file_namespace() {
    let p = Project::new();
    p.write("lib.asm", ".filenamespace lib\ninit: rts\n");
    assert_eq!(p.asm("*=$1000\n#import \"lib.asm\"\n", &[]).code, 0);
    p.write("t.asm", "*=$1000\n.import source \"lib.asm\"\njmp lib.init\n");
    let r = p.run(&["t.asm"]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(p.read("t.prg")[2..], [0x60, 0x4c, 0, 0x10]);
}

// ---------------------------------------------------------------- preprocessor

#[test]
fn preprocessor_conditions() {
    assert_eq!(bytes("#define A\n#if A\n.byte 1\n#else\n.byte 2\n#endif"), [1]);
    assert_eq!(bytes("#if A\n.byte 1\n#elif B\n.byte 2\n#else\n.byte 3\n#endif"), [3]);
    assert_eq!(bytes("#define A\n#undef A\n#if A\n.byte 1\n#else\n.byte 2\n#endif"), [2]);
    assert_eq!(bytes("#define A\n#define B\n#if (A && !C) || D\n.byte 1\n#endif"), [1]);
    assert_eq!(bytes("#define A\n#if A == B\n.byte 1\n#endif\n#if A != B\n.byte 2\n#endif"), [2]);
    assert_eq!(bytes("#if X\nthis is never read\n#endif\nnop"), [0xea]);
    assert_eq!(bytes("#define A\n#if A\n#if B\n.byte 1\n#else\n.byte 2\n#endif\n#endif"), [2]);
}

#[test]
fn preprocessor_imports() {
    let p = Project::new();
    p.write("once.asm", "#importonce\n.byte 1\n");
    p.write("many.asm", ".byte 2\n");
    p.write("sub/deep.asm", "#import \"near.asm\"\n");
    p.write("sub/near.asm", ".byte 3\n");
    let r = p.asm("#import \"once.asm\"\n#import \"once.asm\"\n#import \"many.asm\"\n#import \"many.asm\"\n#import \"sub/deep.asm\"\n#define Y\n#importif Y \"many.asm\"\n#importif !Y \"many.asm\"\n", &[]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(p.read("t.prg")[2..], [1, 2, 2, 3, 2]);
}

#[test]
fn dot_import_source_and_importonce() {
    let p = Project::new();
    p.write("h.asm", ".importonce\n.byte 1\n");
    let r = p.asm(".import source \"h.asm\"\n.import source \"h.asm\"\n", &[]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(p.read("t.prg")[2..], [1]);
}

// ---------------------------------------------------------------- script library

#[test]
fn math() {
    prints(
        ".print round(2.5)\n.print round(-2.5)\n.print mod(-7,3)\n.print mod(7.9,3)\n.print floor(-1.5)\n.print ceil(1.2)\n.print abs(-2)\n.print max(1,2)\n.print min(1,2)\n.print pow(2,8)\n.print sqrt(16)\n.print hypot(3,4)\n.print signum(-5)\n.print cbrt(27)\n.print log10(100)\n.print toDegrees(PI)\n.print toRadians(180)\n.print sin(0)\n.print cos(0)\n.print IEEEremainder(7,3)",
        &["3", "-2", "-1", "1", "-2", "2", "2", "2", "1", "256", "4", "5", "-1", "3", "2", "180", "3.141592653589793", "0", "1", "1"],
    );
    prints(".print PI\n.print E", &["3.141592653589793", "2.718281828459045"]);
    prints(".var r = random()\n.print r>=0 && r<1", &["true"]);
}

#[test]
fn numbers_to_text() {
    prints(
        ".print \"[\"+toIntString(7,3)+\"]\"\n.print toIntString(16.9)\n.print toHexString(255,4)\n.print toHexString(255)\n.print toBinaryString(5,8)\n.print toBinaryString(5)\n.print toOctalString(8)\n.print toOctalString(16,4)",
        &["[  7]", "16", "00ff", "ff", "00000101", "101", "10", "0020"],
    );
}

#[test]
fn string_methods() {
    prints(
        ".print \"abc\".size()\n.print \"abc\".charAt(1)\n.print \"abcd\".substring(1,3)\n.print \"AbC\".toLowerCase()\n.print \"abc\".toUpperCase()\n.print \"35\".asNumber()+1\n.print \"f\".asNumber(16)\n.print \"12\".number()\n.print \"true\".asBoolean()\n.print \"x\".asBoolean()\n.print 1234.string().charAt(2)\n.print (5).string()",
        &["3", "b", "bc", "abc", "ABC", "36", "15", "12", "true", "false", "3", "5"],
    );
}

#[test]
fn lists() {
    prints(
        ".var l = List().add(3,1,2)\n.print l.size()\n.print l.get(0)\n.print l[1]\n.eval l.sort()\n.print l\n.eval l.reverse()\n.print l\n.eval l.set(0,9)\n.print l.remove(0)\n.print l\n.eval l.addAll(List().add(7))\n.print l\n.print List(2)\n.print [1,2]",
        &["3", "3", "1", "[1,2,3]", "[3,2,1]", "9", "[2,1]", "[2,1,7]", "[Null,Null]", "[1,2]"],
    );
    prints(".var l = List().add(1,2,3)\n.eval l.shuffle()\n.print l.size()", &["3"]);
    fails(".var l = List()\n.print l.get(5)", "Index out of bound : 5");
}

#[test]
fn hashtables() {
    prints(
        ".var h = Hashtable().put(\"a\",1,\"b\",2).put(3,\"c\")\n.print h.get(\"a\")\n.print h.get(3)\n.print h.get(\"3\")\n.print h.get(\"z\")\n.print h.containsKey(\"b\")\n.print h.containsKey(\"z\")\n.print h.keys().size()\n.eval h.remove(\"a\")\n.print h.containsKey(\"a\")\n.print h",
        &["1", "c", "c", "Null", "true", "false", "3", "false", "Hashtable"],
    );
}

#[test]
fn equality() {
    prints(
        ".var a = List().add(1)\n.print a==a\n.print a==List().add(1)\n.print 1==\"1\"\n.print \"a\"==\"a\"\n.print Vector(1,2,3)==Vector(1,2,3)\n.print true!=false\n.print null==null\n.print a==null",
        &["true", "false", "false", "true", "true", "true", "true", "false"],
    );
}

#[test]
fn vectors() {
    prints(
        ".var v=Vector(1,2,3)\n.print v\n.print v+Vector(1,1,1)\n.print v-Vector(1,1,1)\n.print v*Vector(1,1,1)\n.print v*2\n.print Vector(2,4,6)/2\n.print -v\n.print Vector(0,1,0).X(Vector(1,0,0))\n.print v.getX()\n.print v.getY()\n.print v.getZ()\n.print v.get(1)",
        &["(1.0,2.0,3.0)", "(2.0,3.0,4.0)", "(0.0,1.0,2.0)", "6", "(2.0,4.0,6.0)", "(1.0,2.0,3.0)", "(-1.0,-2.0,-3.0)", "(0.0,0.0,-1.0)", "1", "2", "3", "2"],
    );
}

#[test]
fn matrices() {
    prints(
        ".var m=Matrix().set(2,3,100)\n.print m.get(2,3)\n.print m.get(0,0)\n.print MoveMatrix(1,0,0)*Vector(1,2,3)\n.print ScaleMatrix(2,2,2)*Vector(1,2,3)\n.print PerspectiveMatrix(2)*Vector(1,2,4)\n.print (MoveMatrix(1,0,0)*MoveMatrix(2,0,0))*Vector(0,0,0)\n.print Matrix()",
        &["100", "1", "(2.0,2.0,3.0)", "(2.0,4.0,6.0)", "(0.5,1.0,0.0)", "(3.0,0.0,0.0)", "[[1.0,0.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]]"],
    );
    // the angle is in radians
    prints(".var v = RotationMatrix(0,0,PI)*Vector(1,0,0)\n.print round(v.getX())", &["-1"]);
}

#[test]
fn load_binary() {
    assert_eq!(bytes(".var f = LoadBinary(\"data.bin\")\n.fill f.getSize(), f.get(i)"), [1, 2, 3, 4, 5]);
    prints(
        ".var f = LoadBinary(\"prog.prg\", \"C64FILE, A=0, B=1\")\n.print f.getASize()\n.print f.getBSize()\n.print f.getA(0)\n.print f.ugetB(1)",
        &["1", "2", "9", "7"],
    );
    prints(".print BF_KOALA", &["C64FILE, Bitmap=$0000, ScreenRam=$1f40, ColorRam=$2328, BackgroundColor = $2710"]);
    prints(".print BF_C64FILE", &["C64FILE"]);
}

#[test]
fn signed_and_unsigned_bytes() {
    let p = Project::new();
    p.write("x.bin", [0xffu8]);
    let r = p.asm(".var f = LoadBinary(\"x.bin\")\n.print f.get(0)\n.print f.uget(0)", &["-nooutput"]);
    assert!(r.has_line("-1") && r.has_line("255"), "{}", r.out);
}

#[test]
fn load_sid() {
    prints(
        ".var m = LoadSid(\"tune.sid\")\n.print m.header\n.print m.version\n.print toHexString(m.location)\n.print toHexString(m.init)\n.print toHexString(m.play)\n.print m.songs\n.print m.startSong\n.print m.name\n.print m.size",
        &["PSID", "2", "1000", "1000", "1003", "1", "1", "Tune", "4"],
    );
    assert_eq!(bytes(".var m = LoadSid(\"tune.sid\")\n.fill m.size, m.getData(i)"), [0x60, 0x61, 0x62, 0x63]);
}

#[test]
fn load_picture() {
    prints(
        ".var p = LoadPicture(\"picture.png\")\n.print p.width\n.print p.height\n.print toHexString(p.getPixel(0,0))",
        &["16", "8", "ffffff"],
    );
    // default table: black is 0, so white is 1
    assert_eq!(bytes(".var p = LoadPicture(\"picture.png\")\n.byte p.getSinglecolorByte(1,7), p.getMulticolorByte(0,0)"), [0xff, 0x55]);
    // given table: white is first
    assert_eq!(bytes(".var p = LoadPicture(\"picture.png\", List().add($ffffff,$000000))\n.byte p.getSinglecolorByte(0,0), p.getMulticolorByte(0,0)"), [0, 0]);
    fails(".var p = LoadPicture(\"nosuch.png\")", "Can't locate file 'nosuch.png'");
}

#[test]
fn source_information() {
    prints(".print getFilename()", &["t.asm"]);
}

#[test]
fn basic_upstart() {
    // 10 SYS 2064, at $0801
    let p = prg("BasicUpstart2(start)\n*=$0810\nstart: rts");
    assert_eq!(p[..14], [0x01, 0x08, 0x0b, 0x08, 0x0a, 0x00, 0x9e, b'2', b'0', b'6', b'4', 0, 0, 0]);
    assert_eq!(*p.last().unwrap(), 0x60);
    assert_eq!(p.len(), 2 + 0x10);
    assert_eq!(bytes("*=$0801\nBasicUpstart($1000)")[4..9], [0x9e, b'4', b'0', b'9', b'6']);
}

// ---------------------------------------------------------------- asserts and messages

#[test]
fn assert_directives() {
    let p = Project::new();
    let r = p.asm(".assert \"good\",1+1,2\n.assert \"bad\",1,2\n.asserterror \"err\",1+\"a\"\n.asserterror \"noerr\",1", &["-nooutput"]);
    assert_eq!(r.code, 0, "a failed assert doesn't change the exit code:\n{}", r.out);
    assert!(r.has_line("good=2 (2)"), "{}", r.out);
    assert!(r.has_line("bad=1 (2) -- ERROR IN ASSERTION!!!"), "{}", r.out);
    assert!(r.out.contains("err -- OK. |"), "{}", r.out);
    assert!(r.out.contains("Made 4 asserts , 2 FAILED!"), "{}", r.out);
}

#[test]
fn print_runs_once_and_printnow_each_pass() {
    let p = Project::new();
    let r = p.asm("jmp l\n.print \"once\"\n.printnow \"every\"\nl:", &["-nooutput"]);
    assert_eq!(r.lines().iter().filter(|l| **l == "once").count(), 1, "{}", r.out);
    assert!(r.lines().iter().filter(|l| **l == "every").count() >= 2, "{}", r.out);
}

#[test]
fn user_errors() {
    fails(".error \"stop here\"", "stop here");
    fails(".errorif 1>0, \"too big\"", "too big");
    assert_eq!(bytes(".errorif 1>2, \"too big\"\nnop"), [0xea]);
    assert_eq!(bytes("*=$1000\nbeq l\n.errorif (>*) != (>l), \"page\"\nnop\nl:"), [0xf0, 1, 0xea]);
}

#[test]
fn plugins_are_not_supported() {
    for src in [".plugin \"x\"", ".modify M() { nop }", ".filemodify M()"] {
        let p = Project::new();
        assert_eq!(p.asm(src, &["-nooutput"]).code, 1, "{}", src);
    }
}
