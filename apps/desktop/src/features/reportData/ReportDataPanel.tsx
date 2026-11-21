import { useMemo, useState } from 'react';
import type { QueryPreview } from '../../domain/types';
import { DataGrid, Modal } from '../../components/ui';
import { TreeView } from '../../components/TreeView';
import { useReport } from '../../state/reportStore';
import { useUi, type DataDialog } from '../../state/uiStore';
import * as cmd from '../../state/commands';
import { ConnectionsDialog } from '../connections/ConnectionsPanel';
import { buildTree, type TreeActions } from './buildTree';
import { DataSourceDialog } from './dialogs/DataSourceDialog';
import { DatasetDialog } from './dialogs/DatasetDialog';
import { GroupDialog } from './dialogs/GroupDialog';
import { ImageDialog } from './dialogs/ImageDialog';
import { ParameterDialog } from './dialogs/ParameterDialog';

/** Barra lateral esquerda: árvore com campos internos, parâmetros, imagens, fontes, conjuntos de dados e grupos. */
export function ReportDataPanel() {
  const report = useReport((s) => s.report);
  const { fields, treeExpanded, set, notify } = useUi();
  const [preview, setPreview] = useState<{ name: string; data: QueryPreview }>();
  const open = (dialog: DataDialog) => set({ dialog });

  const loadFields = async (name: string) => {
    try {
      const res = await cmd.refreshQueryFields(name);
      notify(`${res.columns.length} campos em ${name}`, 'success');
    } catch (e) {
      notify(`${name}: ${(e as Error).message}`, 'error');
    }
  };

  const actions: TreeActions = {
    addParameter: () => open({ kind: 'parameter' }),
    editParameter: (target) => open({ kind: 'parameter', target }),
    removeParameter: cmd.removeParameter,
    addImage: () => open({ kind: 'image' }),
    editImage: (target) => open({ kind: 'image', target }),
    removeImage: cmd.removeImage,
    addDataSource: () => open({ kind: 'dataSource' }),
    editDataSource: (target) => open({ kind: 'dataSource', target }),
    removeDataSource: cmd.removeDataSource,
    openConnections: () => open({ kind: 'connections' }),
    addDataset: (dataSource) => open({ kind: 'dataset', dataSource }),
    editDataset: (target) => open({ kind: 'dataset', target }),
    removeDataset: cmd.removeQuery,
    runDataset: async (name) => {
      try {
        setPreview({ name, data: await cmd.refreshQueryFields(name, 200) });
      } catch (e) {
        notify(`${name}: ${(e as Error).message}`, 'error');
      }
    },
    refreshFields: (name) => (name ? loadFields(name) : cmd.refreshAllFields()),
    addGroup: () => open({ kind: 'group' }),
    editGroup: (target) => open({ kind: 'group', target }),
    removeGroup: cmd.removeGroup,
    moveGroupUp: cmd.moveGroupUp,
  };

  // `actions` só usa funções estáveis; a árvore muda com o relatório e os campos
  const nodes = useMemo(() => buildTree(report, fields, actions), [report, fields]);

  const onToggle = (id: string, isOpen: boolean) => {
    set({ treeExpanded: { ...useUi.getState().treeExpanded, [id]: isOpen } });
    // abrir um conjunto de dados sem campos conhecidos já os carrega
    const q = id.startsWith('q:') ? id.slice(2) : undefined;
    if (isOpen && q && !useUi.getState().fields[q]) loadFields(q);
  };

  return (
    <div className="report-data">
      <header className="left-header">
        <strong>Dados do Relatório</strong>
        <button className="icon-btn" title="Ocultar painel (Exibir › Dados do Relatório mostra novamente)" aria-label="Ocultar painel" onClick={() => set({ showLeft: false })}>
          ×
        </button>
      </header>
      <div className="panel-scroll">
        <TreeView nodes={nodes} expanded={treeExpanded} onToggle={onToggle} label="Dados do Relatório" />
        <p className="muted small tree-hint">Clique com o botão direito para adicionar, editar ou remover. Arraste itens para o relatório.</p>
      </div>

      {preview && (
        <Modal title={`${preview.name} · ${preview.data.rows.length} linha(s) em ${preview.data.elapsedMs} ms`} wide onClose={() => setPreview(undefined)}>
          <DataGrid columns={preview.data.columns} rows={preview.data.rows} />
        </Modal>
      )}
    </div>
  );
}

/** Diálogos da árvore; renderizados pelo App para funcionar também com o painel oculto (Ribbon). */
export function DataDialogs() {
  const dialog = useUi((s) => s.dialog);
  const close = () => useUi.getState().set({ dialog: undefined });
  switch (dialog?.kind) {
    case 'parameter': return <ParameterDialog target={dialog.target} onClose={close} />;
    case 'dataSource': return <DataSourceDialog target={dialog.target} onClose={close} />;
    case 'dataset': return <DatasetDialog target={dialog.target} dataSource={dialog.dataSource} onClose={close} />;
    case 'image': return <ImageDialog target={dialog.target} onClose={close} />;
    case 'group': return <GroupDialog target={dialog.target} onClose={close} />;
    case 'connections': return <ConnectionsDialog onClose={close} />;
    default: return null;
  }
}
