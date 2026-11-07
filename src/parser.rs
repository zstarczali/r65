//! Parser: tokens to syntax tree.

use crate::ast::*;
use crate::lexer::{Tok, Token};
use std::collections::HashSet;
use std::rc::Rc;

pub struct ParseError {
    pub pos: Pos,
    pub msg: String,
}

type R<T> = Result<T, ParseError>;

pub struct Parser<'a> {
    t: &'a [Token],
    p: usize,
    mnemonics: &'a HashSet<String>,
}

fn is_end(t: &Tok) -> bool {
    matches!(t, Tok::Newline | Tok::Eof | Tok::FileEnd | Tok::Op(";") | Tok::Op("}"))
}

impl<'a> Parser<'a> {
    pub fn new(t: &'a [Token], mnemonics: &'a HashSet<String>) -> Self {
        Parser { t, p: 0, mnemonics }
    }

    fn peek(&self) -> &Tok {
        &self.t[self.p.min(self.t.len() - 1)].tok
    }
    fn peek_at(&self, n: usize) -> &Tok {
        &self.t[(self.p + n).min(self.t.len() - 1)].tok
    }
    fn tok_at(&self, n: usize) -> &Token {
        &self.t[(self.p + n).min(self.t.len() - 1)]
    }
    fn pos(&self) -> Pos {
        let t = &self.t[self.p.min(self.t.len() - 1)];
        Pos { file: t.file, line: t.line, col: t.col }
    }
    fn next(&mut self) -> Tok {
        let t = self.peek().clone();
        if self.p < self.t.len() - 1 {
            self.p += 1;
        }
        t
    }
    fn is_op(&self, o: &str) -> bool {
        matches!(self.peek(), Tok::Op(x) if *x == o)
    }
    fn eat_op(&mut self, o: &str) -> bool {
        if self.is_op(o) {
            self.next();
            true
        } else {
            false
        }
    }
    fn err<T>(&self, msg: &str) -> R<T> {
        Err(ParseError { pos: self.pos(), msg: msg.to_string() })
    }
    fn expect_op(&mut self, o: &str) -> R<()> {
        if self.eat_op(o) {
            Ok(())
        } else {
            self.err(&format!("Syntax error, expected '{}'", o))
        }
    }
    fn ident(&mut self) -> R<String> {
        match self.next() {
            Tok::Ident(s) => Ok(s),
            _ => {
                self.p -= 1;
                self.err("Syntax error, expected a name")
            }
        }
    }
    fn skip_nl(&mut self) {
        while matches!(self.peek(), Tok::Newline) || self.is_op(";") {
            self.next();
        }
    }

    // ---------------------------------------------------------------- program
    pub fn program(&mut self) -> R<Block> {
        let b = self.block_until_eof()?;
        Ok(b)
    }

    fn block_until_eof(&mut self) -> R<Block> {
        let mut stmts = vec![];
        loop {
            self.skip_nl();
            match self.peek() {
                Tok::Eof => break,
                Tok::Op("}") => return self.err("Syntax error, unexpected '}'"),
                Tok::FileEnd => {
                    self.next();
                }
                _ => self.statement(&mut stmts)?,
            }
        }
        Ok(make_block(stmts))
    }

    /// statements up to the matching `}` (consumed)
    fn brace_block(&mut self) -> R<Block> {
        self.skip_nl();
        self.expect_op("{")?;
        let mut stmts = vec![];
        loop {
            self.skip_nl();
            match self.peek() {
                Tok::Op("}") => {
                    self.next();
                    break;
                }
                Tok::Eof => return self.err("Syntax error, missing '}'"),
                Tok::FileEnd => {
                    self.next();
                }
                _ => self.statement(&mut stmts)?,
            }
        }
        Ok(make_block(stmts))
    }

    /// statements of an imported file, up to its end
    fn file_block(&mut self) -> R<Block> {
        let mut stmts = vec![];
        loop {
            self.skip_nl();
            match self.peek() {
                Tok::FileEnd => {
                    self.next();
                    break;
                }
                Tok::Eof => break,
                Tok::Op("}") => return self.err("Syntax error, unexpected '}'"),
                _ => self.statement(&mut stmts)?,
            }
        }
        Ok(make_block(stmts))
    }

