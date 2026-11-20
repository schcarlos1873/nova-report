import type { ElementType } from '../../domain/types';
import { DND_MIME, type DropPayload } from '../designer/DesignerCanvas';
import { createElement } from '../../domain/factory';
import { useReport } from '../../state/reportStore';
import { useUi } from '../../state/uiStore';
import { sortBands } from '../../domain/geometry';

const TOOLS: { type: ElementType; label: string; icon: string; hint: string }[] = [
  { type: 'label', label: 'Texto', icon: 'T', hint: 'Texto estático. Use {PAGE} e {PAGES} para paginação.' },
  { type: 'expression', label: 'Campo / Expressão', icon: 'ƒx', hint: 'Expressão Rhai: Fields.x, SUM("x"), IIF(...)' },
  { type: 'table', label: 'Tabela', icon: '▦', hint: 'Tabela com quebra de página e cabeçalho repetido' },
  { type: 'image', label: 'Imagem', icon: '🖼', hint: 'PNG, JPG ou SVG' },
  { type: 'line', label: 'Linha', icon: '╱', hint: 'Linha horizontal, vertical ou diagonal' },
  { type: 'rectangle', label: 'Retângulo', icon: '▭', hint: 'Caixa com borda, fundo e cantos arredondados' },
];

export function Toolbox() {
  const update = useReport((s) => s.update);
  const { selectedBandId, select } = useUi();

  /** Clique adiciona na banda selecionada (ou na de detalhe). */
  const add = (type: ElementType) => {
    const r = useReport.getState().report;
    const band = r.bands.find((b) => b.id === selectedBandId) ?? sortBands(r).find((b) => b.type === 'detail') ?? r.bands[0];
    if (!band) return;
    const el = createElement(r, type, 0, 0);
    update((d) => {
      d.bands.find((b) => b.id === band.id)?.elements.push(el);
    });
    select([el.id], band.id);
  };

  return (
    <div className="toolbox">
      <p className="muted small">Arraste para uma banda ou clique para inserir na banda selecionada.</p>
      <div className="tool-grid">
        {TOOLS.map((t) => (
          <button
            key={t.type}
            className="tool"
            title={t.hint}
            draggable
            onDragStart={(e) => {
              const payload: DropPayload = { elementType: t.type };
              e.dataTransfer.setData(DND_MIME, JSON.stringify(payload));
              e.dataTransfer.effectAllowed = 'copy';
            }}
            onClick={() => add(t.type)}
          >
            <span className="tool-icon">{t.icon}</span>
            <span>{t.label}</span>
          </button>
        ))}
      </div>
      <div className="help-box">
        <strong>Atalhos</strong>
        <ul>
          <li><kbd>⌘Z</kbd> / <kbd>⇧⌘Z</kbd> desfazer / refazer</li>
          <li><kbd>⌘C</kbd> <kbd>⌘V</kbd> <kbd>⌘D</kbd> copiar, colar, duplicar</li>
          <li><kbd>Setas</kbd> mover 1 grade (<kbd>⇧</kbd> ×5)</li>
          <li><kbd>Del</kbd> excluir · <kbd>Esc</kbd> limpar seleção</li>
          <li><kbd>⌘S</kbd> salvar · <kbd>⌘P</kbd> pré-visualizar</li>
        </ul>
      </div>
    </div>
  );
}
