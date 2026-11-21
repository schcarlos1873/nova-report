import { useEffect, useState } from 'react';
import type { ConnectionProfile, DatabaseTree, DriverDescriptor, QueryPreview, TableMetadata } from '../../domain/types';
import { DataGrid, Field, Modal, NumberInput, Section, Select, TextInput, Toggle } from '../../components/ui';
import { api } from '../../infra/api';
import { dialogs } from '../../infra/dialogs';
import { useReport } from '../../state/reportStore';
import { useUi } from '../../state/uiStore';

export async function refreshConnections() {
  try {
    useUi.getState().set({ connections: await api.listConnections() });
  } catch (e) {
    useUi.getState().notify((e as Error).message, 'error');
  }
}

export function ConnectionsPanel() {
  const connections = useUi((s) => s.connections);
  const [editing, setEditing] = useState<ConnectionProfile | null>(null);
  const [isNew, setIsNew] = useState(false);
  const [openAlias, setOpenAlias] = useState<string>();

  useEffect(() => {
    refreshConnections();
  }, []);

  return (
    <div className="connections">
      <Section
        title="Conexões"
        actions={
          <button
            className="link"
            onClick={() => {
              setIsNew(true);
              setEditing({ alias: '', provider: 'postgres', useTls: false, host: 'localhost' });
            }}
          >
            + Nova
          </button>
        }
      >
        {connections.length === 0 && <p className="muted small">Nenhuma conexão salva. Senhas ficam no cofre do sistema operacional, nunca no .nrep.</p>}
        {connections.map((c) => (
          <div key={c.alias} className="conn">
            <div className="conn-row">
              <button className="tree-toggle" onClick={() => setOpenAlias(openAlias === c.alias ? undefined : c.alias)}>
                {openAlias === c.alias ? '▾' : '▸'}
              </button>
              <span className={`db-badge db-${c.provider}`}>{c.provider}</span>
              <strong className="conn-alias" onDoubleClick={() => { setIsNew(false); setEditing(c); }}>{c.alias}</strong>
              <span className="spacer" />
              <button className="icon-btn" title="Editar" onClick={() => { setIsNew(false); setEditing(c); }}>✎</button>
            </div>
            {openAlias === c.alias && <SchemaTree alias={c.alias} profile={c} />}
          </div>
        ))}
      </Section>
      {editing && <ConnectionDialog initial={editing} isNew={isNew} onClose={() => setEditing(null)} />}
    </div>
  );
}

