//! Turns preprocessed source lines into tokens.

use crate::preproc::PLine;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// a character that starts no token: a syntax error wherever it comes
    Bad,
    Ident(String),
    Num(f64),
    Str(String),
    Char(char),
    /// `!name:` declaration of a multi label (name may be empty)
    MultiDecl(String),
    /// `!name+` / `!name--` reference: positive = forward count, negative = backward
    MultiRef(String, i32),
    Op(&'static str),
    Newline,
    FileBegin(usize),
    FileEnd,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub file: usize,
    pub line: u32,
    pub col: u32,
    /// whitespace directly before this token
    pub sp: bool,
}

const OPS2: [&str; 14] = ["<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "++", "--", "+=", "-=", "*=", "/="];
const OPS1: [&str; 23] = [
    "+", "-", "*", "/", "&", "|", "^", "~", "!", "<", ">", "?", ":", ";", ",", "(", ")", "[", "]", "{", "}", "=", "#",
];

fn op_static(s: &str) -> &'static str {
    for o in OPS2.iter().chain(OPS1.iter()) {
        if *o == s {
            return o;
        }
    }
    match s {
        "@" => "@",
        "." => ".",
        _ => "?",
    }
}

pub struct LexError {
    pub file: usize,
    pub line: u32,
    pub col: u32,
    pub msg: String,
}

