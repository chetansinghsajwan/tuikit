//! The Typst `World` for one document: its source, the embedded fonts and the files next to it.

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
        if id == self.main.id() { Ok(self.main.clone()) } else { Err(FileError::NotFound(id.vpath().as_rootless_path().to_path_buf())) }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let rel = id.vpath().as_rootless_path();
        let base = self.base.as_ref().ok_or_else(|| FileError::NotFound(rel.to_path_buf()))?;
        std::fs::read(base.join(rel)).map(Bytes::new).map_err(|_| FileError::NotFound(rel.to_path_buf()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

/// Lay out the Typst `source` and return the PDF.
pub fn compile(source: String, base: Option<PathBuf>) -> Result<Vec<u8>, String> {
    let fonts: Vec<Font> = typst_assets::fonts().flat_map(|data| Font::iter(Bytes::new(data))).collect();
    let book = FontBook::from_infos(fonts.iter().map(|f| f.info().clone()));
    let main = Source::new(FileId::new(None, VirtualPath::new("/main.typ")), source);
    let world = Doc { library: LazyHash::new(Library::default()), book: LazyHash::new(book), fonts, main, base };
    let document = typst::compile::<typst::layout::PagedDocument>(&world).output.map_err(|errors| errors.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; "))?;
    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|errors| errors.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; "))
}
