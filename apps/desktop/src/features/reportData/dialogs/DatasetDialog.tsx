import { useState } from 'react';
import type { ProcedureParam, QueryDef, QueryPreview } from '../../../domain/types';
import { DataGrid, Field, Modal, Select, TextInput } from '../../../components/ui';
import { api } from '../../../infra/api';
import { useReport } from '../../../state/reportStore';
import { useUi } from '../../../state/uiStore';
import { freeName, queryNames, saveDataset } from '../../../state/commands';
import { DialogFooter } from './DialogFooter';
import { ProcParamsDialog } from './ProcParamsDialog';

const PROC_PROVIDERS = ['mssql', 'postgres', 'mysql'];

/** Conjunto de dados: SQL em texto ou chamada de stored procedure com mapeamento de parâmetros. */
export function DatasetDialog({ target, dataSource, onClose }: { target?: string; dataSource?: string; onClose: () => void }) {
  const report = useReport((s) => s.report);
  const filePath = useReport((s) => s.filePath);
  const { paramValues, notify } = useUi();
  const owner = report.dataSources.find((d) => d.queries.some((q) => q.name === target));
  const [dsId, setDsId] = useState(owner?.id ?? dataSource ?? report.dataSources[0]?.id ?? '');
  const [q, setQ] = useState<QueryDef>(() => {
    const cur = owner?.queries.find((x) => x.name === target);
    return cur ? structuredClone(cur) : { name: freeName(queryNames(report), 'Query'), sql: 'SELECT 1 AS exemplo' };
  });
  const [procParams, setProcParams] = useState(false);
  const [preview, setPreview] = useState<QueryPreview>();
  const [busy, setBusy] = useState(false);
  const patch = (x: Partial<QueryDef>) => setQ((cur) => ({ ...cur, ...x }));

  const provider = report.dataSources.find((d) => d.id === dsId)?.provider;
  const isProc = q.kind === 'storedProcedure';
  const error = !dsId
    ? 'Crie uma fonte de dados primeiro'
    : !q.name.trim()
      ? 'Informe o nome'
      : queryNames(report).some((n) => n === q.name && n !== target)
        ? `Já existe o conjunto "${q.name}"`
        : isProc && !q.procedure?.trim()
          ? 'Informe o nome da procedure'
          : isProc && provider && !PROC_PROVIDERS.includes(provider)
            ? `O provider ${provider} não suporta stored procedures`
            : !isProc && !q.sql.trim()
              ? 'A query está vazia'
              : undefined;

  /** Normaliza o rascunho para o formato salvo (campos da outra modalidade não vão para o .nrep). */
  const clean = (): QueryDef =>
    isProc
      ? { name: q.name, sql: '', kind: 'storedProcedure', procedure: q.procedure?.trim(), procedureParams: q.procedureParams?.length ? q.procedureParams : undefined }
      : { name: q.name, sql: q.sql };

  const run = async (show: boolean) => {
    setBusy(true);
    try {
      // executa o rascunho sem gravá-lo: uma cópia do relatório com a query substituída
      const draft = structuredClone(report);
      for (const d of draft.dataSources) d.queries = d.queries.filter((x) => x.name !== target && x.name !== q.name);
      draft.dataSources.find((d) => d.id === dsId)?.queries.push(clean());
      const res = await api.previewReportQuery(draft, q.name, paramValues, filePath, show ? 200 : 1);
      useUi.getState().set({ fields: { ...useUi.getState().fields, [q.name]: res.columns } });
      if (show) setPreview(res);
      else notify(`${res.columns.length} campos em ${q.name}`, 'success');
    } catch (e) {
      notify((e as Error).message, 'error');
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title={target ? `Propriedades do Conjunto de Dados · ${target}` : 'Novo conjunto de dados'}
      wide
      onClose={onClose}
      footer={
        <>
          <button disabled={busy || !!error} onClick={() => run(true)}>▶ Executar</button>
          <button disabled={busy || !!error} onClick={() => run(false)}>⟳ Campos</button>
          <DialogFooter error={error} onOk={() => { saveDataset(target, dsId, clean()); onClose(); }} onCancel={onClose} />
        </>
      }
    >
      <div className="form">
        <div className="grid2">
          <Field label="Nome">
            <TextInput value={q.name} mono live onCommit={(name) => patch({ name: name.replace(/\s+/g, '_') })} />
          </Field>
          <Field label="Fonte de dados">
            <Select value={dsId || undefined} options={report.dataSources.map((d) => ({ value: d.id, label: `${d.id} · ${d.provider}` }))} onChange={setDsId} />
          </Field>
        </div>
        <div className="row gap" role="radiogroup" aria-label="Tipo de comando">
          <label className="toggle">
            <input type="radio" name="qkind" checked={!isProc} onChange={() => patch({ kind: undefined })} />
            <span>Texto</span>
          </label>
          <label className="toggle">
            <input type="radio" name="qkind" checked={isProc} onChange={() => patch({ kind: 'storedProcedure' })} />
            <span>Stored Procedure</span>
          </label>
        </div>
        {isProc ? (
          <>
            <Field label="Nome Procedure">
              <span className="row">
                <TextInput value={q.procedure ?? ''} mono live placeholder="get_new_orders" onCommit={(procedure) => patch({ procedure })} />
                <button onClick={() => setProcParams(true)}>+ Parâmetros{q.procedureParams?.length ? ` (${q.procedureParams.length})` : ''}</button>
              </span>
            </Field>
            {!!q.procedureParams?.length && (
              <p className="muted small mono">
                {q.procedureParams.map((p) => `${p.name} = ${p.passNull ? 'NULL' : p.value || 'NULL'}`).join(' · ')}
              </p>
            )}
            {provider && !PROC_PROVIDERS.includes(provider) && <p className="err small">Stored procedures são suportadas em SQL Server, PostgreSQL e MySQL.</p>}
          </>
        ) : (
          <Field label="SQL" hint="Use :parametro para valores do relatório">
            <textarea className="mono sql" rows={12} value={q.sql} spellCheck={false} onChange={(e) => patch({ sql: e.target.value })} />
          </Field>
        )}
      </div>
      {procParams && (
        <ProcParamsDialog
          initial={q.procedureParams ?? []}
          onSave={(procedureParams: ProcedureParam[]) => patch({ procedureParams })}
          onClose={() => setProcParams(false)}
        />
      )}
      {preview && (
        <Modal title={`${q.name} · ${preview.rows.length} linha(s) em ${preview.elapsedMs} ms`} wide onClose={() => setPreview(undefined)}>
          <DataGrid columns={preview.columns} rows={preview.rows} />
        </Modal>
      )}
    </Modal>
  );
}
