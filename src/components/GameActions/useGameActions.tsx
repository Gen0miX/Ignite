import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Eye, EyeOff, FileText, FolderPlus, ImageIcon, Play, Star, Trash2 } from 'lucide-react';
import type { BulkUpdate, GameSummary } from '../../api/client';
import { useBulkUpdate, useLaunchGame } from '../../api/hooks';
import { fr } from '../../i18n/fr';
import { isDeletable } from '../../lib/selection';
import { toast } from '../Toaster/toasts';
import { ArtworkPicker } from '../ArtworkPicker/ArtworkPicker';
import type { MenuItem } from '../Menu/Menu';
import { CollectionPicker } from './CollectionPicker';
import { DeleteGamesDialog } from './DeleteGamesDialog';

const t = fr.actions;

type Open =
  | { kind: 'images'; game: { id: string; title: string } }
  | { kind: 'collections'; ids: string[]; onAdded?: () => void }
  | { kind: 'delete'; ids: string[]; titles: string[]; onDeleted?: () => void };

/** Actions on a game (card menu, panel, selection bar) and the dialogs they open. */
export function useGameActions() {
  const [open, setOpen] = useState<Open | null>(null);
  const navigate = useNavigate();
  const launch = useLaunchGame();
  const bulk = useBulkUpdate();
  const close = () => setOpen(null);
  const run = (update: BulkUpdate, message: string, id: string) =>
    bulk.mutateAsync({ ids: [id], update }).then(
      () => toast(message, [id]),
      () => undefined,
    );

  const menuItems = (g: GameSummary): MenuItem[] => [
    {
      label: t.play,
      icon: <Play size={16} aria-hidden="true" />,
      onSelect: () => launch.mutate(g.id),
    },
    {
      label: t.openSheet,
      icon: <FileText size={16} aria-hidden="true" />,
      onSelect: () => navigate(`/game/${g.id}`),
    },
    {
      label: t.images,
      icon: <ImageIcon size={16} aria-hidden="true" />,
      onSelect: () => setOpen({ kind: 'images', game: g }),
    },
    {
      label: t.addToCollection,
      icon: <FolderPlus size={16} aria-hidden="true" />,
      onSelect: () => setOpen({ kind: 'collections', ids: [g.id] }),
    },
    {
      label: g.favorite ? t.favoriteRemove : t.favoriteAdd,
      icon: <Star size={16} aria-hidden="true" />,
      onSelect: () =>
        run(
          { favorite: !g.favorite },
          (g.favorite ? fr.toast.favoriteRemoved : fr.toast.favoriteAdded)(g.title),
          g.id,
        ),
      separator: true,
    },
    {
      label: g.hidden ? t.unhide : t.hide,
      icon: g.hidden ? (
        <Eye size={16} aria-hidden="true" />
      ) : (
        <EyeOff size={16} aria-hidden="true" />
      ),
      onSelect: () =>
        run({ hidden: !g.hidden }, (g.hidden ? fr.toast.unhidden : fr.toast.hidden)(1), g.id),
    },
    ...(isDeletable(g)
      ? [
          {
            label: t.delete,
            icon: <Trash2 size={16} aria-hidden="true" />,
            danger: true,
            onSelect: () => setOpen({ kind: 'delete', ids: [g.id], titles: [g.title] }),
          },
        ]
      : []),
  ];

  let element = null;
  if (open?.kind === 'images') {
    element = <ArtworkPicker gameId={open.game.id} title={open.game.title} onClose={close} />;
  } else if (open?.kind === 'collections') {
    element = <CollectionPicker gameIds={open.ids} onClose={close} onAdded={open.onAdded} />;
  } else if (open?.kind === 'delete') {
    element = (
      <DeleteGamesDialog
        ids={open.ids}
        titles={open.titles}
        onClose={close}
        onDeleted={open.onDeleted}
      />
    );
  }

  return { menuItems, open: setOpen, element };
}
