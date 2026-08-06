import { useEffect, useState } from "react";

import type { Item } from "@/lib/ipc";
import { revealItem } from "@/lib/ipc";

export function useDetailBody(item: Item | null): string | null {
  const [loaded, setLoaded] = useState<{ id: string; text: string } | null>(
    null,
  );

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
        if (alive) setLoaded(null);
      });
    return () => {
      alive = false;
    };
  }, [id]);

  return loaded !== null && loaded.id === id ? loaded.text : null;
}