function ConnectionDialog({ initial, isNew, onClose }: { initial: ConnectionProfile; isNew: boolean; onClose: () => void }) {
  const drivers = useUi((s) => s.info?.drivers ?? []);
  const notify = useUi((s) => s.notify);
  const [p, setP] = useState<ConnectionProfile>(initial);
  const [password, setPassword] = useState('');
  const [status, setStatus] = useState<{ ok: boolean; msg: string }>();
  const [busy, setBusy] = useState(false);
  const driver: DriverDescriptor | undefined = drivers.find((d) => d.id === p.provider);
  const has = (f: string) => driver?.fields.includes(f);
  const patch = (x: Partial<ConnectionProfile>) => setP((cur) => ({ ...cur, ...x }));

  const test = async () => {
    setBusy(true);
    setStatus(undefined);
    try {
      const ms = await api.testConnection(p, password || undefined);
      setStatus({ ok: true, msg: `Conectado em ${ms} ms` });
    } catch (e) {
      setStatus({ ok: false, msg: (e as Error).message });
    } finally {
      setBusy(false);
    }
  };

  const save = async () => {
    try {
      await api.saveConnection(p, password || undefined);
      await refreshConnections();
      notify(`Conexão "${p.alias}" salva`, 'success');
      onClose();
    } catch (e) {
      setStatus({ ok: false, msg: (e as Error).message });
    }
  };

  const remove = async () => {
    if (!(await dialogs.confirm(`Excluir a conexão "${p.alias}" e sua senha do cofre?`))) return;
    await api.deleteConnection(p.alias);
    await refreshConnections();
    onClose();
  };

  return (
    <Modal
      title={isNew ? 'Nova conexão' : `Conexão · ${initial.alias}`}
      onClose={onClose}
      footer={
        <>
          {status && <span className={status.ok ? 'ok' : 'err'}>{status.msg}</span>}
          <span className="spacer" />
          {!isNew && <button className="danger" onClick={remove}>Excluir</button>}
          <button disabled={busy} onClick={test}>{busy ? 'Testando…' : 'Testar'}</button>
          <button className="primary" disabled={!p.alias.trim()} onClick={save}>Salvar</button>
        </>
      }
    >
      <div className="form">
        <Field label="Alias (usado no .nrep)">
          <TextInput value={p.alias} mono live onCommit={(v) => patch({ alias: v.replace(/\s+/g, '_') })} />
        </Field>
        <Field label="Driver">
          <Select
            value={p.provider}
            options={drivers.map((d) => ({ value: d.id, label: `${d.name} · tier ${d.tier}` }))}
            onChange={(v) => patch({ provider: v, port: drivers.find((d) => d.id === v)?.defaultPort })}
          />
        </Field>
        {has('host') && (
          <div className="grid2">
            <Field label="Host"><TextInput value={p.host ?? ''} live onCommit={(v) => patch({ host: v || undefined })} /></Field>
            <Field label="Porta"><NumberInput value={p.port ?? driver?.defaultPort ?? 0} step={1} onCommit={(v) => patch({ port: v || undefined })} /></Field>
          </div>
        )}
        {has('database') && (
          <Field label={p.provider === 'odbc' ? 'DSN' : 'Banco de dados'}>
            <TextInput value={p.database ?? ''} live onCommit={(v) => patch({ database: v || undefined })} />
          </Field>
        )}
        {has('path') && (
          <Field label={p.provider === 'rest' ? 'URL base' : 'Arquivo'}>
            <span className="row">
              <TextInput value={p.path ?? ''} mono live onCommit={(v) => patch({ path: v || undefined })} />
              {p.provider !== 'rest' && (
                <button onClick={async () => {
                  const f = await dialogs.pickFile(p.provider === 'sqlite' ? ['db', 'sqlite', 'sqlite3'] : [p.provider]);
                  if (f) patch({ path: f });
                }}>…</button>
              )}
            </span>
          </Field>
        )}
        {has('username') && (
          <div className="grid2">
            <Field label="Usuário"><TextInput value={p.username ?? ''} live onCommit={(v) => patch({ username: v || undefined })} /></Field>
            <Field label={p.provider === 'rest' ? 'Token / senha' : 'Senha'} hint="Armazenada no cofre do SO (Keychain/Credential Manager)">
              <input type="password" value={password} placeholder={isNew ? '' : '•••• (inalterada)'} onChange={(e) => setPassword(e.target.value)} />
            </Field>
          </div>
        )}
        {has('connectionString') && (
          <Field label="Connection string (opcional)" hint="Use {PASSWORD} como marcador da senha">
            <TextInput value={p.connectionString ?? ''} mono live onCommit={(v) => patch({ connectionString: v || undefined })}
              placeholder={p.provider === 'odbc' ? 'Driver={Oracle};Dbq=...;Uid=...;Pwd={PASSWORD}' : ''} />
          </Field>
        )}
        {has('useTls') && <Toggle label="Exigir TLS/SSL" checked={p.useTls} onChange={(v) => patch({ useTls: v })} />}
      </div>
    </Modal>
  );
}

