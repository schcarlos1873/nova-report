import { useState } from 'react';
import type { ProcedureParam } from '../../../domain/types';
import { Modal } from '../../../components/ui';
import { useReport } from '../../../state/reportStore';
import { DialogFooter } from './DialogFooter';

/** "Definir Parâmetros da Procedure": Ok salva e fecha, Cancelar fecha sem salvar. */
export function ProcParamsDialog({ initial, onSave, onClose }: { initial: ProcedureParam[]; onSave: (p: ProcedureParam[]) => void; onClose: () => void }) {
  const params = useReport((s) => s.report.parameters);
  const [rows, setRows] = useState<ProcedureParam[]>(() => (initial.length ? structuredClone(initial) : [{ name: '@param1', value: '', passNull: false }]));
  const set = (i: number, x: Partial<ProcedureParam>) => setRows((r) => r.map((row, k) => (k === i ? { ...row, ...x } : row)));
  const names = rows.map((r) => r.name.trim().replace(/^@/, '').toLowerCase());
  const error = names.some((n) => !n) ? 'Todo parâmetro precisa de nome' : new Set(names).size !== names.length ? 'Nomes de parâmetro repetidos' : undefined;

  const ok = () => {
    onSave(rows.map((r) => ({ name: r.name.trim().startsWith('@') ? r.name.trim() : `@${r.name.trim()}`, value: r.value.trim(), passNull: r.passNull || undefined })));
    onClose();
  };

  return (
    <Modal title="Definir Parâmetros da Procedure" wide onClose={onClose} footer={<DialogFooter error={error} onOk={ok} onCancel={onClose} />}>
      <datalist id="report-params">
        {params.map((p) => <option key={p.name} value={`:${p.name}`}>{p.label}</option>)}
      </datalist>
      <table className="data-grid proc-params">
        <thead>
          <tr>
            <th>Nome Parâmetro</th>
            <th>Valor Parâmetro<small>:parametro do relatório ou literal</small></th>
            <th>Passar Valor Nulo</th>
            <th aria-label="Ações" />
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              <td><input className="mono" value={r.name} placeholder="@param" aria-label="Nome do parâmetro" onChange={(e) => set(i, { name: e.target.value })} /></td>
              <td>
                <input className="mono" list="report-params" value={r.value} disabled={r.passNull} placeholder=":param" aria-label="Valor do parâmetro"
                  onChange={(e) => set(i, { value: e.target.value })} />
              </td>
              <td className="center-cell">
                <input type="checkbox" checked={!!r.passNull} aria-label="Passar valor nulo" onChange={(e) => set(i, { passNull: e.target.checked })} />
              </td>
              <td className="center-cell">
                <button className="icon-btn" title="Remover linha" onClick={() => setRows((x) => x.filter((_, k) => k !== i))}>×</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="row" style={{ marginTop: 8 }}>
        <button
          className="link"
          onClick={() => setRows((r) => [...r, { name: `@param${r.length + 1}`, value: params[r.length] ? `:${params[r.length].name}` : '', passNull: false }])}
        >
          + Adicionar parâmetro
        </button>
        <span className="spacer" />
        {params.length > 0 && (
          <button
            className="link"
            title="Uma linha @nome = :nome para cada parâmetro do relatório ainda não mapeado"
            onClick={() =>
              setRows((r) => [
                ...r.filter((x) => x.value.trim() || x.name !== '@param1'),
                ...params.filter((p) => !r.some((x) => x.value.trim() === `:${p.name}`)).map((p) => ({ name: `@${p.name}`, value: `:${p.name}`, passNull: false })),
              ])
            }
          >
            Mapear parâmetros do relatório
          </button>
        )}
      </div>
    </Modal>
  );
}