    /// a body: either `{...}` or a single statement
    fn body(&mut self) -> R<Block> {
        let save = self.p;
        self.skip_nl();
        if self.is_op("{") {
            return self.brace_block();
        }
        self.p = save;
        while matches!(self.peek(), Tok::Newline) {
            self.next();
        }
        let mut stmts = vec![];
        self.statement(&mut stmts)?;
        Ok(make_block(stmts))
    }

    fn end_stmt(&mut self) -> R<()> {
        let t = self.peek().clone();
        if is_end(&t) {
            if matches!(t, Tok::Op(";")) {
                self.next();
            }
            return Ok(());
        }
        if let Tok::Ident(s) = &t {
            if s == "else" {
                return Ok(());
            }
        }
        self.err("Syntax error")
    }

    fn statement(&mut self, out: &mut Vec<S>) -> R<()> {
        let pos = self.pos();
        let push = |out: &mut Vec<S>, k: SK| out.push(S { k, pos });
        match self.peek().clone() {
            Tok::FileBegin(id) => {
                self.next();
                let b = self.file_block()?;
                push(out, SK::File(id, b));
                return Ok(());
            }
            Tok::MultiDecl(name) => {
                self.next();
                push(out, SK::Label { name, multi: true, root: false });
                return Ok(());
            }
            Tok::Op("{") => {
                let b = self.brace_block()?;
                push(out, SK::Scope(b));
                return Ok(());
            }
            Tok::Op("@") => {
                if let (Tok::Ident(n), Tok::Op(":")) = (self.peek_at(1).clone(), self.peek_at(2).clone()) {
                    self.p += 3;
                    push(out, SK::Label { name: n, multi: false, root: true });
                    return Ok(());
                }
                return self.err("Syntax error");
            }
            Tok::Op(":") => {
                // explicit macro / pseudocommand call
                self.next();
                let root = self.eat_op("@");
                let name = self.ident()?;
                if self.is_op("(") {
                    let args = self.call_args()?;
                    push(out, SK::MacroCall { name, root, args });
                } else {
                    let args = self.pseudo_args()?;
                    push(out, SK::PseudoCall { name, root, args });
                }
                return self.end_stmt();
            }
            Tok::Op("*=") => {
                self.next();
                return self.set_pc(out, pos);
            }
            Tok::Op("*") if matches!(self.peek_at(1), Tok::Op("=")) => {
                self.p += 2;
                return self.set_pc(out, pos);
            }
            Tok::Op(".") => {
                if let Tok::Ident(d) = self.peek_at(1).clone() {
                    self.p += 2;
                    return self.directive(&d, out, pos);
                }
                return self.err("Syntax error");
            }
            Tok::Ident(name) => {
                if matches!(self.peek_at(1), Tok::Op(":")) && !matches!(self.peek_at(2), Tok::Op("=")) {
                    self.p += 2;
                    push(out, SK::Label { name, multi: false, root: false });
                    return Ok(());
                }
                if self.mnemonics.contains(&name) {
                    self.next();
                    let mut suffix = None;
                    if self.is_op(".") && !self.tok_at(0).sp {
                        if let Tok::Ident(s) = self.peek_at(1).clone() {
                            self.p += 2;
                            suffix = Some(s);
                        }
                    }
                    let arg = self.mnem_arg(true)?;
                    push(out, SK::Instr { mn: name, suffix, arg });
                    return self.end_stmt();
                }
                let nxt = self.peek_at(1).clone();
                match nxt {
                    // name( directly after the name is a macro call; with a space it may be
                    // a pseudocommand with an indirect argument: mov ($10),y : $20
                    Tok::Op("(") if !self.tok_at(1).sp => {
                        self.next();
                        let args = self.call_args()?;
                        push(out, SK::MacroCall { name, root: false, args });
                        return self.end_stmt();
                    }
                    Tok::Op(".") | Tok::Op("=") | Tok::Op("+=") | Tok::Op("-=") | Tok::Op("*=") | Tok::Op("/=")
                    | Tok::Op("++") | Tok::Op("--") | Tok::Op("[")
                        if !self.tok_at(1).sp || !matches!(nxt, Tok::Op(".")) =>
                    {
                        let e = self.expr()?;
                        push(out, SK::Eval(e));
                        return self.end_stmt();
                    }
                    _ => {}
                }
                self.next();
                let args = self.pseudo_args()?;
                push(out, SK::PseudoCall { name, root: false, args });
                return self.end_stmt();
            }
            _ => {}
        }
        self.err("Syntax error")
    }

