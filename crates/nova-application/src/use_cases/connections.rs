//! Caso de uso: gerenciamento de conexões, navegação de schema e pré-visualização de dados.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use nova_domain::ParamValue;
use nova_domain::connection::{ConnectionConfig, ConnectionProfile, DatabaseTree};
use nova_domain::report::{DataSourceDef, ParameterDef, QueryDef, ReportDefinition};
use nova_engine::data::CellValue;
use serde::Serialize;

use crate::connection_resolver::ConnectionResolver;
use crate::ports::{ConnectorGateway, DriverDescriptor};
use crate::{AppError, AppResult, params};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewColumn {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterOption {
    pub value: serde_json::Value,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterValues {
    pub values: Vec<ParameterOption>,
    /// A query devolveu mais linhas que o limite.
    pub truncated: bool,
    /// Rótulos dos parâmetros de `dependsOn` ainda sem valor.
    pub waiting_for: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryPreview {
    pub columns: Vec<PreviewColumn>,
    pub rows: Vec<Vec<CellValue>>,
    pub elapsed_ms: u128,
}

#[derive(Clone)]
pub struct ConnectionService {
    gateway: Arc<dyn ConnectorGateway>,
    resolver: ConnectionResolver,
}

impl ConnectionService {
    pub fn new(gateway: Arc<dyn ConnectorGateway>, resolver: ConnectionResolver) -> Self {
        Self { gateway, resolver }
    }

    pub fn drivers(&self) -> Vec<DriverDescriptor> {
        self.gateway.drivers()
    }

    pub fn list(&self) -> AppResult<Vec<ConnectionProfile>> {
        Ok(self.resolver.profiles().list()?)
    }

    /// Salva o perfil; a senha (se informada) vai para o cofre do SO. `Some("")` remove a senha.
    pub fn save(&self, profile: ConnectionProfile, password: Option<String>) -> AppResult<()> {
        if profile.alias.trim().is_empty() {
            return Err(AppError::Invalid("o alias da conexão é obrigatório".into()));
        }
        if let Some(cs) = &profile.connection_string {
            let lower = cs.to_lowercase();
            if ["password=", "pwd="].iter().any(|k| lower.contains(k) && !lower.contains("{password}")) {
                return Err(AppError::Invalid(
                    "a connection string não deve conter a senha; use o campo senha e o marcador {PASSWORD}".into(),
                ));
            }
        }
        match password.as_deref() {
            Some("") => self.resolver.vault().delete(&profile.alias)?,
            Some(pw) => self.resolver.vault().store(&profile.alias, pw)?,
            None => {}
        }
        self.resolver.profiles().save(profile)?;
        Ok(())
    }

    pub fn delete(&self, alias: &str) -> AppResult<()> {
        self.resolver.vault().delete(alias)?;
        Ok(self.resolver.profiles().delete(alias)?)
    }

    /// Testa um perfil ainda não salvo (senha opcional; senão usa o cofre).
    pub async fn test(&self, profile: ConnectionProfile, password: Option<String>) -> AppResult<u64> {
        let password = match password {
            Some(p) if !p.is_empty() => Some(p),
            _ => self.resolver.vault().get(&profile.alias)?,
        };
        self.gateway.test_connection(&ConnectionConfig::new(profile, password)).await
    }

    pub async fn schema(&self, alias: &str) -> AppResult<DatabaseTree> {
        self.gateway.introspect(&self.resolver.by_alias(alias)?).await
    }

    pub fn select_all_sql(&self, alias: &str, schema: Option<&str>, table: &str) -> AppResult<String> {
        let cfg = self.resolver.by_alias(alias)?;
        Ok(self.gateway.select_all(cfg.profile.provider, schema, table))
    }

    /// Executa uma query ad-hoc em uma conexão salva (limitada a `limit` linhas).
    pub async fn preview_sql(
        &self,
        alias: &str,
        sql: &str,
        raw_params: &HashMap<String, serde_json::Value>,
        limit: usize,
    ) -> AppResult<QueryPreview> {
        let cfg = self.resolver.by_alias(alias)?;
        let params = raw_params.iter().map(|(k, v)| (k.clone(), json_param(v))).collect();
        run_preview(self.gateway.as_ref(), cfg, sql, true, params, limit).await
    }

    /// Executa uma query declarada no relatório (usada para descobrir campos no designer).
    pub async fn preview_report_query(
        &self,
        def: &ReportDefinition,
        query_name: &str,
        raw_params: &HashMap<String, serde_json::Value>,
        base_dir: Option<&std::path::Path>,
        limit: usize,
    ) -> AppResult<QueryPreview> {
        let (ds, q) = def.find_query(query_name).ok_or_else(|| AppError::Invalid(format!("query '{query_name}' não existe")))?;
        let cfg = self.resolver.for_data_source(ds, base_dir)?;
        // no designer, parâmetros obrigatórios podem estar vazios: os preenchidos continuam valendo
        let params = params::resolve_lenient(def, raw_params);
        run_preview(self.gateway.as_ref(), cfg, &query_sql(ds, q)?, !q.is_procedure(), params, limit).await
    }

    /// Lista de valores de um parâmetro com `availableValues` (filtros em cascata).
    /// Enquanto algum parâmetro de `dependsOn` estiver vazio, devolve `waitingFor` sem consultar o banco.
    pub async fn parameter_values(
        &self,
        def: &ReportDefinition,
        name: &str,
        raw_params: &HashMap<String, serde_json::Value>,
        base_dir: Option<&std::path::Path>,
        limit: usize,
    ) -> AppResult<ParameterValues> {
        let p = def.parameters.iter().find(|p| p.name == name).ok_or_else(|| AppError::Invalid(format!("parâmetro '{name}' não existe")))?;
        if p.available_values.is_none() {
            return Err(AppError::Invalid(format!("parâmetro '{name}' não tem lista de valores")));
        }
        let params = params::resolve_lenient(def, raw_params);
        let waiting_for: Vec<String> = p
            .depends_on
            .iter()
            .filter(|d| params.get(*d).is_none_or(ParamValue::is_empty))
            .map(|d| def.parameters.iter().find(|x| &x.name == d).map_or(d.clone(), |x| if x.label.is_empty() { x.name.clone() } else { x.label.clone() }))
            .collect();
        if !waiting_for.is_empty() {
            return Ok(ParameterValues { values: vec![], truncated: false, waiting_for });
        }

        let (values, truncated) = fetch_options(self.gateway.as_ref(), &self.resolver, def, p, params, base_dir, limit).await?;
        Ok(ParameterValues { values, truncated, waiting_for: vec![] })
    }
}

/// Executa a query de `availableValues` de `p` e devolve as opções (valor já no tipo do parâmetro)
/// e se a lista foi truncada em `limit`. Compartilhada entre a lista da UI e os rótulos do render.
pub(crate) async fn fetch_options(
    gateway: &dyn ConnectorGateway,
    resolver: &ConnectionResolver,
    def: &ReportDefinition,
    p: &ParameterDef,
    params: HashMap<String, ParamValue>,
    base_dir: Option<&std::path::Path>,
    limit: usize,
) -> AppResult<(Vec<ParameterOption>, bool)> {
    let av = p.available_values.as_ref().ok_or_else(|| AppError::Invalid(format!("parâmetro '{}' não tem lista de valores", p.name)))?;
    let (ds, q) = def.find_query(&av.query).ok_or_else(|| AppError::Invalid(format!("query '{}' não existe", av.query)))?;
    let cfg = resolver.for_data_source(ds, base_dir)?;
    let preview = run_preview(gateway, cfg, &query_sql(ds, q)?, !q.is_procedure(), params, limit + 1).await?;
    let col = |field: &str| {
        preview.columns.iter().position(|c| c.name.eq_ignore_ascii_case(field)).ok_or_else(|| {
            let found: Vec<&str> = preview.columns.iter().map(|c| c.name.as_str()).collect();
            AppError::Invalid(format!("a query '{}' não tem a coluna '{field}' (colunas: {})", q.name, found.join(", ")))
        })
    };
    let vi = col(&av.value_field)?;
    let li = av.label_field.as_deref().filter(|f| !f.is_empty()).map(col).transpose()?.unwrap_or(vi);

    let truncated = preview.rows.len() > limit;
    let mut values: Vec<ParameterOption> = Vec::new();
    for row in preview.rows.iter().take(limit) {
        let cell = &row[vi];
        if *cell == CellValue::Null {
            continue;
        }
        // mesmo tipo que o relatório vai receber (ex.: Id inteiro de um parâmetro `int`)
        let raw = serde_json::to_value(cell).unwrap_or(serde_json::Value::Null);
        let value = ParamValue::coerce(&p.name, p.param_type, &raw).map_err(AppError::from)?.to_json();
        if values.iter().any(|o| o.value == value) {
            continue;
        }
        values.push(ParameterOption { value, label: row[li].to_display() });
    }
    Ok((values, truncated))
}

/// SQL enviada ao banco para a query (texto ou chamada da stored procedure no dialeto da fonte).
pub(crate) fn query_sql(ds: &DataSourceDef, q: &QueryDef) -> AppResult<String> {
    q.effective_sql(ds.provider)
        .ok_or_else(|| AppError::Invalid(format!("o provider {} não suporta stored procedures (query '{}')", ds.provider.id(), q.name)))
}

/// `paginate = false` para procedures: `EXEC`/`CALL` não podem ser envolvidos num subselect; o
/// resultado é cortado em `limit` depois da execução.
async fn run_preview(gateway: &dyn ConnectorGateway, cfg: ConnectionConfig, sql: &str, paginate: bool, params: HashMap<String, ParamValue>, limit: usize) -> AppResult<QueryPreview> {
    let kind = cfg.profile.provider;
    let sql = if kind.is_sql() && paginate { gateway.paginate(kind, sql, limit) } else { sql.to_string() };
    let provider = gateway.provider(cfg)?;
    let start = Instant::now();
    let df = provider.execute_query(&sql, &params).await?;
    let elapsed_ms = start.elapsed().as_millis();
    let df = df.head(Some(limit));
    let columns = df
        .columns()
        .iter()
        .map(|c| PreviewColumn { name: c.name().to_string(), data_type: c.dtype().to_string() })
        .collect();
    let mut rows = Vec::with_capacity(df.height());
    for r in 0..df.height() {
        let mut row = Vec::with_capacity(df.width());
        for c in df.columns() {
            row.push(c.get(r).map(|v| CellValue::from_any(&v)).unwrap_or(CellValue::Null));
        }
        rows.push(row);
    }
    Ok(QueryPreview { columns, rows, elapsed_ms })
}

fn json_param(v: &serde_json::Value) -> ParamValue {
    match v {
        serde_json::Value::Null => ParamValue::Null,
        serde_json::Value::Bool(b) => ParamValue::Bool(*b),
        serde_json::Value::Number(n) => n.as_i64().map(ParamValue::Int).unwrap_or_else(|| ParamValue::Float(n.as_f64().unwrap_or(0.0))),
        serde_json::Value::String(s) => ParamValue::Text(s.clone()),
        other => ParamValue::Text(other.to_string()),
    }
}
