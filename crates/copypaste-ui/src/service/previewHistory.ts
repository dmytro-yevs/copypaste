import { UI_COMMANDS } from "@/generated/ipc";
import { IpcFailure } from "@/lib/errors";
import type {
    ClipboardWriteAvailability,
    ClipboardWriteMode,
    ImagePreview,
    Item,
    ItemPage,
} from "@/lib/ipc";
import type { PreviewResourceState } from "@/service/previewScenario";

const PREVIEW_PIXEL =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

const LONG_READER_BODY = Array.from(
    { length: 144 },
    (_, index) =>
        `Section ${index + 1}. This synthetic clipping checks how a full reader handles many paragraphs, wrapped lines, keyboard focus, and internal scrolling while the list keeps a short preview.`,
).join("\n\n");
const LONG_READER_PREVIEW = LONG_READER_BODY.slice(0, 180);

function item(
    id: string,
    content: string | null,
    contentClass: Item["content_class"],
    overrides: Partial<Item> = {},
): Item {
    return {
        id,
        content,
        content_type: contentClass === "image" ? "image/png" : "text/plain",
        content_class: contentClass,
        created_at: Date.now() - 30_000,
        pinned: false,
        is_sensitive: false,
        sensitive_finding: null,
        origin_device_id: "preview-device",
        origin_device_name: "Preview device",
        source_app_bundle_id: null,
        source_app_name: null,
        too_large_to_sync: false,
        truncated: false,
        ...overrides,
    };
}

function items(): Item[] {
    return [
        item("preview-plain", "Preview text preview", "text", { truncated: true }),
        item("preview-source", LONG_READER_PREVIEW, "text", {
            truncated: true,
            source_app_bundle_id: "com.example.editor",
            source_app_name: "Example Editor",
        }),
        item("preview-image", null, "image"),
        item("preview-file", "[file]", "file", { content_type: "file" }),
        item(
            "preview-unknown",
            "Unsupported preview",
            // Deliberate future-class fixture: refusal coverage, not a shipped class.
            "archive" as Item["content_class"],
            { content_type: "application/x-future", truncated: true },
        ),
    ];
}

/** Fixture behavior for browser previews only; production always asks native. */
export function previewHistoryWriteAvailability(
    contentType: string,
    mode: ClipboardWriteMode,
    platform: string,
): ClipboardWriteAvailability {
    if (contentType === "text" || contentType.startsWith("text/")) {
        return "available";
    }
    if (mode === "plain_text") return "unsupported_content_type";
    if (contentType === "file") {
        return platform === "macos" ? "available" : "unsupported_on_platform";
    }
    if (contentType.startsWith("image/")) {
        if (!["image/png", "image/tiff", "image/bmp"].includes(contentType)) {
            return "unsupported_content_type";
        }
        if (platform === "windows" ||
            (platform === "macos" && contentType !== "image/bmp")) {
            return "available";
        }
        return "unsupported_on_platform";
    }
    return "unsupported_content_type";
}

const bodies = new Map<string, string>([
    ["preview-plain", "Preview clipboard content from a fixture."],
    ["preview-source", LONG_READER_BODY],
]);

export function previewHistoryPage(empty: boolean): ItemPage {
    const history = empty ? [] : items();
    return {
        items: history,
        total: history.length,
        skipped_undecryptable: 0,
        next_cursor: null,
    };
}

export function previewHistoryCount(resource: PreviewResourceState): number {
    return resource === "success" ? items().length : 0;
}

export function previewHistoryResourceResponse(
    command: string,
    args?: Record<string, unknown>,
): ImagePreview | string | undefined {
    if (
        command !== UI_COMMANDS.get_item_body &&
        command !== UI_COMMANDS.get_image_preview
    ) {
        return undefined;
    }
    const id = args?.id;
    if (typeof id !== "string") throw new IpcFailure("invalid_request", false);
    if (command === UI_COMMANDS.get_item_body) {
        const body = bodies.get(id);
        if (body !== undefined) return body;
        if (items().some((item) => item.id === id)) {
            throw new IpcFailure("unsupported_content", false);
        }
        throw new IpcFailure("not_found", false);
    }
    if (id === "preview-image") {
        return { png_base64: PREVIEW_PIXEL, width: 1, height: 1 };
    }
    throw new IpcFailure("not_found", false);
}
