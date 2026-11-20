import { useState } from 'react';
import type { Draft } from 'immer';
import type { DataSourceDef, ParamType, ProviderKind, QueryPreview, ReportDefinition } from '../../domain/types';
import { DataGrid, Field, Modal, Section, Select, TextInput, Toggle } from '../../components/ui';
import { api } from '../../infra/api';
import { dialogs } from '../../infra/dialogs';
import { useReport } from '../../state/reportStore';
import { useUi } from '../../state/uiStore';
import { beginDrag, endDrag, isDate, isNumeric } from '../designer/dnd';
import { addDataSource, addGroup, newParameter } from '../../state/commands';

const PROVIDERS: ProviderKind[] = ['sqlite', 'postgres', 'mysql', 'mssql', 'odbc', 'csv', 'json', 'rest'];
const PARAM_TYPES: { value: ParamType; label: string }[] = [
  { value: 'string', label: 'Texto' },
  { value: 'int', label: 'Inteiro' },
  { value: 'float', label: 'Decimal' },
  { value: 'bool', label: 'Booleano' },
  { value: 'date', label: 'Data' },
  { value: 'datetime', label: 'Data/hora' },
];

export function DataPanel() {
  const report = useReport((s) => s.report);
  const update = useReport((s) => s.update);
  const edit = (key: string, recipe: (d: Draft<ReportDefinition>) => void) => update(recipe, key);

  return (
    <div className="data-panel">
      <Section
        title="Fontes de dados"
        actions={
          <button className="link" onClick={addDataSource}>
            + Fonte
          </button>
        }
      >
        {report.dataSources.length === 0 && <p className="muted small">Nenhuma fonte. Adicione uma ou gere a partir do navegador de Conexões.</p>}
        {report.dataSources.map((ds, i) => (
          <DataSourceCard key={ds.id + i} ds={ds} index={i} />
        ))}
      </Section>

      <Section
        title="Parâmetros"
        actions={
          <button className="link" onClick={newParameter}>
            + Parâmetro
          </button>
        }
      >
        {report.parameters.map((p, i) => (
          <div key={i} className="card">
            <div className="row">
              <TextInput value={p.name} mono onCommit={(v) => edit(`p${i}.n`, (d) => void (d.parameters[i].name = v.replace(/[^A-Za-z0-9_]/g, '_')))} />
              <Select value={p.type} options={PARAM_TYPES} onChange={(v) => edit(`p${i}.t`, (d) => void (d.parameters[i].type = v))} />
              <button className="icon-btn" title="Remover" onClick={() => edit('p.rm', (d) => void d.parameters.splice(i, 1))}>×</button>
            </div>
            <div className="row">
              <TextInput value={p.label} placeholder="Rótulo" onCommit={(v) => edit(`p${i}.l`, (d) => void (d.parameters[i].label = v))} />
              <TextInput
                value={p.defaultValue === undefined ? '' : String(p.defaultValue)}
                placeholder="padrão (ex.: today)"
                onCommit={(v) => edit(`p${i}.d`, (d) => void (d.parameters[i].defaultValue = v === '' ? undefined : v))}
              />
            </div>
            <Toggle label="Obrigatório" checked={p.required} onChange={(v) => edit(`p${i}.r`, (d) => void (d.parameters[i].required = v))} />
            <p className="muted small mono">:{p.name} na SQL · Params.{p.name} nas expressões</p>
          </div>
        ))}
      </Section>

      <GroupsSection />
    </div>
  );
}

