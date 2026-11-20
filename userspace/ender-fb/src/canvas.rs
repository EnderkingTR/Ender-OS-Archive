//! Yazılımsal 2B Framebuffer çizim tuvali (32-bpp BGRA/RGBA, 24-bpp ve 16-bpp RGB565).

use crate::font;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelFormat {
    pub bpp: u32,
    pub r_off: u32,
    pub r_len: u32,
    pub g_off: u32,
    pub g_len: u32,
    pub b_off: u32,
    pub b_len: u32,
}

fn scale_channel(val: u8, len: u32) -> u32 {
    if len == 0 {
        0
    } else if len >= 8 {
        (val as u32) << (len - 8)
    } else {
        (val as u32) >> (8 - len)
    }
}

impl PixelFormat {
    pub fn pack_rgb(&self, r: u8, g: u8, b: u8) -> u32 {
        (scale_channel(r, self.r_len) << self.r_off)
            | (scale_channel(g, self.g_len) << self.g_off)
            | (scale_channel(b, self.b_len) << self.b_off)
    }
}

pub struct Canvas<'a> {
    buf: &'a mut [u8],
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub format: PixelFormat,
}

impl<'a> Canvas<'a> {
    pub fn new(
        buf: &'a mut [u8],
        width: u32,
        height: u32,
        stride: usize,
        format: PixelFormat,
    ) -> Self {
        Self {
            buf,
            width,
            height,
            stride,
            format,
        }
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, rgb: (u8, u8, u8)) {
        if x < 0 || y < 0 || (x as u32) >= self.width || (y as u32) >= self.height {
            return;
        }
        let bytes_pp = (self.format.bpp / 8) as usize;
        if bytes_pp == 0 {
            return;
        }
        let offset = (y as usize) * self.stride + (x as usize) * bytes_pp;
        if offset + bytes_pp > self.buf.len() {
            return;
        }

        let packed = self.format.pack_rgb(rgb.0, rgb.1, rgb.2);
        let raw = packed.to_le_bytes();
        self.buf[offset..offset + bytes_pp].copy_from_slice(&raw[..bytes_pp]);
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, rgb: (u8, u8, u8)) {
        let x_start = x.max(0);
        let y_start = y.max(0);
        let x_end = (x.saturating_add(w as i32)).min(self.width as i32);
        let y_end = (y.saturating_add(h as i32)).min(self.height as i32);

        for py in y_start..y_end {
            for px in x_start..x_end {
                self.set_pixel(px, py, rgb);
            }
        }
    }

    pub fn clear(&mut self, rgb: (u8, u8, u8)) {
        self.fill_rect(0, 0, self.width, self.height, rgb);
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: u32, h: u32, rgb: (u8, u8, u8)) {
        if w == 0 || h == 0 {
            return;
        }
        self.fill_rect(x, y, w, 1, rgb);
        self.fill_rect(x, y + (h as i32) - 1, w, 1, rgb);
        self.fill_rect(x, y, 1, h, rgb);
        self.fill_rect(x + (w as i32) - 1, y, 1, h, rgb);
    }

    pub fn draw_char(&mut self, x: i32, y: i32, scale: i32, ch: char, rgb: (u8, u8, u8)) {
        let bitmap = font::glyph(ch);
        let s = scale.max(1) as u32;
        for (row_idx, row_bits) in bitmap.iter().enumerate() {
            for col_idx in 0..8 {
                if (row_bits >> (7 - col_idx)) & 1 != 0 {
                    let px = x + col_idx * (s as i32);
                    let py = y + (row_idx as i32) * (s as i32);
                    self.fill_rect(px, py, s, s, rgb);
                }
            }
        }
    }

    pub fn draw_text(&mut self, x: i32, y: i32, scale: i32, text: &str, rgb: (u8, u8, u8)) {
        let step = 8 * scale.max(1);
        for (idx, ch) in text.chars().enumerate() {
            self.draw_char(x + (idx as i32) * step, y, scale, ch, rgb);
        }
    }

    pub fn draw_bar(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pct: u32,
        fill_rgb: (u8, u8, u8),
        bg_rgb: (u8, u8, u8),
        border_rgb: (u8, u8, u8),
    ) {
        self.fill_rect(x, y, w, h, bg_rgb);
        let clamped = pct.min(100);
        let fill_w = (w.saturating_sub(4) * clamped) / 100;
        if fill_w > 0 && h > 4 {
            self.fill_rect(x + 2, y + 2, fill_w, h - 4, fill_rgb);
        }
        self.draw_rect(x, y, w, h, border_rgb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_32bpp_bgra_and_rgba() {
        let bgra = PixelFormat {
            bpp: 32,
            r_off: 16,
            r_len: 8,
            g_off: 8,
            g_len: 8,
            b_off: 0,
            b_len: 8,
        };
        assert_eq!(bgra.pack_rgb(0x12, 0x34, 0x56), 0x0012_3456);

        let rgba = PixelFormat {
            bpp: 32,
            r_off: 0,
            r_len: 8,
            g_off: 8,
            g_len: 8,
            b_off: 16,
            b_len: 8,
        };
        assert_eq!(rgba.pack_rgb(0x12, 0x34, 0x56), 0x0056_3412);
    }

    #[test]
    fn packs_16bpp_rgb565() {
        let rgb565 = PixelFormat {
            bpp: 16,
            r_off: 11,
            r_len: 5,
            g_off: 5,
            g_len: 6,
            b_off: 0,
            b_len: 5,
        };
        // Saf kırmızı: 0b11111_000000_00000 = 0xF800
        assert_eq!(rgb565.pack_rgb(255, 0, 0), 0xF800);
        // Saf beyaz: 0xFFFF
        assert_eq!(rgb565.pack_rgb(255, 255, 255), 0xFFFF);
    }

    #[test]
    fn clips_offscreen_drawing_safely() {
        let fmt = PixelFormat {
            bpp: 32,
            r_off: 16,
            r_len: 8,
            g_off: 8,
            g_len: 8,
            b_off: 0,
            b_len: 8,
        };
        let mut buf = vec![0u8; 32 * 32 * 4];
        let mut canvas = Canvas::new(&mut buf, 32, 32, 32 * 4, fmt);
        canvas.clear((10, 20, 30));
        canvas.fill_rect(-10, -10, 60, 60, (56, 189, 248));
        canvas.draw_text(-4, 12, 2, "ENDER-OS", (255, 255, 255));
        canvas.draw_bar(2, 20, 40, 12, 75, (34, 197, 94), (9, 13, 24), (100, 116, 139));
        // Köşe pikselinin B kanalı yazılmış olmalı
        assert_ne!(buf[0], 0);
    }
}