function SchemaTree({ alias, profile }: { alias: string; profile: ConnectionProfile }) {
  const [tree, setTree] = useState<DatabaseTree>();
  const [error, setError] = useState<string>();
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const [preview, setPreview] = useState<{ title: string; data: QueryPreview }>();
  const update = useReport((s) => s.update);
  const notify = useUi((s) => s.notify);

  useEffect(() => {
    api.schemaTree(alias).then(setTree, (e: Error) => setError(e.message));
  }, [alias]);

  if (error) return <p className="err small">{error}</p>;
  if (!tree) return <p className="muted small">Carregando schema…</p>;

  const showData = async (t: TableMetadata) => {
    try {
      const sql = await api.selectAllSql(alias, t.schema, t.name);
      setPreview({ title: `${t.schema}.${t.name}`, data: await api.previewSql(alias, sql, {}, 200) });
    } catch (e) {
      notify((e as Error).message, 'error');
    }
  };

  /** Cria (ou reutiliza) uma fonte de dados com SELECT da tabela. */
  const addQuery = async (t: TableMetadata) => {
    const sql = await api.selectAllSql(alias, t.schema, t.name);
    update((d) => {
      let ds = d.dataSources.find((x) => x.connectionRef === alias);
      if (!ds) {
        ds = { id: `ds_${alias}`.toLowerCase(), provider: profile.provider, connectionRef: alias, queries: [] };
        d.dataSources.push(ds);
      }
      const names = new Set(d.dataSources.flatMap((x) => x.queries.map((q) => q.name)));
      let name = t.name.replace(/\W/g, '_');
      for (let n = 2; names.has(name); n++) name = `${t.name.replace(/\W/g, '_')}_${n}`;
      ds.queries.push({ name, sql });
      const detail = d.bands.find((b) => b.type === 'detail');
      if (detail && !detail.dataSource) detail.dataSource = name;
    });
    useUi.getState().set({
      fields: { ...useUi.getState().fields, [t.name]: t.columns.map((c) => ({ name: c.name, dataType: c.arrowType })) },
    });
    notify(`Query "${t.name}" adicionada ao relatório`, 'success');
  };

  const addProcedure = (schema: string, name: string) => {
    update((d) => {
      let ds = d.dataSources.find((x) => x.connectionRef === alias);
      if (!ds) {
        ds = { id: `ds_${alias}`.toLowerCase(), provider: profile.provider, connectionRef: alias, queries: [] };
        d.dataSources.push(ds);
      }
      const names = new Set(d.dataSources.flatMap((x) => x.queries.map((q) => q.name)));
      let qname = name.replace(/\W/g, '_');
      for (let n = 2; names.has(qname); n++) qname = `${name.replace(/\W/g, '_')}_${n}`;
      const procedure = schema && !['main', 'public'].includes(schema) ? `${schema}.${name}` : name;
      ds.queries.push({ name: qname, sql: '', kind: 'storedProcedure', procedure });
    });
    notify(`Procedure "${name}" adicionada — defina os parâmetros em Propriedades do Conjunto de Dados`, 'success');
  };

  const node = (key: string, label: React.ReactNode, children?: React.ReactNode, extra?: React.ReactNode) => (
    <div className="tree-node" key={key}>
      <div className="tree-label">
        {children ? (
          <button className="tree-toggle" onClick={() => setOpen((o) => ({ ...o, [key]: !o[key] }))}>{open[key] ? '▾' : '▸'}</button>
        ) : (
          <span className="tree-toggle" />
        )}
        {label}
        {extra}
      </div>
      {open[key] && children && <div className="tree-children">{children}</div>}
    </div>
  );

  const tableNode = (t: TableMetadata) =>
    node(
      `${t.schema}.${t.name}`,
      <span className="tree-text">{t.kind === 'VIEW' ? '◫' : '▦'} {t.name}</span>,
      t.columns.map((c) =>
        node(
          `${t.schema}.${t.name}.${c.name}`,
          <span className="tree-text">
            {c.isPrimaryKey ? '🔑' : c.isForeignKey ? '🔗' : '·'} {c.name} <small className="muted">{c.nativeType}</small>
          </span>,
        ),
      ),
      <span className="tree-actions">
        <button className="link" title="Ver dados" onClick={() => showData(t)}>dados</button>
        <button className="link" title="Adicionar como query do relatório" onClick={() => addQuery(t)}>+ query</button>
      </span>,
    );

  return (
    <div className="tree">
      {tree.schemas.map((s) =>
        node(
          `schema:${s.name}`,
          <span className="tree-text">🗂 {s.name}</span>,
          <>
            {s.tables.length > 0 && node(`${s.name}:tables`, <span className="tree-text">Tabelas ({s.tables.length})</span>, s.tables.map(tableNode))}
            {s.views.length > 0 && node(`${s.name}:views`, <span className="tree-text">Views ({s.views.length})</span>, s.views.map(tableNode))}
            {s.procedures.length > 0 &&
              node(`${s.name}:procs`, <span className="tree-text">Procedures ({s.procedures.length})</span>,
                s.procedures.map((p) =>
                  node(`${s.name}:p:${p.name}`, <span className="tree-text">ƒ {p.name}</span>, undefined,
                    <span className="tree-actions">
                      <button className="link" title="Adicionar como conjunto de dados (stored procedure)" onClick={() => addProcedure(p.schema, p.name)}>+ conjunto</button>
                    </span>)))}
          </>,
        ),
      )}
      {preview && (
        <Modal title={`${preview.title} · ${preview.data.rows.length} linha(s) em ${preview.data.elapsedMs} ms`} wide onClose={() => setPreview(undefined)}>
          <DataGrid columns={preview.data.columns} rows={preview.data.rows} />
        </Modal>
      )}
    </div>
  );
}

/** Gerenciador de conexões (aberto pela Ribbon ou pelo menu da Fonte de Dados). */
export function ConnectionsDialog({ onClose }: { onClose: () => void }) {
  return (
    <Modal title="Conexões" wide onClose={onClose} footer={<><span className="spacer" /><button onClick={onClose}>Fechar</button></>}>
      <ConnectionsPanel />
    </Modal>
  );
}