function DataSourceCard({ ds, index: i }: { ds: DataSourceDef; index: number }) {
  const update = useReport((s) => s.update);
  const connections = useUi((s) => s.connections);
  const edit = (key: string, recipe: (d: Draft<DataSourceDef>) => void) =>
    update((d) => recipe(d.dataSources[i]), `ds${i}.${key}`);
  const compatible = connections.filter((c) => c.provider === ds.provider);
  const fileBased = ['sqlite', 'csv', 'json'].includes(ds.provider);

  return (
    <div className="card">
      <div className="row">
        <TextInput value={ds.id} mono onCommit={(v) => edit('id', (d) => void (d.id = v))} />
        <Select value={ds.provider} options={PROVIDERS} onChange={(v) => edit('prov', (d) => void (d.provider = v))} />
        <button
          className="icon-btn"
          title="Remover fonte"
          onClick={async () => {
            if (await dialogs.confirm(`Remover a fonte "${ds.id}" e suas queries?`)) update((d) => void d.dataSources.splice(i, 1));
          }}
        >
          ×
        </button>
      </div>
      <Field label="Conexão salva">
        <Select
          value={ds.connectionRef ?? ''}
          options={[{ value: '', label: fileBased ? '(caminho inline)' : '(nenhuma)' }, ...compatible.map((c) => ({ value: c.alias, label: c.alias }))]}
          onChange={(v) => edit('ref', (d) => void (d.connectionRef = v || undefined))}
        />
      </Field>
      {!ds.connectionRef && (
        fileBased || ds.provider === 'rest' ? (
          <Field label={ds.provider === 'rest' ? 'URL' : 'Arquivo'} hint="Caminho relativo ao .nrep ou absoluto">
            <span className="row">
              <TextInput
                value={ds.options?.path ?? ds.options?.url ?? ''}
                mono
                onCommit={(v) => edit('path', (d) => void (d.options = { ...(d.options ?? {}), [ds.provider === 'rest' ? 'url' : 'path']: v }))}
              />
              {fileBased && (
                <button
                  onClick={async () => {
                    const p = await dialogs.pickFile(ds.provider === 'sqlite' ? ['db', 'sqlite', 'sqlite3'] : [ds.provider]);
                    if (p) edit('path', (d) => void (d.options = { ...(d.options ?? {}), path: relativeToReport(p) }));
                  }}
                >
                  …
                </button>
              )}
            </span>
          </Field>
        ) : (
          <Field label="Variável de ambiente" hint="Nome da variável com a connection string">
            <TextInput value={ds.connectionStringRef ?? ''} mono placeholder="ENV_DB_CONN" onCommit={(v) => edit('env', (d) => void (d.connectionStringRef = v || undefined))} />
          </Field>
        )
      )}
      {ds.queries.map((_, qi) => (
        <QueryEditor key={qi} dsIndex={i} queryIndex={qi} />
      ))}
      <button
        className="link"
        onClick={() =>
          edit('q.add', (d) => {
            const r = useReport.getState().report;
            const taken = new Set(r.dataSources.flatMap((x) => x.queries.map((q) => q.name)));
            let n = 1;
            while (taken.has(`Query${n}`)) n++;
            d.queries.push({ name: `Query${n}`, sql: 'SELECT 1' });
          })
        }
      >
        + Query
      </button>
    </div>
  );
}

/** Caminhos dentro da pasta do .nrep viram relativos (portabilidade do relatório). */
function relativeToReport(p: string): string {
  const fp = useReport.getState().filePath;
  if (!fp) return p;
  const dir = fp.replace(/[\\/][^\\/]*$/, '');
  return p.startsWith(dir + '/') || p.startsWith(dir + '\\') ? p.slice(dir.length + 1) : p;
}

