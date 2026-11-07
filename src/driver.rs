//! Runs one assembly: options, preprocessing, parsing, passes, output and the log.

use crate::ast::Pos;
use crate::eval::{all_mnemonics, Asm, AsmError, Config};
use crate::lexer::lex;
use crate::output;
use crate::parser::Parser;
use crate::preproc::Pre;
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

pub const APP_NAME: &str = "R65";
pub const DESCRIPTION: &str = "KickAssembler compatible compiler for C64";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COPYRIGHT: &str = "(C) 2026 Zsolt Tarczali";

/// Our own start-up macros (the same job as the original's built-in include file).
pub fn auto_include() -> String {
    format!(
        r#"// Built-in macros
// BasicUpstart(address): a one line BASIC program "10 SYS address"
.macro BasicUpstart(address) {{
    .word upstartEnd
    .word 10
    .byte $9e
    .text toIntString(address)
    .byte 0
upstartEnd:
    .word 0
}}
// BasicUpstart2(address): the same, placed at the start of BASIC in its own memory block
.macro BasicUpstart2(address) {{
    * = {basic} "Basic"
    .word upstartEnd
    .word 10
    .byte $9e
    .text toIntString(address)
    .byte 0
upstartEnd:
    .word 0
    * = {end} "Basic End"
}}
"#,
        basic = crate::eval::BASIC_START,
        end = crate::eval::BASIC_END_BLOCK
    )
}

#[derive(Clone, Debug)]
pub struct Options {
    pub file: Option<PathBuf>,
    pub show_mem: bool,
    pub vice_symbols: bool,
    pub symbol_file: bool,
    pub symbol_dir: Option<PathBuf>,
    pub byte_dump: Option<String>,
    pub out_file: Option<String>,
    pub out_dir: Option<PathBuf>,
    pub binfile: bool,
    pub defines: Vec<String>,
    pub libdirs: Vec<PathBuf>,
    pub fill_byte: u8,
    pub max_addr: i64,
    pub mbfiles: bool,
    pub no_output: bool,
    pub no_eval: bool,
    pub allow_overlap: bool,
    pub afo: bool,
    pub time: bool,
    pub warnings: bool,
    pub cpu: String,
    pub cmd_vars: Vec<(String, String)>,
    pub execute: Option<String>,
    pub log_file: Option<PathBuf>,
    pub replace: Vec<(PathBuf, PathBuf)>,
    pub max_diag: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            file: None,
            show_mem: false,
            vice_symbols: false,
            symbol_file: false,
            symbol_dir: None,
            byte_dump: None,
            out_file: None,
            out_dir: None,
            binfile: false,
            defines: vec![],
            libdirs: vec![],
            fill_byte: 0,
            max_addr: 65535,
            mbfiles: false,
            no_output: false,
            no_eval: false,
            allow_overlap: false,
            afo: false,
            time: false,
            warnings: true,
            cpu: "_6502".into(),
            cmd_vars: vec![],
            execute: None,
            log_file: None,
            replace: vec![],
            max_diag: 5,
        }
    }
}

pub const HELP: &str = "Usage: rust65 <file.asm> [options]
  (a command line tool; --gui [file] opens an optional editor window)

  -o <file>            name of the output file
  -odir <dir>          folder for the output files
  -binfile             write a raw .bin file without the 2 byte start address
  -mbfiles             write one file per memory block
  -nooutput            don't write any files
  -showmem             print the memory map
  -vicesymbols         write a VICE label file (.vs)
  -symbolfile          write a symbol file (.sym) that can be imported
  -symbolfiledir <dir> folder for the symbol file
  -bytedump            write ByteDump.txt (address, bytes and source line)
  -bytedumpfile <file> the same with a file name
  -define <name>       define a preprocessor symbol
  -libdir <dir>        extra folder to search for imported files
  -fillbyte <n>        byte used for gaps between memory blocks
  -maxaddr <n>         accepted but not enforced, like the original
  -aom                 allow memory blocks to overlap
  -afo                 allow scripts to write files (createFile)
  -excludeillegal      don't accept illegal 6502 opcodes
  -dtv                 accept the DTV opcodes
  -cpu <name>          _6502NoIllegals, _6502, dtv or _65c02
  -warningsoff         don't print warnings
  -noeval              only parse the source
  -time                print how long it took
  -log <file>          also write the log to a file
  -execute <cmd>       run a program with the output file on success
  -replacefile <a> <b> use file b wherever file a is imported
  -maxdiag <n>         how many errors/warnings to list (default 5)
  :name=value          a value for the script hashtable cmdLineVars
  --gui [file]         open the editor window
  --screenshot <png>   open the window, save a picture of it and quit
  -h, --help [topic]   this help; rust65 --help lists all topics";

