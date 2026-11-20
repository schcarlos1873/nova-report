---
name: nova-report-phase-04-data-connections
description: Trabalhar em fontes de dados, consultas, parâmetros tipados, grupos, conexões, introspecção e resolução segura de aliases no NovaReport.
---

# Fase 4 — Dados, parâmetros e conexões

## Funcionalidades existentes a verificar

Fontes com queries SQL; drivers SQLite, PostgreSQL, MySQL/MariaDB, SQL Server, ODBC opcional, CSV/JSON e REST; listagem e teste de conexões; introspecção de schema; descoberta e preview de campos/dados; parâmetros tipados; grupos e subtotais; cache de preview. Verificar estado real e cobertura de cada driver.

## Regras de domínio e UX

- Consultas devem ter estados claros de execução, erro, duração, linhas e colunas.
- Parâmetros devem respeitar tipo, obrigatoriedade, defaults e conversão consistente entre UI, engine e CLI. Na versão desktop, deve permitir associar uma tabela com valores key,label para seleção; na CLI, aceitar lista de valores separados por vírgula. Na versão desktop deve permitir aninhamento de parâmetros, mas não permitir que um parâmetro dependa de outro que ainda não tenha valor definido.
- Tratar limites de parâmetros (`min`/`max`) como validação distinta de agregações de expressão `MIN`/`MAX`; primeiro confirmar suporte no modelo e no código.
- Resolver aliases explicitamente; mensagens de conexão ausente devem indicar alias e formas de corrigir, sem revelar segredos.
- Segredos permanecem no cofre do SO ou mecanismo já previsto; não gravar senha em texto aberto no `.nrep`.
- Queries e parâmetros precisam manter comportamento equivalente no desktop e CLI.

## Caso de regressão obrigatório

Investigar `conexão 'dbmesqas' não encontrada` ao renderizar `examples/prod_reel.nrep` pela CLI. Conferir referência no relatório, repositório de conexões, cofre/configuração do ambiente e composição CLI. Não criar conexão ou alterar o exemplo para mascarar a falha.

## Testes e aceite

Cobrir alias encontrado/ausente, cada tipo de parâmetro relevante, limites de datas, valores padrão, parsing/serialização, consulta parametrizada, erros de driver e diferenças desktop/CLI. Não registrar credenciais em logs ou snapshots.
