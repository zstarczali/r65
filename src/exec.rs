//! Running statements: directives, instructions, macros and pseudocommands.

use crate::ast::*;
use crate::enc::Encodings;
use crate::eval::*;
use crate::value::*;
use std::rc::Rc;

impl Asm {
    pub fn exec_block_here(&mut self, b: &Block) -> R<Flow> {
        for st in &b.stmts {
            let is_label = matches!(st.k, SK::Label { .. });
            let f = self.exec(st)?;
            if !is_label {
                self.pending_label = None;
            }
            if let Flow::Return(v) = f {
                return Ok(Flow::Return(v));
            }
        }
        Ok(Flow::Normal)
    }

    /// runs a block in a new child scope
    pub fn exec_scoped(&mut self, b: &Block) -> R<(Flow, u32)> {
        let attach = self.pending_label.take();
        let id = self.new_scope(self.cur_scope, Some(b.clone()));
        if let Some((sc, name)) = attach {
            self.attach_scope(sc, &name, id);
        }
        let saved = self.cur_scope;
        self.cur_scope = id;
        let r = self.exec_block_here(b);
        self.cur_scope = saved;
        Ok((r?, id))
    }

    pub fn attach_scope(&mut self, sc: u32, name: &str, id: u32) {
        self.scopes[id as usize].labelled = true;
        if let Some(sy) = self.scopes[sc as usize].syms.get_mut(name) {
            if let Val::Num(_, _, s) = &mut sy.val {
                *s = id + 1;
            }
        }
    }

    pub fn eval_num(&mut self, e: &E) -> R<(f64, u8)> {
        let v = self.eval(e)?;
        match v.as_f64() {
            Some(x) => Ok((x, v.flags())),
            None => err(e.pos, format!("Can't get a double representation from a value of type {}", v.type_name())),
        }
    }
    pub fn eval_int(&mut self, e: &E) -> R<(i64, u8)> {
        let (v, f) = self.eval_num(e)?;
        Ok((v as i64, f))
    }
    pub fn eval_str(&mut self, e: &E) -> R<String> {
        Ok(self.eval(e)?.to_str())
    }
    pub fn eval_bool(&mut self, e: &E) -> R<(bool, u8)> {
        match self.eval(e)? {
            Val::Bool(b, f) => Ok((b, f)),
            Val::Num(_, f, _) if f & F_UNK != 0 => Ok((false, f)),
            o => err(e.pos, format!("Can't get a boolean representation from a value of type {}", o.type_name())),
        }
    }

    fn no_fn_mode(&self, pos: Pos) -> R<()> {
        if self.fn_depth > 0 {
            return err(pos, "Can't output bytes inside a function or a .define block");
        }
        Ok(())
    }

