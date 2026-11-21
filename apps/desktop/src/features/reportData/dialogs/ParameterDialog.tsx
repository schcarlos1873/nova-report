import { useState } from 'react';
import type { ParamType, ParameterDef } from '../../../domain/types';
import { Field, Modal, Select, TextInput, Toggle } from '../../../components/ui';
import { useReport } from '../../../state/reportStore';
import { useUi } from '../../../state/uiStore';
import { freeName, queryNames, saveParameter } from '../../../state/commands';
import { DialogFooter } from './DialogFooter';

export const PARAM_TYPES: { value: ParamType; label: string }[] = [
  { value: 'string', label: 'Texto' },
  { value: 'int', label: 'Inteiro' },
  { value: 'float', label: 'Decimal' },
  { value: 'bool', label: 'Booleano' },
  { value: 'date', label: 'Data' },
  { value: 'datetime', label: 'Data/hora' },
];

/** Propriedades do Parâmetro: edita um rascunho; Ok grava num único passo de desfazer. */
export function ParameterDialog({ target, onClose }: { target?: string; onClose: () => void }) {
  const report = useReport((s) => s.report);
  const fields = useUi((s) => s.fields);
  const [p, setP] = useState<ParameterDef>(() => {
    const cur = report.parameters.find((x) => x.name === target);
    if (cur) return structuredClone(cur);
    const name = freeName(report.parameters.map((x) => x.name), 'param', report.parameters.length + 1);
    return { name, type: 'string', label: `Parâmetro ${name.slice(5)}`, required: false };
  });
  const patch = (x: Partial<ParameterDef>) => setP((cur) => ({ ...cur, ...x }));

  const queries = queryNames(report);
  const others = report.parameters.filter((x) => x.name !== target);
  const av = p.availableValues;
  const known = av ? fields[av.query] ?? [] : [];
  const shownDefault = p.defaultValue === undefined ? '' : Array.isArray(p.defaultValue) ? p.defaultValue.join(',') : String(p.defaultValue);
  const error = !p.name ? 'Informe o nome' : others.some((x) => x.name === p.name) ? `Já existe um parâmetro "${p.name}"` : undefined;

  const ok = () => {
    const clean: ParameterDef = { ...p };
    if (!clean.multiValue) delete clean.multiValue;
    if (!clean.dependsOn?.length) delete clean.dependsOn;
    // dependências para parâmetros removidos ou renomeados para si mesmo não sobrevivem
    if (clean.dependsOn) clean.dependsOn = clean.dependsOn.filter((n) => n !== clean.name && others.some((o) => o.name === n));
    saveParameter(target, clean);
    onClose();
  };

  return (
    <Modal title={target ? `Propriedades do Parâmetro · ${target}` : 'Novo parâmetro'} onClose={onClose} footer={<DialogFooter error={error} onOk={ok} onCancel={onClose} />}>
      <div className="form">
        <div className="grid2">
          <Field label="Nome (usado como :nome na SQL)">
            <TextInput value={p.name} mono live onCommit={(v) => patch({ name: v.replace(/[^A-Za-z0-9_]/g, '_') })} />
          </Field>
          <Field label="Tipo">
            <Select value={p.type} options={PARAM_TYPES} onChange={(type) => patch({ type, multiValue: type === 'bool' ? undefined : p.multiValue })} />
          </Field>
          <Field label="Rótulo">
            <TextInput value={p.label} live onCommit={(label) => patch({ label })} />
          </Field>
          <Field label="Valor padrão" hint={p.multiValue ? 'Separe os valores por vírgula' : 'Para datas: today, yesterday, AAAA-MM-DD'}>
            <TextInput
              value={shownDefault}
              live
              placeholder={p.multiValue ? 'ex.: 1,2,3' : 'ex.: today'}
              onCommit={(v) => patch({ defaultValue: v === '' ? undefined : v })}
            />
          </Field>
        </div>
        <div className="row gap">
          <Toggle label="Obrigatório" checked={p.required} onChange={(required) => patch({ required })} />
          {p.type !== 'bool' && <Toggle label="Vários valores" checked={!!p.multiValue} onChange={(v) => patch({ multiValue: v || undefined })} />}
        </div>
        <Field label="Lista de valores" hint="Query do relatório que fornece os valores permitidos">
          <Select
            value={av?.query ?? ''}
            options={[{ value: '', label: '(digitação livre)' }, ...queries.map((q) => ({ value: q, label: q }))]}
            onChange={(q) => {
              if (!q) return patch({ availableValues: undefined });
              const cols = fields[q] ?? [];
              patch({
                availableValues: {
                  query: q,
                  valueField: av?.valueField || cols[0]?.name || '',
                  labelField: av?.labelField || cols[1]?.name,
                },
              });
            }}
          />
        </Field>
        {av && (
          <>
            <div className="grid2">
              <Field label="Campo de valor">
                {known.length ? (
                  <Select value={av.valueField} options={known.map((f) => f.name)} onChange={(valueField) => patch({ availableValues: { ...av, valueField } })} />
                ) : (
                  <TextInput value={av.valueField} mono live placeholder="Id" onCommit={(valueField) => patch({ availableValues: { ...av, valueField } })} />
                )}
              </Field>
              <Field label="Campo de rótulo">
                {known.length ? (
                  <Select
                    value={av.labelField ?? ''}
                    options={[{ value: '', label: '(o valor)' }, ...known.map((f) => ({ value: f.name, label: f.name }))]}
                    onChange={(v) => patch({ availableValues: { ...av, labelField: v || undefined } })}
                  />
                ) : (
                  <TextInput value={av.labelField ?? ''} mono live placeholder="(o valor)" onCommit={(v) => patch({ availableValues: { ...av, labelField: v || undefined } })} />
                )}
              </Field>
            </div>
            {!known.length && <p className="muted small">Carregue os campos de {av.query} na árvore para escolher as colunas numa lista.</p>}
          </>
        )}
        {others.length > 0 && (
          <Field label="Depende de (cascata)" hint="Ao mudar um destes, a lista é recarregada e o valor escolhido é limpo">
            <div className="row gap">
              {others.map((o) => (
                <Toggle
                  key={o.name}
                  label={o.label || o.name}
                  checked={!!p.dependsOn?.includes(o.name)}
                  onChange={(on) => {
                    const set = new Set(p.dependsOn ?? []);
                    if (on) set.add(o.name);
                    else set.delete(o.name);
                    patch({ dependsOn: set.size ? [...set] : undefined });
                  }}
                />
              ))}
            </div>
          </Field>
        )}
        <p className="muted small mono">
          {p.multiValue ? `IN (:${p.name}) na SQL · JOIN(Params.${p.name}, ", ")` : `:${p.name} na SQL · Params.${p.name}`} nas expressões
          {av?.labelField && ` · rótulo: ${p.multiValue ? `JOIN(ParamLabels.${p.name}, ", ")` : `ParamLabels.${p.name}`}`}
        </p>
      </div>
    </Modal>
  );
}
