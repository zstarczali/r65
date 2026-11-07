//! Things the Kick Assembler reference manual describes, checked one by one.
//!
//! The expected results come from running KickAss.jar 5.25, also where its manual says
//! something else (see the table of differences in docs/R65-Manual.md).
mod common;
use common::{bytes, fails, prints, Project};

#[test]
fn assert_compares_code_blocks() {
    let p = Project::new();
    let r = p.asm(".assert \"same\", { lda $1000 }, { lda $1000 }\n.assert \"diff\", { lda $1000 }, { ldx $1000 }", &["-nooutput"]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("Made 2 asserts , 1 FAILED!"), "{}", r.out);
}

#[test]
fn asserterror_takes_a_code_block() {
    let p = Project::new();
    let r = p.asm(".asserterror \"t\", { lda #\"must fail\" }", &["-nooutput"]);
    assert_eq!(r.code, 0, "{}", r.out);
}

#[test]
fn line_breaks_only_count_outside_brackets() {
    // the manual says an expression goes on after an operator, but the original stops there
    fails(".var q = 1 +\n  2\n.print q", "Syntax error");
    fails(".var q = true ?\n  1 :\n  2", "Syntax error");
    fails(".byte 1,\n 2", "Syntax error");
    prints(".var q = (1 +\n  2)\n.print q", &["3"]);
    prints(".var q = (true ?\n  1 :\n  2)\n.print q", &["1"]);
    prints(".var q = [1 +\n 2]\n.print q", &["3"]);
    assert_eq!(bytes("lda (\n$10\n),y"), [0xb1, 0x10]);
    assert_eq!(bytes(".for (var i=0;\n i<2;\n i++) nop"), [0xea, 0xea]);
    assert_eq!(bytes(".if (1 ==\n 1) nop"), [0xea]);
}

#[test]
fn loop_labels_work_before_the_loop() {
    assert_eq!(
        bytes("*=$1000\n.for (var i=0;i<2;i++) lda loop2[i].c\nloop2: .for (var i=0;i<2;i++) {\nc: nop\n}"),
        [0xad, 6, 0x10, 0xad, 7, 0x10, 0xea, 0xea]
    );
}

#[test]
fn root_macro_call_without_a_colon() {
    assert_eq!(bytes(".macro m() { nop }\n.namespace n {\n.macro m() { sei }\n@m()\n}"), [0xea]);
}

#[test]
fn percent_o_in_every_file_name() {
    let p = Project::new();
    assert_eq!(p.asm(".disk [filename=\"%o.d64\"]\n{\n}\n.segment A [outPrg=\"%o_a.prg\"]\nnop\n.segment B [outBin=\"%o_b.bin\"]\nnop", &[]).code, 0);
    assert!(p.exists("t.d64"));
    assert!(p.exists("t_a.prg"));
    assert!(p.exists("t_b.bin"));
}

#[test]
fn segment_limits_without_output() {
    fails(
        ".segment D [start=$c000, min=$c000, max=$cfff]\n.fill $1800, 0",
        "In segment 'D' the memoryblock 'D' ($c000-$d7ff) is out of segment bounds ($c000-$cfff).",
    );
}

#[test]
fn zero_page_suffix_needs_a_zero_page_mode() {
    // the manual shows jmp.z as allowed, but the original refuses it
    fails("*=$1000\njmp.z $1000", "'jmp' does not support mode 'zeropage'");
    fails("*=$1000\njmp.z $1000,x", "'jmp' does not support mode 'zeropageX'");
    fails("*=$1000\ninc.z $1000,y", "'inc' does not support mode 'zeropageY'");
    fails("*=$1000\nbne.z $1000", "'bne' does not support mode 'zeropage'");
    assert_eq!(bytes("*=$1000\nlda.z $1000"), [0xa5, 0]);
}

#[test]
fn list_in_a_constant_can_change() {
    prints(".const l = List().add(1)\n.eval l.add(2)\n.print l.size()", &["2"]);
}

