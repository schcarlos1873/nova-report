---
name: nova-report-phase-06-quality
description: Planejar e validar testes de regressão, integração Tauri, acessibilidade, compatibilidade e qualidade de ponta a ponta do NovaReport Builder.
---

# Fase 6 — Integração, regressão e qualidade

## Matriz mínima de verificação

- Rust: `cargo test --workspace`; testes específicos dos crates tocados.
- Desktop: typecheck/build npm e execução Tauri quando o ambiente permitir.
- Tauri: versões npm/crates alinhadas e comandos `cargo tauri dev`/build sem incompatibilidade.
- CLI: validate, render e drivers com exemplos seguros.
- UX: mouse e teclado, foco visível, atalhos, seleção múltipla, estados de erro/vazio/carregamento.
- Documento: abrir, editar, salvar, reabrir e validar `.nrep` sem perda de dados.
- Exportação: preview e PDF/XLSX/CSV/Typst para relatórios de teste.

## Regressões obrigatórias já reportadas

1. Erro de versões Tauri incompatíveis no `cargo tauri dev`.
2. Alias `dbmesqas` não encontrado ao renderizar `examples/prod_reel.nrep`.
3. Mínimo/máximo de `date` sem efeito no relatório de exemplo.
4. Propriedades compartilhadas indisponíveis ou incompletas em seleção múltipla.

Registrar para cada item: ambiente, passos, resultado esperado/obtido, causa confirmada ou desconhecida, teste que previne recorrência.

## Acessibilidade e UX

Verificar navegação por teclado, nomes acessíveis, contraste, foco, tooltips não exclusivos, estados mistos, feedback não dependente só de cor e comportamento em diferentes tamanhos de janela. Validar a Ribbon sem sacrificar espaço do canvas ou ocultar comandos frequentes.

## Critério de encerramento

Não afirmar que uma correção foi validada sem executar o teste correspondente. Declarar bloqueios de ambiente e testes não executados. Não alterar fixtures, exemplos ou dependências apenas para fazer um teste passar sem explicar o motivo e o impacto.
