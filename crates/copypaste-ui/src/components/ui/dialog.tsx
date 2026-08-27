import type { ComponentProps } from "react";
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
    return (
        <DialogPortal data-slot="dialog-portal">
            <DialogOverlay className={overlayClassName} />
            <DialogPrimitive.Content
                data-slot="dialog-content"
                className={cn(modalFrameVariants({ presentation }), className)}
                {...props}
            >
                {children}
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
