import { type VariantProps, cva } from "class-variance-authority";

const modalOverlayClass =
  "fixed inset-0 z-[var(--z-scrim)] bg-scrim [backdrop-filter:var(--scrim-blur)] data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:animate-in data-[state=open]:fade-in-0";

const modalFrameVariants = cva(
  "fixed z-[var(--z-dialog)] grid overflow-y-auto overscroll-contain border border-border bg-card shadow-3 duration-[var(--dur)] data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:animate-in data-[state=open]:fade-in-0",
  {
    variants: {
      presentation: {
        modal:
          "top-1/2 right-[calc(var(--inset-right)+var(--s-4))] left-[calc(var(--inset-left)+var(--s-4))] mx-auto max-h-[calc(100dvh-var(--inset-top)-var(--inset-bottom)-var(--s-8))] w-auto max-w-[var(--modal-w)] translate-y-[-50%] gap-4 rounded-xl p-6 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95",
        sheet:
          "top-auto right-[calc(var(--inset-right)+var(--s-2))] bottom-[calc(var(--inset-bottom)+var(--s-2))] left-[calc(var(--inset-left)+var(--s-2))] max-h-[calc(100dvh-var(--inset-top)-var(--inset-bottom)-var(--s-4))] w-auto max-w-none gap-4 rounded-xl p-4",
      },
    },
    defaultVariants: { presentation: "modal" },
  },
);

const modalHeaderClass = "flex flex-col gap-2 text-left";
const modalFooterClass =
  "flex flex-col-reverse gap-2 sm:flex-row sm:justify-end";
const modalTitleClass = "text-lg leading-none font-semibold";
const modalDescriptionClass = "text-sm text-muted-foreground";

type ModalFrameProps = VariantProps<typeof modalFrameVariants>;

export {
  modalDescriptionClass,
  modalFooterClass,
  modalFrameVariants,
  modalHeaderClass,
  modalOverlayClass,
  modalTitleClass,
};
export type { ModalFrameProps };
