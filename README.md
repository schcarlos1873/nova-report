# NovaReport Builder

Construtor de relatórios desktop (Fase 1) — **Rust + Tauri 2 + React/Konva + Typst + Polars + Rhai**.
Designer WYSIWYG por bandas, formato declarativo `.nrep`, múltiplos bancos de dados e
geração de PDF pixel-perfect em memória.

Arquitetura e decisões: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Início rápido

Pré-requisitos: Rust ≥ 1.85 (testado em 1.94), Node ≥ 20, `cargo install tauri-cli --version ^2`.

```bash
# app desktop (designer)
cd apps/desktop
npm install
cargo tauri dev            # abre o NovaReport Builder

# CLI headless (mesmo engine)
cargo run --release -p nova-cli -- render examples/vendas_por_regiao.nrep -o vendas.pdf
cargo run --release -p nova-cli -- render examples/vendas_por_regiao.nrep -o vendas.xlsx
cargo run --release -p nova-cli -- render examples/vendas_por_regiao.nrep -p data_fim=2024-06-30
cargo run --release -p nova-cli -- validate examples/vendas_por_regiao.nrep
cargo run --release -p nova-cli -- drivers

# testes
cargo test --workspace
cargo test -p nova-engine --release -- --ignored --nocapture perf_   # critério de desempenho
```

No app, use **Abrir** e escolha `examples/vendas_por_regiao.nrep` (SQLite incluso em `examples/vendas.db`).

Para desenvolver só a interface no navegador, sem o backend Rust: `cd apps/desktop && npx vite`
e abra `http://localhost:1420/?demo` (IPC simulado por `src/infra/devMock.ts`, só em dev).

## Funcionalidades

| Área | Implementado |
| :--- | :--- |
| **Designer** | Bandas (report/page/group header-footer, detail) redimensionáveis; réguas em mm; grade e encaixe; guias magnéticas; arrastar entre bandas (reparenting); seleção por laço e múltipla; alinhar/distribuir; desfazer/refazer ilimitado; copiar/colar/duplicar; drag-and-drop de campos |
| **Elementos** | Texto (com `{PAGE}`/`{PAGES}`), expressão com formato, imagem (arquivo/data URI/SVG), linha, retângulo, tabela com quebra de página, cabeçalho repetido, zebra e rodapé com totais |
| **Expressões (Rhai, sandbox)** | `Fields.x`, `Params.x`, `Group.Sum_x`; `SUM/AVG/MIN/MAX/COUNT/DISTINCT_COUNT/RUNNING_SUM`, `IIF`, `FORMAT_DATE`, `Format.currency/number/percent/date`, `PAGE_NUMBER/TOTAL_PAGES`... Visibilidade e cores condicionais |
| **Dados** | Grupos aninhados com subtotais, parâmetros tipados (com defaults `today`, `firstDayOfMonth`), múltiplas queries por relatório, cache de dados no preview |
| **Conectores** | SQLite, PostgreSQL, MySQL/MariaDB, SQL Server, ODBC (Oracle, DB2, HANA... — feature `odbc`), CSV/JSON com SQL em memória (Polars), REST/JSON |
| **Conexões** | Navegador de schema estilo DBeaver (tabelas, views, colunas, PK/FK, procedures), pré-visualização de dados, senha no cofre do SO (Keychain / Credential Manager / Secret Service) — o `.nrep` nunca guarda senhas |
| **Saída** | Preview paginado (SVG) ao vivo, PDF vetorial (Typst), XLSX com tipos nativos e fórmulas `SUM`, CSV, código Typst |

## Estrutura

```
crates/
  nova-domain        Agregado .nrep, value objects, invariantes, ports (sem I/O)
  nova-engine        DataContext (Polars), FormulaEngine (Rhai), bandas, transpilador Typst, exportadores
  nova-application   Casos de uso: render/preview/export, conexões, documentos
  nova-connectors    Infraestrutura: drivers, dialetos SQL, introspecção, cofre, repositórios
  nova-cli           Composição headless (CI, servidores, testes)
apps/desktop
  src-tauri          Composition root + comandos IPC
  src                React: designer (Konva), painéis, preview
examples/            vendas.db + vendas_por_regiao.nrep
proto/               Contrato gRPC para conectores sidecar (Tier 3)
docs/ARCHITECTURE.md Decisões de arquitetura
```

## Desempenho medido (MacBook, release)

| Critério (spec Fase 1) | Meta | Medido |
| :--- | :--- | :--- |
| Preview com caches quentes | < 50 ms até 50 páginas | ~40 ms para 97 páginas (layout 8 ms + Typst 25 ms) |
| Execução completa via CLI (processo frio) | — | ~75 ms para 9 páginas / 360 linhas |
| Memória | < 150 MB | 103 MB de pico (CLI com fontes do sistema) |
