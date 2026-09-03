//! Giao diện chọn vùng + chú thích: một cửa sổ toàn màn hình trên MỖI màn hình,
//! cùng dùng chung một trạng thái (toạ độ canvas = pixel ảnh chụp).

use crate::config::Config;
use crate::model::*;
use crate::output;
use crate::render;
use gtk4 as gtk;
use gtk4::cairo;
use gtk4::gdk;
use gtk4::glib;
use gtk4::pango;
use gtk4::prelude::*;
use image::RgbaImage;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

const HANDLE: f64 = 9.0; // px logic
const BTN: f64 = 36.0;
const GAP: f64 = 4.0;
const PAD: f64 = 6.0;
const SWATCH: f64 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Btn {
    Tool(Tool),
    Undo,
    Redo,
    Copy,
    Save,
    SaveAs,
    Exit,
    Color(usize),
    Fill,
    Thick,
    Ratio,
}

impl Btn {
    fn label(&self) -> String {
        match self {
            Btn::Tool(t) => t.label().to_string(),
            Btn::Undo => "Hoàn tác (Ctrl+Z)".into(),
            Btn::Redo => "Làm lại (Ctrl+Shift+Z)".into(),
            Btn::Copy => "Copy vào clipboard (Enter / Ctrl+C)".into(),
            Btn::Save => "Lưu vào thư mục ảnh (Ctrl+S)".into(),
            Btn::SaveAs => "Lưu thành... (Ctrl+Shift+S)".into(),
            Btn::Exit => "Thoát (Esc)".into(),
            Btn::Color(_) => "Chọn màu (phím 1-9, 0)".into(),
            Btn::Fill => "Tô đặc hình (F)".into(),
            Btn::Thick => "Độ dày nét — lăn chuột hoặc [ ]".into(),
            Btn::Ratio => "Tỉ lệ khung chọn (Tab)".into(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Drag {
    NewSel { sx: f64, sy: f64 },
    MoveSel { dx: f64, dy: f64 },
    ResizeSel { handle: usize, anchor: Rect },
    Draw(usize),
}

#[derive(Clone, Debug)]
pub enum Action {
    Copy,
    Save(Option<PathBuf>),
    SaveAs,
}

pub struct Overlay {
    pub base: RgbaImage,
    pub base_surf: cairo::ImageSurface,
    pub bounds: Rect,
    pub monitors: Vec<MonitorInfo>,
    pub cfg: Config,
    pub debug: bool,

    sel: Option<Rect>,
    tool: Tool,
    prev_tool: Tool,
    color: Color,
    thickness: f64,
    filled: bool,
    ratio: Ratio,
    shapes: Vec<Shape>,
    redo: Vec<Shape>,
    drag: Option<Drag>,
    editing: Option<usize>,
    counter: u32,
    cursor: (f64, f64),
    cursor_mon: usize,
    hover: Option<Btn>,
    status: Option<(String, Instant)>,

    windows: Vec<gtk::Window>,
    areas: Vec<gtk::DrawingArea>,
    im: Option<gtk::IMMulticontext>,
    app: Option<gtk::Application>,
    finished: bool,
}

pub type Shared = Rc<RefCell<Overlay>>;

impl Overlay {
    pub fn new(base: RgbaImage, cfg: Config, debug: bool) -> Overlay {
        let base_surf = render::base_surface(&base);
        let bounds = Rect::new(0.0, 0.0, base.width() as f64, base.height() as f64);
        let color = Color::from_hex(&cfg.color).unwrap_or(PALETTE[0]);
        let ratio = Ratio::parse(&cfg.ratio).unwrap_or(Ratio::Free);
        let thickness = cfg.thickness.clamp(1.0, 40.0);
        Overlay {
            base,
            base_surf,
            bounds,
            monitors: Vec::new(),
            cfg,
            debug,
            sel: None,
            tool: Tool::Select,
            prev_tool: Tool::Select,
            color,
            thickness,
            filled: false,
            ratio,
            shapes: Vec::new(),
            redo: Vec::new(),
            drag: None,
            editing: None,
            counter: 0,
            cursor: (0.0, 0.0),
            cursor_mon: 0,
            hover: None,
            status: None,
            windows: Vec::new(),
            areas: Vec::new(),
            im: None,
            app: None,
            finished: false,
        }
    }

    // ---------- Màn hình ----------

    /// Đọc danh sách màn hình từ GDK và tính vùng tương ứng trên ảnh chụp.
    fn detect_monitors(&mut self, display: &gdk::Display) -> Vec<gdk::Monitor> {
        let list = display.monitors();
        let mut mons = Vec::new();
        for i in 0..list.n_items() {
            if let Some(m) = list.item(i).and_downcast::<gdk::Monitor>() {
                mons.push(m);
            }
        }
        if mons.is_empty() {
            eprintln!("quickshot: GDK không thấy màn hình nào");
        }
        // Khung logic bao tất cả màn hình
        let mut minx = f64::MAX;
        let mut miny = f64::MAX;
        let mut maxx = f64::MIN;
        let mut maxy = f64::MIN;
        for m in &mons {
            let g = m.geometry();
            minx = minx.min(g.x() as f64);
            miny = miny.min(g.y() as f64);
            maxx = maxx.max((g.x() + g.width()) as f64);
            maxy = maxy.max((g.y() + g.height()) as f64);
        }
        let lw = (maxx - minx).max(1.0);
        let lh = (maxy - miny).max(1.0);
        // Ảnh chụp là toàn bộ desktop → tỉ lệ đồng nhất giữa toạ độ logic và pixel ảnh
        let rx = self.bounds.w / lw;
        let ry = self.bounds.h / lh;
        self.monitors.clear();
        for (i, m) in mons.iter().enumerate() {
            let g = m.geometry();
            let logical = Rect::new(g.x() as f64, g.y() as f64, g.width() as f64, g.height() as f64);
            let canvas = Rect::new(
                (logical.x - minx) * rx,
                (logical.y - miny) * ry,
                logical.w * rx,
                logical.h * ry,
            );
            let name = m
                .connector()
                .map(|s| s.to_string())
                .or_else(|| m.model().map(|s| s.to_string()))
                .unwrap_or_else(|| format!("monitor{i}"));
            let info = MonitorInfo { index: i, name, logical, canvas, sx: rx, sy: ry };
            if self.debug {
                eprintln!(
                    "[monitor {i}] {} logic=({},{} {}x{}) scale={} canvas=({:.0},{:.0} {:.0}x{:.0}) r=({:.3},{:.3})",
                    info.name, logical.x, logical.y, logical.w, logical.h, m.scale_factor(),
                    canvas.x, canvas.y, canvas.w, canvas.h, rx, ry
                );
            }
            self.monitors.push(info);
        }
        if self.debug {
            eprintln!(
                "[capture] ảnh {}x{}, desktop logic {}x{} (gốc {},{})",
                self.bounds.w, self.bounds.h, lw, lh, minx, miny
            );
        }
        mons
    }

    fn monitor_at(&self, cx: f64, cy: f64) -> Option<usize> {
        self.monitors.iter().position(|m| m.canvas.contains(cx, cy))
    }

    // ---------- Tiện ích ----------

    fn redraw_all(&self) {
        for a in &self.areas {
            a.queue_draw();
        }
    }

    fn set_status(&mut self, s: impl Into<String>) {
        self.status = Some((s.into(), Instant::now()));
    }

    fn snap_ratio(&self, mut r: Rect, anchor_x: f64, anchor_y: f64, shift: bool) -> Rect {
        let ratio = if shift { Some(1.0) } else { self.ratio.value() };
        let Some(v) = ratio else { return r };
        // Giữ chiều rộng, tính chiều cao (trừ khi rất hẹp)
        let w = r.w.max(1.0);
        let h = (w / v).max(1.0);
        // giữ góc neo cố định
        let flip_x = r.x < anchor_x - 0.5;
        let flip_y = r.y < anchor_y - 0.5;
        r.w = w;
        r.h = h;
        if flip_x {
            r.x = anchor_x - w;
        } else {
            r.x = anchor_x;
        }
        if flip_y {
            r.y = anchor_y - h;
        } else {
            r.y = anchor_y;
        }
        r
    }

    /// Ép vùng chọn theo tỉ lệ v (rộng/cao), giữ góc trên-trái, thu nhỏ nếu vượt biên ảnh.
    fn fit_ratio(&self, s: Rect, v: f64) -> Rect {
        let b = self.bounds;
        let max_w = (b.right() - s.x).max(1.0);
        let max_h = (b.bottom() - s.y).max(1.0);
        let mut w = s.w.max(1.0);
        let mut h = w / v;
        if h > max_h {
            h = max_h;
            w = h * v;
        }
        if w > max_w {
            w = max_w;
            h = w / v;
        }
        Rect::new(s.x, s.y, w, h).rounded()
    }

    fn handles(&self, sel: &Rect) -> [(f64, f64); 8] {
        let cx = sel.x + sel.w / 2.0;
        let cy = sel.y + sel.h / 2.0;
        [
            (sel.x, sel.y),
            (cx, sel.y),
            (sel.right(), sel.y),
            (sel.right(), cy),
            (sel.right(), sel.bottom()),
            (cx, sel.bottom()),
            (sel.x, sel.bottom()),
            (sel.x, cy),
        ]
    }

    fn handle_at(&self, cx: f64, cy: f64, mon: &MonitorInfo) -> Option<usize> {
        let sel = self.sel?;
        let tol = HANDLE * mon.sx;
        for (i, (hx, hy)) in self.handles(&sel).iter().enumerate() {
            if (cx - hx).abs() <= tol && (cy - hy).abs() <= tol {
                return Some(i);
            }
        }
        None
    }

    fn cursor_name_for_handle(h: usize) -> &'static str {
        match h {
            0 => "nw-resize",
            1 => "n-resize",
            2 => "ne-resize",
            3 => "e-resize",
            4 => "se-resize",
            5 => "s-resize",
            6 => "sw-resize",
            _ => "w-resize",
        }
    }

    // ---------- Thanh công cụ ----------

    fn toolbar_monitor(&self) -> usize {
        if let Some(sel) = self.sel {
            let px = sel.x + sel.w / 2.0;
            if let Some(i) = self.monitor_at(px, sel.bottom() - 1.0) {
                return i;
            }
            if let Some(i) = self.monitor_at(px, sel.y + sel.h / 2.0) {
                return i;
            }
        }
        self.cursor_mon.min(self.monitors.len().saturating_sub(1))
    }

    fn toolbar_size(&self) -> (f64, f64) {
        let row1 = 17.0 * BTN + 16.0 * GAP + 2.0 * (GAP * 2.0);
        let row2 = 10.0 * (SWATCH + GAP) + 3.0 * GAP + BTN + BTN + 76.0;
        (row1.max(row2) + PAD * 2.0, BTN + SWATCH + PAD * 3.0 + 4.0)
    }

    /// Vị trí (toạ độ logic của màn hình `mi`) và danh sách nút của thanh công cụ.
    fn toolbar_layout(&self, mi: usize) -> Option<(Rect, Vec<(Rect, Btn)>)> {
        let sel = self.sel?;
        let mon = self.monitors.get(mi)?;
        let (tw, th) = self.toolbar_size();
        let (sx, sy) = mon.to_local(sel.x, sel.y);
        let (ex, ey) = mon.to_local(sel.right(), sel.bottom());
        let mw = mon.logical.w;
        let mh = mon.logical.h;
        let cx = (sx + ex) / 2.0;
        let mut x = (cx - tw / 2.0).clamp(4.0, (mw - tw - 4.0).max(4.0));
        if tw > mw - 8.0 {
            x = 4.0;
        }
        let y = if ey + 10.0 + th <= mh - 4.0 {
            ey + 10.0
        } else if sy - 10.0 - th >= 4.0 {
            sy - 10.0 - th
        } else {
            (ey - 10.0 - th).clamp(4.0, mh - th - 4.0)
        };
        let bar = Rect::new(x, y, tw, th);

        let mut btns = Vec::new();
        let mut bx = x + PAD;
        let by = y + PAD;
        let tools = [
            Tool::Select,
            Tool::Rect,
            Tool::Ellipse,
            Tool::Line,
            Tool::Arrow,
            Tool::Pen,
            Tool::Marker,
            Tool::Text,
            Tool::Counter,
            Tool::Pixelate,
            Tool::Picker,
        ];
        for t in tools {
            btns.push((Rect::new(bx, by, BTN, BTN), Btn::Tool(t)));
            bx += BTN + GAP;
        }
        bx += GAP * 2.0;
        for b in [Btn::Undo, Btn::Redo] {
            btns.push((Rect::new(bx, by, BTN, BTN), b));
            bx += BTN + GAP;
        }
        bx += GAP * 2.0;
        for b in [Btn::Copy, Btn::Save, Btn::SaveAs, Btn::Exit] {
            btns.push((Rect::new(bx, by, BTN, BTN), b));
            bx += BTN + GAP;
        }
        // hàng 2
        let mut bx = x + PAD;
        let by2 = by + BTN + PAD + 4.0;
        for i in 0..PALETTE.len() {
            btns.push((Rect::new(bx, by2, SWATCH, SWATCH), Btn::Color(i)));
            bx += SWATCH + GAP;
        }
        bx += GAP * 2.0;
        btns.push((Rect::new(bx, by2, BTN, SWATCH), Btn::Fill));
        bx += BTN + GAP;
        btns.push((Rect::new(bx, by2, BTN, SWATCH), Btn::Thick));
        bx += BTN + GAP;
        btns.push((Rect::new(bx, by2, 76.0, SWATCH), Btn::Ratio));
        Some((bar, btns))
    }

    fn toolbar_hit(&self, mi: usize, lx: f64, ly: f64) -> Option<Btn> {
        if mi != self.toolbar_monitor() {
            return None;
        }
        let (bar, btns) = self.toolbar_layout(mi)?;
        if !bar.contains(lx, ly) {
            return None;
        }
        btns.iter().find(|(r, _)| r.contains(lx, ly)).map(|(_, b)| *b)
    }

    fn toolbar_contains(&self, mi: usize, lx: f64, ly: f64) -> bool {
        if mi != self.toolbar_monitor() {
            return false;
        }
        match self.toolbar_layout(mi) {
            Some((bar, _)) => bar.contains(lx, ly),
            None => false,
        }
    }

    // ---------- Hành động ----------

    fn set_tool(&mut self, t: Tool) {
        self.finish_text();
        if self.tool != t {
            self.prev_tool = self.tool;
        }
        self.tool = t;
    }

    fn begin_text(&mut self, idx: usize) {
        self.editing = Some(idx);
        if let Some(im) = &self.im {
            im.focus_in();
        }
    }

    fn finish_text(&mut self) {
        if let Some(i) = self.editing.take() {
            if let Some(ShapeKind::Text(t)) = self.shapes.get(i).map(|s| &s.kind) {
                if t.trim().is_empty() {
                    self.shapes.remove(i);
                }
            }
            if let Some(im) = &self.im {
                im.focus_out();
                im.reset();
            }
        }
    }

    fn undo(&mut self) {
        self.finish_text();
        if let Some(s) = self.shapes.pop() {
            if let ShapeKind::Counter(_) = s.kind {
                self.counter = self.counter.saturating_sub(1);
            }
            self.redo.push(s);
        }
    }

    fn redo(&mut self) {
        self.finish_text();
        if let Some(s) = self.redo.pop() {
            if let ShapeKind::Counter(_) = s.kind {
                self.counter += 1;
            }
            self.shapes.push(s);
        }
    }

    fn pick_color(&mut self, cx: f64, cy: f64) {
        let x = cx.round().clamp(0.0, self.bounds.w - 1.0) as u32;
        let y = cy.round().clamp(0.0, self.bounds.h - 1.0) as u32;
        let p = self.base.get_pixel(x, y);
        let c = Color::rgb(p[0] as f64 / 255.0, p[1] as f64 / 255.0, p[2] as f64 / 255.0);
        self.color = c;
        let hex = c.to_hex();
        copy_text(&self.windows, &hex);
        self.set_status(format!("Đã lấy màu {hex} (đã copy)"));
        self.tool = if self.prev_tool == Tool::Picker { Tool::Select } else { self.prev_tool };
    }

    fn new_shape(&self, kind: ShapeKind, x: f64, y: f64) -> Shape {
        Shape {
            kind,
            x1: x,
            y1: y,
            x2: x,
            y2: y,
            color: self.color,
            width: self.thickness,
            filled: self.filled,
        }
    }

    // ---------- Sự kiện ----------

    fn on_press(&mut self, mi: usize, button: u32, lx: f64, ly: f64, state: gdk::ModifierType) -> Option<Action> {
        let mon = self.monitors[mi].clone();
        let (cx, cy) = mon.to_canvas(lx, ly);
        self.cursor = (cx, cy);
        self.cursor_mon = mi;

        if let Some(b) = self.toolbar_hit(mi, lx, ly) {
            if button == 1 {
                return self.on_button(b);
            }
            return None;
        }
        if self.toolbar_contains(mi, lx, ly) {
            return None;
        }

        if button == 3 {
            self.finish_text();
            if self.drag.is_some() {
                self.drag = None;
            } else if self.sel.is_some() {
                self.sel = None;
            } else {
                self.quit();
            }
            return None;
        }
        if button != 1 {
            return None;
        }

        if self.editing.is_some() {
            self.finish_text();
            return None;
        }

        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        match self.tool {
            Tool::Select => {
                if let Some(h) = self.handle_at(cx, cy, &mon) {
                    self.drag = Some(Drag::ResizeSel { handle: h, anchor: self.sel.unwrap() });
                } else if self.sel.map(|s| s.contains(cx, cy)).unwrap_or(false) {
                    let s = self.sel.unwrap();
                    self.drag = Some(Drag::MoveSel { dx: cx - s.x, dy: cy - s.y });
                } else {
                    self.drag = Some(Drag::NewSel { sx: cx, sy: cy });
                    self.sel = Some(Rect::new(cx, cy, 0.0, 0.0));
                }
            }
            Tool::Picker => {
                self.pick_color(cx, cy);
            }
            Tool::Text => {
                let sz = render::text_size(&self.new_shape(ShapeKind::Text(String::new()), 0.0, 0.0));
                let s = self.new_shape(ShapeKind::Text(String::new()), cx, cy - sz * 0.6);
                self.shapes.push(s);
                self.redo.clear();
                let idx = self.shapes.len() - 1;
                self.begin_text(idx);
            }
            Tool::Counter => {
                self.counter += 1;
                let s = self.new_shape(ShapeKind::Counter(self.counter), cx, cy);
                self.shapes.push(s);
                self.redo.clear();
            }
            Tool::Pen | Tool::Marker => {
                let kind = if self.tool == Tool::Pen {
                    ShapeKind::Pen(Vec::new())
                } else {
                    ShapeKind::Marker(Vec::new())
                };
                let s = self.new_shape(kind, cx, cy);
                self.shapes.push(s);
                self.redo.clear();
                self.drag = Some(Drag::Draw(self.shapes.len() - 1));
            }
            Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Arrow | Tool::Pixelate => {
                // Nếu chưa có vùng chọn, công cụ vẽ hoạt động như chọn vùng
                if self.sel.is_none() {
                    self.drag = Some(Drag::NewSel { sx: cx, sy: cy });
                    self.sel = Some(Rect::new(cx, cy, 0.0, 0.0));
                    let _ = shift;
                    return None;
                }
                let kind = match self.tool {
                    Tool::Rect => ShapeKind::Rect,
                    Tool::Ellipse => ShapeKind::Ellipse,
                    Tool::Line => ShapeKind::Line,
                    Tool::Arrow => ShapeKind::Arrow,
                    _ => ShapeKind::Pixelate,
                };
                let s = self.new_shape(kind, cx, cy);
                self.shapes.push(s);
                self.redo.clear();
                self.drag = Some(Drag::Draw(self.shapes.len() - 1));
            }
        }
        None
    }

    fn on_motion(&mut self, mi: usize, lx: f64, ly: f64, state: gdk::ModifierType) {
        let mon = self.monitors[mi].clone();
        let (cx, cy) = mon.to_canvas(lx, ly);
        self.cursor = (cx, cy);
        self.cursor_mon = mi;
        self.hover = self.toolbar_hit(mi, lx, ly);
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let b = self.bounds;
        let clampx = |x: f64| x.clamp(b.x, b.right());
        let clampy = |y: f64| y.clamp(b.y, b.bottom());
        match self.drag {
            Some(Drag::NewSel { sx, sy }) => {
                let r = Rect::from_points(sx, sy, clampx(cx), clampy(cy));
                let r = self.snap_ratio(r, sx, sy, shift);
                self.sel = Some(r.clamp_into(&b));
            }
            Some(Drag::MoveSel { dx, dy }) => {
                if let Some(mut s) = self.sel {
                    s.x = cx - dx;
                    s.y = cy - dy;
                    self.sel = Some(s.clamp_into(&b));
                }
            }
            Some(Drag::ResizeSel { handle, anchor }) => {
                let (px, py) = (clampx(cx), clampy(cy));
                // điểm neo đối diện với handle
                let (ax, ay) = match handle {
                    0 => (anchor.right(), anchor.bottom()),
                    1 => (anchor.x, anchor.bottom()),
                    2 => (anchor.x, anchor.bottom()),
                    3 => (anchor.x, anchor.y),
                    4 => (anchor.x, anchor.y),
                    5 => (anchor.x, anchor.y),
                    6 => (anchor.right(), anchor.y),
                    _ => (anchor.right(), anchor.y),
                };
                let mut r = match handle {
                    0 | 2 | 4 | 6 => Rect::from_points(ax, ay, px, py),
                    1 | 5 => Rect::from_points(anchor.x, ay, anchor.right(), py),
                    _ => Rect::from_points(ax, anchor.y, px, anchor.bottom()),
                };
                if shift || self.ratio.value().is_some() {
                    let v = if shift { 1.0 } else { self.ratio.value().unwrap() };
                    match handle {
                        1 | 5 => {
                            // kéo cạnh trên/dưới → giữ chiều cao, đổi chiều rộng quanh tâm
                            let w = r.h * v;
                            let mid = anchor.x + anchor.w / 2.0;
                            r.x = mid - w / 2.0;
                            r.w = w;
                        }
                        3 | 7 => {
                            let h = r.w / v;
                            let mid = anchor.y + anchor.h / 2.0;
                            r.y = mid - h / 2.0;
                            r.h = h;
                        }
                        _ => {
                            r = self.snap_ratio(r, ax, ay, shift);
                        }
                    }
                }
                self.sel = Some(r.clamp_into(&b));
            }
            Some(Drag::Draw(i)) => {
                if let Some(s) = self.shapes.get_mut(i) {
                    match &mut s.kind {
                        ShapeKind::Pen(pts) | ShapeKind::Marker(pts) => {
                            pts.push((cx, cy));
                            s.x2 = cx;
                            s.y2 = cy;
                        }
                        _ => {
                            if shift {
                                // Shift: vuông / góc 45°
                                let dx = cx - s.x1;
                                let dy = cy - s.y1;
                                match s.kind {
                                    ShapeKind::Line | ShapeKind::Arrow => {
                                        let ang = dy.atan2(dx);
                                        let step = std::f64::consts::FRAC_PI_4;
                                        let a = (ang / step).round() * step;
                                        let len = (dx * dx + dy * dy).sqrt();
                                        s.x2 = s.x1 + len * a.cos();
                                        s.y2 = s.y1 + len * a.sin();
                                    }
                                    _ => {
                                        let m = dx.abs().max(dy.abs());
                                        s.x2 = s.x1 + m * dx.signum();
                                        s.y2 = s.y1 + m * dy.signum();
                                    }
                                }
                            } else {
                                s.x2 = cx;
                                s.y2 = cy;
                            }
                        }
                    }
                }
            }
            None => {}
        }
    }

    fn on_release(&mut self, mi: usize, button: u32, lx: f64, ly: f64) {
        if button != 1 {
            return;
        }
        let mon = self.monitors[mi].clone();
        let (cx, cy) = mon.to_canvas(lx, ly);
        match self.drag.take() {
            Some(Drag::NewSel { sx, sy }) => {
                let tiny = (cx - sx).abs() < 3.0 * mon.sx && (cy - sy).abs() < 3.0 * mon.sy;
                if tiny {
                    // click không kéo → chọn cả màn hình dưới con trỏ
                    let m = self.monitor_at(sx, sy).unwrap_or(mi);
                    let mut r = self.monitors[m].canvas;
                    if let Some(v) = self.ratio.value() {
                        let h = (r.w / v).min(r.h);
                        let w = h * v;
                        r = Rect::new(r.x + (r.w - w) / 2.0, r.y + (r.h - h) / 2.0, w, h);
                    }
                    self.sel = Some(r.rounded());
                } else if let Some(s) = self.sel {
                    self.sel = Some(s.rounded());
                }
            }
            Some(Drag::MoveSel { .. }) | Some(Drag::ResizeSel { .. }) => {
                if let Some(s) = self.sel {
                    self.sel = Some(s.rounded());
                }
            }
            Some(Drag::Draw(i)) => {
                let remove = match self.shapes.get(i) {
                    Some(s) => match s.kind {
                        ShapeKind::Rect | ShapeKind::Ellipse | ShapeKind::Pixelate => {
                            (s.x2 - s.x1).abs() < 2.0 && (s.y2 - s.y1).abs() < 2.0
                        }
                        ShapeKind::Line | ShapeKind::Arrow => {
                            (s.x2 - s.x1).abs() < 2.0 && (s.y2 - s.y1).abs() < 2.0
                        }
                        _ => false,
                    },
                    None => false,
                };
                if remove {
                    self.shapes.remove(i);
                }
            }
            None => {}
        }
    }

    fn on_scroll(&mut self, dy: f64) {
        let step = if dy < 0.0 { 1.0 } else { -1.0 };
        self.thickness = (self.thickness + step).clamp(1.0, 40.0);
        if let Some(i) = self.editing {
            if let Some(s) = self.shapes.get_mut(i) {
                s.width = self.thickness;
            }
        }
        self.set_status(format!("Độ dày: {}", self.thickness));
    }

    fn on_button(&mut self, b: Btn) -> Option<Action> {
        match b {
            Btn::Tool(t) => self.set_tool(t),
            Btn::Undo => self.undo(),
            Btn::Redo => self.redo(),
            Btn::Copy => return Some(Action::Copy),
            Btn::Save => return Some(Action::Save(None)),
            Btn::SaveAs => {
                self.finish_text();
                return Some(Action::SaveAs);
            }
            Btn::Exit => self.quit(),
            Btn::Color(i) => {
                self.color = PALETTE[i];
                if let Some(idx) = self.editing {
                    if let Some(s) = self.shapes.get_mut(idx) {
                        s.color = self.color;
                    }
                }
            }
            Btn::Fill => self.filled = !self.filled,
            Btn::Thick => {
                self.thickness = if self.thickness >= 40.0 { 1.0 } else { (self.thickness + 2.0).min(40.0) };
            }
            Btn::Ratio => {
                self.ratio = self.ratio.next();
                self.set_status(format!("Tỉ lệ khung: {}", self.ratio.label()));
                if let (Some(v), Some(s)) = (self.ratio.value(), self.sel) {
                    self.sel = Some(self.fit_ratio(s, v));
                }
            }
        }
        None
    }

    fn on_key(&mut self, key: gdk::Key, state: gdk::ModifierType) -> Option<Action> {
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);

        if let Some(i) = self.editing {
            match key {
                gdk::Key::Escape | gdk::Key::Return | gdk::Key::KP_Enter => {
                    if key == gdk::Key::Return && shift {
                        if let Some(ShapeKind::Text(t)) = self.shapes.get_mut(i).map(|s| &mut s.kind) {
                            t.push('\n');
                        }
                    } else {
                        self.finish_text();
                    }
                }
                gdk::Key::BackSpace => {
                    if let Some(ShapeKind::Text(t)) = self.shapes.get_mut(i).map(|s| &mut s.kind) {
                        t.pop();
                    }
                }
                _ => {
                    // ký tự thường khi không có bộ gõ (IM) xử lý
                    if !ctrl {
                        if let Some(ch) = key.to_unicode() {
                            if !ch.is_control() {
                                if let Some(ShapeKind::Text(t)) = self.shapes.get_mut(i).map(|s| &mut s.kind) {
                                    t.push(ch);
                                }
                            }
                        }
                    }
                }
            }
            return None;
        }

        match key {
            gdk::Key::Escape => self.quit(),
            gdk::Key::Return | gdk::Key::KP_Enter => {
                if self.sel.is_some() {
                    return Some(if self.cfg.enter_copies { Action::Copy } else { Action::Save(None) });
                }
            }
            gdk::Key::c | gdk::Key::C if ctrl => return Some(Action::Copy),
            gdk::Key::s | gdk::Key::S if ctrl && shift => return Some(Action::SaveAs),
            gdk::Key::s | gdk::Key::S if ctrl => return Some(Action::Save(None)),
            gdk::Key::z | gdk::Key::Z if ctrl && shift => self.redo(),
            gdk::Key::z | gdk::Key::Z if ctrl => self.undo(),
            gdk::Key::y | gdk::Key::Y if ctrl => self.redo(),
            gdk::Key::a | gdk::Key::A if ctrl => self.sel = Some(self.bounds),
            gdk::Key::Delete => self.undo(),
            gdk::Key::Tab | gdk::Key::ISO_Left_Tab => {
                self.on_button(Btn::Ratio);
            }
            gdk::Key::space => {
                // Space: chọn màn hình dưới con trỏ
                let (cx, cy) = self.cursor;
                if let Some(m) = self.monitor_at(cx, cy) {
                    self.sel = Some(self.monitors[m].canvas);
                }
            }
            gdk::Key::Left | gdk::Key::Right | gdk::Key::Up | gdk::Key::Down => {
                let d = if ctrl { 10.0 } else { 1.0 };
                if let Some(mut s) = self.sel {
                    let (dx, dy) = match key {
                        gdk::Key::Left => (-d, 0.0),
                        gdk::Key::Right => (d, 0.0),
                        gdk::Key::Up => (0.0, -d),
                        _ => (0.0, d),
                    };
                    if shift {
                        s.w = (s.w + dx).max(1.0);
                        s.h = (s.h + dy).max(1.0);
                    } else {
                        s.x += dx;
                        s.y += dy;
                    }
                    self.sel = Some(s.clamp_into(&self.bounds));
                }
            }
            gdk::Key::bracketleft => self.on_scroll(1.0),
            gdk::Key::bracketright => self.on_scroll(-1.0),
            _ => {
                if let Some(ch) = key.to_unicode() {
                    self.on_char(ch);
                }
            }
        }
        None
    }

    /// Phím tắt một ký tự (không có Ctrl). Gọi từ on_key hoặc từ IM commit khi không gõ chữ.
    fn on_char(&mut self, ch: char) {
        match ch.to_ascii_lowercase() {
            's' => self.set_tool(Tool::Select),
            'r' => self.set_tool(Tool::Rect),
            'e' => self.set_tool(Tool::Ellipse),
            'l' => self.set_tool(Tool::Line),
            'a' => self.set_tool(Tool::Arrow),
            'p' => self.set_tool(Tool::Pen),
            'm' => self.set_tool(Tool::Marker),
            't' => self.set_tool(Tool::Text),
            'n' => self.set_tool(Tool::Counter),
            'b' => self.set_tool(Tool::Pixelate),
            'c' => self.set_tool(Tool::Picker),
            'f' => self.filled = !self.filled,
            '1'..='9' => self.color = PALETTE[(ch as u8 - b'1') as usize],
            '0' => self.color = PALETTE[9],
            '[' => self.on_scroll(1.0),
            ']' => self.on_scroll(-1.0),
            ' ' => {
                let (cx, cy) = self.cursor;
                if let Some(m) = self.monitor_at(cx, cy) {
                    self.sel = Some(self.monitors[m].canvas);
                }
            }
            _ => {}
        }
    }

    /// Văn bản từ bộ gõ (IM). Khi đang gõ chữ → thêm vào; nếu không → xem như phím tắt.
    fn on_commit(&mut self, text: &str) {
        if let Some(i) = self.editing {
            if let Some(ShapeKind::Text(t)) = self.shapes.get_mut(i).map(|s| &mut s.kind) {
                t.push_str(text);
            }
        } else {
            for ch in text.chars() {
                self.on_char(ch);
            }
        }
    }

    fn quit(&mut self) {
        self.finished = true;
        for w in &self.windows {
            w.close();
        }
        if let Some(app) = &self.app {
            app.quit();
        }
    }

    // ---------- Vẽ ----------

    fn draw(&self, mi: usize, cr: &cairo::Context, w: i32, h: i32) {
        let Some(mon) = self.monitors.get(mi) else { return };
        let (w, h) = (w as f64, h as f64);

        // --- Lớp canvas ---
        cr.save().ok();
        cr.scale(1.0 / mon.sx, 1.0 / mon.sy);
        cr.translate(-mon.canvas.x, -mon.canvas.y);
        cr.set_source_surface(&self.base_surf, 0.0, 0.0).ok();
        cr.paint().ok();
        render::draw_shapes(cr, &self.shapes, &self.base, self.editing);

        // lớp phủ tối ngoài vùng chọn
        cr.set_source_rgba(0.0, 0.0, 0.0, self.cfg.dim.clamp(0.0, 0.95));
        let vis = Rect::new(
            mon.canvas.x - 2.0,
            mon.canvas.y - 2.0,
            mon.canvas.w + 4.0,
            mon.canvas.h + 4.0,
        );
        cr.rectangle(vis.x, vis.y, vis.w, vis.h);
        if let Some(sel) = self.sel {
            cr.set_fill_rule(cairo::FillRule::EvenOdd);
            cr.rectangle(sel.x, sel.y, sel.w, sel.h);
        }
        cr.fill().ok();
        cr.set_fill_rule(cairo::FillRule::Winding);

        if let Some(sel) = self.sel {
            // viền vùng chọn
            cr.set_line_width(1.0 * mon.sx);
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.6);
            cr.rectangle(sel.x - 1.0 * mon.sx, sel.y - 1.0 * mon.sy, sel.w + 2.0 * mon.sx, sel.h + 2.0 * mon.sy);
            cr.stroke().ok();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.95);
            cr.rectangle(sel.x, sel.y, sel.w, sel.h);
            cr.stroke().ok();
            // tay cầm
            if self.tool == Tool::Select && self.drag.is_none() || matches!(self.drag, Some(Drag::ResizeSel { .. })) {
                let hs = HANDLE * mon.sx;
                for (hx, hy) in self.handles(&sel) {
                    cr.rectangle(hx - hs / 2.0, hy - hs / 2.0, hs, hs);
                    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                    cr.fill_preserve().ok();
                    cr.set_source_rgba(0.0, 0.0, 0.0, 0.7);
                    cr.stroke().ok();
                }
            }
        }
        cr.restore().ok();

        // --- Lớp giao diện (toạ độ logic) ---
        let font = pango::FontDescription::from_string("Sans 10");

        if let Some(sel) = self.sel {
            // nhãn kích thước
            let sr = sel.rounded();
            let label = format!("{} × {}", sr.w as i64, sr.h as i64);
            let (lx, ly) = mon.to_local(sel.x, sel.y);
            let label_mon = self.monitor_at(sel.x, sel.y).unwrap_or_else(|| self.toolbar_monitor());
            if label_mon == mi {
                let layout = pangocairo::functions::create_layout(cr);
                layout.set_font_description(Some(&font));
                layout.set_text(&label);
                let (_, ext) = layout.pixel_extents();
                let tw = ext.width() as f64 + 12.0;
                let th = ext.height() as f64 + 6.0;
                let mut bx = lx;
                let mut by = ly - th - 4.0;
                if by < 2.0 {
                    by = ly + 4.0;
                    bx = lx + 4.0;
                }
                bx = bx.clamp(2.0, (w - tw - 2.0).max(2.0));
                rounded_rect(cr, bx, by, tw, th, 4.0);
                cr.set_source_rgba(0.1, 0.1, 0.1, 0.85);
                cr.fill().ok();
                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                cr.move_to(bx + 6.0, by + 3.0);
                pangocairo::functions::show_layout(cr, &layout);
            }
        } else {
            // gợi ý
            let hint = "Kéo chuột để chọn vùng  •  Click = cả màn hình  •  Ctrl+A = tất cả màn hình  •  Esc = thoát";
            let layout = pangocairo::functions::create_layout(cr);
            layout.set_font_description(Some(&pango::FontDescription::from_string("Sans 12")));
            layout.set_text(hint);
            let (_, ext) = layout.pixel_extents();
            let tw = ext.width() as f64 + 24.0;
            let th = ext.height() as f64 + 14.0;
            let bx = (w - tw) / 2.0;
            let by = h * 0.08;
            rounded_rect(cr, bx, by, tw, th, 8.0);
            cr.set_source_rgba(0.1, 0.1, 0.1, 0.8);
            cr.fill().ok();
            cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
            cr.move_to(bx + 12.0, by + 7.0);
            pangocairo::functions::show_layout(cr, &layout);

            // toạ độ con trỏ
            if self.cursor_mon == mi {
                let (cx, cy) = self.cursor;
                let (lx, ly) = mon.to_local(cx, cy);
                let t = format!("{}, {}", cx.round() as i64, cy.round() as i64);
                let layout = pangocairo::functions::create_layout(cr);
                layout.set_font_description(Some(&font));
                layout.set_text(&t);
                let (_, ext) = layout.pixel_extents();
                let bw = ext.width() as f64 + 10.0;
                let bh = ext.height() as f64 + 4.0;
                let bx = (lx + 14.0).min(w - bw - 2.0);
                let by = (ly + 14.0).min(h - bh - 2.0);
                rounded_rect(cr, bx, by, bw, bh, 3.0);
                cr.set_source_rgba(0.1, 0.1, 0.1, 0.8);
                cr.fill().ok();
                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                cr.move_to(bx + 5.0, by + 2.0);
                pangocairo::functions::show_layout(cr, &layout);
            }
        }

        // thanh công cụ
        if mi == self.toolbar_monitor() && self.drag.is_none() {
            if let Some((bar, btns)) = self.toolbar_layout(mi) {
                self.draw_toolbar(cr, &bar, &btns);
            }
        }

        // thông báo trạng thái
        if let Some((msg, t)) = &self.status {
            if t.elapsed() < Duration::from_millis(2500) && mi == self.cursor_mon {
                let layout = pangocairo::functions::create_layout(cr);
                layout.set_font_description(Some(&pango::FontDescription::from_string("Sans 11")));
                layout.set_text(msg);
                let (_, ext) = layout.pixel_extents();
                let tw = ext.width() as f64 + 20.0;
                let th = ext.height() as f64 + 12.0;
                let bx = (w - tw) / 2.0;
                let by = if self.sel.is_some() { h * 0.08 } else { h * 0.08 + 50.0 };
                rounded_rect(cr, bx, by, tw, th, 6.0);
                cr.set_source_rgba(0.1, 0.1, 0.1, 0.85);
                cr.fill().ok();
                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                cr.move_to(bx + 10.0, by + 6.0);
                pangocairo::functions::show_layout(cr, &layout);
            }
        }
    }