#[test]
fn start_address_only_on_prg_files_by_default() {
    // the manual gives false as the default for all types; the original skips the address on the others
    let p = Project::new();
    let src = |ty: &str, more: &str| format!(".disk [filename=\"x.d64\"] {{\n[name=\"S\", type=\"{}\"{}, segments=\"C\"]\n}}\n.segment C [start=$1000]\n.byte 1", ty, more);
    for ty in ["seq", "usr", "rel", "del"] {
        assert_eq!(p.asm(&src(ty, ""), &[]).code, 0);
        assert_eq!(p.read("x.d64")[2..3], [1], "{}", ty);
    }
    assert_eq!(p.asm(&src("seq", ", noStartAddr=false"), &[]).code, 0);
    assert_eq!(p.read("x.d64")[2..5], [0, 0x10, 1]);
    assert_eq!(p.asm(&src("prg", ""), &[]).code, 0);
    assert_eq!(p.read("x.d64")[2..5], [0, 0x10, 1]);
}

#[test]
fn memory_map_lists_imported_files() {
    let p = Project::new();
    p.write("base.prg", [0u8, 0x38, 1, 2]);
    let r = p.asm(".segmentdef B [prgFiles=\"base.prg\"]\n.file [name=\"o.prg\", segments=\"B\"]", &["-showmem"]);
    assert!(r.out.contains("$3800-$3801 base.prg"), "{}", r.out);
}

#[test]
fn indirect_x_argument_type() {
    // the manual lists AT_IZEROPAGEX, but ($10,x) has the type AT_INDIRECTX
    prints(".pseudocommand t a { .print a.getType()\n.print a.getType()==AT_IZEROPAGEX }\nt ($10,x)", &["-5", "false"]);
}

#[test]
fn one_symbol_of_a_name_in_a_scope() {
    let early = |src: &str, at: &str| {
        let p = Project::new();
        let r = p.asm(src, &["-nooutput"]);
        assert_eq!(r.code, 1, "{}", r.out);
        assert!(r.out.contains(at), "{}\n{}", at, r.out);
        // it's found before the first pass
        assert!(!r.out.contains("flex pass"), "{}", r.out);
    };
    early(".var x = 1\n.var x = 2", "The symbol 'x' is already defined\nat line 2, column 6");
    early(".const x = 1\n.const x = 2", "at line 2, column 8");
    early(".label x = 1\n.label x = 2", "at line 2, column 8");
    early("x: nop\nx: nop", "at line 2, column 1");
    early(".var x = 1\n.eval var x = 2", "at line 2, column 7");
    early(".enum { a, a }", "at line 1, column 12");
    early("x: nop\n{\n@x: nop\n}", "The symbol '@x' is already defined");
    // also in code that never runs
    early(".macro m() {\nx: nop\nx: nop\n}", "at line 3, column 1");
    early(".if (false) {\n.var x = 1\n.var x = 2\n}", "at line 3, column 6");
    // namespaces share one set of names, and their own name counts too
    early(".namespace n {\nx: nop\n}\n.namespace n {\nx: nop\n}", "at line 5, column 1");
    early(".namespace n {\n}\nn: nop", "The symbol 'n' is already defined");
}

#[test]
fn matrix_set_changes_the_matrix() {
    prints(".var m = Matrix()\n.eval m.set(2,3,100)\n.print m.get(2,3)", &["100"]);
}

/// Not from the manual: the program should report this and not crash.
#[test]
fn endless_function_recursion_reports_an_error() {
    fails(".function f(n) { .return f(n+1) }\n.print f(0)", "Too deep recursion");
}

#[test]
fn missing_options() {
    let p = Project::new();
    p.write("my.cfg", "-showmem\n");
    for args in [
        &["-asminfo", "all"][..],
        &["-asminfo", "all", "-asminfofile", "a.txt"],
        &["-asminfo", "errors", "-asminfotostdout"],
        &["-cfgfile", "my.cfg"],
        &["-debug"],
        &["-debugdump"],
        &["-execute", "x", "-executelog", "e.txt"],
        &["-pseudoc3x"],
    ] {
        let r = p.asm("nop", args);
        assert!(!r.out.contains("Unknown option"), "{:?}\n{}", args, r.out);
    }
}

