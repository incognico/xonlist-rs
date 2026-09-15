use ab_glyph::{FontRef, PxScale};
use image::{Rgb, RgbImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect;

use crate::activity::ActivityRow;

const FONT: &[u8] = include_bytes!("../static/fonts/xolonium-regular.ttf");

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

pub fn render(mut rows: Vec<ActivityRow>) -> anyhow::Result<Vec<u8>> {
    rows.sort_by(|a, b| b.total().cmp(&a.total()).then(a.name.cmp(&b.name)));
    if rows.len() > MAX_SERVERS {
        rows.truncate(MAX_SERVERS);
    }

    let font = FontRef::try_from_slice(FONT).map_err(|e| anyhow::anyhow!("{e}"))?;
    let scale = PxScale::from(FONT_SIZE);

    let n = rows.len() as u32;
    let width = 24 * CELL_W + LEFT + RIGHT;
    let height = n * CELL_H + TOP + BOTTOM;
    let mut img = RgbImage::from_pixel(width, height, Rgb([255, 255, 255]));
    let black = Rgb([0, 0, 0]);
    let gray = Rgb([200, 200, 200]);

    let max_val = rows.iter().flat_map(|r| r.hours).max().unwrap_or(0).max(0);

    for (i, row) in rows.iter().enumerate() {
        let x = LEFT + 24 * CELL_W + 10;
        let y = TOP + i as u32 * CELL_H + 8;
        let label = format!("#{}: {}", i + 1, row.name);
        draw_text_mut(&mut img, black, x as i32, y as i32, scale, &font, &label);
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
        draw_text_mut(&mut img, black, x as i32, y as i32, scale, &font, &label);
    }

    for (i, row) in rows.iter().enumerate() {
        for hour in 0..24 {
            let value = row.hours[hour];
            let ratio = if max_val > 0 {
                value as f32 / max_val as f32
            } else {
                0.0
            };
            let r = (255.0 - (255.0 - R_MAX) * ratio) as u8;
            let g = (255.0 - (255.0 - G_MAX) * ratio) as u8;
            let b = (255.0 - (255.0 - B_MAX) * ratio) as u8;
            let x = LEFT + hour as u32 * CELL_W;
            let y = TOP + i as u32 * CELL_H;
            let rect = Rect::at(x as i32, y as i32).of_size(CELL_W, CELL_H);
            draw_filled_rect_mut(&mut img, rect, Rgb([r, g, b]));
            draw_hollow_rect_mut(&mut img, rect, gray);
        }
    }

    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    use image::ImageEncoder;
    encoder.write_image(
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(buf)
}
