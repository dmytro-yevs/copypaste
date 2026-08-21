import { useEffect, useState, type RefObject } from "react";

export function useHeaderSearch(
  inputRef: RefObject<HTMLInputElement | null>,
  collapsible: boolean,
) {
  const [expanded, setExpanded] = useState(!collapsible);
  const open = () => {
    setExpanded(true);
    requestAnimationFrame(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    });
  };

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        setExpanded(true);
        requestAnimationFrame(() => {
          inputRef.current?.focus();
          inputRef.current?.select();
        });
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [inputRef]);

  return { expanded, setExpanded, open };
}