#[test]
fn assert_code_output() {
    prints(
        ".assert \"same\", { lda $1000 }, { lda $1000 }\n.assert \"diff\", { lda $1000 }, { ldx $1000 }",
        &["same -- OK. | 2000:ad,00,10", "diff -- FAILED! | 2000:ad,00,10  -- 2000:ae,00,10"],
    );
    // the manual's multi line form, with a loop
    prints(
        ".assert \"Test\", {\n.for (var i=0; i<2; i++)\nsta $0400+i\n}, {\nsta $0400\nsta $0401\n}",
        &["Test -- OK. | 2000:8d,00,04,8d,01,04"],
    );
    prints(".assert \"Two\", { *=$3000\nnop\n*=$4000\nsei }, { *=$3000\nnop\n*=$4000\nsei }", &["Two -- OK. | 3000:ea; 4000:78"]);
}

#[test]
fn assert_code_adds_nothing_to_the_program() {
    assert_eq!(bytes("*=$1000\nnop\n.assert \"t\", { lda $1000 }, { lda $1000 }\nsei"), [0xea, 0x78]);
    // labels work inside the block, also forward ones, and end up in the VICE file
    let p = Project::new();
    let r = p.asm("*=$1000\nnop\n.assert \"t\", { jmp inside\ninside: }, { jmp $2003 }", &["-vicesymbols"]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("t -- OK. | 2000:4c,03,20"), "{}", r.out);
    assert!(p.text("t.vs").contains("inside"), "{}", p.text("t.vs"));
}

#[test]
fn asserterror_code_results() {
    let p = Project::new();
    let r = p.asm(".asserterror \"t\", { lda #\"must fail\" }\n.asserterror \"u\", { nop }\nsei", &[]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("t -- OK. | The value of a Command Argument Value must be an integer"), "{}", r.out);
    assert!(r.has_line("u -- FAILED!"), "{}", r.out);
    assert!(r.out.contains("Made 2 asserts , 1 FAILED!"), "{}", r.out);
    assert_eq!(p.read("t.prg")[2..], [0x78]);
}

#[test]
fn root_pseudocommand_call_without_a_colon() {
    assert_eq!(bytes(".pseudocommand p { nop }\n.namespace n {\n.pseudocommand p { sei }\n@p\n}"), [0xea]);
}

#[test]
fn pseudocommand_arguments_with_a_semicolon() {
    let src = ".pseudocommand p a:b { lda a\nsta b }\np #1;$d020";
    let p = Project::new();
    assert_eq!(p.asm(src, &["-pseudoc3x"]).code, 0);
    assert_eq!(p.read("t.prg")[2..], [0xa9, 1, 0x8d, 0x20, 0xd0]);
    // the colon still works with the option
    assert_eq!(p.asm(".pseudocommand p a:b { lda a\nsta b }\np #1 : $d020", &["-pseudoc3x"]).code, 0);
    assert_eq!(p.read("t.prg")[2..], [0xa9, 1, 0x8d, 0x20, 0xd0]);
    // without the option the semicolon ends the statement
    let r = p.asm(src, &[]);
    assert_eq!(r.code, 1);
    assert!(r.out.contains("Invalid token"), "{}", r.out);
}

#[test]
fn parameters_cant_be_defined_again() {
    fails(".macro m(a) {\n.var a = 2\n.byte a\n}\nm(1)", "The symbol 'a' is already defined");
    fails(".function f(a) {\n.var a = 2\n.return a\n}\n.print f(1)", "The symbol 'a' is already defined");
    fails(".macro m(a) {\na: nop\n}\nm(1)", "The symbol 'a' is already defined");
}

#[test]
fn a_new_scope_can_use_the_name_again() {
    assert_eq!(bytes(".var x = 1\n{\n.var x = 2\n.byte x\n}\n.for (var i=0;i<2;i++) {\n.var x = i\n.byte x\n}\n.byte x"), [2, 0, 1, 1]);
    fails("x: nop\n.var x = 2", "The symbol 'x' is already defined");
    fails(".var x = 2\nx: nop", "The symbol 'x' is already defined");
}

