import { useMemo, useState } from 'react';
import { ChevronDown, ChevronRight, FileText, Folder } from 'lucide-react';
import styles from './FileTree.module.css';

interface Node {
  name: string;
  path: string;
  children: Map<string, Node>;
  file: boolean;
}

function build(paths: string[]): Node {
  const root: Node = { name: '', path: '', children: new Map(), file: false };
  for (const p of paths) {
    let cur = root;
    const parts = p.split('/');
    parts.forEach((part, i) => {
      const path = parts.slice(0, i + 1).join('/');
      let next = cur.children.get(part);
      if (!next) {
        next = { name: part, path, children: new Map(), file: i === parts.length - 1 };
        cur.children.set(part, next);
      }
      cur = next;
    });
  }
  return root;
}

/** Directories first, then files, each by name. */
function sorted(node: Node): Node[] {
  return [...node.children.values()].sort((a, b) => Number(a.file) - Number(b.file) || a.name.localeCompare(b.name));
}

interface Props {
  paths: string[];
  selected: string | null;
  onSelect: (path: string) => void;
}

export default function FileTree({ paths, selected, onSelect }: Props) {
  const root = useMemo(() => build(paths), [paths]);
  // Large outputs start with deeper directories collapsed; the user's clicks override that.
  const [toggled, setToggled] = useState<Record<string, boolean>>({});
  const big = paths.length > 24;

  const isOpen = (node: Node, depth: number): boolean => {
    if (selected?.startsWith(`${node.path}/`)) return toggled[node.path] ?? true;
    return toggled[node.path] ?? (!big || depth < 2);
  };

  function render(node: Node, depth: number) {
    return sorted(node).map((child) => {
      if (child.file) {
        return (
          <li key={child.path}>
            <button
              type="button"
              className={`${styles.row} ${selected === child.path ? styles.active : ''}`}
              style={{ paddingLeft: 10 + depth * 14 }}
              onClick={() => onSelect(child.path)}
              title={child.path}
            >
              <FileText size={13} aria-hidden="true" />
              <span>{child.name}</span>
            </button>
          </li>
        );
      }
      const open = isOpen(child, depth);
      return (
        <li key={child.path}>
          <button
            type="button"
            className={styles.row}
            style={{ paddingLeft: 10 + depth * 14 }}
            aria-expanded={open}
            onClick={() => setToggled((t) => ({ ...t, [child.path]: !open }))}
          >
            {open ? <ChevronDown size={13} aria-hidden="true" /> : <ChevronRight size={13} aria-hidden="true" />}
            <Folder size={13} aria-hidden="true" />
            <span>{child.name}</span>
          </button>
          {open && <ul className={styles.list}>{render(child, depth + 1)}</ul>}
        </li>
      );
    });
  }

  return (
    <ul className={styles.list} role="tree" aria-label="Generated files">
      {render(root, 0)}
    </ul>
  );
}
