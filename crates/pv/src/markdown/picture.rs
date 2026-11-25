//! Images.

use super::state::Out;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// The tallest an inline image gets, in rows.
const IMAGE_ROWS: u16 = 16;
/// The widest an inline image gets, in columns.
const IMAGE_COLS: u16 = 64;

impl Out {
    /// Open an image: its target, with the alt text to follow.
    pub(super) fn start_image(&mut self, dest_url: &str) {
        self.image = Some((dest_url.to_string(), String::new()))
    }

    /// Close an image: the picture when it is a local file that fits, else its alt text and target.
    pub(super) fn end_image(&mut self, cx: &ui::Context) {
        let theme = cx.theme;
        let muted = Color::from(theme.colors.muted);
        if let Some((url, alt)) = self.image.take() {
            let local = if url.contains("://") {
                None
            } else {
                self.base.as_ref().map(|b| b.join(&url)).filter(|p| p.is_file())
            };
            let picture = local
                .filter(|p| crate::media::kind(p) == Some(crate::media::Kind::Image))
                .and_then(|p| {
                    let cols =
                        (self.width.saturating_sub(self.prefix_width() + 1) as u16).min(IMAGE_COLS);
                    crate::media::image_rows(&p, cx, cols, IMAGE_ROWS).ok()
                });
            match picture {
                Some(rows) => {
                    self.flush();
                    for row in rows {
                        self.push_line(row.spans);
                    }
                    if !alt.is_empty() {
                        self.push_line(vec![Span::styled(
                            alt,
                            Style::new().fg(muted).add_modifier(Modifier::ITALIC),
                        )]);
                    }
                }
                None => {
                    let shown = if alt.is_empty() { url.clone() } else { format!("{alt} ({url})") };
                    self.words.push((format!("🖼 {shown}"), Style::new().fg(muted)));
                }
            }
        }
    }
}
