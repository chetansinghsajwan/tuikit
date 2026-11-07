//! The Typst `World` for one document: its source, the fonts and the files next to it.

use std::path::PathBuf;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

struct Doc {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: Source,
    base: Option<PathBuf>,
}

impl World for Doc {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else {
            Err(FileError::NotFound(id.vpath().as_rootless_path().to_path_buf()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let rel = id.vpath().as_rootless_path();
        let base = self.base.as_ref().ok_or_else(|| FileError::NotFound(rel.to_path_buf()))?;
        std::fs::read(base.join(rel))
            .map(Bytes::new)
            .map_err(|_| FileError::NotFound(rel.to_path_buf()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

/// The fonts typst-assets embeds.
pub fn embedded_fonts() -> Vec<Font> {
    typst_assets::fonts().flat_map(|data| Font::iter(Bytes::new(data))).collect()
}

/// The fonts installed on the machine (found like fontconfig finds them), only the families in `wanted` when given.
/// A family name without case, spaces or punctuation (`JetBrains Mono` and `JetBrainsMono` are the same).
fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

pub fn system_fonts(
    wanted: Option<&[&str]>,
) -> (Vec<Font>, std::collections::HashMap<String, String>) {
    let mut aliases = std::collections::HashMap::new();
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    for d in
        std::env::var("MDEXPORT_FONT_DIRS").unwrap_or_default().split(':').filter(|s| !s.is_empty())
    {
        db.load_fonts_dir(d);
    }
    let mut out = Vec::new();
    for face in db.faces() {
        let hit = |w: &&str| face.families.iter().any(|(name, _)| norm(name) == norm(w));
        if wanted.is_some_and(|w| !w.iter().any(hit)) {
            continue;
        }
        let path = match &face.source {
            fontdb::Source::File(p) | fontdb::Source::SharedFile(p, _) => p.clone(),
            _ => continue,
        };
        if let Ok(data) = std::fs::read(&path)
            && let Some(font) = Font::new(Bytes::new(data), face.index)
        {
            // The name asked for may be another name of the family than the one Typst reads.
            if let Some(w) = wanted.and_then(|w| w.iter().find(|w| hit(w))) {
                aliases.insert((*w).to_string(), font.info().family.clone());
            }
            out.push(font);
        }
    }
    (out, aliases)
}

/// The family names of `fonts`, sorted, each once.
pub fn families(fonts: &[Font]) -> Vec<String> {
    let mut names: Vec<String> = fonts.iter().map(|f| f.info().family.clone()).collect();
    names.sort();
    names.dedup();
    names
}

/// Lay out the Typst `source` with `fonts` and return the PDF.
pub fn compile(source: String, base: Option<PathBuf>, fonts: Vec<Font>) -> Result<Vec<u8>, String> {
    let book = FontBook::from_infos(fonts.iter().map(|f| f.info().clone()));
    let main = Source::new(FileId::new(None, VirtualPath::new("/main.typ")), source);
    let world = Doc {
        library: LazyHash::new(Library::default()),
        book: LazyHash::new(book),
        fonts,
        main,
        base,
    };
    let document =
        typst::compile::<typst::layout::PagedDocument>(&world).output.map_err(|errors| {
            errors.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; ")
        })?;
    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|errors| {
        errors.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; ")
    })
}
