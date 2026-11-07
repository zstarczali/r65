//! R65: a KickAssembler compatible 6502/C64 command line cross assembler.
//!
//! `r65 program.asm -showmem -vicesymbols` assembles on the command line.
//! Without a source file it says so and exits with code 1, like the original.
//! `--gui [file]` opens the optional editor window, `--screenshot <png>` grabs it.

mod asminfo;
mod names;
mod ast;
mod builtins;
mod driver;
mod enc;
mod eval;
mod exec;
mod gui;
mod help;
mod jpeg;
mod lexer;
mod output;
mod parser;
mod preproc;
mod tables;
mod value;

use std::path::PathBuf;

/// extra options from r65.cfg next to the program (one option per line)
fn cfg_args() -> Vec<String> {
    let Ok(exe) = std::env::current_exe() else { return vec![] };
    let Some(dir) = exe.parent() else { return vec![] };
    let Ok(text) = std::fs::read_to_string(dir.join("r65.cfg")) else { return vec![] };
    driver::cfg_text_args(&text)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // window mode only on request: --gui [file] or --screenshot <png> [file]
    let mut shot: Option<PathBuf> = None;
    let mut gui_file: Option<PathBuf> = None;
    let mut gui = false;
    let mut rest = vec![];
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" => {
                gui = true;
                i += 1;
                shot = Some(PathBuf::from(args.get(i).cloned().unwrap_or_else(|| "screenshot.png".into())));
            }
            "--gui" => gui = true,
            "-h" | "--help" | "-help" => {
                let topic = args.get(i + 1).map(|s| s.as_str());
                match help::text(topic) {
                    Some(t) => println!("{}", t),
                    None => {
                        eprintln!("Unknown help topic '{}'. Topics: {}", topic.unwrap_or_default(), help::topic_names());
                        std::process::exit(1);
                    }
                }
                return;
            }
            a => rest.push(a.to_string()),
        }
        i += 1;
    }
    if gui {
        if let Some(f) = rest.iter().find(|a| !a.starts_with('-')) {
            gui_file = Some(PathBuf::from(f));
        }
        if let Err(e) = gui::run(gui_file, shot) {
            eprintln!("Couldn't open the window: {}", e);
            std::process::exit(1);
        }
        return;
    }

    let mut all = cfg_args();
    all.extend(rest);
    let opts = match driver::parse_args(&all) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Error: {}\n\n{}", e, driver::HELP);
            std::process::exit(1);
        }
    };
    if args.is_empty() && opts.file.is_none() {
        println!("{}", help::overview());
        std::process::exit(1);
    }
    let log = driver::Log::new(true);
    let out = driver::assemble(&opts, None, &log);
    if let Some(lf) = &opts.log_file {
        let _ = std::fs::write(lf, log.lines.lock().unwrap().join("\n") + "\n");
    }
    std::process::exit(if out.ok { 0 } else { 1 });
}
