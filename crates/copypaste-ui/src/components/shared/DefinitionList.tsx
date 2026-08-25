import type { ComponentProps } from "react";

import { cn } from "@/lib/cn";
import styles from "./DefinitionList.module.css";

export function DefinitionList({
  className,
  density = "regular",
  ...props
}: ComponentProps<"dl"> & { density?: "compact" | "regular" }) {
  return (
    <dl
      data-density={density}
      className={cn(styles.list, className)}
      {...props}
    />
  );
}

export function DefinitionRow({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn(styles.row, className)} {...props} />;
}

export function DefinitionTerm({ className, ...props }: ComponentProps<"dt">) {
  return <dt className={cn(styles.term, className)} {...props} />;
}

export function DefinitionValue({ className, ...props }: ComponentProps<"dd">) {
  return <dd className={cn(styles.value, className)} {...props} />;
}
