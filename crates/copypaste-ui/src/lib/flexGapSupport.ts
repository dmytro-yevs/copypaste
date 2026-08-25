export const FLEX_GAP_UNSUPPORTED_CLASS = "flexGapNotSupported";
export const FLEX_GAP_QA_QUERY = "qa-flex-gap";

export function flexGapQaForcesUnsupported(search?: string): boolean {
  if (!import.meta.env.DEV) return false;
  return new URLSearchParams(search ?? window.location.search).get(
    FLEX_GAP_QA_QUERY,
  ) === "unsupported";
}

export function supportsFlexGap(doc: Document = document): boolean {
  const probe = doc.createElement("div");
  probe.style.display = "flex";
  probe.style.flexDirection = "column";
  probe.style.rowGap = "1px";
  probe.append(doc.createElement("div"), doc.createElement("div"));
  doc.documentElement.appendChild(probe);

  try {
    return probe.scrollHeight === 1 || probe.scrollHeight === 2;
  } finally {
    probe.remove();
  }
}

export function applyFlexGapSupportState(
  doc: Document = document,
  forceUnsupported = false,
): boolean {
  const supported = !forceUnsupported && supportsFlexGap(doc);
  doc.documentElement.classList.toggle(FLEX_GAP_UNSUPPORTED_CLASS, !supported);
  doc.documentElement.dataset.flexGap = supported ? "supported" : "unsupported";
  return supported;
}