    pub fn exec(&mut self, st: &S) -> R<Flow> {
        let pos = st.pos;
        match &st.k {
            SK::Label { name, multi, root } => {
                let pc = self.pc();
                let fl = self.pc_flags & !F_FWD | if self.zp_mode > 0 { F_ZP } else { 0 };
                if *multi {
                    self.multi.entry(name.clone()).or_default().push(pc as f64);
                } else {
                    let sc = if *root { 0 } else { self.cur_scope };
                    self.define(sc, name, Val::Num(pc as f64, fl, 0), SymKind::Label, pos)?;
                    self.pending_label = Some((sc, name.clone()));
                    self.dump_labels.push((name.clone(), None));
                }
            }
            SK::Instr { mn, suffix, arg } => {
                self.no_fn_mode(pos)?;
                if !self.cpu.ops.contains_key(mn) {
                    if self.find_pseudo(mn, false).is_some() {
                        self.call_pseudo(mn, false, std::slice::from_ref(arg), pos)?;
                        return Ok(Flow::Normal);
                    }
                    return err(pos, format!("The mnemonic '{}' is not supported by the current cpu", mn));
                }
                self.instr(mn, suffix.as_deref(), arg, pos)?;
            }
            SK::PseudoCall { name, root, args } => self.call_pseudo(name, *root, args, pos)?,
            SK::MacroCall { name, root, args } => self.call_macro(name, *root, args, pos)?,
            SK::SetPc { e, name, virt } => {
                let (v, f) = self.eval_int(e)?;
                if v < 0 && f == 0 {
                    return err(pos, "Memory address may not be negative!");
                }
                let n = match name {
                    Some(n) => Some(self.eval_str(n)?),
                    None => None,
                };
                self.new_block(v.max(0), n, *virt);
                self.pc_flags = f;
                self.pc_delta = 0;
            }
            SK::Memblock(e) => {
                let n = self.eval_str(e)?;
                let pc = self.real_pc();
                self.new_block(pc, Some(n), false);
            }
            SK::Data { size, items } => {
                self.no_fn_mode(pos)?;
                let mut out = vec![];
                for it in items {
                    let v = self.eval(it)?;
                    self.push_data(&v, *size, &mut out, it.pos)?;
                }
                self.emit(&out, pos)?;
            }
            SK::Text(items) => {
                self.no_fn_mode(pos)?;
                let mut out = vec![];
                // A text built from a value that is not known yet has an unknown length,
                // so the address after it is unknown in this pass (like the original).
                let had_unk = std::mem::replace(&mut self.used_unk, false);
                let mut r = Ok(());
                for it in items {
                    match self.eval(it) {
                        Ok(v) => match &v {
                            Val::Num(..) | Val::Char(..) => out.push(v.as_f64().unwrap() as i64 as u8),
                            Val::Null => {}
                            _ => {
                                for c in v.to_str().chars() {
                                    out.push(self.enc.conv(self.encoding, c));
                                }
                            }
                        },
                        Err(e) => {
                            r = Err(e);
                            break;
                        }
                    }
                }
                if self.used_unk {
                    self.pc_flags |= F_UNK;
                }
                self.used_unk |= had_unk;
                r?;
                self.emit(&out, pos)?;
            }
            SK::Fill { word, count, val } => {
                self.no_fn_mode(pos)?;
                let (n, f) = self.eval_int(count)?;
                if f & F_UNK != 0 {
                    self.pc_flags |= F_UNK;
                }
                let id = self.new_scope(self.cur_scope, None);
                let saved = self.cur_scope;
                self.cur_scope = id;
                let size = if *word { 2 } else { 1 };
                let mut out = Vec::with_capacity(n.max(0) as usize);
                let mut r = Ok(());
                for i in 0..n.max(0) {
                    self.scopes[id as usize].syms.insert("i".into(), Sym { val: num(i as f64), kind: SymKind::Var });
                    let v = match self.eval(val) {
                        Ok(v) => v,
                        Err(e) => {
                            r = Err(e);
                            break;
                        }
                    };
                    let items = match &v {
                        Val::List(l) => l.v.borrow().clone(),
                        _ => vec![v],
                    };
                    for x in &items {
                        if let Err(e) = self.push_data(x, size, &mut out, val.pos) {
                            r = Err(e);
                        }
                    }
                    if r.is_err() {
                        break;
                    }
                }
                self.cur_scope = saved;
                self.scopes.truncate(id as usize);
                r?;
                self.emit(&out, pos)?;
            }
            SK::LohiFill { count, val } => {
                self.no_fn_mode(pos)?;
                let (n, _) = self.eval_int(count)?;
                let start = self.pc();
                let attach = self.pending_label.take();
                let id = self.new_scope(self.cur_scope, None);
                let saved = self.cur_scope;
                self.cur_scope = id;
                let mut vals = vec![];
                let mut r = Ok(());
                for i in 0..n.max(0) {
                    self.scopes[id as usize].syms.insert("i".into(), Sym { val: num(i as f64), kind: SymKind::Var });
                    match self.eval_int(val) {
                        Ok((v, _)) => vals.push(v),
                        Err(e) => {
                            r = Err(e);
                            break;
                        }
                    }
                }
                self.cur_scope = saved;
                r?;
                self.scopes[id as usize].syms.clear();
                self.define(id, "lo", num(start as f64), SymKind::Const, pos)?;
                self.define(id, "hi", num((start + n) as f64), SymKind::Const, pos)?;
                self.scopes[id as usize].no_sym = true;
                if let Some((sc, name)) = attach {
                    self.attach_scope(sc, &name, id);
                }
                let mut out: Vec<u8> = vals.iter().map(|v| (*v & 0xff) as u8).collect();
                out.extend(vals.iter().map(|v| ((*v >> 8) & 0xff) as u8));
                self.emit(&out, pos)?;
            }
            SK::Align(e) => {
                let (n, f) = self.eval_int(e)?;
                if f & F_UNK != 0 {
                    return Ok(Flow::Normal);
                }
                if n <= 0 {
                    return err(pos, "Page size must be positive.");
                }
                let pc = self.pc();
                let mut pad = n - pc.rem_euclid(n);
                if pad == n {
                    pad = 0;
                }
                if pad > 0 {
                    self.emit(&vec![0u8; pad as usize], pos)?;
                }
            }
            SK::Pseudopc(e, b) => {
                let (v, f) = self.eval_int(e)?;
                let real = self.real_pc();
                let saved = (self.pc_delta, self.pc_flags);
                self.pc_delta = v - real;
                self.pc_flags |= f;
                let r = self.exec_block_here(b);
                self.pc_delta = saved.0;
                self.pc_flags = saved.1;
                if let Flow::Return(v) = r? {
                    return Ok(Flow::Return(v));
                }
            }
            SK::Var { name, root, init } => {
                let v = match init {
                    Some(e) => self.eval(e)?,
                    None => Val::Null,
                };
                let sc = if *root { 0 } else { self.cur_scope };
                self.define(sc, name, v, SymKind::Var, pos)?;
            }
            SK::Const { name, root, e } => {
                let v = lock(self.eval(e)?);
                let sc = if *root { 0 } else { self.cur_scope };
                self.define(sc, name, v, SymKind::Const, pos)?;
            }
            SK::LabelDef { name, root, e, body } => {
                let v = match self.eval(e)? {
                    Val::Num(x, f, _) => Val::Num(x, f & !F_FWD, 0),
                    Val::Char(_, x) => num(x),
                    o => o,
                };
                let sc = if *root { 0 } else { self.cur_scope };
                if let Some(x) = v.as_f64() {
                    self.dump_labels.push((name.clone(), Some(x as i64)));
                }
                self.define(sc, name, v, SymKind::Label, pos)?;
                self.pending_label = Some((sc, name.clone()));
                if let Some(b) = body {
                    let (f, _) = self.exec_scoped(b)?;
                    if let Flow::Return(v) = f {
                        return Ok(Flow::Return(v));
                    }
                }
            }
            SK::Eval(e) => {
                self.eval(e)?;
            }
            SK::Enum(items) => {
                // entries without a value count 0, 1, 2...; given values don't move the counter
                let mut next = 0.0;
                for (n, e) in items {
                    let v = match e {
                        Some(e) => self.eval(e)?,
                        None => {
                            next += 1.0;
                            num(next - 1.0)
                        }
                    };
                    let sc = self.cur_scope;
                    self.define(sc, n, v, SymKind::Const, pos)?;
                }
            }
            SK::Print(e) => {
                let t = self.eval_str(e)?;
                self.prints.push(t);
            }
            SK::PrintNow(e) => {
                let t = self.eval_str(e)?;
                for l in t.split('\n') {
                    self.say(&format!("  {}", l));
                }
            }
            SK::Error(e) => {
                let t = self.eval_str(e)?;
                return err(pos, t);
            }
            SK::ErrorIf(c, e) => {
                let (b, f) = self.eval_bool(c)?;
                if b && f & F_UNK == 0 {
                    let t = self.eval_str(e)?;
                    return err(pos, t);
                }
            }
            SK::Assert(d, a, b) => {
                let desc = self.eval_str(d)?;
                let va = self.eval(a)?;
                let vb = self.eval(b)?;
                self.asserts.0 += 1;
                let mut line = format!("{}={} ({})", desc, va.to_str(), vb.to_str());
                if !values_equal(&va, &vb) {
                    self.asserts.1 += 1;
                    line.push_str(" -- ERROR IN ASSERTION!!!");
                }
                self.prints.push(line);
            }
            SK::AssertErr(d, e) => {
                let desc = self.eval_str(d)?;
                let saved = (self.cur_scope, self.cur_ns, self.fn_depth, self.call_depth);
                let r = self.eval(e);
                self.cur_scope = saved.0;
                self.cur_ns = saved.1;
                self.fn_depth = saved.2;
                self.call_depth = saved.3;
                self.asserts.0 += 1;
                match r {
                    Err(er) => self.prints.push(format!("{} -- OK. | {}", desc, er.msg)),
                    Ok(_) => {
                        self.asserts.1 += 1;
                        self.prints.push(format!("{} -- ERROR IN ASSERTION!!! (no error was raised)", desc));
                    }
                }
            }
            SK::If(c, t, e) => {
                let (b, _) = self.eval_bool(c)?;
                let attach = self.pending_label.take();
                match if b { Some(t) } else { e.as_ref() } {
                    Some(bd) => {
                        self.pending_label = attach;
                        let (f, _) = self.exec_scoped(bd)?;
                        if let Flow::Return(v) = f {
                            return Ok(Flow::Return(v));
                        }
                    }
                    None => {
                        if let Some((sc, name)) = attach {
                            let id = self.new_scope(self.cur_scope, None);
                            self.attach_scope(sc, &name, id);
                        }
                    }
                }
            }
            SK::For { init, cond, step, body } => {
                if let Flow::Return(v) = self.loop_stmt(init, cond.as_ref(), step, body)? {
                    return Ok(Flow::Return(v));
                }
            }
            SK::While(c, body) => {
                if let Flow::Return(v) = self.loop_stmt(&[], Some(c), &[], body)? {
                    return Ok(Flow::Return(v));
                }
            }
            SK::Scope(b) => {
                if let (Flow::Return(v), _) = self.exec_scoped(b)? {
                    return Ok(Flow::Return(v));
                }
            }
            SK::Return(e) => {
                let v = match e {
                    Some(e) => self.eval(e)?,
                    None => Val::Null,
                };
                return Ok(Flow::Return(v));
            }
            SK::Function(f) | SK::Macro(f) | SK::Pseudo(f) => {
                let ns = self.cur_ns;
                self.register(&st.k, f.clone(), ns, false)?;
            }
            SK::Struct(d, root) => {
                let n = if *root { 0 } else { self.cur_ns };
                self.ns[n].structs.insert(d.name.clone(), d.clone());
            }
            SK::Define(names, b) => {
                let id = self.new_scope(self.cur_scope, Some(b.clone()));
                let saved = self.cur_scope;
                self.cur_scope = id;
                self.fn_depth += 1;
                let r = self.exec_block_here(b);
                self.fn_depth -= 1;
                self.cur_scope = saved;
                r?;
                for n in names {
                    match self.scopes[id as usize].syms.get(n).map(|s| s.val.clone()) {
                        Some(v) => {
                            let sc = self.cur_scope;
                            self.define(sc, n, lock(v), SymKind::Const, pos)?;
                        }
                        None => return err(pos, format!("Symbol '{}' isn't defined in the .define block", n)),
                    }
                }
            }
            SK::Namespace(name, b) => {
                let child = self.ns_child(self.cur_ns, name);
                let sid = match self.ns_scope.get(&child) {
                    Some(s) => *s,
                    None => {
                        let parent_scope = self.ns_scope.get(&self.cur_ns).cloned().unwrap_or(0);
                        let id = self.new_scope(parent_scope, Some(b.clone()));
                        self.scopes[id as usize].ns_name = Some(name.clone());
                        self.scopes[id as usize].ns = child;
                        self.scopes[id as usize].labelled = true;
                        self.ns_scope.insert(child, id);
                        let ps = &mut self.scopes[parent_scope as usize];
                        if !ps.syms.contains_key(name) {
                            ps.order.push(name.clone());
                            ps.syms.insert(name.clone(), Sym { val: Val::Scope(id), kind: SymKind::Const });
                        }
                        id
                    }
                };
                if self.scopes[sid as usize].declared.as_ref().map(|d| !Rc::ptr_eq(d, b)).unwrap_or(true) {
                    let mut merged = self.scopes[sid as usize].declared.as_ref().map(|d| d.labels.clone()).unwrap_or_default();
                    merged.extend(b.labels.iter().cloned());
                    self.scopes[sid as usize].declared = Some(Rc::new(BlockData { stmts: vec![], labels: merged }));
                }
                let saved = (self.cur_scope, self.cur_ns);
                self.cur_scope = sid;
                self.cur_ns = child;
                let r = self.exec_block_here(b);
                self.cur_scope = saved.0;
                self.cur_ns = saved.1;
                if let Flow::Return(v) = r? {
                    return Ok(Flow::Return(v));
                }
            }
            SK::File(id, b) => {
                let saved = self.cur_file;
                self.cur_file = *id;
                let r = self.exec_block_here(b);
                self.cur_file = saved;
                if let Flow::Return(v) = r? {
                    return Ok(Flow::Return(v));
                }
            }
            SK::Import { kind, file, off, len } => {
                self.no_fn_mode(pos)?;
                let name = self.eval_str(file)?;
                let data = self.read_file(&name, pos)?;
                let mut o = match off {
                    Some(e) => self.eval_int(e)?.0.max(0) as usize,
                    None => 0,
                };
                if *kind == ImportKind::C64 {
                    o += 2;
                }
                let o = o.min(data.len());
                let l = match len {
                    Some(e) => (self.eval_int(e)?.0.max(0) as usize).min(data.len() - o),
                    None => data.len() - o,
                };
                let slice = &data[o..o + l];
                let out: Vec<u8> = if *kind == ImportKind::Text {
                    String::from_utf8_lossy(slice).chars().map(|c| self.enc.conv(self.encoding, c)).collect()
                } else {
                    slice.to_vec()
                };
                self.emit(&out, pos)?;
            }
            SK::Encoding(e) => {
                let n = self.eval_str(e)?;
                match Encodings::index(&n) {
                    Some(i) => self.encoding = i,
                    None => return err(pos, format!("Unknown encoding '{}'", n)),
                }
            }
            SK::Cpu(n) => match self.cpus.get(n) {
                Some(c) => self.cpu = c.clone(),
                None => return err(pos, format!("Unknown cpu '{}'", n)),
            },
            SK::Zp(b) => {
                self.zp_mode += 1;
                let r = self.exec_block_here(b);
                self.zp_mode -= 1;
                if let Flow::Return(v) = r? {
                    return Ok(Flow::Return(v));
                }
            }
            SK::Break(e) => {
                let arg = match e {
                    Some(e) => Some(self.eval_str(e)?),
                    None => None,
                };
                let addr = self.pc();
                self.breaks.push(Breakpoint { addr, arg });
            }
            SK::Watch(v) => {
                let a1 = match v.first().and_then(|x| x.as_ref()) {
                    Some(e) => self.eval_int(e)?.0,
                    None => return err(pos, "Missing watch address"),
                };
                let a2 = match v.get(1).and_then(|x| x.as_ref()) {
                    Some(e) => Some(self.eval_int(e)?.0),
                    None => None,
                };
                let arg = match v.get(2).and_then(|x| x.as_ref()) {
                    Some(e) => Some(self.eval_str(e)?),
                    None => None,
                };
                self.watches.push(Watchpoint { a1, a2, arg });
            }
            SK::SegmentDef { name, params } => {
                if self.segs.iter().any(|s| s.name == *name) {
                    return err(pos, format!("The segment '{}' is already defined", name));
                }
                let p = self.seg_params(params, pos)?;
                self.segs.push(Seg { name: name.clone(), p, blocks: vec![], cur: None });
            }
            SK::Segment { name, params, block } => {
                let idx = match self.segs.iter().position(|s| s.name == *name) {
                    Some(i) => {
                        if params.is_some() {
                            return err(pos, format!("The segment '{}' is already defined", name));
                        }
                        i
                    }
                    None => {
                        let p = match params {
                            Some(p) => self.seg_params(p, pos)?,
                            None => SegParams::default(),
                        };
                        self.segs.push(Seg { name: name.clone(), p, blocks: vec![], cur: None });
                        self.segs.len() - 1
                    }
                };
                self.cur_seg = idx;
                self.pc_delta = 0;
                if let Some(b) = block {
                    let n = self.eval_str(b)?;
                    let pc = self.real_pc();
                    self.new_block(pc, Some(n), false);
                }
            }
            SK::SegmentOut(params) => {
                self.no_fn_mode(pos)?;
                let p = self.seg_params(params, pos)?;
                let blocks = self.intermediate_blocks(&p, pos, true)?;
                let bytes = crate::output::flatten(&blocks, p.fill_byte).1;
                self.emit(&bytes, pos)?;
            }
            SK::FileOut(params) => {
                let p = self.seg_params(params, pos)?;
                if p.name.is_none() {
                    return err(pos, "Missing 'name' parameter in the .file directive");
                }
                self.file_reqs.push(p);
            }
            SK::Disk { params, files } => {
                let p = self.seg_params(params, pos)?;
                let mut fs = vec![];
                for f in files {
                    fs.push(self.seg_params(f, pos)?);
                }
                self.disk_reqs.push(DiskReq { p, files: fs });
            }
            SK::Modify { name, .. } | SK::FileModify { name, .. } => {
                return err(pos, format!("Unknown modifier '{}' (plugins are not supported)", name));
            }
            SK::Plugin(_) => return err(pos, "Java plugins are not supported by this assembler"),
        }
        Ok(Flow::Normal)
    }

