import type { ComponentProps, MouseEvent, ReactElement, ReactNode } from "react";
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
export type AlertDialogActionDescriptor = {
  label: ReactNode;
  onClick?: (event: MouseEvent<HTMLButtonElement>) => void;
  disabled?: boolean;
  pending?: boolean;
  variant?: "primary" | "secondary" | "ghost" | "danger";
  tone?: "neutral" | "danger";
  /** Opt in only when the action can safely close before any async work. */
  autoClose?: boolean;
};

export type AlertDialogCancelDescriptor = {
  label: ReactNode;
  disabled?: boolean;
  onClick?: (event: MouseEvent<HTMLButtonElement>) => void;
};

export type AlertDialogProps = Omit<ComponentProps<typeof AlertDialogPrimitive.Root>, "children"> & {
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  trigger?: ReactElement;
  contentProps?: Omit<ComponentProps<typeof AlertDialogContent>, "children">;
  cancel?: AlertDialogCancelDescriptor;
  action?: AlertDialogActionDescriptor;
};

function AlertDialog({ title, description, children, footer, trigger, contentProps, cancel, action, ...rootProps }: AlertDialogProps) {
  const actionButton = action ? (
    <Button
      type="button"
      variant={action.variant ?? "primary"}
      tone={action.tone ?? "neutral"}
      disabled={action.disabled}
      pending={action.pending}
      onClick={action.onClick}
    >
      {action.label}
    </Button>
  ) : null;

  return (
    <AlertDialogPrimitive.Root data-slot="alert-dialog" {...rootProps}>
      {trigger ? <AlertDialogPrimitive.Trigger asChild>{trigger}</AlertDialogPrimitive.Trigger> : null}
      <AlertDialogContent {...contentProps}>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          {description ? <AlertDialogDescription>{description}</AlertDialogDescription> : null}
        </AlertDialogHeader>
        {children}
        {footer || cancel || action ? (
          <AlertDialogFooter>
            {footer}
            {cancel ? <AlertDialogCancel disabled={cancel.disabled || action?.pending} onClick={cancel.onClick}>{cancel.label}</AlertDialogCancel> : null}
            {action?.autoClose && actionButton ? <AlertDialogPrimitive.Action asChild>{actionButton}</AlertDialogPrimitive.Action> : actionButton}
          </AlertDialogFooter>
        ) : null}
      </AlertDialogContent>
    </AlertDialogPrimitive.Root>
  );
}

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

export { AlertDialog };
