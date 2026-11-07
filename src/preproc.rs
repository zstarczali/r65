//! The preprocessor: comments, #define/#undef, #if/#elif/#else/#endif and
//! #import/#importif/#importonce. Works on lines, before the main parser.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum PLine {
    Text { file: usize, line: u32, text: String },
    /// start of an imported file
    Begin(usize),
    /// end of an imported file; second value = the file we return to
    End(usize, usize),
}

pub struct SrcFile {
    pub path: PathBuf,
    pub lines: Vec<String>,
}

#[derive(Default)]
pub struct Sources {
    pub files: Vec<SrcFile>,
}

impl Sources {
    pub fn name(&self, f: usize) -> String {
        self.files
            .get(f)
            .map(|s| s.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
            .unwrap_or_default()
    }
    pub fn full(&self, f: usize) -> String {
        self.files.get(f).map(|s| s.path.to_string_lossy().to_string()).unwrap_or_default()
    }
    pub fn add_text(&mut self, path: PathBuf, text: &str) -> usize {
        self.files.push(SrcFile { path, lines: text.lines().map(|s| s.to_string()).collect() });
        self.files.len() - 1
    }
}

pub struct PreErr {
    pub file: usize,
    pub line: u32,
    pub msg: String,
}

pub struct Pre<'a> {
    pub defines: HashSet<String>,
    pub libdirs: Vec<PathBuf>,
    pub sources: &'a mut Sources,
    pub once: HashSet<PathBuf>,
    pub out: Vec<PLine>,
    pub imported: Vec<PathBuf>,
    pub replace: Vec<(PathBuf, PathBuf)>,
    depth: usize,
}

/// Replaces // and /* */ comments with spaces (newlines are kept) and leaves strings untouched.
pub fn strip_comments(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut o = String::with_capacity(src.len());
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '"' || (ch == '@' && i + 1 < c.len() && c[i + 1] == '"') {
            let esc = ch == '@';
            o.push(ch);
            i += 1;
            if esc {
                o.push('"');
                i += 1;
            }
            while i < c.len() && c[i] != '"' && c[i] != '\n' {
                if esc && c[i] == '\\' && i + 1 < c.len() {
                    o.push(c[i]);
                    i += 1;
                }
                o.push(c[i]);
                i += 1;
            }
            if i < c.len() && c[i] == '"' {
                o.push('"');
                i += 1;
            }
        } else if ch == '\'' && i + 2 < c.len() && c[i + 2] == '\'' {
            o.push(c[i]);
            o.push(c[i + 1]);
            o.push(c[i + 2]);
            i += 3;
        } else if ch == '/' && i + 1 < c.len() && c[i + 1] == '/' {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && i + 1 < c.len() && c[i + 1] == '*' {
            i += 2;
            o.push_str("  ");
            while i < c.len() && !(c[i] == '*' && i + 1 < c.len() && c[i + 1] == '/') {
                o.push(if c[i] == '\n' { '\n' } else { ' ' });
                i += 1;
            }
            i += 2;
            o.push_str("  ");
        } else {
            o.push(ch);
            i += 1;
        }
    }
    o
}

pub fn find_file(name: &str, from_dir: &Path, libdirs: &[PathBuf]) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute() {
        return if p.exists() { Some(p.to_path_buf()) } else { None };
    }
    let c = from_dir.join(p);
    if c.exists() {
        return Some(c);
    }
    for d in libdirs {
        let c = d.join(p);
        if c.exists() {
            return Some(c);
        }
    }
    None
}

impl<'a> Pre<'a> {
    pub fn new(sources: &'a mut Sources, defines: HashSet<String>, libdirs: Vec<PathBuf>) -> Self {
        Pre { defines, libdirs, sources, once: HashSet::new(), out: vec![], imported: vec![], replace: vec![], depth: 0 }
    }

