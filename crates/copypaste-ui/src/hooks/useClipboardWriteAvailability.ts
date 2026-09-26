import { queryOptions, useQuery, type QueryClient } from "@tanstack/react-query";

import {
  getClipboardWriteAvailability,
  type ClipboardWriteAvailability,
  type ClipboardWriteMode,
} from "@/lib/ipc";

export function clipboardWriteAvailabilityOptions(
  contentType: string,
  mode: ClipboardWriteMode = "original",
) {
  return queryOptions({
    queryKey: ["clipboard-write-availability", contentType, mode] as const,
    queryFn: () => getClipboardWriteAvailability(contentType, mode),
    staleTime: Infinity,
    retry: false,
  });
}

export function useClipboardWriteAvailability(
  contentType: string | null,
  mode: ClipboardWriteMode = "original",
) {
  return useQuery({
    ...clipboardWriteAvailabilityOptions(contentType ?? "", mode),
    enabled: contentType !== null,
  });
}

export async function requireClipboardWriteAvailability(
  client: QueryClient,
  contentType: string,
  mode: ClipboardWriteMode = "original",
): Promise<ClipboardWriteAvailability> {
  return client.fetchQuery(clipboardWriteAvailabilityOptions(contentType, mode));
}
