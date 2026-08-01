import { useEffect, useRef } from "react";
import { toast } from "sonner";

interface RevealNoticeProps {
  message: string | null;
}

/** A refused reveal is transient feedback, not a second permanent history
 * panel. Its text is classified by `useReveal`, never supplied by the daemon. */
export function RevealNotice({ message }: RevealNoticeProps) {
  const previous = useRef<string | null>(null);

  useEffect(() => {
    if (message === null) {
      previous.current = null;
      return;
    }
    if (previous.current === message) return;
    previous.current = message;
    toast.error(message, { id: "sensitive-reveal" });
  }, [message]);

  return null;
}
