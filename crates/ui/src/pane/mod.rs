//! `Pane`: the one place a bordered, titled, padded region is drawn.

use crate::component::{Component, Context};
use crate::icons::Icon;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Widget};

/// A bordered, titled and padded region, drawn with the theme of its `Context` so every pane looks
/// alike.
pub struct Pane<'a> {
    title: Option<&'a str>,
    icon: Option<Icon>,
    focused: bool,
}

impl<'a> Pane<'a> {
    /// A pane with no title or icon, not focused.
    pub fn new() -> Self {
        Pane { title: None, icon: None, focused: false }
    }

    /// Set the title shown in the border (only when the theme has titles on).
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// An icon shown before the title (see `ui::icons`).
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Mark the pane as focused, which gives it the focused border color and an accent title.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// The ratatui block for this pane, with the theme's border, colors, padding and title.
    fn block(&self, cx: &Context) -> Block<'a> {
        let theme = cx.theme;
        let c = &theme.colors;
        let l = &theme.layout;
        let border = if self.focused { c.border_focused } else { c.border };
        let mut block = Block::new()
            .borders(Borders::ALL)
            .border_type(theme.chrome.border_type())
            .border_style(Style::new().fg(border.into()))
            .style(if theme.chrome.transparent {
                Style::new().fg(c.fg.into())
            } else {
                Style::new().fg(c.fg.into()).bg(c.bg.into())
            })
            .padding(Padding::symmetric(l.padding_x, l.padding_y));
        if let (true, Some(t)) = (theme.chrome.titles, self.title) {
            block = block.title(self.title_line(cx, t, border.into()));
        }
        block
    }

    /// `─ icon title ─`: set off from the corner by a bar and a space on each side;
    /// bold accent when the pane is focused, muted otherwise.
    fn title_line(&self, cx: &Context, title: &str, border: ratatui::style::Color) -> Line<'a> {
        let c = &cx.theme.colors;
        let bar = cx.theme.chrome.border_type().to_border_set().horizontal_top;
        let style = if self.focused {
            Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(c.muted.into())
        };
        let mut spans = vec![Span::styled(bar, Style::new().fg(border)), Span::raw(" ")];
        if let Some(i) = self.icon {
            spans.push(Span::styled(format!("{} ", i.glyph), i.style()));
        }
        spans.push(Span::styled(title.to_string(), style));
        spans.push(Span::raw(" "));
        Line::from(spans)
    }

    /// The content rect inside the border and padding.
    pub fn inner(&self, cx: &Context, area: Rect) -> Rect {
        self.block(cx).inner(area)
    }

    /// Draws the pane into `area` and returns its content rect, inside the border and padding.
    pub fn draw(self, cx: &Context, area: Rect, buf: &mut Buffer) -> Rect {
        let inner = self.inner(cx, area);
        Component::render(self, cx, area, buf);
        inner
    }
}

impl Default for Pane<'_> {
    fn default() -> Self {
        Pane::new()
    }
}

impl Component for Pane<'_> {
    fn render(self, cx: &Context, area: Rect, buf: &mut Buffer) {
        self.block(cx).render(area, buf);
    }
}

#[cfg(test)]
mod tests;
