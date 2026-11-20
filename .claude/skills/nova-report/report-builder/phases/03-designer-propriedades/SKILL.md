---
name: nova-report-phase-03-designer-properties
description: Evoluir o designer visual, canvas por bandas, seleção, manipulação de elementos e propriedades individuais ou compartilhadas do NovaReport.
---

# Fase 3 — Designer, bandas e propriedades

## Capacidades a preservar e verificar

Canvas Konva, bandas redimensionáveis, réguas em milímetros, zoom, grade/encaixe, guias, seleção por laço e múltipla, movimentação entre bandas, undo/redo, copiar/colar/duplicar e atalhos. Elementos atuais: texto, expressão, imagem, linha, retângulo e tabela.

## Propriedades

O painel individual atual cobre posição/tamanho, estilo, aparência, expressões condicionais e propriedades específicas por tipo. Em seleção múltipla, há alinhamento, distribuição e uniformização de dimensões; propriedades de estilo comuns ainda devem ser avaliadas para edição em lote.

## Requisitos para edição múltipla

- Aplicar somente propriedades compatíveis com todos os selecionados.
- Representar valores distintos como estado misto (não escolher arbitrariamente o primeiro valor).
- Não atualizar uma propriedade até o usuário fazer uma escolha explícita.
- Atualização em lote deve ser uma única operação coerente de undo/redo.
- Preservar propriedades que não foram editadas e não suportar alterações parciais silenciosas.
- Oferecer formatação compartilhada quando aplicável: família/tamanho de fonte, estilo, cor, fundo, borda, alinhamento e espaçamento.
- Para tabela e outros elementos com subestruturas distintas, oferecer apenas propriedades semanticamente seguras.
- Permitir mesclas colunas da tabela, mas não mesclar linhas de diferentes bandas.
- Permitir para os valores de detalhes.

## Requisitos do canvas

Manter precisão mm↔px e coerência com PDF; preservar snap, guias, limites de página/banda e comportamento de arraste. Melhorias de UX devem informar destino e contexto, sem degradar seleção, foco ou desempenho.

## Testes e aceite

Testar seleção homogênea e heterogênea; estados mistos; uma edição seguida de undo/redo; elementos em bandas distintas; limites geométricos; zoom; atalhos; serialização e render final. Nenhuma alteração deve corromper o `.nrep` ou gerar diferença inesperada na saída.
