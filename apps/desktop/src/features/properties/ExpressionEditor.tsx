import { useState } from 'react';
import { api } from '../../infra/api';
import { Modal } from '../../components/ui';
import { useReport } from '../../state/reportStore';
import { useUi } from '../../state/uiStore';

const FUNCTIONS = [
  'SUM("campo")', 'AVG("campo")', 'MIN("campo")', 'MAX("campo")', 'COUNT()', 'DISTINCT_COUNT("campo")', 'RUNNING_SUM("campo")',
  'IIF(cond, a, b)', 'FORMAT_DATE(v, "%d/%m/%Y")', 'Format.currency(v, "BRL")', 'Format.number(v, 2)', 'Format.percent(v, 1)',
  'ROUND(v, 2)', 'UPPER(v)', 'LOWER(v)', 'TRIM(v)', 'LEFT(v, n)', 'RIGHT(v, n)', 'LEN(v)', 'COALESCE(a, b)', 'ISNULL(v)',
  'TODAY()', 'NOW()', 'RECORD_NUMBER()', 'PAGE_NUMBER()', 'TOTAL_PAGES()', 'Report.Title', 'Group.Count',
  'JOIN(Params.lista, ", ")', 'CONTAINS(Params.lista, v)',
];

/** Campo de expressão com botão para abrir o editor completo (validação de sintaxe no motor Rhai). */
export function ExpressionField({ value, onCommit, placeholder }: { value?: string; onCommit: (v?: string) => void; placeholder?: string }) {
  const [open, setOpen] = useState(false);
  return (
    <span className="expr-field">
      <input className="mono" value={value ?? ''} placeholder={placeholder} readOnly onClick={() => setOpen(true)} />
      <button className="icon-btn" title="Editar expressão" onClick={() => setOpen(true)}>ƒx</button>
      {open && <ExpressionEditor initial={value ?? ''} onClose={() => setOpen(false)} onSave={(v) => { onCommit(v.trim() || undefined); setOpen(false); }} />}
    </span>
  );
}

export function ExpressionEditor({ initial, onSave, onClose }: { initial: string; onSave: (v: string) => void; onClose: () => void }) {
  const [text, setText] = useState(initial);
  const [status, setStatus] = useState<{ ok: boolean; msg: string }>();
  const report = useReport((s) => s.report);
  const fields = useUi((s) => s.fields);
  const allFields = [...new Set(Object.values(fields).flat().map((f) => f.name))];

  const insert = (snippet: string) => setText((t) => (t ? `${t} ${snippet}` : snippet));
  const validate = async () => {
    try {
      await api.checkExpression(text);
      setStatus({ ok: true, msg: 'Sintaxe válida' });
      return true;
    } catch (e) {
      setStatus({ ok: false, msg: (e as Error).message });
      return false;
    }
  };

  return (
    <Modal
      title="Editor de expressão"
      wide
      onClose={onClose}
      footer={
        <>
          {status && <span className={status.ok ? 'ok' : 'err'}>{status.msg}</span>}
          <span className="spacer" />
          <button onClick={validate}>Validar</button>
          <button className="primary" onClick={async () => (text.trim() === '' || (await validate())) && onSave(text)}>Aplicar</button>
        </>
      }
    >
      <div className="expr-editor">
        <textarea className="mono" value={text} onChange={(e) => setText(e.target.value)} rows={6} autoFocus spellCheck={false} />
        <div className="expr-lists">
          <div>
            <h4>Campos</h4>
            {allFields.length === 0 && <p className="muted small">Execute uma query no painel Dados para descobrir os campos.</p>}
            {allFields.map((f) => <button key={f} className="chip" onClick={() => insert(`Fields.${f}`)}>{f}</button>)}
          </div>
          <div>
            <h4>Parâmetros</h4>
            {report.parameters.map((p) => <button key={p.name} className="chip" onClick={() => insert(`Params.${p.name}`)}>{p.name}</button>)}
          </div>
          <div>
            <h4>Funções</h4>
            {FUNCTIONS.map((f) => <button key={f} className="chip mono" onClick={() => insert(f)}>{f}</button>)}
          </div>
        </div>
      </div>
    </Modal>
  );
}
