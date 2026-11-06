//! Drawing: the panes, the lists, the status line.

use super::*;

impl App {
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        let [left, right] = ui::layout::main_preview(&theme, body);
        // The focused pane takes the room; the other shows a few rows.
        let constraints = ORDER.map(|f| if f == self.focus { Constraint::Min(7) } else { Constraint::Length(6) });
        let panes = Layout::vertical(constraints).split(left);
        self.render_wifi(panes[0], buf, &theme);
        self.render_bluetooth(panes[1], buf, &theme);
        // The pane without the focus is dimmed a little.
        for (f, pane) in ORDER.iter().zip(panes.iter()) {
            if *f != self.focus {
                ui::dim::dim_to(buf, *pane, &theme, ui::dim::KEEP_INACTIVE);
            }
        }
        match self.focus {
            Focus::Wifi => self.details_wifi(right, buf, &theme),
            Focus::Bluetooth => self.details_bluetooth(right, buf, &theme),
        }
        self.render_status(status, buf, &theme);
        if let Some(menu) = self.maps[Self::pane(self.focus)].menu() {
            ui::keys::render_menu(area, buf, &theme, &menu);
        }
        if self.help.is_some() {
            let rows = self.keys_rows();
            if let Some(h) = self.help.as_mut() {
                h.render(area, buf, &theme, "Keys", &rows);
            }
        }
    }

    /// A pane with the list `rows` drawn in it: each entry is `(left, right, highlighted tint)`.
    pub(super) fn render_list(&mut self, focus: Focus, title: &str, area: Rect, buf: &mut Buffer, theme: &Theme, note: Option<(String, bool)>, rows: Vec<(Line<'static>, String, bool)>) {
        let c = &theme.colors;
        let focused = self.focus == focus;
        let mut pane = Pane::new(theme).title(title).focused(focused);
        if let Some(icon) = ui::icons::for_title(title) {
            pane = pane.icon(icon);
        }
        let inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 8 {
            return;
        }
        if let Some((text, warn)) = note {
            // A reason can be long: wrap it inside the pane.
            Paragraph::new(text).wrap(ratatui::widgets::Wrap { trim: true }).style(Style::new().fg(if warn { c.warning } else { c.muted }.into())).render(inner, buf);
            return;
        }
        let page = usize::from(inner.height);
        let st = &mut self.lists[Self::pane(focus)];
        if st.cursor < st.offset {
            st.offset = st.cursor;
        } else if st.cursor >= st.offset + page {
            st.offset = st.cursor + 1 - page;
        }
        let (cursor, offset) = (st.cursor, st.offset);
        let geo = ui::list::Rows::new(area, inner);
        for (row, i) in (offset..(offset + page).min(rows.len())).enumerate() {
            let (left, right, active) = &rows[i];
            let y = inner.y + row as u16;
            ui::list::ListRow { left: left.spans.clone(), right: vec![Span::raw(right.clone())], min_left: 12, tint: active.then(|| c.success.into()), selected: i == cursor, focused, ..Default::default() }.draw(theme, buf, &geo, y);
        }
    }

    pub(super) fn render_wifi(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let note = if let Some(e) = &self.wifi_error {
            Some((format!("unavailable: {e}"), false))
        } else if self.wifi_status.device.is_none() {
            Some(("no Wi-Fi device on this machine".to_string(), false))
        } else if !self.wifi_status.enabled {
            Some(("Wi-Fi is off (space p turns it on)".to_string(), true))
        } else if self.networks.is_empty() {
            Some(("no networks in range (s scans)".to_string(), false))
        } else {
            None
        };
        let rows: Vec<(Line<'static>, String, bool)> = self
            .networks
            .iter()
            .map(|n| {
                let name_style = if n.active { Style::new().fg(c.success.into()).add_modifier(Modifier::BOLD) } else { Style::new().fg(c.fg.into()) };
                let left = Line::from(vec![Span::styled(format!("{} ", bars(n.strength)), Style::new().fg(strength_color(theme, n.strength))), Span::styled(n.ssid.clone(), name_style)]);
                let mut tags = String::new();
                if n.known {
                    tags.push_str("saved ");
                }
                tags.push_str(n.security.label());
                if n.active {
                    tags.push_str(" ●");
                }
                (left, tags, n.active)
            })
            .collect();
        self.render_list(Focus::Wifi, "Wi-Fi", area, buf, theme, note, rows);
    }

    pub(super) fn render_bluetooth(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let note = if let Some(e) = &self.bt_error {
            Some((format!("unavailable: {e}"), false))
        } else if self.bt_status.adapter.is_none() {
            Some(("no Bluetooth adapter on this machine".to_string(), false))
        } else if !self.bt_status.powered {
            Some(("Bluetooth is off (space p turns it on)".to_string(), true))
        } else if self.devices.is_empty() {
            Some(("no devices (s looks for some)".to_string(), false))
        } else {
            None
        };
        let rows: Vec<(Line<'static>, String, bool)> = self
            .devices
            .iter()
            .map(|d| {
                let name_style = if d.connected { Style::new().fg(c.success.into()).add_modifier(Modifier::BOLD) } else if d.paired { Style::new().fg(c.fg.into()) } else { Style::new().fg(c.muted.into()) };
                let signal = d.rssi.map(rssi_percent);
                let left = Line::from(vec![
                    match signal {
                        Some(s) => Span::styled(format!("{} ", bars(s)), Style::new().fg(strength_color(theme, s))),
                        None => Span::styled("     ", Style::new()),
                    },
                    Span::styled(d.name.clone(), name_style),
                ]);
                let mut tags = String::from(d.kind.label());
                if d.paired {
                    tags.push_str(" paired");
                }
                if d.connected {
                    tags.push_str(" ●");
                }
                (left, tags, d.connected)
            })
            .collect();
        let title = if self.bt_status.discovering { "Bluetooth (scanning)" } else { "Bluetooth" };
        // The title keeps its icon by name; the scanning note is appended to the title text only.
        self.render_list(Focus::Bluetooth, title, area, buf, theme, note, rows);
    }

    pub(super) fn details_wifi(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let pane = Pane::new(theme).title(self.network().map(|n| n.ssid.as_str()).unwrap_or("Details")).icon(ui::icons::WIFI);
        let inner = pane.inner(area);
        pane.render(area, buf);
        let Some(n) = self.network() else { return };
        let label = |t: &str| Span::styled(format!("{t:<10}"), Style::new().fg(c.muted.into()));
        let val = |t: String| Span::styled(t, Style::new().fg(c.fg.into()));
        let band = if n.frequency_mhz >= 5900 { "6 GHz" } else if n.frequency_mhz >= 4900 { "5 GHz" } else { "2.4 GHz" };
        let lines = vec![
            Line::from(vec![label("Signal"), Span::styled(format!("{} ", bars(n.strength)), Style::new().fg(strength_color(theme, n.strength))), val(format!("{}%", n.strength))]),
            Line::from(vec![label("Security"), val(n.security.label().to_string())]),
            Line::from(vec![label("Band"), val(format!("{band} ({} MHz)", n.frequency_mhz))]),
            Line::from(vec![label("Access pt"), val(n.bssid.clone())]),
            Line::from(vec![label("Saved"), val(yes(n.known))]),
            Line::from(vec![label("Connected"), val(yes(n.active))]),
        ];
        Paragraph::new(lines).render(inner, buf);
    }

    pub(super) fn details_bluetooth(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let pane = Pane::new(theme).title(self.device().map(|d| d.name.as_str()).unwrap_or("Details")).icon(ui::icons::BLUETOOTH);
        let inner = pane.inner(area);
        pane.render(area, buf);
        let Some(d) = self.device() else { return };
        let label = |t: &str| Span::styled(format!("{t:<10}"), Style::new().fg(c.muted.into()));
        let val = |t: String| Span::styled(t, Style::new().fg(c.fg.into()));
        let signal = match d.rssi {
            Some(r) => {
                let s = rssi_percent(r);
                Line::from(vec![label("Signal"), Span::styled(format!("{} ", bars(s)), Style::new().fg(strength_color(theme, s))), val(format!("{r} dBm"))])
            }
            None => Line::from(vec![label("Signal"), Span::styled("not in range", Style::new().fg(c.muted.into()))]),
        };
        let lines = vec![
            Line::from(vec![label("Type"), val(d.kind.label().to_string())]),
            Line::from(vec![label("Address"), val(d.address.clone())]),
            signal,
            Line::from(vec![label("Paired"), val(yes(d.paired))]),
            Line::from(vec![label("Trusted"), val(yes(d.trusted))]),
            Line::from(vec![label("Connected"), val(yes(d.connected))]),
        ];
        Paragraph::new(lines).render(inner, buf);
    }

    pub(super) fn render_status(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let line = match &self.mode {
            Mode::Password(ssid, input) => {
                let label = format!("password for {ssid}: ");
                ui::prompt::cursor(buf, area, &label, input.visual_cursor());
                ui::prompt::line(theme, &label, &"•".repeat(input.value().chars().count()))
            }
            Mode::ForgetWifi(ssid) => Line::from(Span::styled(format!("Forget {ssid}? (d again or y)"), Style::new().fg(c.warning.into()))),
            Mode::ForgetDevice(_, name) => Line::from(Span::styled(format!("Forget {name}? (d again or y)"), Style::new().fg(c.warning.into()))),
            Mode::Normal if self.job.is_some() => Line::from(Span::styled(format!("{} …", self.job.as_ref().map(|j| j.label.as_str()).unwrap_or("")), Style::new().fg(c.accent.into()))),
            Mode::Normal => match &self.message {
                Some((m, err)) => Line::from(Span::styled(m.clone(), Style::new().fg(if *err { c.error } else { c.accent }.into()))),
                None => Line::from(Span::styled(self.hints(), Style::new().fg(c.muted.into()))),
            },
        };
        Paragraph::new(line).render(Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area }, buf);
    }

    pub(super) fn hints(&self) -> &'static str {
        match self.focus {
            Focus::Wifi => "enter connect  x disconnect  d forget  s scan  space p on/off  tab pane  space space commands  ? keys",
            Focus::Bluetooth => "enter connect  x disconnect  t trust  d forget  s scan  space p on/off  tab pane  space space commands  ? keys",
        }
    }
}

pub(super) fn yes(b: bool) -> String {
    if b { "yes" } else { "no" }.to_string()
}

/// Four-step signal bars: `▂▄▆█` filled by strength.
pub(super) fn bars(strength: u8) -> String {
    let filled = match strength {
        0..=19 => 1,
        20..=44 => 2,
        45..=69 => 3,
        _ => 4,
    };
    ["▂", "▄", "▆", "█"].iter().enumerate().map(|(i, b)| if i < filled { *b } else { " " }).collect()
}

pub(super) fn strength_color(theme: &Theme, strength: u8) -> ratatui::style::Color {
    let c = &theme.colors;
    match strength {
        0..=29 => c.error,
        30..=54 => c.warning,
        _ => c.success,
    }
    .into()
}