#[test]
fn a_matrix_is_shared_like_a_list() {
    prints(".var m = Matrix()\n.var n = m\n.eval n.set(0,1,5)\n.print m.get(0,1)", &["5"]);
    prints(".var m = Matrix().set(0,1,5)\n.var n = m*Matrix()\n.eval n.set(0,1,6)\n.print m.get(0,1)", &["5"]);
}

#[test]
fn deep_recursion_works() {
    prints(".function sum(n) {\n.if (n==0) .return 0\n.return n+sum(n-1)\n}\n.print sum(500)", &["125250"]);
    assert_eq!(bytes(".macro m(n) {\n.if (n>0) m(n-1)\n}\nm(300)\nnop"), [0xea]);
    fails(".macro m(n) { m(n+1) }\nm(0)", "Too deep recursion");
}

#[test]
fn define_still_locks() {
    fails(".define l {\n.var l = List()\n}\n.eval l.add(2)", "You can't modify a locked value");
}

#[test]
fn limits_of_a_segment_nobody_writes() {
    fails(".segment D [start=$0f00, min=$1000]\n.byte 1", "In segment 'D' the memoryblock 'D' ($0f00-$0f00) is out of segment bounds ($1000- ).");
    fails(".segmentdef D [max=$1000]\n.segment D\n*=$1000\n.byte 1,2", "is out of segment bounds ($0000-$1000).");
    // virtual blocks count too
    fails(".segmentdef D [virtual, start=$1000, max=$1000]\n.segment D\n.byte 1,2", "is out of segment bounds ($0000-$1000).");
    // and the blocks that a segment takes from another one, also when nothing writes it
    fails(".segmentdef A [start=$1000]\n.segmentdef B [segments=\"A\", max=$1000]\n.segment A\n.byte 1,2", "In segment 'B' the memoryblock");
    // the name of a .file is shown as an intermediate segment
    fails(".segmentdef A [start=$1000]\n.file [name=\"o.prg\", segments=\"A\", max=$1000]\n.segment A\n.byte 1,2", "In segment '<<.file:o.prg>>' the memoryblock");
}

fn dir_entry(img: &[u8], i: usize) -> (u8, u8, u8) {
    let o = 0x16600 + 32 * i;
    (img[o + 3], img[o + 4], img[o + 30])
}

#[test]
fn disk_formats() {
    let p = Project::new();
    assert_eq!(p.asm(".disk [filename=\"a.d64\", format=\"commodore\"] {\n}", &[]).code, 0);
    assert_eq!(p.read("a.d64").len(), 174848);
    for (name, at) in [("speeddos", 0xb0), ("dolphindos", 0xc4)] {
        assert_eq!(p.asm(&format!(".disk [filename=\"a.d64\", format=\"{}\"] {{\n}}", name), &[]).code, 0);
        let img = p.read("a.d64");
        assert_eq!(img.len(), 196608, "{}", name);
        // track 36 is free: 17 sectors
        assert_eq!(img[0x16500 + at], 17, "{}", name);
    }
    let r = p.asm(".disk [filename=\"a.d64\", format=\"cbm\"] {\n}", &[]);
    assert_eq!(r.code, 1);
    assert!(r.out.contains("Unknown litteral 'cbm'\nat line 1, column 26"), "{}", r.out);
}

#[test]
fn disk_with_40_tracks_holds_more() {
    let src = |f: &str| format!(".disk [filename=\"a.d64\"{}] {{\n[name=\"B\", type=\"prg\", noStartAddr, segments=\"C\"]\n}}\n.segment C [start=$1000]\n.fill 254*700, 0", f);
    let p = Project::new();
    let r = p.asm(&src(""), &[]);
    assert_eq!(r.code, 1);
    assert!(r.out.contains("Disk is full!"), "{}", r.out);
    assert_eq!(p.asm(&src(", format=\"speeddos\""), &[]).code, 0);
}

