# Spec de Engenharia: Portal de Relatórios Enterprise (Fase 2)
**Stack Tecnológica:** Rust (Axum / Tokio) | PostgreSQL | React Query + Tailwind | S3 / MinIO | Distributed Worker Pool

---

## 1. Visão Geral da Arquitetura (Fase 2)

O **NovaReport Server/Portal** é a plataforma web centralizada para publicação, gerenciamento, execução agendada e controle de acesso a relatórios criados no **NovaReport Builder (Fase 1)**.

### Diferenciais Competitivos sobre o SSRS:
* **Arquitetura Stateless & Cloud-Native:** Executável via Docker/Kubernetes em instâncias leves, sem dependência de Windows Server.
* **Streaming de Relatórios:** Renderização de relatórios de grande porte (100.000+ linhas) via streaming diretamente para a Web/PDF sem sobrecarregar a memória do servidor.
* **Row-Level Security (RLS) Nativo:** Injeção de contextos de segurança do usuário logado diretamente nas queries SQL executadas.
* **Webhooks & Subscrições Modernas:** Envio de relatórios por E-mail, Slack, Teams, S3 ou Webhooks configuráveis em formato Cron.

---

## 2. Roster de Agentes Especializados (Fase 2)

### Agente 1: `Portal-Architect-Agent`
* **Papel:** Arquiteto de Software para Sistemas Distribuídos e Multi-tenant.
* **Responsabilidade:** Projetar as APIs REST/gRPC do Portal, catálogo de pastas, controle de versão dos relatórios e isolamento de tenants.

### Agente 2: `Server-Execution-Agent`
* **Papel:** Engenheiro Backend Rust (High-Throughput Services).
* **Responsabilidade:** Implementar a fila de execução de relatórios (Job Queue), isolamento de processos de renderização e exportadores multi-formato (Excel nativo via `rust_xlsxwriter`, PDF via Typst headless).

### Agente 3: `Portal-Frontend-Agent`
* **Papel:** Engenheiro Web Frontend (React / TypeScript).
* **Responsabilidade:** Desenvolver a interface do Portal no estilo SSRS modernizado: visualizador interativo com barra de parâmetros, árvore de pastas, gerenciador de agendamentos e histórico de execuções.

### Agente 4: `Security-Scheduler-Agent`
* **Papel:** Especialista em Cibersegurança & Automação Cron.
* **Responsabilidade:** Garantir autenticação OIDC/OAuth2/SAML, controle fino de permissões (RBAC) e motor de agendamentos distribuídos sem conflitos (*race conditions*).

---

## 3. Catálogo de Skills (Fase 2)

---

### Skill 2.1: `Report_Catalog_And_Versioning`
* **Pertence a:** `Portal-Architect-Agent`
* **Descrição:** Gestão do ciclo de vida dos arquivos `.nrep` dentro da infraestrutura.

#### Modelo do Banco de Dados Metadata (PostgreSQL DDL Exemplo):
```sql
CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE report_folders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id),
    parent_id UUID REFERENCES report_folders(id),
    name VARCHAR(255) NOT NULL,
    path TEXT UNIQUE NOT NULL -- ex: /Financeiro/Faturamento/
);

CREATE TABLE reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    folder_id UUID REFERENCES report_folders(id),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    active_version_id UUID,
    created_by UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE report_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    report_id UUID REFERENCES reports(id),
    version_number INT NOT NULL,
    definition_json JSONB NOT NULL, -- Conteúdo do arquivo .nrep
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

---

### Skill 2.2: `High_Concurrency_Worker_Pool`
* **Pertence a:** `Server-Execution-Agent`
* **Descrição:** Gerenciador de tarefas assíncronas para disparar renderizações paralelas sem travar a API HTTP principal.

#### Arquitetura de Filas em Rust:
```rust
use tokio::sync::mpsc;
use std::sync::Arc;

