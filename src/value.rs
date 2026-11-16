//! Script values.

use crate::ast::StructDef;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// value depends on a label that was not known at all yet
pub const F_UNK: u8 = 1;
/// value depends on a forward reference (taken from the previous pass)
pub const F_FWD: u8 = 2;
/// value is bound to the zeropage (.zp)
pub const F_ZP: u8 = 4;

#[derive(Debug)]
pub struct ListV {
    pub v: RefCell<Vec<Val>>,
    pub locked: Cell<bool>,
}

#[derive(Debug)]
pub struct HashV {
    pub v: RefCell<Vec<(String, Val, Val)>>,
    pub locked: Cell<bool>,
}

#[derive(Debug)]
pub struct BinFile {
    pub data: Vec<u8>,
    pub offset: usize,
    /// (name, start, length) relative to the offset
    pub blocks: Vec<(String, usize, usize)>,
}

#[derive(Debug)]
pub struct Picture {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u32>,
    pub table: Vec<(u32, u32)>,
}

impl Picture {
    pub fn color(&self, rgb: u32) -> u32 {
        let k = rgb & 0xffffff;
        for (c, i) in &self.table {
            if *c == k {
                return *i;
            }
        }
        0
    }
}

#[derive(Debug)]
pub struct SidFile {
    pub file: Vec<u8>,
    pub data_offset: usize,
    pub location: u32,
}

#[derive(Debug)]
pub struct OutFileV {
    pub path: std::path::PathBuf,
    pub text: RefCell<String>,
}

#[derive(Clone, Debug)]
pub enum Val {
    /// number, flags, attached label scope (id+1, 0 = none)
    Num(f64, u8, u32),
    Char(char, f64),
    Bool(bool, u8),
    Str(Rc<str>),
    Null,
    List(Rc<ListV>),
    Hash(Rc<HashV>),
    Struct(Rc<StructDef>, Rc<RefCell<Vec<Val>>>),
    Vector([f64; 3]),
    /// shared like lists: `.set` changes the matrix itself
    Matrix(Rc<RefCell<[[f64; 4]; 4]>>),
    /// a command argument (addressing mode id, value)
    Arg(i32, Box<Val>),
    /// a scope (namespace, or a scope without a label value)
    Scope(u32),
    Binary(Rc<BinFile>),
    Picture(Rc<Picture>),
    Sid(Rc<SidFile>),
    OutFile(Rc<OutFileV>),
}

pub fn num(v: f64) -> Val {
    Val::Num(v, 0, 0)
}
pub fn s(v: &str) -> Val {
    Val::Str(Rc::from(v))
}

/// Java's Double.toString, with whole numbers printed without decimals.
pub fn num_str(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if v == v.trunc() && v >= i32::MIN as f64 && v <= i32::MAX as f64 {
        return format!("{}", v as i32);
    }
    java_double(v)
}

pub fn java_double(v: f64) -> String {
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let a = v.abs();
    if (1e-3..1e7).contains(&a) {
        let mut r = format!("{}", v);
        if !r.contains('.') {
            r.push_str(".0");
        }
        r
    } else {
        let e = format!("{:e}", v);
        let (m, ex) = e.split_once('e').unwrap();
        let m = if m.contains('.') { m.to_string() } else { format!("{}.0", m) };
        format!("{}E{}", m, ex)
    }
}

