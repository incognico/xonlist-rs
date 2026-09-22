use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};

use crate::activity::ActivityRow;
use crate::assets::Assets;

const CELL_W: u32 = 50;
const CELL_H: u32 = 30;
const LEFT: u32 = 50;
const RIGHT: u32 = 550;
const TOP: u32 = 25;
const BOTTOM: u32 = 25;
const FONT_SIZE: f32 = 10.0;
const R_MAX: f32 = 51.0;
const G_MAX: f32 = 196.0;
const B_MAX: f32 = 240.0;
const MAX_SERVERS: usize = 50;

struct Canvas {
    width: u32,
    height: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            px: vec![255u8; (width as usize) * (height as usize) * 3],
        }
    }

    fn blend(&mut self, x: i32, y: i32, color: [u8; 3], coverage: f32) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as u32, y as u32);
        if x >= self.width || y >= self.height {
            return;
        }
        let i = ((y * self.width + x) * 3) as usize;
        let a = coverage.clamp(0.0, 1.0);
        for k in 0..3 {
            let dst = self.px[i + k] as f32;
            let src = color[k] as f32;
            self.px[i + k] = (dst * (1.0 - a) + src * a).round() as u8;
        }
    }

    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: [u8; 3]) {
        let x1 = x.min(self.width);
        let y1 = y.min(self.height);
        let x2 = x.saturating_add(w).min(self.width);
        let y2 = y.saturating_add(h).min(self.height);
        for yy in y1..y2 {
            for xx in x1..x2 {
                let i = ((yy * self.width + xx) * 3) as usize;
                self.px[i..i + 3].copy_from_slice(&color);
            }
        }
    }

    fn stroke_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: [u8; 3]) {
        if w == 0 || h == 0 {
            return;
        }
        self.fill_rect(x, y, w, 1, color);
        if h > 1 {
            self.fill_rect(x, y + h - 1, w, 1, color);
        }
        self.fill_rect(x, y, 1, h, color);
        if w > 1 {
            self.fill_rect(x + w - 1, y, 1, h, color);
        }
    }

    fn text(
        &mut self,
        x: i32,
        y: i32,
        scale: PxScale,
        font: &FontRef<'_>,
        text: &str,
        color: [u8; 3],
    ) {
        let scaled = font.as_scaled(scale);
        let mut cursor = 0.0f32;
        let mut prev = None;
        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(prev) = prev {
                cursor += scaled.kern(prev, id);
            }
            let glyph = id.with_scale_and_position(scale, point(cursor, scaled.ascent()));
            cursor += scaled.h_advance(id);
            prev = Some(id);
            let Some(outlined) = scaled.outline_glyph(glyph) else {
                continue;
            };
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = x + bounds.min.x.round() as i32 + gx as i32;
                let py = y + bounds.min.y.round() as i32 + gy as i32;
                self.blend(px, py, color, coverage);
            });
        }
    }

    fn encode_png(&self) -> anyhow::Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut encoder = png::Encoder::new(&mut buf, self.width, self.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&self.px)?;
        drop(writer);
        Ok(buf)
    }
}

pub fn render(mut rows: Vec<ActivityRow>) -> anyhow::Result<Vec<u8>> {
    rows.sort_by(|a, b| b.total().cmp(&a.total()).then(a.name.cmp(&b.name)));
    if rows.len() > MAX_SERVERS {
        rows.truncate(MAX_SERVERS);
    }

    let font_file = Assets::get("fonts/xolonium-regular.ttf")
        .ok_or_else(|| anyhow::anyhow!("embedded font missing"))?;
    let font = FontRef::try_from_slice(&font_file.data).map_err(|e| anyhow::anyhow!("{e}"))?;
    let scale = PxScale::from(FONT_SIZE);
    let black = [0, 0, 0];
    let gray = [200, 200, 200];

    let n = rows.len() as u32;
    let width = 24 * CELL_W + LEFT + RIGHT;
    let height = n * CELL_H + TOP + BOTTOM;
    let mut img = Canvas::new(width, height);

    let max_val = rows.iter().flat_map(|r| r.hours).max().unwrap_or(0).max(0);

    for (i, row) in rows.iter().enumerate() {
        let x = LEFT + 24 * CELL_W + 10;
        let y = TOP + i as u32 * CELL_H + 8;
        let label = format!("#{}: {}", i + 1, row.name);
        img.text(x as i32, y as i32, scale, &font, &label, black);
    }

    for hour in 0..24u32 {
        let x = LEFT + hour * CELL_W + 5;
        let y = height - BOTTOM + 6;
        let label = if hour == 23 {
            format!(
                "{hour:02}:00                              All times are UTC / press F5 to refresh"
            )
        } else {
            format!("{hour:02}:00")
        };
        img.text(x as i32, y as i32, scale, &font, &label, black);
    }

    for (i, row) in rows.iter().enumerate() {
        for hour in 0..24 {
            let value = row.hours[hour];
            let ratio = if max_val > 0 {
                value as f32 / max_val as f32
            } else {
                0.0
            };
            let color = [
                (255.0 - (255.0 - R_MAX) * ratio) as u8,
                (255.0 - (255.0 - G_MAX) * ratio) as u8,
                (255.0 - (255.0 - B_MAX) * ratio) as u8,
            ];
            let x = LEFT + hour as u32 * CELL_W;
            let y = TOP + i as u32 * CELL_H;
            img.fill_rect(x, y, CELL_W, CELL_H, color);
            img.stroke_rect(x, y, CELL_W, CELL_H, gray);
        }
    }

    img.encode_png()
}
