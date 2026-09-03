//! Dữ liệu dùng chung: hình chữ nhật, màu, công cụ, chú thích.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[allow(dead_code)]
impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Rect { x, y, w, h }
    }
    /// Tạo rect chuẩn hoá từ 2 điểm bất kỳ.
    pub fn from_points(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Rect {
            x: x1.min(x2),
            y: y1.min(y2),
            w: (x2 - x1).abs(),
            h: (y2 - y1).abs(),
        }
    }
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && py >= self.y && px < self.right() && py < self.bottom()
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
    pub fn intersect(&self, o: &Rect) -> Option<Rect> {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        if r > x && b > y {
            Some(Rect::new(x, y, r - x, b - y))
        } else {
            None
        }
    }
    /// Giới hạn rect trong một rect khác (dùng để không chọn ra ngoài ảnh).
    pub fn clamp_into(&self, bounds: &Rect) -> Rect {
        let mut r = *self;
        if r.w > bounds.w {
            r.w = bounds.w;
        }
        if r.h > bounds.h {
            r.h = bounds.h;
        }
        if r.x < bounds.x {
            r.x = bounds.x;
        }
        if r.y < bounds.y {
            r.y = bounds.y;
        }
        if r.right() > bounds.right() {
            r.x = bounds.right() - r.w;
        }
        if r.bottom() > bounds.bottom() {
            r.y = bounds.bottom() - r.h;
        }
        r
    }
    pub fn rounded(&self) -> Rect {
        let x = self.x.round();
        let y = self.y.round();
        Rect::new(x, y, (self.right().round() - x).max(1.0), (self.bottom().round() - y).max(1.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    pub const fn rgb(r: f64, g: f64, b: f64) -> Self {
        Color { r, g, b, a: 1.0 }
    }
    pub fn with_alpha(self, a: f64) -> Self {
        Color { a, ..self }
    }
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('#');
        if s.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(s, 16).ok()?;
        Some(Color::rgb(
            ((v >> 16) & 0xff) as f64 / 255.0,
            ((v >> 8) & 0xff) as f64 / 255.0,
            (v & 0xff) as f64 / 255.0,
        ))
    }
    pub fn to_hex(&self) -> String {
        format!(
            "#{:02X}{:02X}{:02X}",
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8
        )
    }
    /// Màu chữ tương phản (đen/trắng) để vẽ số lên nền màu này.
    pub fn contrast(&self) -> Color {
        let l = 0.299 * self.r + 0.587 * self.g + 0.114 * self.b;
        if l > 0.6 {
            Color::rgb(0.0, 0.0, 0.0)
        } else {
            Color::rgb(1.0, 1.0, 1.0)
        }
    }
}

/// Bảng màu nhanh trên thanh công cụ (giống Flameshot).
pub const PALETTE: [Color; 10] = [
    Color::rgb(1.0, 0.0, 0.0),
    Color::rgb(1.0, 0.55, 0.0),
    Color::rgb(1.0, 0.9, 0.0),
    Color::rgb(0.2, 0.8, 0.2),
    Color::rgb(0.0, 0.6, 1.0),
    Color::rgb(0.55, 0.3, 0.9),
    Color::rgb(1.0, 0.3, 0.7),
    Color::rgb(1.0, 1.0, 1.0),
    Color::rgb(0.5, 0.5, 0.5),
    Color::rgb(0.0, 0.0, 0.0),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Rect,
    Ellipse,
    Line,
    Arrow,
    Pen,
    Marker,
    Text,
    Counter,
    Pixelate,
    Picker,
}

impl Tool {
    pub fn label(&self) -> &'static str {
        match self {
            Tool::Select => "Chọn vùng (S)",
            Tool::Rect => "Hình chữ nhật (R)",
            Tool::Ellipse => "Hình elip (E)",
            Tool::Line => "Đường thẳng (L)",
            Tool::Arrow => "Mũi tên (A)",
            Tool::Pen => "Bút vẽ (P)",
            Tool::Marker => "Bút dạ quang (M)",
            Tool::Text => "Chữ (T)",
            Tool::Counter => "Đánh số (N)",
            Tool::Pixelate => "Làm mờ / pixel hoá (B)",
            Tool::Picker => "Lấy màu (C)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ratio {
    Free,
    R1x1,
    R4x3,
    R16x9,
    R3x2,
    R9x16,
}

impl Ratio {
    pub const ALL: [Ratio; 6] = [
        Ratio::Free,
        Ratio::R1x1,
        Ratio::R4x3,
        Ratio::R16x9,
        Ratio::R3x2,
        Ratio::R9x16,
    ];
    pub fn value(&self) -> Option<f64> {
        match self {
            Ratio::Free => None,
            Ratio::R1x1 => Some(1.0),
            Ratio::R4x3 => Some(4.0 / 3.0),
            Ratio::R16x9 => Some(16.0 / 9.0),
            Ratio::R3x2 => Some(3.0 / 2.0),
            Ratio::R9x16 => Some(9.0 / 16.0),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Ratio::Free => "Tự do",
            Ratio::R1x1 => "1:1",
            Ratio::R4x3 => "4:3",
            Ratio::R16x9 => "16:9",
            Ratio::R3x2 => "3:2",
            Ratio::R9x16 => "9:16",
        }
    }
    pub fn next(&self) -> Ratio {
        let i = Ratio::ALL.iter().position(|r| r == self).unwrap_or(0);
        Ratio::ALL[(i + 1) % Ratio::ALL.len()]
    }
    pub fn parse(s: &str) -> Option<Ratio> {
        match s.trim().to_ascii_lowercase().as_str() {
            "free" | "0" | "tudo" | "tự do" => Some(Ratio::Free),
            "1:1" | "1x1" | "square" => Some(Ratio::R1x1),
            "4:3" | "4x3" => Some(Ratio::R4x3),
            "16:9" | "16x9" => Some(Ratio::R16x9),
            "3:2" | "3x2" => Some(Ratio::R3x2),
            "9:16" | "9x16" => Some(Ratio::R9x16),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Line,
    Arrow,
    /// Bút vẽ tự do (danh sách điểm).
    Pen(Vec<(f64, f64)>),
    Marker(Vec<(f64, f64)>),
    Text(String),
    Counter(u32),
    Pixelate,
}

/// Một chú thích, toạ độ tính theo pixel của ảnh chụp (canvas).
#[derive(Clone, Debug)]
pub struct Shape {
    pub kind: ShapeKind,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub color: Color,
    /// Độ dày nét (px canvas) — với Text là cỡ chữ, với Pixelate là kích thước ô.
    pub width: f64,
    pub filled: bool,
}

#[allow(dead_code)]
impl Shape {
    pub fn bbox(&self) -> Rect {
        match &self.kind {
            ShapeKind::Pen(pts) | ShapeKind::Marker(pts) => {
                let mut r = Rect::from_points(self.x1, self.y1, self.x2, self.y2);
                for &(x, y) in pts {
                    let nx = r.x.min(x);
                    let ny = r.y.min(y);
                    let rx = r.right().max(x);
                    let by = r.bottom().max(y);
                    r = Rect::new(nx, ny, rx - nx, by - ny);
                }
                r
            }
            _ => Rect::from_points(self.x1, self.y1, self.x2, self.y2),
        }
    }
}

/// Thông tin một màn hình: vị trí logic (GTK) và vùng tương ứng trên ảnh chụp.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    /// Toạ độ logic (GTK/GDK).
    pub logical: Rect,
    /// Vùng trên ảnh chụp (pixel vật lý).
    pub canvas: Rect,
    /// Tỉ lệ pixel canvas / pixel logic theo x, y.
    pub sx: f64,
    pub sy: f64,
}

impl MonitorInfo {
    pub fn to_canvas(&self, lx: f64, ly: f64) -> (f64, f64) {
        (self.canvas.x + lx * self.sx, self.canvas.y + ly * self.sy)
    }
    pub fn to_local(&self, cx: f64, cy: f64) -> (f64, f64) {
        ((cx - self.canvas.x) / self.sx, (cy - self.canvas.y) / self.sy)
    }
}