    fn draw_toolbar(&self, cr: &cairo::Context, bar: &Rect, btns: &[(Rect, Btn)]) {
        rounded_rect(cr, bar.x, bar.y, bar.w, bar.h, 8.0);
        cr.set_source_rgba(0.13, 0.13, 0.15, 0.94);
        cr.fill_preserve().ok();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.12);
        cr.set_line_width(1.0);
        cr.stroke().ok();

        let mut tooltip: Option<(String, Rect)> = None;
        for (r, b) in btns {
            let active = match b {
                Btn::Tool(t) => *t == self.tool,
                Btn::Fill => self.filled,
                Btn::Color(i) => PALETTE[*i] == self.color,
                _ => false,
            };
            let hovered = self.hover == Some(*b);
            if let Btn::Color(i) = b {
                let c = PALETTE[*i];
                rounded_rect(cr, r.x, r.y, r.w, r.h, 4.0);
                cr.set_source_rgb(c.r, c.g, c.b);
                cr.fill().ok();
                if active || hovered {
                    rounded_rect(cr, r.x - 1.5, r.y - 1.5, r.w + 3.0, r.h + 3.0, 5.0);
                    cr.set_source_rgba(1.0, 1.0, 1.0, if active { 1.0 } else { 0.5 });
                    cr.set_line_width(2.0);
                    cr.stroke().ok();
                }
            } else {
                if active || hovered {
                    rounded_rect(cr, r.x, r.y, r.w, r.h, 6.0);
                    if active {
                        cr.set_source_rgba(0.25, 0.55, 1.0, 0.9);
                    } else {
                        cr.set_source_rgba(1.0, 1.0, 1.0, 0.15);
                    }
                    cr.fill().ok();
                }
                draw_icon(cr, *b, r, self);
            }
            if hovered {
                tooltip = Some((b.label(), *r));
            }
        }
        if let Some((text, r)) = tooltip {
            let layout = pangocairo::functions::create_layout(cr);
            layout.set_font_description(Some(&pango::FontDescription::from_string("Sans 10")));
            layout.set_text(&text);
            let (_, ext) = layout.pixel_extents();
            let tw = ext.width() as f64 + 14.0;
            let th = ext.height() as f64 + 8.0;
            let bx = (r.x + r.w / 2.0 - tw / 2.0).max(bar.x);
            let by = bar.bottom() + 6.0;
            rounded_rect(cr, bx, by, tw, th, 5.0);
            cr.set_source_rgba(0.1, 0.1, 0.1, 0.92);
            cr.fill().ok();
            cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
            cr.move_to(bx + 7.0, by + 4.0);
            pangocairo::functions::show_layout(cr, &layout);
        }
    }
}

fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    cr.arc(x + r, y + r, r, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
    cr.close_path();
}

/// Vẽ icon đơn giản bằng cairo cho từng nút.
fn draw_icon(cr: &cairo::Context, b: Btn, r: &Rect, ov: &Overlay) {
    cr.save().ok();
    let cx = r.x + r.w / 2.0;
    let cy = r.y + r.h / 2.0;
    let s = r.h.min(r.w) * 0.28; // bán kính icon
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.92);
    cr.set_line_width(2.0);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    let small = pango::FontDescription::from_string("Sans Bold 9");
    match b {
        Btn::Tool(Tool::Select) => {
            cr.set_dash(&[3.0, 3.0], 0.0);
            cr.rectangle(cx - s, cy - s, 2.0 * s, 2.0 * s);
            cr.stroke().ok();
        }
        Btn::Tool(Tool::Rect) => {
            cr.rectangle(cx - s, cy - s * 0.8, 2.0 * s, 1.6 * s);
            cr.stroke().ok();
        }
        Btn::Tool(Tool::Ellipse) => {
            cr.save().ok();
            cr.translate(cx, cy);
            cr.scale(s, s * 0.8);
            cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
            cr.restore().ok();
            cr.stroke().ok();
        }
        Btn::Tool(Tool::Line) => {
            cr.move_to(cx - s, cy + s);
            cr.line_to(cx + s, cy - s);
            cr.stroke().ok();
        }
        Btn::Tool(Tool::Arrow) => {
            cr.move_to(cx - s, cy + s);
            cr.line_to(cx + s, cy - s);
            cr.stroke().ok();
            cr.move_to(cx + s, cy - s);
            cr.line_to(cx + s * 0.1, cy - s);
            cr.move_to(cx + s, cy - s);
            cr.line_to(cx + s, cy - s * 0.1);
            cr.stroke().ok();
        }
        Btn::Tool(Tool::Pen) => {
            cr.move_to(cx - s, cy + s * 0.6);
            cr.curve_to(cx - s * 0.3, cy - s * 1.2, cx + s * 0.3, cy + s * 1.2, cx + s, cy - s * 0.6);
            cr.stroke().ok();
        }
        Btn::Tool(Tool::Marker) => {
            cr.set_line_width(7.0);
            cr.set_source_rgba(1.0, 0.9, 0.2, 0.6);
            cr.move_to(cx - s, cy);
            cr.line_to(cx + s, cy);
            cr.stroke().ok();
            cr.set_line_width(2.0);
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.92);
            cr.move_to(cx - s * 0.8, cy - s * 0.8);
            cr.line_to(cx + s * 0.8, cy + s * 0.8);
            cr.stroke().ok();
        }
        Btn::Tool(Tool::Text) => {
            let layout = pangocairo::functions::create_layout(cr);
            layout.set_font_description(Some(&pango::FontDescription::from_string("Sans Bold 15")));
            layout.set_text("T");
            center_layout(cr, &layout, cx, cy);
        }
        Btn::Tool(Tool::Counter) => {
            cr.arc(cx, cy, s, 0.0, std::f64::consts::TAU);
            cr.stroke().ok();
            let layout = pangocairo::functions::create_layout(cr);
            layout.set_font_description(Some(&small));
            layout.set_text(&(ov.counter + 1).to_string());
            center_layout(cr, &layout, cx, cy);
        }
        Btn::Tool(Tool::Pixelate) => {
            let n = 4;
            let cell = 2.0 * s / n as f64;
            for i in 0..n {
                for j in 0..n {
                    let a = if (i + j) % 2 == 0 { 0.9 } else { 0.35 };
                    cr.set_source_rgba(1.0, 1.0, 1.0, a);
                    cr.rectangle(cx - s + i as f64 * cell, cy - s + j as f64 * cell, cell, cell);
                    cr.fill().ok();
                }
            }
        }
        Btn::Tool(Tool::Picker) => {
            cr.move_to(cx - s, cy + s);
            cr.line_to(cx + s * 0.2, cy - s * 0.2);
            cr.stroke().ok();
            cr.set_line_width(4.0);
            cr.move_to(cx + s * 0.1, cy - s * 0.1);
            cr.line_to(cx + s * 0.8, cy - s * 0.8);
            cr.stroke().ok();
            let c = ov.color;
            cr.set_source_rgb(c.r, c.g, c.b);
            cr.rectangle(r.x + 3.0, r.bottom() - 7.0, r.w - 6.0, 4.0);
            cr.fill().ok();
        }
        Btn::Undo | Btn::Redo => {
            let dir = if b == Btn::Undo { -1.0 } else { 1.0 };
            cr.arc_negative(cx, cy + s * 0.3, s, 0.0_f64.max(0.1), std::f64::consts::PI);
            cr.new_path();
            cr.save().ok();
            cr.translate(cx, cy);
            cr.scale(dir, 1.0);
            cr.arc(0.0, s * 0.3, s, std::f64::consts::PI * 1.05, std::f64::consts::PI * 1.9);
            cr.stroke().ok();
            let (ax, ay) = (-s * 0.95, s * 0.3 - s * 0.35);
            cr.move_to(ax - s * 0.35, ay - s * 0.5);
            cr.line_to(ax, ay + s * 0.15);
            cr.line_to(ax + s * 0.55, ay - s * 0.35);
            cr.stroke().ok();
            cr.restore().ok();
        }
        Btn::Copy => {
            cr.rectangle(cx - s, cy - s, s * 1.3, s * 1.3);
            cr.stroke().ok();
            cr.rectangle(cx - s * 0.3, cy - s * 0.3, s * 1.3, s * 1.3);
            cr.set_source_rgba(0.13, 0.13, 0.15, 1.0);
            cr.fill_preserve().ok();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.92);
            cr.stroke().ok();
        }
        Btn::Save | Btn::SaveAs => {
            // đĩa mềm
            cr.move_to(cx - s, cy - s);
            cr.line_to(cx + s * 0.6, cy - s);
            cr.line_to(cx + s, cy - s * 0.6);
            cr.line_to(cx + s, cy + s);
            cr.line_to(cx - s, cy + s);
            cr.close_path();
            cr.stroke().ok();
            cr.rectangle(cx - s * 0.5, cy + s * 0.2, s, s * 0.8);
            cr.stroke().ok();
            cr.rectangle(cx - s * 0.6, cy - s, s * 1.0, s * 0.5);
            cr.fill().ok();
            if b == Btn::SaveAs {
                cr.set_source_rgba(0.13, 0.13, 0.15, 1.0);
                cr.arc(cx + s * 0.9, cy + s * 0.9, s * 0.6, 0.0, std::f64::consts::TAU);
                cr.fill().ok();
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.92);
                let layout = pangocairo::functions::create_layout(cr);
                layout.set_font_description(Some(&small));
                layout.set_text("…");
                center_layout(cr, &layout, cx + s * 0.9, cy + s * 0.7);
            }
        }
        Btn::Exit => {
            cr.set_source_rgba(1.0, 0.45, 0.45, 1.0);
            cr.move_to(cx - s, cy - s);
            cr.line_to(cx + s, cy + s);
            cr.move_to(cx + s, cy - s);
            cr.line_to(cx - s, cy + s);
            cr.stroke().ok();
        }
        Btn::Fill => {
            cr.rectangle(cx - s, cy - s * 0.7, 2.0 * s, 1.4 * s);
            if ov.filled {
                cr.fill().ok();
            } else {
                cr.stroke().ok();
            }
        }
        Btn::Thick => {
            let lw = (ov.thickness / 40.0 * (r.h - 6.0)).clamp(1.5, r.h - 6.0);
            cr.set_line_width(lw);
            cr.move_to(cx - s * 0.7, cy);
            cr.line_to(cx + s * 0.7, cy);
            cr.stroke().ok();
            let layout = pangocairo::functions::create_layout(cr);
            layout.set_font_description(Some(&pango::FontDescription::from_string("Sans 7")));
            layout.set_text(&format!("{}", ov.thickness as i64));
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
            let (_, ext) = layout.pixel_extents();
            cr.move_to(r.right() - ext.width() as f64 - 2.0, r.y + 1.0);
            pangocairo::functions::show_layout(cr, &layout);
        }
        Btn::Ratio => {
            let layout = pangocairo::functions::create_layout(cr);
            layout.set_font_description(Some(&small));
            layout.set_text(&format!("▭ {}", ov.ratio.label()));
            center_layout(cr, &layout, cx, cy);
        }
        Btn::Color(_) => {}
    }
    cr.restore().ok();
}

