//! Syntax tree for the assembler language.

use std::collections::HashSet;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pos {
    pub file: usize,
    pub line: u32,
    pub col: u32,
}

#[derive(Clone, Debug)]
pub struct E {
    pub k: EK,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub enum EK {
    Num(f64),
    Str(Rc<str>),
    Char(char),
    Bool(bool),
    Null,
    Id { name: String, root: bool },
    Pc,
    Multi(String, i32),
    Un(&'static str, Box<E>),
    Bin(&'static str, Box<E>, Box<E>),
    Tern(Box<E>, Box<E>, Box<E>),
    Call { name: String, root: bool, args: Vec<E> },
    Method { obj: Box<E>, name: String, args: Vec<E> },
    Field { obj: Box<E>, name: String },
    Index { obj: Box<E>, idx: Box<E> },
    Assign { op: &'static str, target: Box<E>, val: Box<E> },
    Post { target: Box<E>, delta: f64 },
    Pre { target: Box<E>, delta: f64 },
    Decl { name: String, root: bool, init: Option<Box<E>> },
    List(Vec<E>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    None,
    Imm,
    Abs,
    AbsX,
    AbsY,
    /// `(e)`
    Ind,
    /// `(e,x)`
    IndX,
    /// `(e),y`
    IndY,
    /// `e1,e2` for the 65c02 bbr/bbs commands
    ZpRel,
}

#[derive(Clone, Debug)]
pub struct Arg {
    pub mode: Mode,
    pub e: Option<E>,
    pub e2: Option<E>,
    /// argument label, as in `ldx tmpX:#$00`
    pub label: Option<String>,
}

pub type Params = Vec<(String, Option<E>, Pos)>;

#[derive(Debug)]
pub struct BlockData {
    pub stmts: Vec<S>,
    /// labels declared directly in this block (visible in the whole block)
    pub labels: HashSet<String>,
}
pub type Block = Rc<BlockData>;

#[derive(Clone, Debug)]
pub struct S {
    pub k: SK,
    pub pos: Pos,
}

#[derive(Debug)]
pub struct FuncDef {
    pub name: String,
    pub root: bool,
    pub params: Vec<String>,
    pub body: Block,
    pub pos: Pos,
}

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImportKind {
    Binary,
    C64,
    Text,
}

#[derive(Clone, Debug)]
pub enum SK {
    Label { name: String, multi: bool, root: bool },
    Instr { mn: String, suffix: Option<String>, arg: Arg },
    PseudoCall { name: String, root: bool, args: Vec<Arg> },
    MacroCall { name: String, root: bool, args: Vec<E> },
    SetPc { e: E, name: Option<E>, virt: bool },
    Data { size: u8, items: Vec<E> },
    Text(Vec<E>),
    Fill { word: bool, count: E, val: E },
    LohiFill { count: E, val: E },
    Align(E),
    Pseudopc(E, Block),
    Var { name: String, root: bool, npos: Pos, init: Option<E> },
    Const { name: String, root: bool, npos: Pos, e: E },
    LabelDef { name: String, root: bool, npos: Pos, e: E, body: Option<Block> },
    Eval(E),
    Enum(Vec<(String, Option<E>, Pos)>),
    Print(E),
    PrintNow(E),
    Error(E),
    ErrorIf(E, E),
    Assert(E, E, E),
    AssertErr(E, E),
    AssertCode(E, Block, Block),
    AssertErrCode(E, Block),
    If(E, Block, Option<Block>),
    For { init: Vec<E>, cond: Option<E>, step: Vec<E>, body: Block },
    While(E, Block),
    Scope(Block),
    Return(Option<E>),
    Function(Rc<FuncDef>),
    Macro(Rc<FuncDef>),
    Pseudo(Rc<FuncDef>),
    Struct(Rc<StructDef>, bool),
    Define(Vec<String>, Block),
    Namespace(String, Block),
    File(usize, Block),
    Import { kind: ImportKind, file: E, off: Option<E>, len: Option<E> },
    Encoding(E),
    Cpu(String),
    Zp(Block),
    Break(Option<E>),
    Watch(Vec<Option<E>>),
    Memblock(E),
    Segment { name: String, params: Option<Params>, block: Option<E> },
    SegmentDef { name: String, params: Params },
    SegmentOut(Params),
    FileOut(Params),
    Disk { params: Params, files: Vec<Params> },
    #[allow(dead_code)] // kept so the parser can read plugin directives and report them
    Modify { name: String, args: Vec<E>, body: Block },
    #[allow(dead_code)]
    FileModify { name: String, args: Vec<E> },
    #[allow(dead_code)]
    Plugin(E),
}
