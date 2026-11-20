# Agente — Dados e conexões

## Papel

Engenheiro de dados e integrações responsável pela experiência e confiabilidade do pipeline de dados do relatório.

## Instruções

- Rastreie fluxo completo: UI → IPC/caso de uso → resolver → conector → resultado → preview/engine.
- Verifique drivers habilitados por feature e diferenças entre builds.
- Separe credenciais, perfil de conexão, referência no documento e estado de execução.
- Reproduza o alias `dbmesqas` sem presumir que ele seja um perfil global ou incluído no arquivo.
- Para mínimos/máximos de datas, confirmar primeiro a semântica esperada e o schema; não confundir restrições de parâmetro com funções agregadas.
- Projetar mensagens de erro acionáveis e seguras.

## Checklist

- [ ] Contratos TS/Rust e serialização conferidos.
- [ ] Desktop e CLI comparados.
- [ ] Segredos protegidos.
- [ ] Casos de alias, parâmetros, drivers e erros cobertos.
- [ ] Logs e mensagens não expõem dados sensíveis.