    fn loop_stmt(&mut self, init: &[E], cond: Option<&E>, step: &[E], body: &Block) -> R<Flow> {
        let attach = self.pending_label.take();
        let holder = self.new_scope(self.cur_scope, None);
        if let Some((sc, name)) = attach {
            self.attach_scope(sc, &name, holder);
        }
        let saved = self.cur_scope;
        self.cur_scope = holder;
        let r = self.run_loop(holder, init, cond, step, body);
        self.cur_scope = saved;
        r
    }

    fn run_loop(&mut self, holder: u32, init: &[E], cond: Option<&E>, step: &[E], body: &Block) -> R<Flow> {
        for e in init {
            self.eval(e)?;
        }
        let mut guard = 0u64;
        loop {
            if let Some(c) = cond {
                let (b, _) = self.eval_bool(c)?;
                if !b {
                    break;
                }
            }
            guard += 1;
            if guard > 20_000_000 {
                return err(body.stmts.first().map(|s| s.pos).unwrap_or_default(), "The loop runs too long");
            }
            let mark = self.scopes.len();
            let (f, id) = self.exec_scoped(body)?;
            if self.fn_depth > 0 {
                self.scopes.truncate(mark);
            } else {
                self.scopes[holder as usize].iter.push(id);
            }
            if let Flow::Return(v) = f {
                return Ok(Flow::Return(v));
            }
            for e in step {
                self.eval(e)?;
            }
        }
        Ok(Flow::Normal)
    }

