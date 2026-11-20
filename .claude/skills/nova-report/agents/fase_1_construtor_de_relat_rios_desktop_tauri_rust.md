# Spec de Engenharia: Construtor de Relatórios Desktop (Fase 1)
**Stack Tecnológica:** Tauri 2.0 | Rust | React + Canvas (Konva/Fabric) | Typst Engine | Rhai Expressions

---

## 1. Visão Geral da Arquitetura (Fase 1)

O **NovaReport Builder** é uma aplicação desktop nativa cross-platform de altíssimo desempenho. Ele resolve os problemas históricos do Crystal Reports e MS Report Builder (lentidão, travamentos de renderização, estouro de memória e visualizadores imprecisos).

### Principais Inovações Técnicas:
1. **Formato Declarativo (`.nrep`):** Especificação baseada em JSON/YAML estritamente tipado contendo layouts por bandas, fontes de dados, parâmetros e expressões.
2. **Motor de Layout Híbrido Rust + Typst:** Compilação de layouts em milissegundos utilizando Typst como biblioteca em Rust para geração nativa de PDF pixel-perfect.
3. **Canvas de Designer Banded (Frontend):** Interface moderna baseada em bandas redimensionáveis (*Report Header, Page Header, Group Header, Detail, Group Footer, Page Footer, Report Footer*) com réguas milimétricas, snap-to-grid e desfazer/refazer ilimitado.

---

## 2. Roster de Agentes Especializados (Fase 1)

### Agente 1: `Architect-Agent-Phase1`
* **Papel:** Arquiteto de Software Principal & Designer da DSL (`.nrep`).
* **Responsabilidade:** Definir o esquema JSON/YAML do relatório, contratos de IPC (Inter-Process Communication) entre Tauri Frontend e Rust Backend, e versionamento do formato de arquivo.
* **Instrução de Sistema:**
  > "Você é o Arquiteto-Chefe do NovaReport. Sua missão é garantir zero acoplamento entre o Designer Visual e o Engine de Renderização. Toda alteração no relatório deve ser serializada em um formato `.nrep` determinístico e imutável."

### Agente 2: `Rust-Engine-Agent`
* **Papel:** Engenheiro de Sistemas Rust.
* **Responsabilidade:** Desenvolver o parser do `.nrep`, pipeline de conexão de dados (SQLx/Tiberius), avaliador de fórmulas com Rhai e integração de compilação em PDF via Typst.
* **Instrução de Sistema:**
  > "Você é um especialista em Rust focado em baixa latência e segurança de memória. Nenhuma operação de I/O de banco ou compilação de PDF deve bloquear a Thread principal da UI. Use `tokio` para assincronismo e otimize alocações de memória."

### Agente 3: `UI-Canvas-Agent`
* **Papel:** Desenvolvedor Frontend Senior (Tauri + React + HTML5 Canvas).
* **Responsabilidade:** Construir o designer visual WYSIWYG, réguas interativas, alinhamento magnético, manipuladores de redimensionamento e painel de propriedades dinamicamente vinculado ao esquema Rust.
* **Instrução de Sistema:**
  > "Você constrói interfaces WYSIWYG intuitivas e responsivas. Garanta que 1 milímetro desenhado na tela corresponda exatamente a 1 milímetro no PDF gerado. O canvas deve suportar 10.000+ elementos sem perda de taxa de quadros (60 FPS)."

### Agente 4: `QA-Performance-Agent`
* **Papel:** Engenheiro de Testes de Carga e Fidelidade Tipográfica.
* **Responsabilidade:** Validar se a impressão física bate 100% com a visualização no editor, além de garantir consumo de RAM inferior a 150MB no desktop.

---

## 3. Catálogo de Skills (Fase 1)

---

### Skill 1.1: `DSL_Schema_Design`
* **Pertence a:** `Architect-Agent-Phase1`
* **Descrição:** Criação e manutenção do esquema estrito do arquivo `.nrep`.

