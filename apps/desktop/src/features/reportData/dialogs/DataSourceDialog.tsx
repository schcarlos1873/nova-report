import { useState } from 'react';
import type { DataSourceDef, ProviderKind } from '../../../domain/types';
import { Field, Modal, Select, TextInput } from '../../../components/ui';
import { dialogs } from '../../../infra/dialogs';
import { relativeToReport } from '../../../infra/paths';
import { useReport } from '../../../state/reportStore';
import { useUi } from '../../../state/uiStore';
import { freeName, saveDataSource } from '../../../state/commands';
import { ConnectionsDialog } from '../../connections/ConnectionsPanel';
import { DialogFooter } from './DialogFooter';

export const PROVIDERS: ProviderKind[] = ['sqlite', 'postgres', 'mysql', 'mssql', 'odbc', 'csv', 'json', 'rest'];

type Source = Omit<DataSourceDef, 'queries'>;

export function DataSourceDialog({ target, onClose }: { target?: string; onClose: () => void }) {
  const report = useReport((s) => s.report);
  const connections = useUi((s) => s.connections);
  const [ds, setDs] = useState<Source>(() => {
    const cur = report.dataSources.find((x) => x.id === target);
    if (cur) {
      const { queries: _, ...rest } = structuredClone(cur);
      return rest;
    }
    return { id: freeName(report.dataSources.map((x) => x.id), 'ds_', report.dataSources.length + 1), provider: 'sqlite', options: { path: '' } };
  });
  const patch = (x: Partial<Source>) => setDs((cur) => ({ ...cur, ...x }));
  const [showConnections, setConnections] = useState(false);
  const compatible = connections.filter((c) => c.provider === ds.provider);
  const fileBased = ['sqlite', 'csv', 'json'].includes(ds.provider);
  const pathKey = ds.provider === 'rest' ? 'url' : 'path';
  const error = !ds.id.trim()
    ? 'Informe o identificador'
    : report.dataSources.some((x) => x.id === ds.id && x.id !== target)
      ? `Já existe a fonte "${ds.id}"`
      : undefined;

  return (
    <Modal
      title={target ? `Propriedades da Fonte de Dados · ${target}` : 'Nova fonte de dados'}
      onClose={onClose}
      footer={<DialogFooter error={error} onOk={() => { saveDataSource(target, ds); onClose(); }} onCancel={onClose} />}
    >
      <div className="form">
        <div className="grid2">
          <Field label="Identificador">
            <TextInput value={ds.id} mono live onCommit={(id) => patch({ id: id.replace(/\s+/g, '_') })} />
          </Field>
          <Field label="Provider">
            <Select value={ds.provider} options={PROVIDERS} onChange={(provider) => patch({ provider, connectionRef: undefined })} />
          </Field>
        </div>
        <Field label="Conexão salva" hint="Perfis com senha no cofre do sistema operacional">
          <span className="row">
            <Select
              value={ds.connectionRef ?? ''}
              options={[{ value: '', label: fileBased ? '(caminho inline)' : '(nenhuma)' }, ...compatible.map((c) => ({ value: c.alias, label: c.alias }))]}
              onChange={(v) => patch({ connectionRef: v || undefined })}
            />
            <button title="Gerenciar conexões" onClick={() => setConnections(true)}>🔌</button>
          </span>
        </Field>
        {!ds.connectionRef &&
          (fileBased || ds.provider === 'rest' ? (
            <Field label={ds.provider === 'rest' ? 'URL' : 'Arquivo'} hint="Caminho relativo ao .nrep ou absoluto">
              <span className="row">
                <TextInput value={ds.options?.[pathKey] ?? ''} mono live onCommit={(v) => patch({ options: { ...(ds.options ?? {}), [pathKey]: v } })} />
                {fileBased && (
                  <button
                    onClick={async () => {
                      const p = await dialogs.pickFile(ds.provider === 'sqlite' ? ['db', 'sqlite', 'sqlite3'] : [ds.provider]);
                      if (p) patch({ options: { ...(ds.options ?? {}), path: relativeToReport(p) } });
                    }}
                  >
                    …
                  </button>
                )}
              </span>
            </Field>
          ) : (
            <Field label="Variável de ambiente" hint="Nome da variável com a connection string">
              <TextInput value={ds.connectionStringRef ?? ''} mono live placeholder="ENV_DB_CONN" onCommit={(v) => patch({ connectionStringRef: v || undefined })} />
            </Field>
          ))}
      </div>
      {showConnections && <ConnectionsDialog onClose={() => setConnections(false)} />}
    </Modal>
  );
}
