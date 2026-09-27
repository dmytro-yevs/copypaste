export const IMAGE_PREVIEW_KEY = ["image-preview"] as const;

export const imagePreviewKey = (id: string, maxEdge = 384) =>
  [...IMAGE_PREVIEW_KEY, id, maxEdge] as const;
