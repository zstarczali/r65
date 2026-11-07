//! The -asminfo file: information about the assembler and the source, for editors.
//!
//! The sections, their order and the function and constant names follow the original. Its
//! descriptions of the directives aren't copied, so those lines have our own words.

use crate::driver::{Outcome, VERSION};
use crate::lexer::{Tok, Token};
use crate::preproc::Sources;
use std::collections::HashSet;

/// (type, start line, start position, end line, end position, file index)
pub type SyntaxEntry = (&'static str, u32, u32, u32, u32, usize);

const PREDEFINED: [&str; 4] = ["version", "libraries", "directives", "preprocessorDirectives"];
const SOURCE_SPECIFIC: [&str; 3] = ["files", "syntax", "errors"];
const GROUPS: [&str; 3] = ["all", "allPredefined", "allSourceSpecific"];

/// Selections that don't exist: the original warns about them and goes on.
pub fn unknown_selections(sel: &str) -> Vec<&str> {
    sel.split('|').filter(|s| !PREDEFINED.contains(s) && !SOURCE_SPECIFIC.contains(s) && !GROUPS.contains(s)).collect()
}

fn chosen(sel: &str, section: &str) -> bool {
    sel.split('|').any(|s| {
        s == section || s == "all" || (s == "allPredefined" && PREDEFINED.contains(&section)) || (s == "allSourceSpecific" && SOURCE_SPECIFIC.contains(&section))
    })
}

const LIBRARIES: &str = include_str!("asminfo_libraries.txt");

const DIRECTIVES: [(&str, &str, &str); 55] = [
    ("*", "* = $1000", "Sets the address of the following code."),
    (".align", ".align $100", "Pads with zeros up to the next multiple of the value."),
    (".assert", ".assert \"2+2\", 2+2, 4", "Compares two values or two code blocks."),
    (".asserterror", ".asserterror \"Test\", 20/false", "Checks that an expression or a code block gives an error."),
    (".break", ".break", "Adds a VICE breakpoint at the current address."),
    (".by", ".by $01,$02,$03", "Short for .byte."),
    (".byte", ".byte $01,$02,$03", "Writes bytes."),
    (".const", ".const BORDER = $d020", "Defines a constant."),
    (".cpu", ".cpu _65c02", "Sets the instruction set."),
    (".define", ".define list { .var list = List() }", "Runs directives in function mode and locks the defined symbols."),
    (".disk", ".disk [filename=\"a.d64\"] { }", "Writes a disk image."),
    (".dw", ".dw $12345678", "Short for .dword."),
    (".dword", ".dword $12345678", "Writes 32 bit values."),
    (".encoding", ".encoding \"petscii_upper\"", "Sets the text encoding."),
    (".enum", ".enum { ON, OFF }", "Defines a series of constants."),
    (".error", ".error \"Not allowed\"", "Stops with an error."),
    (".errorif", ".errorif x>2, \"Too big\"", "Stops with an error if the condition is true."),
    (".eval", ".eval x = 2", "Evaluates an expression."),
    (".file", ".file [name=\"a.prg\", segments=\"Code\"]", "Writes segments to a file."),
    (".filemodify", ".filemodify Name()", "Modifies the output file with a plugin; not supported."),
    (".filenamespace", ".filenamespace n", "Puts the rest of the file in a namespace."),
    (".fill", ".fill 10, i", "Writes bytes from an expression."),
    (".fillword", ".fillword 10, i*256", "Writes 16 bit values from an expression."),
    (".for", ".for (var i=0; i<3; i++) nop", "A for loop."),
    (".function", ".function f(x) { .return x*2 }", "Defines a function."),
    (".if", ".if (x>2) nop", "Runs code if the condition is true."),
    (".import binary", ".import binary \"a.bin\"", "Loads a file as bytes."),
    (".import c64", ".import c64 \"a.prg\"", "Loads a file as bytes, without its start address."),
    (".import source", ".import source \"a.asm\"", "Loads a source file."),
    (".import text", ".import text \"a.txt\"", "Loads a file as text."),
    (".importonce", ".importonce", "Makes sure the file is only loaded once."),
    (".label", ".label screen = $0400", "Defines a label."),
    (".lohifill", ".lohifill 10, i*40", "Writes a table of low bytes and a table of high bytes."),
    (".macro", ".macro m() { nop }", "Defines a macro."),
    (".memblock", ".memblock \"Code\"", "Starts a named memory block."),
    (".modify", ".modify Name() { }", "Modifies code with a plugin; not supported."),
    (".namespace", ".namespace n { }", "Defines a namespace."),
    (".pc", ".pc = $1000", "Sets the address of the following code."),
    (".plugin", ".plugin \"a.b.C\"", "Loads a plugin; not supported."),
    (".print", ".print \"Hello\"", "Prints in the output pass."),
    (".printnow", ".printnow \"Hello\"", "Prints in every pass."),
    (".pseudocommand", ".pseudocommand mov a:b { }", "Defines a pseudocommand."),
    (".pseudopc", ".pseudopc $2000 { }", "Assembles for another address."),
    (".return", ".return 1", "Gives the result of a function."),
    (".segment", ".segment Code", "Switches to a segment."),
    (".segmentdef", ".segmentdef Code [start=$1000]", "Defines a segment."),
    (".segmentout", ".segmentout [segments=\"Code\"]", "Writes the bytes of segments here."),
    (".struct", ".struct Point { x, y }", "Defines a struct."),
    (".te", ".te \"hello\"", "Short for .text."),
    (".text", ".text \"hello\"", "Writes text as bytes."),
    (".var", ".var x = 2", "Defines a variable."),
    (".while", ".while (x<3) .eval x++", "A while loop."),
    (".wo", ".wo $1000", "Short for .word."),
    (".word", ".word $1000", "Writes 16 bit values."),
    (".zp", ".zp { a: .byte 0 }", "Marks labels as zero page labels."),
];

const PP_DIRECTIVES: [(&str, &str, &str); 9] = [
    ("#define", "#define DEBUG", "Defines a preprocessor symbol."),
    ("#elif", "#elif TEST", "The else if part of an #if."),
    ("#else", "#else", "The else part of an #if."),
    ("#endif", "#endif", "Ends an #if."),
    ("#if", "#if DEBUG", "Uses the following lines if the condition is true."),
    ("#import", "#import \"file.asm\"", "Loads a source file."),
    ("#importif", "#importif DEBUG \"file.asm\"", "Loads a source file if the condition is true."),
    ("#importonce", "#importonce", "Makes sure the file is only loaded once."),
    ("#undef", "#undef DEBUG", "Removes a preprocessor symbol."),
];

/// Comments and preprocessor directives, which never reach the tokens.
fn comments(file: usize, sources: &Sources, out: &mut Vec<SyntaxEntry>) {
    let Some(src) = sources.files.get(file) else { return };
    let mut open: Option<(u32, u32)> = None;
    for (ln, text) in src.lines.iter().enumerate() {
        let line = ln as u32 + 1;
        let ch: Vec<char> = text.chars().collect();
        let mut i = 0;
        if open.is_none() {
            let start = ch.iter().take_while(|c| c.is_whitespace()).count();
            if ch.get(start) == Some(&'#') {
                let end = start + 1 + ch[start + 1..].iter().take_while(|c| c.is_alphanumeric()).count();
                out.push(("ppDirective", line, start as u32 + 1, line, end as u32, file));
                i = end;
            }
        }
        let mut in_str = false;
        while i < ch.len() {
            if let Some((l0, c0)) = open {
                if ch[i] == '*' && ch.get(i + 1) == Some(&'/') {
                    out.push(("comment", l0, c0, line, i as u32 + 2, file));
                    open = None;
                    i += 2;
                    continue;
                }
            } else if in_str {
                in_str = ch[i] != '"';
            } else if ch[i] == '"' {
                in_str = true;
            } else if ch[i] == '/' && ch.get(i + 1) == Some(&'/') {
                out.push(("comment", line, i as u32 + 1, line, ch.len() as u32 + 1, file));
                break;
            } else if ch[i] == '/' && ch.get(i + 1) == Some(&'*') {
                open = Some((line, i as u32 + 1));
                i += 2;
                continue;
            }
            i += 1;
        }
    }
}

const NAME_LISTS: [&str; 5] = ["macro", "function", "pseudocommand", "struct", "enum"];
const ONE_NAME: [&str; 8] = ["var", "const", "label", "namespace", "filenamespace", "segment", "segmentdef", "define"];
const OPERATORS: [&str; 21] = ["+", "-", "*", "/", "<", ">", "<=", ">=", "==", "!=", "&&", "||", "&", "|", "^", "!", "~", "<<", ">>", "++", "--"];

fn is_op(t: Option<&Token>, o: &str) -> bool {
    matches!(t, Some(Token { tok: Tok::Op(x), .. }) if *x == o)
}

/// The syntax entries of the source files, without those of the built-in macros.
/// The types and ranges follow the original, but it lists them in another order.
pub fn syntax(tokens: &[Token], sources: &Sources, mnemonics: &HashSet<String>) -> Vec<SyntaxEntry> {
    let mut out = vec![];
    for f in 1..sources.files.len() {
        comments(f, sources, &mut out);
    }
    let mut at_start = true;
    let mut skip_names = false;
    let mut skip_one = false;
    let mut k = 0;
    while k < tokens.len() {
        let t = &tokens[k];
        k += 1;
        let (l, c) = (t.line, t.col);
        let mut was_start = false;
        match &t.tok {
            Tok::Newline | Tok::FileBegin(_) | Tok::FileEnd | Tok::Eof => {
                at_start = true;
                skip_names = false;
                skip_one = false;
                continue;
            }
            Tok::Op(";") | Tok::Op("{") | Tok::Op("}") => {
                at_start = true;
                skip_names = false;
                skip_one = false;
                continue;
            }
            _ if t.file == 0 => {
                at_start = false;
                continue;
            }
            Tok::Op(".") => {
                if let Some(Token { tok: Tok::Ident(n), sp: false, .. }) = tokens.get(k) {
                    k += 1;
                    if at_start {
                        out.push(("directive", l, c, l, c + n.chars().count() as u32, t.file));
                        skip_names = NAME_LISTS.contains(&n.as_str());
                        skip_one = ONE_NAME.contains(&n.as_str());
                    } else if is_op(tokens.get(k), "(") {
                        let col = tokens[k - 1].col;
                        out.push(("objFunctionCall", l, col, l, col + n.chars().count() as u32 - 1, t.file));
                    }
                }
            }
            Tok::Ident(n) => {
                let end = c + n.chars().count() as u32 - 1;
                let call = matches!(tokens.get(k), Some(Token { tok: Tok::Op("("), sp: false, .. }));
                if skip_names || skip_one || is_op(tokens.get(k), "=") && !at_start {
                    skip_one = false;
                } else if n == "var" {
                    skip_one = true;
                } else if at_start && is_op(tokens.get(k), ":") && !is_op(tokens.get(k + 1), "=") {
                    out.push(("label", l, c, l, end + 1, t.file));
                    k += 1;
                    was_start = true;
                } else if at_start && mnemonics.contains(n) {
                    out.push(("mnemonic", l, c, l, end, t.file));
                } else if at_start && call {
                    out.push(("macroExecution", l, c, l, end, t.file));
                } else if at_start && !is_op(tokens.get(k), "=") && !is_op(tokens.get(k), ".") {
                    out.push(("pseudoCommandExecution", l, c, l, end, t.file));
                } else if call {
                    out.push(("functionCall", l, c, l, end, t.file));
                } else if !matches!(n.as_str(), "true" | "false" | "null") && !(matches!(n.as_str(), "x" | "y" | "z") && is_op(tokens.get(k.wrapping_sub(2)), ",")) {
                    out.push(("symbolReference", l, c, l, end, t.file));
                }
            }
            Tok::MultiDecl(n) => {
                out.push(("label", l, c, l, c + n.chars().count() as u32 + 1, t.file));
                was_start = true;
            }
            Tok::MultiRef(n, d) => {
                out.push(("symbolReference", l, c, l, c + n.chars().count() as u32 + d.unsigned_abs(), t.file));
            }
            Tok::Op(o) if OPERATORS.contains(o) && !at_start => {
                out.push(("operator", l, c, l, c + o.len() as u32 - 1, t.file));
            }
            _ => {}
        }
        at_start = was_start;
    }
    out
}

/// The files in the order of the [files] section. The original numbers them as its other sections
/// use them: the file with the error, then the main source, its imports, the built-in macros last.
fn file_order(sel: &str, o: &Outcome) -> Vec<usize> {
    let mut order = vec![];
    if chosen(sel, "errors") {
        for e in o.errors.iter().filter(|e| e.1 > 0) {
            if !order.contains(&e.0) {
                order.push(e.0);
            }
        }
    }
    if chosen(sel, "syntax") {
        for f in (1..o.files.len()).chain(0..1.min(o.files.len())) {
            if !order.contains(&f) {
                order.push(f);
            }
        }
    }
    order
}

/// The whole text, with the sections in the original's order and its line ends.
pub fn text(sel: &str, o: &Outcome) -> String {
    let order = file_order(sel, o);
    let file_index = |f: usize| order.iter().position(|x| *x == f).unwrap_or(f);
    let mut s = String::new();
    if chosen(sel, "version") {
        s.push_str(&format!("[version]\n{}\n", VERSION));
    }
    if chosen(sel, "libraries") {
        s.push_str("[libraries]\n");
        s.push_str(&LIBRARIES.replace('\r', ""));
    }
    if chosen(sel, "directives") {
        s.push_str("[directives]\n");
        for (d, e, t) in DIRECTIVES {
            s.push_str(&format!("{};{};{}\n", d, e, t));
        }
    }
    if chosen(sel, "preprocessorDirectives") {
        s.push_str("[ppdirectives]\n");
        for (d, e, t) in PP_DIRECTIVES {
            s.push_str(&format!("{};{};{}\n", d, e, t));
        }
    }
    if chosen(sel, "errors") {
        s.push_str("[errors]\n");
        for (f, l, c, end, msg) in &o.errors {
            s.push_str(&format!("Error;{},{},{},{},{};{}\n", l, c, l, end.max(c), file_index(*f), msg));
        }
    }
    if chosen(sel, "syntax") {
        s.push_str("[syntax]\n");
        for (kind, l1, c1, l2, c2, f) in &o.syntax {
            s.push_str(&format!("{};{},{},{},{},{}\n", kind, l1, c1, l2, c2, file_index(*f)));
        }
    }
    if chosen(sel, "files") {
        s.push_str("[files]\n");
        for (i, f) in order.iter().enumerate() {
            if let Some(name) = o.files.get(*f) {
                // the name of the built-in file is R65's own
                s.push_str(&format!("{};{}\n", i, name));
            }
        }
    }
    s.replace('\n', "\r\n")
}
