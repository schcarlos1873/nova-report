//! `World` do Typst totalmente em memória: fonte principal + assets virtuais + fontes.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::fonts::FontStore;

static LIBRARY: LazyLock<LazyHash<Library>> = LazyLock::new(|| LazyHash::new(Library::default()));

/// Catálogo de fontes carregado uma única vez e compartilhado entre compilações.
pub struct FontLibrary {
    store: FontStore,
}

impl FontLibrary {
    /// Fontes embutidas (Libertinus, New Computer Modern, DejaVu Sans Mono) e,
    /// opcionalmente, as fontes do sistema operacional.
    pub fn load(include_system: bool) -> Self {
        let mut store = FontStore::new();
        store.extend(typst_kit::fonts::embedded());
        if include_system {
            store.extend(typst_kit::fonts::system());
        }
        Self { store }
    }

    pub fn len(&self) -> usize {
        self.store.book().families().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Famílias disponíveis (para o seletor de fontes do designer).
    pub fn families(&self) -> Vec<String> {
        let mut v: Vec<String> = self.store.book().families().map(|(name, _)| name.to_string()).collect();
        v.sort();
        v.dedup();
        v
    }
}

pub struct InMemoryReportWorld {
    fonts: Arc<FontLibrary>,
    main_id: FileId,
    main: Source,
    assets: HashMap<FileId, Bytes>,
}

fn file_id(path: &str) -> FileId {
    let vpath = VirtualPath::new(path).unwrap_or_else(|_| VirtualPath::new("/invalid").expect("caminho fixo válido"));
    FileId::new(RootedPath::new(VirtualRoot::Project, vpath))
}

impl InMemoryReportWorld {
    pub fn new(source: String, assets: impl IntoIterator<Item = (String, Vec<u8>)>, fonts: Arc<FontLibrary>) -> Self {
        let main_id = file_id("/main.typ");
        let assets = assets.into_iter().map(|(path, data)| (file_id(&path), Bytes::new(data))).collect();
        Self { fonts, main_id, main: Source::new(main_id, source), assets }
    }
}

impl World for InMemoryReportWorld {
    fn library(&self) -> &LazyHash<Library> {
        &LIBRARY
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.store.book()
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id { Ok(self.main.clone()) } else { Err(FileError::NotSource) }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main_id {
            return Ok(Bytes::from_string(self.main.text().to_string()));
        }
        self.assets
            .get(&id)
            .cloned()
            .ok_or_else(|| FileError::NotFound(std::path::PathBuf::from(id.vpath().get_without_slash())))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.store.font(index)
    }

    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        use chrono::Datelike;
        let now = chrono::Local::now();
        Datetime::from_ymd(now.year(), now.month() as u8, now.day() as u8)
    }
}
