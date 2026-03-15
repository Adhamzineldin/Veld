import { ChevronDown, ChevronUp } from 'lucide-react';
import type { DocsNavGroup } from './docsNavigation';
import styles from '../../pages/DocsPage.module.css';

type DocsSidebarProps = {
  groups: DocsNavGroup[];
  activeGroup: string;
  activeId: string;
  expandedGroups: Record<string, boolean>;
  onToggleGroup: (group: string) => void;
  onSelectSection: (id: string) => void;
  closeMobile: () => void;
  className?: string;
};

export default function DocsSidebar({
  groups,
  activeGroup,
  activeId,
  expandedGroups,
  onToggleGroup,
  onSelectSection,
  closeMobile,
  className,
}: DocsSidebarProps) {
  return (
    <aside className={className}>
      {groups.map((section) => {
        const isActiveGroup = activeGroup === section.group;
        const isExpanded = expandedGroups[section.group] ?? isActiveGroup;

        return (
          <div key={section.group} className={styles.sidebarGroup}>
            <button
              type="button"
              className={`${styles.sidebarTab} ${isActiveGroup ? styles.sidebarTabActive : ''}`}
              onClick={() => onToggleGroup(section.group)}
              aria-expanded={isExpanded}
            >
              <span>{section.group}</span>
              {isExpanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
            </button>

            {isExpanded && (
              <div className={styles.sidebarDropdown}>
                {section.items.map((item) => (
                  <a
                    key={item.id}
                    href={`#${item.id}`}
                    className={`${styles.sidebarLink} ${activeId === item.id ? styles.sidebarLinkActive : ''}`}
                    onClick={(event) => {
                      event.preventDefault();
                      onSelectSection(item.id);
                      closeMobile();
                    }}
                  >
                    {item.label}
                  </a>
                ))}
              </div>
            )}
          </div>
        );
      })}
    </aside>
  );
}
