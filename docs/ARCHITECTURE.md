# Arquitetura do NovaReport Builder

## Camadas (Clean Architecture)

```
            ┌──────────────────────────────────────────────┐
            │ Apresentação: apps/desktop (Tauri + React)    │
            │               crates/nova-cli                 │  composition roots
            └───────────────┬──────────────────────────────┘
                            │ injeta implementações
            ┌───────────────▼──────────────┐   ┌──────────────────────────┐
            │ nova-application (casos de    │◄──┤ nova-connectors           │
            │ uso + port ConnectorGateway)  │   │ (drivers, cofre, repos)   │
            └───────────────┬──────────────┘   └────────────┬─────────────┘
                            │                               │ implementa DataProvider
            ┌───────────────▼───────────────────────────────▼┐
            │ nova-engine (Polars, Rhai, bandas, Typst)        │
            └───────────────┬─────────────────────────────────┘
            ┌───────────────▼──────────────┐
            │ nova-domain (agregado .nrep,  │  sem dependências de I/O
            │ invariantes, ports)           │
            └──────────────────────────────┘
```

As dependências apontam sempre para dentro. A infraestrutura (`nova-connectors`) implementa os
ports do domínio (`CredentialVault`, `ConnectionProfileRepository`, `ReportRepository`), do engine
(`DataProvider`) e da aplicação (`ConnectorGateway`). Os composition roots (desktop e CLI) fazem a
ligação.

## Domínio (DDD)

* **Agregado** `ReportDefinition`: raiz de consistência do `.nrep`. Invariantes em `validate()`:
  ids únicos, bandas singleton, grupos existentes, queries referenciadas, parâmetros SQL
  declarados, elementos dentro da área útil, cabeçalho/rodapé cabendo na página.
* **Value objects**: `PageGeometry`, `Margins`, `ParamValue` (coerção tipada), `Style`.
* **Serviço de domínio** `report::sql`: scanner de parâmetros nomeados `:nome` (ignora literais,
  comentários e casts `::`), usado na validação e na reescrita de placeholders por driver.
* **Serialização determinística**: `to_canonical_json()` normaliza a ordem das bandas e campos.

## Pipeline de renderização

```
.nrep ─► params::resolve ─► ConnectionResolver (perfil + cofre do SO / env)
      ─► DataProvider* (paralelo, tokio) ─► DataContext { DataFrame por query }   [cache 120 s]
      ─► ReportProcessor (spawn_blocking)
           · 2 passadas: segmentos de grupo → emissão header/detail/footer
           · FormulaEngine Rhai (AST cacheado, limites de operações/profundidade)
      ─► RenderedReport (modelo intermediário em mm)
      ─► TypstTranspiler  (texto do usuário sempre como literal de string Typst)
      ─► typst::compile (World em memória, fontes compartilhadas, memoização comemo)
      ─► PDF (typst-pdf) | SVG por página (typst-svg) | XLSX/CSV (dados)
```

Decisões relevantes:

* **Segurança contra injeção no Typst**: nenhum texto vindo de dados/usuário é emitido como markup;
  tudo vira `"string"` escapada. Cores são validadas como hex.
* **Paginação**: `PAGE_NUMBER()`/`{PAGE}` geram marcadores substituídos por `counter(page)` do Typst.
* **Report header antes do page header** na 1ª página (convenção Crystal/Jasper) via cabeçalho
  condicional + deslocamento negativo do fluxo.
* **Tabelas** quebram entre páginas com `table.header(repeat: true)`; elementos acima da tabela na
  mesma banda formam um bloco fixo.
* **Can-grow**: altura estimada pela métrica média da fonte (heurística conservadora).

## Conectores

| Tier | Driver | Crate | Bind de parâmetros |
| :--- | :--- | :--- | :--- |
| 1 | SQLite | rusqlite (bundled, somente leitura) | `?N` |
| 1 | PostgreSQL | sqlx (pool por fingerprint) | `$N` |
| 1 | MySQL/MariaDB | sqlx (pool por fingerprint) | `?` |
| 1 | SQL Server | tiberius (TDS) | `@PN` |
| 2 | ODBC | odbc-api (feature `odbc`) | `?` |
| — | CSV / JSON | Polars + Polars SQL (`FROM data`) | literais escapados |
| — | REST | reqwest (Bearer/Basic, `jsonPath`) | URL-encoded |
| 3 | gRPC sidecar | contrato em `proto/` | — |

A introspecção usa `INFORMATION_SCHEMA` (PostgreSQL/MySQL/SQL Server), `sqlite_master` +
`pragma_table_info`/`pragma_foreign_key_list` (SQLite) e `SQLColumns` (ODBC).

## Frontend

* `domain/` espelha o `.nrep` em TypeScript e concentra a matemática mm↔px
  (`px = mm · DPI/25.4 · zoom`, DPI 96), garantindo 1 mm na tela = 1 mm no PDF a 100%.
* `state/reportStore` usa zustand + immer: snapshots imutáveis com compartilhamento estrutural,
  histórico ilimitado e agrupamento de edições contínuas (digitação vira um único passo de undo).
* `infra/api.ts` é o único ponto que conhece os comandos IPC.
* `features/designer`: Konva em 3 layers (bandas/grade/réguas, elementos, overlay com guias e
  transformer). A grade e as réguas são `Shape`s únicos (um draw call cada).

## Pendências conhecidas

* Cliente gRPC (Tier 3), ADBC, ClickHouse/DuckDB e cofre AES-256-GCM do Portal (Fase 2).
* Preview transfere o SVG de todas as páginas; para relatórios muito longos, buscar páginas sob demanda.
* `can_grow` usa estimativa de largura de caractere em vez de medição real do Typst.
* Sub-relatórios, gráficos e códigos de barras ainda não existem como elementos.
