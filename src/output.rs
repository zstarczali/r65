//! Everything written after the passes: program files, memory map,
//! symbol files, the byte dump and disk images.

use crate::ast::Pos;
use crate::eval::*;
use crate::value::*;
use std::path::{Path, PathBuf};

/// Joins the real (non virtual) blocks into one memory image.
/// Returns the lowest address and the bytes; gaps get the fill byte.
pub fn flatten(blocks: &[Blk], fill: u8) -> (i64, Vec<u8>) {
    let real: Vec<&Blk> = blocks.iter().filter(|b| !b.virt && !b.bytes.is_empty()).collect();
    if real.is_empty() {
        return (0, vec![]);
    }
    let lo = real.iter().map(|b| b.start).min().unwrap();
    let hi = real.iter().map(|b| b.start + b.bytes.len() as i64).max().unwrap();
    let mut out = vec![fill; (hi - lo) as usize];
    for b in real {
        let o = (b.start - lo) as usize;
        out[o..o + b.bytes.len()].copy_from_slice(&b.bytes);
    }
    (lo, out)
}

pub fn file_bytes(blocks: &[Blk], fill: u8, prg: bool) -> Vec<u8> {
    let (lo, data) = flatten(blocks, fill);
    let mut out = vec![];
    if prg {
        out.push((lo & 0xff) as u8);
        out.push(((lo >> 8) & 0xff) as u8);
    }
    out.extend(data);
    out
}

fn hex(v: i64) -> String {
    format!("{:x}", v as i32)
}

// ------------------------------------------------------------ memory map
pub fn memory_map(asm: &mut Asm) -> String {
    let mut parts = vec![];
    for i in 0..asm.segs.len() {
        let p = asm.segs[i].p.clone();
        if p.hide {
            continue;
        }
        // all blocks, also empty ones, by start address (SegmentManager.printMemoryBlocks),
        // and the files the segment loads; a file that can't be read is reported where it's used
        let from_files = asm.file_blocks(&p, p.pos).unwrap_or_default();
        let sg = &asm.segs[i];
        let mut bl: Vec<&Blk> = sg.blocks.iter().chain(from_files.iter()).collect();
        if bl.is_empty() {
            continue;
        }
        bl.sort_by_key(|b| b.start);
        let mut s = format!("{}-segment:\n", sg.name);
        for b in bl {
            s.push_str(&format!(
                "  {}${:04x}-${:04x} {}\n",
                if b.virt { "*" } else { "" },
                b.start,
                b.start + b.bytes.len() as i64 - 1,
                b.name
            ));
        }
        parts.push(s);
    }
    format!("\nMemory Map\n----------\n{}\n", parts.join("\n"))
}

// ------------------------------------------------------------ symbol files
fn label_addr(v: &Val) -> Option<i64> {
    match v {
        Val::Num(x, f, _) if f & F_UNK == 0 => Some(*x as i64),
        _ => None,
    }
}

const HASHMAP_INITIAL_CAPACITY: usize = 16;
const HASHMAP_LOAD_FACTOR: f64 = 0.75;

/// Bucket of a string key in a Java HashMap of the given (power of two) capacity.
fn java_bucket(s: &str, cap: usize) -> usize {
    let mut h: i32 = 0;
    for u in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u as i32);
    }
    let h = h as u32;
    ((h ^ (h >> 16)) as usize) & (cap - 1)
}

