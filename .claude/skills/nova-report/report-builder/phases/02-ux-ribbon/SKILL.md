---
name: nova-report-phase-02-ux-ribbon
description: Projetar e evoluir a experiência do NovaReport Builder, incluindo Ribbon inspirada nas referências visuais, organização de comandos e fluxos de produtividade.
---

# Fase 2 — UX, arquitetura da informação e Ribbon

## Referências

As imagens `screen_1.png` a `screen_4.png` mostram o Microsoft Report Builder: menu Arquivo, abas de Ribbon, grupos de comandos, painel lateral de dados, canvas com réguas e região de agrupamento. Usar como referência de organização, não como obrigação de copiar aparência ou funcionalidades que não existem no produto.

## Objetivo de experiência

Reduzir tempo para criar, ajustar, validar e exportar relatórios. Tornar ações frequentes descobríveis sem sobrecarregar a área de edição. Manter coerência entre Ribbon, atalhos, menus contextuais, painéis e seleção atual.

## Estrutura candidata da Ribbon

- **Arquivo:** novo, abrir, recentes, salvar, salvar como e estado de alterações.
- **Página Inicial:** desfazer/refazer, copiar/colar/duplicar, fonte, estilo, parágrafo, bordas e alinhamento.
- **Inserir:** texto, expressão/campo, tabela, matriz, gráficos, imagem, linha, retângulo e bandas.
- **Dados:** fontes, conexões, consultas, parâmetros, grupos, executar e atualizar campos.
- **Layout:** papel, orientação, margens, bandas, dimensões, posição e distribuição.
- **Exibir:** painéis, réguas, grade, encaixe, zoom e alternância Design/Preview.

Esta é uma proposta inicial; validar nomes, comandos e hierarquia contra os fluxos reais antes de implementar. Comandos dependentes de contexto devem aparecer habilitados apenas quando aplicáveis, com explicação acessível.

## Princípios de interação

- Ações frequentes em primeiro plano; avançadas agrupadas sem perder acesso.
- Preservar espaço útil para o canvas; permitir recolher/expandir Ribbon e painéis quando viável.
- Mostrar documento atual, salvamento pendente e validação sem depender somente de cor.
- Formatação da seleção deve funcionar para um item ou para múltiplos itens compatíveis.
- Estado misto em propriedades divergentes; alteração explícita aplica o novo valor a todos.
- Evitar comandos duplicados com semânticas diferentes entre Ribbon, painéis e menus.
- Manter atalhos existentes e apresentar atalhos junto às ações.
- Projetar navegação por teclado, foco, dicas, tooltips e escalas de janela.
- Preservar um caminho direto para iniciantes e eficiência para usuários recorrentes.
- Exibir propriedades do objeto selecionaod ou se houver mais mostre as comuns aplicáveis a todos; se não houver, não mostrar propriedades do container.

## Processo

1. Mapear tarefas e frequência, dificuldades atuais e comandos existentes.
2. Especificar hierarquia, estado/contexto, habilitação e feedback de cada comando.
3. Avaliar densidade visual, responsividade desktop, acessibilidade e área do canvas.
4. Validar com tarefas representativas: criar relatório, inserir campo, formatar seleção múltipla, configurar dados, validar e exportar.
5. Só então decompor a implementação em mudanças incrementais.

## Aceite

Toda ação da Ribbon tem comportamento definido e origem de estado clara; nenhum recurso existente fica inacessível; comandos indisponíveis explicam o motivo; tarefas-chave têm fluxo verificável por teclado e mouse.
