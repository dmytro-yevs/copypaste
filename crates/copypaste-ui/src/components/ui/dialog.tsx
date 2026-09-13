import { useRef, useState, type ComponentProps, type PointerEvent } from "react";
import * as DialogPrimitive from "@radix-ui/react-dialog";

import { cn } from "@/lib/cn";
import { Button } from "./button";
import { Icon } from "./icon";
import { Tooltip } from "./tooltip";
import { VisuallyHidden } from "./visually-hidden";
import {
    modalDescriptionClass,
    modalFooterClass,
    modalFrameVariants,
    modalHeaderClass,
    modalOverlayClass,
    modalTitleClass,
    type ModalFrameProps,
} from "./modal-shell";
import styles from "./dialog.module.css";

/**
 * A11Y-4 and INV-19 come from Radix; do not hand-write the focus trap
 * and the scroll lock (`useFocusTrap`, `lib/dialog/scrollLock.ts`) and shipped
 * bugs in both.
 */
function Dialog(props: ComponentProps<typeof DialogPrimitive.Root>) {
    return <DialogPrimitive.Root data-slot="dialog" {...props} />;
}

const DialogTrigger = DialogPrimitive.Trigger;
const DialogPortal = DialogPrimitive.Portal;
const DialogClose = DialogPrimitive.Close;

function DialogOverlay({
    className,
    ...props
}: ComponentProps<typeof DialogPrimitive.Overlay>) {
    return (
        <DialogPrimitive.Overlay
            data-slot="dialog-overlay"
            className={cn(modalOverlayClass, className)}
            {...props}
        />
    );
}

const SHEET_DISMISS_PX = 72;

function DialogContent({
    className,
    children,
    showCloseButton = true,
    closeLabel = "Close",
    overlayClassName,
    presentation,
    ...props
}: ComponentProps<typeof DialogPrimitive.Content> & {
    showCloseButton?: boolean;
    closeLabel?: string;
    overlayClassName?: string;
} & ModalFrameProps) {
    const sheet = presentation === "sheet";
    const closeRef = useRef<HTMLButtonElement>(null);
    const dragOrigin = useRef<number | null>(null);
    const [dragOffset, setDragOffset] = useState(0);

    const onSheetPointerDown = (event: PointerEvent<HTMLDivElement>) => {
        if (!sheet || event.button !== 0) return;
        const handle = (event.target as HTMLElement | null)?.closest(
            "[data-slot='dialog-sheet-handle']",
        );
        if (!handle) return;
        dragOrigin.current = event.clientY;
        handle.setPointerCapture(event.pointerId);
    };
    const onSheetPointerMove = (event: PointerEvent<HTMLDivElement>) => {
        if (dragOrigin.current === null) return;
        setDragOffset(Math.max(0, event.clientY - dragOrigin.current));
    };
    const endSheetDrag = () => {
        if (dragOrigin.current === null) return;
        const dismiss = dragOffset >= SHEET_DISMISS_PX;
        dragOrigin.current = null;
        setDragOffset(0);
        if (dismiss) closeRef.current?.click();
    };

    return (
        <DialogPortal data-slot="dialog-portal">
            <DialogOverlay className={overlayClassName} />
            <DialogPrimitive.Content
                data-slot="dialog-content"
                className={cn(modalFrameVariants({ presentation }), className)}
                style={
                    sheet && dragOffset > 0
                        ? { transform: `translateY(${dragOffset}px)` }
                        : undefined
                }
                onPointerDown={sheet ? onSheetPointerDown : undefined}
                onPointerMove={sheet ? onSheetPointerMove : undefined}
                onPointerUp={sheet ? endSheetDrag : undefined}
                onPointerCancel={sheet ? endSheetDrag : undefined}
                {...props}
            >
                {sheet ? (
                    <div
                        data-slot="dialog-sheet-handle"
                        className={styles.sheetHandle}
                        aria-hidden="true"
                    />
                ) : null}
                {children}
                {sheet ? (
                    <DialogPrimitive.Close
                        ref={closeRef}
                        className={styles.sheetDismiss}
                    >
                        <VisuallyHidden>{closeLabel}</VisuallyHidden>
                    </DialogPrimitive.Close>
                ) : null}
                {showCloseButton && (
                    <Tooltip content={closeLabel}>
                        <Button
                            asChild
                            variant="ghost"
                            size="compactIcon"
                            className={styles.close}
                        >
                            <DialogPrimitive.Close data-slot="dialog-close">
                                <Icon name="close" />
                                <VisuallyHidden>{closeLabel}</VisuallyHidden>
                            </DialogPrimitive.Close>
                        </Button>
                    </Tooltip>
                )}
            </DialogPrimitive.Content>
        </DialogPortal>
    );
}

function DialogHeader({ className, ...props }: ComponentProps<"div">) {
    return (
        <div
            data-slot="dialog-header"
            className={cn(modalHeaderClass, className)}
            {...props}
        />
    );
}

function DialogFooter({ className, ...props }: ComponentProps<"div">) {
    return (
        <div
            data-slot="dialog-footer"
            className={cn(modalFooterClass, className)}
            {...props}
        />
    );
}

function DialogTitle({
    className,
    ...props
}: ComponentProps<typeof DialogPrimitive.Title>) {
    return (
        <DialogPrimitive.Title
            data-slot="dialog-title"
            className={cn(modalTitleClass, className)}
            {...props}
        />
    );
}

function DialogDescription({
    className,
    ...props
}: ComponentProps<typeof DialogPrimitive.Description>) {
    return (
        <DialogPrimitive.Description
            data-slot="dialog-description"
            className={cn(modalDescriptionClass, className)}
            {...props}
        />
    );
}

export {
    Dialog,
    DialogClose,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogOverlay,
    DialogPortal,
    DialogTitle,
    DialogTrigger,
};