fn center_layout(cr: &cairo::Context, layout: &pango::Layout, cx: f64, cy: f64) {
    let (_, ext) = layout.pixel_extents();
    cr.move_to(cx - ext.width() as f64 / 2.0, cy - ext.height() as f64 / 2.0);
    pangocairo::functions::show_layout(cr, layout);
}

fn copy_text(windows: &[gtk::Window], text: &str) {
    let ok = match output::copy_text_external(text) {
        true => true,
        false => false,
    };
    if !ok {
        if let Some(w) = windows.first() {
            WidgetExt::display(w).clipboard().set_text(text);
        }
    }
}

thread_local! {
    static PENDING_SAVE_AS: RefCell<Option<Shared>> = const { RefCell::new(None) };
}

fn save_as_dialog(win: Option<gtk::Window>) {
    let Some(win) = win else { return };
    PENDING_SAVE_AS.with(|p| {
        let Some(shared) = p.borrow().clone() else { return };
        let (dir, name) = {
            let ov = shared.borrow();
            let p = output::default_save_path(&ov.cfg);
            (
                p.parent().map(|d| d.to_path_buf()),
                p.file_name().map(|n| n.to_string_lossy().to_string()),
            )
        };
        let dialog = gtk::FileDialog::new();
        dialog.set_title("Lưu ảnh chụp");
        if let Some(d) = dir {
            let _ = std::fs::create_dir_all(&d);
            dialog.set_initial_folder(Some(&gtk::gio::File::for_path(d)));
        }
        if let Some(n) = name {
            dialog.set_initial_name(Some(&n));
        }
        let shared2 = shared.clone();
        dialog.save(Some(&win), gtk::gio::Cancellable::NONE, move |res| {
            if let Ok(file) = res {
                if let Some(path) = file.path() {
                    perform(&shared2, Action::Save(Some(path)));
                }
            }
        });
    });
}

