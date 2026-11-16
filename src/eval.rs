//! The evaluator state: passes, scopes, labels and memory.

use crate::ast::*;
use crate::enc::Encodings;
use crate::preproc::Sources;
use crate::tables;
use crate::value::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct AsmError {
    pub pos: Option<Pos>,
    pub msg: String,
    pub called: Vec<Pos>,
}

pub type R<T> = Result<T, AsmError>;

pub fn err<T>(pos: Pos, msg: impl Into<String>) -> R<T> {
    Err(AsmError { pos: Some(pos), msg: msg.into(), called: vec![] })
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SymKind {
    Var,
    Const,
    Label,
}

#[derive(Clone, Debug)]
pub struct Sym {
    pub val: Val,
    pub kind: SymKind,
}

#[derive(Clone, Debug, Default)]
pub struct Scope {
    pub parent: Option<u32>,
    pub syms: HashMap<String, Sym>,
    pub order: Vec<String>,
    pub declared: Option<Block>,
    /// iteration scopes of a labelled loop
    pub iter: Vec<u32>,
    /// true when this scope belongs to a label (so it goes into the symbol file)
    pub labelled: bool,
    pub ns_name: Option<String>,
    /// the namespace the scope belongs to
    pub ns: usize,
    /// not shown as a { } block in the symbol file (.lohifill tables)
    pub no_sym: bool,
    /// the body of a macro, function or pseudocommand call: its labels stay out of the symbol file
    pub call_scope: bool,
}

#[derive(Clone, Debug)]
pub struct DumpEntry {
    pub off: usize,
    pub len: usize,
    pub pos: Pos,
    pub labels: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Blk {
    pub start: i64,
    pub name: String,
    pub virt: bool,
    pub named: bool,
    pub bytes: Vec<u8>,
    pub dump: Vec<DumpEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct SegParams {
    pub start: Option<i64>,
    pub start_after: Option<String>,
    pub align: Option<i64>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub fill: bool,
    pub fill_byte: u8,
    pub virt: bool,
    pub allow_overlap: bool,
    pub hide: bool,
    pub out_prg: Option<String>,
    pub out_bin: Option<String>,
    pub segments: Vec<String>,
    pub prg_files: Vec<String>,
    pub sid_files: Vec<String>,
    pub modify: Option<String>,
    pub margs: HashMap<String, Val>,
    pub dest: Option<String>,
    pub name: Option<String>,
    pub ftype: Option<String>,
    pub mbfiles: bool,
    pub format_pos: Pos,
    pub other: HashMap<String, Val>,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub struct Seg {
    pub name: String,
    pub p: SegParams,
    pub blocks: Vec<Blk>,
    pub cur: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Namespace {
    pub name: String,
    pub parent: Option<usize>,
    pub funcs: HashMap<(String, usize), Rc<FuncDef>>,
    pub macros: HashMap<String, Vec<Rc<FuncDef>>>,
    pub pseudos: HashMap<String, Rc<FuncDef>>,
    pub structs: HashMap<String, Rc<StructDef>>,
}

#[derive(Clone)]
#[allow(dead_code)] // the full option set is kept with the run
pub struct Config {
    pub defines: Vec<String>,
    pub libdirs: Vec<PathBuf>,
    pub cmd_vars: Vec<(String, String)>,
    pub fill_byte: u8,
    pub max_addr: i64,
    pub allow_overlap: bool,
    pub afo: bool,
    pub warnings: bool,
    pub cpu: String,
    pub src_dir: PathBuf,
    pub out_dir: PathBuf,
    pub src_name: String,
    pub no_output: bool,
    pub binfile: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            defines: vec![],
            libdirs: vec![],
            cmd_vars: vec![],
            fill_byte: 0,
            max_addr: 65535,
            allow_overlap: false,
            afo: false,
            warnings: true,
            cpu: "_6502".into(),
            src_dir: PathBuf::from("."),
            out_dir: PathBuf::from("."),
            src_name: "".into(),
            no_output: false,
            binfile: false,
        }
    }
}

pub struct Cpu {
    pub ops: HashMap<String, [i16; 15]>,
}

pub fn build_cpu(name: &str) -> Option<Cpu> {
    let mut ops: HashMap<String, [i16; 15]> = HashMap::new();
    let mut add = |t: &[(&str, [i16; 15])]| {
        for (n, o) in t {
            for nm in n.split(',') {
                ops.insert(nm.to_string(), *o);
            }
        }
    };
    match name {
        "_6502NoIllegals" => add(tables::STD),
        "_6502" => {
            add(tables::STD);
            add(tables::ILL)
        }
        "dtv" => {
            add(tables::STD);
            add(tables::ILL);
            add(tables::DTV)
        }
        "_65c02" => {
            add(tables::STD);
            add(tables::C02)
        }
        _ => return None,
    }
    Some(Cpu { ops })
}

pub fn all_mnemonics() -> std::collections::HashSet<String> {
    let mut s = std::collections::HashSet::new();
    for t in [tables::STD, tables::ILL, tables::DTV, tables::C02] {
        for (n, _) in t {
            for nm in n.split(',') {
                s.insert(nm.to_string());
            }
        }
    }
    s
}

/// bytes of the argument for each addressing mode id (0..=14)
pub fn mode_size(t: i32) -> usize {
    match t {
        0 => 0,
        1..=6 => 1,
        7..=10 => 2,
        11 | 12 => 1,
        _ => 2,
    }
}

pub struct Warning {
    pub pos: Pos,
    pub msg: String,
}
pub struct Breakpoint {
    pub addr: i64,
    pub arg: Option<String>,
}
pub struct Watchpoint {
    pub a1: i64,
    pub a2: Option<i64>,
    pub arg: Option<String>,
}
pub struct DiskReq {
    pub p: SegParams,
    pub files: Vec<SegParams>,
}

pub enum Flow {
    Normal,
    Return(Val),
}

pub struct Asm {
    pub cfg: Config,
    pub src: Sources,
    pub enc: Encodings,
    pub cpus: HashMap<String, Rc<Cpu>>,
    pub ns: Vec<Namespace>,
    pub ns_by_path: HashMap<String, usize>,
    pub consts: HashMap<String, Val>,
    pub program: Option<Block>,

    // per pass
    pub pass: u32,
    pub lenient: bool,
    pub scopes: Vec<Scope>,
    pub prev_scopes: Vec<Scope>,
    pub cur_scope: u32,
    pub cur_ns: usize,
    pub ns_scope: HashMap<usize, u32>,
    pub segs: Vec<Seg>,
    pub prev_segs: HashMap<String, Seg>,
    pub cur_seg: usize,
    pub encoding: usize,
    pub cpu: Rc<Cpu>,
    pub used_unk: bool,
    pub used_fwd: bool,
    pub first_unk: Option<(Pos, String)>,
    pub unks: Vec<(Pos, String)>,
    pub multi: HashMap<String, Vec<f64>>,
    pub prev_multi: HashMap<String, Vec<f64>>,
    pub zp_mode: u32,
    pub pc_delta: i64,
    pub pc_flags: u8,
    pub fn_depth: u32,
    pub call_depth: u32,
    pub pending_label: Option<(u32, String)>,
    pub cur_file: usize,
    pub prints: Vec<String>,
    pub asserts: (u32, u32),
    pub warnings: Vec<Warning>,
    pub breaks: Vec<Breakpoint>,
    pub watches: Vec<Watchpoint>,
    pub file_reqs: Vec<SegParams>,
    pub disk_reqs: Vec<DiskReq>,
    pub out_files: Vec<Rc<OutFileV>>,
    pub log: Box<dyn FnMut(&str)>,
    pub rng: Cell<u64>,
    /// labels waiting for the next bytes (for the byte dump); value None = at the pc
    pub dump_labels: Vec<(String, Option<i64>)>,
    /// code labels with their segment and address, for the debug dump
    pub placed_labels: Vec<(String, i64, String, Pos)>,
    /// how many .segmentout directives this pass has run
    pub segouts: u32,
    /// fields that a known scope didn't have in the first pass, with the type name of the owner
    pub member_unks: Vec<(Pos, &'static str, u32, String)>,
    /// prgFiles and sidFiles that the first pass couldn't find
    pub missing_files: Vec<(Pos, String)>,
    /// labels in the order they were placed (ScopeManager.addResolvedSymbol order)
    /// kept over all passes: a label is added in the first pass where its value is known
    pub label_order: Vec<(u32, String)>,
    pub label_seen: std::collections::HashSet<(u32, String)>,
    /// the namespace each macro/function/pseudocommand was written in
    pub fn_home: HashMap<usize, usize>,
    /// the error was found before the passes in the original (gets a blank line first)
    pub early_error: bool,
    /// the names of the program, from the check before the first pass
    pub declared: crate::names::Declared,
    /// inside `.asserterror`, where an unknown name is what is expected
    pub expect_error: u32,
    first_unk_in_namespace: bool,
    /// the names read from the previous pass in this pass
    fwd_reads: std::collections::HashSet<(u32, String)>,
    /// something else than a name was read from the previous pass
    pub fwd_other: bool,
    /// a branch was too far with a value of the previous pass
    pub range_deferred: bool,
    /// such a branch is an error in this pass
    pub range_strict: bool,
}

pub const MAX_PASSES: u32 = 12;
/// R65's own limit on nested calls. The original has none: it runs out of Java stack.
pub const MAX_CALL_DEPTH: u32 = 10_000;
/// where code goes when no *= was given
pub const DEFAULT_BLOCK_START: i64 = 8192;
/// the BASIC program area of the C64
pub const BASIC_START: i64 = 2049;
/// the empty "Basic End" block opened by BasicUpstart2
pub const BASIC_END_BLOCK: i64 = 2062;

impl Asm {
    pub fn new(cfg: Config, log: Box<dyn FnMut(&str)>) -> Self {
        let mut cpus = HashMap::new();
        for n in ["_6502NoIllegals", "_6502", "dtv", "_65c02"] {
            cpus.insert(n.to_string(), Rc::new(build_cpu(n).unwrap()));
        }
        let cpu = cpus.get(&cfg.cpu).cloned().unwrap_or_else(|| cpus["_6502"].clone());
        let mut a = Asm {
            cfg,
            src: Sources::default(),
            enc: Encodings::new(),
            cpus,
            ns: vec![Namespace::default()],
            ns_by_path: HashMap::new(),
            consts: HashMap::new(),
            program: None,
            pass: 0,
            lenient: false,
            scopes: vec![],
            prev_scopes: vec![],
            cur_scope: 0,
            cur_ns: 0,
            ns_scope: HashMap::new(),
            segs: vec![],
            prev_segs: HashMap::new(),
            cur_seg: 0,
            encoding: 3,
            cpu,
            used_unk: false,
            used_fwd: false,
            first_unk: None,
            unks: vec![],
            multi: HashMap::new(),
            prev_multi: HashMap::new(),
            zp_mode: 0,
            pc_delta: 0,
            pc_flags: 0,
            fn_depth: 0,
            call_depth: 0,
            pending_label: None,
            cur_file: 0,
            prints: vec![],
            asserts: (0, 0),
            warnings: vec![],
            breaks: vec![],
            watches: vec![],
            file_reqs: vec![],
            disk_reqs: vec![],
            out_files: vec![],
            log,
            rng: Cell::new(0x2545F4914F6CDD1D),
            dump_labels: vec![],
            placed_labels: vec![],
            segouts: 0,
            member_unks: vec![],
            missing_files: vec![],
            label_order: vec![],
            label_seen: Default::default(),
            fn_home: HashMap::new(),
            early_error: false,
            declared: Default::default(),
            expect_error: 0,
            first_unk_in_namespace: false,
            fwd_reads: Default::default(),
            fwd_other: false,
            range_deferred: false,
            range_strict: false,
        };
        a.init_consts();
        a.ns_by_path.insert("".into(), 0);
        a
    }

    fn init_consts(&mut self) {
        let colors = [
            ("BLACK", 0),
            ("WHITE", 1),
            ("RED", 2),
            ("CYAN", 3),
            ("PURPLE", 4),
            ("GREEN", 5),
            ("BLUE", 6),
            ("YELLOW", 7),
            ("ORANGE", 8),
            ("BROWN", 9),
            ("LIGHT_RED", 10),
            ("DARK_GRAY", 11),
            ("DARK_GREY", 11),
            ("GRAY", 12),
            ("GREY", 12),
            ("LIGHT_GREEN", 13),
            ("LIGHT_BLUE", 14),
            ("LIGHT_GRAY", 15),
            ("LIGHT_GREY", 15),
        ];
        for (n, v) in colors {
            self.consts.insert(n.into(), num(v as f64));
        }
        self.consts.insert("PI".into(), num(std::f64::consts::PI));
        self.consts.insert("E".into(), num(std::f64::consts::E));
        for (n, v) in [
            ("AT_ABSOLUTE", AT_ABSOLUTE),
            ("AT_ZEROPAGE", AT_ZEROPAGE),
            ("AT_ABSOLUTEX", AT_ABSOLUTEX),
            ("AT_ABSOLUTEY", AT_ABSOLUTEY),
            ("AT_IMMEDIATE", AT_IMMEDIATE),
            ("AT_INDIRECT", AT_INDIRECT),
            ("AT_IZEROPAGEX", AT_IZEROPAGEX),
            ("AT_IZEROPAGEY", AT_IZEROPAGEY),
            ("AT_NONE", AT_NONE),
        ] {
            self.consts.insert(n.into(), num(v as f64));
        }
        for (n, v) in [
            ("BF_C64FILE", "C64FILE"),
            ("BF_KOALA", "C64FILE, Bitmap=$0000, ScreenRam=$1f40, ColorRam=$2328, BackgroundColor = $2710"),
            ("BF_FLI", "C64FILE, ColorRam=$0000, ScreenRam=$0400, Bitmap=$2400"),
            ("BF_BITMAP_SINGLECOLOR", "C64FILE, Bitmap=$0000, ScreenRam=$2000"),
            ("BF_DOODLE", "C64FILE, ColorRam=$0000,Bitmap=$0400"),
        ] {
            self.consts.insert(n.into(), s(v));
        }
        // opcode constants of the default instruction set, e.g. LDA_IMM
        let suffix = ["NONE", "IMM", "ZP", "ZPX", "ZPY", "IZPX", "IZPY", "ABS", "ABSX", "ABSY", "IND", "REL"];
        let cpu = build_cpu("_6502").unwrap();
        for (n, ops) in &cpu.ops {
            for (i, o) in ops.iter().enumerate().take(12) {
                if *o >= 0 {
                    let name = if i == 0 { n.to_uppercase() } else { format!("{}_{}", n.to_uppercase(), suffix[i]) };
                    self.consts.insert(name, num(*o as f64));
                }
            }
        }
        let h = Rc::new(HashV { v: RefCell::new(vec![]), locked: Cell::new(true) });
        for (k, v) in &self.cfg.cmd_vars {
            h.v.borrow_mut().push((k.clone(), s(k), s(v)));
        }
        self.consts.insert("cmdLineVars".into(), Val::Hash(h));
    }

    pub fn opcode_size(&self, op: i64) -> i64 {
        let cpu = &self.cpus["_6502"];
        for ops in cpu.ops.values() {
            for (i, o) in ops.iter().enumerate() {
                if *o as i64 == op {
                    return 1 + mode_size(i as i32) as i64;
                }
            }
        }
        -1
    }

    pub fn say(&mut self, s: &str) {
        (self.log)(s);
    }

    // ------------------------------------------------------------ hoisting
    pub fn hoist(&mut self, b: &Block, ns: usize) -> R<()> {
        for st in &b.stmts {
            match &st.k {
                SK::Function(f) | SK::Macro(f) | SK::Pseudo(f) => self.register(&st.k, f.clone(), ns, true)?,
                SK::Struct(d, root) => {
                    let n = if *root { 0 } else { ns };
                    self.ns[n].structs.insert(d.name.clone(), d.clone());
                }
                SK::Namespace(name, body) => {
                    let child = self.ns_child(ns, name);
                    self.hoist(body, child)?;
                }
                SK::File(_, body) | SK::Scope(body) | SK::Zp(body) | SK::Pseudopc(_, body) => self.hoist(body, ns)?,
                _ => {}
            }
        }
        Ok(())
    }

    pub fn ns_child(&mut self, parent: usize, name: &str) -> usize {
        let path = if self.ns[parent].name.is_empty() { name.to_string() } else { format!("{}.{}", self.ns[parent].name, name) };
        if let Some(i) = self.ns_by_path.get(&path) {
            return *i;
        }
        self.ns.push(Namespace { name: path.clone(), parent: Some(parent), ..Default::default() });
        let id = self.ns.len() - 1;
        self.ns_by_path.insert(path, id);
        id
    }

    pub fn register(&mut self, k: &SK, f: Rc<FuncDef>, ns: usize, hoisting: bool) -> R<()> {
        let n = if f.root { 0 } else { ns };
        self.fn_home.insert(Rc::as_ptr(&f) as usize, ns);
        let nsr = &mut self.ns[n];
        match k {
            SK::Function(_) => {
                let key = (f.name.clone(), f.params.len());
                if let Some(old) = nsr.funcs.get(&key) {
                    if !Rc::ptr_eq(old, &f) && hoisting {
                        return err(f.pos, format!("The function '{}' is already defined.", f.name));
                    }
                }
                nsr.funcs.insert(key, f);
            }
            SK::Macro(_) => {
                let list = nsr.macros.entry(f.name.clone()).or_default();
                if let Some(i) = list.iter().position(|m| m.params.len() == f.params.len()) {
                    if !Rc::ptr_eq(&list[i], &f) && hoisting {
                        return err(f.pos, format!("The macro '{}'  is already defined", f.name));
                    }
                    list[i] = f;
                } else {
                    list.push(f);
                }
            }
            _ => {
                if let Some(old) = nsr.pseudos.get(&f.name) {
                    if !Rc::ptr_eq(old, &f) && hoisting {
                        return err(f.pos, format!("The pseudo command '{}'  is already defined", f.name));
                    }
                }
                nsr.pseudos.insert(f.name.clone(), f);
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------ passes
    fn reset_pass(&mut self) {
        self.scopes.clear();
        self.scopes.push(Scope { declared: self.program.clone(), ..Default::default() });
        self.cur_scope = 0;
        self.cur_ns = 0;
        self.ns_scope.clear();
        self.ns_scope.insert(0, 0);
        self.segs.clear();
        self.segs.push(Seg { name: "Default".into(), p: SegParams::default(), blocks: vec![], cur: None });
        self.cur_seg = 0;
        self.encoding = 3;
        self.cpu = self.cpus.get(&self.cfg.cpu).cloned().unwrap_or_else(|| self.cpus["_6502"].clone());
        self.used_unk = false;
        self.used_fwd = false;
        self.first_unk = None;
        self.first_unk_in_namespace = false;
        self.fwd_reads.clear();
        self.fwd_other = false;
        self.range_deferred = false;
        self.unks.clear();
        self.multi.clear();
        self.zp_mode = 0;
        self.pc_delta = 0;
        self.pc_flags = 0;
        self.fn_depth = 0;
        self.call_depth = 0;
        self.pending_label = None;
        self.prints.clear();
        self.asserts = (0, 0);
        self.warnings.clear();
        self.breaks.clear();
        self.watches.clear();
        self.file_reqs.clear();
        self.disk_reqs.clear();
        self.out_files.clear();
        self.dump_labels.clear();
        self.placed_labels.clear();
        self.segouts = 0;
        self.rng.set(0x2545F4914F6CDD1D);
    }

    fn signature(&self) -> Vec<(u32, String, u64)> {
        let mut v = vec![];
        for (i, sc) in self.scopes.iter().enumerate() {
            for n in &sc.order {
                if let Some(sy) = sc.syms.get(n) {
                    if sy.kind == SymKind::Label {
                        if let Some(f) = sy.val.as_f64() {
                            v.push((i as u32, n.clone(), f.to_bits()));
                        }
                    }
                }
            }
        }
        // the length doesn't count: a shorter instruction moves no label when nothing follows it
        for sg in &self.segs {
            for b in &sg.blocks {
                v.push((u32::MAX, sg.name.clone(), b.start as u64));
            }
        }
        let mut keys: Vec<&String> = self.multi.keys().collect();
        keys.sort();
        for k in keys {
            for x in &self.multi[k] {
                v.push((u32::MAX - 1, k.clone(), x.to_bits()));
            }
        }
        v
    }

    /// Runs all passes. Returns the number of passes on success.
    pub fn run(&mut self, program: Block) -> R<u32> {
        self.program = Some(program.clone());
        self.hoist(&program, 0)?;
        let mut prev_sig: Option<Vec<(u32, String, u64)>> = None;
        let mut last_err: Option<AsmError> = None;
        self.pass = 0;
        loop {
            self.pass += 1;
            self.reset_pass();
            if self.pass > 1 && !self.range_strict {
                self.say(&format!("flex pass {}", self.pass));
            }
            let r = self.exec_block_here(&program).map(|_| ());
            if let Err(e) = r {
                // a call of something that doesn't exist is found before the first pass
                let undefined = e.msg.ends_with(" not defined") && (e.msg.starts_with("Macro command") || e.msg.starts_with("Pseudo command"));
                let wrong_call = e.msg.starts_with("Invalid number of arguments for") || e.msg.starts_with("Too many arguments for") || (e.msg.starts_with("Unknown function '") && e.called.is_empty());
                if self.pass == 1 && (undefined || wrong_call) {
                    self.early_error = true;
                    return Err(e);
                }
                if self.range_strict {
                    return Err(e);
                }
                if self.pass == 1 {
                    self.say("flex pass 1");
                }
                // type errors and unknown functions don't go away in a later pass
                let hopeless = ["Can't get a", "Unknown function", "Variable '", "Macro command", "Pseudo command", "Syntax error", "Index out of range", "Invalid number", "Too many arguments"]
                    .iter()
                    .any(|p| e.msg.starts_with(p));
                let retry = !hopeless && (self.used_unk || self.used_fwd) && self.pass < MAX_PASSES;
                let same = last_err.as_ref().map(|l| l.msg == e.msg && l.pos == e.pos).unwrap_or(false);
                if !retry || (same && !self.used_unk) || self.pass >= 6 {
                    return Err(e);
                }
                // keep what this pass learned and try again
                let mut merged = std::mem::take(&mut self.scopes);
                if self.prev_scopes.len() > merged.len() {
                    merged.extend(self.prev_scopes.drain(merged.len()..));
                }
                self.prev_scopes = merged;
                self.save_prev_segs();
                self.prev_multi = std::mem::take(&mut self.multi);
                self.lenient = true;
                last_err = Some(e);
                prev_sig = None;
                continue;
            }
            if self.pass == 1 && self.used_unk {
                if let Some((p, n)) = self.undeclared() {
                    self.early_error = true;
                    return err(p, format!("Unknown symbol '{}'", n));
                }
            }
            if self.pass == 1 {
                self.say("flex pass 1");
                if let Some(p) = self.declared.unresolved {
                    self.say("flex pass 2");
                    return err(p, "Value can't be resolved");
                }
                let missing = self.member_unks.iter().find(|(_, _, id, n)| self.scopes[*id as usize].syms.contains_key(n) || !self.declared_anywhere(n));
                if let Some((p, ty, _, n)) = missing {
                    return err(*p, format!("Variable '{}' not present on value of type '{}'", n, ty));
                }
                if let Some((p, f)) = self.missing_files.first() {
                    self.early_error = true;
                    return err(*p, format!("Can't open file '{}'", f));
                }
            }
            let sig = self.signature();
            let stable = prev_sig.as_ref().map(|p| *p == sig).unwrap_or(false) || self.forward_values_held();
            if !self.used_unk && (!self.used_fwd || stable) && !self.range_deferred {
                return Ok(self.pass);
            }
            // the values are final now, so a jump that is too far is a real error
            self.range_strict = !self.used_unk && (!self.used_fwd || stable);
            if self.used_unk && !self.lenient && self.pass >= 2 || self.pass >= MAX_PASSES {
                if let Some((p, n)) = self.first_unk.clone() {
                    return match n.strip_prefix('.') {
                        Some(field) => {
                            let ty = if self.first_unk_in_namespace { "Namespace" } else { "LabelValue" };
                            err(p, format!("Variable '{}' not present on value of type '{}'", field, ty))
                        }
                        None => err(p, format!("Unknown symbol '{}'", n)),
                    };
                }
                if self.pass >= MAX_PASSES {
                    return Err(AsmError { pos: None, msg: "Made no progress and can't solve the program".into(), called: vec![] });
                }
            }
            self.lenient = false;
            prev_sig = Some(sig);
            self.prev_scopes = std::mem::take(&mut self.scopes);
            self.save_prev_segs();
            self.prev_multi = std::mem::take(&mut self.multi);
        }
    }

    /// Did every name read from the previous pass end up with that same value? Then one more
    /// pass would only give the same result.
    fn forward_values_held(&self) -> bool {
        !self.fwd_other
            && self.fwd_reads.iter().all(|(id, name)| {
                let old = self.prev_scopes.get(*id as usize).and_then(|s| s.syms.get(name));
                let new = self.scopes.get(*id as usize).and_then(|s| s.syms.get(name));
                match (old.map(|s| &s.val), new.map(|s| &s.val)) {
                    (Some(Val::Num(a, ..)), Some(Val::Num(b, f, _))) => a == b && f & F_UNK == 0,
                    (Some(Val::Scope(_)), Some(Val::Scope(_))) => true,
                    _ => false,
                }
            })
    }

    /// the first unknown name that isn't declared anywhere in the program
    fn undeclared(&self) -> Option<(Pos, String)> {
        for (p, n) in &self.unks {
            if n == "[]" || n.starts_with('!') {
                continue;
            }
            if !self.declared_anywhere(n) {
                return Some((*p, n.clone()));
            }
        }
        None
    }

    fn declared_anywhere(&self, n: &str) -> bool {
        self.scopes.iter().chain(self.prev_scopes.iter()).any(|s| s.syms.contains_key(n))
            || self.consts.contains_key(n)
            || self.ns.iter().any(|x| x.name == n || x.name.ends_with(&format!(".{}", n)))
    }

    fn save_prev_segs(&mut self) {
        self.prev_segs.clear();
        for sg in &self.segs {
            self.prev_segs.insert(sg.name.clone(), sg.clone());
        }
    }

    /// checks that memory blocks don't overlap (done once, in the output pass)
    pub fn check_overlap(&mut self) -> Vec<AsmError> {
        let mut errors = vec![];
        for i in 0..self.segs.len() {
            let sg = &self.segs[i];
            if sg.p.allow_overlap {
                continue;
            }
            let mut bl: Vec<&Blk> = sg.blocks.iter().filter(|b| !b.virt && !b.bytes.is_empty()).collect();
            bl.sort_by_key(|b| b.start);
            let mut found = vec![];
            for w in bl.windows(2) {
                if w[0].start + w[0].bytes.len() as i64 > w[1].start {
                    // the one with the lower address comes first; with the same address, the later one
                    let (a, b) = if w[0].start == w[1].start { (w[1], w[0]) } else { (w[0], w[1]) };
                    found.push(format!("In segment '{}' the memoryblock {} overlaps {}", sg.name, blk_desc(a), blk_desc(b)));
                }
            }
            for msg in found {
                if self.cfg.allow_overlap {
                    self.warnings.push(Warning { pos: Pos::default(), msg });
                } else {
                    errors.push(AsmError { pos: None, msg, called: vec![] });
                }
            }
        }
        errors
    }

    // ------------------------------------------------------------ scopes
    pub fn new_scope(&mut self, parent: u32, declared: Option<Block>) -> u32 {
        let ns = self.cur_ns;
        self.scopes.push(Scope { parent: Some(parent), declared, ns, ..Default::default() });
        (self.scopes.len() - 1) as u32
    }

    fn scope_parent(&self, id: u32) -> Option<u32> {
        if (id as usize) < self.scopes.len() {
            self.scopes[id as usize].parent
        } else {
            self.prev_scopes.get(id as usize).and_then(|s| s.parent)
        }
    }

    fn lookup_in(&mut self, id: u32, name: &str, pos: Pos) -> Option<Val> {
        let cur = (id as usize) < self.scopes.len();
        if cur {
            if let Some(sy) = self.scopes[id as usize].syms.get(name) {
                return Some(sy.val.clone());
            }
        }
        if let Some(ps) = self.prev_scopes.get(id as usize) {
            if let Some(sy) = ps.syms.get(name) {
                if sy.kind == SymKind::Label || matches!(sy.val, Val::Scope(_)) {
                    self.used_fwd = true;
                    self.fwd_reads.insert((id, name.to_string()));
                    return Some(match sy.val.clone() {
                        Val::Num(x, f, sc) => Val::Num(x, (f | F_FWD) & !F_UNK, sc),
                        o => o,
                    });
                }
            }
        }
        let declared = cur && self.scopes[id as usize].declared.as_ref().map(|b| b.labels.contains(name)).unwrap_or(false);
        if declared {
            return Some(self.unknown(pos, name));
        }
        None
    }

    pub fn unknown(&mut self, pos: Pos, name: &str) -> Val {
        self.used_unk = true;
        if self.first_unk.is_none() {
            self.first_unk = Some((pos, name.to_string()));
        }
        if self.unks.len() < 256 {
            self.unks.push((pos, name.to_string()));
        }
        Val::Num(0.0, F_UNK, 0)
    }

    pub fn lookup(&mut self, name: &str, root: bool, pos: Pos) -> R<Val> {
        let mut id = if root { 0 } else { self.cur_scope };
        loop {
            if let Some(v) = self.lookup_in(id, name, pos) {
                return Ok(v);
            }
            match self.scope_parent(id) {
                Some(p) => id = p,
                None => break,
            }
        }
        if let Some(v) = self.consts.get(name) {
            return Ok(v.clone());
        }
        // The check before the first pass has passed, so only the body of a macro or function
        // can get here, when it runs before the line that declares the name.
        if !self.declared.labels.contains(name) {
            if self.declared.consts.contains(name) {
                return err(pos, "Symbol is not yet initialized");
            }
            if self.declared.vars.contains(name) {
                return Ok(Val::Null);
            }
            if self.expect_error > 0 {
                return err(pos, format!("Unknown symbol '{}'", name));
            }
        }
        if self.pass == 1 || self.lenient {
            return Ok(self.unknown(pos, name));
        }
        err(pos, format!("Unknown symbol '{}'", name))
    }

    /// a symbol inside a given scope (label scope, namespace, macro scope...)
    pub fn scope_field(&mut self, id: u32, name: &str, ty: &'static str, pos: Pos) -> R<Val> {
        let cur = (id as usize) < self.scopes.len();
        if cur {
            // from the outside only labels and namespaces can be reached
            if let Some(sy) = self.scopes[id as usize].syms.get(name) {
                if sy.kind == SymKind::Label || matches!(sy.val, Val::Scope(_)) || self.scopes[id as usize].no_sym {
                    return Ok(sy.val.clone());
                }
            }
        }
        if let Some(ps) = self.prev_scopes.get(id as usize) {
            if let Some(sy) = ps.syms.get(name) {
                if sy.kind == SymKind::Label || ps.no_sym || matches!(sy.val, Val::Scope(_)) {
                    self.used_fwd = true;
                    self.fwd_reads.insert((id, name.to_string()));
                    return Ok(match sy.val.clone() {
                        Val::Num(x, f, sc) => Val::Num(x, (f | F_FWD) & !F_UNK, sc),
                        o => o,
                    });
                }
            }
        }
        if self.pass == 1 && cur {
            self.member_unks.push((pos, ty, id, name.to_string()));
        }
        if self.pass == 1 || self.lenient || !cur {
            if self.first_unk.is_none() && ty == "Namespace" {
                self.first_unk_in_namespace = true;
            }
            return Ok(self.unknown_member(pos, name));
        }
        err(pos, format!("Variable '{}' not present on value of type '{}'", name, ty))
    }

    /// A field of something that isn't known yet: not an unknown symbol, the next pass decides.
    pub fn unknown_member(&mut self, pos: Pos, name: &str) -> Val {
        self.used_unk = true;
        if self.first_unk.is_none() {
            self.first_unk = Some((pos, format!(".{}", name)));
        }
        Val::Num(0.0, F_UNK, 0)
    }

    /// is this name a label in the scopes around the current code?
    pub fn is_label(&self, name: &str, root: bool) -> bool {
        let mut id = if root { 0 } else { self.cur_scope };
        loop {
            if let Some(sy) = self.scopes.get(id as usize).and_then(|s| s.syms.get(name)) {
                return sy.kind == SymKind::Label;
            }
            match self.scope_parent(id) {
                Some(p) => id = p,
                None => return false,
            }
        }
    }

    pub fn define(&mut self, scope: u32, name: &str, val: Val, kind: SymKind, pos: Pos) -> R<()> {
        let sc = &mut self.scopes[scope as usize];
        if sc.syms.contains_key(name) {
            return err(pos, format!("The symbol '{}' is already defined", name));
        } else {
            sc.order.push(name.to_string());
        }
        let known = matches!(val, Val::Num(_, fl, _) if fl & F_UNK == 0);
        sc.syms.insert(name.to_string(), Sym { val, kind });
        if kind == SymKind::Label && known && self.label_seen.insert((scope, name.to_string())) {
            self.label_order.push((scope, name.to_string()));
        }
        Ok(())
    }

    fn find_var_scope(&self, name: &str) -> Option<u32> {
        let mut id = self.cur_scope;
        loop {
            if self.scopes.get(id as usize).map(|s| s.syms.contains_key(name)).unwrap_or(false) {
                return Some(id);
            }
            match self.scopes.get(id as usize).and_then(|s| s.parent) {
                Some(p) => id = p,
                None => return None,
            }
        }
    }

    pub fn assign_var(&mut self, name: &str, root: bool, v: Val, pos: Pos) -> R<()> {
        let sid = if root {
            if self.scopes[0].syms.contains_key(name) { Some(0) } else { None }
        } else {
            self.find_var_scope(name)
        };
        match sid {
            Some(id) => {
                let sy = self.scopes[id as usize].syms.get_mut(name).unwrap();
                if sy.kind != SymKind::Var {
                    return err(pos, format!("Can't assign to '{}' since it isn't a variable", name));
                }
                sy.val = v;
                Ok(())
            }
            None => {
                if self.pass == 1 || self.lenient {
                    self.used_unk = true;
                    return Ok(());
                }
                err(pos, format!("Unknown symbol '{}'", name))
            }
        }
    }

    // ------------------------------------------------------------ memory
    pub fn seg_default_start(&mut self, si: usize) -> (i64, u8) {
        let p = self.segs[si].p.clone();
        if let Some(st) = p.start {
            return (st, 0);
        }
        if let Some(after) = &p.start_after {
            let end = self.prev_segs.get(after).and_then(|sg| sg.blocks.last().map(|b| b.start + b.bytes.len() as i64));
            return match end {
                Some(mut e) => {
                    self.used_fwd = true;
                    self.fwd_other = true;
                    if let Some(a) = p.align {
                        if a > 0 && e % a != 0 {
                            e += a - e % a;
                        }
                    }
                    (e, F_FWD)
                }
                None => {
                    self.used_unk = true;
                    (0, F_UNK)
                }
            };
        }
        (DEFAULT_BLOCK_START, 0)
    }

    pub fn ensure_block(&mut self) {
        let si = self.cur_seg;
        if self.segs[si].cur.is_none() {
            let (st, fl) = self.seg_default_start(si);
            self.pc_flags = fl;
            let name = self.segs[si].name.clone();
            let virt = self.segs[si].p.virt;
            self.segs[si].blocks.push(Blk { start: st, name, virt, named: true, bytes: vec![], dump: vec![] });
            self.segs[si].cur = Some(self.segs[si].blocks.len() - 1);
        }
    }

    pub fn real_pc(&mut self) -> i64 {
        let si = self.cur_seg;
        match self.segs[si].cur {
            Some(b) => {
                let bl = &self.segs[si].blocks[b];
                bl.start + bl.bytes.len() as i64
            }
            None => {
                let (st, fl) = self.seg_default_start(si);
                self.pc_flags |= fl;
                st
            }
        }
    }

    pub fn pc(&mut self) -> i64 {
        self.real_pc() + self.pc_delta
    }

    pub fn pc_val(&mut self) -> Val {
        let p = self.pc();
        Val::Num(p as f64, self.pc_flags, 0)
    }

    pub fn emit(&mut self, bytes: &[u8], pos: Pos) -> R<()> {
        self.ensure_block();
        let si = self.cur_seg;
        let bi = self.segs[si].cur.unwrap();
        let pending = std::mem::take(&mut self.dump_labels);
        let max = self.cfg.max_addr;
        let bl = &mut self.segs[si].blocks[bi];
        let off = bl.bytes.len();
        // like the original, the maximum address option is accepted but not enforced
        let _ = (max, pos);
        let addr = bl.start + off as i64;
        let len = bytes.len() as i64;
        let labels: Vec<String> = pending
            .into_iter()
            .filter_map(|(n, v)| match v {
                None => Some(n),
                Some(v) if v == addr => Some(n),
                Some(v) if v > addr && v < addr + len => Some(format!("{}-{}", n, v - addr)),
                _ => None,
            })
            .collect();
        bl.bytes.extend_from_slice(bytes);
        bl.dump.push(DumpEntry { off, len: bytes.len(), pos, labels });
        Ok(())
    }

    pub fn new_block(&mut self, start: i64, name: Option<String>, virt: bool) {
        let si = self.cur_seg;
        let named = name.is_some();
        let virt = virt || self.segs[si].p.virt;
        self.segs[si].blocks.push(Blk {
            start,
            name: name.unwrap_or_else(|| "Unnamed".into()),
            virt,
            named,
            bytes: vec![],
            dump: vec![],
        });
        self.segs[si].cur = Some(self.segs[si].blocks.len() - 1);
    }

    pub fn random(&self) -> f64 {
        let mut x = self.rng.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng.set(x);
        (x >> 11) as f64 / (1u64 << 53) as f64
    }
}

pub fn blk_desc(b: &Blk) -> String {
    format!("'{}' (${:04x}-${:04x})", b.name, b.start, b.start + b.bytes.len() as i64 - 1)
}

pub fn lock(v: Val) -> Val {
    match &v {
        Val::List(l) => l.locked.set(true),
        Val::Hash(h) => h.locked.set(true),
        _ => {}
    }
    v
}

pub fn values_equal(a: &Val, b: &Val) -> bool {
    match (a, b) {
        (Val::Num(..) | Val::Char(..), Val::Num(..) | Val::Char(..)) => a.as_f64() == b.as_f64(),
        (Val::Bool(x, _), Val::Bool(y, _)) => x == y,
        (Val::Null, Val::Null) => true,
        (Val::Null, _) | (_, Val::Null) => false,
        (Val::List(x), Val::List(y)) => {
            let (x, y) = (x.v.borrow(), y.v.borrow());
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| values_equal(p, q))
        }
        (Val::Vector(x), Val::Vector(y)) => x == y,
        _ => a.to_str() == b.to_str(),
    }
}
