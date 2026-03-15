import { ArrowDown, ArrowLeft, ArrowRight } from 'lucide-react';
import styles from '../../pages/DocsPage.module.css';

type DocsPagerProps = {
  activeGroupLabel: string;
  previousGroupLabel: string | null;
  nextGroupLabel: string | null;
  nextSectionLabel: string | null;
  onPreviousGroup: () => void;
  onNextGroup: () => void;
  onNextSection: () => void;
};

export default function DocsPager({
  activeGroupLabel,
  previousGroupLabel,
  nextGroupLabel,
  nextSectionLabel,
  onPreviousGroup,
  onNextGroup,
  onNextSection,
}: DocsPagerProps) {
  return (
    <div className={styles.docsPagerWrap}>
      {nextSectionLabel && (
        <button type="button" className={styles.nextSectionBtn} onClick={onNextSection}>
          <ArrowDown size={16} />
          Next in {activeGroupLabel}: {nextSectionLabel}
        </button>
      )}

      <div className={styles.docsPager}>
        <button
          type="button"
          className={styles.docsPagerButton}
          onClick={onPreviousGroup}
          disabled={!previousGroupLabel}
        >
          <ArrowLeft size={16} />
          {previousGroupLabel ? `Previous: ${previousGroupLabel}` : 'No previous tab'}
        </button>

        <button
          type="button"
          className={styles.docsPagerButton}
          onClick={onNextGroup}
          disabled={!nextGroupLabel}
        >
          {nextGroupLabel ? `Next: ${nextGroupLabel}` : 'No next tab'}
          <ArrowRight size={16} />
        </button>
      </div>
    </div>
  );
}
