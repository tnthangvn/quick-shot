//! Vẽ ảnh nền + chú thích bằng cairo. Toạ độ luôn là pixel canvas (ảnh chụp).

use crate::model::*;
use gtk4::cairo;
use gtk4::pango;
use image::RgbaImage;

/// Chuyển ảnh RGBA sang cairo ImageSurface (ARGB32 premultiplied).
pub fn base_surface(img: &RgbaImage) -> cairo::ImageSurface {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let stride = cairo::Format::ARgb32.stride_for_width(w as u32).unwrap();
    let mut data = vec![0u8; (stride * h) as usize];
    let raw = img.as_raw();
    for y in 0..h as usize {
        let row = &raw[y * w as usize * 4..(y + 1) * w as usize * 4];
        let dst = &mut data[y * stride as usize..y * stride as usize + w as usize * 4];
        for x in 0..w as usize {
            let r = row[x * 4] as u32;
            let g = row[x * 4 + 1] as u32;
            let b = row[x * 4 + 2] as u32;
            let a = row[x * 4 + 3] as u32;
            dst[x * 4] = ((b * a + 127) / 255) as u8;
            dst[x * 4 + 1] = ((g * a + 127) / 255) as u8;
            dst[x * 4 + 2] = ((r * a + 127) / 255) as u8;
            dst[x * 4 + 3] = a as u8;
        }
    }
    cairo::ImageSurface::create_for_data(data, cairo::Format::ARgb32, w, h, stride)
        .expect("tạo cairo surface")
}

/// Chuyển cairo ImageSurface (ARGB32 premultiplied) về RGBA.
pub fn surface_to_image(surf: &mut cairo::ImageSurface) -> RgbaImage {
    surf.flush();
    let w = surf.width() as u32;
    let h = surf.height() as u32;
    let stride = surf.stride() as usize;
    let data = surf.data().expect("đọc dữ liệu surface");
    let mut out = RgbaImage::new(w, h);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = y * stride + x * 4;
            let b = data[i] as u32;
            let g = data[i + 1] as u32;
            let r = data[i + 2] as u32;
            let a = data[i + 3] as u32;
            let (r, g, b) = if a == 0 || a == 255 {
                (r, g, b)
            } else {
                ((r * 255 + a / 2) / a, (g * 255 + a / 2) / a, (b * 255 + a / 2) / a)
            };
            out.put_pixel(x as u32, y as u32, image::Rgba([r as u8, g as u8, b as u8, a as u8]));
        }
    }
    out
}

pub fn set_color(cr: &cairo::Context, c: &Color) {
    cr.set_source_rgba(c.r, c.g, c.b, c.a);
}

pub fn text_font(size: f64) -> pango::FontDescription {
    let mut fd = pango::FontDescription::new();
    fd.set_family("Sans");
    fd.set_weight(pango::Weight::Bold);
    fd.set_absolute_size(size * pango::SCALE as f64);
    fd
}

pub fn text_size(shape: &Shape) -> f64 {
    (12.0 + shape.width * 3.0).max(10.0)
}

pub fn pixel_block(shape: &Shape) -> f64 {
    (shape.width * 3.0).clamp(6.0, 80.0)
}

/// Vẽ toàn bộ chú thích. `base` cần cho công cụ pixel hoá.
pub fn draw_shapes(cr: &cairo::Context, shapes: &[Shape], base: &RgbaImage, editing: Option<usize>) {
    for (i, s) in shapes.iter().enumerate() {
        draw_shape(cr, s, base, editing == Some(i));
    }
}