    pub fn run_text(&mut self, path: PathBuf, text: &str, back: usize) -> Result<usize, PreErr> {
        let id = self.sources.add_text(path.clone(), text);
        self.out.push(PLine::Begin(id));
        let stripped = strip_comments(text);
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        // stack of (this branch active, some branch taken, parent active)
        let mut stack: Vec<(bool, bool, bool)> = vec![];
        let active = |st: &Vec<(bool, bool, bool)>| st.last().map(|s| s.0 && s.2).unwrap_or(true);
        for (n, raw) in stripped.split('\n').enumerate() {
            let line_no = n as u32 + 1;
            let l = raw.trim_end_matches('\r');
            let t = l.trim_start();
            let perr = |msg: String| PreErr { file: id, line: line_no, msg };
            if let Some(rest) = t.strip_prefix('#') {
                let (word, arg) = split_word(rest);
                match word {
                    "if" => {
                        let pa = active(&stack);
                        let v = pa && self.eval(arg).map_err(perr)?;
                        stack.push((v, v, pa));
                    }
                    "elif" => {
                        let s = stack.last_mut().ok_or_else(|| perr("#elif without #if".into()))?;
                        if s.1 {
                            s.0 = false;
                        } else {
                            let v = s.2 && self.eval(arg).map_err(perr)?;
                            s.0 = v;
                            s.1 = v;
                        }
                    }
                    "else" => {
                        let s = stack.last_mut().ok_or_else(|| perr("#else without #if".into()))?;
                        s.0 = !s.1;
                        s.1 = true;
                    }
                    "endif" => {
                        stack.pop().ok_or_else(|| perr("#endif without #if".into()))?;
                    }
                    _ if !active(&stack) => {}
                    "define" => {
                        self.defines.insert(arg.trim().to_string());
                    }
                    "undef" => {
                        self.defines.remove(arg.trim());
                    }
                    "importonce" => {
                        let canon = path.canonicalize().unwrap_or(path.clone());
                        if self.once.contains(&canon) {
                            break;
                        }
                        self.once.insert(canon);
                    }
                    "import" | "importif" => {
                        let (cond, fname) = if word == "importif" {
                            let q = arg.find('"').ok_or_else(|| perr("Missing filename".into()))?;
                            (self.eval(&arg[..q]).map_err(perr)?, &arg[q..])
                        } else {
                            (true, arg)
                        };
                        if cond {
                            let fname = fname.trim().trim_matches('"');
                            self.import(fname, &dir, id).map_err(perr)?;
                        }
                    }
                    _ => {
                        // not a preprocessor directive we know: hand it to the parser
                        self.out.push(PLine::Text { file: id, line: line_no, text: l.to_string() });
                    }
                }
                continue;
            }
            if !active(&stack) {
                continue;
            }
            // deprecated forms of the import directives
            if let Some(rest) = t.strip_prefix(".importonce") {
                if rest.trim().is_empty() {
                    let canon = path.canonicalize().unwrap_or(path.clone());
                    if self.once.contains(&canon) {
                        break;
                    }
                    self.once.insert(canon);
                    continue;
                }
            }
            if let Some(rest) = t.strip_prefix(".import") {
                let r = rest.trim_start();
                if let Some(f) = r.strip_prefix("source") {
                    let fname = f.trim().trim_matches('"');
                    self.import(fname, &dir, id).map_err(perr)?;
                    continue;
                }
            }
            self.out.push(PLine::Text { file: id, line: line_no, text: l.to_string() });
        }
        self.out.push(PLine::End(id, back));
        Ok(id)
    }

    fn import(&mut self, fname: &str, dir: &Path, cur: usize) -> Result<(), String> {
        let mut p = find_file(fname, dir, &self.libdirs).ok_or_else(|| format!("Can't open file '{}'", fname))?;
        for (a, b) in &self.replace {
            if p.canonicalize().ok() == a.canonicalize().ok() {
                p = b.clone();
            }
        }
        if self.depth > 64 {
            return Err("Too deep import nesting".into());
        }
        let bytes = std::fs::read(&p).map_err(|_| format!("Can't open file '{}'", fname))?;
        let text = String::from_utf8_lossy(&bytes).to_string();
        self.imported.push(p.clone());
        self.depth += 1;
        let r = self.run_text(p, &text, cur).map_err(|e| e.msg);
        self.depth -= 1;
        r.map(|_| ())
    }

    fn eval(&self, s: &str) -> Result<bool, String> {
        let toks = pre_tokens(s);
        let mut p = 0;
        let v = self.e_or(&toks, &mut p)?;
        if p != toks.len() {
            return Err(format!("Invalid preprocessor expression: '{}'", s.trim()));
        }
        Ok(v)
    }
    fn e_or(&self, t: &[String], p: &mut usize) -> Result<bool, String> {
        let mut v = self.e_and(t, p)?;
        while *p < t.len() && t[*p] == "||" {
            *p += 1;
            let r = self.e_and(t, p)?;
            v = v || r;
        }
        Ok(v)
    }
    fn e_and(&self, t: &[String], p: &mut usize) -> Result<bool, String> {
        let mut v = self.e_eq(t, p)?;
        while *p < t.len() && t[*p] == "&&" {
            *p += 1;
            let r = self.e_eq(t, p)?;
            v = v && r;
        }
        Ok(v)
    }
    fn e_eq(&self, t: &[String], p: &mut usize) -> Result<bool, String> {
        let mut v = self.e_un(t, p)?;
        while *p < t.len() && (t[*p] == "==" || t[*p] == "!=") {
            let eq = t[*p] == "==";
            *p += 1;
            let r = self.e_un(t, p)?;
            v = if eq { v == r } else { v != r };
        }
        Ok(v)
    }
    fn e_un(&self, t: &[String], p: &mut usize) -> Result<bool, String> {
        if *p >= t.len() {
            return Err("Missing preprocessor expression".into());
        }
        let tok = t[*p].clone();
        *p += 1;
        match tok.as_str() {
            "!" => Ok(!self.e_un(t, p)?),
            "(" => {
                let v = self.e_or(t, p)?;
                if *p < t.len() && t[*p] == ")" {
                    *p += 1;
                    Ok(v)
                } else {
                    Err("Missing )".into())
                }
            }
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Ok(self.defines.contains(&tok)),
        }
    }
}

fn split_word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    let e = s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(s.len());
    (&s[..e], &s[e..])
}

fn pre_tokens(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut v = vec![];
    let mut i = 0;
    while i < c.len() {
        if c[i].is_whitespace() {
            i += 1;
            continue;
        }
        if c[i].is_ascii_alphanumeric() || c[i] == '_' {
            let st = i;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            v.push(c[st..i].iter().collect());
            continue;
        }
        let two: String = c[i..(i + 2).min(c.len())].iter().collect();
        if ["||", "&&", "==", "!="].contains(&two.as_str()) {
            v.push(two);
            i += 2;
        } else {
            v.push(c[i].to_string());
            i += 1;
        }
    }
    v
}
