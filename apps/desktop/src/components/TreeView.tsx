import { useCallback, useState, type ReactNode } from 'react';
import { ContextMenu, type MenuItem } from './ContextMenu';

export interface TreeNode {
  id: string;
  label: ReactNode;
  /** Texto para leitores de tela e dicas (quando `label` não é texto simples). */
  title?: string;
  icon?: ReactNode;
  /** Informação secundária à direita do rótulo (ex.: tipo do campo). */
  detail?: ReactNode;
  children?: TreeNode[];
  /** Nó raiz/agrupador começa aberto? (padrão: sim) */
  defaultOpen?: boolean;
  menu?: MenuItem[];
  /** Ação de Enter / duplo clique (normalmente "Propriedades"). */
  onActivate?: () => void;
  /** Ação da tecla Delete. */
  onDelete?: () => void;
  onDragStart?: (e: React.DragEvent) => void;
  onDragEnd?: () => void;
  muted?: boolean;
}

interface Props {
  nodes: TreeNode[];
  expanded: Record<string, boolean>;
  onToggle: (id: string, open: boolean) => void;
  label: string;
}

/** Árvore acessível (role=tree) com expandir/recolher, teclado, menu de contexto e arrastar. */
export function TreeView({ nodes, expanded, onToggle, label }: Props) {
  const [selected, setSelected] = useState<string>();
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] }>();
  const isOpen = useCallback((n: TreeNode) => expanded[n.id] ?? n.defaultOpen ?? true, [expanded]);

  // ordem visível, para a navegação por setas
  const visible: { node: TreeNode; parent?: TreeNode }[] = [];
  const walk = (list: TreeNode[], parent?: TreeNode) => {
    for (const n of list) {
      visible.push({ node: n, parent });
      if (n.children?.length && isOpen(n)) walk(n.children, n);
    }
  };
  walk(nodes);

  const focus = (id: string) => {
    setSelected(id);
    requestAnimationFrame(() => document.querySelector<HTMLElement>(`[data-tree-id="${CSS.escape(id)}"]`)?.focus());
  };

  const openMenu = (n: TreeNode, x: number, y: number) => {
    setSelected(n.id);
    if (n.menu?.length) setMenu({ x, y, items: n.menu });
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const i = visible.findIndex((v) => v.node.id === selected);
    if (i < 0) return;
    const { node, parent } = visible[i];
    const hasKids = !!node.children?.length;
    switch (e.key) {
      case 'ArrowDown': e.preventDefault(); if (visible[i + 1]) focus(visible[i + 1].node.id); break;
      case 'ArrowUp': e.preventDefault(); if (visible[i - 1]) focus(visible[i - 1].node.id); break;
      case 'ArrowRight':
        e.preventDefault();
        if (hasKids && !isOpen(node)) onToggle(node.id, true);
        else if (hasKids) focus(node.children![0].id);
        break;
      case 'ArrowLeft':
        e.preventDefault();
        if (hasKids && isOpen(node)) onToggle(node.id, false);
        else if (parent) focus(parent.id);
        break;
      case 'Enter': e.preventDefault(); node.onActivate?.(); break;
      case 'Delete':
      case 'Backspace': if (node.onDelete) { e.preventDefault(); node.onDelete(); } break;
      case 'ContextMenu': {
        e.preventDefault();
        const r = (e.target as HTMLElement).getBoundingClientRect();
        openMenu(node, r.left + 24, r.bottom);
        break;
      }
      case 'F10': if (e.shiftKey) { e.preventDefault(); const r = (e.target as HTMLElement).getBoundingClientRect(); openMenu(node, r.left + 24, r.bottom); } break;
    }
    // não deixa as teclas chegarem aos atalhos globais do designer (Delete apagaria a seleção do canvas)
    e.stopPropagation();
  };

  const render = (list: TreeNode[], depth: number): ReactNode =>
    list.map((n) => {
      const hasKids = !!n.children?.length;
      const open = hasKids && isOpen(n);
      return (
        <li key={n.id} role="treeitem" aria-expanded={hasKids ? open : undefined} aria-selected={selected === n.id} aria-label={n.title}>
          <div
            className={`rtree-row${selected === n.id ? ' selected' : ''}${n.muted ? ' muted' : ''}`}
            style={{ paddingLeft: 4 + depth * 14 }}
            data-tree-id={n.id}
            tabIndex={selected === n.id || (!selected && n === nodes[0]) ? 0 : -1}
            title={n.title}
            draggable={!!n.onDragStart}
            onDragStart={n.onDragStart}
            onDragEnd={n.onDragEnd}
            onClick={() => setSelected(n.id)}
            onDoubleClick={() => (n.onActivate ? n.onActivate() : hasKids && onToggle(n.id, !open))}
            onContextMenu={(e) => {
              e.preventDefault();
              openMenu(n, e.clientX, e.clientY);
            }}
            onFocus={() => setSelected(n.id)}
          >
            {hasKids ? (
              <button
                className="rtree-chevron"
                tabIndex={-1}
                aria-label={open ? 'Recolher' : 'Expandir'}
                onClick={(e) => {
                  e.stopPropagation();
                  onToggle(n.id, !open);
                }}
              >
                {open ? '▾' : '▸'}
              </button>
            ) : (
              <span className="rtree-chevron" />
            )}
            {n.icon !== undefined && <span className="rtree-icon" aria-hidden>{n.icon}</span>}
            <span className="rtree-label">{n.label}</span>
            {n.detail !== undefined && <span className="rtree-detail">{n.detail}</span>}
          </div>
          {open && <ul role="group">{render(n.children!, depth + 1)}</ul>}
        </li>
      );
    });

  return (
    <>
      <ul className="rtree" role="tree" aria-label={label} onKeyDown={onKeyDown}>
        {render(nodes, 0)}
      </ul>
      {menu && <ContextMenu {...menu} onClose={() => setMenu(undefined)} />}
    </>
  );
}
