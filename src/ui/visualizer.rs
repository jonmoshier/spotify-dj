use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

const BLOCKS: [char; 9] = [
    ' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
    '\u{2588}',
];

pub struct Visualizer<'a> {
    bands: &'a [f32],
    peaks: &'a [f32],
}

impl<'a> Visualizer<'a> {
    pub fn new(bands: &'a [f32], peaks: &'a [f32]) -> Self {
        Self { bands, peaks }
    }
}

/// Color based on position within the bar (0.0 = bottom, 1.0 = top).
fn bar_color(row_frac: f32) -> Color {
    if row_frac < 0.60 {
        Color::Green
    } else if row_frac < 0.85 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn peak_color(row_frac: f32) -> Color {
    if row_frac < 0.60 {
        Color::LightGreen
    } else if row_frac < 0.85 {
        Color::LightYellow
    } else {
        Color::LightRed
    }
}

impl Widget for Visualizer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.bands.is_empty() {
            return;
        }

        let band_count = self.bands.len();
        let peak_count = self.peaks.len();
        let total_eighths = area.height as u32 * 8;

        for x in 0..area.width {
            let band_idx =
                ((x as usize * band_count) / area.width as usize).min(band_count - 1);
            let value = self.bands[band_idx].clamp(0.0, 1.0);
            let fill_eighths = (value * total_eighths as f32).round() as u32;

            let peak_val = if peak_count > 0 {
                let peak_idx =
                    ((x as usize * peak_count) / area.width as usize).min(peak_count - 1);
                self.peaks[peak_idx].clamp(0.0, 1.0)
            } else {
                value
            };
            let peak_eighths = (peak_val * total_eighths as f32).round() as u32;
            let peak_row_from_bottom = peak_eighths / 8;

            for y in 0..area.height {
                let row_from_bottom = (area.height - 1 - y) as u32;
                let row_frac = row_from_bottom as f32 / area.height as f32;
                let row_floor = row_from_bottom * 8;
                let cell_fill = fill_eighths.saturating_sub(row_floor).min(8);

                let is_peak_marker = row_from_bottom == peak_row_from_bottom
                    && peak_eighths > fill_eighths
                    && peak_val > 0.0;

                let (ch, color) = if is_peak_marker && cell_fill == 0 {
                    ('\u{2581}', peak_color(row_frac))
                } else if cell_fill > 0 {
                    (BLOCKS[cell_fill as usize], bar_color(row_frac))
                } else {
                    (' ', Color::Reset)
                };

                let cell = &mut buf[(area.x + x, area.y + y)];
                cell.set_char(ch);
                cell.set_style(Style::default().fg(color));
            }
        }
    }
}