    fn set_pc(&mut self, out: &mut Vec<S>, pos: Pos) -> R<()> {
        let e = self.expr()?;
        let mut name = None;
        let mut virt = false;
        if matches!(self.peek(), Tok::Str(_)) {
            name = Some(self.expr()?);
        }
        if let Tok::Ident(v) = self.peek() {
            if v == "virtual" {
                virt = true;
                self.next();
            }
        }
        out.push(S { k: SK::SetPc { e, name, virt }, pos });
        self.end_stmt()
    }

    fn call_args(&mut self) -> R<Vec<E>> {
        self.expect_op("(")?;
        let mut v = vec![];
        self.skip_nl_in();
        if self.eat_op(")") {
            return Ok(v);
        }
        loop {
            self.skip_nl_in();
            v.push(self.expr()?);
            self.skip_nl_in();
            if self.eat_op(",") {
                continue;
            }
            self.expect_op(")")?;
            break;
        }
        Ok(v)
    }
    fn skip_nl_in(&mut self) {
        while matches!(self.peek(), Tok::Newline) {
            self.next();
        }
    }

    fn pseudo_args(&mut self) -> R<Vec<Arg>> {
        let mut v = vec![];
        if is_end(self.peek()) {
            return Ok(v);
        }
        loop {
            v.push(self.mnem_arg(false)?);
            if self.eat_op(":") {
                continue;
            }
            break;
        }
        Ok(v)
    }

    fn arg_end(&self) -> bool {
        let t = self.peek();
        is_end(t) || matches!(t, Tok::Op(":")) || matches!(t, Tok::Ident(s) if s == "else")
    }

    fn mnem_arg(&mut self, allow_label: bool) -> R<Arg> {
        let mut arg = Arg { mode: Mode::None, e: None, e2: None, label: None };
        if self.arg_end() {
            return Ok(arg);
        }
        if allow_label {
            if let (Tok::Ident(n), Tok::Op(":")) = (self.peek().clone(), self.peek_at(1).clone()) {
                if !is_end(self.peek_at(2)) {
                    self.p += 2;
                    arg.label = Some(n);
                }
            }
        }
        if self.eat_op("#") {
            arg.mode = Mode::Imm;
            arg.e = Some(self.expr()?);
            return Ok(arg);
        }
        if self.is_op("(") {
            // find the matching parenthesis
            let mut depth = 0;
            let mut j = self.p;
            loop {
                match &self.t[j].tok {
                    Tok::Op("(") | Tok::Op("[") => depth += 1,
                    Tok::Op(")") | Tok::Op("]") => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    Tok::Newline | Tok::Eof | Tok::FileEnd => break,
                    _ => {}
                }
                j += 1;
            }
            let after = &self.t[(j + 1).min(self.t.len() - 1)].tok;
            let after2 = &self.t[(j + 2).min(self.t.len() - 1)].tok;
            let ind_end = is_end(after) || matches!(after, Tok::Op(":")) || matches!(after, Tok::Ident(s) if s=="else");
            let ind_y = matches!(after, Tok::Op(",")) && matches!(after2, Tok::Ident(s) if s == "y");
            if ind_end || ind_y {
                self.next();
                let e = self.expr()?;
                if self.eat_op(",") {
                    match self.next() {
                        Tok::Ident(s) if s == "x" => {}
                        _ => return self.err("Syntax error, expected x"),
                    }
                    self.expect_op(")")?;
                    arg.mode = Mode::IndX;
                    arg.e = Some(e);
                    return Ok(arg);
                }
                self.expect_op(")")?;
                if ind_y {
                    self.p += 2;
                    arg.mode = Mode::IndY;
                } else {
                    arg.mode = Mode::Ind;
                }
                arg.e = Some(e);
                return Ok(arg);
            }
        }
        let e = self.expr()?;
        arg.mode = Mode::Abs;
        if self.is_op(",") {
            match self.peek_at(1).clone() {
                Tok::Ident(s) if s == "x" && self.arg_end_at(2) => {
                    self.p += 2;
                    arg.mode = Mode::AbsX
                }
                Tok::Ident(s) if s == "y" && self.arg_end_at(2) => {
                    self.p += 2;
                    arg.mode = Mode::AbsY
                }
                _ => {
                    self.next();
                    arg.e2 = Some(self.expr()?);
                    arg.mode = Mode::ZpRel;
                }
            }
        }
        arg.e = Some(e);
        Ok(arg)
    }
    fn arg_end_at(&self, n: usize) -> bool {
        let t = self.peek_at(n);
        is_end(t) || matches!(t, Tok::Op(":")) || matches!(t, Tok::Ident(s) if s == "else")
    }

