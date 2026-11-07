//! Expression evaluation, operators and the built-in function library.

use crate::ast::*;
use crate::eval::*;
use crate::value::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn tn(v: &Val) -> &'static str {
    v.type_name()
}

fn need_num(v: &Val, pos: Pos) -> R<f64> {
    match v {
        Val::Num(x, _, _) => Ok(*x),
        Val::Char(_, x) => Ok(*x),
        _ => err(pos, format!("Can't get a double representation from a value of type {}", tn(v))),
    }
}
fn need_bool(v: &Val, pos: Pos) -> R<bool> {
    match v {
        Val::Bool(b, _) => Ok(*b),
        Val::Num(_, f, _) if f & F_UNK != 0 => Ok(false),
        _ => err(pos, format!("Can't get a boolean representation from a value of type {}", tn(v))),
    }
}
/// errors raised inside a built-in operation get the call position in their trace
fn trace(mut e: AsmError, pos: Pos) -> AsmError {
    if e.called.is_empty() && e.pos == Some(pos) && !e.msg.starts_with("Unknown function") && !e.msg.starts_with("Invalid number") {
        e.called.push(pos);
    }
    e
}

fn i(x: f64) -> i64 {
    x as i64
}
fn nf(x: f64, f: u8) -> Val {
    Val::Num(x, f, 0)
}
pub fn new_list(v: Vec<Val>) -> Val {
    Val::List(Rc::new(ListV { v: RefCell::new(v), locked: Cell::new(false) }))
}

fn new_matrix(m: [[f64; 4]; 4]) -> Val {
    Val::Matrix(Rc::new(RefCell::new(m)))
}

pub fn parse_sid(file: Vec<u8>, pos: Pos) -> R<SidFile> {
    let head = sid_string(&file, 0, 4);
    if file.len() < 0x7c || (head != "PSID" && head != "RSID") {
        return err(pos, format!("Unknown sid format : {}", head));
    }
    let mut data_offset = sid_int(&file, 6, 2) as usize;
    let mut location = sid_int(&file, 8, 2);
    if location == 0 && data_offset + 1 < file.len() {
        location = file[data_offset] as u32 + 256 * file[data_offset + 1] as u32;
        data_offset += 2;
    }
    Ok(SidFile { file, data_offset: data_offset.min(usize::MAX), location })
}

fn mat_mul(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut r = [[0.0; 4]; 4];
    for (y, row) in r.iter_mut().enumerate() {
        for (x, c) in row.iter_mut().enumerate() {
            *c = (0..4).map(|k| a[y][k] * b[k][x]).sum();
        }
    }
    r
}
fn mat_vec(m: &[[f64; 4]; 4], v: &[f64; 3]) -> Val {
    let r: Vec<f64> = (0..4).map(|k| v[0] * m[k][0] + v[1] * m[k][1] + v[2] * m[k][2] + m[k][3]).collect();
    Val::Vector([r[0] / r[3], r[1] / r[3], r[2] / r[3]])
}

fn pad_left(s: String, n: i64, c: char) -> String {
    let mut s = s;
    while (s.chars().count() as i64) < n {
        s.insert(0, c);
    }
    s
}

