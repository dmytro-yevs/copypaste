import { useRef, useState } from "react";
import { GlideHighlight } from "../../../popup/GlideHighlight";
import { PopupRow } from "../../../popup/PopupRow";
import { makeHistoryEntry } from "../../../lib/fixtures";

// Task 6.7: "popup row/keycap/glide-highlight". Reproduces Popup.tsx's real
// GlideHighlight + <ul role="listbox"> + PopupRow wiring (Popup.tsx:381-421)
// against fixture items instead of live IPC data.
const ITEMS = [
  makeHistoryEntry({
    id: "gallery-popup-1",
    preview: "Meeting notes for the clipboard sync review",
  }),
  makeHistoryEntry({
    id: "gallery-popup-2",
    kind: "URL",
    preview: "https://copypaste.app/changelog",
  }),
  makeHistoryEntry({
    id: "gallery-popup-3",
    is_sensitive: true,
    sensitive_spans: [[10, 20]],
    preview: "Password: Hunter2!@#",
  }),
];

export function PopupSection() {
  const [selectedIdx, setSelectedIdx] = useState(0);
  const listRef = useRef<HTMLUListElement>(null);
  const glideItems = ITEMS.map((item) => ({ item, positions: [] as number[] }));

  return (
    <section id="gallery-popup-row">
      <h2>Popup row · keycap · glide-highlight</h2>
      <div className="gallery__popup-frame">
        <GlideHighlight
          selectedIdx={selectedIdx}
          items={glideItems}
          textRowHeight={28}
          imageMaxHeight={40}
          listRef={listRef}
        />
        <ul
          ref={listRef}
          role="listbox"
          aria-label="Popup row example"
          aria-activedescendant={`popup-item-${ITEMS[selectedIdx].id}`}
        >
          {ITEMS.map((item, idx) => (
            <PopupRow
              key={item.id}
              item={item}
              index={idx}
              selected={idx === selectedIdx}
              textRowHeight={28}
              imageMaxHeight={40}
              maskSensitive={true}
              matchPositions={[]}
              previewLines={1}
              // Every row keycapped — demonstrates the ⌘N keycap pill.
              showKeycap={idx < 9}
              onMouseEnter={() => setSelectedIdx(idx)}
              onClick={() => setSelectedIdx(idx)}
              onPin={() => {}}
            />
          ))}
        </ul>
      </div>
    </section>
  );
}