    pub fn push_data(&mut self, v: &Val, size: u8, out: &mut Vec<u8>, pos: Pos) -> R<()> {
        let x = match v {
            Val::Num(x, _, _) => *x,
            Val::Char(_, x) => *x,
            Val::Arg(_, inner) if inner.as_f64().is_some() => inner.as_f64().unwrap(),
            _ => return err(pos, format!("Can't get a double representation from a value of type {}", v.type_name())),
        };
        let n = x as i64;
        for i in 0..size {
            out.push(((n >> (8 * i)) & 0xff) as u8);
        }
        Ok(())
    }

    pub fn read_file(&mut self, name: &str, pos: Pos) -> R<Vec<u8>> {
        let dir = self
            .src
            .files
            .get(pos.file)
            .and_then(|f| f.path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| self.cfg.src_dir.clone());
        let mut libs = vec![self.cfg.src_dir.clone()];
        libs.extend(self.cfg.libdirs.iter().cloned());
        match crate::preproc::find_file(name, &dir, &libs) {
            Some(p) => std::fs::read(&p).or_else(|_| err(pos, format!("Can't open file '{}'", name))),
            None => err(pos, format!("Can't open file '{}'", name)),
        }
    }

    // ------------------------------------------------------------ segments
    pub fn seg_params(&mut self, params: &Params, pos: Pos) -> R<SegParams> {
        let mut p = SegParams { pos, ..Default::default() };
        for (name, e, _) in params {
            let v = match e {
                Some(e) => self.eval(e)?,
                None => Val::Bool(true, 0),
            };
            let as_i = |v: &Val| v.as_f64().map(|x| x as i64);
            let as_b = |v: &Val| matches!(v, Val::Bool(true, _));
            let list = |v: &Val| -> Vec<String> { v.to_str().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect() };
            match name.as_str() {
                "start" => p.start = as_i(&v),
                "startAfter" => p.start_after = Some(v.to_str()),
                "align" => p.align = as_i(&v),
                "min" => p.min = as_i(&v),
                "max" => p.max = as_i(&v),
                "fill" => p.fill = as_b(&v),
                "fillByte" => p.fill_byte = as_i(&v).unwrap_or(0) as u8,
                "virtual" => p.virt = as_b(&v),
                "allowOverlap" => p.allow_overlap = as_b(&v),
                "hide" => p.hide = as_b(&v),
                "outPrg" => p.out_prg = Some(v.to_str()),
                "outBin" => p.out_bin = Some(v.to_str()),
                "segments" => p.segments = list(&v),
                "prgFiles" => p.prg_files = list(&v),
                "sidFiles" => p.sid_files = list(&v),
                "modify" => p.modify = Some(v.to_str()),
                "dest" => p.dest = Some(v.to_str()),
                "name" => p.name = Some(v.to_str()),
                "type" => p.ftype = Some(v.to_str()),
                "mbfiles" => p.mbfiles = as_b(&v),
                n if n.starts_with('_') || n.starts_with("marg") => {
                    p.margs.insert(n.to_string(), v);
                }
                n => {
                    p.other.insert(n.to_string(), v);
                }
            }
        }
        Ok(p)
    }