pub fn vice_symbols(asm: &Asm) -> String {
    // name -> addresses, in the order first seen
    let mut names: Vec<(String, Vec<i64>)> = vec![];
    let mut seen: std::collections::HashSet<(u32, &str)> = std::collections::HashSet::new();
    let placed = asm.label_order.iter().map(|(s, n)| (*s, n.as_str()));
    let rest = asm
        .scopes
        .iter()
        .enumerate()
        .flat_map(|(i, sc)| sc.order.iter().map(move |n| (i as u32, n.as_str())));
    for (si, n) in placed.chain(rest) {
        if !seen.insert((si, n)) {
            continue;
        }
        {
            let Some(sy) = asm.scopes.get(si as usize).and_then(|sc| sc.syms.get(n)) else { continue };
            if sy.kind != SymKind::Label {
                continue;
            }
            if let Some(a) = label_addr(&sy.val) {
                let n = n.trim_start_matches('@').to_string();
                match names.iter_mut().find(|e| e.0 == n) {
                    Some(e) => e.1.push(a),
                    None => names.push((n, vec![a])),
                }
            }
        }
    }
    // ViceSymbolFile.writeSymbols groups names in a java.util.HashMap and writes them in its
    // iteration order: by bucket (String hash spread, masked by the table size), then insertion.
    let mut cap = HASHMAP_INITIAL_CAPACITY;
    while names.len() as f64 > cap as f64 * HASHMAP_LOAD_FACTOR {
        cap *= 2;
    }
    names.sort_by_key(|(n, _)| java_bucket(n, cap));
    let mut out = String::new();
    for (n, addrs) in &names {
        let many = addrs.len() > 1;
        for (i, a) in addrs.iter().enumerate() {
            if many {
                out.push_str(&format!("al C:{} .{}__{}\n", hex(*a), n, i));
            } else {
                out.push_str(&format!("al C:{} .{}\n", hex(*a), n));
            }
        }
    }
    for b in &asm.breaks {
        out.push_str(&format!("break {} {}\n", hex(b.addr), b.arg.clone().unwrap_or_default()));
    }
    for w in &asm.watches {
        let a2 = w.a2.map(|x| format!("{} ", hex(x))).unwrap_or_default();
        let arg = w.arg.clone().map(|x| format!("{} ", x)).unwrap_or_default();
        out.push_str(&format!("watch {}{} {}\n", arg, hex(w.a1), a2));
    }
    out
}

const NL: &str = "\r\n";

/// SymbolFile.writeScope: the names of a scope in the order of a Java HashMap, namespaces among
/// them. The body of a macro or function call stays empty (it has its own symbol table).
fn sym_scope(asm: &Asm, id: u32, depth: usize, out: &mut String, guard: &mut Vec<u32>) {
    if guard.contains(&id) || depth > 32 {
        return;
    }
    let sc = &asm.scopes[id as usize];
    if sc.call_scope {
        return;
    }
    guard.push(id);
    let ind = "  ".repeat(depth);
    let mut cap = HASHMAP_INITIAL_CAPACITY;
    while sc.order.len() as f64 > cap as f64 * HASHMAP_LOAD_FACTOR {
        cap *= 2;
    }
    let mut names: Vec<&String> = sc.order.iter().collect();
    names.sort_by_key(|n| java_bucket(n, cap));
    for n in names {
        let sy = &sc.syms[n];
        if let Val::Scope(s) = &sy.val {
            if let Some(nm) = &asm.scopes[*s as usize].ns_name {
                out.push_str(&format!("{}.namespace {} {{{}", ind, nm, NL));
                sym_scope(asm, *s, depth + 1, out, guard);
                out.push_str(&format!("{}}}{}", ind, NL));
            }
            continue;
        }
        if sy.kind != SymKind::Label {
            continue;
        }
        let Some(a) = label_addr(&sy.val) else { continue };
        let inner = match &sy.val {
            // loop labels point at a list of iteration scopes: no braces for them
            Val::Num(_, _, s) if *s > 0 && ((*s - 1) as usize) < asm.scopes.len() && asm.scopes[(*s - 1) as usize].iter.is_empty() && !asm.scopes[(*s - 1) as usize].no_sym => Some(*s - 1),
            _ => None,
        };
        out.push_str(&format!("{}.label {}=${}", ind, n, hex(a)));
        match inner {
            Some(s) => {
                out.push_str(&format!(" {{{}", NL));
                sym_scope(asm, s, depth + 1, out, guard);
                out.push_str(&format!("{}}}{}", ind, NL));
            }
            None => out.push_str(NL),
        }
    }
    guard.pop();
}

pub fn sym_file(asm: &Asm) -> String {
    let mut out = String::new();
    if !asm.scopes.is_empty() {
        sym_scope(asm, 0, 0, &mut out, &mut vec![]);
    }
    out
}

// ------------------------------------------------------------ byte dump
// From ByteDumpWriter.printBytes / writeBytesLine in the original.
const BYTEDUMP_SMALL_ENTRY: usize = 3;
const BYTEDUMP_SMALL_LINE: usize = 3;
const BYTEDUMP_MEDIUM_ENTRY: usize = 64;
const BYTEDUMP_MEDIUM_LINE: usize = 16;
const BYTEDUMP_LARGE_LINE: usize = 32;
const BYTEDUMP_LABEL_WIDTH: usize = 8;
const BYTEDUMP_HEADER_WIDTH: usize = 80;