/// Thực hiện hành động cuối (copy / lưu) rồi thoát.
pub fn perform(shared: &Shared, action: Action) {
    if let Action::SaveAs = action {
        let has_sel = shared.borrow().sel.is_some();
        if has_sel {
            let win = shared.borrow().windows.first().cloned();
            save_as_dialog(win);
        } else {
            let mut ov = shared.borrow_mut();
            ov.set_status("Hãy chọn vùng trước");
            ov.redraw_all();
        }
        return;
    }
    let (img, cfg, windows, app) = {
        let mut ov = shared.borrow_mut();
        ov.finish_text();
        let Some(sel) = ov.sel else {
            ov.set_status("Hãy chọn vùng trước");
            ov.redraw_all();
            return;
        };
        if ov.finished {
            return;
        }
        ov.finished = true;
        let img = render::render_final(&ov.base, &ov.base_surf, &sel, &ov.shapes);
        (img, ov.cfg.clone(), ov.windows.clone(), ov.app.clone())
    };
    for w in &windows {
        w.set_visible(false);
    }

    let mut saved_path: Option<PathBuf> = None;
    let mut need_copy = false;
    match &action {
        Action::Copy => need_copy = true,
        Action::SaveAs => unreachable!(),
        Action::Save(p) => {
            let path = p.clone().unwrap_or_else(|| output::default_save_path(&cfg));
            match output::save_png(&img, &path) {
                Ok(()) => {
                    println!("{}", path.display());
                    saved_path = Some(path);
                    need_copy = cfg.copy_on_save;
                }
                Err(e) => {
                    eprintln!("quickshot: {e}");
                    if cfg.notify {
                        output::notify("quickshot", &format!("Lưu thất bại: {e}"), None);
                    }
                }
            }
        }
    }

    let mut keep_alive = false;
    if need_copy {
        match output::encode_png(&img) {
            Ok(png) => match output::copy_png_external(&png) {
                output::ClipResult::Done => {}
                output::ClipResult::NeedGtk => {
                    // Không có wl-copy/xclip: GTK giữ clipboard, tiến trình sống tới khi
                    // app khác ghi đè clipboard (GNOME không có clipboard manager).
                    if let Some(w) = windows.first() {
                        let (iw, ih) = (img.width() as i32, img.height() as i32);
                        let bytes = glib::Bytes::from_owned(img.clone().into_raw());
                        let tex = gdk::MemoryTexture::new(iw, ih, gdk::MemoryFormat::R8g8b8a8, &bytes, (iw * 4) as usize);
                        let clip = WidgetExt::display(w).clipboard();
                        clip.set_texture(&tex);
                        keep_alive = true;
                        let app2 = app.clone();
                        clip.connect_changed(move |c| {
                            if !c.is_local() {
                                if let Some(a) = &app2 {
                                    a.quit();
                                }
                            }
                        });
                        let app3 = app.clone();
                        glib::timeout_add_local_once(Duration::from_secs(1800), move || {
                            if let Some(a) = &app3 {
                                a.quit();
                            }
                        });
                        eprintln!(
                            "quickshot: không thấy wl-copy/xclip — giữ tiến trình để clipboard còn hiệu lực \
                             (cài 'wl-clipboard' để không cần vậy)."
                        );
                    }
                }
            },
            Err(e) => eprintln!("quickshot: mã hoá PNG lỗi: {e}"),
        }
    }

    if cfg.notify {
        let sr = format!("{}×{}", img.width(), img.height());
        match (&saved_path, need_copy) {
            (Some(p), true) => output::notify(
                "Đã lưu và copy ảnh",
                &format!("{sr} — {}", p.display()),
                Some(p),
            ),
            (Some(p), false) => output::notify("Đã lưu ảnh", &format!("{sr} — {}", p.display()), Some(p)),
            (None, true) => output::notify("Đã copy ảnh vào clipboard", &sr, None),
            (None, false) => {}
        }
    }

    if !keep_alive {
        for w in &windows {
            w.close();
        }
        if let Some(a) = app {
            a.quit();
        }
    }
}

