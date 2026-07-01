import { FileChip } from "../../../components/FileChip";

// FileChip — file identity chip + Open/Save As…/Copy actions (used by
// DetailsModal for `file`-kind items).
export function FileChipSection() {
  return (
    <section id="gallery-file-chip">
      <h2>File chip</h2>
      <div className="gallery__row">
        <FileChip
          id="gallery-file-chip-example"
          filename="Q3_Report_2026.pdf"
          mime="application/pdf"
          sizeBytes={245_000}
        />
      </div>
    </section>
  );
}