pub fn byte_dump(asm: &Asm, auto_file: Option<usize>) -> String {
    let mut out = String::new();
    for sg in &asm.segs {
        let mut bl: Vec<&Blk> = sg.blocks.iter().filter(|b| !b.virt && !b.bytes.is_empty()).collect();
        bl.sort_by_key(|b| b.start);
        let title = format!(" Segment: {} ", sg.name);
        let pad = BYTEDUMP_HEADER_WIDTH.saturating_sub(title.len()).max(4);
        let left = pad / 2;
        let right = pad - left;
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("{}{}{}\n", "*".repeat(left), title, "*".repeat(right)));
        let mut first = true;
        for b in bl {
            if !first {
                out.push('\n');
            }
            first = false;
            out.push_str(&format!("[{}]\n", b.name));
            for d in &b.dump {
                if d.len == 0 {
                    continue;
                }
                let bytes = &b.bytes[d.off..d.off + d.len];
                let per_line = if d.len <= BYTEDUMP_SMALL_ENTRY {
                    BYTEDUMP_SMALL_LINE
                } else if d.len <= BYTEDUMP_MEDIUM_ENTRY {
                    BYTEDUMP_MEDIUM_LINE
                } else {
                    BYTEDUMP_LARGE_LINE
                };
                let text = if Some(d.pos.file) == auto_file {
                    String::new()
                } else {
                    asm.src
                        .files
                        .get(d.pos.file)
                        .and_then(|f| f.lines.get(d.pos.line.saturating_sub(1) as usize))
                        .map(|l| {
                            let l = l.trim_end_matches('\r');
                            let c: Vec<char> = l.chars().collect();
                            let from = (d.pos.col.saturating_sub(1) as usize).min(c.len());
                            c[from..].iter().collect::<String>().trim_start().to_string()
                        })
                        .unwrap_or_default()
                };
                let lab = if d.labels.is_empty() { String::new() } else { format!("{}:", d.labels.join(",")) };
                // One line per `per_line` bytes: "%04x: ", one 3-char slot per byte
                // ("%02x " or blanks past the end), then " - labels source" on the first line only.
                for (row, chunk) in bytes.chunks(per_line).enumerate() {
                    let addr = b.start + (d.off + row * per_line) as i64;
                    if b.virt {
                        out.push('*');
                    }
                    out.push_str(&format!("{:04x}: ", addr));
                    for k in 0..per_line {
                        match chunk.get(k) {
                            Some(x) => out.push_str(&format!("{:02x} ", x)),
                            None => out.push_str("   "),
                        }
                    }
                    if row == 0 {
                        out.push_str(&format!(" - {:<w$}{}", lab, text, w = BYTEDUMP_LABEL_WIDTH));
                    }
                    out.push('\n');
                }
            }
        }
    }
    out
}

// ------------------------------------------------------------ d64 images
fn sectors_in(track: usize) -> usize {
    match track {
        1..=17 => 21,
        18..=24 => 19,
        25..=30 => 18,
        _ => 17,
    }
}

fn ts_offset(t: usize, s: usize) -> usize {
    let mut o = 0;
    for k in 1..t {
        o += sectors_in(k);
    }
    (o + s) * 256
}

fn petscii_name(s: &str, len: usize) -> Vec<u8> {
    let mut v: Vec<u8> = s
        .chars()
        .take(len)
        .map(|c| {
            let u = c as u32;
            if (0x800..0x900).contains(&u) {
                (u - 0x800) as u8
            } else if c.is_ascii_lowercase() {
                c.to_ascii_uppercase() as u8
            } else if u < 256 {
                u as u8
            } else {
                b'?'
            }
        })
        .collect();
    v.resize(len, 0xa0);
    v
}

