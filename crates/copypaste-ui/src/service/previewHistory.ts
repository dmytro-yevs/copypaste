import { UI_COMMANDS } from "@/generated/ipc";
import { IpcFailure } from "@/lib/errors";
import type { ImagePreview, Item, ItemPage } from "@/lib/ipc";
import type { PreviewResourceState } from "@/service/previewScenario";

const PREVIEW_PIXEL =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

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
        item("preview-source", "Draft preview", "text", {
            truncated: true,
            source_app_bundle_id: "com.example.editor",
            source_app_name: "Example Editor",
        }),
        item("preview-image", null, "image"),
        item(
            "preview-unknown",
            "Unsupported preview",
            // Deliberate future-class fixture: refusal coverage, not a shipped class.
            "archive" as Item["content_class"],
            { truncated: true },
        ),
    ];
}

const bodies = new Map<string, string>([
    ["preview-plain", "Preview clipboard content from a fixture."],
    ["preview-source", "Fixture body from Example Editor."],
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
