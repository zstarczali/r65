//! Memory blocks, segments and the files that R65 writes.
mod common;
use common::{fails, Project};

fn ok(p: &Project, src: &str, args: &[&str]) -> common::Run {
    p.fixtures();
    let r = p.asm(src, args);
    assert_eq!(r.code, 0, "{}\n{}", src, r.out);
    r
}

// ---------------------------------------------------------------- memory blocks

#[test]
fn gaps_are_filled() {
    let p = Project::new();
    ok(&p, "*=$1000\n.byte 1\n*=$1003\n.byte 2", &[]);
    assert_eq!(p.read("t.prg"), [0, 0x10, 1, 0, 0, 2]);
    ok(&p, "*=$1000\n.byte 1\n*=$1003\n.byte 2", &["-fillbyte", "$ea"]);
    assert_eq!(p.read("t.prg"), [0, 0x10, 1, 0xea, 0xea, 2]);
    ok(&p, "*=$1000\n.byte 1\n*=$1003\n.byte 2", &["-fillbyte", "0x11"]);
    assert_eq!(p.read("t.prg")[3], 0x11);
}

#[test]
fn virtual_blocks_write_nothing() {
    let p = Project::new();
    let r = ok(&p, "*=$c000 \"V\" virtual\nt: .fill 4,0\n*=$1000 \"P\"\nlda t", &["-showmem"]);
    assert_eq!(p.read("t.prg"), [0, 0x10, 0xad, 0, 0xc0]);
    assert!(r.has_line("$1000-$1002 P"), "{}", r.out);
    assert!(r.has_line("*$c000-$c003 V"), "{}", r.out);
}

#[test]
fn overlap_is_an_error_unless_allowed() {
    let src = "*=$1000 \"A\"\n.byte 1,2\n*=$1001 \"B\"\n.byte 3";
    fails(src, "In segment 'Default' the memoryblock 'A' ($1000-$1001) overlaps 'B' ($1001-$1001)");
    let p = Project::new();
    ok(&p, src, &["-aom"]);
    assert_eq!(p.read("t.prg"), [0, 0x10, 1, 3]);
}

#[test]
fn memory_map_text() {
    let p = Project::new();
    let r = ok(&p, "BasicUpstart2(s)\n* = $0810 \"Main\"\ns: rts\n.memblock \"Next\"\nnop", &["-showmem"]);
    let text = r.lines();
    let at = text.iter().position(|l| *l == "Memory Map").unwrap();
    assert_eq!(text[at + 1], "----------");
    assert_eq!(text[at + 2], "Default-segment:");
    assert_eq!(text[at + 3], "$0801-$080c Basic");
    assert_eq!(text[at + 4], "$080e-$080d Basic End");
    assert_eq!(text[at + 5], "$0810-$0810 Main");
    assert_eq!(text[at + 6], "$0811-$0811 Next");
}

#[test]
fn file_per_block() {
    let p = Project::new();
    ok(&p, "*=$1000 \"One\"\n.byte 1\n*=$2000 \"Two\"\n.byte 2", &["-mbfiles"]);
    assert_eq!(p.read("t_One.prg"), [0, 0x10, 1]);
    assert_eq!(p.read("t_Two.prg"), [0, 0x20, 2]);
}

// ---------------------------------------------------------------- segments

#[test]
fn segment_blocks_go_where_they_are_sent() {
    let p = Project::new();
    ok(&p, ".segment A [outPrg=\"a.prg\"]\n*=$1000\n.byte 1\n.segment B [outBin=\"b.bin\"]\n*=$2000\n.byte 2\n.segment C []\n.byte 3", &[]);
    assert_eq!(p.read("a.prg"), [0, 0x10, 1]);
    assert_eq!(p.read("b.bin"), [2]);
    assert!(!p.exists("t.prg"), "only the Default segment makes the main file");
}

