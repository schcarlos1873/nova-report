## O que mudou em cada banco

┌────────────────────┬─────────────────────────────────────────────┬──────────────────────────────────┬────────────────────────────────────┐
│       Banco                        │            Reaproveitar conexões                                              │        Instância nomeada                                 │            Diagnóstico                                            │
├────────────────────┼─────────────────────────────────────────────┼──────────────────────────────────┼────────────────────────────────────┤
│ SQL Server                      │ Novo pool                                                                           │ Novo, HOST\INSTANCIA                           │ Etapas completas                                               │
├────────────────────┼─────────────────────────────────────────────┼──────────────────────────────────┼────────────────────────────────────┤
│ PostgreSQL / MySQL      │ Já tinham pool; agora renovam conexões                             │ Não existe nesses bancos                              │ Etapas completas                                              │
│                                         │ paradas                                                                                │                                                                     │                                                                          │
├────────────────────┼─────────────────────────────────────────────┼──────────────────────────────────┼────────────────────────────────────┤
│ REST                               │ O cliente HTTP já reaproveita conexões                              │ —                                                                 │ Endereço chamado, código HTTP e                   │
│                                         │                                                                                             │                                                                      │ tempo                                                                │
├────────────────────┼─────────────────────────────────────────────┼──────────────────────────────────┼────────────────────────────────────┤
│ SQLite / CSV /                 │ Não precisa (arquivo local)                                                 │ —                                                                  │ Resultado final                                                 │
│ JSON                               │                                                                                            │                                                                      │                                                                          │
├────────────────────┼─────────────────────────────────────────────┼──────────────────────────────────┼────────────────────────────────────┤
│ ODBC                             │ Não ativei (motivo abaixo)                                                  │ Resolvida pelo próprio driver                        │ Resultado final                                                  │
│                                         │                                                                                            │ ODBC                                                           │                                                                          │
└────────────────────┴─────────────────────────────────────────────┴──────────────────────────────────┴────────────────────────────────────┘

## 1. Reaproveitar conexões
- SQL Server: guarda até 4 conexões paradas por perfil.
  - Uma conexão parada há mais de 30 s é conferida com SELECT 1 antes de ser usada.
  - Depois de 5 min parada, é descartada, porque firewalls costumam derrubar conexões ociosas sem avisar.
  - O botão "Testar" sempre abre uma conexão nova, para testar a rede de verdade.
- Uma regra que você precisa conhecer: a conexão só volta ao pool se a consulta não altera a sessão. Consultas com tabela temporária (#), SET, USE ou transação usam uma conexão própria, que é fechada no fim. Isso evita, por exemplo, um #tmp de um relatório quebrar o próximo.
- PostgreSQL / MySQL: conexões paradas há 5 min são renovadas, e nenhuma conexão dura mais de 30 min.

## 2. Instâncias nomeadas no SQL Server
- Aceita SERVIDOR\INSTANCIA, tcp:host\inst,porta e .\SQLEXPRESS, tanto no formulário quanto na connection string.
- Escrevi a consulta ao serviço SQL Server Browser (porta UDP 1434) em vez de usar a feature do tiberius. A do tiberius tinha o mesmo defeito do problema original: tentava o fe80:: e não respeitava o timeout.
- Se você informar uma porta diferente de 1433, ela é usada direto, sem consultar o Browser.
- Os erros vêm com dica: Browser parado ou porta UDP bloqueada, instância sem TCP/IP habilitado, nome de instância errado.

## 3. Modo diagnóstico
- Novo botão Diagnóstico na tela de conexão. Ele abre uma janela com cada etapa e o tempo de cada uma, e um botão Copiar para mandar ao suporte.
- A senha e a connection string nunca entram no relatório.

Exemplo real deste Linux (enviei uma senha errada de propósito):
[  203 ms] DNS respondeu: fe80::22e8:d4ff:feb6:750c, 92.112.179.43
[  203 ms] ⚠ Descartado fe80::22e8:… (IPv6 link-local/inválido vindo do DNS)
[  217 ms] ✔ TCP conectado em 92.112.179.43:1433 (13 ms)
[  295 ms] ✖ Login recusado para o usuário 'dbmesqas'.
Com servidor\INSTX, o resultado foi "SQL Server Browser não respondeu" após 2 s, com a dica de liberar a porta UDP 1434.

Por que não ativei o pool no ODBC: o pool do ODBC é global para todo o programa. Ele exige que o driver de cada banco aceite ser usado por várias threads, e o app abre as conexões ODBC em threads diferentes. Drivers antigos (Firebird, Oracle mais velho) podem travar ou corromper dados nessa situação. Dá para ligar como opção desativada por padrão, se quiser.
Falta testar pela interface: rode npm run tauri dev e use Diagnóstico e Testar na conexão dbmesqas, com a senha guardada no KWallet. Teste também abrir um relatório e gerar o preview, que é o caminho que passa pelo pool.
O driver SQL Server virou uma pasta: crates/nova-connectors/src/drivers/mssql/, com mod.rs, browser.rs e pool.rs. O diagnóstico está em crates/nova-connectors/src/diag.rs.