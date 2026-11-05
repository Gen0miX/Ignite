import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { Check } from 'lucide-react';
import {
  commands,
  isTauri,
  unwrap,
  type ImportCandidate,
  type ImportReport,
  type ImportScan,
  type SourceCard,
  type SourceKind,
} from '../../api/client';
import { useSourcesStatus } from '../../api/hooks';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { PageMain } from '../../app/AppShell';
import buttons from '../../components/Button/Button.module.css';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { SourceBadge, SourceChip } from '../../components/SourceBadge/SourceBadge';
import { fr } from '../../i18n/fr';
import { formatBytes, formatPlaytimeLong } from '../../lib/format';
import {
  buildDecisions,
  initialSelection,
  summary,
  type Selection,
} from '../../lib/importDecisions';
import styles from './ImportPage.module.css';

const t = fr.import;
type Step = 1 | 2 | 3 | 4;

function Stepper({ step }: { step: Step }) {
  return (
    <ol className={styles.stepper} aria-label={t.stepsLabel}>
      {t.steps.map((label, i) => {
        const n = (i + 1) as Step;
        const state = n < step ? styles.stepDone : n === step ? styles.stepCurrent : '';
        return (
          <li
            key={label}
            className={`${styles.step} ${state}`}
            aria-current={n === step ? 'step' : undefined}
          >
            <span className={styles.stepNumber}>
              {n < step ? <Check size={14} aria-hidden="true" /> : n}
            </span>
            {label}
          </li>
        );
      })}
    </ol>
  );
}

function SourcesStep({
  cards,
  chosen,
  onToggle,
  onScan,
  scanning,
}: {
  cards: SourceCard[];
  chosen: Set<SourceKind>;
  onToggle: (k: SourceKind) => void;
  onScan: () => void;
  scanning: boolean;
}) {
  return (
    <section className={styles.panel} aria-labelledby="import-sources">
      <h2 id="import-sources" className="section-label">
        {t.chooseSources}
      </h2>
      <ul className={styles.sourceChoices}>
        {cards.map((card) => {
          const installed = card.status.state !== 'not_installed';
          return (
            <li key={card.kind}>
              <label className={`${styles.sourceChoice} ${installed ? '' : styles.disabled}`}>
                <input
                  type="checkbox"
                  checked={chosen.has(card.kind)}
                  disabled={!installed}
                  onChange={() => onToggle(card.kind)}
                />
                <SourceChip kind={card.kind} />
                <span className={styles.sourceName}>{fr.sources[card.kind]}</span>
                <span className={styles.muted}>
                  {installed ? (card.enabled ? t.alreadySynced : t.notYetImported) : t.notInstalled}
                </span>
              </label>
            </li>
          );
        })}
      </ul>
      <p className={styles.muted}>{t.readOnlyNote}</p>
      <div>
        <button
          type="button"
          className={buttons.primary}
          disabled={chosen.size === 0 || scanning}
          onClick={onScan}
        >
          {scanning ? t.scanning : t.scan}
        </button>
      </div>
    </section>
  );
}

function Remark({
  c,
  sel,
  setSel,
}: {
  c: ImportCandidate;
  sel: Selection;
  setSel: (s: Selection) => void;
}) {
  if (c.state.kind === 'imported') return <span className={styles.muted}>{t.remark.imported}</span>;
  if (c.state.kind === 'duplicate') {
    const merge = !sel.keepSeparate.has(c.key);
    const id = `merge-${c.key}`;
    return (
      <label htmlFor={id} className={styles.mergeToggle}>
        <input
          id={id}
          type="checkbox"
          checked={merge}
          disabled={!sel.selected.has(c.key)}
          onChange={() => {
            const keep = new Set(sel.keepSeparate);
            if (merge) keep.add(c.key);
            else keep.delete(c.key);
            setSel({ ...sel, keepSeparate: keep });
          }}
        />
        <span className={styles.warnText}>{t.remark.duplicate(c.state.title)}</span>
      </label>
    );
  }
  if (c.note === 'notAGame') return <span className={styles.muted}>{t.remark.notAGame}</span>;
  if (c.note === 'previouslyIgnored')
    return <span className={styles.muted}>{t.remark.ignored}</span>;
  return null;
}