#### Estrutura do Modelo de Dados (JSON Schema Exemplo):
```json
{
  "$schema": "https://novareport.io/schemas/v1/report.json",
  "meta": {
    "title": "Fatura de Vendas",
    "paperSize": "A4",
    "orientation": "portrait",
    "margins": { "top": 10, "bottom": 10, "left": 15, "right": 15 }
  },
  "dataSources": [
    {
      "id": "ds_main",
      "provider": "postgres",
      "connectionStringRef": "ENV_DB_CONN",
      "queries": [
        {
          "name": "Pedidos",
          "sql": "SELECT id, cliente, valor_total, data FROM pedidos WHERE data >= :data_inicio"
        }
      ]
    }
  ],
  "parameters": [
    { "name": "data_inicio", "type": "date", "label": "Data Inicial", "required": true }
  ],
  "bands": [
    {
      "type": "pageHeader",
      "height": 25,
      "elements": [
        {
          "id": "txt_title",
          "type": "label",
          "x": 0, "y": 5, "width": 180, "height": 10,
          "text": "RELATÓRIO DE VENDAS",
          "style": { "fontSize": 16, "bold": true, "align": "center" }
        }
      ]
    },
    {
      "type": "detail",
      "height": 8,
      "dataSource": "Pedidos",
      "elements": [
        {
          "id": "field_cliente",
          "type": "expression",
          "x": 0, "y": 0, "width": 100, "height": 8,
          "expression": "Fields.cliente",
          "style": { "fontSize": 10 }
        },
        {
          "id": "field_valor",
          "type": "expression",
          "x": 100, "y": 0, "width": 50, "height": 8,
          "expression": "Format.currency(Fields.valor_total, 'BRL')",
          "style": { "fontSize": 10, "align": "right" }
        }
      ]
    }
  ]
}
```

---

### Skill 1.2: `Typst_Layout_Compilation`
* **Pertence a:** `Rust-Engine-Agent`
* **Descrição:** Transforma a árvore de objetos do `.nrep` + Dados do Banco em código Typst intermediário para compilação instantânea em PDF.

#### Exemplo do Algoritmo Rust:
```rust
use typst::eval::Tracer;
use typst::model::Document;

pub struct ReportCompiler;

impl ReportCompiler {
    pub fn compile_to_pdf(template: &ReportTemplate, data: &DataSet) -> Result<Vec<u8>, EngineError> {
        // 1. Converte a estrutura .nrep para código Markup Typst em memória
        let typst_code = Self::generate_typst_markup(template, data)?;
        
        // 2. Compila a string via Typst Compiler crate
        let world = NovaTypstWorld::new(typst_code);
        let mut tracer = Tracer::new();
        let document = typst::compile(&world, &mut tracer)
            .map_err(|e| EngineError::CompilationFailed(format!("{:?}", e)))?;

        // 3. Exporta para Bytes PDF
        let pdf_bytes = typst_pdf::pdf(&document, Smart::Auto, None);
        Ok(pdf_bytes)
    }

    fn generate_typst_markup(template: &ReportTemplate, data: &DataSet) -> Result<String, EngineError> {
        let mut markup = String::new();
        markup.push_str("#set page(paper: \"a4\", margin: (top: 10mm, bottom: 10mm, left: 15mm, right: 15mm))\n");
        // Injeção de fontes e bandas
        // ...
        Ok(markup)
    }
}
```

---

### Skill 1.3: `Banded_Canvas_Manipulation`
* **Pertence a:** `UI-Canvas-Agent`
* **Descrição:** Lógica de interface React/Canvas para drag-and-drop de elementos respeitando os limites das bandas.

#### Regras Operacionais da Skill:
1. **Controle de Limites:** Se o usuário arrastar um elemento do *Page Header* para o *Detail*, a propriedade `parentBand` do elemento é atualizada dinamicamente.
2. **Guias Inteligentes:** Exibir linhas azuis magnéticas ao alinhar margens de elementos com outros objetos adjacentes.
3. **Escala Milimétrica:** Converter pixels da tela para milímetros considerando o DPI e o fator de Zoom atual (`1px = (25.4 / DPI) * zoom`).

---

### Skill 1.4: `Expression_Parser_Integration`
* **Pertence a:** `Rust-Engine-Agent`
* **Descrição:** Execução segura de expressões e agrupamentos utilizando `Rhai`.

#### Funções Embutidas Padrão no Motor Rhai:
* `SUM("field_name")`: Acumula valores do grupo atual.
* `COUNT()`: Retorna o número de linhas processadas.
* `FORMAT_DATE(val, fmt)`: Formata datas ISO para padrões locais.
* `IFF(condition, true_val, false_val)`: Operador condicional para visibilidade e cores dinâmicas.

---

## 4. Matriz de Qualidade & Critérios de Aceite (Fase 1)

| Funcionalidade | Critério Aceite | Métrica de Sucesso |
| :--- | :--- | :--- |
| **Tempo de Preview** | Alteração no layout ou parâmetro refletida na tela | $< 50ms$ para relatórios até 50 páginas |
| **Fidelidade WYSIWYG** | Medição da margem impressa vs desenhada | Variação $< 0.1mm$ |
| **Consumo de Memória** | Edição e renderização contínua | $< 150 MB$ RAM consumida |
| **Exportação NFM** | PDF, XLSX, CSV nativos gerados | Preservação de fórmulas e dados numéricos |
