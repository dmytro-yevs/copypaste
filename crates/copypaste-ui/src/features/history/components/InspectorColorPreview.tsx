import styles from "./InspectorColorPreview.module.css";

export function InspectorColorPreview({
    label,
    value,
}: {
    label: string;
    value: string;
}) {
    const color = value.trim();

    return (
        <figure
            className={styles.root}
            role="img"
            aria-label={`${label}: ${color}`}
        >
            <span
                aria-hidden="true"
                className={styles.swatch}
                data-slot="inspector-color-swatch"
            >
                <span
                    className={styles.colorField}
                    data-slot="inspector-color-field"
                    style={{ backgroundColor: color }}
                />
            </span>
            <figcaption aria-hidden="true" className={styles.label}>
                {color}
            </figcaption>
        </figure>
    );
}
