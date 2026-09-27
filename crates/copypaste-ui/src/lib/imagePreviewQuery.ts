export const IMAGE_PREVIEW_KEY = ["image-preview"] as const;

export const imagePreviewKey = (id: string, maxEdge?: number) =>
  maxEdge === undefined
    ? [...IMAGE_PREVIEW_KEY, id] as const
    : [...IMAGE_PREVIEW_KEY, id, maxEdge] as const;