#[test]
fn start_after_and_align() {
    let p = Project::new();
    let r = ok(
        &p,
        ".segmentdef C [start=$8000]\n.segmentdef D [startAfter=\"C\"]\n.segmentdef V [startAfter=\"D\", align=$100, virtual]\n.file [name=\"o.prg\", segments=\"C,D\"]\n.segment C\n.byte 1,2\n.segment D\n.byte 3\n.segment V \"Tab\"\n.fill 2,0",
        &["-showmem"],
    );
    assert_eq!(p.read("o.prg"), [0, 0x80, 1, 2, 3]);
    assert!(r.has_line("*$8100-$8101 Tab"), "{}", r.out);
}

#[test]
fn fill_covers_min_to_max() {
    let p = Project::new();
    ok(&p, ".segment D [min=$1000, max=$1007, fill, outBin=\"d.bin\"]\n*=$1002\n.byte 1,2,3", &[]);
    assert_eq!(p.read("d.bin"), [0, 0, 1, 2, 3, 0, 0, 0]);
    ok(&p, ".segment D [min=$1000, max=$1001, fill, fillByte=$ea, outBin=\"d.bin\"]\n*=$1000\n.byte 1", &[]);
    assert_eq!(p.read("d.bin"), [1, 0xea]);
}

#[test]
fn limits_are_checked_when_written() {
    fails(".segment D [start=$1000, min=$1000, max=$1001, outBin=\"d.bin\"]\n.byte 1,2,3", "is out of segment bounds ($1000-$1001).");
    fails(".segment D [start=$0f00, min=$1000, max=$1001, outBin=\"d.bin\"]\n.byte 1", "is out of segment bounds ($1000-$1001).");
}

#[test]
fn segment_is_defined_once() {
    fails(".segmentdef M [start=$1000]\n.segment M [start=$1000]", "The segment 'M' is already defined");
}

#[test]
fn switching_back_continues_the_segment() {
    let p = Project::new();
    ok(&p, ".segment A [start=$1000, outBin=\"a.bin\"]\n.byte 1\n.segment B [start=$2000]\n.byte 9\n.segment A\n.byte 2", &[]);
    assert_eq!(p.read("a.bin"), [1, 2]);
}

#[test]
fn including_other_segments_and_files() {
    let p = Project::new();
    ok(&p, ".segmentdef A [start=$1000]\n.segment A\n.byte 1\n.segmentdef B [start=$1001, segments=\"A\", outPrg=\"b.prg\"]\n.segment B\n.byte 2", &[]);
    assert_eq!(p.read("b.prg"), [0, 0x10, 1, 2]);
    ok(&p, ".segment C [prgFiles=\"prog.prg\", outPrg=\"c.prg\"]", &[]);
    assert_eq!(p.read("c.prg"), [0, 0x30, 9, 8, 7]);
    ok(&p, ".segment S [sidFiles=\"tune.sid\", outPrg=\"s.prg\"]", &[]);
    assert_eq!(p.read("s.prg"), [0, 0x10, 0x60, 0x61, 0x62, 0x63]);
}

#[test]
fn patching_with_overlap() {
    let p = Project::new();
    ok(&p, ".file [name=\"o.prg\", segments=\"Base,Patch\", allowOverlap]\n.segmentdef Base [prgFiles=\"prog.prg\"]\n.segmentdef Patch []\n.segment Patch\n*=$3001\nlda #$ff", &[]);
    assert_eq!(p.read("o.prg"), [0, 0x30, 9, 0xa9, 0xff]);
}

#[test]
fn basic_upstart_modifier() {
    for arg in ["_start", "marg1"] {
        let p = Project::new();
        ok(&p, &format!(".segmentdef C [start=$0810]\n.file [name=\"o.prg\", segments=\"C\", modify=\"BasicUpstart\", {}=$0810]\n.segment C\nrts", arg), &[]);
        let o = p.read("o.prg");
        assert_eq!(o[..2], [0x01, 0x08]);
        assert_eq!(o[6..11], [0x9e, b'2', b'0', b'6', b'4']);
        assert_eq!(*o.last().unwrap(), 0x60);
    }
}