    fn params(&mut self) -> R<Params> {
        self.expect_op("[")?;
        let mut v = vec![];
        loop {
            self.skip_nl_in();
            if self.eat_op("]") {
                break;
            }
            let pos = self.pos();
            let name = self.ident()?;
            let val = if self.eat_op("=") { Some(self.expr()?) } else { None };
            v.push((name, val, pos));
            self.skip_nl_in();
            if self.eat_op(",") {
                continue;
            }
            self.skip_nl_in();
            self.expect_op("]")?;
            break;
        }
        Ok(v)
    }

    fn opt_root_ident(&mut self) -> R<(String, bool)> {
        let root = self.eat_op("@");
        Ok((self.ident()?, root))
    }

    fn expr_list(&mut self) -> R<Vec<E>> {
        let mut v = vec![self.expr()?];
        while self.eat_op(",") {
            self.skip_nl_in();
            v.push(self.expr()?);
        }
        Ok(v)
    }

    fn directive(&mut self, d: &str, out: &mut Vec<S>, pos: Pos) -> R<()> {
        let k = match d {
            "byte" | "by" => SK::Data { size: 1, items: self.expr_list()? },
            "word" | "wo" => SK::Data { size: 2, items: self.expr_list()? },
            "dword" | "dw" => SK::Data { size: 4, items: self.expr_list()? },
            "text" | "te" => SK::Text(self.expr_list()?),
            "fill" | "fillword" => {
                let count = self.expr()?;
                self.expect_op(",")?;
                let val = self.expr()?;
                SK::Fill { word: d == "fillword", count, val }
            }
            "lohifill" => {
                let count = self.expr()?;
                self.expect_op(",")?;
                let val = self.expr()?;
                SK::LohiFill { count, val }
            }
            "align" => SK::Align(self.expr()?),
            "pc" => {
                self.expect_op("=")?;
                return self.set_pc(out, pos);
            }
            "pseudopc" => {
                let e = self.expr()?;
                let b = self.brace_block()?;
                out.push(S { k: SK::Pseudopc(e, b), pos });
                return Ok(());
            }
            "var" => {
                let (name, root) = self.opt_root_ident()?;
                let init = if self.eat_op("=") { Some(self.expr()?) } else { None };
                SK::Var { name, root, init }
            }
            "const" => {
                let (name, root) = self.opt_root_ident()?;
                self.expect_op("=")?;
                SK::Const { name, root, e: self.expr()? }
            }
            "label" => {
                let (name, root) = self.opt_root_ident()?;
                self.expect_op("=")?;
                let e = self.expr()?;
                let body = if self.is_op("{") { Some(self.brace_block()?) } else { None };
                out.push(S { k: SK::LabelDef { name, root, e, body }, pos });
                return self.end_stmt();
            }
            "eval" => SK::Eval(self.expr()?),
            "enum" => {
                self.skip_nl_in();
                self.expect_op("{")?;
                let mut v = vec![];
                loop {
                    self.skip_nl_in();
                    if self.eat_op("}") {
                        break;
                    }
                    let n = self.ident()?;
                    let e = if self.eat_op("=") { Some(self.expr()?) } else { None };
                    v.push((n, e));
                    self.skip_nl_in();
                    if self.eat_op(",") {
                        continue;
                    }
                    self.skip_nl_in();
                    self.expect_op("}")?;
                    break;
                }
                SK::Enum(v)
            }
            "print" => SK::Print(if self.arg_end() { E { k: EK::Str("".into()), pos } } else { self.expr()? }),
            "printnow" => SK::PrintNow(self.expr()?),
            "error" => SK::Error(self.expr()?),
            "errorif" => {
                let c = self.expr()?;
                self.eat_op(",");
                SK::ErrorIf(c, self.expr()?)
            }
            "assert" => {
                let a = self.expr()?;
                self.expect_op(",")?;
                let b = self.expr()?;
                self.expect_op(",")?;
                SK::Assert(a, b, self.expr()?)
            }
            "asserterror" => {
                let a = self.expr()?;
                self.expect_op(",")?;
                SK::AssertErr(a, self.expr()?)
            }
            "if" => {
                self.expect_op("(")?;
                let c = self.expr()?;
                self.expect_op(")")?;
                let t = self.body()?;
                let save = self.p;
                self.skip_nl();
                let e = if matches!(self.peek(), Tok::Ident(s) if s == "else") {
                    self.next();
                    Some(self.body()?)
                } else {
                    self.p = save;
                    None
                };
                out.push(S { k: SK::If(c, t, e), pos });
                return Ok(());
            }
            "for" => {
                self.expect_op("(")?;
                let init = if self.is_op(";") { vec![] } else { self.expr_list()? };
                self.expect_op(";")?;
                let cond = if self.is_op(";") { None } else { Some(self.expr()?) };
                self.expect_op(";")?;
                let step = if self.is_op(")") { vec![] } else { self.expr_list()? };
                self.expect_op(")")?;
                let body = self.body()?;
                out.push(S { k: SK::For { init, cond, step, body }, pos });
                return Ok(());
            }
            "while" => {
                self.expect_op("(")?;
                let c = self.expr()?;
                self.expect_op(")")?;
                let body = self.body()?;
                out.push(S { k: SK::While(c, body), pos });
                return Ok(());
            }
            "return" => SK::Return(if self.arg_end() { None } else { Some(self.expr()?) }),
            "function" | "macro" | "pseudocommand" => {
                let (name, root) = self.opt_root_ident()?;
                let mut params = vec![];
                if d == "pseudocommand" {
                    self.skip_nl_in();
                    while let Tok::Ident(p) = self.peek().clone() {
                        self.next();
                        params.push(p);
                        // a line break may come before the ':'; without one the list has ended
                        let after_param = self.p;
                        self.skip_nl_in();
                        if !self.eat_op(":") {
                            self.p = after_param;
                            break;
                        }
                        self.skip_nl_in();
                    }
                } else if self.eat_op("(") {
                    self.skip_nl_in();
                    if !self.eat_op(")") {
                        loop {
                            self.skip_nl_in();
                            params.push(self.ident()?);
                            self.skip_nl_in();
                            if self.eat_op(",") {
                                continue;
                            }
                            self.expect_op(")")?;
                            break;
                        }
                    }
                }
                let body = self.brace_block()?;
                let f = Rc::new(FuncDef { name, root, params, body, pos });
                out.push(S {
                    k: match d {
                        "function" => SK::Function(f),
                        "macro" => SK::Macro(f),
                        _ => SK::Pseudo(f),
                    },
                    pos,
                });
                return Ok(());
            }
            "struct" => {
                let (name, root) = self.opt_root_ident()?;
                self.expect_op("{")?;
                let mut fields = vec![];
                loop {
                    self.skip_nl_in();
                    if self.eat_op("}") {
                        break;
                    }
                    fields.push(self.ident()?);
                    self.skip_nl_in();
                    if self.eat_op(",") {
                        continue;
                    }
                    self.skip_nl_in();
                    self.expect_op("}")?;
                    break;
                }
                SK::Struct(Rc::new(StructDef { name, fields }), root)
            }
            "define" => {
                let mut names = vec![self.ident()?];
                while self.eat_op(",") {
                    names.push(self.ident()?);
                }
                let b = self.brace_block()?;
                out.push(S { k: SK::Define(names, b), pos });
                return Ok(());
            }
            "namespace" => {
                let name = self.ident()?;
                let b = if self.is_op("{") || matches!(self.peek(), Tok::Newline) && self.next_nonnl_is("{") {
                    self.brace_block()?
                } else {
                    self.rest_of_file()?
                };
                out.push(S { k: SK::Namespace(name, b), pos });
                return Ok(());
            }
            "filenamespace" => {
                let name = self.ident()?;
                let b = self.rest_of_file()?;
                out.push(S { k: SK::Namespace(name, b), pos });
                return Ok(());
            }
            "import" => {
                let kind = match self.ident()?.as_str() {
                    "binary" => ImportKind::Binary,
                    "c64" => ImportKind::C64,
                    "text" => ImportKind::Text,
                    _ => return self.err("Unknown import type"),
                };
                let file = self.expr()?;
                let mut off = None;
                let mut len = None;
                if self.eat_op(",") {
                    off = Some(self.expr()?);
                    if self.eat_op(",") {
                        len = Some(self.expr()?);
                    }
                }
                SK::Import { kind, file, off, len }
            }
            "encoding" => SK::Encoding(self.expr()?),
            "cpu" => SK::Cpu(self.ident()?),
            "zp" => {
                let b = self.brace_block()?;
                out.push(S { k: SK::Zp(b), pos });
                return Ok(());
            }
            "break" => SK::Break(if self.arg_end() { None } else { Some(self.expr()?) }),
            "watch" => {
                let mut v = vec![];
                loop {
                    if self.is_op(",") || self.arg_end() {
                        v.push(None);
                    } else {
                        v.push(Some(self.expr()?));
                    }
                    if !self.eat_op(",") {
                        break;
                    }
                }
                SK::Watch(v)
            }
            "memblock" => SK::Memblock(self.expr()?),
            "segment" => {
                let name = self.ident()?;
                let params = if self.is_op("[") { Some(self.params()?) } else { None };
                let block = if matches!(self.peek(), Tok::Str(_)) { Some(self.expr()?) } else { None };
                SK::Segment { name, params, block }
            }
            "segmentdef" => {
                let name = self.ident()?;
                let params = if self.is_op("[") { self.params()? } else { vec![] };
                SK::SegmentDef { name, params }
            }
            "segmentout" => SK::SegmentOut(self.params()?),
            "file" => SK::FileOut(self.params()?),
            "disk" => {
                if let Tok::Ident(_) = self.peek() {
                    self.next(); // custom writer name: only the built-in writer exists here
                }
                let params = self.params()?;
                self.skip_nl_in();
                self.expect_op("{")?;
                let mut files = vec![];
                loop {
                    self.skip_nl_in();
                    if self.eat_op("}") {
                        break;
                    }
                    files.push(self.params()?);
                    self.skip_nl_in();
                    self.eat_op(",");
                }
                out.push(S { k: SK::Disk { params, files }, pos });
                return Ok(());
            }
            "modify" => {
                let name = self.ident()?;
                let args = if self.is_op("(") { self.call_args()? } else { vec![] };
                let body = self.brace_block()?;
                out.push(S { k: SK::Modify { name, args, body }, pos });
                return Ok(());
            }
            "filemodify" => {
                let name = self.ident()?;
                let args = if self.is_op("(") { self.call_args()? } else { vec![] };
                SK::FileModify { name, args }
            }
            "plugin" => SK::Plugin(self.expr()?),
            _ => {
                self.p -= 1;
                return self.err(&format!("Unknown directive '.{}'", d));
            }
        };
        out.push(S { k, pos });
        self.end_stmt()
    }

