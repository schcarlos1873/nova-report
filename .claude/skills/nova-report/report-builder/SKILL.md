---
name: report-builder
description: Coordenar análise, especificação, UX, implementação e validação das funcionalidades do NovaReport Builder desktop. Usar ao evoluir o designer, dados, conexões, propriedades, preview, exportação ou corrigir regressões do projeto.
---

# NovaReport Builder — skill coordenadora

## Objetivo

Evoluir o NovaReport Builder existente com segurança, mantendo compatibilidade entre `.nrep`, desktop Tauri, CLI e engine Rust. Este conjunto descreve as fases de trabalho e os agentes especializados. A existência de uma capacidade nesta documentação não significa que ela esteja completa: verificar o código e os testes antes de propor ou implementar mudanças.

## Contexto técnico confirmado

- Desktop: Tauri 2, React/TypeScript, Vite, Zustand/Immer e React-Konva.
- Backend: workspace Rust com `nova-domain`, `nova-engine`, `nova-application`, `nova-connectors` e `nova-cli`.
- Formato declarativo `.nrep`; renderização via Typst; dados com Polars; expressões Rhai.
- Interface atual: barra superior, painéis laterais Elementos/Dados/Conexões, canvas central por bandas e painel de propriedades.
- Recursos existentes incluem elementos de texto, expressão, imagem, linha, retângulo e tabela; bandas; fontes e queries; parâmetros e grupos; conectores; validação; preview paginado; PDF, XLSX, CSV e Typst.
- Confirmar versões compatíveis de pacotes Tauri npm e crates Rust antes de alterar dependências.

## Como conduzir o trabalho

1. Identificar a fase principal e ler seu `SKILL.md` e `AGENT.md`.
2. Inspecionar a implementação e os testes atuais. Não inferir ausência de funcionalidade apenas pela tela.
3. Declarar escopo, contratos afetados, riscos de compatibilidade e critérios de aceite antes de mudanças amplas.
4. Fazer alterações pequenas, preservando o formato `.nrep`, as camadas da arquitetura e o comportamento da CLI, salvo decisão explícita em contrário.
5. Validar com os testes/builds pertinentes e relatar o que foi executado e o que não foi.
6. Se uma tarefa atravessar fases, coordenar as fases em ordem: fundação e contratos → UX/UI → editor → dados/conexões → preview/exportação → qualidade.

## Fases

| Fase | Skill | Agente | Escopo |
|---|---|---|---|
| 1 | `phases/01-inventario-arquitetura/SKILL.md` | `phases/01-inventario-arquitetura/AGENT.md` | Inventário, contratos, dependências e planejamento |
| 2 | `phases/02-ux-ribbon/SKILL.md` | `phases/02-ux-ribbon/AGENT.md` | Arquitetura da informação, Ribbon e produtividade |
| 3 | `phases/03-designer-propriedades/SKILL.md` | `phases/03-designer-propriedades/AGENT.md` | Canvas, bandas, seleção múltipla e propriedades em comum |
| 4 | `phases/04-dados-conexoes/SKILL.md` | `phases/04-dados-conexoes/AGENT.md` | Fontes, consultas, parâmetros, grupos e conectores |
| 5 | `phases/05-preview-render-export/SKILL.md` | `phases/05-preview-render-export/AGENT.md` | Preview, renderização, paginação e exportadores |
| 6 | `phases/06-integracao-qualidade/SKILL.md` | `phases/06-integracao-qualidade/AGENT.md` | Regressão, integração Tauri/CLI, acessibilidade e qualidade |

## Princípios transversais

- **UX de produtividade:** privilegiar descoberta, previsibilidade, baixo número de ações, atalhos, feedback imediato e recuperação segura de erros.
- **Ribbon opcional e contextual:** organizar comandos por tarefa sem esconder fluxos atuais nem reduzir área útil do canvas desnecessariamente.
- **Seleção múltipla:** propriedades compartilhadas devem permitir edição em lote; valores divergentes devem aparecer em estado misto, sem sobrescrever silenciosamente valores não editados.
- **Compatibilidade:** evitar migrações incompatíveis no `.nrep`; quando inevitáveis, definir versão, conversão e testes.
- **Segurança:** credenciais não devem ser persistidas em texto aberto no `.nrep`; expressões e conteúdo de usuário devem continuar tratados conforme as proteções existentes.
- **Acessibilidade:** estados compreensíveis sem depender apenas de cor, navegação por teclado, foco visível, rótulos e dicas claras.
- **Idioma:** interface e documentação funcional em português brasileiro, mantendo nomes de código e termos técnicos quando necessário.
- **Não prometer:** documentar lacunas e suposições; não apresentar item planejado como implementado.

## Regressões reportadas a incluir na validação

1. `cargo tauri dev` acusa versões incompatíveis entre pacotes Tauri npm e crates Rust. Conferir todos os pacotes envolvidos e o lockfile, não apenas silenciar a mensagem.
2. CLI falha ao renderizar `examples/prod_reel.nrep` com `conexão 'dbmesqas' não encontrada`. Rastrear resolução de alias, configuração e cofre compartilhado; não inventar credenciais nem substituir conexão sem confirmação.
3. Valores mínimo/máximo de parâmetros `date` no relatório de exemplo não funcionam. Distinguir limites de validação de parâmetro de agregações `MIN`/`MAX`; reproduzir, localizar a camada e testar datas, limites e formatos.

## Critério para encerrar uma fase

Entregar resumo do escopo realizado, arquivos alterados, testes executados, limitações conhecidas e decisões pendentes. Mudanças de interface devem indicar o impacto em atalhos, painel de propriedades, canvas e acessibilidade. Mudanças de dados devem incluir impacto em `.nrep`, engine e CLI.
