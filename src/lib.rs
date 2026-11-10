//! R65 as a library: `r65::build` assembles a KickAssembler source in memory, without writing
//! files, and gives the program, the errors and what an editor needs (labels, the source of each
//! byte, the VICE / C64 Debugger files). The `r65` program (`src/main.rs`) uses the same modules.

pub mod asminfo;
pub mod ast;
pub mod builtins;
pub mod driver;
pub mod enc;
pub mod eval;
pub mod exec;
#[cfg(feature = "gui")]
pub mod gui;
pub mod help;
pub mod jpeg;
pub mod lexer;
pub mod names;
pub mod output;
pub mod parser;
pub mod preproc;
pub mod tables;
pub mod value;

use std::path::{Path, PathBuf};

pub use driver::{Extra, MemBlock, VERSION};

/// The command line's -define, :name=value, -libdir and -cpu.
#[derive(Clone, Debug, Default)]
pub struct BuildOptions {
    pub defines: Vec<String>,
    pub cmd_vars: Vec<(String, String)>,
    pub libdirs: Vec<PathBuf>,
    /// "_6502" (default), "_6502NoIllegals", "dtv" or "_65c02"
    pub cpu: Option<String>,
}

#[derive(Clone, Debug)]
pub struct BuildError {
    /// the whole path of the source file ("" when the error has no place)
    pub file: String,
    pub line: u32,
    pub col: u32,
    /// the column where the word at the error ends
    pub end_col: u32,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct Build {
    pub ok: bool,
    /// the Default segment as a program file: load address + bytes (empty without bytes)
    pub prg: Vec<u8>,
    /// the log the command line would print
    pub log: Vec<String>,
    /// every source file (the file index of `errors` and `extra`)
    pub files: Vec<String>,
    pub errors: Vec<BuildError>,
    /// memory blocks of every segment
    pub blocks: Vec<MemBlock>,
    pub passes: u32,
    /// labels, the source of each byte, the debugger files (after a successful build)
    pub extra: Option<Extra>,
}

/// Assembles `source` as if it were the file `path` (#import and the Load… functions read files
/// next to it). Nothing is written.
pub fn build(source: &str, path: &Path, options: &BuildOptions) -> Build {
    let o = driver::Options {
        file: Some(path.to_path_buf()),
        no_output: true,
        extra: true,
        defines: options.defines.clone(),
        cmd_vars: options.cmd_vars.clone(),
        libdirs: options.libdirs.clone(),
        cpu: options.cpu.clone().unwrap_or_else(|| "_6502".into()),
        ..driver::Options::default()
    };
    let log = driver::Log::new(false);
    let out = driver::assemble(&o, Some(source), &log);
    let file_of = |i: usize| out.files.get(i).cloned().unwrap_or_default();
    let mut errors: Vec<BuildError> = out
        .errors
        .iter()
        .map(|(f, line, col, end_col, message)| BuildError { file: file_of(*f), line: *line, col: *col, end_col: *end_col, message: message.clone() })
        .collect();
    if !out.ok && errors.is_empty() {
        let (file, line, col, message) = out.error.clone().unwrap_or_else(|| (String::new(), 0, 0, "Assembly failed".into()));
        errors.push(BuildError { file, line, col, end_col: col, message });
    }
    let prg = match &out.image {
        Some((lo, data)) if out.ok && !data.is_empty() && (0..=0xFFFF).contains(lo) => {
            let mut prg = vec![(*lo & 0xFF) as u8, (*lo >> 8) as u8];
            prg.extend_from_slice(data);
            prg
        }
        _ => vec![],
    };
    let lines = log.lines.lock().map(|l| l.clone()).unwrap_or_default();
    Build { ok: out.ok, prg, log: lines, files: out.files.clone(), errors, blocks: out.blocks.clone(), passes: out.passes, extra: out.extra.clone() }
}