#[test]
fn files_in_the_directory_track() {
    // 664 sectors are free outside track 18, and 17 on it
    let src = |f: &str, n: usize| format!(".disk [filename=\"a.d64\"{}] {{\n[name=\"B\", type=\"prg\", noStartAddr, segments=\"C\"]\n}}\n.segment C [start=$1000]\n.fill 254*{}, 0", f, n);
    let p = Project::new();
    assert_eq!(p.asm(&src("", 664), &[]).code, 0);
    assert_eq!(p.asm(&src("", 665), &[]).code, 1);
    assert_eq!(p.asm(&src(", storeFilesInDir", 665), &[]).code, 0);
    assert_eq!(p.asm(&src(", storeFilesInDir", 681), &[]).code, 0);
    assert_eq!(p.asm(&src(", storeFilesInDir", 682), &[]).code, 1);
}

#[test]
fn files_not_split_over_the_directory() {
    // tracks 1-17 have 357 sectors: 350 are used, and 10 more would cross track 18
    let src = |f: &str| {
        format!(
            ".disk [filename=\"a.d64\"{}] {{\n[name=\"A\", type=\"prg\", noStartAddr, segments=\"X\"],\n[name=\"B\", type=\"prg\", noStartAddr, segments=\"Y\"]\n}}\n.segment X [start=$1000]\n.fill 254*350, 0\n.segment Y [start=$1000]\n.fill 254*10, 0",
            f
        )
    };
    let p = Project::new();
    assert_eq!(p.asm(&src(""), &[]).code, 0);
    assert!(dir_entry(&p.read("a.d64"), 1).0 < 18);
    assert_eq!(p.asm(&src(", dontSplitFilesOverDir"), &[]).code, 0);
    let img = p.read("a.d64");
    assert_eq!(dir_entry(&img, 0).0, 1);
    assert_eq!(dir_entry(&img, 1), (19, 0, 10));
}

#[test]
fn disk_info_is_printed_on_request() {
    let p = Project::new();
    let r = p.asm(".disk [filename=\"a.d64\", showInfo] {\n[name=\"A\", type=\"prg\", segments=\"X\"],\n[name=\"H\", type=\"prg\", hide, segments=\"X\"]\n}\n.segment X [start=$1000]\nnop", &[]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("Info for disk 'UNNAMED'"), "{}", r.out);
    assert!(r.out.contains("\"A\", sectors=1, startTS=1:0"), "{}", r.out);
    assert!(r.out.contains("\"H\", sectors=1, startTS=1:10"), "{}", r.out);
}

#[test]
fn asminfo_sections() {
    let p = Project::new();
    assert_eq!(p.asm("x: nop\njmp x", &["-asminfo", "all"]).code, 0);
    let s = p.text("asminfo.txt").replace("\r\n", "\n");
    for part in ["[version]\n1.", "[libraries]\nMath;constant;PI\n", "Math;function;abs;1\n", "[directives]\n", ".text;.text \"hello\";", "[ppdirectives]\n#define;#define DEBUG;", "[errors]\n[syntax]\n", "[files]\n0;", "1;r65:autoinclude.asm\n", "mnemonic;2,1,2,3,0\n", "symbolReference;2,5,2,5,0\n"] {
        assert!(s.contains(part), "{}\n{}", part, s);
    }
    assert_eq!(s.matches(";function;").count() + s.matches(";constant;").count(), 352);
    // the line ends are the ones of the original on Windows
    assert!(p.text("asminfo.txt").starts_with("[version]\r\n"));
}

#[test]
fn asminfo_selections() {
    let p = Project::new();
    assert_eq!(p.asm("nop", &["-asminfo", "version|errors", "-asminfofile", "a.txt"]).code, 0);
    let s = p.text("a.txt");
    assert!(s.starts_with("[version]\r\n") && s.ends_with("[errors]\r\n") && !s.contains("[files]"), "{}", s);
    assert_eq!(p.asm("nop", &["-asminfo", "allPredefined", "-asminfofile", "a.txt"]).code, 0);
    let s = p.text("a.txt");
    assert!(s.contains("[ppdirectives]") && !s.contains("[syntax]"), "{}", s);
    assert_eq!(p.asm("nop", &["-asminfo", "allSourceSpecific", "-asminfofile", "a.txt"]).code, 0);
    let s = p.text("a.txt");
    assert!(s.contains("[syntax]") && !s.contains("[libraries]"), "{}", s);
    // [files] only names the files that other sections refer to
    assert_eq!(p.asm("nop", &["-asminfo", "files", "-asminfofile", "a.txt"]).code, 0);
    assert_eq!(p.text("a.txt"), "[files]\r\n");
    // a selection that doesn't exist is only a warning
    let r = p.asm("nop", &["-asminfo", "version|nothing"]);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("Invalid asminfo type: nothing"), "{}", r.out);
}

