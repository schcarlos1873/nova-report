Daqui não consigo criar arquivos, mas segue o conteúdo pronto para salvar como `README.md` na raiz do projeto (ou peça na conversa principal para eu gravá-lo). Um aviso: o build do app desktop ainda estava rodando quando escrevi isto. Por isso os comandos do desktop seguem o padrão do Tauri 2, mas ainda não foram validados neste projeto. Já o CLI e os testes foram executados e passaram.

````markdown
# NovaReport

Construtor de relatórios desktop (Fase 1) em **Rust + Tauri 2 + React/Konva**. O PDF é gerado pelo **Typst**, os dados passam pelo **Polars** e as expressões rodam no **Rhai**. O projeto segue Clean Architecture com DDD.

## Estrutura

| Caminho | Camada | Descrição |
| :--- | :--- | :--- |
| `crates/nova-domain` | Domínio | Agregado `.nrep`, value objects, validação, ports |
| `crates/nova-engine` | Núcleo | DataContext (Polars), fórmulas (Rhai), bandas/grupos, transpilador Typst, PDF/SVG, CSV/XLSX |
| `crates/nova-application` | Aplicação | Casos de uso: render/preview/export, conexões, documentos |
| `crates/nova-connectors` | Infraestrutura | Drivers SQLite, PostgreSQL, MySQL, SQL Server, CSV, JSON, REST e ODBC; dialetos; keyring |
| `crates/nova-cli` | Apresentação | CLI headless `nova-report` |
| `apps/desktop` | Apresentação | App Tauri: `src-tauri` (Rust) + `src` (React) |
| `examples/` | — | `vendas_por_regiao.nrep` + `vendas.db` + `logo.svg` |

## 1. Pré-requisitos

| Ferramenta | Versão testada | Observação |
| :--- | :--- | :--- |
| Rust | 1.94 | O Polars está fixado em 0.53, porque 0.54 e 0.55 exigem Rust 1.95 |
| Node.js / npm | 26 / 12 | Necessário para o frontend |
| Tauri CLI | 2.x | Use `cargo install tauri-cli --version "^2"` ou o `npm run tauri` local |

Dependências de sistema:

- **macOS:** `xcode-select --install`
- **Windows:** Visual Studio Build Tools (C++) e WebView2 (já vem no Windows 11)
- **Linux (Debian/Ubuntu):**
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
    libayatana-appindicator3-dev libssl-dev build-essential pkg-config
  ```
  O cofre de senhas usa o Secret Service (GNOME Keyring ou KWallet).
- **ODBC (opcional):**
  - macOS: `brew install unixodbc`
  - Linux: `apt install unixodbc-dev`
  - Windows: já incluso

## 2. Compilar e testar o núcleo

```bash
cargo build --workspace
cargo test --workspace          # testes de domínio, engine, conectores (inclui SQLite real)
```

Para incluir a ponte ODBC:

```bash
cargo build -p nova-connectors --features odbc
```

## 3. CLI `nova-report`

### Compilar

```bash
cargo build --release -p nova-cli
# binário: target/release/nova-report  (nova-report.exe no Windows)
```

### Usar

```bash
nova-report validate examples/vendas_por_regiao.nrep
nova-report render   examples/vendas_por_regiao.nrep -o vendas.pdf --system-fonts
nova-report render   examples/vendas_por_regiao.nrep -f xlsx -o vendas.xlsx
nova-report render   examples/vendas_por_regiao.nrep -p data_inicio=2024-06-01 -p data_fim=2024-06-30
nova-report svg      examples/vendas_por_regiao.nrep ./paginas
nova-report drivers
```

O CLI lê duas variáveis de ambiente:

- `NOVA_CONNECTIONS`: caminho do `connections.json` com os perfis de conexão.
- `NOVA_SECRET_<ALIAS>`: senha da conexão `<ALIAS>`.

### Publicar

Basta copiar `target/release/nova-report`. O binário é estático em relação a Typst, Polars e SQLite, mas o ODBC continua dependendo do driver manager do sistema.

Para gerar para outros alvos:

```bash
rustup target add x86_64-unknown-linux-gnu
cargo build --release -p nova-cli --target x86_64-unknown-linux-gnu
```

## 4. App desktop (NovaReport Builder)

### Desenvolvimento

```bash
cd apps/desktop
npm install
npm run tauri dev        # sobe o Vite em :1420 e abre a janela com hot reload
```

### Checagens do frontend

```bash
npm run typecheck
npm run build            # gera apps/desktop/dist
```

### Build de produção e instaladores

```bash
cd apps/desktop
npm run tauri build
```

Os artefatos ficam em `target/release/bundle/`:

- **macOS:** `.app` e `.dmg`
- **Windows:** `.msi` (WiX) e `-setup.exe` (NSIS)
- **Linux:** `.deb`, `.rpm` e `.AppImage`

Para gerar só alguns formatos:

```bash
npm run tauri build -- --bundles dmg
npm run tauri build -- --bundles msi,nsis
npm run tauri build -- --bundles deb,appimage
```

Universal binary para macOS:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin
```

### Assinatura e notarização

**macOS:** defina as variáveis abaixo antes do `tauri build`.

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Empresa (TEAMID)"
export APPLE_ID="voce@empresa.com" APPLE_PASSWORD="app-specific-password" APPLE_TEAM_ID="TEAMID"
```

**Windows:** configure `bundle.windows.certificateThumbprint` em `src-tauri/tauri.conf.json`, ou use `signCommand` para assinar com um HSM ou com o Azure Trusted Signing.

**Linux:** assine os pacotes com as ferramentas da sua distribuição, se for necessário.

### Ícones

```bash
cd apps/desktop
cargo tauri icon caminho/para/icone-1024.png -o src-tauri/icons
```

### Versão

Atualize a versão em três lugares, mantendo-os iguais:

- `[workspace.package].version` no `Cargo.toml` da raiz
- `version` em `apps/desktop/src-tauri/tauri.conf.json`
- `version` em `apps/desktop/package.json`

## 5. Publicação contínua (sugestão de CI)

Use uma matriz no GitHub Actions com `macos-latest`, `windows-latest` e `ubuntu-22.04`:

1. `actions/checkout`
2. `dtolnay/rust-toolchain@1.94`
3. `actions/setup-node`
4. Em Linux, instalar as dependências da seção 1
5. `cargo test --workspace`
6. `tauri-apps/tauri-action@v0` com `projectPath: apps/desktop`. Essa action compila, assina (com os secrets configurados) e anexa os instaladores a um GitHub Release.
7. Publicar `target/release/nova-report` de cada SO como artefato do CLI.

## 6. Onde ficam os dados do usuário

- Os perfis de conexão ficam em `connections.json`, no diretório de configuração do app (`com.novareport.builder`).
- As senhas ficam no cofre do sistema operacional: Keychain no macOS, Credential Manager no Windows e Secret Service no Linux.
- O arquivo `.nrep` **nunca** armazena senhas.
````

Se quiser, peça na conversa principal para gravar este conteúdo em `README.md`. Lá também posso conferir os comandos do desktop depois que o build terminar.