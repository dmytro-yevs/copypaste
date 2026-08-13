import { useEffect, useState } from "react";

import type { Item } from "@/lib/ipc";
import { revealItem } from "@/lib/ipc";

export interface DetailBody {
  /** The whole body, or `null` while it is still coming and when there is no
   *  more of it than the row already shows. */
  readonly text: string | null;
  /** The read failed. Distinct from `text === null`, because the fallback is
   *  the row's *truncated* preview: rendering that silently presents a fragment
   *  as the whole clipping, in the one view whose purpose is to show all of it. */
  readonly failed: boolean;
}

export function useDetailBody(item: Item | null): DetailBody {
  const [loaded, setLoaded] = useState<{
    id: string;
    text: string | null;
  } | null>(null);

  const id =
    item !== null && item.truncated && !item.is_sensitive ? item.id : null;

  useEffect(() => {
    if (id === null) {
      setLoaded(null);
      return;
    }
    let alive = true;
    void revealItem(id)
      .then((text) => {
        if (alive) setLoaded({ id, text });
      })
      .catch(() => {
        if (alive) setLoaded({ id, text: null });
      });
    return () => {
      alive = false;
    };
  }, [id]);

  const current = loaded !== null && loaded.id === id ? loaded : null;
  return {
    text: current?.text ?? null,
    failed: current !== null && current.text === null,
  };
}
