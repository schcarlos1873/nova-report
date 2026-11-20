---
name: nova-report-phase-05-preview-render-export
description: Evoluir preview paginado, processamento de bandas, expressões e exportação PDF/XLSX/CSV/Typst mantendo paridade entre desktop e CLI.
---

# Fase 5 — Preview, renderização e exportação

## Pipeline a preservar e verificar

Resolução de parâmetros e conexões → execução de provedores → contexto de dados → processamento de layout/bandas e expressões Rhai → modelo renderizado → transpilação/compilação Typst → SVG por página, PDF ou exportadores tabulares.

## Fluxos de usuário

- Inserir e aplicar parâmetros antes de renderizar.
- Navegar páginas, ajustar zoom e atualizar dados ignorando cache quando solicitado.
- Exibir duração, avisos e falhas com contexto útil.
- Exportar para PDF, XLSX, CSV e Typst com nome/extensão coerentes.
- Preservar paridade de engine com CLI e evitar diferenças silenciosas entre preview e arquivo final.

## Regressões

- Reproduzir a geração de `examples/prod_reel.nrep` para PDF via `cargo run --release -p nova-cli -- render ...`; separar falha de resolução da conexão de falha de renderização.
- Validar campos de data e limites mínimo/máximo onde aplicáveis, incluindo valores ausentes e fronteiras.
- Não trocar o engine usado pelo preview por uma implementação divergente do caminho de exportação.

## Qualidade

Validar paginação, cabeçalhos/rodapés, quebras de tabela, totais, imagens, expressões, fontes, precisão geométrica, escapamento seguro de conteúdo, cache e relatórios longos. Mensurar desempenho com critérios documentados; otimizar apenas com evidências.
