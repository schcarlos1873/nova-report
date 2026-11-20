//! APIs REST / JSON. `path` do perfil é a URL base; a "query" é o endpoint relativo
//! (ou URL absoluta). Parâmetros `:nome` são substituídos (URL-encoded).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use nova_domain::ParamValue;
use nova_domain::connection::{ConnectionConfig, DatabaseTree, TableKind, TableMetadata};
use nova_domain::report::ProviderKind;
use nova_domain::report::sql::{SqlToken, tokenize};
use nova_engine::EngineResult;
use nova_engine::data::DataProvider;
use polars::prelude::DataFrame;

use super::file::json_to_rowset;
use crate::registry::{Driver, DriverInfo};
use crate::{ConnectorError, ConnectorResult};

#[derive(Default)]
pub struct RestDriver;

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn build_url(base: Option<&str>, endpoint: &str, params: &HashMap<String, ParamValue>) -> ConnectorResult<String> {
    let endpoint: String = tokenize(endpoint.trim())
        .into_iter()
        .map(|t| match t {
            SqlToken::Text(s) => s.to_string(),
            SqlToken::Param(p) => encode(&params.get(p).map(ParamValue::to_text).unwrap_or_default()),
        })
        .collect();
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return Ok(endpoint);
    }
    let base = base.ok_or_else(|| ConnectorError::Config("informe a URL base da API".into()))?;
    Ok(match (base.ends_with('/'), endpoint.starts_with('/')) {
        (true, true) => format!("{base}{}", &endpoint[1..]),
        (false, false) if !endpoint.is_empty() => format!("{base}/{endpoint}"),
        _ => format!("{base}{endpoint}"),
    })
}

pub struct RestProvider {
    client: reqwest::Client,
    config: ConnectionConfig,
}

impl RestProvider {
    fn new(config: ConnectionConfig) -> ConnectorResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("NovaReport/0.1")
            .build()
            .map_err(|e| ConnectorError::Config(e.to_string()))?;
        Ok(Self { client, config })
    }

    async fn fetch(&self, endpoint: &str, params: &HashMap<String, ParamValue>) -> ConnectorResult<serde_json::Value> {
        let p = &self.config.profile;
        let url = build_url(p.path.as_deref(), endpoint, params)?;
        let mut req = self.client.get(&url).header("Accept", "application/json");
        for (k, v) in &p.options {
            if let Some(h) = k.strip_prefix("header.") {
                req = req.header(h, v);
            }
        }
        if let Some(secret) = &self.config.password {
            req = match p.options.get("authScheme").map(String::as_str) {
                Some("basic") => req.basic_auth(p.username.clone().unwrap_or_default(), Some(secret)),
                Some("none") => req,
                _ => req.bearer_auth(secret),
            };
        }
        let resp = req.send().await.map_err(|e| ConnectorError::Connection(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(ConnectorError::Query(format!("HTTP {status} em {url}")));
        }
        resp.json().await.map_err(|e| ConnectorError::Query(format!("resposta não é JSON: {e}")))
    }
}

#[async_trait]
impl DataProvider for RestProvider {
    async fn execute_query(&self, endpoint: &str, params: &HashMap<String, ParamValue>) -> EngineResult<DataFrame> {
        let json = self.fetch(endpoint, params).await?;
        let rs = json_to_rowset(&json, self.config.profile.options.get("jsonPath").map(String::as_str))?;
        Ok(rs.into_dataframe()?)
    }
}

#[async_trait]
impl Driver for RestDriver {
    fn info(&self) -> DriverInfo {
        DriverInfo {
            id: "rest",
            name: "API REST (JSON)",
            tier: "api",
            default_port: None,
            fields: &["path", "username", "password"],
            supports_introspection: false,
        }
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Rest
    }

    async fn test_connection(&self, config: &ConnectionConfig) -> ConnectorResult<u64> {
        let start = Instant::now();
        RestProvider::new(config.clone())?.fetch("", &HashMap::new()).await?;
        Ok(super::elapsed_ms(start))
    }

    async fn introspect(&self, config: &ConnectionConfig) -> ConnectorResult<DatabaseTree> {
        Ok(DatabaseTree::build(
            config.profile.path.clone().unwrap_or_default(),
            vec![TableMetadata { schema: "api".into(), name: "endpoint".into(), kind: TableKind::View, comment: None, columns: vec![] }],
            vec![],
        ))
    }

    fn provider(&self, config: ConnectionConfig) -> ConnectorResult<Arc<dyn DataProvider>> {
        Ok(Arc::new(RestProvider::new(config)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_building_and_param_encoding() {
        let params = HashMap::from([("q".to_string(), ParamValue::Text("são paulo".into()))]);
        assert_eq!(
            build_url(Some("https://api.x.com:8443/v1/"), "/clientes?cidade=:q", &params).unwrap(),
            "https://api.x.com:8443/v1/clientes?cidade=s%C3%A3o%20paulo"
        );
        assert_eq!(build_url(None, "https://a.b/c", &params).unwrap(), "https://a.b/c");
    }
}
