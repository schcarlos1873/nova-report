// Monta os nós da árvore "Dados do Relatório" a partir da definição (função pura, sem React).
import type { MenuItem } from '../../components/ContextMenu';
import type { TreeNode } from '../../components/TreeView';
import type { ImageResource, ParameterDef, ReportDefinition } from '../../domain/types';
import type { FieldInfo } from '../../state/uiStore';
import { beginDrag, endDrag, isDate, isNumeric, type DropPayload } from '../designer/dnd';

/** Campos internos: atalhos somente leitura (não podem ser criados, alterados nem excluídos). */
export const INTERNAL_FIELDS: { expression: string; hint: string; format?: string }[] = [
  { expression: 'TODAY()', hint: 'Data atual', format: 'date:%d/%m/%Y' },
  { expression: 'NOW()', hint: 'Data e hora atuais', format: 'date:%d/%m/%Y %H:%M' },
  { expression: 'RECORD_NUMBER()', hint: 'Número do registro (reinicia a cada grupo)' },
  { expression: 'PAGE_NUMBER()', hint: 'Página atual' },
  { expression: 'TOTAL_PAGES()', hint: 'Total de páginas' },
  { expression: '"Página " + PAGE_NUMBER() + " de " + TOTAL_PAGES()', hint: 'Página N de M' },
  { expression: 'Report.Title', hint: 'Título do relatório' },
  { expression: 'Report.Description', hint: 'Descrição do relatório' },
  { expression: 'Report.Author', hint: 'Autor do relatório' },
  { expression: 'Report.FileName', hint: 'Nome do arquivo .nrep' },
  { expression: 'Report.FilePath', hint: 'Caminho completo do arquivo .nrep' },
  { expression: 'Report.RecordCount', hint: 'Total de registros do dataset principal' },
  { expression: 'Report.ExecutionTime', hint: 'Início da geração', format: 'date:%d/%m/%Y %H:%M' },
  { expression: 'USER_NAME()', hint: 'Usuário do sistema que gerou o relatório' },
];

/** Ações que a árvore dispara; implementadas pelo painel (diálogos e comandos). */
export interface TreeActions {
  addParameter: () => void;
  editParameter: (name: string) => void;
  removeParameter: (name: string) => void;
  addImage: () => void;
  editImage: (name: string) => void;
  removeImage: (name: string) => void;
  addDataSource: () => void;
  editDataSource: (id: string) => void;
  removeDataSource: (id: string) => void;
  openConnections: () => void;
  addDataset: (dataSource?: string) => void;
  editDataset: (name: string) => void;
  removeDataset: (name: string) => void;
  runDataset: (name: string) => void;
  refreshFields: (name?: string) => void;
  addGroup: () => void;
  editGroup: (name: string) => void;
  removeGroup: (name: string) => void;
  moveGroupUp: (name: string) => void;
}

const drag = (payload: DropPayload) => ({
  onDragStart: (e: React.DragEvent) => beginDrag(e, payload),
  onDragEnd: endDrag,
});

const typeGlyph = (t?: string) => (isNumeric(t) ? '#' : isDate(t) ? '◷' : 'a');

function paramExpression(p: ParameterDef) {
  return p.multiValue ? `JOIN(Params.${p.name}, ", ")` : `Params.${p.name}`;
}

const PARAM_FORMAT: Partial<Record<ParameterDef['type'], string>> = {
  date: 'date:%d/%m/%Y',
  datetime: 'date:%d/%m/%Y %H:%M',
};