/// Parses command line options. Returns the options or an error text.
pub fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut o = Options::default();
    let mut i = 0;
    let need = |i: &mut usize, name: &str| -> Result<String, String> {
        *i += 1;
        args.get(*i).cloned().ok_or_else(|| format!("Missing argument for option '{}'", name))
    };
    let num = |s: &str| -> Result<i64, String> {
        let t = s.trim();
        let r = if let Some(h) = t.strip_prefix('$') {
            i64::from_str_radix(h, 16)
        } else if let Some(h) = t.strip_prefix("0x") {
            i64::from_str_radix(h, 16)
        } else {
            t.parse()
        };
        r.map_err(|_| format!("Not a valid number : {}", s))
    };
    while i < args.len() {
        let a = args[i].clone();
        let low = a.to_lowercase();
        if let Some(kv) = a.strip_prefix(':') {
            match kv.split_once('=') {
                Some((k, v)) => o.cmd_vars.push((k.to_string(), v.to_string())),
                None => return Err(format!("Invalid user argument '{}' (use :name=value)", a)),
            }
        } else if !a.starts_with('-') {
            if o.file.is_some() {
                return Err(format!("Only one source file can be given ('{}')", a));
            }
            o.file = Some(PathBuf::from(&a));
        } else {
            match low.as_str() {
                "-o" => o.out_file = Some(need(&mut i, &a)?),
                "-odir" => o.out_dir = Some(PathBuf::from(need(&mut i, &a)?)),
                "-binfile" => o.binfile = true,
                "-mbfiles" => o.mbfiles = true,
                "-nooutput" => o.no_output = true,
                "-showmem" => o.show_mem = true,
                "-vicesymbols" => o.vice_symbols = true,
                "-symbolfile" => o.symbol_file = true,
                "-symbolfiledir" => {
                    o.symbol_file = true;
                    o.symbol_dir = Some(PathBuf::from(need(&mut i, &a)?))
                }
                "-bytedump" => o.byte_dump = Some("ByteDump.txt".into()),
                "-bytedumpfile" => o.byte_dump = Some(need(&mut i, &a)?),
                "-define" => o.defines.push(need(&mut i, &a)?),
                "-libdir" => o.libdirs.push(PathBuf::from(need(&mut i, &a)?)),
                "-fillbyte" => o.fill_byte = num(&need(&mut i, &a)?)? as u8,
                "-maxaddr" => o.max_addr = num(&need(&mut i, &a)?)?,
                "-aom" => o.allow_overlap = true,
                "-afo" => o.afo = true,
                "-excludeillegal" => o.cpu = "_6502NoIllegals".into(),
                "-dtv" => o.cpu = "dtv".into(),
                "-cpu" => {
                    let c = need(&mut i, &a)?;
                    if !["_6502NoIllegals", "_6502", "dtv", "_65c02"].contains(&c.as_str()) {
                        return Err(format!("Unknown cpu '{}'", c));
                    }
                    o.cpu = c
                }
                "-warningsoff" => o.warnings = false,
                "-noeval" => o.no_eval = true,
                "-time" => o.time = true,
                "-log" => o.log_file = Some(PathBuf::from(need(&mut i, &a)?)),
                "-execute" => o.execute = Some(need(&mut i, &a)?),
                "-replacefile" => {
                    let x = need(&mut i, &a)?;
                    let y = need(&mut i, &a)?;
                    o.replace.push((PathBuf::from(x), PathBuf::from(y)))
                }
                "-maxdiag" => o.max_diag = num(&need(&mut i, &a)?)?.max(1) as usize,
                _ => return Err(format!("Unknown option '{}'", a)),
            }
        }
        i += 1;
    }
    Ok(o)
}