pub struct DiskFile {
    pub name: String,
    pub ftype: u8,
    pub locked: bool,
    pub hide: bool,
    pub data: Vec<u8>,
    pub interleave: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DiskFormat {
    Commodore,
    SpeedDos,
    DolphinDos,
}

#[derive(Clone, Copy)]
pub struct DiskOpts {
    pub format: DiskFormat,
    /// a file that would have sectors on both sides of the directory track moves after it
    pub dont_split_over_dir: bool,
    /// also use the free sectors of the directory track, when the files get there
    pub store_in_dir: bool,
}

/// where a file went: first track, first sector and number of sectors
pub type Placed = (usize, usize, usize);

/// A standard 35 track disk image, laid out like D64Image.generateImage:
/// - at most 8 x 19 = 152 visible files ("Too many visible files. Got N, max is M.")
/// - the directory uses consecutive sectors 18/1, 18/2, ... (8 entries each)
/// - files (also hidden ones) are placed from track 1 sector 0 upwards, skipping track 18;
///   after each sector the next one is searched from sector + interleave (wrapping in the
///   track), a new track starts at sector 0, and the next file continues where the last stopped
/// - an empty file takes no sectors (track/sector 0, 0 blocks)
///
/// The speeddos and dolphindos formats have 40 tracks and keep the BAM of tracks 36-40 in
/// sector 18/0 at $b0 and $c4 (observed).
pub fn make_d64(name: &str, id: &str, files: &[DiskFile], opts: DiskOpts) -> Result<(Vec<u8>, Vec<Placed>), String> {
    let last_track = if opts.format == DiskFormat::Commodore { 35 } else { 40 };
    let mut img = vec![0u8; ts_offset(last_track + 1, 0)];
    let mut used = vec![vec![false; 21]; last_track + 1];
    used[18][0] = true;
    let visible = files.iter().filter(|f| !f.hide).count();
    let max_files = 8 * sectors_in(19);
    if visible > max_files {
        return Err(format!("Too many visible files. Got {}, max is {}.", visible, max_files));
    }
    for k in 0..visible / 8 + 1 {
        used[18][k + 1] = true;
    }
    let mut track = 1usize;
    let mut sec = 0usize;
    let mut placed: Vec<Placed> = vec![];
    for f in files {
        if opts.dont_split_over_dir && track < 18 {
            let free: usize = (track..18).map(|t| (0..sectors_in(t)).filter(|s| !used[t][*s]).count()).sum();
            if f.data.len().div_ceil(254) > free {
                track = 19;
                sec = 0;
            }
        }
        let mut first = (0usize, 0usize);
        let mut count = 0usize;
        let mut prev: Option<usize> = None;
        for ch in f.data.chunks(254) {
            let found = loop {
                let n = sectors_in(track);
                if let Some(s) = (sec..sec + n).map(|k| k % n).find(|s| !used[track][*s]) {
                    break s;
                }
                track += 1;
                sec = 0;
                if track == 18 && !opts.store_in_dir {
                    track = 19;
                }
                if track > last_track {
                    return Err("Disk is full!".into());
                }
            };
            sec = found;
            let o = ts_offset(track, sec);
            match prev {
                None => first = (track, sec),
                Some(po) => {
                    img[po] = track as u8;
                    img[po + 1] = sec as u8;
                }
            }
            img[o + 2..o + 2 + ch.len()].copy_from_slice(ch);
            img[o] = 0;
            img[o + 1] = (ch.len() + 1) as u8;
            used[track][sec] = true;
            prev = Some(o);
            sec += f.interleave;
            count += 1;
        }
        placed.push((first.0, first.1, count));
    }
    // directory entries, 8 per sector
    let mut remaining = visible;
    let mut dsec = 1usize;
    let mut slot = 0usize;
    for (f, (t, s, n)) in files.iter().zip(placed.iter()) {
        if f.hide {
            continue;
        }
        let o = ts_offset(18, dsec) + slot * 32;
        if slot == 0 {
            if remaining > 8 && dsec < sectors_in(19) - 1 {
                img[o] = 18;
                img[o + 1] = (dsec + 1) as u8;
            } else {
                img[o] = 0;
                img[o + 1] = 0xff;
            }
        }
        img[o + 2] = f.ftype | 0x80 | if f.locked { 0x40 } else { 0 };
        img[o + 3] = *t as u8;
        img[o + 4] = *s as u8;
        img[o + 5..o + 21].copy_from_slice(&petscii_name(&f.name, 16));
        img[o + 30] = (n & 0xff) as u8;
        img[o + 31] = (n >> 8) as u8;
        slot += 1;
        if slot == 8 {
            dsec += 1;
            slot = 0;
        }
        remaining -= 1;
    }
    // BAM
    let b = ts_offset(18, 0);
    img[b] = 18;
    img[b + 1] = 1;
    img[b + 2] = 0x41;
    for t in 1..=last_track {
        let n = sectors_in(t);
        let mut bits = 0u32;
        let mut free = 0;
        for s in 0..n {
            if !used[t][s] {
                bits |= 1 << s;
                free += 1;
            }
        }
        let o = match (t, opts.format) {
            (36.., DiskFormat::SpeedDos) => b + 0xb0 + 4 * (t - 36),
            (36.., _) => b + 0xc4 + 4 * (t - 36),
            _ => b + 4 * t,
        };
        img[o] = free;
        img[o + 1] = (bits & 0xff) as u8;
        img[o + 2] = ((bits >> 8) & 0xff) as u8;
        img[o + 3] = ((bits >> 16) & 0xff) as u8;
    }
    img[b + 0x90..b + 0xa0].copy_from_slice(&petscii_name(name, 16));
    img[b + 0xa0] = 0xa0;
    img[b + 0xa1] = 0xa0;
    let idb = petscii_name(id, 5);
    img[b + 0xa2..b + 0xa7].copy_from_slice(&idb);
    for k in 0xa7..0xab {
        img[b + k] = 0xa0;
    }
    Ok((img, placed))
}

// ------------------------------------------------------------ debug dump
fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The column where the word at a source position ends: a mnemonic or a directive's name.
fn word_end(asm: &Asm, p: Pos) -> u32 {
    let Some(text) = asm.src.files.get(p.file).and_then(|f| f.lines.get(p.line.saturating_sub(1) as usize)) else { return p.col };
    let chars: Vec<char> = text.chars().collect();
    let mut i = p.col.saturating_sub(1) as usize;
    if chars.get(i) == Some(&'.') || chars.get(i) == Some(&'*') {
        i += 1;
    }
    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
        i += 1;
    }
    (i as u32).max(p.col)
}

