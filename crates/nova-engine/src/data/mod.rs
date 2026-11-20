//! Camada unificada de dados: todo conector entrega um `polars::DataFrame`.

mod builder;
mod cell;
mod transform;

pub use builder::{ColumnKind, DataFrameBuilder};
pub use cell::CellValue;
pub use transform::{crosstab, join_datasets};

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use nova_domain::ParamValue;
use polars::prelude::DataFrame;

use crate::{EngineError, EngineResult};

/// Contrato único dos conectores (Tier 1 nativo, Tier 2 ODBC, Tier 3 gRPC, arquivos, REST).
#[async_trait]
pub trait DataProvider: Send + Sync {
    async fn execute_query(
        &self,
        query_or_endpoint: &str,
        params: &HashMap<String, ParamValue>,
    ) -> EngineResult<DataFrame>;
}

/// Uma query a ser carregada no contexto.
pub struct DataRequest {
    pub name: String,
    pub provider: Arc<dyn DataProvider>,
    pub query: String,
}

/// Todos os datasets do relatório já materializados + parâmetros.
#[derive(Debug, Clone, Default)]
pub struct DataContext {
    pub datasets: HashMap<String, DataFrame>,
    pub parameters: HashMap<String, ParamValue>,
}

impl DataContext {
    /// Executa todas as queries em paralelo (I/O assíncrono, sem bloquear a UI).
    pub async fn load_all(
        requests: Vec<DataRequest>,
        parameters: HashMap<String, ParamValue>,
    ) -> EngineResult<Self> {
        let futures = requests.into_iter().map(|req| {
            let p = &parameters;
            async move {
                let df = req
                    .provider
                    .execute_query(&req.query, p)
                    .await
                    .map_err(|e| EngineError::DataSource(format!("query '{}': {e}", req.name)))?;
                Ok::<_, EngineError>((req.name, df))
            }
        });
        let mut datasets = HashMap::new();
        for res in futures::future::join_all(futures).await {
            let (name, df) = res?;
            datasets.insert(name, df);
        }
        Ok(Self { datasets, parameters })
    }

    pub fn dataset(&self, name: &str) -> EngineResult<&DataFrame> {
        self.datasets.get(name).ok_or_else(|| EngineError::DatasetNotFound(name.to_string()))
    }
}
