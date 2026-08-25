import { type VariantProps, cva } from "class-variance-authority";
import styles from "./modal-shell.module.css";

const modalOverlayClass = styles.overlay;

const modalFrameVariants = cva(
  styles.frame,
  {
    variants: {
      presentation: {
        modal: styles.modal,
        sheet: styles.sheet,
        drawer: styles.drawer,
      },
    },
    defaultVariants: { presentation: "modal" },
  },
);

const modalHeaderClass = styles.header;
const modalFooterClass = styles.footer;
const modalTitleClass = styles.title;
const modalDescriptionClass = styles.description;

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