impl Asm {
    pub fn eval(&mut self, e: &E) -> R<Val> {
        let pos = e.pos;
        match &e.k {
            EK::Num(n) => Ok(num(*n)),
            EK::Str(s) => Ok(Val::Str(s.clone())),
            EK::Char(c) => {
                let b = self.enc.conv(self.encoding, *c) as i8;
                Ok(Val::Char(*c, b as f64))
            }
            EK::Bool(b) => Ok(Val::Bool(*b, 0)),
            EK::Null => Ok(Val::Null),
            EK::Id { name, root } => self.lookup(name, *root, pos),
            EK::Pc => Ok(self.pc_val()),
            EK::Multi(n, d) => self.multi_ref(n, *d, pos),
            EK::List(items) => {
                let mut v = vec![];
                for x in items {
                    v.push(self.eval(x)?);
                }
                Ok(new_list(v))
            }
            EK::Un(op, a) => {
                let v = self.eval(a)?;
                let f = v.flags();
                if *op != "!" && v.as_f64().is_none() && !matches!(v, Val::Vector(_)) {
                    return err(pos, format!("Unknown function : {}", op));
                }
                match *op {
                    "-" => match v {
                        Val::Vector(x) => Ok(Val::Vector([-x[0], -x[1], -x[2]])),
                        _ => Ok(nf(-need_num(&v, a.pos)?, f)),
                    },
                    "!" => match v {
                        Val::Bool(..) | Val::Num(_, F_UNK, _) => Ok(Val::Bool(!need_bool(&v, a.pos)?, f)),
                        _ => err(pos, "Unknown function : !"),
                    },
                    "~" => Ok(nf(!i(need_num(&v, a.pos)?) as f64, f)),
                    "<" => Ok(nf((i(need_num(&v, a.pos)?) & 0xff) as f64, f & !F_ZP)),
                    _ => Ok(nf(((i(need_num(&v, a.pos)?) >> 8) & 0xff) as f64, f & !F_ZP)),
                }
            }
            EK::Bin(op, a, b) => {
                if *op == "&&" || *op == "||" {
                    let va = self.eval(a)?;
                    let x = need_bool(&va, pos)?;
                    if (*op == "&&" && !x) || (*op == "||" && x) {
                        return Ok(Val::Bool(x, va.flags()));
                    }
                    let vb = self.eval(b)?;
                    let y = need_bool(&vb, b.pos)?;
                    return Ok(Val::Bool(y, va.flags() | vb.flags()));
                }
                let va = self.eval(a)?;
                let vb = self.eval(b)?;
                self.binop(op, &va, &vb, pos).map_err(|e| trace(e, pos))
            }
            EK::Tern(c, a, b) => {
                let vc = self.eval(c)?;
                if need_bool(&vc, c.pos)? {
                    Ok(self.eval(a)?.with_flags(vc.flags() & F_UNK))
                } else {
                    Ok(self.eval(b)?.with_flags(vc.flags() & F_UNK))
                }
            }
            EK::Call { name, root, args } => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.eval(a)?);
                }
                self.call(name, *root, vals, pos)
            }
            EK::Method { obj, name, args } => {
                let o = self.eval(obj)?;
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.eval(a)?);
                }
                // errors point at the method name, right after the dot
                let npos = Pos { col: pos.col + 1, ..pos };
                self.method(o, name, vals, npos).map_err(|e| trace(e, npos))
            }
            EK::Field { obj, name } => {
                let o = self.eval(obj)?;
                let npos = Pos { col: pos.col + 1, ..pos };
                if let (Val::Num(_, f, 0), EK::Id { name: id, root }) = (&o, &obj.k) {
                    if f & (F_UNK | F_FWD) == 0 && self.is_label(id, *root) {
                        return err(npos, format!("Variable '{}' not present on value of type 'LabelValue'", name));
                    }
                }
                self.field(o, name, npos)
            }
            EK::Index { obj, idx } => {
                let o = self.eval(obj)?;
                let (n, _) = self.eval_int(idx)?;
                let sc = match &o {
                    Val::Num(_, f, sc) if *sc > 0 => {
                        let _ = f;
                        Some(sc - 1)
                    }
                    Val::Scope(s) => Some(*s),
                    _ => None,
                };
                if let Some(sc) = sc {
                    let it = self.scope_iters(sc);
                    return match it.get(n as usize) {
                        Some(id) => Ok(Val::Scope(*id)),
                        None => {
                            if self.pass == 1 || self.lenient {
                                Ok(self.unknown(pos, "[]"))
                            } else {
                                err(obj.pos, format!("Index out of range: {}", n))
                            }
                        }
                    };
                }
                // a loop label that comes later in the source isn't known in the first pass
                if o.is_unknown() {
                    return Ok(self.unknown(pos, "[]"));
                }
                if let Val::List(l) = &o {
                    return l.v.borrow().get(n as usize).cloned().map_or_else(|| err(pos, format!("Index out of bound : {}", n)), Ok);
                }
                err(pos, format!("Can't index a value of type {}", tn(&o)))
            }
            EK::Assign { op, target, val } => {
                let v = self.eval(val)?;
                let nv = if *op == "=" {
                    v
                } else {
                    let old = self.eval(target)?;
                    self.binop(op, &old, &v, pos)?
                };
                self.store(target, nv.clone())?;
                Ok(nv)
            }
            EK::Post { target, delta } => {
                let old = self.eval(target)?;
                let nv = nf(need_num(&old, pos)? + delta, old.flags());
                self.store(target, nv)?;
                Ok(old)
            }
            EK::Pre { target, delta } => {
                let old = self.eval(target)?;
                let nv = nf(need_num(&old, pos)? + delta, old.flags());
                self.store(target, nv.clone())?;
                Ok(nv)
            }
            EK::Decl { name, root, init } => {
                let v = match init {
                    Some(x) => self.eval(x)?,
                    None => Val::Null,
                };
                let sc = if *root { 0 } else { self.cur_scope };
                self.define(sc, name, v.clone(), SymKind::Var, pos)?;
                Ok(v)
            }
        }
    }

    fn scope_iters(&self, id: u32) -> Vec<u32> {
        if (id as usize) < self.scopes.len() {
            let it = &self.scopes[id as usize].iter;
            if !it.is_empty() {
                return it.clone();
            }
        }
        self.prev_scopes.get(id as usize).map(|s| s.iter.clone()).unwrap_or_default()
    }

    fn store(&mut self, target: &E, v: Val) -> R<()> {
        match &target.k {
            EK::Id { name, root } => self.assign_var(name, *root, v, target.pos),
            EK::Decl { name, root, .. } => {
                let sc = if *root { 0 } else { self.cur_scope };
                self.define(sc, name, v, SymKind::Var, target.pos)
            }
            EK::Field { obj, name } => {
                let o = self.eval(obj)?;
                match o {
                    Val::Struct(d, vals) => match d.fields.iter().position(|f| f == name) {
                        Some(i) => {
                            vals.borrow_mut()[i] = v;
                            Ok(())
                        }
                        None => err(target.pos, format!("Unknown field '{}'", name)),
                    },
                    _ => err(target.pos, "Can't assign to this field"),
                }
            }
            _ => err(target.pos, "Can't assign a value to this expression"),
        }
    }

    pub fn binop(&mut self, op: &str, a: &Val, b: &Val, pos: Pos) -> R<Val> {
        let f = (a.flags() | b.flags()) & !F_ZP;
        if op == "+" {
            if let Val::Str(s) = a {
                return Ok(Val::Str(Rc::from(format!("{}{}", s, b.to_str()))));
            }
        }
        if op == "==" || op == "!=" {
            let eq = match (a, b) {
                (Val::Num(..) | Val::Char(..), Val::Num(..) | Val::Char(..)) => a.as_f64() == b.as_f64(),
                (Val::Char(..), Val::Str(_)) | (Val::Str(_), Val::Char(..)) | (Val::Str(_), Val::Str(_)) => a.to_str() == b.to_str(),
                (Val::Bool(x, _), Val::Bool(y, _)) => x == y,
                (Val::Null, Val::Null) => true,
                (Val::Null, _) | (_, Val::Null) => false,
                (Val::Num(..), _) => false,
                (Val::Vector(x), Val::Vector(y)) => x == y,
                (Val::List(x), Val::List(y)) => Rc::ptr_eq(x, y),
                _ => a.to_str() == b.to_str(),
            };
            return Ok(Val::Bool(if op == "==" { eq } else { !eq }, f));
        }
        match (a, b) {
            (Val::Vector(x), Val::Vector(y)) => {
                return match op {
                    "+" => Ok(Val::Vector([x[0] + y[0], x[1] + y[1], x[2] + y[2]])),
                    "-" => Ok(Val::Vector([x[0] - y[0], x[1] - y[1], x[2] - y[2]])),
                    "*" => Ok(num(x[0] * y[0] + x[1] * y[1] + x[2] * y[2])),
                    _ => err(pos, format!("Unknown operator '{}' for vectors", op)),
                }
            }
            (Val::Vector(_), _) if b.as_f64().is_some() && (op == "+" || op == "-") => {
                return err(pos, "Can't get a vector from value of type number");
            }
            (Val::Vector(x), _) if b.as_f64().is_some() => {
                let k = b.as_f64().unwrap();
                return match op {
                    "*" => Ok(Val::Vector([x[0] * k, x[1] * k, x[2] * k])),
                    "/" => Ok(Val::Vector([x[0] / k, x[1] / k, x[2] / k])),
                    _ => err(pos, format!("Unknown operator '{}' for vectors", op)),
                };
            }
            (Val::Matrix(m), Val::Matrix(n)) if op == "*" => return Ok(new_matrix(mat_mul(&m.borrow(), &n.borrow()))),
            (Val::Matrix(m), Val::Vector(v)) if op == "*" => return Ok(mat_vec(&m.borrow(), v)),
            (Val::Matrix(_), _) => return err(pos, format!("Can't multiply a matrix with an object of type {}", tn(b))),
            _ => {}
        }
        // the operators are methods of the left value
        if a.as_f64().is_none() {
            return err(pos, format!("Unknown function : {}", op));
        }
        let x = need_num(a, pos)?;
        let y = need_num(b, pos)?;
        Ok(match op {
            "+" => nf(x + y, f),
            "-" => nf(x - y, f),
            "*" => nf(x * y, f),
            "/" => nf(x / y, f),
            "&" => nf((i(x) & i(y)) as f64, f),
            "|" => nf((i(x) | i(y)) as f64, f),
            "^" => nf((i(x) ^ i(y)) as f64, f),
            "<<" => nf(i(x).wrapping_shl(i(y) as u32) as f64, f),
            ">>" => nf(i(x).wrapping_shr(i(y) as u32) as f64, f),
            "<" => Val::Bool(x < y, f),
            ">" => Val::Bool(x > y, f),
            "<=" => Val::Bool(x <= y, f),
            ">=" => Val::Bool(x >= y, f),
            _ => return err(pos, format!("Unknown operator '{}'", op)),
        })
    }

    fn field(&mut self, o: Val, name: &str, pos: Pos) -> R<Val> {
        match &o {
            Val::Num(_, f, sc) => {
                if *sc > 0 {
                    return self.scope_field(sc - 1, name, "LabelValue", pos);
                }
                if f & (F_UNK | F_FWD) != 0 {
                    // the label's scope isn't known yet
                    return Ok(self.unknown_member(pos, name));
                }
                err(pos, format!("Variable '{}' not present on value of type 'number'", name))
            }
            Val::Scope(id) => {
                let scope = self.scopes.get(*id as usize).or(self.prev_scopes.get(*id as usize));
                let ty = if scope.is_some_and(|s| s.ns_name.is_some()) { "Namespace" } else { "ScopeValue" };
                self.scope_field(*id, name, ty, pos)
            }
            Val::Struct(d, v) => match d.fields.iter().position(|f| f == name) {
                Some(i) => Ok(v.borrow()[i].clone()),
                None => err(pos, format!("Variable '{}' not present on value of type 'struct'", name)),
            },
            Val::Picture(p) => match name {
                "width" => Ok(num(p.w as f64)),
                "height" => Ok(num(p.h as f64)),
                _ => err(pos, format!("Variable '{}' not present on value of type '{}'", name, tn(&o))),
            },
            Val::Sid(sd) => {
                let f = &sd.file;
                let ver = sid_int(f, 4, 2);
                Ok(match name {
                    "header" => s(&sid_string(f, 0, 4)),
                    "version" => num(ver as f64),
                    "location" => num(sd.location as f64),
                    "init" => num(sid_int(f, 10, 2) as f64),
                    "play" => num(sid_int(f, 12, 2) as f64),
                    "songs" => num(sid_int(f, 14, 2) as f64),
                    "startSong" => num(sid_int(f, 16, 2) as f64),
                    "speed" => num(sid_int(f, 18, 4) as f64),
                    "name" => s(&sid_string(f, 22, 32)),
                    "author" => s(&sid_string(f, 54, 32)),
                    "copyright" => s(&sid_string(f, 86, 32)),
                    "size" => num((f.len() - sd.data_offset) as f64),
                    "flags" if ver >= 2 => num(sid_int(f, 118, 2) as f64),
                    "startpage" if ver >= 2 => num(sid_int(f, 120, 1) as f64),
                    "pagelength" if ver >= 2 => num(sid_int(f, 121, 1) as f64),
                    "flags" | "startpage" | "pagelength" => Val::Null,
                    _ => return err(pos, format!("Variable '{}' not present on value of type '{}'", name, tn(&o))),
                })
            }
            _ => {
                if o.is_unknown() {
                    return Ok(self.unknown(pos, name));
                }
                err(pos, format!("Variable '{}' not present on value of type '{}'", name, tn(&o)))
            }
        }
    }

    fn check_lock(locked: bool, pos: Pos) -> R<()> {
        if locked {
            return err(pos, "You can't modify a locked value");
        }
        Ok(())
    }

    fn method(&mut self, o: Val, name: &str, a: Vec<Val>, pos: Pos) -> R<Val> {
        let arg = |n: usize| -> R<f64> {
            match a.get(n) {
                Some(v) => need_num(v, pos),
                None => err(pos, format!("Invalid number of arguments : {}", name)),
            }
        };
        if method_arg_count(&o, name).is_some_and(|n| n != a.len()) && !(name == "asNumber" && a.len() == 1) {
            return err(pos, format!("Invalid number of arguments : {}", name));
        }
        if name == "string" && a.is_empty() {
            return Ok(s(&o.to_str()));
        }
        match &o {
            Val::Num(..) | Val::Char(..) if name == "number" => return Ok(num(o.as_f64().unwrap())),
            Val::Str(st) => {
                let chars: Vec<char> = st.chars().collect();
                return Ok(match name {
                    "size" => num(chars.len() as f64),
                    "charAt" => {
                        let n = arg(0)? as i64;
                        match chars.get(n as usize) {
                            Some(c) if n >= 0 => Val::Char(*c, self.enc.conv(self.encoding, *c) as i8 as f64),
                            _ => return err(pos, format!("String index out of range: {}", n)),
                        }
                    }
                    "substring" => {
                        let a1 = arg(0)? as i64;
                        let a2 = if a.len() > 1 { arg(1)? as i64 } else { chars.len() as i64 };
                        if a1 < 0 || a2 > chars.len() as i64 || a1 > a2 {
                            return err(pos, format!("String index out of range: {}", a2 - a1));
                        }
                        s(&chars[a1 as usize..a2 as usize].iter().collect::<String>())
                    }
                    "toLowerCase" => s(&st.to_lowercase()),
                    "toUpperCase" => s(&st.to_uppercase()),
                    "asBoolean" => Val::Bool(st.trim().eq_ignore_ascii_case("true"), 0),
                    "asNumber" | "number" => {
                        let t = st.trim();
                        let radix = if a.is_empty() { 10 } else { arg(0)? as u32 };
                        if radix == 10 {
                            match t.parse::<f64>() {
                                Ok(x) => num(x),
                                Err(_) => return err(pos, format!("Not a valid number : {}", st)),
                            }
                        } else {
                            match i64::from_str_radix(t, radix) {
                                Ok(x) => num(x as f64),
                                Err(_) => return err(pos, format!("Not a valid number : {}", st)),
                            }
                        }
                    }
                    _ => return err(pos, format!("Unknown function : {}", name)),
                });
            }
            Val::List(l) => {
                return Ok(match name {
                    "get" => {
                        let n = arg(0)? as i64;
                        let v = l.v.borrow();
                        if n < 0 || n as usize >= v.len() {
                            return err(pos, format!("Index out of bound : {}", n));
                        }
                        v[n as usize].clone()
                    }
                    "set" => {
                        Self::check_lock(l.locked.get(), pos)?;
                        let n = arg(0)? as i64;
                        let val = a.get(1).cloned().unwrap_or(Val::Null);
                        let mut v = l.v.borrow_mut();
                        if n < 0 || n as usize >= v.len() {
                            return err(pos, format!("Index out of bound : {}", n));
                        }
                        v[n as usize] = val.clone();
                        val
                    }
                    "add" => {
                        Self::check_lock(l.locked.get(), pos)?;
                        l.v.borrow_mut().extend(a.iter().cloned());
                        o.clone()
                    }
                    "addAll" => {
                        Self::check_lock(l.locked.get(), pos)?;
                        for x in &a {
                            if let Val::List(m) = x {
                                let items = m.v.borrow().clone();
                                l.v.borrow_mut().extend(items);
                            }
                        }
                        o.clone()
                    }
                    "size" => num(l.v.borrow().len() as f64),
                    "remove" => {
                        Self::check_lock(l.locked.get(), pos)?;
                        let n = arg(0)? as i64;
                        let mut v = l.v.borrow_mut();
                        if n < 0 || n as usize >= v.len() {
                            return err(pos, format!("Index out of bound : {}", n));
                        }
                        v.remove(n as usize)
                    }
                    "shuffle" => {
                        Self::check_lock(l.locked.get(), pos)?;
                        let n = l.v.borrow().len();
                        for k in (1..n).rev() {
                            let j = (self.random() * (k + 1) as f64) as usize;
                            l.v.borrow_mut().swap(k, j);
                        }
                        o.clone()
                    }
                    "reverse" => {
                        Self::check_lock(l.locked.get(), pos)?;
                        l.v.borrow_mut().reverse();
                        o.clone()
                    }
                    "sort" => {
                        Self::check_lock(l.locked.get(), pos)?;
                        let mut v = l.v.borrow_mut();
                        for x in v.iter() {
                            need_num(x, pos)?;
                        }
                        v.sort_by(|p, q| p.as_f64().unwrap().partial_cmp(&q.as_f64().unwrap()).unwrap_or(std::cmp::Ordering::Equal));
                        drop(v);
                        o.clone()
                    }
                    "lock" => {
                        l.locked.set(true);
                        o.clone()
                    }
                    _ => return err(pos, format!("Unknown function : {}", name)),
                });
            }
            Val::Hash(h) => {
                return Ok(match name {
                    "put" => {
                        let mut it = a.chunks(2);
                        for kv in &mut it {
                            let k = kv[0].to_str();
                            let v = kv.get(1).cloned().unwrap_or(Val::Null);
                            let mut t = h.v.borrow_mut();
                            match t.iter().position(|e| e.0 == k) {
                                Some(p) => t[p] = (k, kv[0].clone(), v),
                                None => t.push((k, kv[0].clone(), v)),
                            }
                        }
                        o.clone()
                    }
                    "get" => {
                        let k = a.first().map(|x| x.to_str()).unwrap_or_default();
                        h.v.borrow().iter().find(|e| e.0 == k).map(|e| e.2.clone()).unwrap_or(Val::Null)
                    }
                    "containsKey" => {
                        let k = a.first().map(|x| x.to_str()).unwrap_or_default();
                        Val::Bool(h.v.borrow().iter().any(|e| e.0 == k), 0)
                    }
                    "remove" => {
                        Self::check_lock(h.locked.get(), pos)?;
                        let k = a.first().map(|x| x.to_str()).unwrap_or_default();
                        let mut t = h.v.borrow_mut();
                        match t.iter().position(|e| e.0 == k) {
                            Some(p) => t.remove(p).2,
                            None => Val::Null,
                        }
                    }
                    "keys" => new_list(h.v.borrow().iter().map(|e| e.1.clone()).collect()),
                    "lock" => {
                        h.locked.set(true);
                        o.clone()
                    }
                    _ => return err(pos, format!("Unknown function : {}", name)),
                });
            }
            Val::Struct(d, v) => {
                let idx = |x: &Val| -> R<usize> {
                    match x {
                        Val::Str(n) => d.fields.iter().position(|f| **f == **n).ok_or(AsmError {
                            pos: Some(pos),
                            msg: format!("Unknown field '{}'", n),
                            called: vec![],
                        }),
                        _ => {
                            let k = need_num(x, pos)? as usize;
                            if k < d.fields.len() {
                                Ok(k)
                            } else {
                                err(pos, format!("Index out of bound : {}", k))
                            }
                        }
                    }
                };
                return Ok(match name {
                    "getStructName" => s(&d.name),
                    "getNoOfFields" => num(d.fields.len() as f64),
                    "getFieldNames" => new_list(d.fields.iter().map(|f| s(f)).collect()),
                    "get" => {
                        let k = idx(a.first().unwrap_or(&Val::Null))?;
                        v.borrow()[k].clone()
                    }
                    "set" => {
                        let k = idx(a.first().unwrap_or(&Val::Null))?;
                        let val = a.get(1).cloned().unwrap_or(Val::Null);
                        v.borrow_mut()[k] = val.clone();
                        val
                    }
                    _ => return err(pos, format!("Unknown function : {}", name)),
                });
            }
            Val::Vector(v) => {
                return Ok(match name {
                    "get" => num(*v.get(arg(0)? as usize).unwrap_or(&0.0)),
                    "getX" => num(v[0]),
                    "getY" => num(v[1]),
                    "getZ" => num(v[2]),
                    "X" => match a.first() {
                        Some(Val::Vector(w)) => Val::Vector([v[1] * w[2] - v[2] * w[1], v[2] * w[0] - v[0] * w[2], v[0] * w[1] - v[1] * w[0]]),
                        _ => return err(pos, "X needs a vector"),
                    },
                    _ => return err(pos, format!("Unknown function : {}", name)),
                });
            }
            Val::Matrix(m) => {
                let (r, c) = (arg(0)? as usize, arg(1)? as usize);
                if r > 3 || c > 3 {
                    return err(pos, format!("Index out of bound ({},{})", r, c));
                }
                return match name {
                    "get" => Ok(num(m.borrow()[r][c])),
                    "set" => {
                        m.borrow_mut()[r][c] = arg(2)?;
                        Ok(o.clone())
                    }
                    _ => err(pos, format!("Unknown function : {}", name)),
                };
            }
            Val::Arg(t, v) => {
                return match name {
                    "getType" => Ok(num(*t as f64)),
                    "getValue" => Ok((**v).clone()),
                    _ => err(pos, format!("Unknown function : {}", name)),
                }
            }
            Val::Binary(b) => {
                let data = &b.data[b.offset.min(b.data.len())..];
                let (blk_start, blk_len, fname) = if let Some(rest) = name.strip_prefix("uget").or_else(|| name.strip_prefix("get")) {
                    let rest = rest.strip_suffix("Size").map(|r| (r, true)).unwrap_or((rest, false));
                    match b.blocks.iter().find(|(n, _, _)| n == rest.0) {
                        Some((_, st, ln)) => (*st, *ln, if rest.1 { "size" } else if name.starts_with("uget") { "uget" } else { "get" }),
                        None => return err(pos, format!("Unknown function : {}", name)),
                    }
                } else {
                    return err(pos, format!("Unknown function : {}", name));
                };
                if fname == "size" {
                    return Ok(num(blk_len as f64));
                }
                let k = if a.is_empty() { 0 } else { arg(0)? as i64 };
                if k < 0 || k as usize >= blk_len || blk_start + k as usize >= data.len() {
                    return err(pos, format!("index out of range : {}", k));
                }
                let byte = data[blk_start + k as usize];
                return Ok(num(if fname == "uget" { byte as f64 } else { byte as i8 as f64 }));
            }
            Val::Picture(p) => {
                let (x, y) = (arg(0)? as i64, arg(1)? as i64);
                return match name {
                    "getPixel" => {
                        if x < 0 || x as usize >= p.w {
                            return err(pos, format!("x coordinate out of range : {}", x));
                        }
                        if y < 0 || y as usize >= p.h {
                            return err(pos, format!("y coordinate out of range : {}", y));
                        }
                        Ok(num((p.px[x as usize + y as usize * p.w] & 0xffffff) as f64))
                    }
                    "getSinglecolorByte" | "getMulticolorByte" => {
                        let px = x * 8;
                        if px < 0 || px as usize >= p.w {
                            return err(pos, format!("byte x coordinate out of range : {}", x));
                        }
                        if y < 0 || y as usize >= p.h {
                            return err(pos, format!("y coordinate out of range : {}", y));
                        }
                        let base = px as usize + y as usize * p.w;
                        let get = |d: usize| -> u32 { if px as usize + d >= p.w { p.color(0) } else { p.color(p.px[base + d]) } };
                        let mut r = 0u32;
                        if name == "getSinglecolorByte" {
                            for d in 0..8 {
                                r = (r << 1) | (get(d) & 1);
                            }
                        } else {
                            for d in 0..4 {
                                r = (r << 2) | (get(d * 2) & 3);
                            }
                        }
                        Ok(num(r as f64))
                    }
                    _ => err(pos, format!("Unknown function : {}", name)),
                };
            }
            Val::Sid(sd) => {
                if name == "getData" {
                    let k = arg(0)? as i64;
                    let at = sd.data_offset as i64 + k;
                    if k < 0 || at as usize >= sd.file.len() {
                        return err(pos, format!("Index ({}) out of range", k));
                    }
                    return Ok(num(sd.file[at as usize] as f64));
                }
            }
            Val::OutFile(f) => {
                if name == "writeln" || name == "write" {
                    let t = a.first().map(|x| x.to_str()).unwrap_or_default();
                    let mut b = f.text.borrow_mut();
                    b.push_str(&t);
                    if name == "writeln" {
                        b.push('\n');
                    }
                    return Ok(Val::Null);
                }
            }
            Val::Scope(id) if name == "size" || name == "get" => {
                let it = self.scope_iters(*id);
                if name == "size" {
                    return Ok(num(it.len() as f64));
                }
                let k = arg(0)? as usize;
                return it.get(k).map(|x| Val::Scope(*x)).map_or_else(|| err(pos, format!("Index out of range: {}", k)), Ok);
            }
            _ => {}
        }
        if o.is_unknown() {
            return Ok(Val::Num(0.0, F_UNK, 0));
        }
        err(pos, format!("Unknown function : {}", name))
    }

    pub fn call(&mut self, name: &str, root: bool, a: Vec<Val>, pos: Pos) -> R<Val> {
        if let Some((f, ns)) = self.find_function(name, root, a.len()) {
            return self.call_function(&f, ns, a, pos);
        }
        if let Some(d) = self.find_struct(name, root) {
            let vals = if a.is_empty() {
                vec![Val::Null; d.fields.len()]
            } else if a.len() == d.fields.len() {
                a
            } else {
                return err(pos, format!("The struct '{}' takes {} arguments", d.name, d.fields.len()));
            };
            return Ok(Val::Struct(d, Rc::new(RefCell::new(vals))));
        }
        let unk_arg = a.iter().any(|v| v.flags() & F_UNK != 0);
        let r = self.builtin_call(name, root, a, pos).map_err(|e| trace(e, pos))?;
        // A text made from an unknown number is itself not known yet.
        if unk_arg && matches!(r, Val::Str(..)) {
            self.used_unk = true;
        }
        Ok(r)
    }

    fn builtin_call(&mut self, name: &str, root: bool, a: Vec<Val>, pos: Pos) -> R<Val> {
        let fl = a.iter().fold(0u8, |acc, v| acc | v.flags()) & !F_ZP;
        let n = |k: usize| -> R<f64> {
            match a.get(k) {
                Some(v) => need_num(v, pos),
                None => err(pos, format!("Invalid number of arguments for function '{}'", name)),
            }
        };
        let argc = a.len();
        let m1 = |f: fn(f64) -> f64| -> R<Val> {
            if argc != 1 {
                return err(pos, format!("Invalid number of arguments for function '{}'", name));
            }
            Ok(nf(f(n(0)?), fl))
        };
        match name {
            "abs" => m1(f64::abs),
            "acos" => m1(f64::acos),
            "asin" => m1(f64::asin),
            "atan" => m1(f64::atan),
            "cbrt" => m1(f64::cbrt),
            "ceil" => m1(f64::ceil),
            "cos" => m1(f64::cos),
            "cosh" => m1(f64::cosh),
            "exp" => m1(f64::exp),
            "expm1" => m1(f64::exp_m1),
            "floor" => m1(f64::floor),
            "log" => m1(f64::ln),
            "log10" => m1(f64::log10),
            "log1p" => m1(f64::ln_1p),
            "sin" => m1(f64::sin),
            "sinh" => m1(f64::sinh),
            "sqrt" => m1(f64::sqrt),
            "tan" => m1(f64::tan),
            "tanh" => m1(f64::tanh),
            "toDegrees" => m1(f64::to_degrees),
            "toRadians" => m1(f64::to_radians),
            "signum" => m1(|x| if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { x }),
            "round" => m1(|x| (x + 0.5).floor()),
            "atan2" => Ok(nf(n(0)?.atan2(n(1)?), fl)),
            "hypot" => Ok(nf(n(0)?.hypot(n(1)?), fl)),
            "pow" => Ok(nf(n(0)?.powf(n(1)?), fl)),
            "max" => Ok(nf(n(0)?.max(n(1)?), fl)),
            "min" => Ok(nf(n(0)?.min(n(1)?), fl)),
            "IEEEremainder" => {
                let (x, y) = (n(0)?, n(1)?);
                Ok(nf(x - (x / y).round_ties_even() * y, fl))
            }
            "mod" => {
                let (x, y) = (n(0)? as i32, n(1)? as i32);
                if y == 0 {
                    return err(pos, "Division by zero in mod");
                }
                Ok(nf(x.wrapping_rem(y) as f64, fl))
            }
            "random" => Ok(num(self.random())),
            "setSeed" => {
                self.rng.set((n(0)? as i64 as u64) | 1);
                Ok(Val::Null)
            }
            "toIntString" => {
                let st = (n(0)? as i32).to_string();
                Ok(s(&pad_left(st, if argc > 1 { n(1)? as i64 } else { 0 }, ' ')))
            }
            "toBinaryString" | "toOctalString" | "toHexString" => {
                let x = n(0)? as i32 as u32;
                let st = match name {
                    "toBinaryString" => format!("{:b}", x),
                    "toOctalString" => format!("{:o}", x),
                    _ => format!("{:x}", x),
                };
                Ok(s(&pad_left(st, if argc > 1 { n(1)? as i64 } else { 0 }, '0')))
            }
            "List" => {
                let k = if argc > 0 { n(0)? as usize } else { 0 };
                Ok(new_list(vec![Val::Null; k]))
            }
            "Hashtable" => Ok(Val::Hash(Rc::new(HashV { v: RefCell::new(vec![]), locked: Cell::new(false) }))),
            "Vector" => Ok(Val::Vector([n(0)?, n(1)?, n(2)?])),
            "Matrix" => Ok(new_matrix(ident())),
            "RotationMatrix" => {
                let (ax, ay, az) = (n(0)?, n(1)?, n(2)?);
                let (cx, sx, cy, sy, cz, sz) = (ax.cos(), ax.sin(), ay.cos(), ay.sin(), az.cos(), az.sin());
                Ok(new_matrix([
                    [cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx, 0.0],
                    [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx, 0.0],
                    [-sy, cy * sx, cy * cx, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ]))
            }
            "ScaleMatrix" => {
                let mut m = ident();
                m[0][0] = n(0)?;
                m[1][1] = n(1)?;
                m[2][2] = n(2)?;
                Ok(new_matrix(m))
            }
            "MoveMatrix" => {
                let mut m = ident();
                m[0][3] = n(0)?;
                m[1][3] = n(1)?;
                m[2][3] = n(2)?;
                Ok(new_matrix(m))
            }
            "PerspectiveMatrix" => {
                let mut m = ident();
                m[2][2] = 0.0;
                m[3][2] = 1.0 / n(0)?;
                m[3][3] = 0.0;
                Ok(new_matrix(m))
            }
            "CmdArgument" => Ok(Val::Arg(n(0)? as i32, Box::new(a.get(1).cloned().unwrap_or(Val::Null)))),
            "asmCommandSize" => Ok(num(self.opcode_size(n(0)? as i64) as f64)),
            "getFilename" => Ok(s(&self.src.name(pos.file))),
            "getPath" => Ok(s(&self.src.full(pos.file))),
            "getNamespace" => {
                let nm = self.ns[self.cur_ns].name.clone();
                Ok(s(if nm.is_empty() { "<RootNS>" } else { &nm }))
            }
            "LoadBinary" => {
                if argc == 0 || argc > 2 {
                    return err(pos, format!("Invalid number of arguments for function '{}'", name));
                }
                let data = self.read_file(&a[0].to_str(), pos)?;
                let tpl = if argc > 1 { a[1].to_str() } else { String::new() };
                Ok(Val::Binary(Rc::new(load_binary(data, &tpl, pos)?)))
            }
            "LoadSid" => {
                let data = self.read_file(&a.first().map(|x| x.to_str()).unwrap_or_default(), pos)?;
                Ok(Val::Sid(Rc::new(parse_sid(data, pos)?)))
            }
            "LoadPicture" => {
                if argc == 0 || argc > 2 {
                    return err(pos, format!("Invalid number of arguments for function '{}'", name));
                }
                let fname = a[0].to_str();
                let data = self.read_file(&fname, pos).map_err(|mut e| {
                    e.msg = format!("Can't locate file '{}'", fname);
                    e
                })?;
                let (w, h, px) = match crate::jpeg::decode(&data) {
                    Some(x) => x,
                    None => {
                        let img = image::load_from_memory(&data).or_else(|_| err(pos, format!("Couldnt load image file '{}'", fname)))?;
                        let rgb = img.to_rgba8();
                        let (w, h) = (rgb.width() as usize, rgb.height() as usize);
                        (w, h, rgb.pixels().map(|p| (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32).collect::<Vec<u32>>())
                    }
                };
                let mut table: Vec<(u32, u32)> = vec![];
                if argc > 1 {
                    let list = match &a[1] {
                        Val::List(l) => l.v.borrow().clone(),
                        _ => return err(pos, "The color table must be a list"),
                    };
                    for c in list {
                        let k = (need_num(&c, pos)? as i64 as u32) & 0xffffff;
                        if !table.iter().any(|t| t.0 == k) {
                            let idx = table.len() as u32;
                            table.push((k, idx));
                        }
                    }
                } else {
                    // default table: black first, then the colors in the order they appear
                    table.push((0, 0));
                    for p in &px {
                        if table.len() >= 4 {
                            break;
                        }
                        if !table.iter().any(|t| t.0 == *p) {
                            let idx = table.len() as u32;
                            table.push((*p, idx));
                        }
                    }
                }
                Ok(Val::Picture(Rc::new(Picture { w, h, px, table })))
            }
            "createFile" => {
                let fname = a.first().map(|x| x.to_str()).unwrap_or_default();
                if !self.cfg.afo {
                    return err(pos, "File output is blocked. Use the -afo option to allow it");
                }
                let f = Rc::new(OutFileV { path: self.cfg.out_dir.join(&fname), text: RefCell::new(String::new()) });
                self.out_files.push(f.clone());
                Ok(Val::OutFile(f))
            }
            _ => {
                if self.has_function_name(name, root) {
                    return err(pos, format!("Invalid number of arguments for function '{}'", name));
                }
                if self.find_macro_any(name) {
                    return err(pos, format!("'{}' is a macro and can't be used in an expression", name));
                }
                err(pos, format!("Unknown function '{}'", name))
            }
        }
    }

    fn find_macro_any(&self, name: &str) -> bool {
        self.ns.iter().any(|n| n.macros.contains_key(name))
    }
}

fn ident() -> [[f64; 4]; 4] {
    let mut m = [[0.0; 4]; 4];
    for (k, row) in m.iter_mut().enumerate() {
        row[k] = 1.0;
    }
    m
}

pub fn load_binary(data: Vec<u8>, tpl: &str, pos: Pos) -> R<BinFile> {
    let mut offset = 0;
    let mut blocks: Vec<(String, usize, usize)> = vec![];
    for tok in tpl.split(',') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        if t == "C64FILE" {
            offset = 2;
            continue;
        }
        let (n, v) = match t.split_once('=') {
            Some(x) => x,
            None => return err(pos, format!("Invalid token in template: '{}'", tok)),
        };
        let v = v.trim();
        let st = if let Some(h) = v.strip_prefix('$') { usize::from_str_radix(h, 16) } else { v.parse::<usize>() };
        let st = st.or_else(|_| err(pos, format!("Invalid address of token : '{}'", tok)))?;
        blocks.push((n.trim().to_string(), st, 0));
    }
    blocks.sort_by_key(|b| b.1);
    let total = data.len().saturating_sub(offset);
    for k in 0..blocks.len() {
        let end = if k + 1 < blocks.len() { blocks[k + 1].1 } else { total };
        blocks[k].2 = end.saturating_sub(blocks[k].1);
    }
    blocks.push((String::new(), 0, total));
    Ok(BinFile { data, offset, blocks })
}

/// The number of arguments of a method, observed by running KickAss.jar 5.25.
fn method_arg_count(o: &Val, name: &str) -> Option<usize> {
    let n = match (o, name) {
        (_, "string") => 0,
        (Val::Str(_), "size" | "toLowerCase" | "toUpperCase" | "asBoolean" | "asNumber" | "number") => 0,
        (Val::Str(_), "charAt") => 1,
        (Val::Str(_), "substring") => 2,
        (Val::List(_), "size" | "shuffle" | "reverse" | "sort" | "lock") => 0,
        (Val::List(_), "get" | "addAll" | "remove") => 1,
        (Val::List(_), "set") => 2,
        (Val::Hash(_), "keys" | "lock") => 0,
        (Val::Hash(_), "get" | "containsKey" | "remove") => 1,
        (Val::Vector(_), "getX" | "getY" | "getZ") => 0,
        (Val::Vector(_), "get" | "X") => 1,
        (Val::Matrix(_), "get") => 2,
        (Val::Matrix(_), "set") => 3,
        (Val::Num(..) | Val::Char(..), "number") => 0,
        _ => return None,
    };
    Some(n)
}