function QueryEditor({ dsIndex, queryIndex }: { dsIndex: number; queryIndex: number }) {
  const report = useReport((s) => s.report);
  const update = useReport((s) => s.update);
  const filePath = useReport((s) => s.filePath);
  const { fields, paramValues, set, notify } = useUi();
  const q = report.dataSources[dsIndex].queries[queryIndex];
  const [sql, setSql] = useState(q.sql);
  const [preview, setPreview] = useState<QueryPreview>();
  const [busy, setBusy] = useState(false);
  const [expanded, setExpanded] = useState(false);
  if (sql !== q.sql && !expanded) setSql(q.sql);

  const commit = () => sql !== q.sql && update((d) => void (d.dataSources[dsIndex].queries[queryIndex].sql = sql), `q${dsIndex}.${queryIndex}`);

  const run = async (show: boolean) => {
    commit();
    setBusy(true);
    try {
      const r = useReport.getState().report;
      const res = await api.previewReportQuery(r, q.name, paramValues, filePath, show ? 200 : 1);
      set({ fields: { ...useUi.getState().fields, [q.name]: res.columns } });
      if (show) setPreview(res);
      else notify(`${res.columns.length} campos em ${q.name}`, 'success');
    } catch (e) {
      notify((e as Error).message, 'error');
    } finally {
      setBusy(false);
    }
  };

  const known = fields[q.name] ?? [];
  return (
    <div className="query">
      <div className="row">
        <span className="query-badge">SQL</span>
        <TextInput
          value={q.name}
          mono
          onCommit={(v) =>
            update((d) => {
              const old = d.dataSources[dsIndex].queries[queryIndex].name;
              d.dataSources[dsIndex].queries[queryIndex].name = v;
              // renomeia referências em bandas e tabelas
              for (const b of d.bands) {
                if (b.dataSource === old) b.dataSource = v;
                for (const e of b.elements) if (e.type === 'table' && e.dataSource === old) e.dataSource = v;
              }
            })
          }
        />
        <button className="icon-btn" title="Remover query" onClick={() => update((d) => void d.dataSources[dsIndex].queries.splice(queryIndex, 1))}>×</button>
      </div>
      <textarea
        className="mono sql"
        rows={expanded ? 10 : 4}
        value={sql}
        spellCheck={false}
        onFocus={() => setExpanded(true)}
        onChange={(e) => setSql(e.target.value)}
        onBlur={() => {
          commit();
          setExpanded(false);
        }}
      />
      <div className="row gap">
        <button disabled={busy} onClick={() => run(true)}>▶ Executar</button>
        <button disabled={busy} onClick={() => run(false)}>⟳ Campos</button>
      </div>
      {known.length === 0 && <p className="muted small">Clique em ⟳ Campos para listar os campos e arrastá-los ao canvas.</p>}
      {known.length > 0 && (
        <div className="fields">
          {known.map((f) => (
            <span
              key={f.name}
              className="chip field-chip"
              draggable
              title={`${f.dataType} — arraste para uma banda ou tabela (⌥ ao soltar: valor + rótulo)`}
              onDragStart={(e) => beginDrag(e, { field: f.name, query: q.name, dataType: f.dataType })}
              onDragEnd={endDrag}
            >
              <span className="field-type" aria-hidden>{isNumeric(f.dataType) ? '#' : isDate(f.dataType) ? '◷' : 'a'}</span>
              {f.name}
            </span>
          ))}
        </div>
      )}
      {preview && (
        <Modal title={`${q.name} · ${preview.rows.length} linha(s) em ${preview.elapsedMs} ms`} wide onClose={() => setPreview(undefined)}>
          <DataGrid columns={preview.columns} rows={preview.rows} />
        </Modal>
      )}
    </div>
  );
}

function GroupsSection() {
  const report = useReport((s) => s.report);
  const update = useReport((s) => s.update);
  const groups = report.groups ?? [];
  return (
    <Section title="Grupos" actions={<button className="link" onClick={addGroup}>+ Grupo</button>}>
      {groups.length === 0 && <p className="muted small">Grupos quebram o detalhe por uma chave e permitem subtotais (SUM, COUNT...).</p>}
      {groups.map((g, i) => (
        <div key={i} className="card">
          <div className="row">
            <TextInput
              value={g.name}
              mono
              onCommit={(v) =>
                update((d) => {
                  const old = d.groups![i].name;
                  d.groups![i].name = v;
                  for (const b of d.bands) if (b.group === old) b.group = v;
                })
              }
            />
            <button className="icon-btn" title="Subir (grupo mais externo)" disabled={i === 0}
              onClick={() => update((d) => { const [x] = d.groups!.splice(i, 1); d.groups!.splice(i - 1, 0, x); })}>↑</button>
            <button
              className="icon-btn"
              title="Remover grupo e suas bandas"
              onClick={() =>
                update((d) => {
                  const name = d.groups![i].name;
                  d.groups!.splice(i, 1);
                  d.bands = d.bands.filter((b) => b.group !== name);
                })
              }
            >
              ×
            </button>
          </div>
          <Field label="Chave (expressão)">
            <TextInput value={g.expression} mono onCommit={(v) => update((d) => void (d.groups![i].expression = v))} />
          </Field>
          <Toggle label="Nova página por grupo" checked={!!g.pageBreakBetween} onChange={(v) => update((d) => void (d.groups![i].pageBreakBetween = v || undefined))} />
        </div>
      ))}
      {groups.length > 0 && <p className="muted small">A query do detalhe deve estar ordenada pelas chaves dos grupos.</p>}
    </Section>
  );
}
