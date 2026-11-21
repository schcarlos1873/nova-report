import { useState } from 'react';
import type { GroupDef } from '../../../domain/types';
import { Field, Modal, Select, TextInput, Toggle } from '../../../components/ui';
import { useReport } from '../../../state/reportStore';
import { useUi } from '../../../state/uiStore';
import { freeName, saveGroup } from '../../../state/commands';
import { DialogFooter } from './DialogFooter';

export function GroupDialog({ target, onClose }: { target?: string; onClose: () => void }) {
  const report = useReport((s) => s.report);
  const fields = useUi((s) => s.fields);
  const groups = report.groups ?? [];
  const detailQuery = report.bands.find((b) => b.type === 'detail')?.dataSource;
  const known = detailQuery ? fields[detailQuery] ?? [] : [];
  const [g, setG] = useState<GroupDef>(() => {
    const cur = groups.find((x) => x.name === target);
    return cur ? structuredClone(cur) : { name: freeName(groups.map((x) => x.name), 'grupo', groups.length + 1), expression: known[0] ? `Fields.${known[0].name}` : 'Fields.campo' };
  });
  const patch = (x: Partial<GroupDef>) => setG((cur) => ({ ...cur, ...x }));
  const error = !g.name.trim()
    ? 'Informe o nome'
    : groups.some((x) => x.name === g.name && x.name !== target)
      ? `Já existe o grupo "${g.name}"`
      : !g.expression.trim()
        ? 'Informe a chave'
        : undefined;

  return (
    <Modal
      title={target ? `Propriedades do Grupo · ${target}` : 'Novo grupo'}
      onClose={onClose}
      footer={<DialogFooter error={error} onOk={() => { saveGroup(target, { ...g, pageBreakBetween: g.pageBreakBetween || undefined }); onClose(); }} onCancel={onClose} />}
    >
      <div className="form">
        <Field label="Nome">
          <TextInput value={g.name} mono live onCommit={(name) => patch({ name: name.replace(/\s+/g, '_') })} />
        </Field>
        <Field label="Chave (expressão)" hint="O detalhe é quebrado sempre que o valor muda">
          <TextInput value={g.expression} mono live onCommit={(expression) => patch({ expression })} />
        </Field>
        {known.length > 0 && (
          <Field label={`Campo de ${detailQuery}`}>
            <Select
              value={known.find((f) => g.expression === `Fields.${f.name}`)?.name ?? ''}
              options={[{ value: '', label: '(expressão livre)' }, ...known.map((f) => ({ value: f.name, label: f.name }))]}
              onChange={(f) => f && patch({ expression: `Fields.${f}` })}
            />
          </Field>
        )}
        <Toggle label="Nova página por grupo" checked={!!g.pageBreakBetween} onChange={(pageBreakBetween) => patch({ pageBreakBetween })} />
        <p className="muted small">A query do detalhe deve estar ordenada pelas chaves dos grupos. Um grupo novo cria seu cabeçalho e rodapé.</p>
      </div>
    </Modal>
  );
}
