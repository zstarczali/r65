//! r65::build, the library entry an editor uses: assembles in memory, writes nothing.

use std::path::Path;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("r65-library-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn builds_a_program_in_memory_with_labels_spans_and_debugger_files() {
    let dir = temp_dir("ok");
    let src = "BasicUpstart2(start)\n* = $0810 \"Main\"\nstart:\n    lda #1\n    sta $d020\nloop: jmp loop\n";
    let path = dir.join("main.asm");
    let b = r65::build(src, &path, &r65::BuildOptions::default());
    assert!(b.ok, "{:?}", b.errors);
    assert_eq!(&b.prg[..2], &[0x01, 0x08]);
    // the code at $0810 behind the BASIC line
    let code = &b.prg[2 + 0x10 - 1..];
    assert_eq!(&code[..8], &[0xa9, 0x01, 0x8d, 0x20, 0xd0, 0x4c, 0x15, 0x08]);
    let ex = b.extra.expect("extra");
    assert!(ex.labels.iter().any(|l| l.2 == "start" && l.1 == 0x0810));
    assert!(ex.labels.iter().any(|l| l.2 == "loop" && l.1 == 0x0815));
    let main = b.files.iter().position(|f| f.ends_with("main.asm")).unwrap();
    // lda #1 is line 4 of main.asm, at $0810, two bytes
    assert!(ex.spans.iter().any(|s| (s.0, s.1, s.2, s.3, s.4, s.5.as_str()) == (0x0810, 2, main, 4, 5, "lda #1")), "{:?}", ex.spans);
    assert!(ex.vice_symbols.contains("al C:810 .start"));
    assert!(ex.debug_dump.starts_with("<C64debugger"));
    // nothing written next to the source
    assert!(!dir.join("main.prg").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reports_every_error_with_its_file_and_line() {
    let dir = temp_dir("err");
    let path = dir.join("main.asm");
    let b = r65::build("* = $1000\n    lda #1\n    jmp nowhere\n", &path, &r65::BuildOptions::default());
    assert!(!b.ok);
    assert!(b.prg.is_empty());
    let e = &b.errors[0];
    assert_eq!(Path::new(&e.file).file_name().unwrap(), "main.asm");
    assert_eq!(e.line, 3);
    assert!(e.message.contains("nowhere"), "{}", e.message);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn takes_defines_and_command_line_variables() {
    let dir = temp_dir("opts");
    let path = dir.join("main.asm");
    let src = "* = $1000\n#if FAST\n    .byte cmdLineVars.get(\"n\").asNumber()\n#else\n    .byte 0\n#endif\n";
    let o = r65::BuildOptions { defines: vec!["FAST".into()], cmd_vars: vec![("n".into(), "7".into())], ..Default::default() };
    let b = r65::build(src, &path, &o);
    assert!(b.ok, "{:?}", b.errors);
    assert_eq!(b.prg, vec![0x00, 0x10, 0x07]);
    let _ = std::fs::remove_dir_all(&dir);
}
