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

/// Vertical FFT bar chart drawn directly into the buffer.
///
/// Each band is mapped onto a contiguous slice of columns and rendered with
/// 1/8-row block characters so short bars still get visible motion.
pub struct Visualizer<'a> {
    bands: &'a [f32],
    color: Color,
}

impl<'a> Visualizer<'a> {
    pub fn new(bands: &'a [f32], color: Color) -> Self {
        Self { bands, color }
    }
}

impl Widget for Visualizer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.bands.is_empty() {
            return;
        }

        let band_count = self.bands.len();
        let total_eighths = area.height as u32 * 8;
        let style = Style::default().fg(self.color);

        for x in 0..area.width {
            let band_idx = ((x as usize * band_count) / area.width as usize).min(band_count - 1);
            let value = self.bands[band_idx].clamp(0.0, 1.0);
            let fill_eighths = (value * total_eighths as f32).round() as u32;

            for y in 0..area.height {
                let row_from_bottom = (area.height - 1 - y) as u32;
                let row_floor = row_from_bottom * 8;
                let cell_fill = fill_eighths.saturating_sub(row_floor).min(8);
                let block = BLOCKS[cell_fill as usize];
                let cell = &mut buf[(area.x + x, area.y + y)];
                cell.set_char(block);
                cell.set_style(style);
            }
        }
    }
}