pub fn draw_shape(cr: &cairo::Context, s: &Shape, base: &RgbaImage, editing: bool) {
    cr.save().ok();
    set_color(cr, &s.color);
    cr.set_line_width(s.width);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    let r = Rect::from_points(s.x1, s.y1, s.x2, s.y2);
    match &s.kind {
        ShapeKind::Rect => {
            cr.rectangle(r.x, r.y, r.w, r.h);
            if s.filled {
                cr.fill().ok();
            } else {
                cr.stroke().ok();
            }
        }
        ShapeKind::Ellipse => {
            if r.w > 0.5 && r.h > 0.5 {
                cr.save().ok();
                cr.translate(r.x + r.w / 2.0, r.y + r.h / 2.0);
                cr.scale(r.w / 2.0, r.h / 2.0);
                cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
                cr.restore().ok();
                if s.filled {
                    cr.fill().ok();
                } else {
                    cr.stroke().ok();
                }
            }
        }
        ShapeKind::Line => {
            cr.move_to(s.x1, s.y1);
            cr.line_to(s.x2, s.y2);
            cr.stroke().ok();
        }
        ShapeKind::Arrow => {
            let dx = s.x2 - s.x1;
            let dy = s.y2 - s.y1;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.5 {
                let head = (s.width * 4.0).max(12.0).min(len);
                let ang = dy.atan2(dx);
                let spread = 0.5;
                // thân mũi tên ngắn lại một chút để đầu nhọn không bị nét tròn lấn
                let bx = s.x2 - head * 0.6 * ang.cos();
                let by = s.y2 - head * 0.6 * ang.sin();
                cr.move_to(s.x1, s.y1);
                cr.line_to(bx, by);
                cr.stroke().ok();
                cr.move_to(s.x2, s.y2);
                cr.line_to(
                    s.x2 - head * (ang - spread).cos(),
                    s.y2 - head * (ang - spread).sin(),
                );
                cr.line_to(
                    s.x2 - head * (ang + spread).cos(),
                    s.y2 - head * (ang + spread).sin(),
                );
                cr.close_path();
                cr.fill().ok();
            }
        }
        ShapeKind::Pen(pts) => {
            stroke_path(cr, (s.x1, s.y1), pts);
        }
        ShapeKind::Marker(pts) => {
            cr.set_line_cap(cairo::LineCap::Square);
            cr.set_line_join(cairo::LineJoin::Miter);
            set_color(cr, &s.color.with_alpha(0.4));
            cr.set_line_width(s.width * 4.0);
            stroke_path(cr, (s.x1, s.y1), pts);
        }
        ShapeKind::Text(t) => {
            let size = text_size(s);
            let layout = pangocairo::functions::create_layout(cr);
            layout.set_font_description(Some(&text_font(size)));
            layout.set_text(t);
            let (_, logical) = layout.pixel_extents();
            // viền mờ để chữ nổi trên mọi nền
            cr.move_to(s.x1, s.y1);
            pangocairo::functions::layout_path(cr, &layout);
            let outline = s.color.contrast().with_alpha(0.55);
            set_color(cr, &outline);
            cr.set_line_width((size / 9.0).max(1.5));
            cr.stroke_preserve().ok();
            set_color(cr, &s.color);
            cr.fill().ok();
            if editing {
                let cx = s.x1 + logical.width() as f64 + 2.0;
                set_color(cr, &s.color);
                cr.set_line_width(2.0);
                cr.move_to(cx, s.y1);
                cr.line_to(cx, s.y1 + size * 1.25);
                cr.stroke().ok();
            }
        }
        ShapeKind::Counter(n) => {
            let rad = (s.width * 3.0).max(12.0);
            cr.arc(s.x1, s.y1, rad, 0.0, std::f64::consts::TAU);
            cr.fill().ok();
            let layout = pangocairo::functions::create_layout(cr);
            layout.set_font_description(Some(&text_font(rad * 1.2)));
            layout.set_text(&n.to_string());
            let (_, logical) = layout.pixel_extents();
            set_color(cr, &s.color.contrast());
            cr.move_to(
                s.x1 - logical.width() as f64 / 2.0,
                s.y1 - logical.height() as f64 / 2.0,
            );
            pangocairo::functions::show_layout(cr, &layout);
        }
        ShapeKind::Pixelate => {
            draw_pixelate(cr, &r, pixel_block(s), base);
        }
    }
    cr.restore().ok();
}

fn stroke_path(cr: &cairo::Context, start: (f64, f64), pts: &[(f64, f64)]) {
    cr.move_to(start.0, start.1);
    if pts.is_empty() {
        cr.line_to(start.0 + 0.01, start.1);
    }
    for &(x, y) in pts {
        cr.line_to(x, y);
    }
    cr.stroke().ok();
}

fn draw_pixelate(cr: &cairo::Context, r: &Rect, block: f64, base: &RgbaImage) {
    let bounds = Rect::new(0.0, 0.0, base.width() as f64, base.height() as f64);
    let Some(r) = r.rounded().intersect(&bounds) else { return };
    let r = r.rounded();
    let (x0, y0, w, h) = (r.x as u32, r.y as u32, r.w as u32, r.h as u32);
    if w == 0 || h == 0 {
        return;
    }
    let b = block.round().max(2.0) as u32;
    let bw = (w + b - 1) / b;
    let bh = (h + b - 1) / b;
    let stride = cairo::Format::ARgb32.stride_for_width(bw).unwrap();
    let mut data = vec![0u8; (stride as u32 * bh) as usize];
    for by in 0..bh {
        for bx in 0..bw {
            let (mut sr, mut sg, mut sb, mut n) = (0u64, 0u64, 0u64, 0u64);
            let ye = ((by + 1) * b).min(h);
            let xe = ((bx + 1) * b).min(w);
            for y in by * b..ye {
                for x in bx * b..xe {
                    let p = base.get_pixel(x0 + x, y0 + y);
                    sr += p[0] as u64;
                    sg += p[1] as u64;
                    sb += p[2] as u64;
                    n += 1;
                }
            }
            if n == 0 {
                continue;
            }
            let i = (by * stride as u32 + bx * 4) as usize;
            data[i] = (sb / n) as u8;
            data[i + 1] = (sg / n) as u8;
            data[i + 2] = (sr / n) as u8;
            data[i + 3] = 255;
        }
    }
    let surf = cairo::ImageSurface::create_for_data(
        data,
        cairo::Format::ARgb32,
        bw as i32,
        bh as i32,
        stride,
    )
    .expect("surface pixelate");
    cr.save().ok();
    cr.rectangle(r.x, r.y, r.w, r.h);
    cr.clip();
    cr.translate(r.x, r.y);
    cr.scale(b as f64, b as f64);
    cr.set_source_surface(&surf, 0.0, 0.0).ok();
    cr.source().set_filter(cairo::Filter::Nearest);
    cr.paint().ok();
    cr.restore().ok();
}

/// Kết xuất ảnh cuối cùng: vùng chọn + chú thích, độ phân giải gốc.
pub fn render_final(
    base: &RgbaImage,
    base_surf: &cairo::ImageSurface,
    sel: &Rect,
    shapes: &[Shape],
) -> RgbaImage {
    let sel = sel.rounded();
    let mut surf = cairo::ImageSurface::create(cairo::Format::ARgb32, sel.w as i32, sel.h as i32)
        .expect("surface kết quả");
    {
        let cr = cairo::Context::new(&surf).expect("cairo context");
        cr.translate(-sel.x, -sel.y);
        cr.set_source_surface(base_surf, 0.0, 0.0).ok();
        cr.paint().ok();
        draw_shapes(&cr, shapes, base, None);
    }
    surface_to_image(&mut surf)
}