#[test]
fn segmentout_places_bytes_here() {
    let p = Project::new();
    ok(&p, "*=$1000\nc: .segmentout [segments=\"Z\"]\n.byte *-c\n.segment Z [start=$10]\nz: jmp z", &[]);
    assert_eq!(p.read("t.prg"), [0, 0x10, 0x4c, 0x10, 0, 3]);
    ok(&p, "*=$8000\n.segmentout [sidFiles=\"tune.sid\"]", &[]);
    assert_eq!(p.read("t.prg"), [0, 0x80, 0x60, 0x61, 0x62, 0x63]);
}

// ---------------------------------------------------------------- .file

#[test]
fn file_directive() {
    let p = Project::new();
    ok(
        &p,
        ".segmentdef D [start=$2000]\n.file [name=\"%o_d.prg\", segments=\"D\"]\n.file [name=\"d.bin\", type=\"bin\", segments=\"D\"]\n.file [name=\"m.prg\", mbfiles, segments=\"D\"]\n.segment D \"Tab\"\n.byte 1\n*=$3000 \"Two\"\n.byte 2",
        &[],
    );
    assert_eq!(p.read("t_d.prg")[..3], [0, 0x20, 1]);
    assert_eq!(p.read("d.bin")[0], 1);
    assert_eq!(p.read("m_Tab.prg"), [0, 0x20, 1]);
    assert_eq!(p.read("m_Two.prg"), [0, 0x30, 2]);
}

// ---------------------------------------------------------------- .disk

const DIR: usize = 0x16600; // track 18 sector 1
const BAM: usize = 0x16500; // track 18 sector 0

fn entry(d64: &[u8], n: usize) -> (u8, String) {
    let e = &d64[DIR + n * 32..DIR + (n + 1) * 32];
    let name = e[5..21].iter().take_while(|b| **b != 0xa0).map(|b| *b as char).collect();
    (e[2], name)
}

#[test]
fn disk_image() {
    let p = Project::new();
    ok(
        &p,
        ".segmentdef C [start=$1000]\n.disk [filename=\"x.d64\", name=\"MY DISK\", id=\"2A\"] {\n[name=\"AA\", type=\"prg\", segments=\"C\"],\n[name=\"BB\", type=\"seq\", segments=\"C\"],\n[name=\"CC\", type=\"prg<\", segments=\"C\"],\n[name=\"HH\", type=\"prg\", hide, segments=\"C\"],\n[name=\"DD\", type=\"usr\", prgFiles=\"prog.prg\"],\n}\n.segment C\n.byte 1,2,3",
        &[],
    );
    let d = p.read("x.d64");
    assert_eq!(d.len(), 174848);
    assert_eq!(&d[BAM + 0x90..BAM + 0x97], b"MY DISK");
    assert_eq!(d[BAM + 2], 0x41);
    assert_eq!(entry(&d, 0), (0x82, "AA".to_string()));
    assert_eq!(entry(&d, 1), (0x81, "BB".to_string()));
    assert_eq!(entry(&d, 2), (0xc2, "CC".to_string()));
    assert_eq!(entry(&d, 3), (0x83, "DD".to_string()), "a hidden file isn't listed");
}

#[test]
fn disk_file_contents() {
    let p = Project::new();
    ok(&p, ".disk [filename=\"x.d64\"] {\n[name=\"P\", type=\"prg\", segments=\"C\"]\n}\n.segment C [start=$1000]\n.byte 1,2,3", &[]);
    assert_eq!(p.read("x.d64")[2..7], [0, 0x10, 1, 2, 3]);
    ok(&p, ".disk [filename=\"x.d64\"] {\n[name=\"P\", type=\"prg\", noStartAddr, segments=\"C\"]\n}\n.segment C [start=$1000]\n.byte 1,2,3", &[]);
    assert_eq!(p.read("x.d64")[2..5], [1, 2, 3]);
    ok(&p, ".disk [filename=\"x.d64\"] {\n[name=\"S\", type=\"seq\", segments=\"C\"]\n}\n.segment C [start=$1000]\n.byte 1,2,3", &[]);
    assert_eq!(p.read("x.d64")[2..5], [1, 2, 3], "only a prg file keeps the address by default");
}

