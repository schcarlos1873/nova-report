# Arquitetura do Engine Core: DataSources Unificados & Motor de Expressões

Este documento detalha o funcionamento interno do núcleo de processamento de dados e cálculo de expressões do **NovaReport Engine**.

---

## Reference
Utililize essa referencia como mandatória para o desenvolvimento [Arquitetura de Conectores Universais](universal_connectors_spec.md) 

## 1. Camada Unificada de DataSources (Polars & Apache Arrow)

Para suportar bancos de dados heterogêneos (PostgreSQL, SQL Server, Oracle, ODBC, APIs REST e arquivos locais) sem duplicar regras de agrupamento e formatação, todos os conectores convertem seus resultados para a estrutura tabular `polars::prelude::DataFrame`.

```text
[ PostgreSQL (sqlx) ] ──┐
[ SQL Server (tiberius) ] ┼─> [ DataProvider Trait ] ──> [ Polars DataFrame (Arrow zero-copy) ]
[ REST API / JSON ] ────┘
```

### 1.1 Contrato Único (`DataProvider Trait`)

```rust
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ParamValue {
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    Date(chrono::NaiveDate),
    DateTime(chrono::NaiveDateTime),
    Null,
}

#[async_trait]
pub trait DataProvider: Send + Sync {
    async fn execute_query(
        &self,
        query_or_endpoint: &str,
        params: &HashMap<String, ParamValue>,
    ) -> Result<DataFrame, String>;
}

pub struct DataContext {
    pub datasets: HashMap<String, DataFrame>,
    pub parameters: HashMap<String, ParamValue>,
}

impl DataContext {
    pub async fn load_all(
        sources: Vec<(String, Arc<dyn DataProvider>, String)>,
        params: HashMap<String, ParamValue>,
    ) -> Result<Self, String> {
        let mut datasets = HashMap::new();
        
        let futures = sources.into_iter().map(|(name, provider, query)| {
            let p = params.clone();
            async move {
                let df = provider.execute_query(&query, &p).await?;
                Ok::<(String, DataFrame), String>((name, df))
            }
        });

        for res in futures::future::join_all(futures).await {
            let (name, df) = res?;
            datasets.insert(name, df);
        }

        Ok(Self { datasets, parameters: params })
    }
}
```

### 1.2 Joins Heterogêneos & Matrizes (Cross-Tabs)

O Polars permite cruzar fontes diferentes e pivotar tabelas dinamicamente antes do layout:

* **Join Heterogêneo em Memória:**
  ```rust
  pub fn join_datasets(
      left: DataFrame,
      right: DataFrame,
      left_key: &str,
      right_key: &str,
  ) -> PolarsResult<DataFrame> {
      left.lazy()
          .join(
              right.lazy(),
              [col(left_key)],
              [col(right_key)],
              JoinArgs::new(JoinType::Left),
          )
          .collect()
  }
  ```

* **Pivotagem para Cross-Tabs:** O método nativo `pivot_stable` converte linhas dinâmicas em colunas com totais horizontais/verticais calculados vetorizadamente.

---

## 2. Árvore de Agrupamento e Fatiamento de Memória (`EvaluationScope`)

A varredura das bandas (*ReportHeader*, *GroupHeader*, *Detail*, *GroupFooter*, *ReportFooter*) consome fatias (*slices*) dos `DataFrame`s sem realizar cópia de memória:

```rust
pub struct EvaluationScope<'a> {
    pub current_row_idx: Option<usize>,
    pub group_slice: &'a DataFrame,
    pub full_dataset: &'a DataFrame,
    pub parameters: &'a HashMap<String, ParamValue>,
    pub running_totals: &'a HashMap<String, f64>,
}
```

---

## 3. Motor de Fórmulas Sandboxed com Rhai

O **Rhai** executa expressões dinâmicas compiladas em AST com proteção contra loops infinitos e acesso isolado à memória.

```rust
use rhai::{Dynamic, Engine, EvalAltResult, Scope, AST};
use polars::prelude::*;
use std::collections::HashMap;

pub struct FormulaEngine {
    engine: Engine,
    compiled_formulas: HashMap<String, AST>,
}

impl FormulaEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.set_max_operations(50_000);
        engine.set_max_expr_depths(32, 32);

        // Registro de funções auxiliares estilo Crystal / Excel
        engine.register_fn("IIF", |cond: bool, t_val: Dynamic, f_val: Dynamic| -> Dynamic {
            if cond { t_val } else { f_val }
        });

        engine.register_fn("FormatBRL", |val: f64| -> String {
            format!("R$ {:.2}", val).replace('.', ",")
        });

        Self {
            engine,
            compiled_formulas: HashMap::new(),
        }
    }

    pub fn compile(&mut self, id: &str, expr: &str) -> Result<(), Box<EvalAltResult>> {
        let ast = self.engine.compile(expr)?;
        self.compiled_formulas.insert(id.to_string(), ast);
        Ok(())
    }

    pub fn evaluate(
        &self,
        id: &str,
        ctx: &EvaluationScope,
    ) -> Result<Dynamic, Box<EvalAltResult>> {
        let ast = self.compiled_formulas.get(id).ok_or("Fórmula não encontrada")?;
        let mut scope = Scope::new();

        // 1. Injeta os campos da linha atual
        let mut fields = rhai::Map::new();
        if let Some(row_idx) = ctx.current_row_idx {
            for col in ctx.group_slice.get_columns() {
                // Injeta valor celular correspondente ao índice
                let val_str = format!("{:?}", col.get(row_idx));
                fields.insert(col.name().as_str().into(), Dynamic::from(val_str));
            }
        }
        scope.push_constant("Fields", fields);

        // 2. Injeta agregações do grupo atual
        let mut group_agg = rhai::Map::new();
        for col in ctx.group_slice.get_columns() {
            if col.dtype().is_numeric() {
                if let Ok(sum_val) = col.sum::<f64>() {
                    group_agg.insert(format!("Sum_{}", col.name()).into(), Dynamic::from(sum_val));
                }
            }
        }
        scope.push_constant("Group", group_agg);

        self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, ast)
    }
}
```