    fn next_nonnl_is(&self, o: &str) -> bool {
        let mut j = self.p;
        while matches!(self.t[j].tok, Tok::Newline) {
            j += 1;
        }
        matches!(&self.t[j].tok, Tok::Op(x) if *x == o)
    }

    /// the remaining statements of the current file (used by .filenamespace); the FileEnd is left in place
    fn rest_of_file(&mut self) -> R<Block> {
        let mut stmts = vec![];
        loop {
            self.skip_nl();
            match self.peek() {
                Tok::FileEnd | Tok::Eof => break,
                Tok::Op("}") => return self.err("Syntax error, unexpected '}'"),
                _ => self.statement(&mut stmts)?,
            }
        }
        Ok(make_block(stmts))
    }

    // ---------------------------------------------------------------- expressions
    pub fn expr(&mut self) -> R<E> {
        self.assign()
    }

    fn mk(k: EK, pos: Pos) -> E {
        E { k, pos }
    }

    fn assign(&mut self) -> R<E> {
        let pos = self.pos();
        let lhs = self.ternary()?;
        for op in ["=", "+=", "-=", "*=", "/="] {
            if self.is_op(op) {
                self.next();
                let rhs = self.assign()?;
                let opst: &'static str = match op {
                    "=" => "=",
                    "+=" => "+",
                    "-=" => "-",
                    "*=" => "*",
                    _ => "/",
                };
                return Ok(Self::mk(EK::Assign { op: opst, target: Box::new(lhs), val: Box::new(rhs) }, pos));
            }
        }
        Ok(lhs)
    }

    fn ternary(&mut self) -> R<E> {
        let pos = self.pos();
        let c = self.or()?;
        if self.eat_op("?") {
            let a = self.ternary()?;
            self.expect_op(":")?;
            let b = self.ternary()?;
            return Ok(Self::mk(EK::Tern(Box::new(c), Box::new(a), Box::new(b)), pos));
        }
        Ok(c)
    }

    fn binlevel(&mut self, ops: &[&'static str], next: fn(&mut Self) -> R<E>) -> R<E> {
        let mut l = next(self)?;
        'outer: loop {
            for op in ops {
                if self.is_op(op) {
                    let pos = self.pos();
                    self.next();
                    let r = next(self)?;
                    l = Self::mk(EK::Bin(op, Box::new(l), Box::new(r)), pos);
                    continue 'outer;
                }
            }
            break;
        }
        Ok(l)
    }
    fn or(&mut self) -> R<E> {
        self.binlevel(&["||"], Self::and)
    }
    fn and(&mut self) -> R<E> {
        self.binlevel(&["&&"], Self::bitor)
    }
    fn bitor(&mut self) -> R<E> {
        self.binlevel(&["|"], Self::bitxor)
    }
    fn bitxor(&mut self) -> R<E> {
        self.binlevel(&["^"], Self::bitand)
    }
    fn bitand(&mut self) -> R<E> {
        self.binlevel(&["&"], Self::equality)
    }
    fn equality(&mut self) -> R<E> {
        self.binlevel(&["==", "!="], Self::relational)
    }
    fn relational(&mut self) -> R<E> {
        self.binlevel(&["<=", ">=", "<", ">"], Self::shift)
    }
    fn shift(&mut self) -> R<E> {
        self.binlevel(&["<<", ">>"], Self::additive)
    }
    fn additive(&mut self) -> R<E> {
        self.binlevel(&["+", "-"], Self::mult)
    }
    fn mult(&mut self) -> R<E> {
        self.binlevel(&["*", "/"], Self::unary)
    }

    fn unary(&mut self) -> R<E> {
        let pos = self.pos();
        match self.peek().clone() {
            Tok::Op("-") => {
                self.next();
                let e = self.unary()?;
                Ok(Self::mk(EK::Un("-", Box::new(e)), pos))
            }
            Tok::Op("+") => {
                self.next();
                self.unary()
            }
            Tok::Op("!") => {
                self.next();
                let e = self.unary()?;
                Ok(Self::mk(EK::Un("!", Box::new(e)), pos))
            }
            Tok::Op("~") => {
                self.next();
                let e = self.unary()?;
                Ok(Self::mk(EK::Un("~", Box::new(e)), pos))
            }
            Tok::Op("<") | Tok::Op(">") => {
                // low/high byte: applies to the whole following (arithmetic) expression
                let o = if self.is_op("<") { "<" } else { ">" };
                self.next();
                let e = self.shift()?;
                Ok(Self::mk(EK::Un(o, Box::new(e)), pos))
            }
            Tok::Op("++") | Tok::Op("--") => {
                let d = if self.is_op("++") { 1.0 } else { -1.0 };
                self.next();
                let e = self.unary()?;
                Ok(Self::mk(EK::Pre { target: Box::new(e), delta: d }, pos))
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> R<E> {
        let mut e = self.primary()?;
        loop {
            let pos = self.pos();
            let adj = !self.tok_at(0).sp;
            if self.is_op(".") && adj {
                if let Tok::Ident(n) = self.peek_at(1).clone() {
                    self.p += 2;
                    if self.is_op("(") && !self.tok_at(0).sp {
                        let args = self.call_args()?;
                        e = Self::mk(EK::Method { obj: Box::new(e), name: n, args }, pos);
                    } else {
                        e = Self::mk(EK::Field { obj: Box::new(e), name: n }, pos);
                    }
                    continue;
                }
                break;
            }
            if self.is_op("[") && adj {
                self.next();
                let idx = self.expr()?;
                self.expect_op("]")?;
                e = Self::mk(EK::Index { obj: Box::new(e), idx: Box::new(idx) }, pos);
                continue;
            }
            if self.is_op("++") || self.is_op("--") {
                let d = if self.is_op("++") { 1.0 } else { -1.0 };
                self.next();
                e = Self::mk(EK::Post { target: Box::new(e), delta: d }, pos);
                continue;
            }
            break;
        }
        Ok(e)
    }

    fn primary(&mut self) -> R<E> {
        let pos = self.pos();
        match self.next() {
            Tok::Num(n) => Ok(Self::mk(EK::Num(n), pos)),
            Tok::Str(s) => Ok(Self::mk(EK::Str(s.into()), pos)),
            Tok::Char(c) => Ok(Self::mk(EK::Char(c), pos)),
            Tok::MultiRef(n, d) => Ok(Self::mk(EK::Multi(n, d), pos)),
            Tok::Op("*") => Ok(Self::mk(EK::Pc, pos)),
            Tok::Op("(") => {
                self.skip_nl_in();
                let e = self.expr()?;
                self.skip_nl_in();
                self.expect_op(")")?;
                Ok(e)
            }
            Tok::Op("[") => {
                self.skip_nl_in();
                let e = self.expr()?;
                self.skip_nl_in();
                if self.eat_op(",") {
                    let mut v = vec![e];
                    loop {
                        self.skip_nl_in();
                        v.push(self.expr()?);
                        self.skip_nl_in();
                        if !self.eat_op(",") {
                            break;
                        }
                    }
                    self.expect_op("]")?;
                    return Ok(Self::mk(EK::List(v), pos));
                }
                self.expect_op("]")?;
                Ok(e)
            }
            Tok::Op("@") => {
                let n = self.ident()?;
                if self.is_op("(") {
                    let args = self.call_args()?;
                    return Ok(Self::mk(EK::Call { name: n, root: true, args }, pos));
                }
                Ok(Self::mk(EK::Id { name: n, root: true }, pos))
            }
            Tok::Ident(n) => match n.as_str() {
                "true" => Ok(Self::mk(EK::Bool(true), pos)),
                "false" => Ok(Self::mk(EK::Bool(false), pos)),
                "null" => Ok(Self::mk(EK::Null, pos)),
                "var" => {
                    let root = self.eat_op("@");
                    let name = self.ident()?;
                    let init = if self.eat_op("=") { Some(Box::new(self.ternary()?)) } else { None };
                    Ok(Self::mk(EK::Decl { name, root, init }, pos))
                }
                _ => {
                    if self.is_op("(") {
                        let args = self.call_args()?;
                        return Ok(Self::mk(EK::Call { name: n, root: false, args }, pos));
                    }
                    Ok(Self::mk(EK::Id { name: n, root: false }, pos))
                }
            },
            _ => {
                self.p -= 1;
                self.err("Syntax error")
            }
        }
    }
}

pub fn make_block(stmts: Vec<S>) -> Block {
    let mut labels = HashSet::new();
    for s in &stmts {
        match &s.k {
            SK::Label { name, multi: false, root: false } => {
                labels.insert(name.clone());
            }
            SK::LabelDef { name, root: false, .. } => {
                labels.insert(name.clone());
            }
            SK::Instr { arg: Arg { label: Some(l), .. }, .. } => {
                labels.insert(l.clone());
            }
            SK::File(_, b) => {
                // imported files share the scope they are imported into
                for l in &b.labels {
                    labels.insert(l.clone());
                }
            }
            SK::Zp(b) | SK::Pseudopc(_, b) => {
                for l in &b.labels {
                    labels.insert(l.clone());
                }
            }
            _ => {}
        }
    }
    Rc::new(BlockData { stmts, labels })
}