fn is_id_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
fn is_id(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// A problem found before parsing. `key` is the place in the token list, for the order of the errors.
pub struct Diag {
    pub key: usize,
    pub err: LexError,
}

fn is_group(t: &Token) -> bool {
    matches!(t.tok, Tok::Op("(") | Tok::Op("["))
}

/// Pairs the brackets of each source file. A closing bracket without its pair is reported and
/// removed, and so are the line ends inside `( )` and `[ ]`, where they mean nothing.
/// An open `(` makes the original read ahead, so what it finds there comes before later syntax errors.
fn pair_brackets(raw: Vec<Token>, mut lex_errors: Vec<(usize, LexError)>) -> (Vec<Token>, Vec<Diag>) {
    let mut out: Vec<Token> = Vec::with_capacity(raw.len());
    let mut diags = vec![];
    let mut stack: Vec<usize> = vec![];
    let mut outer: Vec<Vec<usize>> = vec![];
    lex_errors.reverse();
    for (i, t) in raw.into_iter().enumerate() {
        while lex_errors.last().is_some_and(|e| e.0 <= i) {
            if let Some((_, err)) = lex_errors.pop() {
                diags.push(Diag { key: out.len(), err });
            }
        }
        let problem = |t: &Token, msg: &str| LexError { file: t.file, line: t.line, col: t.col, msg: msg.to_string() };
        match t.tok {
            Tok::FileBegin(_) => outer.push(std::mem::take(&mut stack)),
            Tok::FileEnd | Tok::Eof => {
                for o in stack.iter().rev() {
                    let key = if is_group(&out[*o]) { *o } else { out.len() };
                    diags.push(Diag { key, err: problem(&out[*o], "Missing end token") });
                }
                stack = outer.pop().unwrap_or_default();
            }
            Tok::Op("(") | Tok::Op("[") | Tok::Op("{") => stack.push(out.len()),
            Tok::Op(c @ (")" | "]" | "}")) => {
                let open = match c {
                    ")" => "(",
                    "]" => "[",
                    _ => "{",
                };
                match stack.last() {
                    Some(o) if matches!(out[*o].tok, Tok::Op(x) if x == open) => {
                        stack.pop();
                    }
                    last => {
                        let key = last.filter(|o| is_group(&out[**o])).copied().unwrap_or(out.len());
                        diags.push(Diag { key, err: problem(&t, "Missing start token") });
                        continue;
                    }
                }
            }
            Tok::Newline if stack.last().is_some_and(|o| is_group(&out[*o])) => continue,
            _ => {}
        }
        out.push(t);
    }
    (out, diags)
}

pub fn lex(lines: &[PLine]) -> (Vec<Token>, Vec<Diag>) {
    let mut out = Vec::new();
    let mut errors = vec![];
    let mut cur_file = 0usize;
    // the end of the last line: where the file ends for an error
    let mut end = (0u32, 0u32);
    for pl in lines {
        match pl {
            PLine::Begin(f) => {
                out.push(Token { tok: Tok::FileBegin(*f), file: *f, line: 0, col: 0, sp: true });
                cur_file = *f;
            }
            PLine::End(f, back) => {
                out.push(Token { tok: Tok::FileEnd, file: *f, line: end.0, col: end.1, sp: true });
                cur_file = *back;
            }
            PLine::Error { file, line, col, msg } => {
                errors.push((out.len(), LexError { file: *file, line: *line, col: *col, msg: msg.clone() }));
            }
            PLine::Text { file, line, text } => {
                let first = out.len();
                if let Err(e) = lex_line(*file, *line, text, &mut out) {
                    out.truncate(first);
                    errors.push((first, e));
                }
                end = (*line, text.len() as u32 + 1);
                out.push(Token { tok: Tok::Newline, file: *file, line: *line, col: end.1, sp: false });
            }
        }
    }
    out.push(Token { tok: Tok::Eof, file: cur_file, line: end.0, col: end.1, sp: true });
    pair_brackets(out, errors)
}

fn lex_line(file: usize, line: u32, text: &str, out: &mut Vec<Token>) -> Result<(), LexError> {
    let ch: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    let mut sp = true;
    let err = |col: usize, msg: &str| LexError { file, line, col: col as u32 + 1, msg: msg.to_string() };
    while i < ch.len() {
        let c = ch[i];
        if c.is_whitespace() {
            sp = true;
            i += 1;
            continue;
        }
        let start = i;
        let push = |out: &mut Vec<Token>, tok: Tok, sp: bool| {
            out.push(Token { tok, file, line, col: start as u32 + 1, sp });
        };
        if is_id_start(c) {
            let mut j = i;
            while j < ch.len() && is_id(ch[j]) {
                j += 1;
            }
            let s: String = ch[i..j].iter().collect();
            push(out, Tok::Ident(s), sp);
            i = j;
        } else if c.is_ascii_digit() {
            let mut j = i;
            while j < ch.len() && ch[j].is_ascii_digit() {
                j += 1;
            }
            if j + 1 < ch.len() && ch[j] == '.' && ch[j + 1].is_ascii_digit() {
                j += 1;
                while j < ch.len() && ch[j].is_ascii_digit() {
                    j += 1;
                }
            }
            let s: String = ch[i..j].iter().collect();
            push(out, Tok::Num(s.parse::<f64>().unwrap_or(0.0)), sp);
            i = j;
        } else if c == '$' && i + 1 < ch.len() && ch[i + 1].is_ascii_hexdigit() {
            let mut j = i + 1;
            while j < ch.len() && ch[j].is_ascii_hexdigit() {
                j += 1;
            }
            let s: String = ch[i + 1..j].iter().collect();
            push(out, Tok::Num(u64::from_str_radix(&s, 16).unwrap_or(0) as f64), sp);
            i = j;
        } else if c == '%' && i + 1 < ch.len() && (ch[i + 1] == '0' || ch[i + 1] == '1') {
            let mut j = i + 1;
            while j < ch.len() && (ch[j] == '0' || ch[j] == '1') {
                j += 1;
            }
            let s: String = ch[i + 1..j].iter().collect();
            push(out, Tok::Num(u64::from_str_radix(&s, 2).unwrap_or(0) as f64), sp);
            i = j;
        } else if c == '"' || (c == '@' && i + 1 < ch.len() && ch[i + 1] == '"') {
            let esc = c == '@';
            let mut j = if esc { i + 2 } else { i + 1 };
            let mut s = String::new();
            loop {
                if j >= ch.len() {
                    return Err(err(start, "Unterminated string"));
                }
                let d = ch[j];
                if d == '"' {
                    j += 1;
                    break;
                }
                if esc && d == '\\' && j + 1 < ch.len() {
                    let e = ch[j + 1];
                    j += 2;
                    match e {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        'b' => s.push('\u{8}'),
                        'f' => s.push('\u{c}'),
                        '\\' => s.push('\\'),
                        '"' => s.push('"'),
                        '$' => {
                            let mut h = String::new();
                            while h.len() < 2 && j < ch.len() && ch[j].is_ascii_hexdigit() {
                                h.push(ch[j]);
                                j += 1;
                            }
                            let v = u32::from_str_radix(&h, 16).unwrap_or(0);
                            // raw byte values travel as chars from $800 upwards
                            s.push(char::from_u32(0x800 + v).unwrap());
                        }
                        other => {
                            s.push('\\');
                            s.push(other)
                        }
                    }
                    continue;
                }
                s.push(d);
                j += 1;
            }
            push(out, Tok::Str(s), sp);
            i = j;
        } else if c == '\'' && i + 2 < ch.len() && ch[i + 2] == '\'' {
            push(out, Tok::Char(ch[i + 1]), sp);
            i += 3;
        } else if c == '!' && !(i + 1 < ch.len() && ch[i + 1] == '=') {
            // multi label declaration or reference?
            let mut j = i + 1;
            while j < ch.len() && is_id(ch[j]) {
                j += 1;
            }
            let name: String = ch[i + 1..j].iter().collect();
            if j < ch.len() && ch[j] == ':' && !(j + 1 < ch.len() && ch[j + 1] == '=') {
                push(out, Tok::MultiDecl(name), sp);
                i = j + 1;
            } else if j < ch.len() && (ch[j] == '+' || ch[j] == '-') && (name.is_empty() || !name.chars().next().unwrap().is_ascii_digit()) {
                let d = ch[j];
                let mut k = j;
                while k < ch.len() && ch[k] == d {
                    k += 1;
                }
                // `!x - 1` style with spaces never reaches here; `!a+b` (not followed by a separator) is a boolean not.
                let next = if k < ch.len() { ch[k] } else { ' ' };
                if is_id(next) && !name.is_empty() {
                    push(out, Tok::Op("!"), sp);
                    i += 1;
                } else {
                    let n = (k - j) as i32;
                    push(out, Tok::MultiRef(name, if d == '+' { n } else { -n }), sp);
                    i = k;
                }
            } else {
                push(out, Tok::Op("!"), sp);
                i += 1;
            }
        } else {
            let two: String = ch[i..(i + 2).min(ch.len())].iter().collect();
            if two.len() == 2 && OPS2.contains(&two.as_str()) {
                push(out, Tok::Op(op_static(&two)), sp);
                i += 2;
            } else {
                let one = c.to_string();
                if OPS1.contains(&one.as_str()) || c == '@' || c == '.' {
                    push(out, Tok::Op(op_static(&one)), sp);
                    i += 1;
                } else {
                    push(out, Tok::Bad, sp);
                    i += 1;
                }
            }
        }
        sp = false;
    }
    Ok(())
}
