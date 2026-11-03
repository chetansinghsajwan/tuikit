//! Images, audio and video. An image is drawn with half blocks (`▀`: the upper pixel is the
//! foreground, the lower one the background), which any truecolor terminal shows, in a multiplexer
//! too. Audio and video show what the file says about itself (tags, duration, codecs), not the content.

use crate::Error;
use crate::Options;
use image::GenericImageView;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::path::Path;
use std::time::Duration;
use ui::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Image,
    Audio,
    Video,
}

/// What kind of media the file is by its extension.
pub fn kind(path: &Path) -> Option<Kind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => Some(Kind::Image),
        "mp3" | "flac" | "ogg" | "oga" | "opus" | "wav" | "m4a" | "aac" | "aiff" | "aif" | "ape" | "wv" => Some(Kind::Audio),
        "mp4" | "m4v" | "mov" | "mkv" | "webm" | "avi" | "wmv" | "flv" | "mpg" | "mpeg" => Some(Kind::Video),
        _ => None,
    }
}

pub fn render(path: &Path, kind: Kind, theme: &Theme, opts: &Options) -> Result<Vec<Line<'static>>, Error> {
    match kind {
        Kind::Image => image(path, theme, opts),
        Kind::Audio => audio(path, theme),
        Kind::Video => video(path, theme),
    }
}

fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let (mut v, mut u) = (bytes as f64, 0);
    while v >= 1024.0 && u < 3 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 { format!("{bytes} B") } else { format!("{v:.1} {}", UNITS[u]) }
}

/// `1:02:03` or `4:05`.
pub fn duration_text(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// `label  value` rows, the labels aligned.
fn facts(theme: &Theme, rows: &[(&str, String)]) -> Vec<Line<'static>> {
    let c = &theme.colors;
    let w = rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    rows.iter().map(|(l, v)| Line::from(vec![Span::styled(format!("{l:<w$}  "), Style::new().fg(c.muted.into())), Span::styled(v.clone(), Style::new().fg(c.fg.into()))])).collect()
}

fn unreadable(what: &str, e: impl std::fmt::Display) -> Error {
    Error::Unsupported(format!("{what}: {e}"))
}

fn image(path: &Path, theme: &Theme, opts: &Options) -> Result<Vec<Line<'static>>, Error> {
    let img = image::ImageReader::open(path)?.with_guessed_format()?.decode().map_err(|e| unreadable("image", e))?;
    let (w, h) = img.dimensions();
    let bytes = std::fs::metadata(path)?.len();
    let mut out = facts(theme, &[("image", format!("{w}×{h}  {}", size_text(bytes)))]);
    out.push(Line::default());
    let cols = u32::from(opts.width.unwrap_or(80)).saturating_sub(1).max(1);
    let rows = opts.height.map(|h| u32::from(h).saturating_sub(out.len() as u32)).unwrap_or(30).max(1);
    out.extend(picture(&img, theme, cols, rows));
    Ok(out)
}

/// The image file as half-block rows, at most `cols` wide and `rows` tall.
pub fn image_rows(path: &Path, theme: &Theme, cols: u16, rows: u16) -> Result<Vec<Line<'static>>, Error> {
    let img = image::ImageReader::open(path)?.with_guessed_format()?.decode().map_err(|e| unreadable("image", e))?;
    Ok(picture(&img, theme, u32::from(cols).max(1), u32::from(rows).max(1)))
}

/// A cell is one pixel wide and two tall.
fn picture(img: &image::DynamicImage, theme: &Theme, cols: u32, rows: u32) -> Vec<Line<'static>> {
    let c = &theme.colors;
    let mut out = Vec::new();
    let small = img.resize(cols, rows * 2, image::imageops::FilterType::Triangle).to_rgba8();
    let bg: Color = c.bg.into();
    let (br, bgc, bb) = match bg {
        Color::Rgb(r, g, b) => (u32::from(r), u32::from(g), u32::from(b)),
        _ => (0, 0, 0),
    };
    // Transparent pixels take the background color.
    let at = |x: u32, y: u32| -> Color {
        if y >= small.height() {
            return bg;
        }
        let p = small.get_pixel(x, y).0;
        let a = u32::from(p[3]);
        let mix = |v: u8, b: u32| ((u32::from(v) * a + b * (255 - a)) / 255) as u8;
        Color::Rgb(mix(p[0], br), mix(p[1], bgc), mix(p[2], bb))
    };
    for y in (0..small.height()).step_by(2) {
        let spans: Vec<Span<'static>> = (0..small.width()).map(|x| Span::styled("▀", Style::new().fg(at(x, y)).bg(at(x, y + 1)))).collect();
        out.push(Line::from(spans));
    }
    out
}

