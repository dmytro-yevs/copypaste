import { createElement, type ComponentType, type ReactNode } from "react";
import {
  toast as sonnerToast,
  type Action,
  type ExternalToast,
  type ToastT,
  type ToastToDismiss,
} from "sonner";

import { StateView, type StateMode } from "@/components/shared/StateView";
import { Button } from "@/components/ui/button";
import { t } from "@/i18n";

type ToastMessage = ReactNode | (() => ReactNode);
type ToastAction = Action | ReactNode;
type ToastMode = Extract<StateMode, "loading" | "success" | "info" | "warning" | "error">;
type NotifyOptions = ExternalToast;
type SonnerCustomOptions = ExternalToast & { type?: ToastT["type"] };

function asRenderable(node: ToastMessage | undefined): ReactNode {
  return typeof node === "function"
    ? createElement(node as ComponentType)
    : node;
}

function isAction(action: ToastAction): action is Action {
  return typeof action === "object" && action !== null && "label" in action;
}

function isToastRecord(toast: ToastT | ToastToDismiss): toast is ToastT {
  return !("dismiss" in toast);
}

function renderAction(
  action: ToastAction | undefined,
  id: string | number,
  kind: "action" | "cancel",
  dismissible: boolean,
  options: NotifyOptions,
): ReactNode {
  if (action == null) return null;
  if (!isAction(action)) return action;

  const buttonStyle = kind === "action" ? options.actionButtonStyle : options.cancelButtonStyle;
  const buttonClass = kind === "action"
    ? options.classNames?.actionButton
    : options.classNames?.cancelButton;

  return (
    <Button
      className={buttonClass}
      size="compact"
      variant={kind === "cancel" ? "secondary" : "primary"}
      style={buttonStyle}
      disabled={kind === "cancel" && !dismissible}
      onClick={(event) => {
        if (kind === "cancel" && !dismissible) return;
        action.onClick.call(action, event);
        if (kind === "cancel" || !event.defaultPrevented) sonnerToast.dismiss(id);
      }}
    >
      {action.label}
    </Button>
  );
}

function notifyAs(mode: ToastMode, message: ToastMessage, options: NotifyOptions = {}) {
  const {
    action,
    cancel,
    description,
    ...sonnerOptions
  } = options;
  const dismissible = options.dismissible !== false;

  const customOptions: SonnerCustomOptions = {
    ...sonnerOptions,
    icon: null,
    type: mode,
  };

  return sonnerToast.custom(
    (id) => {
      let closedByUser = false;
      const close = () => {
        if (closedByUser) return;
        closedByUser = true;
        const current = sonnerToast.getToasts().find(
          (candidate): candidate is ToastT => candidate.id === id && isToastRecord(candidate),
        );
        sonnerToast.dismiss(id);
        current?.onDismiss?.call(current, current);
      };
      const actions = action != null || cancel != null ? (
        <>
          {renderAction(cancel, id, "cancel", dismissible, options)}
          {renderAction(action, id, "action", dismissible, options)}
        </>
      ) : undefined;

      return (
        <div className="copypaste-toast-layout">
          <StateView
            mode={mode}
            placement="panel"
            role="group"
            aria-live="off"
            title={asRenderable(message)}
            description={asRenderable(description)}
            actions={actions}
          />
          {options.closeButton === false ? null : (
            <Button
              variant="ghost"
              size="compactIcon"
              icon="close"
              data-toast-close=""
              aria-label={t("common.close")}
              disabled={!dismissible}
              onClick={close}
            />
          )}
        </div>
      );
    },
    customOptions,
  );
}

function notify(message: ToastMessage, options?: NotifyOptions) {
  return notifyAs("info", message, options);
}

export const toast = Object.assign(notify, {
  success: (message: ToastMessage, options?: NotifyOptions) => notifyAs("success", message, options),
  info: (message: ToastMessage, options?: NotifyOptions) => notifyAs("info", message, options),
  warning: (message: ToastMessage, options?: NotifyOptions) => notifyAs("warning", message, options),
  error: (message: ToastMessage, options?: NotifyOptions) => notifyAs("error", message, options),
  loading: (message: ToastMessage, options?: NotifyOptions) => notifyAs("loading", message, options),
  dismiss: (id?: string | number) => sonnerToast.dismiss(id),
});