    /// blocks of a named segment after its own parameters were applied
    pub fn segment_blocks(&mut self, name: &str, pos: Pos, during_pass: bool, depth: u32) -> R<Vec<Blk>> {
        if depth > 16 {
            return err(pos, "Segments include each other in a loop");
        }
        let sg = if during_pass && self.prev_segs.contains_key(name) {
            // during a pass a segment may not be complete yet: the previous pass knows all of it
            self.used_fwd = true;
            self.prev_segs.get(name).cloned()
        } else {
            self.segs.iter().find(|s| s.name == name).cloned()
        };
        let sg = match sg {
            Some(s) => s,
            None => {
                if during_pass && (self.pass == 1 || self.lenient) {
                    self.used_unk = true;
                    return Ok(vec![]);
                }
                return err(pos, format!("Unknown segment '{}'", name));
            }
        };
        let mut blocks = sg.blocks.clone();
        let mut inc = self.included_blocks(&sg.p, pos, during_pass, depth)?;
        blocks.append(&mut inc);
        self.apply_params(blocks, &sg.p, &sg.name, pos)
    }

    fn included_blocks(&mut self, p: &SegParams, pos: Pos, during_pass: bool, depth: u32) -> R<Vec<Blk>> {
        let mut out = vec![];
        for s in &p.segments {
            let mut b = self.segment_blocks(s, pos, during_pass, depth + 1)?;
            out.append(&mut b);
        }
        for f in &p.prg_files {
            let d = self.read_file(f, pos)?;
            if d.len() < 2 {
                return err(pos, format!("The file '{}' is not a valid prg file", f));
            }
            let start = d[0] as i64 | (d[1] as i64) << 8;
            let short = std::path::Path::new(f).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(f.clone());
            out.push(Blk { start, name: short, virt: false, named: true, bytes: d[2..].to_vec(), dump: vec![] });
        }
        for f in &p.sid_files {
            let d = self.read_file(f, pos)?;
            let sid = crate::builtins::parse_sid(d, pos)?;
            let short = std::path::Path::new(f).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(f.clone());
            out.push(Blk {
                start: sid.location as i64,
                name: short,
                virt: false,
                named: true,
                bytes: sid.file[sid.data_offset..].to_vec(),
                dump: vec![],
            });
        }
        Ok(out)
    }

