import type { ElementType } from '../../domain/types';
import { beginDrag, endDrag } from '../designer/dnd';
import { insertElement } from '../../state/commands';

const TOOLS: { type: ElementType; label: string; icon: string; hint: string }[] = [
  { type: 'label', label: 'Texto', icon: 'T', hint: 'Texto estático. Use {PAGE} e {PAGES} para paginação.' },
  { type: 'expression', label: 'Campo / Expressão', icon: 'ƒx', hint: 'Expressão Rhai: Fields.x, SUM("x"), IIF(...)' },
  { type: 'table', label: 'Tabela', icon: '▦', hint: 'Tabela com quebra de página e cabeçalho repetido' },
  { type: 'image', label: 'Imagem', icon: '🖼', hint: 'PNG, JPG ou SVG' },
  { type: 'line', label: 'Linha', icon: '╱', hint: 'Linha horizontal, vertical ou diagonal' },
  { type: 'rectangle', label: 'Retângulo', icon: '▭', hint: 'Caixa com borda, fundo e cantos arredondados' },
];

export function Toolbox() {
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
            onDragStart={(e) => beginDrag(e, { elementType: t.type })}
            onDragEnd={endDrag}
            onClick={() => insertElement(t.type)}
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
          <li><kbd>⌘C</kbd> <kbd>⌘X</kbd> <kbd>⌘V</kbd> <kbd>⌘D</kbd> copiar, recortar, colar, duplicar</li>
          <li><kbd>⌘A</kbd> selecionar tudo (na banda selecionada)</li>
          <li><kbd>Setas</kbd> mover 1 grade (<kbd>⇧</kbd> ×5)</li>
          <li><kbd>Del</kbd> excluir · <kbd>Esc</kbd> limpar seleção</li>
          <li><kbd>⌘S</kbd> salvar · <kbd>⌘P</kbd> pré-visualizar</li>
          <li><kbd>⌘+</kbd> <kbd>⌘−</kbd> <kbd>⌘0</kbd> zoom · <kbd>⌃F1</kbd> recolher Ribbon</li>
          <li>Campo + <kbd>⌥</kbd> ao soltar: valor + rótulo de coluna</li>
        </ul>
      </div>
    </div>
  );
}