#[test]
fn asminfo_reports_errors() {
    let p = Project::new();
    let r = p.asm("nop\nlda #\"x\"", &["-asminfo", "errors"]);
    assert_eq!(r.code, 1);
    assert!(p.text("asminfo.txt").starts_with("[errors]\r\nError;2,6,2,8,0;The value of a Command Argument Value must be an integer"), "{}", p.text("asminfo.txt"));
    // the file with the error gets the first index
    p.write("o.asm", "foo\n");
    let r = p.asm("#import \"o.asm\"\nnop", &["-asminfo", "errors|syntax|files"]);
    assert_eq!(r.code, 1);
    let s = p.text("asminfo.txt");
    assert!(s.contains("Error;1,1,1,3,0;Pseudo command 'foo' not defined\r\n"), "{}", s);
    assert!(s.contains("ppDirective;1,1,1,7,1\r\n"), "{}", s);
    assert!(s.contains("pseudoCommandExecution;1,1,1,3,0\r\n"), "{}", s);
    // a syntax error is reported by -noeval too, without a program
    let r = p.asm(".byte )", &["-noeval", "-asminfo", "errors|files"]);
    assert_eq!(r.code, 1);
    assert!(p.text("asminfo.txt").contains("Error;1,7,1,7,0;Missing start token"), "{}", p.text("asminfo.txt"));
}

#[test]
fn asminfo_to_stdout_writes_no_file() {
    let p = Project::new();
    let r = p.asm("nop", &["-asminfo", "version", "-asminfotostdout"]);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("### ASMINFO START ###\n[version]\n"), "{}", r.out);
    assert!(r.has_line("### ASMINFO END ###"), "{}", r.out);
    assert!(!p.exists("asminfo.txt"));
}

#[test]
fn config_files() {
    let p = Project::new();
    p.write("a.cfg", "// comment\n-showmem\n-cfgfile b.cfg\n");
    p.write("b.cfg", "-define YES\n");
    let r = p.asm("#if YES\n.print \"yes\"\n#endif\nnop", &["-cfgfile", "a.cfg"]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("Memory Map"), "{}", r.out);
    assert!(r.has_line("yes"), "{}", r.out);
    let r = p.asm("nop", &["-cfgfile", "none.cfg"]);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("Warning. Can't find configfile : none.cfg"), "{}", r.out);
    // a file that names itself doesn't hang
    p.write("c.cfg", "-cfgfile c.cfg\n");
    assert_eq!(p.asm("nop", &["-cfgfile", "c.cfg"]).code, 0);
}

#[test]
fn debug_prints_nothing_extra() {
    let p = Project::new();
    let plain = p.asm("nop\nnop nop", &[]);
    let r = p.asm("nop\nnop nop", &["-debug"]);
    assert_eq!(r.code, plain.code);
    assert_eq!(r.lines().len(), plain.lines().len(), "{}", r.out);
}