    fn apply_params(&mut self, mut blocks: Vec<Blk>, p: &SegParams, name: &str, pos: Pos) -> R<Vec<Blk>> {
        if let Some(m) = &p.modify {
            if m != "BasicUpstart" {
                return err(pos, format!("Unknown segment modifier '{}'", m));
            }
            let st = p.margs.get("_start").or_else(|| p.margs.get("marg1")).and_then(|v| v.as_f64()).unwrap_or(0.0) as i64;
            let digits = st.to_string();
            let next = BASIC_START + 8 + digits.len() as i64 - 2;
            let mut b = vec![(next & 0xff) as u8, (next >> 8) as u8, 10, 0, 0x9e];
            for c in digits.chars() {
                b.push(self.enc.conv(self.encoding, c));
            }
            b.extend_from_slice(&[0, 0, 0]);
            blocks.push(Blk { start: BASIC_START, name: "BasicUpstart".into(), virt: false, named: true, bytes: b, dump: vec![] });
        }
        for b in &blocks {
            if b.virt || b.bytes.is_empty() {
                continue;
            }
            let end = b.start + b.bytes.len() as i64 - 1;
            if let Some(mn) = p.min {
                if b.start < mn {
                    return err(pos, format!("In segment '{}' the memoryblock {} is below the minimum address ${:04x}", name, blk_desc(b), mn));
                }
            }
            if let Some(mx) = p.max {
                if end > mx {
                    return err(pos, format!("In segment '{}' the memoryblock {} is above the maximum address ${:04x}", name, blk_desc(b), mx));
                }
            }
        }
        if p.fill {
            if let (Some(mn), Some(mx)) = (p.min, p.max) {
                let mut data = vec![p.fill_byte; (mx - mn + 1).max(0) as usize];
                for b in blocks.iter().filter(|b| !b.virt) {
                    for (i, x) in b.bytes.iter().enumerate() {
                        let a = b.start + i as i64 - mn;
                        if a >= 0 && (a as usize) < data.len() {
                            data[a as usize] = *x;
                        }
                    }
                }
                return Ok(vec![Blk { start: mn, name: name.to_string(), virt: false, named: true, bytes: data, dump: vec![] }]);
            }
        }
        if !p.allow_overlap && !self.cfg.allow_overlap {
            let mut bl: Vec<&Blk> = blocks.iter().filter(|b| !b.virt && !b.bytes.is_empty()).collect();
            bl.sort_by_key(|b| b.start);
            for w in bl.windows(2) {
                if w[0].start + w[0].bytes.len() as i64 > w[1].start {
                    return err(pos, format!("In segment '{}' the memoryblock {} overlaps {}", name, blk_desc(w[1]), blk_desc(w[0])));
                }
            }
        } else {
            // later blocks win where blocks overlap
            let mut lo = i64::MAX;
            let mut hi = i64::MIN;
            for b in blocks.iter().filter(|b| !b.virt && !b.bytes.is_empty()) {
                lo = lo.min(b.start);
                hi = hi.max(b.start + b.bytes.len() as i64);
            }
            if lo < hi {
                let mut data = vec![None; (hi - lo) as usize];
                for b in blocks.iter().filter(|b| !b.virt) {
                    for (i, x) in b.bytes.iter().enumerate() {
                        data[(b.start - lo) as usize + i] = Some(*x);
                    }
                }
                let mut out = vec![];
                let mut i = 0;
                while i < data.len() {
                    if data[i].is_none() {
                        i += 1;
                        continue;
                    }
                    let st = i;
                    let mut bytes = vec![];
                    while i < data.len() && data[i].is_some() {
                        bytes.push(data[i].unwrap());
                        i += 1;
                    }
                    out.push(Blk { start: lo + st as i64, name: name.into(), virt: false, named: true, bytes, dump: vec![] });
                }
                return Ok(out);
            }
        }
        Ok(blocks)
    }

    /// blocks of an intermediate segment given by parameters (.file, .segmentout, disk files)
    pub fn intermediate_blocks(&mut self, p: &SegParams, pos: Pos, during_pass: bool) -> R<Vec<Blk>> {
        let blocks: Vec<Blk> = self.included_blocks(p, pos, during_pass, 0)?.into_iter().filter(|b| !b.virt).collect();
        self.apply_params(blocks, p, "Intermediate", pos)
    }

    // ------------------------------------------------------------ instructions
    fn instr(&mut self, mn: &str, suffix: Option<&str>, arg: &Arg, pos: Pos) -> R<()> {
        if let Some(l) = &arg.label {
            let pc = self.pc() + 1;
            let sc = self.cur_scope;
            let fl = self.pc_flags & !F_FWD;
            self.define(sc, l, Val::Num(pc as f64, fl, 0), SymKind::Label, pos)?;
        }
        let (ty, v, v2) = self.arg_value(arg)?;
        self.assemble(mn, suffix, ty, v, v2, arg, pos)
    }

