//! The desktop window: a source editor, assembler options, the log,
//! a memory map picture and a hex view of the result.

use crate::driver::{self, Log, Options, Outcome, APP_NAME};
use eframe::egui::{self, text::LayoutJob, Color32, FontId, RichText, Stroke, TextFormat};
use std::collections::HashSet;
use std::path::PathBuf;

const SAMPLE: &str = r#"// Welcome! Press F5 (or Assemble) to build this program.
// It shows a moving colour bar in the border of a C64.

BasicUpstart2(start)

        * = $0810 "Program"
start:  sei
        lda #0
        sta $d020
loop:   ldx #0
wait:   lda $d012               // wait for the raster line
        cmp #$80
        bne wait
        ldx frame
        lda colors,x
        sta $d020
        inc frame
        jmp loop

frame:  .byte 0

        // a sine table made by a script loop
colors: .fill 256, round(7.5 + 7.5 * sin(toRadians(i * 360 / 256)))

        .print "Table at $" + toHexString(colors) + ", program size " + (* - start) + " bytes"
        .assert "loop goes back to the start", loop > start, true
"#;

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Log,
    Memory,
    Bytes,
}

pub struct App {
    text: String,
    path: Option<PathBuf>,
    dirty: bool,
    write_files: bool,
    opts: Options,
    log: Vec<String>,
    outcome: Outcome,
    tab: Tab,
    mnemonics: HashSet<String>,
    shot: Option<PathBuf>,
    frames: u32,
    shot_sent: bool,
    message: Option<String>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, file: Option<PathBuf>, shot: Option<PathBuf>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        cc.egui_ctx.set_style(style);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut app = App {
            text: SAMPLE.to_string(),
            path: None,
            dirty: false,
            write_files: false,
            opts: Options { show_mem: true, ..Default::default() },
            log: vec![],
            outcome: Outcome::default(),
            tab: Tab::Log,
            mnemonics: crate::eval::all_mnemonics(),
            shot,
            frames: 0,
            shot_sent: false,
            message: None,
        };
        if let Some(f) = file {
            app.load(f);
        }
        app.assemble();
        app
    }

    fn load(&mut self, p: PathBuf) {
        match std::fs::read(&p) {
            Ok(b) => {
                self.text = String::from_utf8_lossy(&b).replace("\r\n", "\n");
                self.path = Some(p);
                self.dirty = false;
                self.write_files = true;
                self.message = None;
            }
            Err(e) => self.message = Some(format!("Couldn't open {}: {}", p.display(), e)),
        }
    }

    fn save(&mut self, as_new: bool) -> bool {
        let target = if as_new || self.path.is_none() {
            rfd::FileDialog::new().add_filter("Assembler source", &["asm", "s", "inc", "lib"]).set_file_name("program.asm").save_file()
        } else {
            self.path.clone()
        };
        let Some(p) = target else { return false };
        match std::fs::write(&p, &self.text) {
            Ok(_) => {
                self.path = Some(p);
                self.dirty = false;
                self.message = None;
                true
            }
            Err(e) => {
                self.message = Some(format!("Couldn't save {}: {}", p.display(), e));
                false
            }
        }
    }

    fn assemble(&mut self) {
        let mut o = self.opts.clone();
        o.file = Some(self.path.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default().join("untitled.asm")));
        o.no_output = !self.write_files || self.path.is_none();
        let log = Log::new(false);
        self.outcome = driver::assemble(&o, Some(&self.text), &log);
        self.log = log.lines.borrow().clone();
        if !self.outcome.ok && self.tab != Tab::Log {
            self.tab = Tab::Log;
        }
    }

    fn highlight(mnemonics: &HashSet<String>, text: &str) -> LayoutJob {
        let font = FontId::monospace(14.0);
        let mut job = LayoutJob::default();
        let c_text = Color32::from_rgb(212, 212, 212);
        let c_comment = Color32::from_rgb(106, 153, 85);
        let c_dir = Color32::from_rgb(86, 156, 214);
        let c_mn = Color32::from_rgb(220, 170, 100);
        let c_num = Color32::from_rgb(181, 206, 168);
        let c_str = Color32::from_rgb(206, 145, 120);
        let c_label = Color32::from_rgb(197, 134, 192);
        let ch: Vec<char> = text.chars().collect();
        let mut i = 0;
        let push = |job: &mut LayoutJob, s: String, c: Color32| {
            job.append(&s, 0.0, TextFormat { font_id: font.clone(), color: c, ..Default::default() });
        };
        while i < ch.len() {
            let c = ch[i];
            let start = i;
            let color;
            if c == '/' && ch.get(i + 1) == Some(&'/') {
                while i < ch.len() && ch[i] != '\n' {
                    i += 1;
                }
                color = c_comment;
            } else if c == '/' && ch.get(i + 1) == Some(&'*') {
                i += 2;
                while i < ch.len() && !(ch[i] == '*' && ch.get(i + 1) == Some(&'/')) {
                    i += 1;
                }
                i = (i + 2).min(ch.len());
                color = c_comment;
            } else if c == '"' || c == '\'' {
                i += 1;
                while i < ch.len() && ch[i] != c && ch[i] != '\n' {
                    i += 1;
                }
                i = (i + 1).min(ch.len());
                color = c_str;
            } else if c == '$' || c == '%' && ch.get(i + 1).map(|d| *d == '0' || *d == '1').unwrap_or(false) || c.is_ascii_digit() {
                i += 1;
                while i < ch.len() && (ch[i].is_ascii_hexdigit() || ch[i] == '.') {
                    i += 1;
                }
                color = c_num;
            } else if c == '.' || c == '#' && ch.get(i + 1).map(|d| d.is_ascii_alphabetic()).unwrap_or(false) || c.is_alphabetic() || c == '_' || c == '!' {
                i += 1;
                while i < ch.len() && (ch[i].is_alphanumeric() || ch[i] == '_') {
                    i += 1;
                }
                let w: String = ch[start..i].iter().collect();
                color = if c == '.' || c == '#' {
                    c_dir
                } else if ch.get(i) == Some(&':') && ch.get(i + 1) != Some(&':') {
                    i += 1;
                    c_label
                } else if mnemonics.contains(&w.to_lowercase()) && w.len() == 3 {
                    c_mn
                } else {
                    c_text
                };
            } else {
                i += 1;
                color = c_text;
            }
            push(&mut job, ch[start..i].iter().collect(), color);
        }
        job
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("New").clicked() {
                self.text = String::from("BasicUpstart2(start)\n\n        * = $0810\nstart:  rts\n");
                self.path = None;
                self.dirty = false;
                self.write_files = false;
            }
            if ui.button("Open…").clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("Assembler source", &["asm", "s", "inc", "lib"]).pick_file() {
                    self.load(p);
                    self.assemble();
                }
            }
            if ui.button("Save").clicked() {
                self.save(false);
            }
            if ui.button("Save as…").clicked() {
                self.save(true);
            }
            ui.separator();
            let run = ui.add(egui::Button::new(RichText::new("▶ Assemble  (F5)").strong()).fill(Color32::from_rgb(40, 90, 60)));
            if run.clicked() || ctx.input(|i| i.key_pressed(egui::Key::F5)) {
                if self.path.is_some() && self.dirty {
                    self.save(false);
                }
                self.assemble();
            }
            ui.separator();
            ui.add_enabled(self.path.is_some(), egui::Checkbox::new(&mut self.write_files, "Write files"))
                .on_hover_text("Write the program file (and the files below) next to the source. Save the source first.");
            ui.checkbox(&mut self.opts.show_mem, "Memory map");
            ui.checkbox(&mut self.opts.vice_symbols, "VICE labels");
            ui.checkbox(&mut self.opts.symbol_file, "Symbol file");
            let mut bd = self.opts.byte_dump.is_some();
            if ui.checkbox(&mut bd, "Byte dump").changed() {
                self.opts.byte_dump = if bd { Some("ByteDump.txt".into()) } else { None };
            }
            ui.checkbox(&mut self.opts.allow_overlap, "Allow overlap");
            egui::ComboBox::from_id_salt("cpu").selected_text(cpu_label(&self.opts.cpu)).show_ui(ui, |ui| {
                for c in ["_6502NoIllegals", "_6502", "dtv", "_65c02"] {
                    ui.selectable_value(&mut self.opts.cpu, c.to_string(), cpu_label(c));
                }
            });
        });
    }

    fn status(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let name = self.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "untitled (not saved)".into());
            ui.label(RichText::new(format!("{}{}", name, if self.dirty { " •" } else { "" })).weak());
            ui.separator();
            if let Some(m) = &self.message {
                ui.colored_label(Color32::from_rgb(240, 180, 80), m);
            } else if self.outcome.ok {
                let mut s = format!("OK, {} passes", self.outcome.passes);
                if let Some((lo, d)) = &self.outcome.image {
                    if !d.is_empty() {
                        s.push_str(&format!("  •  ${:04x}-${:04x}  ({} bytes)", lo, lo + d.len() as i64 - 1, d.len()));
                    }
                }
                ui.colored_label(Color32::from_rgb(120, 200, 120), s);
            } else if let Some((_, l, c, m)) = &self.outcome.error {
                ui.colored_label(Color32::from_rgb(240, 100, 100), format!("Error at line {}, column {}: {}", l, c, m));
            }
        });
    }

    fn log_view(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::both().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            for l in &self.log {
                let t = RichText::new(l).monospace().size(13.0);
                let t = if l.starts_with("Error") {
                    t.color(Color32::from_rgb(240, 100, 100))
                } else if l.contains("Warning:") || l.contains("ERROR IN ASSERTION") || l.contains("FAILED") {
                    t.color(Color32::from_rgb(240, 180, 80))
                } else if l.starts_with("  ") {
                    t.color(Color32::from_rgb(200, 220, 255))
                } else {
                    t.color(Color32::from_gray(190))
                };
                ui.label(t);
            }
        });
    }

    fn memory_view(&self, ui: &mut egui::Ui) {
        ui.label(RichText::new("64K memory  ($0000 – $ffff)").strong());
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 40.0), egui::Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 3.0, Color32::from_gray(35));
        let palette = [
            Color32::from_rgb(90, 160, 230),
            Color32::from_rgb(230, 150, 70),
            Color32::from_rgb(120, 200, 120),
            Color32::from_rgb(200, 110, 200),
            Color32::from_rgb(230, 210, 90),
        ];
        let x_of = |a: i64| rect.left() + (a.clamp(0, 0x10000) as f32 / 65536.0) * rect.width();
        for (i, b) in self.outcome.blocks.iter().enumerate() {
            if b.len == 0 {
                continue;
            }
            let r = egui::Rect::from_min_max(egui::pos2(x_of(b.start), rect.top() + 4.0), egui::pos2(x_of(b.start + b.len as i64).max(x_of(b.start) + 2.0), rect.bottom() - 4.0));
            if b.virt {
                p.rect_stroke(r, 1.0, Stroke::new(1.0, Color32::from_gray(150)));
            } else {
                p.rect_filled(r, 1.0, palette[i % palette.len()]);
            }
        }
        for k in 0..=4 {
            let x = rect.left() + rect.width() * k as f32 / 4.0;
            p.line_segment([egui::pos2(x, rect.bottom() - 6.0), egui::pos2(x, rect.bottom())], Stroke::new(1.0, Color32::from_gray(120)));
        }
        ui.horizontal(|ui| {
            for t in ["$0000", "$4000", "$8000", "$c000", "$ffff"] {
                ui.label(RichText::new(t).monospace().size(11.0).weak());
                ui.add_space((w - 5.0 * 40.0) / 4.0 - 8.0);
            }
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("blocks").striped(true).num_columns(5).show(ui, |ui| {
                for h in ["", "Segment", "Block", "Range", "Bytes"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (i, b) in self.outcome.blocks.iter().enumerate() {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    if b.virt {
                        ui.painter().rect_stroke(r, 1.0, Stroke::new(1.0, Color32::from_gray(150)));
                    } else {
                        ui.painter().rect_filled(r, 2.0, palette[i % palette.len()]);
                    }
                    ui.label(&b.segment);
                    ui.label(if b.virt { format!("{} (virtual)", b.name) } else { b.name.clone() });
                    ui.label(RichText::new(format!("${:04x}-${:04x}", b.start, b.start + b.len as i64 - 1)).monospace());
                    ui.label(b.len.to_string());
                    ui.end_row();
                }
            });
        });
    }

    fn bytes_view(&self, ui: &mut egui::Ui) {
        let Some((lo, data)) = &self.outcome.image else {
            ui.label("Nothing assembled yet.");
            return;
        };
        if data.is_empty() {
            ui.label("The program has no bytes.");
            return;
        }
        let rows = data.len().div_ceil(16);
        let h = 18.0;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, h, rows, |ui, range| {
            for r in range {
                let chunk = &data[r * 16..(r * 16 + 16).min(data.len())];
                let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
                let asc: String = chunk.iter().map(|b| if (32..127).contains(b) { *b as char } else { '.' }).collect();
                ui.label(RichText::new(format!("{:04x}  {:<47}  {}", lo + (r * 16) as i64, hex.join(" "), asc)).monospace().size(13.0));
            }
        });
    }
}