/// The -debugdump file, for C64 Debugger (DebugDumpWriter).
pub fn debug_dump(asm: &Asm) -> String {
    let mut out = String::from("<C64debugger version=\"1.0\">\n   <Sources values=\"INDEX,FILE\">\n");
    for i in 0..asm.src.files.len() {
        out.push_str(&format!("      {},{}\n", i, xml(&asm.src.full(i))));
    }
    out.push_str("   </Sources>\n\n");
    for sg in &asm.segs {
        out.push_str(&format!("   <Segment name=\"{}\" dest=\"\" values=\"START,END,FILE_IDX,LINE1,COL1,LINE2,COL2\">\n", xml(&sg.name)));
        let mut bl: Vec<&Blk> = sg.blocks.iter().filter(|b| !b.virt && !b.bytes.is_empty()).collect();
        bl.sort_by_key(|b| b.start);
        for b in bl {
            out.push_str(&format!("      <Block name=\"{}\">\n", xml(&b.name)));
            for d in b.dump.iter().filter(|d| d.len > 0) {
                let from = b.start + d.off as i64;
                out.push_str(&format!(
                    "         ${:04x},${:04x},{},{},{},{},{}\n",
                    from,
                    from + d.len as i64 - 1,
                    d.pos.file,
                    d.pos.line,
                    d.pos.col,
                    d.pos.line,
                    word_end(asm, d.pos)
                ));
            }
            out.push_str("      </Block>\n");
        }
        out.push_str("   </Segment>\n\n");
    }
    out.push_str("   <Labels values=\"SEGMENT,ADDRESS,NAME,START,END,FILE_IDX,LINE1,COL1,LINE2,COL2\">\n");
    for (seg, addr, name, p) in &asm.placed_labels {
        let end = p.col + name.chars().count() as u32;
        out.push_str(&format!("      {},${:04x},{},{},{},{},{},{}\n", xml(seg), addr, xml(name), p.file, p.line, p.col, p.line, end));
    }
    out.push_str("   </Labels>\n\n   <Breakpoints values=\"SEGMENT,ADDRESS,ARGUMENT\">\n");
    for b in &asm.breaks {
        out.push_str(&format!("      Default,${:04x},{}\n", b.addr, xml(&b.arg.clone().unwrap_or_default())));
    }
    out.push_str("   </Breakpoints>\n\n   <Watchpoints values=\"SEGMENT,ADDRESS1,ADDRESS2,ARGUMENT\">\n");
    for w in &asm.watches {
        let a2 = w.a2.map(|x| format!("${:04x}", x)).unwrap_or_default();
        out.push_str(&format!("      Default,${:04x},{},{}\n", w.a1, a2, xml(&w.arg.clone().unwrap_or_default())));
    }
    out.push_str("   </Watchpoints>\n\n</C64debugger>\n");
    out
}