#[test]
fn disk_defaults_and_errors() {
    let p = Project::new();
    ok(&p, ".disk [filename=\"x.d64\"]\n{\n}", &[]);
    let d = p.read("x.d64");
    assert_eq!(&d[BAM + 0x90..BAM + 0x97], b"UNNAMED");
    let p = Project::new();
    assert_eq!(p.asm(".disk [name=\"X\"] {\n}", &[]).code, 1);
    assert_eq!(p.asm(".disk [filename=\"x.d64\"] {\n[name=\"A\", type=\"bad\"]\n}", &[]).code, 1);
}

// ---------------------------------------------------------------- other files

#[test]
fn symbol_file() {
    let p = Project::new();
    ok(&p, "*=$2000\nroot: nop\nsc: {\nin: nop\n}\n.namespace ns {\nx: nop\n}", &["-symbolfile"]);
    let s = p.text("t.sym");
    assert!(s.contains(".label root=$2000\r\n"), "{}", s);
    assert!(s.contains(".label sc=$2001 {\r\n  .label in=$2001\r\n}\r\n"), "{}", s);
    assert!(s.contains(".namespace ns {\r\n  .label x=$2002\r\n}\r\n"), "{}", s);
}

#[test]
fn symbol_file_can_be_imported() {
    let p = Project::new();
    p.write("a.asm", "*=$2000\nentry: nop\n");
    assert_eq!(p.run(&["a.asm", "-symbolfile"]).code, 0);
    let r = p.asm("#import \"a.sym\"\njmp entry", &[]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(p.read("t.prg")[2..], [0x4c, 0, 0x20]);
}

#[test]
fn symbol_file_folder() {
    let p = Project::new();
    std::fs::create_dir(p.dir.join("syms")).unwrap();
    ok(&p, "l: nop", &["-symbolfile", "-symbolfiledir", "syms"]);
    assert!(p.exists("syms/t.sym"));
}

#[test]
fn vice_file() {
    let p = Project::new();
    ok(&p, "*=$1000\nlbl: nop\n.break\nnop\n.break \"if A == 1\"\n.watch $d020\n.watch $1000,$1010,\"store\"\n.watch $d021,,\"hex8\"", &["-vicesymbols"]);
    let lines: Vec<String> = p.text("t.vs").lines().map(String::from).collect();
    assert_eq!(lines, ["al C:1000 .lbl", "break 1001 ", "break 1002 if A == 1", "watch d020 ", "watch store 1000 1010 ", "watch hex8 d021 "]);
}

#[test]
fn byte_dump() {
    let p = Project::new();
    ok(&p, "*=$1000 \"Foo\"\nlda #1\n.byte 2", &["-bytedump"]);
    let t = p.text("ByteDump.txt");
    assert!(t.contains(" Segment: Default "), "{}", t);
    assert!(t.contains("[Foo]"), "{}", t);
    assert!(t.contains("1000: a9 01     -"), "{}", t);
    assert!(t.contains("lda #1"), "{}", t);
    assert!(t.contains("1002: 02        -"), "{}", t);
    ok(&p, "nop", &["-bytedumpfile", "d.txt"]);
    assert!(p.exists("d.txt"));
}

#[test]
fn create_file_needs_permission() {
    fails(".var f = createFile(\"o.txt\")", "File output is blocked. Use the -afo option to allow it");
    let p = Project::new();
    ok(&p, ".var f = createFile(\"o.txt\")\n.eval f.writeln(\"Hello\")\n.eval f.writeln()\n.eval f.write(\"x\")", &["-afo"]);
    assert_eq!(p.text("o.txt"), "Hello\n\nx");
}
