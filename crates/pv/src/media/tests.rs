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
    let lines =
        image(&f, &Theme::default(), &Options { width: Some(21), height: Some(8), git: false, focus: None }).unwrap();
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
