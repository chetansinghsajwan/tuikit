use super::*;

#[test]
fn a_pdf_made_by_hand_shows_its_pages_and_text() {
    use lopdf::{Document, Object, Stream, dictionary};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
    let content = Stream::new(dictionary! {}, b"BT /F1 24 Tf 100 600 Td (Hello preview world) Tj ET".to_vec());
    let content_id = doc.add_object(content);
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => content_id, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Resources" => resources });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let d = std::env::temp_dir().join(format!("pv-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("hello.pdf");
    doc.save(&p).unwrap();
    assert!(is_pdf(&p));
    let lines = render(&p, &Theme::default(), &Options::default()).unwrap();
    let t: String = lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>() + "\n").collect();
    assert!(t.contains("1 page") && t.contains("Hello preview world"), "{t}");
}
