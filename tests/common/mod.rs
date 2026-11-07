//! Shared helpers: every test runs the real `r65` program in its own temporary folder.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub struct Run {
    pub code: i32,
    pub out: String,
}

impl Run {
    /// The text that a `.print` wrote, as trimmed lines.
    pub fn lines(&self) -> Vec<&str> {
        self.out.lines().map(str::trim).collect()
    }

    pub fn has_line(&self, line: &str) -> bool {
        self.lines().contains(&line)
    }
}

/// A temporary folder, removed when the test ends.
pub struct Project {
    pub dir: PathBuf,
}

impl Project {
    pub fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("r65_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        Project { dir }
    }

    pub fn write(&self, name: &str, data: impl AsRef<[u8]>) {
        let path = self.dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, data).unwrap();
    }

    pub fn read(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.dir.join(name)).unwrap_or_else(|e| panic!("can't read {}: {}", name, e))
    }

    pub fn text(&self, name: &str) -> String {
        String::from_utf8(self.read(name)).unwrap()
    }

    pub fn exists(&self, name: &str) -> bool {
        self.dir.join(name).exists()
    }

    /// Runs `r65` in this folder with the given arguments.
    pub fn run(&self, args: &[&str]) -> Run {
        let o = Command::new(env!("CARGO_BIN_EXE_r65")).args(args).current_dir(&self.dir).output().unwrap();
        let mut out = String::from_utf8_lossy(&o.stdout).into_owned();
        out.push_str(&String::from_utf8_lossy(&o.stderr));
        Run { code: o.status.code().unwrap_or(-1), out }
    }

    /// Writes `t.asm` and assembles it.
    pub fn asm(&self, src: &str, args: &[&str]) -> Run {
        self.write("t.asm", src);
        let mut a = vec!["t.asm"];
        a.extend_from_slice(args);
        self.run(&a)
    }

    /// The files used by the examples in the manuals.
    pub fn fixtures(&self) {
        self.write("data.bin", [1u8, 2, 3, 4, 5]);
        self.write("prog.prg", [0u8, 0x30, 9, 8, 7]);
        self.write("tune.sid", sid());
        self.write("helper.asm", "#importonce\n.print \"helper\"\n");
        self.write("scroll.txt", "abc");
        image::RgbImage::from_pixel(16, 8, image::Rgb([255, 255, 255])).save(self.dir.join("picture.png")).unwrap();
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A minimal PSID file: load and init at $1000, play at $1003, four bytes of music.
pub fn sid() -> Vec<u8> {
    let mut h = vec![0u8; 0x7c];
    h[0..4].copy_from_slice(b"PSID");
    h[5] = 2;
    h[7] = 0x7c;
    h[8] = 0x10;
    h[10] = 0x10;
    h[12] = 0x10;
    h[13] = 3;
    h[15] = 1;
    h[17] = 1;
    h[0x16..0x1a].copy_from_slice(b"Tune");
    h.extend_from_slice(&[0x60, 0x61, 0x62, 0x63]);
    h
}

/// Assembles a source that must succeed and returns the bytes of the program, without the start address.
pub fn bytes(src: &str) -> Vec<u8> {
    let p = Project::new();
    p.fixtures();
    let r = p.asm(src, &[]);
    assert_eq!(r.code, 0, "assembly failed:\n{}\n{}", src, r.out);
    p.read("t.prg")[2..].to_vec()
}

/// Assembles a source that must succeed and returns the program, with the start address.
pub fn prg(src: &str) -> Vec<u8> {
    let p = Project::new();
    let r = p.asm(src, &[]);
    assert_eq!(r.code, 0, "assembly failed:\n{}\n{}", src, r.out);
    p.read("t.prg")
}

/// Checks that a source writes these lines, in this order, with `.print`.
pub fn prints(src: &str, expected: &[&str]) {
    let p = Project::new();
    p.fixtures();
    let r = p.asm(src, &["-nooutput"]);
    assert_eq!(r.code, 0, "assembly failed:\n{}\n{}", src, r.out);
    let lines = r.lines();
    let mut at = 0;
    for e in expected {
        match lines[at..].iter().position(|l| l == e) {
            Some(i) => at += i + 1,
            None => panic!("expected the line {:?} (in order) in:\n{}", e, r.out),
        }
    }
}

/// Checks that a source fails with exit code 1 and this message.
pub fn fails(src: &str, message: &str) {
    let p = Project::new();
    p.fixtures();
    let r = p.asm(src, &["-nooutput"]);
    assert_eq!(r.code, 1, "expected a failure:\n{}\n{}", src, r.out);
    assert!(r.out.contains(message), "expected {:?} in:\n{}", message, r.out);
}