fn cpu_label(c: &str) -> &'static str {
    match c {
        "_6502NoIllegals" => "6502 (no illegal opcodes)",
        "dtv" => "6502 + DTV",
        "_65c02" => "65C02",
        _ => "6502 (with illegal opcodes)",
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let title = format!(
            "{} - {}{}",
            APP_NAME,
            self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "untitled".into()),
            if self.dirty { " *" } else { "" }
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            self.save(false);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            if let Some(p) = rfd::FileDialog::new().add_filter("Assembler source", &["asm", "s", "inc", "lib"]).pick_file() {
                self.load(p);
                self.assemble();
            }
        }

        egui::TopBottomPanel::top("tools").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(APP_NAME).strong().size(16.0).color(Color32::from_rgb(130, 180, 255)));
                ui.label(RichText::new("6502 / C64 cross assembler").weak());
            });
            self.toolbar(ui, ctx);
            ui.add_space(2.0);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.status(ui));
        egui::SidePanel::right("output").resizable(true).default_width(470.0).show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Log, "Log");
                ui.selectable_value(&mut self.tab, Tab::Memory, "Memory");
                ui.selectable_value(&mut self.tab, Tab::Bytes, "Bytes");
            });
            ui.separator();
            match self.tab {
                Tab::Log => self.log_view(ui),
                Tab::Memory => self.memory_view(ui),
                Tab::Bytes => self.bytes_view(ui),
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let err_line = self.outcome.error.as_ref().and_then(|(f, l, _, _)| {
                let same = match &self.path {
                    Some(p) => *f == p.to_string_lossy(),
                    None => f.ends_with("untitled.asm"),
                };
                if same { Some(*l) } else { None }
            });
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let n = self.text.split('\n').count().max(1);
                    let mut nums = String::new();
                    for k in 1..=n {
                        nums.push_str(&format!("{:>4}\n", k));
                    }
                    let mut job = LayoutJob::default();
                    for (k, l) in nums.lines().enumerate() {
                        let col = if Some(k as u32 + 1) == err_line { Color32::from_rgb(240, 90, 90) } else { Color32::from_gray(100) };
                        job.append(&format!("{}\n", l), 0.0, TextFormat { font_id: FontId::monospace(14.0), color: col, ..Default::default() });
                    }
                    ui.add_space(2.0);
                    ui.label(job);
                    let mn = &self.mnemonics;
                    let hl = |ui: &egui::Ui, s: &str, _w: f32| {
                        let job = App::highlight(mn, s);
                        ui.fonts(|f| f.layout_job(job))
                    };
                    let mut text = std::mem::take(&mut self.text);
                    let resp = ui.add(
                        egui::TextEdit::multiline(&mut text)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(40)
                            .frame(false)
                            .layouter(&mut { hl }),
                    );
                    self.text = text;
                    if resp.changed() {
                        self.dirty = true;
                    }
                });
            });
        });

        // --screenshot: draw some frames, ask for a picture, save it and quit
        if let Some(path) = self.shot.clone() {
            self.frames += 1;
            ctx.request_repaint();
            if self.frames >= 120 && !self.shot_sent {
                self.shot_sent = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
            let shots: Vec<std::sync::Arc<egui::ColorImage>> = ctx.input(|i| {
                i.raw.events.iter().filter_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }).collect()
            });
            if let Some(img) = shots.first() {
                let [w, h] = img.size;
                let mut buf = Vec::with_capacity(w * h * 4);
                for p in &img.pixels {
                    buf.extend_from_slice(&p.to_srgba_unmultiplied());
                }
                if let Some(d) = path.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                match image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                    Some(im) => {
                        if let Err(e) = im.save(&path) {
                            eprintln!("Couldn't save the screenshot: {}", e);
                        }
                    }
                    None => eprintln!("Couldn't build the screenshot image"),
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if self.frames > 600 {
                eprintln!("No screenshot arrived; closing");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

pub fn run(file: Option<PathBuf>, shot: Option<PathBuf>) -> Result<(), String> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]).with_min_inner_size([700.0, 400.0]).with_title(APP_NAME),
        ..Default::default()
    };
    eframe::run_native(APP_NAME, opts, Box::new(move |cc| Ok(Box::new(App::new(cc, file, shot))))).map_err(|e| e.to_string())
}
