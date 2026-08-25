import { useEffect, useState } from "react";

import type { Item } from "@/lib/ipc";
import { getItemBody } from "@/lib/ipc";

export interface ItemBody {
  readonly text: string | null;
  readonly failed: boolean;
}

export function useItemBody(item: Item | null): ItemBody {
  const [loaded, setLoaded] = useState<{
    id: string;
    text: string | null;
  } | null>(null);
  const id = item?.truncated && !item.is_sensitive ? item.id : null;

  useEffect(() => {
    if (id === null) {
      setLoaded(null);
      return;
    }
    let alive = true;
    void getItemBody(id)
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

  const current = loaded?.id === id ? loaded : null;
  return {
    text: current?.text ?? null,
    failed: current !== null && current.text === null,
  };
}