pub struct ExecutionJob {
    pub job_id: String,
    pub report_definition: ReportTemplate,
    pub parameters: HashMap<String, Value>,
    pub output_format: OutputFormat, // PDF, XLSX, HTML
}

pub struct ReportWorkerPool {
    tx: mpsc::Sender<ExecutionJob>,
}

impl ReportWorkerPool {
    pub fn new(worker_count: usize) -> Self {
        let (tx, mut rx) = mpsc::channel::<ExecutionJob>(1000);

        for id in 0..worker_count {
            let mut rx = rx.clone(); // Utiliza canais concorrentes ou background tasks
            tokio::spawn(async move {
                while let Some(job) = rx.recv().await {
                    println!("Worker {} processando Job: {}", id, job.job_id);
                    let result = render_report(job.report_definition, job.parameters, job.output_format).await;
                    save_execution_result(&job.job_id, result).await;
                }
            });
        }

        Self { tx }
    }

    pub async fn submit_job(&self, job: ExecutionJob) -> Result<(), String> {
        self.tx.send(job).await.map_err(|e| e.to_string())
    }
}
```

---

### Skill 2.3: `Web_Viewer_Pagination`
* **Pertence a:** `Portal-Frontend-Agent`
* **Descrição:** Componente de interface para exibir os relatórios executados no navegador com barras de pesquisa, zoom e barra de parâmetros expansível.

#### Protocolo de Comunicação do Visualizador:
1. **Requisição de Parâmetros:** Portal envia `GET /api/v1/reports/{id}/parameters`. O React renderiza formulários dinâmicos (selects, pickers de data, inputs).
2. **Execução:** Formulário submete `POST /api/v1/reports/{id}/render` enviando o payload dos parâmetros.
3. **Stream Virtualizado:** O backend retorna o relatório paginado em HTML/SVG otimizado ou blob PDF.

---

### Skill 2.4: `Multi_Format_Export_Engine`
* **Pertence a:** `Server-Execution-Agent`
* **Descrição:** Exportação otimizada para formatos corporativos sem perdas de layout.

#### Suporte a Formatos:
* **PDF / Print PDF:** Renderização vetorial via Typst Engine.
* **XLSX (Excel):** Geração usando `rust_xlsxwriter` onde tabelas mantêm fórmulas ativas e tipos numéricos nativos (não texto).
* **CSV / JSON Data:** Exportação puramente de dados brutos extraídos após a execução das queries e filtros.
* **HTML Interativo:** Para visualização e embedding via `<iframe>` em dashboards corporativos.

---

### Skill 2.5: `Enterprise_Security_And_Scheduling`
* **Pertence a:** `Security-Scheduler-Agent`
* **Descrição:** Motor de controle de acesso (RBAC) e agendador de relatórios (*Subscriptions*).

#### Fluxo de Segurança & Subscrição:
```
[ Cron Trigger ] 
       │
       ▼
[ Validar Permissão do Job (Service Account) ]
       │
       ▼
[ Executar Queries com RLS Injetado ]
       │
       ▼
[ Renderizar para PDF / Excel ]
       │
       ▼
[ Enviar para Destinatários (SMTP / Slack / S3 Webhook) ]
```

---

## 4. Roteiro de Implementação Integrado (Roadmap)

### Fase 1: Desktop Builder (Sprints 1 a 6)
* Sprint 1-2: Especificação da DSL `.nrep` + Motor Rust de parsing e Typst PDF.
* Sprint 3-4: Editor Canvas WYSIWYG em React/Tauri com réguas e bandas.
* Sprint 5-6: Motor de Expressões Rhai + Conectores de Banco de Dados Locais.

### Fase 2: Portal Enterprise (Sprints 7 a 12)
* Sprint 7-8: Servidor Axum Rust + Autenticação OIDC + Catálogo de Pastas em PG.
* Sprint 9-10: Worker Pool de renderização distribuída + Visualizador Web React.
* Sprint 11-12: Agendador Cron + Exportadores XLSX/PDF nativos + Testes de escala.
