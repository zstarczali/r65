//! Command line options, help texts, log text and exit codes.
mod common;
use common::Project;

#[test]
fn banner_names_the_program_and_version() {
    let b = Project::new().asm("nop", &["-nooutput"]);
    assert!(b.out.contains("R65 - KickAssembler compatible compiler for C64"), "{}", b.out);
    assert!(b.out.contains(&format!("Version {}", env!("CARGO_PKG_VERSION"))), "{}", b.out);
    assert!(b.out.contains("(C) 2026 Zsolt Tarczali"), "{}", b.out);
}

#[test]
fn help_topics() {
    let p = Project::new();
    let index = p.run(&["--help"]);
    assert_eq!(index.code, 0);
    for topic in ["options", "syntax", "directives", "preprocessor", "functions", "values", "segments", "disk", "encodings", "cpu", "output", "config", "examples", "limits", "all"] {
        assert!(index.out.contains(topic), "{} isn't listed in:\n{}", topic, index.out);
        let r = p.run(&["--help", topic]);
        assert_eq!(r.code, 0, "{}", topic);
        assert!(r.out.len() > 50, "the topic {} is too short:\n{}", topic, r.out);
    }
    assert_eq!(p.run(&["-h"]).code, 0);
    assert_eq!(p.run(&["-help"]).code, 0);
}

#[test]
fn exit_codes() {
    let p = Project::new();
    assert_eq!(p.asm("nop", &[]).code, 0);
    assert_eq!(p.asm("nosuch #1", &[]).code, 1);
    assert_eq!(p.run(&["missing.asm"]).code, 1);
    let r = p.asm("nop", &["-nosuchoption"]);
    assert_eq!(r.code, 1);
    assert!(r.out.contains("Unknown option '-nosuchoption'"), "{}", r.out);
}

#[test]
fn error_shows_the_place() {
    let p = Project::new();
    let r = p.asm(".byte 1\nlda nosuch\n", &["-nooutput"]);
    assert_eq!(r.code, 1);
    assert!(r.out.contains("lda nosuch\n    ^") || r.out.contains("lda nosuch\r\n    ^"), "{}", r.out);
    assert!(r.out.contains("Error: Unknown symbol 'nosuch'"), "{}", r.out);
    assert!(r.out.contains("at line 2, column 5 in t.asm"), "{}", r.out);
}

#[test]
fn output_names_and_folders() {
    let p = Project::new();
    assert_eq!(p.asm("nop", &[]).code, 0);
    assert_eq!(p.read("t.prg"), [0, 0x20, 0xea]);

    assert_eq!(p.asm("nop", &["-o", "other.prg"]).code, 0);
    assert!(p.exists("other.prg"));

    std::fs::create_dir(p.dir.join("out")).unwrap();
    assert_eq!(p.asm("nop", &["-odir", "out"]).code, 0);
    assert!(p.exists("out/t.prg"));

    assert_eq!(p.asm("nop", &["-binfile"]).code, 0);
    assert_eq!(p.read("t.bin"), [0xea]);

    let q = Project::new();
    assert_eq!(q.asm("nop", &["-nooutput"]).code, 0);
    assert!(!q.exists("t.prg"));
}

#[test]
fn source_in_another_folder_writes_next_to_it() {
    let p = Project::new();
    p.write("sub/a.asm", "nop");
    assert_eq!(p.run(&["sub/a.asm"]).code, 0);
    assert!(p.exists("sub/a.prg"));
}

#[test]
fn define_option() {
    let p = Project::new();
    let src = "#if TEST\n.byte 1\n#else\n.byte 2\n#endif";
    p.asm(src, &[]);
    assert_eq!(p.read("t.prg")[2], 2);
    p.asm(src, &["-define", "TEST"]);
    assert_eq!(p.read("t.prg")[2], 1);
}

#[test]
fn libdir_and_replacefile() {
    let p = Project::new();
    p.write("lib/l.asm", ".byte 1\n");
    p.write("r.asm", ".byte 2\n");
    let src = "#import \"l.asm\"";
    assert_eq!(p.asm(src, &[]).code, 1);
    assert_eq!(p.asm(src, &["-libdir", "lib"]).code, 0);
    assert_eq!(p.read("t.prg")[2], 1);

    p.write("o.asm", ".byte 3\n");
    assert_eq!(p.asm("#import \"o.asm\"", &["-replacefile", "o.asm", "r.asm"]).code, 0);
    assert_eq!(p.read("t.prg")[2], 2);
}

#[test]
fn command_line_values() {
    let p = Project::new();
    let r = p.asm(".print cmdLineVars.get(\"x\")\n.print cmdLineVars.get(\"n\").asNumber()*2", &[":x=abc", ":n=21", "-nooutput"]);
    assert!(r.has_line("abc") && r.has_line("42"), "{}", r.out);
}

#[test]
fn cpu_options() {
    let p = Project::new();
    assert_eq!(p.asm("lax $10", &["-excludeillegal"]).code, 1);
    assert_eq!(p.asm("lax $10", &["-cpu", "_6502NoIllegals"]).code, 1);
    assert_eq!(p.asm("sac #1", &[]).code, 1);
    assert_eq!(p.asm("sac #1", &["-dtv"]).code, 0);
    assert_eq!(p.asm("phx", &["-cpu", "_65c02"]).code, 0);
}

#[test]
fn log_and_time_options() {
    let p = Project::new();
    let r = p.asm(".print \"hello\"", &["-log", "log.txt", "-time", "-nooutput"]);
    assert_eq!(r.code, 0);
    assert!(p.text("log.txt").contains("hello"));
}

#[test]
fn noeval_only_parses() {
    let p = Project::new();
    assert_eq!(p.asm("lda nosuch", &["-noeval"]).code, 0);
    assert_eq!(p.asm("lda (", &["-noeval"]).code, 1);
}

#[test]
fn warnings_can_be_limited() {
    let p = Project::new();
    let src = "lda a\nlda a\nlda a\n.label a = $10";
    let r = p.asm(src, &["-maxdiag", "1", "-nooutput"]);
    assert_eq!(r.code, 0);
    let few = r.out.matches("Warning").count();
    let all = p.asm(src, &["-nooutput"]).out.matches("Warning").count();
    assert!(few < all, "{} {}", few, all);
}

#[test]
fn execute_runs_nothing_after_a_failure() {
    let p = Project::new();
    let r = p.asm("nosuch #1", &["-execute", "program_that_does_not_exist"]);
    assert_eq!(r.code, 1);
    assert!(!r.out.contains("executing external program"), "{}", r.out);
    let r = p.asm("nop", &["-execute", "program_that_does_not_exist"]);
    assert!(r.out.contains("Error while executing external program"), "{}", r.out);
}

/// `-maxaddr` is accepted but not enforced, see MECHANICS.md.
#[test]
fn maxaddr_is_not_enforced() {
    let p = Project::new();
    assert_eq!(p.asm("*=$3000\nnop", &["-maxaddr", "8191"]).code, 0);
    assert_eq!(p.read("t.prg")[..2], [0, 0x30]);
}
