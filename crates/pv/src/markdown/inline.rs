//! Text and its inline styles: emphasis, links, breaks and raw HTML.

use super::state::Out;
use ratatui::style::{Color, Modifier, Style};
use ui::Theme;

impl Out {
    /// Text goes to the code block, the front matter, the image alt text or the paragraph, whichever is open.
    pub(super) fn text(&mut self, t: &str) {
        if self.in_code {
            self.code.push_str(&t);
        } else if let Some(meta) = self.meta.as_mut() {
            meta.push_str(&t);
        } else if let Some((_, alt)) = self.image.as_mut() {
            alt.push_str(&t);
        } else {
            self.push_text(&t);
        }
    }

    /// Italic text.
    pub(super) fn start_emphasis(&mut self) {
        self.styles.push(self.style().add_modifier(Modifier::ITALIC));
    }

    /// Bold text.
    pub(super) fn start_strong(&mut self) {
        self.styles.push(self.style().add_modifier(Modifier::BOLD));
    }

    /// Crossed-out text.
    pub(super) fn start_strikethrough(&mut self) {
        self.styles.push(self.style().add_modifier(Modifier::CROSSED_OUT))
    }

    /// End emphasis, strong or strikethrough.
    pub(super) fn end_style(&mut self) {
        self.styles.pop();
    }

    /// Open a link: underlined, with its target kept for the end.
    pub(super) fn start_link(&mut self, dest_url: &str, theme: &Theme) {
        let c = &theme.colors;
        self.links.push(dest_url.to_string());
        self.styles.push(self.style().fg(Color::from(c.info)).add_modifier(Modifier::UNDERLINED));
    }

    /// Close a link: the target follows the text, muted.
    pub(super) fn end_link(&mut self, theme: &Theme) {
        let muted = Color::from(theme.colors.muted);
        self.styles.pop();
        if let Some(url) = self.links.pop() {
            self.words.push((format!("({url})"), Style::new().fg(muted)));
        }
    }

    /// A soft line break.
    pub(super) fn soft_break(&mut self) {
        self.words.push((String::new(), self.style()));
    }

    /// A hard line break.
    pub(super) fn hard_break(&mut self) {
        self.flush();
    }

    /// Raw HTML: comments are dropped, other markup shows muted.
    pub(super) fn html(&mut self, h: &str, theme: &Theme) {
        let muted = Color::from(theme.colors.muted);
        // Comments are for the author; other markup shows as it is, muted.
        if !h.trim_start().starts_with("<!--") {
            for l in h.lines().filter(|l| !l.trim().is_empty()) {
                self.words.push((l.trim().to_string(), Style::new().fg(muted)));
            }
        }
    }
}