fn audio(path: &Path, theme: &Theme) -> Result<Vec<Line<'static>>, Error> {
    use lofty::prelude::*;
    let bytes = std::fs::metadata(path)?.len();
    let file = lofty::read_from_path(path).map_err(|e| unreadable("audio", e))?;
    let mut rows: Vec<(&str, String)> = Vec::new();
    if let Some(t) = file.primary_tag().or_else(|| file.first_tag()) {
        let mut add = |label: &'static str, v: Option<String>| {
            if let Some(v) = v.filter(|v| !v.is_empty()) {
                rows.push((label, v));
            }
        };
        add("title", t.title().map(|s| s.into_owned()));
        add("artist", t.artist().map(|s| s.into_owned()));
        add("album", t.album().map(|s| s.into_owned()));
        add("year", t.get_string(&lofty::tag::ItemKey::RecordingDate).or_else(|| t.get_string(&lofty::tag::ItemKey::Year)).map(|s| s.chars().take(4).collect()));
        add("track", t.track().map(|n| n.to_string()));
        add("genre", t.genre().map(|s| s.into_owned()));
    }
    let p = file.properties();
    rows.push(("length", duration_text(p.duration())));
    if let Some(b) = p.audio_bitrate() {
        rows.push(("bitrate", format!("{b} kbps")));
    }
    if let Some(r) = p.sample_rate() {
        rows.push(("sample rate", format!("{r} Hz")));
    }
    if let Some(n) = p.channels() {
        rows.push(("channels", n.to_string()));
    }
    rows.push(("format", format!("{:?}", file.file_type())));
    rows.push(("size", size_text(bytes)));
    Ok(facts(theme, &rows))
}

fn video(path: &Path, theme: &Theme) -> Result<Vec<Line<'static>>, Error> {
    let bytes = std::fs::metadata(path)?.len();
    let mut rows: Vec<(&str, String)> = Vec::new();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if matches!(ext.as_str(), "mp4" | "m4v" | "mov") {
        let file = std::fs::File::open(path)?;
        let mp4 = mp4::Mp4Reader::read_header(std::io::BufReader::new(file), bytes).map_err(|e| unreadable("video", e))?;
        rows.push(("length", duration_text(mp4.duration())));
        let mut ids: Vec<_> = mp4.tracks().keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let t = &mp4.tracks()[&id];
            match t.track_type() {
                Ok(mp4::TrackType::Video) => {
                    let codec = t.media_type().map(|m| m.to_string()).unwrap_or_else(|_| "?".into());
                    rows.push(("video", format!("{codec}  {}×{}  {:.0} fps", t.width(), t.height(), t.frame_rate())));
                }
                Ok(mp4::TrackType::Audio) => {
                    let codec = t.media_type().map(|m| m.to_string()).unwrap_or_else(|_| "?".into());
                    rows.push(("audio", format!("{codec}  {} kbps", t.bitrate() / 1000)));
                }
                _ => {}
            }
        }
    } else {
        rows.push(("format", ext.to_uppercase()));
    }
    rows.push(("size", size_text(bytes)));
    let mut out = facts(theme, &rows);
    if !matches!(ext.as_str(), "mp4" | "m4v" | "mov") {
        out.push(Line::default());
        out.push(Line::from(Span::styled("details are only read from mp4, m4v and mov files", Style::new().fg(theme.colors.muted.into()).add_modifier(Modifier::ITALIC))));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn kinds_by_extension() {
        assert_eq!(kind(Path::new("a.PNG")), Some(Kind::Image));
        assert_eq!(kind(Path::new("a.flac")), Some(Kind::Audio));
        assert_eq!(kind(Path::new("a.mkv")), Some(Kind::Video));
        assert_eq!(kind(Path::new("a.rs")), None);
        assert_eq!(kind(Path::new("noext")), None);
    }

    #[test]
    fn durations_and_sizes() {
        assert_eq!(duration_text(Duration::from_secs(65)), "1:05");
        assert_eq!(duration_text(Duration::from_secs(3725)), "1:02:05");
        assert_eq!(size_text(512), "512 B");
        assert_eq!(size_text(3 * 1024 * 1024), "3.0 MiB");
    }

    #[test]
    fn an_image_is_drawn_in_half_blocks_that_fit() {
        let d = std::env::temp_dir().join(format!("pv-media-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("p.png");
        let mut img = image::RgbaImage::new(40, 20);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = if x < 20 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) };
        }
        img.save(&f).unwrap();
        let lines = image(&f, &Theme::default(), &Options { width: Some(21), height: Some(8), git: false, focus: None }).unwrap();
        assert!(flat(&lines[0]).contains("40×20"));
        assert!(lines.len() <= 8, "{}", lines.len());
        let row = lines.last().unwrap();
        assert!(row.spans.len() <= 20 && row.spans[0].content == "▀");
        assert_eq!(row.spans[0].style.fg, Some(Color::Rgb(255, 0, 0)));
        assert_eq!(row.spans.last().unwrap().style.fg, Some(Color::Rgb(0, 0, 255)));
    }

    #[test]
    fn a_broken_audio_file_is_unsupported_not_a_panic() {
        let d = std::env::temp_dir().join(format!("pv-media-a-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("x.mp3");
        std::fs::write(&f, b"not audio").unwrap();
        assert!(matches!(render(&f, Kind::Audio, &Theme::default(), &Options::default()), Err(Error::Unsupported(_))));
    }
}