#[test]
fn debug_dump_file() {
    let p = Project::new();
    assert_eq!(p.asm("*=$1000 \"Main\"\nstart: nop\n.break \"x\"\n.watch $1000,$1010,\"store\"\nsei", &["-debugdump"]).code, 0);
    let s = p.text("t.dbg");
    for part in [
        "<C64debugger version=\"1.0\">\n   <Sources values=\"INDEX,FILE\">\n      0,r65:autoinclude.asm\n      1,",
        "   </Sources>\n\n   <Segment name=\"Default\" dest=\"\" values=\"START,END,FILE_IDX,LINE1,COL1,LINE2,COL2\">\n      <Block name=\"Main\">\n         $1000,$1000,1,2,8,2,10\n",
        "<Labels values=\"SEGMENT,ADDRESS,NAME,START,END,FILE_IDX,LINE1,COL1,LINE2,COL2\">\n      Default,$1000,start,1,2,1,2,6\n",
        "<Breakpoints values=\"SEGMENT,ADDRESS,ARGUMENT\">\n      Default,$1001,x\n",
        "<Watchpoints values=\"SEGMENT,ADDRESS1,ADDRESS2,ARGUMENT\">\n      Default,$1000,$1010,store\n",
        "   </Watchpoints>\n\n</C64debugger>\n",
    ] {
        assert!(s.contains(part), "{}\n{}", part, s);
    }
    std::fs::remove_file(p.dir.join("t.dbg")).unwrap();
    assert_eq!(p.asm("nop", &["-debugdump", "-nooutput"]).code, 0);
    assert!(!p.exists("t.dbg"));
}

#[test]
fn execute_log_holds_the_program_output() {
    let p = Project::new();
    let exe = env!("CARGO_BIN_EXE_r65");
    assert_eq!(p.asm("nop", &["-execute", exe, "-executelog", "e.txt"]).code, 0);
    // the assembler doesn't wait for the program
    for _ in 0..100 {
        if p.exists("e.txt") && p.text("e.txt").contains("R65") {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("no output in e.txt");
}

#[test]
fn execute_log_without_a_program() {
    let p = Project::new();
    let r = p.asm("nop", &["-execute", "program_that_does_not_exist", "-executelog", "e.txt"]);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("Error while executing external program"), "{}", r.out);
    assert!(p.exists("e.txt"));
    let p = Project::new();
    assert_eq!(p.asm("nop", &["-executelog", "e.txt"]).code, 0);
    assert!(!p.exists("e.txt"));
}

#[test]
fn symbol_file_lists_namespaces() {
    let p = Project::new();
    assert_eq!(p.asm(".filenamespace n\n*=$2000\nf: nop", &["-symbolfile"]).code, 0);
    assert_eq!(p.text("t.sym"), ".namespace n {\r\n  .label f=$2000\r\n}\r\n");
    assert_eq!(p.asm("*=$2000\nm: mm()\n.macro mm() {\nk: nop\n}\n.namespace empty {\n}", &["-symbolfile"]).code, 0);
    let s = p.text("t.sym");
    assert!(s.contains(".namespace empty {\r\n}\r\n"), "{}", s);
    assert!(!s.contains(".label k"), "{}", s);
}

#[test]
fn brackets_need_their_pair() {
    fails(".byte 1)", "Missing start token\nat line 1, column 8");
    fails("}", "Missing start token\nat line 1, column 1");
    fails(".byte (1]", "Missing start token\nat line 1, column 9");
    fails("{\nnop", "Missing end token\nat line 1, column 1");
    fails(".byte (1", "Missing end token\nat line 1, column 7");
    // per file
    let p = Project::new();
    p.write("o.asm", "{\nnop\n");
    let r = p.asm("#import \"o.asm\"\n}", &["-nooutput"]);
    assert_eq!(r.code, 1);
    assert!(r.out.contains("Missing end token\nat line 1, column 1 in o.asm"), "{}", r.out);
}

#[test]
fn only_labels_can_be_reached_from_outside() {
    assert_eq!(bytes("*=$1000\n.namespace s {\nl: nop\n.label k = 5\n}\njmp s.l\n.byte s.k"), [0xea, 0x4c, 0, 0x10, 5]);
    fails(".namespace s { .const C = 15 }\n.print s.C", "Variable 'C' not present on value of type 'Namespace'");
    fails(".namespace s { .var C = 15 }\n.print s.C", "Variable 'C' not present on value of type 'Namespace'");
    fails("l: { .const C = 15 }\n.print l.C", "Variable 'C' not present on value of type 'LabelValue'");
    fails("s: .for (var i=0;i<2;i++) { .const C = 15\nd: nop }\n.print s[0].C", "Variable 'C' not present on value of type 'ScopeValue'");
    fails(".namespace s { .function f() { .return 1 } }\n.print s.f()", "Unknown function : f");
    fails(".namespace s { .macro m() { nop } }\ns.m()", "Syntax error\nat line 2, column 2");
    fails(".var h = 1\n.print h.b", "Variable 'b' not present on value of type 'number'");
    // the tables of .lohifill can
    assert_eq!(bytes("*=$1000\nlda t.lo\nt: .lohifill 1, $1234"), [0xad, 3, 0x10, 0x34, 0x12]);
}

#[test]
fn assignment_needs_eval() {
    fails(".var x = 1\nx = 2", "Syntax error\nat line 2, column 3");
    fails(".var x = 1\nx++", "Syntax error\nat line 2, column 2");
    fails(".var x = 1\nx += 2", "Syntax error\nat line 2, column 3");
    fails(".var l = List()\nl.add(1)", "Syntax error\nat line 2, column 2");
    fails(".var l = List()\nl[0]", "Pseudo command 'l' not defined");
    prints(".var x = 1\n.eval x += 2\n.print x", &["3"]);
}

#[test]
fn operators_on_values_without_them() {
    fails(".print \"x\"/2", "Unknown function : /");
    fails(".byte true+1", "Unknown function : +");
    fails(".print <\"a\"", "Unknown function : <\nat line 1, column 8");
    fails(".print -\"a\"", "Unknown function : -");
    fails(".print !1", "Unknown function : !");
    fails(".print List().foo()", "Unknown function : foo");
    fails(".print \"a\".charAt()", "Invalid number of arguments : charAt");
    // a value that .byte can't write is reported at the directive
    fails(".byte 1,\"a\"", "Can't get a double representation from a value of type String\nat line 1, column 1");
    fails(".byte true", "from a value of type boolean\nat line 1, column 1");
}

#[test]
fn files_that_are_missing() {
    fails("#import \"no.asm\"", "Can't open file: no.asm\nat line 1, column 9");
    fails(".import source \"no.asm\"", "Can't open file: no.asm\nat line 1, column 1");
    fails(".var a = LoadPicture(\"no.png\")", "Can't locate file 'no.png'");
    fails(".segment A [outBin=\"o.bin\", prgFiles=\"no.prg\"]\nnop", "Can't open file 'no.prg'\nat line 1, column 29");
    fails(".segment A [sidFiles=\"no.sid\"]\nnop", "Can't open file 'no.sid'\nat line 1, column 13");
}

#[test]
fn a_second_source_file_is_ignored() {
    let p = Project::new();
    let r = p.asm("nop", &["other.asm"]);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("Warning. Already have an inputfile. Won't use 'other.asm'"), "{}", r.out);
}

