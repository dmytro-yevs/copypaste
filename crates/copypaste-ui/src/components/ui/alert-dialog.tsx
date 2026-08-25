import type { ComponentProps } from "react";
import * as AlertDialogPrimitive from "@radix-ui/react-alert-dialog";

import { cn } from "@/lib/cn";
import { Button } from "./button";
import {
  modalDescriptionClass,
  modalFooterClass,
  modalFrameVariants,
  modalHeaderClass,
  modalOverlayClass,
  modalTitleClass,
  type ModalFrameProps,
} from "./modal-shell";

/**
 * The confirm dialog for destructive actions. `AlertDialog` rather than
 * `Dialog` on purpose: it is `role="alertdialog"`, it focuses the *cancel*
 * action by default, and it does not close on a backdrop click — a misclick
 * must not be able to dismiss a prompt whose other button erases history
 * (AGENTS.md rule 4: data loss is the worst outcome).
 */
function AlertDialog(props: ComponentProps<typeof AlertDialogPrimitive.Root>) {
  return <AlertDialogPrimitive.Root data-slot="alert-dialog" {...props} />;
}

const AlertDialogTrigger = AlertDialogPrimitive.Trigger;
const AlertDialogPortal = AlertDialogPrimitive.Portal;

function AlertDialogOverlay({
  className,
  ...props
}: ComponentProps<typeof AlertDialogPrimitive.Overlay>) {
  return (
    <AlertDialogPrimitive.Overlay
      data-slot="alert-dialog-overlay"
      className={cn(modalOverlayClass, className)}
      {...props}
    />
  );
}

function AlertDialogContent({
  className,
  presentation,
  ...props
}: ComponentProps<typeof AlertDialogPrimitive.Content> & ModalFrameProps) {
  return (
    <AlertDialogPortal>
      <AlertDialogOverlay />
      <AlertDialogPrimitive.Content
        data-slot="alert-dialog-content"
        className={cn(modalFrameVariants({ presentation }), className)}
        {...props}
      />
    </AlertDialogPortal>
  );
}

function AlertDialogHeader({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="alert-dialog-header"
      className={cn(modalHeaderClass, className)}
      {...props}
    />
  );
}

function AlertDialogFooter({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="alert-dialog-footer"
      className={cn(modalFooterClass, className)}
      {...props}
    />
  );
}

function AlertDialogTitle({
  className,
  ...props
}: ComponentProps<typeof AlertDialogPrimitive.Title>) {
  return (
    <AlertDialogPrimitive.Title
      data-slot="alert-dialog-title"
      className={cn(modalTitleClass, className)}
      {...props}
    />
  );
}

function AlertDialogDescription({
  className,
  ...props
}: ComponentProps<typeof AlertDialogPrimitive.Description>) {
  return (
    <AlertDialogPrimitive.Description
      data-slot="alert-dialog-description"
      className={cn(modalDescriptionClass, className)}
      {...props}
    />
  );
}

function AlertDialogAction({
  className,
  variant = "primary",
  size = "md",
  tone = "neutral",
  ...props
}: ComponentProps<typeof AlertDialogPrimitive.Action> & {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md" | "lg";
  tone?: "neutral" | "danger";
}) {
  return (
    <Button asChild variant={variant} size={size} tone={tone} className={className}>
      <AlertDialogPrimitive.Action {...props} />
    </Button>
  );
}

function AlertDialogCancel({
  className,
  size = "md",
  ...props
}: ComponentProps<typeof AlertDialogPrimitive.Cancel> & {
  size?: "sm" | "md" | "lg";
}) {
  return (
    <Button asChild variant="secondary" size={size} className={className}>
      <AlertDialogPrimitive.Cancel {...props} />
    </Button>
  );
}

export {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogOverlay,
  AlertDialogPortal,
  AlertDialogTitle,
  AlertDialogTrigger,
};
