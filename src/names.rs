//! The check of all names before the first pass, like the original's parse step.
//!
//! It also covers code that never runs, such as macros without a call and false `.if` branches.
//! First come names defined twice and definitions in the wrong place, then the references, each
//! group in source order. The rules were observed by running KickAss.jar 5.25.

use crate::ast::*;
use crate::eval::AsmError;
use std::collections::{HashMap, HashSet};

const LIBRARIES: &str = include_str!("asminfo_libraries.txt");

/// What a macro or function body finds when it runs before a declaration.
#[derive(Default)]
pub struct Declared {
    pub labels: HashSet<String>,
    pub vars: HashSet<String>,
    pub consts: HashSet<String>,
    /// the first use of a `.label` that only depends on itself
    pub unresolved: Option<Pos>,
}

pub struct Report {
    pub errors: Vec<AsmError>,
    pub declared: Declared,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Label,
    Var,
    Const,
}

#[derive(Clone, Copy)]
struct Def {
    /// a variable or constant must come before its use; a label needn't
    ordered: bool,
    seq: u32,
}

#[derive(Default)]
struct Scope {
    parent: Option<usize>,
    names: HashMap<String, Def>,
    multis: Vec<(String, u32)>,
    /// the body of a function, macro or pseudocommand runs later than its place in the source
    call_body: bool,
    funcs: HashMap<String, Vec<usize>>,
    macros: HashMap<String, Vec<usize>>,
    pseudos: HashMap<String, usize>,
    structs: HashSet<String>,
}

enum Ref {
    Sym { name: String, root: bool },
    Multi { name: String, delta: i32 },
    Func { name: String, root: bool, argc: usize },
    Macro { name: String, root: bool, argc: usize },
    Pseudo { name: String, root: bool, argc: usize },
}

struct Check<'a> {
    scopes: Vec<Scope>,
    namespaces: HashMap<String, usize>,
    /// duplicates and misplaced definitions
    defs: Vec<AsmError>,
    refs: Vec<(Ref, usize, Pos, u32)>,
    seq: u32,
    declared: Declared,
    /// the `.label` definitions: scope, name and the references of the value
    label_defs: Vec<(usize, String, std::ops::Range<usize>)>,
    is_const: &'a dyn Fn(&str) -> bool,
}

fn error(pos: Pos, msg: String) -> AsmError {
    AsmError { pos: Some(pos), msg, called: vec![] }
}

/// for a built-in function that takes any number of arguments
const ANY_COUNT: usize = usize::MAX;

/// (name -> argument counts) of the built-in functions
fn library_functions() -> HashMap<&'static str, Vec<usize>> {
    let mut map: HashMap<&'static str, Vec<usize>> = HashMap::new();
    for line in LIBRARIES.lines() {
        let f: Vec<&str> = line.trim_end().split(';').collect();
        if f.len() == 4 && f[1] == "function" {
            map.entry(f[2]).or_default().push(f[3].parse().unwrap_or(ANY_COUNT));
        }
    }
    map
}

/// Returns the errors of a program, in the order that the original lists them.
pub fn check(program: &Block, is_const: &dyn Fn(&str) -> bool) -> Report {
    let mut c = Check { scopes: vec![Scope::default()], namespaces: HashMap::new(), defs: vec![], refs: vec![], seq: 0, declared: Declared::default(), label_defs: vec![], is_const };
    c.block(program, 0, 0, "", true);
    c.declared.unresolved = c.unresolved_label();
    let refs = std::mem::take(&mut c.refs);
    let lib = library_functions();
    let mut out = std::mem::take(&mut c.defs);
    for (r, scope, pos, seq) in &refs {
        if let Some(msg) = c.resolve(r, *scope, *seq, &lib) {
            out.push(error(*pos, msg));
        }
    }
    Report { errors: out, declared: c.declared }
}

