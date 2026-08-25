export const IMAGE_PREVIEW_KEY = ["image-preview"] as const;

export const imagePreviewKey = (id: string) =>
  [...IMAGE_PREVIEW_KEY, id] as const;
