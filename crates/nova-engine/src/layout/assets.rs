use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use base64::Engine as _;

/// Tipo MIME de uma imagem pelos primeiros bytes.
pub fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    let head = &bytes[..bytes.len().min(256)];
    if head.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if head.starts_with(b"GIF8") {
        Some("image/gif")
    } else if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        let text = String::from_utf8_lossy(head);
        let t = text.trim_start();
        (t.starts_with("<svg") || (t.starts_with("<?xml") && text.contains("<svg"))).then_some("image/svg+xml")
    }
}

/// Valor de um campo do banco como origem de imagem: `data:` URI e caminhos passam direto;
/// base64 puro vira `data:` URI com o tipo detectado.
pub fn field_image_source(value: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Err("campo de imagem vazio".into());
    }
    if v.starts_with("data:") {
        return Ok(v.to_string());
    }
    let looks_b64 = v.len() >= 64 && v.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'=' | b'\n' | b'\r'));
    if looks_b64 {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(v.replace(['\n', '\r'], "")) {
            let mime = sniff_image_mime(&bytes).unwrap_or("image/png");
            return Ok(format!("data:{mime};base64,{}", v.replace(['\n', '\r'], "")));
        }
    }
    Ok(v.to_string())
}

/// Resolve imagens (`data:` URI, caminho relativo ao `.nrep` ou absoluto) para bytes
/// expostos ao Typst por caminho virtual.
#[derive(Default)]
pub struct AssetResolver {
    base_dir: Option<PathBuf>,
    by_source: BTreeMap<String, String>,
    pub files: BTreeMap<String, Vec<u8>>,
}

impl AssetResolver {
    pub fn new(base_dir: Option<&Path>) -> Self {
        Self { base_dir: base_dir.map(Path::to_path_buf), ..Default::default() }
    }

    /// Retorna o caminho virtual do recurso, carregando-o na primeira vez.
    pub fn resolve(&mut self, source: &str) -> Result<String, String> {
        if let Some(v) = self.by_source.get(source) {
            return Ok(v.clone());
        }
        let (bytes, ext) = if let Some(rest) = source.strip_prefix("data:") {
            let (meta, data) = rest.split_once(',').ok_or("data URI inválida")?;
            let ext = meta.split(';').next().and_then(|m| m.split('/').nth(1)).unwrap_or("png");
            let bytes = if meta.ends_with(";base64") {
                base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|e| e.to_string())?
            } else {
                data.as_bytes().to_vec()
            };
            (bytes, ext.replace("svg+xml", "svg").replace("jpeg", "jpg"))
        } else {
            let p = Path::new(source);
            let full = if p.is_absolute() { p.to_path_buf() } else { self.base_dir.clone().unwrap_or_default().join(p) };
            let bytes = std::fs::read(&full).map_err(|e| format!("{}: {e}", full.display()))?;
            let ext = full.extension().and_then(|e| e.to_str()).unwrap_or("png").to_lowercase();
            (bytes, ext)
        };
        let vpath = format!("/assets/img_{}.{}", self.files.len() + 1, ext);
        self.files.insert(vpath.clone(), bytes);
        self.by_source.insert(source.to_string(), vpath.clone());
        Ok(vpath)
    }
}