impl Check<'_> {
    fn tick(&mut self) -> u32 {
        self.seq += 1;
        self.seq
    }

    fn add(&mut self, cur: usize, name: &str, root: bool, pos: Pos, kind: Kind) {
        let seq = self.tick();
        let set = if root { 0 } else { cur };
        let ordered = kind != Kind::Label;
        match kind {
            Kind::Label => &mut self.declared.labels,
            Kind::Var => &mut self.declared.vars,
            Kind::Const => &mut self.declared.consts,
        }
        .insert(name.to_string());
        if self.scopes[set].names.insert(name.to_string(), Def { ordered, seq }).is_some() {
            self.defs.push(error(pos, format!("The symbol '{}{}' is already defined", if root { "@" } else { "" }, name)));
        }
    }

    fn fresh(&mut self, parent: usize, call_body: bool, params: &[String]) -> usize {
        let names = params.iter().map(|p| (p.clone(), Def { ordered: false, seq: 0 })).collect();
        self.scopes.push(Scope { parent: Some(parent), names, call_body, ..Default::default() });
        self.scopes.len() - 1
    }

    fn sub(&mut self, b: &Block, cur: usize, ns: usize, path: &str) {
        let id = self.fresh(cur, false, &[]);
        self.block(b, id, ns, path, false);
    }

    fn refer(&mut self, r: Ref, cur: usize, pos: Pos) {
        let seq = self.tick();
        self.refs.push((r, cur, pos, seq));
    }

    fn exprs(&mut self, list: &[E], cur: usize) {
        for e in list {
            self.expr(e, cur);
        }
    }

    fn opt(&mut self, e: &Option<E>, cur: usize) {
        if let Some(e) = e {
            self.expr(e, cur);
        }
    }

    fn params(&mut self, list: &Params, cur: usize) {
        for (_, e, _) in list {
            self.opt(e, cur);
        }
    }

    fn arg(&mut self, a: &Arg, cur: usize, pos: Pos) {
        if let Some(l) = &a.label {
            self.add(cur, l, false, pos, Kind::Label);
        }
        self.opt(&a.e, cur);
        self.opt(&a.e2, cur);
    }

    fn expr(&mut self, e: &E, cur: usize) {
        match &e.k {
            EK::Id { name, root } => self.refer(Ref::Sym { name: name.clone(), root: *root }, cur, e.pos),
            EK::Multi(name, delta) => self.refer(Ref::Multi { name: name.clone(), delta: *delta }, cur, e.pos),
            EK::Un(_, a) | EK::Field { obj: a, .. } | EK::Post { target: a, .. } | EK::Pre { target: a, .. } => self.expr(a, cur),
            EK::Bin(_, a, b) | EK::Index { obj: a, idx: b } | EK::Assign { target: a, val: b, .. } => {
                self.expr(a, cur);
                self.expr(b, cur);
            }
            EK::Tern(a, b, c) => {
                self.expr(a, cur);
                self.expr(b, cur);
                self.expr(c, cur);
            }
            EK::Call { name, root, args } => {
                self.exprs(args, cur);
                self.refer(Ref::Func { name: name.clone(), root: *root, argc: args.len() }, cur, e.pos);
            }
            EK::Method { obj, args, .. } => {
                self.expr(obj, cur);
                self.exprs(args, cur);
            }
            EK::Decl { name, root, init } => {
                if let Some(i) = init {
                    self.expr(i, cur);
                }
                self.add(cur, name, *root, e.pos, Kind::Var);
            }
            EK::List(items) => self.exprs(items, cur),
            _ => {}
        }
    }

    /// the value expression of `.fill` and `.lohifill`, which can use `i`
    fn fill_value(&mut self, e: &E, cur: usize) {
        let id = self.fresh(cur, false, &["i".to_string()]);
        self.expr(e, id);
    }

    fn define(&mut self, k: &SK, f: &FuncDef, cur: usize, ns: usize, direct: bool, pos: Pos) {
        let kind = match k {
            SK::Function(_) => "function",
            SK::Macro(_) => "macro",
            _ => "pseudo",
        };
        if !direct {
            let place = match kind {
                "function" => "a function here. Define it in the direct scope of the current namespace, not inside constructs like {}, .if, .for, etc.",
                "macro" => "a macro here. Define it in the direct scope of the current namespace, not inside constructs like {}, .if, .for etc.",
                _ => "a pseudo command here. Define it in the direct scope of the current namespace, not inside constructs like {}, .if, .for etc.",
            };
            self.defs.push(error(pos, format!("Can't define {}", place)));
        } else {
            let home = &mut self.scopes[if f.root { 0 } else { ns }];
            let argc = f.params.len();
            let msg = match kind {
                "function" => {
                    let list = home.funcs.entry(f.name.clone()).or_default();
                    let twice = list.contains(&argc);
                    list.push(argc);
                    twice.then(|| format!("The function '{}' is already defined.", f.name))
                }
                "macro" => {
                    let list = home.macros.entry(f.name.clone()).or_default();
                    let twice = list.contains(&argc);
                    list.push(argc);
                    twice.then(|| format!("The macro '{}'  is already defined", f.name))
                }
                _ => home.pseudos.insert(f.name.clone(), argc).map(|_| format!("The pseudo command '{}'  is already defined", f.name)),
            };
            if let Some(m) = msg {
                self.defs.push(error(pos, m));
            }
        }
        let id = self.fresh(cur, true, &f.params);
        self.block(&f.body, id, ns, "", false);
    }

    /// `direct`: this block is directly in a namespace, so macros can be defined here
    fn block(&mut self, b: &Block, cur: usize, ns: usize, path: &str, direct: bool) {
        for st in &b.stmts {
            let pos = st.pos;
            match &st.k {
                SK::Label { name, multi: false, root } => self.add(cur, name, *root, pos, Kind::Label),
                SK::Label { name, multi: true, .. } => {
                    let seq = self.tick();
                    self.scopes[cur].multis.push((name.clone(), seq));
                }
                SK::Instr { arg, .. } => self.arg(arg, cur, pos),
                SK::PseudoCall { name, root, args } => {
                    for a in args {
                        self.arg(a, cur, pos);
                    }
                    self.refer(Ref::Pseudo { name: name.clone(), root: *root, argc: args.len() }, cur, pos);
                }
                SK::MacroCall { name, root, args } => {
                    self.exprs(args, cur);
                    self.refer(Ref::Macro { name: name.clone(), root: *root, argc: args.len() }, cur, pos);
                }
                SK::SetPc { e, name, .. } => {
                    self.expr(e, cur);
                    self.opt(name, cur);
                }
                SK::Data { items, .. } | SK::Text(items) => self.exprs(items, cur),
                SK::Fill { count, val, .. } | SK::LohiFill { count, val } => {
                    self.expr(count, cur);
                    self.fill_value(val, cur);
                }
                SK::Align(e) | SK::Eval(e) | SK::Print(e) | SK::PrintNow(e) | SK::Error(e) | SK::Encoding(e) | SK::Memblock(e) | SK::Plugin(e) => self.expr(e, cur),
                SK::Pseudopc(e, body) => {
                    self.expr(e, cur);
                    self.block(body, cur, ns, path, direct);
                }
                SK::Var { name, root, npos, init } => {
                    self.opt(init, cur);
                    self.add(cur, name, *root, *npos, Kind::Var);
                }
                SK::Const { name, root, npos, e } => {
                    self.expr(e, cur);
                    self.add(cur, name, *root, *npos, Kind::Const);
                }
                SK::LabelDef { name, root, npos, e, body } => {
                    let first = self.refs.len();
                    self.expr(e, cur);
                    self.label_defs.push((if *root { 0 } else { cur }, name.clone(), first..self.refs.len()));
                    self.add(cur, name, *root, *npos, Kind::Label);
                    if let Some(body) = body {
                        self.sub(body, cur, ns, path);
                    }
                }
                SK::Enum(items) => {
                    for (n, e, npos) in items {
                        self.opt(e, cur);
                        self.add(cur, n, false, *npos, Kind::Const);
                    }
                }
                SK::ErrorIf(a, c) => {
                    self.expr(a, cur);
                    self.expr(c, cur);
                }
                SK::Assert(n, a, c) => {
                    self.expr(n, cur);
                    self.expr(a, cur);
                    self.expr(c, cur);
                }
                // what these expect to fail is only known when they run
                SK::AssertErr(n, _) | SK::AssertErrCode(n, _) => self.expr(n, cur),
                SK::AssertCode(n, a, c) => {
                    self.expr(n, cur);
                    self.sub(a, cur, ns, path);
                    self.sub(c, cur, ns, path);
                }
                SK::If(c, t, e) => {
                    self.expr(c, cur);
                    self.sub(t, cur, ns, path);
                    if let Some(e) = e {
                        self.sub(e, cur, ns, path);
                    }
                }
                SK::For { init, cond, step, body } => {
                    let holder = self.fresh(cur, false, &[]);
                    self.exprs(init, holder);
                    self.opt(cond, holder);
                    self.exprs(step, holder);
                    self.sub(body, holder, ns, path);
                }
                SK::While(c, body) => {
                    self.expr(c, cur);
                    self.sub(body, cur, ns, path);
                }
                SK::Scope(body) => self.sub(body, cur, ns, path),
                SK::Return(e) => self.opt(e, cur),
                SK::Function(f) | SK::Macro(f) | SK::Pseudo(f) => self.define(&st.k, f, cur, ns, direct, pos),
                SK::Struct(d, root) => {
                    self.scopes[if *root { 0 } else { ns }].structs.insert(d.name.clone());
                }
                SK::Define(names, body) => {
                    self.sub(body, cur, ns, path);
                    for n in names {
                        self.add(cur, n, false, pos, Kind::Const);
                    }
                }
                SK::Namespace(name, body) => {
                    let child = if path.is_empty() { name.clone() } else { format!("{}.{}", path, name) };
                    let seq = self.tick();
                    self.scopes[cur].names.entry(name.clone()).or_insert(Def { ordered: false, seq });
                    let id = match self.namespaces.get(&child) {
                        Some(i) => *i,
                        None => {
                            let i = self.fresh(cur, false, &[]);
                            self.namespaces.insert(child.clone(), i);
                            i
                        }
                    };
                    self.block(body, id, id, &child, true);
                }
                SK::File(_, body) | SK::Zp(body) => self.block(body, cur, ns, path, direct),
                SK::Import { file, off, len, .. } => {
                    self.expr(file, cur);
                    self.opt(off, cur);
                    self.opt(len, cur);
                }
                SK::Break(e) => self.opt(e, cur),
                SK::Watch(list) => {
                    for e in list {
                        self.opt(e, cur);
                    }
                }
                SK::Segment { params, block, .. } => {
                    if let Some(p) = params {
                        self.params(p, cur);
                    }
                    self.opt(block, cur);
                }
                SK::SegmentDef { params, .. } | SK::SegmentOut(params) | SK::FileOut(params) => self.params(params, cur),
                SK::Disk { params, files } => {
                    self.params(params, cur);
                    for f in files {
                        self.params(f, cur);
                    }
                }
                SK::Modify { args, body, .. } => {
                    self.exprs(args, cur);
                    self.sub(body, cur, ns, path);
                }
                SK::FileModify { args, .. } => self.exprs(args, cur),
                SK::Cpu(_) => {}
            }
        }
    }

    /// the scope that a symbol reference finds its name in
    fn found_in(&self, r: &Ref, cur: usize) -> Option<(usize, String)> {
        let Ref::Sym { name, root } = r else { return None };
        let id = self.chain(cur, *root).into_iter().find(|id| self.scopes[*id].names.contains_key(name))?;
        Some((id, name.clone()))
    }

    /// Finds the first use of a `.label` whose value can never be known, like `.label l = l`.
    fn unresolved_label(&self) -> Option<Pos> {
        let mut open: HashMap<(usize, String), Vec<(usize, String)>> = HashMap::new();
        for (scope, name, range) in &self.label_defs {
            let deps = self.refs[range.clone()].iter().filter_map(|(r, cur, _, _)| self.found_in(r, *cur)).collect();
            open.insert((*scope, name.clone()), deps);
        }
        loop {
            let done: Vec<(usize, String)> = open.iter().filter(|(_, deps)| !deps.iter().any(|d| open.contains_key(d))).map(|(k, _)| k.clone()).collect();
            if done.is_empty() {
                break;
            }
            for k in done {
                open.remove(&k);
            }
        }
        self.refs.iter().find(|(r, cur, _, _)| self.found_in(r, *cur).is_some_and(|k| open.contains_key(&k))).map(|(_, _, pos, _)| *pos)
    }

    /// the scopes from `cur` outward, or only the root scope
    fn chain(&self, cur: usize, root: bool) -> Vec<usize> {
        if root {
            return vec![0];
        }
        let mut v = vec![cur];
        let mut id = cur;
        while let Some(p) = self.scopes[id].parent {
            v.push(p);
            id = p;
        }
        v
    }

    fn resolve(&self, r: &Ref, cur: usize, seq: u32, lib: &HashMap<&str, Vec<usize>>) -> Option<String> {
        match r {
            Ref::Sym { name, root } => {
                let mut crossed = false;
                for id in self.chain(cur, *root) {
                    if let Some(d) = self.scopes[id].names.get(name) {
                        return (d.ordered && !crossed && d.seq > seq).then(|| format!("Reference to not yet defined symbol '{}'", name));
                    }
                    crossed |= self.scopes[id].call_body;
                }
                (!(self.is_const)(name)).then(|| format!("Unknown symbol '{}'", name))
            }
            Ref::Multi { name, delta } => {
                let found = self.chain(cur, false).iter().any(|id| {
                    self.scopes[*id].multis.iter().any(|(n, s)| n == name && if *delta > 0 { *s > seq } else { *s < seq })
                });
                let mark = if *delta > 0 { "+" } else { "-" };
                (!found).then(|| format!("Unknown symbol '!{}{}'", name, mark.repeat(delta.unsigned_abs() as usize)))
            }
            Ref::Func { name, root, argc } => {
                let scopes = self.chain(cur, *root);
                if scopes.iter().any(|id| self.scopes[*id].structs.contains(name)) {
                    return None;
                }
                let mine: Vec<&Vec<usize>> = scopes.iter().filter_map(|id| self.scopes[*id].funcs.get(name)).collect();
                let built_in = if *root { None } else { lib.get(name.as_str()) };
                if mine.iter().any(|l| l.contains(argc)) || built_in.is_some_and(|l| l.contains(argc) || l.contains(&ANY_COUNT)) {
                    return None;
                }
                if mine.is_empty() && built_in.is_none() {
                    return Some(format!("Unknown function '{}'", name));
                }
                Some(format!("Invalid number of arguments for function '{}'", name))
            }
            Ref::Macro { name, root, argc } => {
                let lists: Vec<&Vec<usize>> = self.chain(cur, *root).iter().filter_map(|id| self.scopes[*id].macros.get(name)).collect();
                if lists.is_empty() {
                    return Some(format!("Macro command '{}' not defined", name));
                }
                (!lists.iter().any(|l| l.contains(argc))).then(|| "Invalid number of arguments for macro".to_string())
            }
            Ref::Pseudo { name, root, argc } => {
                let Some(n) = self.chain(cur, *root).iter().find_map(|id| self.scopes[*id].pseudos.get(name)) else {
                    return Some(format!("Pseudo command '{}' not defined", name));
                };
                (argc > n).then(|| "Too many arguments for pseudo command".to_string())
            }
        }
    }
}
