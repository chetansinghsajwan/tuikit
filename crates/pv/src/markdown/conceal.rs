use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// A piece of Markdown syntax that a live view hides (`replace` None) or shows as `replace`, until
/// the cursor is in `element` (the heading, emphasis, link or item the syntax belongs to).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conceal {
    pub range: std::ops::Range<usize>,
    pub element: std::ops::Range<usize>,
    pub replace: Option<&'static str>,
}

/// The syntax to hide in `text` (byte ranges): heading marks, emphasis, strikethrough and code
/// marks, link brackets and targets, and list bullets (shown as `•`).
pub fn conceal(text: &str) -> Vec<Conceal> {
    let mut out = Vec::new();
    let mut hide = |range: std::ops::Range<usize>, element: &std::ops::Range<usize>, replace| {
        if range.start < range.end {
            out.push(Conceal { range, element: element.clone(), replace });
        }
    };
    // The extent of what is inside each open link.
    let mut links: Vec<Option<std::ops::Range<usize>>> = Vec::new();
    for (event, r) in Parser::new_ext(
        text,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS,
    )
    .into_offset_iter()
    {
        let is_link_edge =
            matches!(event, Event::Start(Tag::Link { .. }) | Event::End(TagEnd::Link));
        if !is_link_edge && let Some(slot) = links.last_mut() {
            match slot {
                Some(inner) => {
                    inner.start = inner.start.min(r.start);
                    inner.end = inner.end.max(r.end);
                }
                None => *slot = Some(r.clone()),
            }
        }
        let head = &text[r.start..r.end.min(text.len())];
        match event {
            Event::Start(Tag::Heading { .. }) if head.starts_with('#') => {
                let hashes = head.len() - head.trim_start_matches('#').len();
                let spaces = head[hashes..].len() - head[hashes..].trim_start_matches(' ').len();
                let line_end = head.find('\n').map_or(r.end, |n| r.start + n);
                hide(r.start..r.start + hashes + spaces, &(r.start..line_end), None);
            }
            Event::End(TagEnd::Strong)
            | Event::End(TagEnd::Emphasis)
            | Event::End(TagEnd::Strikethrough) => {
                let n = match event {
                    Event::End(TagEnd::Strong) => 2,
                    Event::End(TagEnd::Strikethrough) if head.starts_with("~~") => 2,
                    _ => 1,
                };
                if head.len() >= 2 * n {
                    hide(r.start..r.start + n, &r, None);
                    hide(r.end - n..r.end, &r, None);
                }
            }
            Event::Code(_) => {
                let n = head.len() - head.trim_start_matches('`').len();
                if n > 0 && head.len() > 2 * n {
                    hide(r.start..r.start + n, &r, None);
                    hide(r.end - n..r.end, &r, None);
                }
            }
            Event::Start(Tag::Link { .. }) => links.push(None),
            Event::End(TagEnd::Link) => {
                if let Some(Some(inner)) = links.pop()
                    && head.starts_with('[')
                    && inner.start > r.start
                    && inner.end < r.end
                {
                    hide(r.start..inner.start, &r, None);
                    hide(inner.end..r.end, &r, None);
                }
            }
            Event::Start(Tag::Item)
                if head.starts_with(['-', '*', '+']) && head[1..].starts_with(' ') =>
            {
                let line_end = head.find('\n').map_or(r.end, |n| r.start + n);
                hide(r.start..r.start + 1, &(r.start..line_end), Some("•"));
            }
            _ => {}
        }
    }
    out.sort_by_key(|c| c.range.start);
    out
}