#[test]
fn disk_errors_while_writing() {
    let src = |files: String| format!(".disk [filename=\"x.d64\"] {{\n{}\n}}", files);
    let many = (0..153).map(|i| format!("[name=\"F{}\", type=\"prg\"]", i)).collect::<Vec<_>>().join(",\n");
    let p = Project::new();
    let r = p.asm(&src(many), &[]);
    assert_eq!(r.code, 1);
    assert!(r.out.contains("Writing d64 file: x.d64"), "{}", r.out);
    assert!(r.out.contains("Error: Error while generating 'x.d64': Too many visible files. Got 153, max is 152."), "{}", r.out);
    // the file is there, and empty
    assert_eq!(p.read("x.d64").len(), 0);
    fails(".disk [name=\"x\"] {\n}", "Paremter 'filename' is missing.");
}

#[test]
fn an_empty_disk_directory() {
    let p = Project::new();
    assert_eq!(p.asm(".disk [filename=\"x.d64\"] {\n}", &[]).code, 0);
    let img = p.read("x.d64");
    assert_eq!(img[0x16600 + 1], 0);
}

#[test]
fn segmentout_takes_a_second_pass() {
    let p = Project::new();
    let r = p.asm(".segmentdef P [start=$2000]\n.segment P\n.byte 1\n.segment Default\n*=$1000\n.segmentout [segments=\"P\"]", &[]);
    assert_eq!(r.code, 0);
    assert!(r.has_line("flex pass 2"), "{}", r.out);
}

