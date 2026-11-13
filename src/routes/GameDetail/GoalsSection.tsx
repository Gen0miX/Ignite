import { useState, type FormEvent } from 'react';
import { X } from 'lucide-react';
import { useGoalMutations, useGoals } from '../../api/hooks';
import buttons from '../../components/Button/Button.module.css';
import { strings } from '../../i18n';
import styles from './GameDetailPage.module.css';

const t = strings.game;

export function GoalsSection({ gameId }: { gameId: string }) {
  const { data: goals = [] } = useGoals(gameId);
  const m = useGoalMutations(gameId);
  const [title, setTitle] = useState('');

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;
    m.add.mutate(title, { onSuccess: () => setTitle('') });
  };

  return (
    <section className={styles.card} aria-labelledby="sheet-goals">
      <h2 id="sheet-goals" className="section-label">
        {t.goalsTitle}
      </h2>
      {goals.length === 0 && <p className={styles.muted}>{t.goalsEmpty}</p>}
      <ul className={styles.goals}>
        {goals.map((g) => (
          <li key={g.id} className={styles.goal}>
            <label>
              <input
                type="checkbox"
                checked={!!g.doneAt}
                onChange={(e) => m.toggle.mutate({ id: g.id, done: e.target.checked })}
              />
              <span className={g.doneAt ? styles.goalDone : undefined}>{g.title}</span>
            </label>
            <button
              type="button"
              className={styles.iconAction}
              aria-label={t.goalDelete(g.title)}
              onClick={() => m.remove.mutate(g.id)}
            >
              <X size={14} aria-hidden="true" />
            </button>
          </li>
        ))}
      </ul>
      <form className={styles.inlineForm} onSubmit={submit}>
        <label className="visually-hidden" htmlFor="goal-new">
          {t.goalPlaceholder}
        </label>
        <input
          id="goal-new"
          className={styles.input}
          placeholder={t.goalPlaceholder}
          maxLength={200}
          value={title}
          onChange={(e) => setTitle(e.target.value)}
        />
        <button
          type="submit"
          className={buttons.secondary}
          disabled={!title.trim() || m.add.isPending}
        >
          {t.goalAdd}
        </button>
      </form>
    </section>
  );
}
