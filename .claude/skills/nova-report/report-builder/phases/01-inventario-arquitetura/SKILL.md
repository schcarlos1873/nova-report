---
name: nova-report-phase-01-architecture
description: Mapear funcionalidades existentes, arquitetura, contratos, lacunas e riscos do NovaReport antes de mudanças que afetem múltiplas camadas ou o formato .nrep.
---

# Fase 1 — Inventário e arquitetura

## Missão

Produzir uma base factual para as fases seguintes, separando o que existe, o que está parcial, o que falha e o que é proposta. Não reescrever arquitetura funcional sem necessidade demonstrada.

## Escopo de análise

- `README.md`, `docs/ARCHITECTURE.md`, exemplos e testes.
- `apps/desktop/src` e `apps/desktop/src-tauri`.
- crates de domínio, aplicação, engine, conectores e CLI.
- Contratos `.nrep`, comandos IPC, tipos TypeScript e serialização Rust.
- Dependências e alinhamento de versões Tauri no npm, Cargo e lockfiles.

## Mapa funcional mínimo

Verificar o estado de: arquivos e histórico; canvas/bandas; elementos; seleção e edição em lote; propriedades do relatório/banda/elemento; expressões; fontes e queries; parâmetros; grupos; conexões e introspecção; validação; preview; exportação; CLI; credenciais; documentação e testes.

## Regras

1. Confirmar cada alegação por código, teste ou documentação identificável.
2. Não alterar arquivos `.nrep`, schema, dependências ou APIs durante um inventário sem escopo autorizado.
3. Identificar divergências entre README, implementação e testes.
4. Mapear cada mudança potencial às camadas afetadas e definir compatibilidade.
5. Tratar mensagens reportadas pelo usuário como hipóteses reproduzíveis, não como causa já comprovada.

## Saída esperada

Um inventário priorizado com: evidência/localização, estado (implementado/parcial/ausente/regressão), impacto no usuário, dependências, risco e critérios de aceite recomendados. Indicar explicitamente itens não verificados.