    /// evaluates an argument into (argument type id, value, second value)
    fn arg_value(&mut self, arg: &Arg) -> R<(i32, Val, Option<Val>)> {
        let v = match &arg.e {
            Some(e) => self.eval(e)?,
            None => Val::Null,
        };
        let ty = match arg.mode {
            Mode::None => AT_NONE,
            Mode::Imm => AT_IMMEDIATE,
            Mode::Abs => {
                if let Val::Arg(t, inner) = v {
                    return Ok((t, *inner, None));
                }
                AT_ABSOLUTE
            }
            Mode::AbsX => AT_ABSOLUTEX,
            Mode::AbsY => AT_ABSOLUTEY,
            Mode::Ind => AT_INDIRECT,
            Mode::IndX => AT_INDIRECTX,
            Mode::IndY => AT_IZEROPAGEY,
            Mode::ZpRel => 13,
        };
        let v = match v {
            Val::Arg(_, inner) => *inner,
            o => o,
        };
        let v2 = match &arg.e2 {
            Some(e) => Some(self.eval(e)?),
            None => None,
        };
        Ok((ty, v, v2))
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(&mut self, mn: &str, suffix: Option<&str>, ty: i32, v: Val, v2: Option<Val>, arg: &Arg, pos: Pos) -> R<()> {
        let ops = self.cpu.ops[mn];
        let get = |t: i32| -> i16 { if (0..15).contains(&t) { ops[t as usize] } else { -1 } };
        let mut force_abs = false;
        let mut force_zp = false;
        let mut ty = ty;
        if let Some(sfx) = suffix {
            match sfx {
                "a" | "abs" => force_abs = true,
                "z" | "zp" => force_zp = true,
                "im" | "imm" => ty = 1,
                "zx" | "zpx" => ty = 3,
                "zy" | "zpy" => ty = 4,
                "izx" | "izpx" => ty = 5,
                "izy" | "izpy" => ty = 6,
                "ax" | "absx" => ty = 8,
                "ay" | "absy" => ty = 9,
                "i" | "ind" => ty = 10,
                "r" | "rel" => ty = 11,
                other => return err(pos, format!("Unknown mnemonic extension '.{}'", other)),
            }
        }
        let value = v.as_f64();
        let flags = v.flags();
        let unk = flags & F_UNK != 0;
        let fwd = flags & F_FWD != 0;
        let epos = arg.e.as_ref().map(|e| e.pos).unwrap_or(pos);
        if ty != AT_NONE && value.is_none() {
            return err(
                epos,
                format!("The value of a Command Argument Value must be an integer. This expression returns a value of type '{}'", v.type_name()),
            );
        }
        let iv = value.unwrap_or(0.0) as i64;
        let mut warn_zp = false;
        if ty < 0 {
            let (zpf, absf) = match ty {
                -1 => (2, 7),
                -2 => (3, 8),
                -3 => (4, 9),
                -4 => (12, 10),
                _ => (5, 14),
            };
            if force_zp && get(zpf) < 0 {
                return err(pos, format!("'{}' does not support mode '{}'", mn, arg_type_name(zpf)));
            }
            let zp_ok = get(zpf) >= 0;
            let known = !(unk || fwd);
            // .zp labels choose the zeropage form, but only when their value fits
            let zp_bound = flags & F_ZP != 0 && (unk || (0..=255).contains(&iv));
            ty = if force_abs {
                absf
            } else if zp_ok && (zp_bound || (known && iv <= 255) || force_zp) {
                zpf
            } else if ty == -1 && get(11) >= 0 {
                11
            } else {
                absf
            };
            if !force_abs && zp_ok && ty == absf && !known && (0..256).contains(&iv) {
                warn_zp = true;
            }
        }
        let op = get(ty);
        if op < 0 {
            return err(pos, format!("invalid adressingmode. '{}' doesn't support {} mode", mn, arg_type_name(ty)));
        }
        let size = 1 + mode_size(ty);
        let mut bytes = vec![op as u8];
        let pc = self.pc();
        if unk || (ty == 13 && v2.as_ref().map(|x| x.flags() & F_UNK != 0).unwrap_or(false)) {
            bytes.resize(size, 0);
            return self.emit(&bytes, pos);
        }
        let mut val = iv;
        if ty == 11 {
            let d = iv - (pc + size as i64);
            if !(-128..=127).contains(&d) {
                return err(epos, format!("relative address is illegal (jump distance is too far: {}).", d));
            }
            val = d;
        }
        if ty == 13 {
            let t = v2.as_ref().and_then(|x| x.as_f64()).unwrap_or(0.0) as i64;
            let d = t - (pc + size as i64);
            if !(-128..=127).contains(&d) {
                return err(pos, "relative address is illegal (jump distance is too far).");
            }
            val = (iv & 0xff) | ((d & 0xff) << 8);
        }
        for i in 1..size {
            bytes.push(((val >> (8 * (i - 1))) & 0xff) as u8);
        }
        if warn_zp && self.cfg.warnings {
            self.warnings.push(Warning {
                pos,
                msg: "Using absolute mode for zeropage argument. Try defining involved labels before this statement.".into(),
            });
        }
        self.emit(&bytes, pos)
    }

    // ------------------------------------------------------------ macros & pseudocommands
    pub fn find_pseudo(&self, name: &str, root: bool) -> Option<(Rc<FuncDef>, usize)> {
        let mut n = if root { 0 } else { self.cur_ns };
        loop {
            if let Some(f) = self.ns[n].pseudos.get(name) {
                return Some((f.clone(), n));
            }
            n = self.ns[n].parent?;
        }
    }
    fn find_macro(&self, name: &str, root: bool, argc: usize) -> Option<(Rc<FuncDef>, usize)> {
        let mut n = if root { 0 } else { self.cur_ns };
        loop {
            if let Some(list) = self.ns[n].macros.get(name) {
                if let Some(f) = list.iter().find(|f| f.params.len() == argc) {
                    return Some((f.clone(), n));
                }
            }
            n = self.ns[n].parent?;
        }
    }
    pub fn find_function(&self, name: &str, root: bool, argc: usize) -> Option<(Rc<FuncDef>, usize)> {
        let mut n = if root { 0 } else { self.cur_ns };
        loop {
            if let Some(f) = self.ns[n].funcs.get(&(name.to_string(), argc)) {
                return Some((f.clone(), n));
            }
            n = self.ns[n].parent?;
        }
    }
    pub fn has_function_name(&self, name: &str, root: bool) -> bool {
        let mut n = if root { 0 } else { self.cur_ns };
        loop {
            if self.ns[n].funcs.keys().any(|(k, _)| k == name) {
                return true;
            }
            match self.ns[n].parent {
                Some(p) => n = p,
                None => return false,
            }
        }
    }
    pub fn find_struct(&self, name: &str, root: bool) -> Option<Rc<StructDef>> {
        let mut n = if root { 0 } else { self.cur_ns };
        loop {
            if let Some(f) = self.ns[n].structs.get(name) {
                return Some(f.clone());
            }
            n = self.ns[n].parent?;
        }
    }

    fn call_body(&mut self, f: &Rc<FuncDef>, def_ns: usize, args: Vec<Val>, pos: Pos) -> R<Flow> {
        if self.call_depth > 200 {
            return err(pos, "Too deep recursion (does a macro call itself without stopping?)");
        }
        let def_ns = self.fn_home.get(&(Rc::as_ptr(f) as usize)).copied().unwrap_or(def_ns);
        let parent = self.ns_scope.get(&def_ns).cloned().unwrap_or(0);
        let attach = self.pending_label.take();
        let id = self.new_scope(parent, Some(f.body.clone()));
        self.scopes[id as usize].ns = def_ns;
        if let Some((sc, name)) = attach {
            self.attach_scope(sc, &name, id);
        }
        for (p, v) in f.params.iter().zip(args) {
            self.scopes[id as usize].order.push(p.clone());
            self.scopes[id as usize].syms.insert(p.clone(), Sym { val: v, kind: SymKind::Var });
        }
        let saved = (self.cur_scope, self.cur_ns);
        self.cur_scope = id;
        self.cur_ns = def_ns;
        self.call_depth += 1;
        let r = self.exec_block_here(&f.body);
        self.call_depth -= 1;
        self.cur_scope = saved.0;
        self.cur_ns = saved.1;
        r.map_err(|mut e| {
            e.called.push(pos);
            e
        })
    }

    fn call_macro(&mut self, name: &str, root: bool, args: &[E], pos: Pos) -> R<()> {
        let (f, ns) = match self.find_macro(name, root, args.len()) {
            Some(x) => x,
            None => {
                if self.find_pseudo(name, root).is_some() {
                    return err(pos, format!("'{}' is a pseudocommand, call it without parenthesis", name));
                }
                return err(pos, format!("Unknown macro '{}'", name));
            }
        };
        let mut vals = vec![];
        for a in args {
            vals.push(self.eval(a)?);
        }
        self.no_fn_mode(pos)?;
        self.call_body(&f, ns, vals, pos)?;
        Ok(())
    }

    fn call_pseudo(&mut self, name: &str, root: bool, args: &[Arg], pos: Pos) -> R<()> {
        let (f, ns) = match self.find_pseudo(name, root) {
            Some(x) => x,
            None => {
                if args.is_empty() && self.find_macro(name, root, 0).is_some() {
                    return self.call_macro(name, root, &[], pos);
                }
                return err(pos, format!("Unknown pseudocommand '{}'", name));
            }
        };
        let mut vals = vec![];
        for a in args {
            if a.mode == Mode::None {
                vals.push(Val::Arg(AT_NONE, Box::new(num(0.0))));
                continue;
            }
            let (t, v, _) = self.arg_value(a)?;
            vals.push(Val::Arg(t, Box::new(v)));
        }
        if vals.len() > f.params.len() {
            return err(pos, format!("The pseudocommand '{}' takes {} arguments", name, f.params.len()));
        }
        while vals.len() < f.params.len() {
            vals.push(Val::Arg(AT_NONE, Box::new(num(0.0))));
        }
        self.call_body(&f, ns, vals, pos)?;
        Ok(())
    }

    /// calls a user defined function (in function mode)
    pub fn call_function(&mut self, f: &Rc<FuncDef>, def_ns: usize, args: Vec<Val>, pos: Pos) -> R<Val> {
        let mark = self.scopes.len();
        self.fn_depth += 1;
        let saved_pending = self.pending_label.take();
        let r = self.call_body(f, def_ns, args, pos);
        self.pending_label = saved_pending;
        self.fn_depth -= 1;
        self.scopes.truncate(mark);
        match r? {
            Flow::Return(v) => Ok(v),
            Flow::Normal => Ok(Val::Null),
        }
    }

    pub fn multi_ref(&mut self, name: &str, d: i32, pos: Pos) -> R<Val> {
        let cur_len = self.multi.get(name).map(|l| l.len()).unwrap_or(0);
        if d < 0 {
            let k = (-d) as usize;
            if cur_len >= k {
                return Ok(num(self.multi[name][cur_len - k]));
            }
        } else {
            let idx = cur_len + d as usize - 1;
            if let Some(v) = self.prev_multi.get(name).and_then(|l| l.get(idx)) {
                self.used_fwd = true;
                return Ok(Val::Num(*v, F_FWD, 0));
            }
        }
        let sign = if d < 0 { "-" } else { "+" };
        let label = format!("!{}{}", name, sign.repeat(d.unsigned_abs() as usize));
        if self.pass == 1 || self.lenient || d > 0 && self.pass <= 2 {
            return Ok(self.unknown(pos, &label));
        }
        err(pos, format!("Unknown symbol '{}'", label))
    }
}