#[derive(Clone)]
pub struct Log {
    pub lines: Rc<RefCell<Vec<String>>>,
    pub echo: bool,
}

impl Log {
    pub fn new(echo: bool) -> Self {
        Log { lines: Rc::new(RefCell::new(vec![])), echo }
    }
    pub fn say(&self, s: &str) {
        if self.echo {
            println!("{}", s);
        }
        self.lines.borrow_mut().push(s.to_string());
    }
}

#[derive(Clone, Debug)]
pub struct MemBlock {
    pub segment: String,
    pub name: String,
    pub start: i64,
    pub len: usize,
    pub virt: bool,
}

#[derive(Default)]
pub struct Outcome {
    pub ok: bool,
    pub error: Option<(String, u32, u32, String)>,
    pub blocks: Vec<MemBlock>,
    pub image: Option<(i64, Vec<u8>)>,
    pub passes: u32,
}

/// file names in messages are shown relative to the main source file's folder
fn rel_name(p: &str, base: &Path) -> String {
    let b = base.to_string_lossy().to_string();
    if !b.is_empty() && p.starts_with(&b) && p.len() > b.len() + 1 {
        return p[b.len() + 1..].to_string();
    }
    p.to_string()
}

fn pos_string(asm: &Asm, p: &Pos) -> String {
    format!("at line {}, column {} in {}", p.line, p.col, rel_name(&asm.src.full(p.file), &asm.cfg.src_dir))
}

/// The error text: a few source lines, a ^ under the column, the message and the call trace.
pub fn error_text(asm: &Asm, e: &AsmError) -> String {
    let Some(p) = e.pos else {
        return format!("Error: {}\n", e.msg);
    };
    let mut s = String::new();
    let lines = asm.src.files.get(p.file).map(|f| f.lines.clone()).unwrap_or_default();
    let line = p.line as usize;
    let from = line.saturating_sub(3).max(1);
    for l in from..=line {
        if let Some(t) = lines.get(l - 1) {
            s.push_str(&t.trim_end_matches('\r').replace('\t', " "));
            s.push('\n');
        }
    }
    s.push_str(&" ".repeat(p.col.saturating_sub(1) as usize));
    s.push_str("^\n\n");
    s.push_str(&format!("Error: {}\n", e.msg));
    s.push_str(&pos_string(asm, &p));
    s.push('\n');
    for c in &e.called {
        s.push_str(&format!("called {}\n", pos_string(asm, c)));
    }
    s
}

/// The start-up text: name, description, version and copyright, centred in 54 columns.
pub fn banner_lines() -> Vec<String> {
    let line = format!("//{}", "-".repeat(54));
    let mut out = vec![line.clone(), line.clone()];
    let texts = [format!("{} - {}", APP_NAME, DESCRIPTION), format!("Version {}", VERSION), COPYRIGHT.to_string()];
    for t in &texts {
        let pad = 54usize.saturating_sub(t.len());
        out.push(format!("//{}{}{}", " ".repeat(pad / 2), t, " ".repeat(pad - pad / 2)));
    }
    out.push(line.clone());
    out.push(line);
    out
}

fn banner(log: &Log) {
    for l in banner_lines() {
        log.say(&l);
    }
}