export function imageLabel(img: ImageResource) {
  const ext = img.mimeType?.split('/')[1]?.replace('svg+xml', 'svg').replace('jpeg', 'jpg')
    ?? (img.kind === 'file' || img.kind === 'url' ? img.value?.split(/[?#]/)[0].match(/\.([a-z0-9]{1,5})$/i)?.[1]?.toLowerCase() : undefined);
  const kind = { file: 'local', url: 'url', embedded: 'embutida', database: 'banco' }[img.kind];
  return ext ? `${ext} · ${kind}` : kind;
}

export function buildTree(report: ReportDefinition, fields: Record<string, FieldInfo[]>, a: TreeActions): TreeNode[] {
  const detailQuery = report.bands.find((b) => b.type === 'detail')?.dataSource;
  const groups = report.groups ?? [];
  const images = report.images ?? [];
  const hasQueries = report.dataSources.some((d) => d.queries.length);

  const internal: TreeNode = {
    id: 'internal',
    label: 'Campos Internos',
    icon: '⚙',
    defaultOpen: false,
    children: INTERNAL_FIELDS.map((f) => ({
      id: `internal:${f.expression}`,
      label: <span className="mono">{f.expression}</span>,
      title: `${f.hint} — arraste para o relatório`,
      icon: 'ƒ',
      ...drag({ expression: f.expression, format: f.format, label: f.expression }),
    })),
  };

  const params: TreeNode = {
    id: 'params',
    label: 'Parâmetros',
    icon: '⌥',
    menu: [{ label: 'Adicionar Parâmetro', icon: '+', onSelect: a.addParameter }],
    onActivate: a.addParameter,
    children: report.parameters.map((p) => ({
      id: `param:${p.name}`,
      label: <span className="mono">{p.name}</span>,
      title: `${p.label || p.name} — arraste para exibir o valor no relatório`,
      icon: '◇',
      detail: p.multiValue ? `${p.type}[]` : p.type,
      onActivate: () => a.editParameter(p.name),
      onDelete: () => a.removeParameter(p.name),
      menu: [
        { label: 'Propriedades do Parâmetro', icon: '✎', onSelect: () => a.editParameter(p.name), shortcut: '↵' },
        { label: 'Remover', icon: '✕', danger: true, separator: true, onSelect: () => a.removeParameter(p.name), shortcut: 'Del' },
      ],
      ...drag({ expression: paramExpression(p), format: p.multiValue ? undefined : PARAM_FORMAT[p.type], label: `Params.${p.name}` }),
    })),
  };

  const imgs: TreeNode = {
    id: 'images',
    label: 'Imagens',
    icon: '🖼',
    menu: [{ label: 'Adicionar Imagem', icon: '+', onSelect: a.addImage }],
    onActivate: a.addImage,
    children: images.map((img) => ({
      id: `image:${img.name}`,
      label: img.name,
      title: 'Arraste para o relatório',
      icon: '▣',
      detail: imageLabel(img),
      onActivate: () => a.editImage(img.name),
      onDelete: () => a.removeImage(img.name),
      menu: [
        { label: 'Propriedades da Imagem', icon: '✎', onSelect: () => a.editImage(img.name), shortcut: '↵' },
        { label: 'Remover', icon: '✕', danger: true, separator: true, onSelect: () => a.removeImage(img.name), shortcut: 'Del' },
      ],
      ...drag({ imageRef: img.name, label: img.name }),
    })),
  };

  const sources: TreeNode = {
    id: 'sources',
    label: 'Fonte de Dados',
    icon: '🗄',
    menu: [
      { label: 'Adicionar Fonte de Dados', icon: '+', onSelect: a.addDataSource },
      { label: 'Gerenciar Conexões…', icon: '🔌', separator: true, onSelect: a.openConnections },
    ],
    onActivate: a.addDataSource,
    children: report.dataSources.map((ds) => ({
      id: `ds:${ds.id}`,
      label: <span className="mono">{ds.id}</span>,
      icon: '⛁',
      detail: ds.connectionRef ?? ds.provider,
      title: `${ds.provider}${ds.connectionRef ? ` · conexão ${ds.connectionRef}` : ''}`,
      onActivate: () => a.editDataSource(ds.id),
      onDelete: () => a.removeDataSource(ds.id),
      menu: [
        { label: 'Propriedades da Fonte', icon: '✎', onSelect: () => a.editDataSource(ds.id), shortcut: '↵' },
        { label: 'Adicionar Conjunto de Dados', icon: '+', onSelect: () => a.addDataset(ds.id) },
        { label: 'Remover', icon: '✕', danger: true, separator: true, onSelect: () => a.removeDataSource(ds.id), shortcut: 'Del' },
      ],
    })),
  };

  const datasets: TreeNode = {
    id: 'datasets',
    label: 'Conjunto de Dados',
    icon: '☷',
    menu: [
      { label: 'Adicionar Conjunto de Dados', icon: '+', onSelect: () => a.addDataset(), disabled: !report.dataSources.length },
      { label: 'Atualizar todos os campos', icon: '⟳', separator: true, onSelect: () => a.refreshFields(), disabled: !hasQueries },
    ],
    onActivate: report.dataSources.length ? () => a.addDataset() : undefined,
    children: report.dataSources.flatMap((ds) =>
      ds.queries.map((q) => {
        const known = fields[q.name];
        const proc = q.kind === 'storedProcedure';
        return {
          id: `q:${q.name}`,
          label: q.name,
          icon: proc ? 'ƒ' : '▦',
          detail: proc ? 'procedure' : ds.id,
          title: proc ? `Stored procedure ${q.procedure ?? ''} em ${ds.id}` : `SQL em ${ds.id}`,
          defaultOpen: q.name === detailQuery,
          onActivate: () => a.editDataset(q.name),
          onDelete: () => a.removeDataset(q.name),
          menu: [
            { label: 'Propriedades do Conjunto de Dados', icon: '✎', onSelect: () => a.editDataset(q.name), shortcut: '↵' },
            { label: 'Executar', icon: '▶', onSelect: () => a.runDataset(q.name) },
            { label: 'Atualizar campos', icon: '⟳', onSelect: () => a.refreshFields(q.name) },
            { label: 'Remover', icon: '✕', danger: true, separator: true, onSelect: () => a.removeDataset(q.name), shortcut: 'Del' },
          ] satisfies MenuItem[],
          children: known
            ? known.map((f) => ({
                id: `q:${q.name}:f:${f.name}`,
                label: f.name,
                icon: <span className="field-type">{typeGlyph(f.dataType)}</span>,
                detail: f.dataType,
                title: `${f.dataType} — arraste para uma banda ou tabela (⌥ ao soltar: valor + rótulo)`,
                ...drag({ field: f.name, query: q.name, dataType: f.dataType }),
              }))
            : [
                {
                  id: `q:${q.name}:pending`,
                  label: 'Carregar campos…',
                  icon: '⟳',
                  muted: true,
                  title: 'Executa a query (1 linha) para listar os campos',
                  onActivate: () => a.refreshFields(q.name),
                },
              ],
        } satisfies TreeNode;
      }),
    ),
  };

  const grp: TreeNode = {
    id: 'groups',
    label: 'Grupos',
    icon: '⧉',
    defaultOpen: groups.length > 0,
    menu: [{ label: 'Adicionar Grupo', icon: '+', onSelect: a.addGroup }],
    onActivate: a.addGroup,
    children: groups.map((g, i) => ({
      id: `group:${g.name}`,
      label: <span className="mono">{g.name}</span>,
      icon: '⧉',
      detail: g.expression,
      onActivate: () => a.editGroup(g.name),
      onDelete: () => a.removeGroup(g.name),
      menu: [
        { label: 'Propriedades do Grupo', icon: '✎', onSelect: () => a.editGroup(g.name), shortcut: '↵' },
        { label: 'Subir (mais externo)', icon: '↑', disabled: i === 0, onSelect: () => a.moveGroupUp(g.name) },
        { label: 'Remover', icon: '✕', danger: true, separator: true, onSelect: () => a.removeGroup(g.name), shortcut: 'Del' },
      ],
    })),
  };

  return [internal, params, imgs, sources, datasets, grp];
}
