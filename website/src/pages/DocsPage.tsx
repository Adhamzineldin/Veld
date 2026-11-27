import { useState, useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import { Menu, X } from 'lucide-react';
import V1Banner from '../components/V1Banner';
import styles from './DocsPage.module.css';
import { nav } from './docs/nav';
import GettingStarted from './docs/GettingStarted';
import Language from './docs/Language';
import Rules from './docs/Rules';
import EndpointRules from './docs/EndpointRules';
import StateMachines from './docs/StateMachines';
import Policies from './docs/Policies';
import RulesTesting from './docs/RulesTesting';
import RulesTools from './docs/RulesTools';
import UsingGenerated from './docs/UsingGenerated';
import Microservices from './docs/Microservices';
import ErrorsEquality from './docs/ErrorsEquality';
import Configuration from './docs/Configuration';
import CliReference from './docs/CliReference';
import Diagnostics from './docs/Diagnostics';
import Registry from './docs/Registry';
import Editors from './docs/Editors';
import Extending from './docs/Extending';

/** The docs page: a sidebar plus the section components in `./docs`. */
export default function DocsPage() {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [activeId, setActiveId] = useState('getting-started');
  const location = useLocation();

  useEffect(() => {
    if (location.hash) {
      const el = document.getElementById(location.hash.slice(1));
      if (el) el.scrollIntoView({ behavior: 'smooth' });
    }
  }, [location.hash]);

  useEffect(() => {
    const ids = nav.flatMap((s) => s.items.map((i) => i.id));
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setActiveId(entry.target.id);
            break;
          }
        }
      },
      { rootMargin: '-80px 0px -60% 0px', threshold: 0 },
    );
    for (const id of ids) {
      const el = document.getElementById(id);
      if (el) observer.observe(el);
    }
    return () => observer.disconnect();
  }, []);

  return (
    <>
      <V1Banner />
      <div className={styles.docsLayout}>
        <div
          className={`${styles.mobileOverlay} ${sidebarOpen ? styles.mobileOverlayOpen : ''}`}
          onClick={() => setSidebarOpen(false)}
        />

        <aside className={`${styles.sidebar} ${sidebarOpen ? styles.sidebarOpen : ''}`}>
          {nav.map((section) => (
            <div key={section.group} className={styles.sidebarGroup}>
              <div className={styles.sidebarGroupTitle}>{section.group}</div>
              {section.items.map((item) => (
                <a
                  key={item.id}
                  href={`#${item.id}`}
                  className={`${styles.sidebarLink} ${activeId === item.id ? styles.sidebarLinkActive : ''}`}
                  onClick={() => setSidebarOpen(false)}
                >
                  {item.label}
                </a>
              ))}
            </div>
          ))}
        </aside>

        <button
          className={styles.mobileSidebarToggle}
          onClick={() => setSidebarOpen(!sidebarOpen)}
          aria-label="Toggle docs sidebar"
        >
          {sidebarOpen ? <X size={22} /> : <Menu size={22} />}
        </button>

        <div className={styles.content}>
          <h1 className={styles.pageTitle}>
            Veld <span className={styles.pageTitleGradient}>Documentation</span>
          </h1>
          <p className={styles.pageSubtitle}>
            Veld 2.0: write a <code>.veld</code> contract once, generate typed servers and clients. Zero
            runtime dependencies, deterministic output, and files you wrote are never overwritten.
          </p>

          <GettingStarted />
          <Language />
          <Rules />
          <EndpointRules />
          <StateMachines />
          <Policies />
          <RulesTesting />
          <RulesTools />
          <UsingGenerated />
          <Microservices />
          <ErrorsEquality />
          <Configuration />
          <CliReference />
          <Registry />
          <Editors />
          <Extending />
          <Diagnostics />
        </div>
      </div>
    </>
  );
}