/// Assembles `o.file` (or `text` when given, using `o.file` as its name).
pub fn assemble(o: &Options, text: Option<&str>, log: &Log) -> Outcome {
    let t0 = Instant::now();
    let mut out = Outcome::default();
    banner(log);
    let Some(file) = o.file.clone() else {
        log.say("Error: No input file given.");
        log.say("");
        return out;
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let abs = if file.is_absolute() { file.clone() } else { cwd.join(&file) };
    let src_dir = abs.parent().map(|p| p.to_path_buf()).unwrap_or(cwd.clone());
    let out_dir = match &o.out_dir {
        Some(d) if d.is_absolute() => d.clone(),
        Some(d) => cwd.join(d),
        None => src_dir.clone(),
    };
    log.say(&format!("Output dir: {}", out_dir.display()));
    if !o.cmd_vars.is_empty() {
        let parts: Vec<String> = o.cmd_vars.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        log.say(&format!("User Arguments: {}", parts.join(", ")));
    }
    log.say("parsing");
    let source = match text {
        Some(t) => t.to_string(),
        None => match std::fs::read(&abs) {
            Ok(b) => String::from_utf8_lossy(&b).to_string(),
            Err(_) => {
                log.say(&format!("Error: Can't open file '{}'", file.display()));
                out.error = Some((abs.to_string_lossy().to_string(), 0, 0, format!("Can't open file '{}'", file.display())));
                return out;
            }
        },
    };
    let src_name = abs.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let base = Path::new(&src_name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let cfg = Config {
        defines: o.defines.clone(),
        libdirs: o.libdirs.iter().map(|d| if d.is_absolute() { d.clone() } else { cwd.join(d) }).collect(),
        cmd_vars: o.cmd_vars.clone(),
        fill_byte: o.fill_byte,
        max_addr: o.max_addr,
        allow_overlap: o.allow_overlap,
        afo: o.afo,
        warnings: o.warnings,
        cpu: o.cpu.clone(),
        src_dir: src_dir.clone(),
        out_dir: out_dir.clone(),
        src_name: src_name.clone(),
        no_output: o.no_output,
        binfile: o.binfile,
    };
    let l2 = log.clone();
    let mut asm = Asm::new(cfg.clone(), Box::new(move |s: &str| l2.say(s)));

    let fail = |asm: &Asm, e: AsmError, out: &mut Outcome, early: bool| {
        if early || e.pos.is_none() {
            log.say("");
        }
        for l in error_text(asm, &e).lines() {
            log.say(l);
        }
        log.say("");
        if e.pos.is_some() {
            log.say("");
        }
        let (f, l, c) = match e.pos {
            Some(p) => (asm.src.full(p.file), p.line, p.col),
            None => (String::new(), 0, 0),
        };
        out.error = Some((f, l, c, e.msg.clone()));
    };

    // preprocess the built-in macros and the source
    let pre_res = {
        let mut pre = Pre::new(&mut asm.src, o.defines.iter().cloned().collect::<HashSet<_>>(), cfg.libdirs.clone());
        pre.replace = o.replace.clone();
        let r = pre.run_text(PathBuf::from("rust65:autoinclude.asm"), &auto_include(), 0).and_then(|_| pre.run_text(abs.clone(), &source, 0));
        r.map(|_| std::mem::take(&mut pre.out))
    };
    let lines = match pre_res {
        Ok(l) => l,
        Err(e) => {
            fail(&asm, AsmError { pos: Some(Pos { file: e.file, line: e.line, col: 1 }), msg: e.msg, called: vec![] }, &mut out, true);
            return out;
        }
    };
    let toks = match lex(&lines) {
        Ok(t) => t,
        Err(e) => {
            fail(&asm, AsmError { pos: Some(Pos { file: e.file, line: e.line, col: e.col }), msg: e.msg, called: vec![] }, &mut out, true);
            return out;
        }
    };
    let mn = all_mnemonics();
    let prog = match Parser::new(&toks, &mn).program() {
        Ok(p) => p,
        Err(e) => {
            fail(&asm, AsmError { pos: Some(e.pos), msg: e.msg, called: vec![] }, &mut out, true);
            return out;
        }
    };
    if o.no_eval {
        log.say("Preparse done and no evaluation requested. Exiting.");
        out.ok = true;
        return out;
    }
    let t_parse = t0.elapsed();
    match asm.run(prog) {
        Ok(n) => out.passes = n,
        Err(e) => {
            let early = asm.early_error;
            fail(&asm, e, &mut out, early);
            return out;
        }
    }
    let t_pass = t0.elapsed();
    log.say("Output pass");
    for p in asm.prints.clone() {
        log.say(&format!("  {}", p));
    }
    if let Err(e) = asm.check_overlap() {
        fail(&asm, e, &mut out, false);
        return out;
    }
    // collect the memory blocks for the window
    for sg in &asm.segs {
        for b in &sg.blocks {
            if !b.bytes.is_empty() || b.named {
                out.blocks.push(MemBlock { segment: sg.name.clone(), name: b.name.clone(), start: b.start, len: b.bytes.len(), virt: b.virt });
            }
        }
    }
    if let Ok(bl) = asm.segment_blocks("Default", Pos::default(), false, 0) {
        let (lo, data) = output::flatten(&bl, o.fill_byte);
        out.image = Some((lo, data));
    }
    let out_file = match &o.out_file {
        Some(f) if Path::new(f).is_absolute() => f.clone(),
        Some(f) => cwd.join(f).to_string_lossy().to_string(),
        None => format!("{}{}", base, if o.binfile { ".bin" } else { ".prg" }),
    };
    let oo = output::OutOpts { out_file: PathBuf::from(&out_file), binfile: o.binfile, mbfiles: o.mbfiles, fill_byte: o.fill_byte, dry: o.no_output };
    match output::write_outputs(&mut asm, &oo) {
        Ok(lines) => {
            for l in lines {
                log.say(&l);
            }
        }
        Err(e) => {
            fail(&asm, e, &mut out, false);
            return out;
        }
    }
    if o.show_mem {
        for l in output::memory_map(&asm).lines() {
            log.say(l);
        }
    }
    if o.warnings && !asm.warnings.is_empty() {
        log.say(&format!("Got 0 errors and {} warnings while executing:", asm.warnings.len()));
        for w in asm.warnings.iter().take(o.max_diag) {
            if w.pos.line > 0 {
                log.say(&format!("  ({} {}:{}) Warning: {}", asm.src.full(w.pos.file), w.pos.line, w.pos.col, w.msg));
            } else {
                log.say(&format!("  Warning: {}", w.msg));
            }
        }
        if asm.warnings.len() > o.max_diag {
            log.say("  ...");
        }
        log.say("");
    }
    if let Some(bd) = &o.byte_dump {
        if !o.no_output {
            let txt = output::byte_dump(&asm, Some(0));
            let p = if Path::new(bd).is_absolute() { PathBuf::from(bd) } else { out_dir.join(bd) };
            if let Err(e) = std::fs::write(&p, txt) {
                log.say(&format!("Error: Can't write '{}': {}", p.display(), e));
            }
        }
    }
    if o.time {
        log.say(&format!("Parse time  = {} ms", t_parse.as_millis()));
        log.say(&format!("Evaluations passes time  = {} ms", (t_pass - t_parse).as_millis()));
        log.say(&format!("Total assemble time = {} ms", t0.elapsed().as_millis()));
    }
    let (made, failed) = asm.asserts;
    if made > 0 {
        log.say("");
        log.say(&format!("Made {} asserts , {}{}", made, failed, if failed == 0 { " failed" } else { " FAILED!" }));
    }
    if o.vice_symbols {
        let parent = Path::new(&out_file).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let name = if parent.is_empty() { format!("{}.vs", base) } else { format!("{}{}{}.vs", parent, std::path::MAIN_SEPARATOR, base) };
        log.say(&format!("Writing Vice symbol file: {}", name));
        if !o.no_output {
            let _ = std::fs::write(out_dir.join(&name), output::vice_symbols(&asm));
        }
    }
    if o.symbol_file {
        let (shown, path) = match &o.symbol_dir {
            Some(d) => {
                let s = format!("{}{}{}.sym", d.display(), std::path::MAIN_SEPARATOR, base);
                (s.clone(), if d.is_absolute() { PathBuf::from(&s) } else { cwd.join(&s) })
            }
            None => (format!("{}.sym", base), out_dir.join(format!("{}.sym", base))),
        };
        log.say(&format!("Writing Symbol file: {}", shown));
        if !o.no_output {
            let _ = std::fs::write(path, output::sym_file(&asm));
        }
    }
    out.ok = true;
    if let Some(cmd) = &o.execute {
        if !o.no_output {
            let target = out_dir.join(&out_file);
            let mut parts = cmd.split_whitespace();
            if let Some(prog) = parts.next() {
                let mut c = std::process::Command::new(prog);
                c.args(parts).arg(&target);
                if let Err(e) = c.spawn() {
                    log.say(&format!("Error: Couldn't run '{}': {}", cmd, e));
                }
            }
        }
    }
    out
}