impl Val {
    pub fn flags(&self) -> u8 {
        match self {
            Val::Num(_, f, _) => *f,
            Val::Bool(_, f) => *f,
            Val::Arg(_, v) => v.flags(),
            _ => 0,
        }
    }
    pub fn with_flags(self, add: u8) -> Val {
        if add == 0 {
            return self;
        }
        match self {
            Val::Num(v, f, _) => Val::Num(v, f | add, 0),
            Val::Char(_, v) => Val::Num(v, add, 0),
            Val::Bool(b, f) => Val::Bool(b, f | add),
            other => other,
        }
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Val::Num(..) => "Number",
            Val::Char(..) => "Char",
            Val::Bool(..) => "boolean",
            Val::Str(_) => "String",
            Val::Null => "Null",
            Val::List(_) => "List",
            Val::Hash(_) => "Hashtable",
            Val::Struct(..) => "Struct",
            Val::Vector(_) => "Vector",
            Val::Matrix(_) => "Matrix",
            Val::Arg(..) => "65xxArgument",
            Val::Scope(_) => "Scope",
            Val::Binary(_) => "BinaryFile",
            Val::Picture(_) => "Picture",
            Val::Sid(_) => "SID file",
            Val::OutFile(_) => "OutputFile",
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Val::Num(v, _, _) => Some(*v),
            Val::Char(_, v) => Some(*v),
            _ => None,
        }
    }
    pub fn is_unknown(&self) -> bool {
        self.flags() & F_UNK != 0
    }
    /// the text form used by .print, string concatenation and hashtable keys
    pub fn to_str(&self) -> String {
        match self {
            Val::Num(v, f, _) => {
                if f & F_UNK != 0 {
                    "<<Invalid Number>>".into()
                } else {
                    num_str(*v)
                }
            }
            Val::Char(c, _) => {
                let u = *c as u32;
                if (0x800..0x900).contains(&u) {
                    ((u - 0x800) as u8 as char).to_string()
                } else {
                    c.to_string()
                }
            }
            Val::Bool(b, f) => {
                if f & F_UNK != 0 {
                    "<<Invalid Boolean>>".into()
                } else {
                    b.to_string()
                }
            }
            Val::Str(s) => s.to_string(),
            Val::Null => "Null".into(),
            Val::List(l) => {
                let v = l.v.borrow();
                let parts: Vec<String> = v.iter().map(|x| x.to_str()).collect();
                format!("[{}]", parts.join(","))
            }
            Val::Hash(_) => "Hashtable".into(),
            Val::Struct(d, _) => format!("{}-struct", d.name),
            Val::Vector(v) => format!("({},{},{})", java_double(v[0]), java_double(v[1]), java_double(v[2])),
            Val::Matrix(m) => {
                let rows: Vec<String> = m
                    .borrow()
                    .iter()
                    .map(|r| format!("[{}]", r.iter().map(|x| java_double(*x)).collect::<Vec<_>>().join(",")))
                    .collect();
                format!("[{}]", rows.join(","))
            }
            Val::Arg(t, _) => format!("Argument<{}>", arg_type_name(*t)),
            Val::Scope(_) => "Scope".into(),
            Val::Binary(_) => "BinaryFile".into(),
            Val::Picture(_) => "Picture".into(),
            Val::Sid(s) => format!("SID<\"{}\">", sid_string(&s.file, 22, 32)),
            Val::OutFile(_) => "OutputFile".into(),
        }
    }
}

pub fn sid_string(f: &[u8], at: usize, len: usize) -> String {
    let mut s = String::new();
    for i in 0..len {
        let b = *f.get(at + i).unwrap_or(&0);
        if b == 0 {
            break;
        }
        s.push(b as char);
    }
    s
}

pub fn sid_int(f: &[u8], at: usize, len: usize) -> u32 {
    let mut v = 0u32;
    for i in 0..len {
        v = (v << 8) | *f.get(at + i).unwrap_or(&0) as u32;
    }
    v
}

// command argument type ids (same numbers as the original)
pub const AT_NONE: i32 = 0;
pub const AT_IMMEDIATE: i32 = 1;
pub const AT_ZEROPAGE: i32 = 2;
pub const AT_IZEROPAGEX: i32 = 5;
pub const AT_IZEROPAGEY: i32 = 6;
pub const AT_ABSOLUTE: i32 = -1;
pub const AT_ABSOLUTEX: i32 = -2;
pub const AT_ABSOLUTEY: i32 = -3;
pub const AT_INDIRECT: i32 = -4;
pub const AT_INDIRECTX: i32 = -5;

pub fn arg_type_name(t: i32) -> &'static str {
    match t {
        AT_NONE => "NONE",
        AT_IMMEDIATE => "IMMEDIATE",
        AT_ZEROPAGE => "ZEROPAGE",
        3 => "ZEROPAGEX",
        4 => "ZEROPAGEY",
        AT_IZEROPAGEX => "IZEROPAGEX",
        AT_IZEROPAGEY => "IZEROPAGEY",
        7 => "ABSOLUTE",
        8 => "ABSOLUTEX",
        9 => "ABSOLUTEY",
        10 => "INDIRECT",
        AT_ABSOLUTE => "*ABSOLUTE",
        AT_ABSOLUTEX => "*ABSOLUTEX",
        AT_ABSOLUTEY => "*ABSOLUTEY",
        AT_INDIRECT => "*INDIRECT",
        AT_INDIRECTX => "*INDIRECTX",
        _ => "?",
    }
}
