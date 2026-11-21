use serde::{Deserialize, Serialize};

/// Prefixo do `source` de um elemento imagem que referencia um recurso de `images`.
pub const IMAGE_REF_PREFIX: &str = "image:";

/// Imagem declarada no relatório, reutilizável por vários elementos (`source: "image:logo"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageResource {
    pub name: String,
    pub kind: ImageKind,
    /// `file`: caminho relativo ao `.nrep` ou absoluto; `url`: endereço http(s);
    /// `embedded`: `data:` URI. Não usado em `database`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// `database`: query e campo com a imagem (data URI, base64 ou caminho), lidos a cada registro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageKind {
    File,
    Url,
    Embedded,
    Database,
}

impl ImageResource {
    /// Extensão exibida na árvore (`logo (svg)`).
    pub fn format_hint(&self) -> Option<String> {
        if let Some(m) = &self.mime_type {
            return m.split('/').nth(1).map(|s| s.replace("svg+xml", "svg").replace("jpeg", "jpg"));
        }
        match self.kind {
            ImageKind::File | ImageKind::Url => {
                let path = self.value.split(['?', '#']).next().unwrap_or("");
                path.rsplit_once('.').map(|(_, e)| e.to_lowercase()).filter(|e| e.len() <= 5)
            }
            _ => None,
        }
    }
}

/// Nome do recurso quando `source` é `image:<nome>`.
pub fn image_ref(source: &str) -> Option<&str> {
    source.strip_prefix(IMAGE_REF_PREFIX).map(str::trim)
}
