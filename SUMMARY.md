No app, clique em Abrir e escolha examples/vendas_por_regiao.nrep. Ele já vem com um banco SQLite de exemplo (360 pedidos).

O que foi construído

A estrutura segue a Clean Architecture com DDD, em camadas:
- nova-domain: o formato .nrep e suas regras de validação, sem acesso a disco ou rede.
- nova-engine: o motor. Os dados viram tabelas Polars, as fórmulas rodam em Rhai isolado, as bandas são processadas com grupos aninhados e o Typst gera PDF e SVG.
- nova-application: os casos de uso (preview, exportação, conexões, documentos).
- nova-connectors: SQLite, PostgreSQL, MySQL, SQL Server, ODBC, CSV/JSON (com SQL em memória) e REST. Inclui navegação de schema estilo DBeaver; as senhas ficam no cofre do sistema operacional, nunca no .nrep.
- Desktop (Tauri 2 + React + Konva):
  - designer com bandas redimensionáveis, réguas em mm, grade e guias magnéticas;
  - arrastar elementos entre bandas, seleção por laço, desfazer/refazer ilimitado;
  - painéis de dados, parâmetros, grupos e conexões;
  - preview paginado ao vivo e exportação para PDF, Excel, CSV e Typst.

Como verifiquei

- PDF do exemplo: gerei e conferi visualmente. Grupos, subtotais, cores condicionais, "Página X de 9" e a tabela-resumo saem corretos, e o total geral bate com um SUM direto no SQLite (R$ 3.598.816,87).
- Designer: não consegui capturar a tela do app nativo (o ambiente não tem essa permissão). Inspecionei a interface no Chrome headless, com o backend simulado. Arrastar e redimensionar não foram testados com mouse.
- Desempenho (release): 97 páginas em ~40 ms com cache quente, dentro da meta de 50 ms para 50 páginas. O pico de memória foi 103 MB, dentro da meta de 150 MB.

Decisões tomadas sem perguntar

- Escopo: fiz só a Fase 1 (o app de desenvolvimento). O Portal da Fase 2 ficou fora.
- Tauri 2.x estável, como pede a spec, e não o 3.0, que ainda é alpha.
- Versões fixadas: o Polars mais novo exige Rust 1.95 e você tem o 1.94. Em vez de atualizar sua toolchain, fixei o Polars 0.53, com sqlx e rusqlite em versões compatíveis.
- Ordem do cabeçalho: na primeira página, o cabeçalho do relatório sai antes do cabeçalho de página, como no Crystal e no Jasper.

O que ainda falta

- Conectores gRPC (Tier 3): só o contrato existe, em proto/; o cliente não foi implementado.
- Não testados contra servidor real: os drivers de PostgreSQL, MySQL, SQL Server e ODBC compilam, mas só o SQLite rodou de verdade.
- Preview longo: o preview ainda gera e envia todas as páginas de uma vez. Para relatórios muito longos, o próximo passo é carregar as páginas sob demanda.
- "Pode crescer": a altura do texto é estimada, não medida.
- Elementos ausentes: sub-relatórios, gráficos e códigos de barras ainda não existem.

A pasta target/ ocupa 42 GB (builds debug e release do Polars e do Typst). cargo clean libera esse espaço quando quiser.

A arquitetura e as decisões estão detalhadas em [Architecture](docs/ARCHITECTURE.md), e as instruções de uso em [README](README.md).