/// Tạo cửa sổ trên từng màn hình và chạy vòng lặp GTK.
pub fn run(overlay: Overlay) -> i32 {
    let shared: Shared = Rc::new(RefCell::new(overlay));
    PENDING_SAVE_AS.with(|p| *p.borrow_mut() = Some(shared.clone()));

    let app = gtk::Application::new(Some(crate::DESKTOP_ID), gtk::gio::ApplicationFlags::NON_UNIQUE);
    let shared_act = shared.clone();
    app.connect_activate(move |app| {
        build_windows(app, &shared_act);
    });
    let code = app.run_with_args::<&str>(&[]);
    code.into()
}

fn build_windows(app: &gtk::Application, shared: &Shared) {
    let display = gdk::Display::default().expect("không mở được display");
    let monitors = shared.borrow_mut().detect_monitors(&display);
    shared.borrow_mut().app = Some(app.clone());

    let im = gtk::IMMulticontext::new();
    {
        let sh = shared.clone();
        im.connect_commit(move |_, text| {
            let mut ov = sh.borrow_mut();
            ov.on_commit(text);
            ov.redraw_all();
        });
    }
    shared.borrow_mut().im = Some(im.clone());

    for (mi, monitor) in monitors.iter().enumerate() {
        let win = gtk::ApplicationWindow::new(app);
        let win: gtk::Window = win.upcast();
        win.set_title(Some("quickshot"));
        win.set_decorated(false);
        win.set_resizable(true);
        let g = monitor.geometry();
        win.set_default_size(g.width(), g.height());

        let area = gtk::DrawingArea::new();
        area.set_hexpand(true);
        area.set_vexpand(true);
        area.set_focusable(true);
        area.set_cursor_from_name(Some("crosshair"));
        win.set_child(Some(&area));

        {
            let sh = shared.clone();
            area.set_draw_func(move |_, cr, w, h| {
                sh.borrow().draw(mi, cr, w, h);
            });
        }

        // Click
        {
            let click = gtk::GestureClick::new();
            click.set_button(0);
            let sh = shared.clone();
            click.connect_pressed(move |g, _n, x, y| {
                let state = g.current_event_state();
                let action = {
                    let mut ov = sh.borrow_mut();
                    let a = ov.on_press(mi, g.current_button(), x, y, state);
                    ov.redraw_all();
                    a
                };
                if let Some(a) = action {
                    perform(&sh, a);
                }
            });
            let sh = shared.clone();
            click.connect_released(move |g, _n, x, y| {
                let mut ov = sh.borrow_mut();
                ov.on_release(mi, g.current_button(), x, y);
                ov.redraw_all();
            });
            area.add_controller(click);
        }
        // Di chuyển chuột
        {
            let motion = gtk::EventControllerMotion::new();
            let sh = shared.clone();
            let area2 = area.clone();
            motion.connect_motion(move |c, x, y| {
                let state = c.current_event_state();
                let mut ov = sh.borrow_mut();
                let prev_hover = ov.hover;
                ov.on_motion(mi, x, y, state);
                update_cursor(&ov, mi, x, y, &area2);
                if ov.drag.is_some() {
                    ov.redraw_all();
                } else if prev_hover != ov.hover || ov.sel.is_none() {
                    area2.queue_draw();
                }
            });
            area.add_controller(motion);
        }
        // Cuộn = độ dày
        {
            let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
            let sh = shared.clone();
            scroll.connect_scroll(move |_, _dx, dy| {
                let mut ov = sh.borrow_mut();
                ov.on_scroll(dy);
                ov.redraw_all();
                glib::Propagation::Stop
            });
            area.add_controller(scroll);
        }
        // Phím
        {
            let key = gtk::EventControllerKey::new();
            key.set_im_context(Some(&im));
            let sh = shared.clone();
            key.connect_key_pressed(move |_, k, _code, state| {
                if sh.borrow().debug {
                    eprintln!("[key] {:?} state={:?}", k.name(), state);
                }
                let action = {
                    let mut ov = sh.borrow_mut();
                    let a = ov.on_key(k, state);
                    ov.redraw_all();
                    a
                };
                if let Some(a) = action {
                    perform(&sh, a);
                }
                glib::Propagation::Stop
            });
            win.add_controller(key);
        }

        win.fullscreen_on_monitor(monitor);
        win.present();
        area.grab_focus();

        let mut ov = shared.borrow_mut();
        ov.windows.push(win);
        ov.areas.push(area);
    }
    im.set_client_widget(shared.borrow().areas.first());

    // đồng hồ để ẩn thông báo trạng thái
    let sh = shared.clone();
    glib::timeout_add_local(Duration::from_millis(500), move || {
        let ov = sh.borrow();
        if ov.finished {
            return glib::ControlFlow::Break;
        }
        if let Some((_, t)) = &ov.status {
            if t.elapsed() < Duration::from_millis(3000) {
                ov.redraw_all();
            }
        }
        glib::ControlFlow::Continue
    });
}

fn update_cursor(ov: &Overlay, mi: usize, lx: f64, ly: f64, area: &gtk::DrawingArea) {
    let name = if ov.hover.is_some() || ov.toolbar_contains(mi, lx, ly) {
        "default"
    } else {
        let mon = &ov.monitors[mi];
        let (cx, cy) = mon.to_canvas(lx, ly);
        match ov.drag {
            Some(Drag::MoveSel { .. }) => "grabbing",
            Some(Drag::ResizeSel { handle, .. }) => Overlay::cursor_name_for_handle(handle),
            Some(_) => "crosshair",
            None => match ov.tool {
                Tool::Select => {
                    if let Some(h) = ov.handle_at(cx, cy, mon) {
                        Overlay::cursor_name_for_handle(h)
                    } else if ov.sel.map(|s| s.contains(cx, cy)).unwrap_or(false) {
                        "grab"
                    } else {
                        "crosshair"
                    }
                }
                Tool::Text => "text",
                Tool::Picker => "cell",
                _ => "crosshair",
            },
        }
    };
    area.set_cursor_from_name(Some(name));
}
