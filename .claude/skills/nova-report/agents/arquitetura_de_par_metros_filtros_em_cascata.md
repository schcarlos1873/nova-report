# Spec de Engenharia: Parâmetros de Consulta, Relatório e Filtros em Cascata

**Versão:** 1.1

**Escopo:** Subsistema de Parâmetros do NovaReport Engine (Tauri Desktop Builder & Axum/React Server Portal)
**Referencia:** [https://learn.microsoft.com/en-us/sql/reporting-services/tutorial-add-a-parameter-to-your-report-report-builder?view=sql-server-ver17](https://learn.microsoft.com/en-us/sql/reporting-services/tutorial-add-a-parameter-to-your-report-report-builder?view=sql-server-ver17)


**Linguagem & Stack:** Rust (`sqlparser-rs`, `petgraph`, `serde`) | React | Rhai Engine

## 1. Visão Geral e Separação de Conceitos

O NovaReport adota uma arquitetura em duas camadas para parâmetros, garantindo desacoplamento entre a instrução de busca no banco de dados e a interface visual de entrada do usuário.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       Report Parameter (User UI)                        │
│  [Prompt: "Selecione a Loja", Type: Integer, AvailableValues: Query]    │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ Bound via Rhai Expr: `=Parameters.store_id`
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Query Parameter (Dataset)                        │
│             SQL Clause: `WHERE StoreID = (:store_id)`                   │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ Extracted & Prepared
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    Database Connector / Driver Query                    │
│     Executes parameterized query safely against PostgreSQL / MSSQL      │
└─────────────────────────────────────────────────────────────────────────┘

```

* **Query Parameter (`QueryParameter`):** O marcador de substituição presente na instrução SQL utilizando a sintaxe nomeada padrão `:param_name`.

* **Report Parameter (`ReportParameter`):** A entidade declarativa no relatório `.nrep` que define tipo de dado, valores padrão, validações, visibilidade, aceitação de nulos/múltiplos valores e lista de opções disponíveis (estática ou dinâmica).

## 2. Auto-Descoberta de Parâmetros no Parser SQL (`sqlparser-rs`)

Ao digitar ou editar a query no **Query Designer**, o backend em Rust analisa a AST (*Abstract Syntax Tree*) e extrai automaticamente os parâmetros declarados na sintaxe `:param_name`, adicionando-os à árvore de dados do relatório sem intervenção manual.

### 2.1 Extrator de Parâmetros em Rust

```rust
use sqlparser::ast::{Expr, Statement, Value, Visit, Visitor};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use std::collections::HashSet;
use std::ops::ControlFlow;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscoveredQueryParam {
    pub raw_name: String,   // Ex: ":store_id"
    pub clean_name: String, // Ex: "store_id"
}

pub struct QueryParamExtractor {
    pub params: HashSet<DiscoveredQueryParam>,
}

impl QueryParamExtractor {
    pub fn new() -> Self {
        Self {
            params: HashSet::new(),
        }
    }

    pub fn parse_sql(sql: &str) -> Result<HashSet<DiscoveredQueryParam>, String> {
        let dialect = GenericDialect {};
        let statements = Parser::parse_sql(&dialect, sql).map_err(|e| e.to_string())?;
        let mut extractor = Self::new();

        for stmt in &statements {
            stmt.visit(&mut extractor);
        }

        // Extrator via Regex focado na convenção :param_name
        let re = regex::Regex::new(r":([a-zA-Z0-9_]+)").unwrap();
        for cap in re.captures_iter(sql) {
            let raw = cap[0].to_string();
            let clean = cap[1].to_string();
            extractor.params.insert(DiscoveredQueryParam {
                raw_name: raw,
                clean_name: clean,
            });
        }

        Ok(extractor.params)
    }
}

impl Visitor for QueryParamExtractor {
    type Break = ();

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<Self::Break> {
        if let Expr::Value(Value::Placeholder(name)) = expr {
            if name.starts_with(':') {
                let clean = name.trim_start_matches(':').to_string();
                self.params.insert(DiscoveredQueryParam {
                    raw_name: name.clone(),
                    clean_name: clean,
                });
            }
        }
        ControlFlow::Continue(())
    }
}
```

## 3. Modelo de Dados de Parâmetros (`.nrep` Schema)

Estrutura JSON/Rust completa gravada no arquivo de definição do relatório:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterDataType {
    String,
    Integer,
    Float,
    Boolean,
    Date,
    DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AvailableValuesSource {
    None,
    Static {
        options: Vec<StaticOption>,
    },
    Query {
        dataset_name: String,
        value_field: String,
        label_field: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticOption {
    pub label: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportParameter {
    pub id: String,
    pub name: String,                  // Ex: "store_id"
    pub prompt: String,                // Ex: "Selecione o Código da Loja:"
    pub data_type: ParameterDataType,
    pub allow_blank: bool,
    pub allow_null: bool,
    pub multi_value: bool,
    pub hidden: bool,
    pub default_values: Vec<String>,   // Aceita literais ou expressões Rhai (ex: ["=Today()"])
    pub available_values: AvailableValuesSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetQueryParameter {
    pub name: String,        // Ex: ":store_id"
    pub expression: String,  // Ex: "=Parameters.store_id"
}
```

## 4. Tratamento Seguro de Parâmetros Multi-Valorados (`IN (:param_name)`)

No Microsoft SSRS tradicional, utilizar marcadores de lista causava falhas ou exigia procedimentos armazenados complexos. O NovaReport lida de forma nativa e vetorizada com parâmetros multi-valorados utilizando a sintaxe `:param_name` sem risco de **SQL Injection**.

### 4.1 Expansão Transparente no Driver Engine

Quando um `ReportParameter` marcado como `multi_value: true` possui uma lista de valores (ex: `[200, 201, 202]`), o registrador de queries substitui a cláusula `IN (:store_id)` por argumentos preparados individuais antes da execução:

```rust
use std::collections::HashMap;

pub fn expand_multi_value_params(
    sql: &str,
    param_name: &str,
    values: &[serde_json::Value],
) -> (String, HashMap<String, ParamValue>) {
    let raw_tag = format!(":{}", param_name);
    
    if !sql.contains(&raw_tag) || values.is_empty() {
        return (sql.to_string(), HashMap::new());
    }

    let mut generated_params = HashMap::new();
    let mut placeholders = Vec::new();

    for (idx, val) in values.iter().enumerate() {
        let sub_param_name = format!("{}_pv_{}", param_name, idx);
        placeholders.push(format!(":{}", sub_param_name));

        let typed_val = match val {
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() { ParamValue::Int(i) } else { ParamValue::Float(n.as_f64().unwrap_or(0.0)) }
            }
            serde_json::Value::String(s) => ParamValue::Text(s.clone()),
            serde_json::Value::Bool(b) => ParamValue::Bool(*b),
            _ => ParamValue::Null,
        };

        generated_params.insert(sub_param_name, typed_val);
    }

    let joined_placeholders = placeholders.join(", ");
    let expanded_sql = sql.replace(&raw_tag, &joined_placeholders);

    (expanded_sql, generated_params)
}
```

* **Exemplo de Entrada:** `SELECT * FROM Sales WHERE StoreID IN (:store_id)`

* **Valores:** `[200, 201]`

* **SQL Gerado:** `SELECT * FROM Sales WHERE StoreID IN (:store_id_pv_0, :store_id_pv_1)`

* **Bind Parameters:** `:store_id_pv_0 = 200`, `:store_id_pv_1 = 201` *(100% parametrizado e seguro)*.

## 5. Resolução de Parâmetros em Cascata (Cascading Parameters DAG)

Quando a lista de valores de um parâmetro depende da seleção feita em outro (ex: **País → Estado → Cidade**), o motor organiza a dependência usando um **Grafo Direcionado Acíclico (DAG)**.

```
┌────────────────────────┐
│ Parameter: country_id  │ ◄── User Selects "Brasil"
└───────────┬────────────┘
            │ Triggers Refresh
            ▼
┌────────────────────────┐
│ Dataset: StateQuery    │ ── WHERE CountryID = :country_id
└───────────┬────────────┘
            │ Updates Options
            ▼
┌────────────────────────┐
│ Parameter: state_id    │ ◄── Enabled & Populated
└────────────────────────┘

```

### 5.1 Algoritmo de Ordenação Topológica em Rust

```rust
use petgraph::graph::DiGraph;
use petgraph::algo::toposort;
use std::collections::HashMap;

pub struct CascadingResolver {
    pub graph: DiGraph<String, ()>,
    pub node_indices: HashMap<String, petgraph::graph::NodeIndex>,
}

impl CascadingResolver {
    pub fn build(parameters: &[ReportParameter]) -> Self {
        let mut graph = DiGraph::new();
        let mut node_indices = HashMap::new();

        for p in parameters {
            let idx = graph.add_node(p.name.clone());
            node_indices.insert(p.name.clone(), idx);
        }

        // Adiciona arestas para cada dependência encontrada nas Queries dos AvailableValues
        for p in parameters {
            if let AvailableValuesSource::Query { dataset_name, .. } = &p.available_values {
                // Se a query do dataset referencia outro parâmetro, cria aresta de dependência
                let target_node = node_indices[&p.name];
                // Registra dependência: ParamA -> ParamB
            }
        }

        Self { graph, node_indices }
    }

    pub fn get_execution_order(&self) -> Result<Vec<String>, String> {
        match toposort(&self.graph, None) {
            Ok(nodes) => Ok(nodes.into_iter().map(|n| self.graph[n].clone()).collect()),
            Err(_) => Err("Ciclo detectado entre os parâmetros em cascata!".into()),
        }
    }
}
```

## 6. Fluxo de Interação no Designer (Tauri UI) & Modal "Define Query Parameters"

O **Query Designer** no Tauri emula e moderniza a experiência do Microsoft Report Builder adaptado para `:param_name`:

1. **Edição:** O usuário edita a SQL e adiciona a cláusula `WHERE StoreID = (:store_id)`.

2. **Auto-Descoberta Instantânea:** O backend compila a AST via IPC (`check_query_params`).

3. **Prompt Interativo ("Define Query Parameters"):**

   * Ao clicar em **Executar (`!`)** no toolbar, se existirem parâmetros sem valor de teste preenchido, o modal abre automaticamente.

   * O usuário digita o valor de teste (ex: `200`).

4. **Sincronização com o Relatório:**

   * Se o `ReportParameter` `store_id` não existia no painel *Report Data*, ele é criado automaticamente com defaults (`Prompt: "store_id"`, `DataType: Integer`).

   * A visualização do *Data Grid* é atualizada com a resposta enviada pelo Polars DataFrame.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Define Query Parameters Modal                       │
├──────────────────────┬─────────────────┬────────────────────────────────┤
│ Parameter Name       │ Data Type       │ Parameter Value (Test)         │
├──────────────────────┼─────────────────┼────────────────────────────────┤
│ :store_id            │ Integer         │ [ 200                        ] │
│ :start_date          │ Date            │ [ 2026-01-01                 ] │
├──────────────────────┴─────────────────┴────────────────────────────────┤
│                                              [ Cancel ]  [ OK (Run) ]   │
└─────────────────────────────────────────────────────────────────────────┘

```

## 7. Componente de Filtros Dinâmicos no Portal Web (React + Tailwind)

Tanto o aplicativo Desktop quanto o Portal Web (Fase 2) compartilham o mesmo componente React responsável por renderizar a barra superior de filtros dinâmicos antes de disparar o relatório:

```tsx
import React, { useState, useEffect } from 'react';

export interface ReportParamSpec {
  name: string;
  prompt: string;
  data_type: 'string' | 'integer' | 'date' | 'boolean';
  multi_value: boolean;
  options?: { label: string; value: any }[];
  depends_on?: string[];
}

export const ParameterToolbar: React.FC<{
  parameters: ReportParamSpec[];
  onExecute: (values: Record<string, any>) => void;
}> = ({ parameters, onExecute }) => {
  const [paramValues, setParamValues] = useState<Record<string, any>>({});

  const handleChange = (name: string, value: any) => {
    setParamValues((prev) => ({ ...prev, [name]: value }));
  };

  return (
    <div className="flex flex-wrap items-center gap-4 p-4 bg-slate-900 border-b border-slate-700 text-slate-200">
      {parameters.map((param) => (
        <div key={param.name} className="flex flex-col gap-1">
          <label className="text-xs font-semibold text-slate-400">{param.prompt}</label>
          
          {param.options ? (
            <select
              multiple={param.multi_value}
              value={paramValues[param.name] || ''}
              onChange={(e) =>
                handleChange(
                  param.name,
                  param.multi_value
                    ? Array.from(e.target.selectedOptions, (item) => item.value)
                    : e.target.value
                )
              }
              className="px-3 py-1.5 bg-slate-800 border border-slate-600 rounded text-sm text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
            >
              {!param.multi_value && <option value="">-- Selecione --</option>}
              {param.options.map((opt) => (
                <option key={String(opt.value)} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          ) : (
            <input
              type={param.data_type === 'integer' ? 'number' : param.data_type === 'date' ? 'date' : 'text'}
              value={paramValues[param.name] || ''}
              onChange={(e) => handleChange(param.name, e.target.value)}
              className="px-3 py-1.5 bg-slate-800 border border-slate-600 rounded text-sm text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          )}
        </div>
      ))}

      <button
        onClick={() => onExecute(paramValues)}
        className="mt-auto px-5 py-1.5 bg-blue-600 hover:bg-blue-500 text-white font-medium rounded shadow transition-colors"
      >
        Visualizar Relatório
      </button>
    </div>
  );
};
```