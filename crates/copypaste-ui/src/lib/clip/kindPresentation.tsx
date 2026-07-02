import {
  Braces,
  Code2,
  File,
  FileQuestion,
  Hash,
  Image as ImageIcon,
  Link2,
  Mail,
  Palette,
  Type,
  type LucideIcon,
} from "lucide-react";
import type { NormalizedKind } from "./normalizeContentKind";

export interface KindPresentation {
  /** Content-type color token name, e.g. "--c-url". */
  token: string;
  /** Lucide glyph for the tile/meta. */
  Icon: LucideIcon;
  /** Human-readable "type word" for the meta line. */
  label: string;
}

/**
 * Typed presentation map — one entry per normalized kind, including an explicit
 * `unknown` entry (generic glyph, dim token, "Unknown"). Shared by the tile,
 * preview, and metadata units so History and Popup present kinds identically
 * (design.md Decision 8). Icons are `lucide-react` (task 2.6).
 */
export const KIND_PRESENTATION: Record<NormalizedKind, KindPresentation> = {
  text: { token: "--c-text", Icon: Type, label: "Text" },
  url: { token: "--c-url", Icon: Link2, label: "URL" },
  mail: { token: "--c-mail", Icon: Mail, label: "Email" },
  num: { token: "--c-num", Icon: Hash, label: "Number" },
  color: { token: "--c-color", Icon: Palette, label: "Color" },
  json: { token: "--c-json", Icon: Braces, label: "JSON" },
  code: { token: "--c-code", Icon: Code2, label: "Code" },
  file: { token: "--c-file", Icon: File, label: "File" },
  image: { token: "--c-image", Icon: ImageIcon, label: "Image" },
  unknown: { token: "--dim", Icon: FileQuestion, label: "Unknown" },
};