// ------------------------------------------------------------ writing everything
pub struct OutOpts {
    pub out_file: PathBuf,
    pub binfile: bool,
    pub mbfiles: bool,
    pub fill_byte: u8,
    /// only log, don't write (-nooutput)
    pub dry: bool,
}

thread_local! {
    static DRY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn resolve(dir: &Path, name: &str) -> PathBuf {
    let p = Path::new(name);
    if p.is_absolute() { p.to_path_buf() } else { dir.join(p) }
}

fn write(path: &Path, data: &[u8]) -> R<()> {
    if DRY.with(|d| d.get()) {
        return Ok(());
    }
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(path, data).map_err(|e| AsmError {
        pos: None,
        msg: format!("Problem while writing file '{}': {}", path.display(), e),
        called: vec![],
    })
}

/// Writes the program files and disk images. The log lines stay when an error comes after them.
pub fn write_outputs(asm: &mut Asm, o: &OutOpts, log: &mut Vec<String>) -> R<()> {
    DRY.with(|d| d.set(o.dry));
    let dir = asm.cfg.out_dir.clone();
    // (file name, blocks, prg?, mbfiles, fill)
    let mut files: Vec<(String, Vec<Blk>, bool, bool, u8)> = vec![];
    // %o in a file name stands for the name of the root source file
    let base = Path::new(&asm.cfg.src_name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let default_used = asm.segs.len() == 1 || asm.segs[0].blocks.iter().any(|b| !b.bytes.is_empty());
    if default_used {
        let name = o.out_file.to_string_lossy().to_string();
        let blocks = asm.segment_blocks("Default", Pos::default(), false, 0)?;
        files.push((name, blocks, !o.binfile, o.mbfiles, o.fill_byte));
    }
    let segs: Vec<(String, SegParams)> = asm.segs.iter().map(|s| (s.name.clone(), s.p.clone())).collect();
    for (name, p) in &segs {
        for (f, prg) in [(&p.out_prg, true), (&p.out_bin, false)] {
            if let Some(f) = f {
                let blocks = asm.segment_blocks(name, p.pos, false, 0)?;
                files.push((f.replace("%o", &base), blocks, prg, false, p.fill_byte));
            }
        }
    }
    let reqs = asm.file_reqs.clone();
    for p in &reqs {
        let name = p.name.clone().unwrap_or_default().replace("%o", &base);
        if files.iter().any(|f| f.0 == name) {
            return Err(AsmError { pos: Some(p.pos), msg: format!("The file '{}' is already defined", name), called: vec![] });
        }
        let blocks = asm.intermediate_blocks(p, &format!("<<.file:{}>>", p.name.clone().unwrap_or_default()), p.pos, false)?;
        let prg = p.ftype.as_deref().unwrap_or("prg") != "bin";
        files.push((name, blocks, prg, p.mbfiles, p.fill_byte));
    }
    for (name, blocks, prg, mb, fill) in &files {
        if !*mb {
            log.push(format!("Writing prg file: {}", name));
            write(&resolve(&dir, name), &file_bytes(blocks, *fill, *prg))?;
        } else {
            let (b, ext) = match [".bin", ".prg"].iter().find(|e| name.ends_with(*e)) {
                Some(e) => (name[..name.len() - e.len()].to_string(), e.to_string()),
                None => (name.clone(), String::new()),
            };
            let mut names: Vec<String> = vec![];
            for bl in blocks.iter().filter(|b| !b.virt) {
                if !names.contains(&bl.name) {
                    names.push(bl.name.clone());
                }
            }
            for n in names {
                let group: Vec<Blk> = blocks.iter().filter(|b| !b.virt && b.name == n).cloned().collect();
                if group.len() != 1 {
                    log.push(format!("{} blocks have the name '{}'. They will be merged in the same file.", group.len(), n));
                }
                let fname = format!("{}_{}{}", b, n, ext);
                log.push(format!("Writing prg file: {}", fname));
                write(&resolve(&dir, &fname), &file_bytes(&group, *fill, *prg))?;
            }
        }
    }
    // disk images
    let disks: Vec<(SegParams, Vec<SegParams>)> = asm.disk_reqs.iter().map(|d| (d.p.clone(), d.files.clone())).collect();
    for (dp, fps) in disks {
        let fname = match dp.other.get("filename") {
            Some(v) => v.to_str().replace("%o", &base),
            None => return Err(AsmError { pos: Some(dp.pos), msg: "Missing the 'filename' parameter of the disk".into(), called: vec![] }),
        };
        let format = match dp.other.get("format").map(|v| v.to_str()).as_deref() {
            None | Some("commodore") => DiskFormat::Commodore,
            Some("speeddos") => DiskFormat::SpeedDos,
            Some("dolphindos") => DiskFormat::DolphinDos,
            Some(f) => return Err(AsmError { pos: Some(dp.format_pos), msg: format!("Unknown litteral '{}'", f), called: vec![] }),
        };
        let flag = |n: &str| dp.other.get(n).map(|v| matches!(v, Val::Bool(true, _))).unwrap_or(false);
        let opts = DiskOpts { format, dont_split_over_dir: flag("dontSplitFilesOverDir"), store_in_dir: flag("storeFilesInDir") };
        let interleave = dp.other.get("interleave").and_then(|v| v.as_f64()).unwrap_or(10.0) as i64;
        if interleave < 0 {
            return Err(AsmError { pos: Some(dp.pos), msg: format!("Interleave can't be negative: {}", interleave), called: vec![] });
        }
        let mut dfiles = vec![];
        for (i, fp) in fps.iter().enumerate() {
            let blocks = asm.intermediate_blocks(fp, &format!("<<.diskfile-{}>>", i), fp.pos, false)?;
            let mut t = fp.ftype.clone().unwrap_or_else(|| "prg".into()).to_lowercase();
            let locked = t.ends_with('<');
            if locked {
                t.pop();
            }
            let ftype = match t.as_str() {
                "del" => 0,
                "seq" => 1,
                "prg" => 2,
                "usr" => 3,
                "rel" => 4,
                _ => return Err(AsmError { pos: Some(fp.pos), msg: format!("Unknown type: {}", t), called: vec![] }),
            };
            let no_start = fp.other.get("noStartAddr").map(|v| matches!(v, Val::Bool(true, _))).unwrap_or(ftype != 2);
            let il = fp.other.get("interleave").and_then(|v| v.as_f64()).map(|x| x as i64).unwrap_or(interleave);
            dfiles.push(DiskFile {
                name: fp.name.clone().unwrap_or_default(),
                ftype,
                locked,
                hide: fp.hide,
                data: file_bytes(&blocks, 0, !no_start),
                interleave: il.max(1) as usize,
            });
        }
        log.push(format!("Writing d64 file: {}", fname));
        let name = dp.name.clone().unwrap_or_else(|| "UNNAMED".into());
        let id = dp.other.get("id").map(|v| v.to_str()).unwrap_or_else(|| "   2A".into());
        let (img, placed) = make_d64(&name, &id, &dfiles, opts).map_err(|e| {
            // the original has opened the file by then, so it stays empty
            let _ = write(&resolve(&dir, &fname), &[]);
            AsmError { pos: None, msg: format!("Error while generating '{}': {}", fname, e), called: vec![] }
        })?;
        if flag("showInfo") {
            log.push(format!("Info for disk '{}'", name));
            for (f, (t, s, n)) in dfiles.iter().zip(&placed) {
                log.push(format!("  \"{}\", sectors={}, startTS={}:{}", f.name, n, t, s));
            }
        }
        write(&resolve(&dir, &fname), &img)?;
    }
    // text files made with createFile()
    for f in asm.out_files.clone() {
        write(&f.path, f.text.borrow().as_bytes())?;
    }
    Ok(())
}
