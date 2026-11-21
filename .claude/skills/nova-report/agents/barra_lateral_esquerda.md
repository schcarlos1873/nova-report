# Melhorias de UX

# Barra lateral esquerda
- Permitir redimensionar.
- Adicionar title `Dados do Relatório` e no canto direito um botão para ocultar `x` o paneil, em exibir já existe a opção para mostrar novamente as opções
- Utilize Treeview para exibir as opções
- Utilize menu de contexto para mostrar as opções para, Adicionar, Remover, Editar, Propriedades conform o nó da treeview.
- Implemente drag drop dos elementos que podem ser movidos para o relatório.
- `Campos Internos (Internal Field)`: crie atalhos para as funções TODAY(),NOW(),RECORD_NUMBER(),PAGE_NUMBER()  ,TOTAL_PAGES(),Report.Title. Sugire outras que possam ser adicionadas por exemplo Reporte.Filename entre outras. Deve permitir arrastar para o relatório, não pode ser excluido, alterado, criado.
- `Parametros (Parameters)`: Adiciione contexto para para adicionar novos parametros, e abra um dialog para que o mesmos seja configurado. Para cada parametro é permitido `Remover` e `Propriedade do Paramametro`, ajuste as funcionalidades existentes de parametros no novo dialog. Deve permitir arrastar o parametro para ser visto no relatório.
- `Imagens`: Mostra as imagens adicionada e deve permitir através de um menu de contexto adicionar imagems. Abre um , onde o usuário pode escolher se o endereço é local, url, embed ou atravez do banco de dados.
- `Fonte de Dados (Data Sources)`: adicione um menu de contexto `Adicionar Fonte de Dados`, quando clicado abre um dialog para a criação.
- `Conjunto de dados (dataset)`: adicione um menu de contexto `Adicionar Conjunto de Dados`, quando clicado abre um dialog para a configuração do conjunto de dados. Adicione um radio button `Texto` ou `Stored Procedure`. Para o texto mantenha o existente no Dialog. Para a `Stored Procedure`, exibir um input para a entrada do nome da procedure, adicione um bottão ao lado para ser adicionado o mapeamenteo entre o nome da procedure no do parametro. exemplo:

Nome Procedure: [get_new_orders         ]  `+ Parâmetros ` 

Ao clicar em `+ Parâmetros ` abre o dialog abaixo

Definir Paramtros da Procedure

|Nome Parametro | Valor Parametro  | Passar Valor Nulo |  
|---------------|------------------|-------------------|
|@param1        | :param1          |       false       |
|@param2        | :param2          |       false       |
|@param3        | :param3          |       true        |

                                    `Ok`     `Cancelar`
Botão ok salva e fecha o dialog, cancelar somente fecha o dialog sem salvar.

## Estrutura da Treeview

```
-------------------------------
| Dados do relatório        x |
-------------------------------
| - Campos Internos           |
|   TODAY()                   |
|   NOW()                     |
|   RECORD_NUMBER()           |
|   PAGE_NUMBER()             |
|   TOTAL_PAGES()             |
|   Report.Title              |
| - Parametros                |
|   data_inicio               |
|   data_fim                  |
|   data_inicio               |
|   regioes                   |
|   clientes                  |
| - Imagens                   |
|   logo(svg)                 |
| - pslogo(jpg)               |
| - Fonte de Dados            |
|   ds_vendas                 |
|   ds_faturamento            |
|   ds_producao               |
| - Conjunto de dados         |
|   - Pedidos                 |
|      regiao                 |
|      cliente                |
|      cidade                 | 
|      pedido                 | 
|      data                   |
|      produto                |  
|      quantidade             |  
|      valor_total            |   
|      status                 | 
|   + ResumoProdutos          |
|   + LkRegioes               |
|   + LkClientes              |
-------------------------------
```