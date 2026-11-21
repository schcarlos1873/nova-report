import { useState } from 'react';
import type { ImageKind, ImageResource } from '../../../domain/types';
import { Field, Modal, Select, TextInput } from '../../../components/ui';
import { api } from '../../../infra/api';
import { dialogs } from '../../../infra/dialogs';
import { relativeToReport } from '../../../infra/paths';
import { useReport } from '../../../state/reportStore';
import { useUi } from '../../../state/uiStore';
import { freeName, queryNames, saveImage } from '../../../state/commands';
import { DialogFooter } from './DialogFooter';

const KINDS: { value: ImageKind; label: string; hint: string }[] = [
  { value: 'file', label: 'Local', hint: 'Arquivo no disco, relativo ao .nrep quando estiver na mesma pasta' },
  { value: 'url', label: 'URL', hint: 'Baixada a cada geração do relatório' },
  { value: 'embedded', label: 'Embed', hint: 'Gravada dentro do .nrep (até 5 MB)' },
  { value: 'database', label: 'Banco de dados', hint: 'Campo de uma query (BLOB, base64 ou data URI), lido a cada registro' },
];
const EXTENSIONS = ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'];
const formatBytes = (n: number) => (n < 1024 ? `${Math.round(n)} bytes` : `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`);

export function ImageDialog({ target, onClose }: { target?: string; onClose: () => void }) {
  const report = useReport((s) => s.report);
  const fields = useUi((s) => s.fields);
  const notify = useUi((s) => s.notify);
  const images = report.images ?? [];
  const [img, setImg] = useState<ImageResource>(() => {
    const cur = images.find((x) => x.name === target);
    return cur ? structuredClone(cur) : { name: freeName(images.map((x) => x.name), 'imagem'), kind: 'file', value: '' };
  });
  const patch = (x: Partial<ImageResource>) => setImg((cur) => ({ ...cur, ...x }));
  const queries = queryNames(report);
  const known = img.query ? fields[img.query] ?? [] : [];

  const error = !img.name.trim()
    ? 'Informe o nome'
    : images.some((x) => x.name === img.name && x.name !== target)
      ? `Já existe a imagem "${img.name}"`
      : img.kind === 'database'
        ? !img.query || !img.field ? 'Escolha a query e o campo' : undefined
        : !img.value?.trim()
          ? 'Informe a origem da imagem'
          : img.kind === 'url' && !/^https?:\/\//i.test(img.value)
            ? 'A URL deve começar com http:// ou https://'
            : undefined;

  const ok = () => {
    const clean: ImageResource =
      img.kind === 'database'
        ? { name: img.name, kind: 'database', query: img.query, field: img.field, mimeType: img.mimeType }
        : { name: img.name, kind: img.kind, value: img.value?.trim(), mimeType: img.kind === 'embedded' ? img.value?.match(/^data:([^;,]+)/)?.[1] : undefined };
    saveImage(target, clean);
    onClose();
  };

  const pick = async () => {
    const p = await dialogs.pickFile(EXTENSIONS);
    if (!p) return;
    const base = p.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '').replace(/\W+/g, '_');
    const name = !target && img.name.startsWith('imagem') && base ? base : img.name;
    if (img.kind === 'embedded') {
      try {
        patch({ value: await api.readFileDataUri(p), name });
      } catch (e) {
        notify((e as Error).message, 'error');
      }
    } else patch({ value: relativeToReport(p), name });
  };

  // miniatura: data URI e URL renderizam direto; arquivos locais dependem do caminho absoluto
  const thumb = img.kind === 'embedded' || img.kind === 'url' ? img.value : undefined;

  return (
    <Modal title={target ? `Propriedades da Imagem · ${target}` : 'Adicionar imagem'} onClose={onClose} footer={<DialogFooter error={error} onOk={ok} onCancel={onClose} />}>
      <div className="form">
        <Field label="Nome (referência image:nome)">
          <TextInput value={img.name} mono live onCommit={(name) => patch({ name: name.replace(/[^\w.-]/g, '_') })} />
        </Field>
        <div className="row gap" role="radiogroup" aria-label="Origem da imagem">
          {KINDS.map((k) => (
            <label key={k.value} className="toggle" title={k.hint}>
              <input
                type="radio"
                name="imgkind"
                checked={img.kind === k.value}
                // trocar de tipo descarta a origem anterior (um caminho não é uma data URI)
                onChange={() => patch({ kind: k.value, value: '', query: undefined, field: undefined })}
              />
              <span>{k.label}</span>
            </label>
          ))}
        </div>
        <p className="muted small">{KINDS.find((k) => k.value === img.kind)?.hint}</p>

        {img.kind === 'file' && (
          <Field label="Arquivo">
            <span className="row">
              <TextInput value={img.value ?? ''} mono live placeholder="imagens/logo.svg" onCommit={(value) => patch({ value })} />
              <button onClick={pick}>…</button>
            </span>
          </Field>
        )}
        {img.kind === 'url' && (
          <Field label="URL">
            <TextInput value={img.value ?? ''} mono live placeholder="https://exemplo.com/logo.png" onCommit={(value) => patch({ value })} />
          </Field>
        )}
        {img.kind === 'embedded' && (
          <Field label="Arquivo a embutir">
            <span className="row">
              <span className="muted small grow">{img.value ? `${img.value.match(/^data:([^;,]+)/)?.[1] ?? 'imagem'} · ${formatBytes(((img.value.length - img.value.indexOf(',') - 1) * 3) / 4)}` : 'Nenhum arquivo escolhido'}</span>
              <button onClick={pick}>Escolher…</button>
            </span>
          </Field>
        )}
        {img.kind === 'database' && (
          <div className="grid2">
            <Field label="Conjunto de dados">
              <Select value={img.query} options={queries} onChange={(query) => patch({ query, field: undefined })} />
            </Field>
            <Field label="Campo">
              {known.length ? (
                <Select value={img.field} options={known.map((f) => f.name)} onChange={(field) => patch({ field })} />
              ) : (
                <TextInput value={img.field ?? ''} mono live placeholder="foto" onCommit={(field) => patch({ field: field || undefined })} />
              )}
            </Field>
          </div>
        )}
        {thumb && <img className="img-thumb" src={thumb} alt={`Prévia de ${img.name}`} />}
      </div>
    </Modal>
  );
}