function DetectedStep({
  scan,
  sel,
  setSel,
  onBack,
  onCommit,
  committing,
  error,
}: {
  scan: ImportScan;
  sel: Selection;
  setSel: (s: Selection) => void;
  onBack: () => void;
  onCommit: () => void;
  committing: boolean;
  error: boolean;
}) {
  const [showImported, setShowImported] = useState(false);
  const imported = scan.candidates.filter((c) => c.state.kind === 'imported').length;
  const rows = scan.candidates.filter((c) => showImported || c.state.kind !== 'imported');
  const counts = summary(scan.candidates, sel);

  const toggle = (key: string) => {
    const selected = new Set(sel.selected);
    if (selected.has(key)) selected.delete(key);
    else selected.add(key);
    setSel({ ...sel, selected });
  };

  return (
    <div className={styles.detected}>
      <aside className={styles.scanned} aria-label={t.scannedSources}>
        {scan.sources.map((s) => (
          <div key={s.kind} className={styles.scannedSource}>
            <SourceChip kind={s.kind} />
            <div>
              <div className={styles.sourceName}>{fr.sources[s.kind]}</div>
              <div className={styles.muted}>
                {s.status.state === 'error'
                  ? t.scanError
                  : s.status.state === 'not_installed'
                    ? t.notInstalled
                    : t.detected(s.detected)}
              </div>
              {s.warnings.length > 0 && (
                <div className={styles.warnText}>{t.warnings(s.warnings.length)}</div>
              )}
            </div>
          </div>
        ))}
      </aside>

      <section className={styles.tableSection} aria-labelledby="import-detected">
        <div className={styles.tableHeader}>
          <h2 id="import-detected" className="section-label">
            {t.detectedTitle}
          </h2>
          {imported > 0 && (
            <label className={styles.inlineCheck}>
              <input
                type="checkbox"
                checked={showImported}
                onChange={(e) => setShowImported(e.target.checked)}
              />
              {t.showImported(imported)}
            </label>
          )}
        </div>
        {rows.length === 0 ? (
          <p className={styles.muted}>{t.nothingNew}</p>
        ) : (
          <div className={styles.tableWrap}>
            <table className={styles.table}>
              <thead>
                <tr>
                  <th scope="col">
                    <span className="visually-hidden">{t.col.select}</span>
                  </th>
                  <th scope="col">{t.col.game}</th>
                  <th scope="col">{t.col.source}</th>
                  <th scope="col">{t.col.state}</th>
                  <th scope="col">{t.col.time}</th>
                  <th scope="col">{t.col.achievements}</th>
                  <th scope="col">{t.col.remark}</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((c) => (
                  <tr key={c.key} className={sel.selected.has(c.key) ? '' : styles.unselected}>
                    <td>
                      <input
                        type="checkbox"
                        aria-label={t.selectGame(c.title)}
                        checked={sel.selected.has(c.key)}
                        disabled={c.state.kind === 'imported'}
                        onChange={() => toggle(c.key)}
                      />
                    </td>
                    <td className={styles.titleCell}>{c.title}</td>
                    <td>
                      <SourceBadge kind={c.source} />
                    </td>
                    <td className={styles.muted}>
                      {c.installed
                        ? `${t.installed}${c.sizeBytes ? ` · ${formatBytes(c.sizeBytes)}` : ''}`
                        : t.owned}
                    </td>
                    <td className="mono">
                      {c.playtimeMinutes ? formatPlaytimeLong(c.playtimeMinutes) : '—'}
                    </td>
                    <td className="mono">{c.achievements ?? '—'}</td>
                    <td>
                      <Remark c={c} sel={sel} setSel={setSel} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        <footer className={styles.footer}>
          <span className="mono">{t.footer(counts.toImport, counts.merges)}</span>
          {error && (
            <span role="alert" className={styles.errorText}>
              {t.commitError}
            </span>
          )}
          <div className={styles.footerActions}>
            <button
              type="button"
              className={buttons.secondary}
              onClick={onBack}
              disabled={committing}
            >
              {t.back}
            </button>
            <button
              type="button"
              className={buttons.primary}
              onClick={onCommit}
              disabled={committing}
            >
              {committing ? t.importing : t.continue}
            </button>
          </div>
        </footer>
      </section>
    </div>
  );
}

function ArtworkStep({ report, onNext }: { report: ImportReport; onNext: () => void }) {
  return (
    <section className={styles.panel} aria-labelledby="import-artwork">
      <h2 id="import-artwork" className="section-label">
        {t.artworkTitle}
      </h2>
      {report.withoutCover.length === 0 ? (
        <p>{t.allCovers}</p>
      ) : (
        <>
          <p>{t.missingCovers(report.withoutCover.length)}</p>
          <ul className={styles.missing}>
            {report.withoutCover.slice(0, 40).map((g) => (
              <li key={g.id}>
                <Link to={`/game/${g.id}`}>{g.title}</Link>
              </li>
            ))}
          </ul>
          <p className={styles.muted}>{t.coversHint}</p>
        </>
      )}
      <div>
        <button type="button" className={buttons.primary} onClick={onNext}>
          {t.continue}
        </button>
      </div>
    </section>
  );
}

function DoneStep({ report }: { report: ImportReport }) {
  return (
    <section className={styles.panel} aria-labelledby="import-done">
      <h2 id="import-done" className={styles.doneTitle}>
        {t.doneTitle}
      </h2>
      <p>{t.doneSummary(report.added, report.merged, report.skipped)}</p>
      <div>
        <Link to="/library" className={buttons.primary}>
          {t.toLibrary}
        </Link>
      </div>
    </section>
  );
}

export function ImportPage() {
  const client = useQueryClient();
  const sources = useSourcesStatus();
  const cards = useMemo(() => sources.data ?? [], [sources.data]);
  const [step, setStep] = useState<Step>(1);
  const [picked, setPicked] = useState<Set<SourceKind> | null>(null);
  const [scan, setScan] = useState<ImportScan | null>(null);
  const [sel, setSel] = useState<Selection>({ selected: new Set(), keepSeparate: new Set() });
  const [report, setReport] = useState<ImportReport | null>(null);

  // Default: every installed launcher.
  const chosen =
    picked ?? new Set(cards.filter((c) => c.status.state !== 'not_installed').map((c) => c.kind));

  const scanMutation = useMutation({
    mutationFn: () => unwrap(commands.importScan([...chosen])),
    onSuccess: (data) => {
      setScan(data);
      setSel(initialSelection(data.candidates));
      setStep(2);
    },
  });
  const commitMutation = useMutation({
    mutationFn: () =>
      unwrap(
        commands.importCommit({
          enable: [...chosen],
          decisions: buildDecisions(scan?.candidates ?? [], sel),
        }),
      ),
    onSuccess: (data) => {
      setReport(data);
      setStep(3);
      for (const queryKey of [['library'], ['counts'], ['sources'], ['duplicates'], ['game']]) {
        void client.invalidateQueries({ queryKey });
      }
    },
  });

  if (!isTauri()) {
    return (
      <PageMain>
        <PageHeader title={t.title} />
        <EmptyState>{fr.library.backendUnavailable}</EmptyState>
      </PageMain>
    );
  }

  return (
    <PageMain>
      <PageHeader title={t.title} />
      <Stepper step={step} />
      {step === 1 && (
        <SourcesStep
          cards={cards}
          chosen={chosen}
          onToggle={(k) => {
            const next = new Set(chosen);
            if (next.has(k)) next.delete(k);
            else next.add(k);
            setPicked(next);
          }}
          onScan={() => scanMutation.mutate()}
          scanning={scanMutation.isPending}
        />
      )}
      {step === 1 && scanMutation.isError && (
        <p role="alert" className={styles.errorText}>
          {t.scanFailed}
        </p>
      )}
      {step === 2 && scan && (
        <DetectedStep
          scan={scan}
          sel={sel}
          setSel={setSel}
          onBack={() => setStep(1)}
          onCommit={() => commitMutation.mutate()}
          committing={commitMutation.isPending}
          error={commitMutation.isError}
        />
      )}
      {step === 3 && report && <ArtworkStep report={report} onNext={() => setStep(4)} />}
      {step === 4 && report && <DoneStep report={report} />}
    </PageMain>
  );
}
