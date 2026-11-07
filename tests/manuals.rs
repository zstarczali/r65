//! The manuals are part of the tests: each assembly example has to assemble, and the opcode tables have to match.
mod common;
use common::Project;

const EN: &str = include_str!("../docs/R65-Manual.md");
const HU: &str = include_str!("../docs/R65-Manual.hu.md");

/// The ```asm blocks of a manual, without the indentation of a list item.
fn examples(md: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur: Option<(usize, String)> = None;
    for line in md.lines() {
        let t = line.trim_start();
        match &mut cur {
            None if t == "```asm" => cur = Some((line.len() - t.len(), String::new())),
            None => {}
            Some(_) if t == "```" => out.push(cur.take().unwrap().1),
            Some((indent, text)) => {
                text.push_str(line.get(*indent..).unwrap_or(""));
                text.push('\n');
            }
        }
    }
    out
}

fn assemble_all(md: &str) {
    let list = examples(md);
    assert!(list.len() > 50, "only {} examples found", list.len());
    let mut failed = vec![];
    for src in &list {
        let p = Project::new();
        p.fixtures();
        let r = p.asm(src, &[]);
        if r.code != 0 {
            failed.push(format!("{}\n{}", src, r.out));
        }
    }
    assert!(failed.is_empty(), "{} examples failed:\n{}", failed.len(), failed.join("\n-----\n"));
}

#[test]
fn english_examples_assemble() {
    assemble_all(EN);
}

#[test]
fn hungarian_examples_assemble() {
    assemble_all(HU);
}

/// The two versions show the same programs. Only the `//` comments are translated.
#[test]
fn both_manuals_have_the_same_examples() {
    let strip = |s: &String| -> Vec<String> {
        s.lines().map(|l| l.split("//").next().unwrap().trim_end().to_string()).collect()
    };
    let en: Vec<_> = examples(EN).iter().map(strip).collect();
    let hu: Vec<_> = examples(HU).iter().map(strip).collect();
    assert_eq!(en.len(), hu.len());
    for (e, h) in en.iter().zip(&hu) {
        assert_eq!(e, h);
    }
}

/// Every link to a heading inside a manual has to lead somewhere.
#[test]
fn links_to_headings_work() {
    for md in [EN, HU] {
        let mut anchors = vec![];
        let mut in_code = false;
        for line in md.lines() {
            if line.trim_start().starts_with("```") {
                in_code = !in_code;
            } else if !in_code && line.starts_with('#') {
                let title = line.trim_start_matches('#').trim().to_lowercase();
                let slug: String = title.chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-').map(|c| if c == ' ' { '-' } else { c }).collect();
                anchors.push(slug);
            }
        }
        for part in md.split("](#").skip(1) {
            let target = part.split(')').next().unwrap();
            assert!(anchors.iter().any(|a| a == target), "broken link #{}", target);
        }
    }
}

/// How each column of the opcode tables is written in a source.
fn operand(column: &str) -> &'static str {
    match column {
        "impl" => "",
        "imm" => " #$11",
        "zp" => " $11",
        "zp,x" => " $11,x",
        "zp,y" => " $11,y",
        "(zp,x)" => " ($11,x)",
        "(zp),y" => " ($11),y",
        "abs" => " $1234",
        "abs,x" => " $1234,x",
        "abs,y" => " $1234,y",
        "(ind)" => " ($1234)",
        "rel" => " *",
        "(zp)" => " ($11)",
        "zp,rel" => " $11,*",
        "(abs,x)" => " ($1234,x)",
        other => panic!("unknown column {}", other),
    }
}

fn size(column: &str) -> usize {
    match column {
        "impl" => 1,
        "abs" | "abs,x" | "abs,y" | "(ind)" | "(abs,x)" | "zp,rel" => 3,
        _ => 2,
    }
}

/// Reads the table below a heading and checks each cell against the assembler.
fn check_table(heading: &str, cpu: &str) {
    let body = EN.split(heading).nth(1).unwrap_or_else(|| panic!("no heading {}", heading));
    let rows: Vec<Vec<&str>> = body
        .lines()
        .skip_while(|l| !l.starts_with('|'))
        .take_while(|l| l.starts_with('|'))
        .map(|l| l.trim_matches('|').split('|').map(str::trim).collect())
        .collect();
    let columns = &rows[0][1..];
    let mut src = format!(".cpu {}\n*=$1000\n", cpu);
    let mut expected = vec![];
    for row in &rows[2..] {
        for name in row[0].split(',') {
            let name = name.trim().trim_matches('`');
            for (col, cell) in columns.iter().zip(&row[1..]) {
                if !cell.is_empty() {
                    src.push_str(&format!("{}{}\n", name, operand(col)));
                    expected.push((name.to_string(), *col, u8::from_str_radix(cell, 16).unwrap()));
                }
            }
        }
    }
    assert!(expected.len() > 20);
    let p = Project::new();
    let r = p.asm(&src, &[]);
    assert_eq!(r.code, 0, "{}", r.out);
    let prg = p.read("t.prg");
    let mut at = 2;
    for (name, col, op) in expected {
        assert_eq!(prg[at], op, "{} {} in {}", name, col, cpu);
        at += size(col);
    }
    assert_eq!(at, prg.len(), "unexpected instruction sizes in {}", cpu);
}

#[test]
fn opcode_table_standard() {
    check_table("### Standard 6502", "_6502NoIllegals");
}

#[test]
fn opcode_table_illegal() {
    check_table("### Illegal opcodes", "_6502");
}

#[test]
fn opcode_table_65c02() {
    check_table("### 65C02", "_65c02");
}
