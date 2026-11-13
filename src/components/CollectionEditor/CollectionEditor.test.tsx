import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const ok = <T,>(data: T) => Promise.resolve({ status: 'ok' as const, data });
const preview = vi.fn(() => ok({ count: 197, sample: [] }));
const create = vi.fn((_name: string, _kind: string, rules: string | null) =>
  ok({ id: 'c1', name: 'Multi', kind: 'smart', rules, count: 197 }),
);

vi.mock('../../api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../api/client')>();
  return {
    ...actual,
    isTauri: () => true,
    commands: {
      ...actual.commands,
      facets: () =>
        ok({
          genres: [
            { name: 'Action', count: 213 },
            { name: 'RPG', count: 95 },
          ],
          modes: [
            { name: 'Solo', count: 243 },
            { name: 'Multijoueur', count: 197 },
          ],
        }),
      rulesPreview: preview,
      collectionCreate: create,
    },
  };
});

const { CollectionEditor } = await import('./CollectionEditor');

function renderEditor() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const onClose = vi.fn();
  render(
    <QueryClientProvider client={client}>
      <CollectionEditor collection={null} onClose={onClose} />
    </QueryClientProvider>,
  );
  return onClose;
}

describe('CollectionEditor', () => {
  it('offers the library genres and player modes for a genre rule', async () => {
    const user = userEvent.setup();
    renderEditor();
    await user.click(screen.getByRole('button', { name: /Ajouter une règle/ }));
    await user.selectOptions(screen.getByLabelText('Critère'), 'genre');

    const value = await screen.findByLabelText('Valeur');
    expect(within(value).getByRole('group', { name: 'Genres' })).toBeInTheDocument();
    expect(within(value).getByRole('group', { name: 'Modes de jeu' })).toBeInTheDocument();
    expect(within(value).getByRole('option', { name: 'Multijoueur (197)' })).toBeInTheDocument();

    await user.selectOptions(value, 'Multijoueur');
    await user.type(screen.getByLabelText(/Nom/), 'Multi');
    await user.click(screen.getByRole('button', { name: /Créer|Enregistrer/ }));
    const rules = create.mock.calls.at(-1)?.[2];
    expect(JSON.parse(rules!)).toEqual({
      all: [{ field: 'genre', op: 'in', value: ['Multijoueur'] }],
    });
  });

  it('lists only player modes for a players rule', async () => {
    const user = userEvent.setup();
    renderEditor();
    await user.click(screen.getByRole('button', { name: /Ajouter une règle/ }));
    await user.selectOptions(screen.getByLabelText('Critère'), 'mode');
    const value = await screen.findByLabelText('Valeur');
    expect(within(value).queryByRole('option', { name: 'Action (213)' })).toBeNull();
    expect(within(value).getByRole('option', { name: 'Solo (243)' })).toBeInTheDocument();
  });
});
