import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const css = readFileSync(
    resolve(
        process.cwd(),
        "src/features/devices/patterns/DiscoveryStage.module.css",
    ),
    "utf8",
);

describe("DiscoveryStage radar styles", () => {
    it("uses one clipped boundary with the radar reaching every edge", () => {
        const stateRule = css.match(/\.state\s*\{([^}]*)\}/);
        const radarRule = css.match(/\.radar\s*\{([^}]*)\}/);

        expect(stateRule?.[1]).toMatch(/padding:\s*0/);
        expect(stateRule?.[1]).toMatch(/overflow:\s*hidden/);
        expect(stateRule?.[1]).toMatch(/border-radius:\s*var\(--r-card\)/);
        expect(radarRule?.[1]).toMatch(/border:\s*0/);
        expect(radarRule?.[1]).toMatch(/border-radius:\s*0/);
    });

    it("keeps hovered radar nodes opaque", () => {
        const hoverRule = css.match(
            /\.radarDevice\.radarDevice:hover:not\(:disabled\)\s*\{([^}]*)\}/,
        );

        expect(hoverRule?.[1]).toMatch(/opacity:\s*1/);
        expect(hoverRule?.[1]).toMatch(/background:\s*var\(--raised\)/);
    });

    it("pulses the local device without fading its icon", () => {
        const localRule = css.match(/\.localNode\s*\{([^}]*)\}/);
        const ringRule = css.match(/\.localNode::after\s*\{([^}]*)\}/);

        expect(localRule?.[1]).toMatch(
            /animation:\s*discovery-radar-local-breathe/,
        );
        expect(localRule?.[1]).not.toMatch(/opacity:/);
        expect(ringRule?.[1]).toMatch(
            /animation:\s*discovery-radar-local-ring/,
        );
        expect(css).toMatch(
            /@keyframes discovery-radar-local-breathe[\s\S]*scale\(1\.07\)/,
        );
        expect(css).toMatch(
            /@media \(prefers-reduced-motion: reduce\)[\s\S]*\.localNode,[\s\S]*\.localNode::after,[\s\S]*animation:\s*none/,
        );
    });

    it("moves the complete device card and reserves a center clearance", () => {
        const layerRule = [
            ...css.matchAll(/\.deviceLayer\s*\{([^}]*)\}/g),
        ].find((match) => match[1].includes("grid-template"));
        const cardRule = css.match(/\.radarDevice\s*\{([^}]*)\}/);
        const contentRule = css.match(/\.radarDevice > span\s*\{([^}]*)\}/);

        expect(layerRule?.[1]).toMatch(/repeat\(13,/);
        expect(cardRule?.[1]).toMatch(
            /animation:[\s\S]*discovery-radar-live/,
        );
        expect(cardRule?.[1]).toMatch(/padding:\s*0/);
        expect(cardRule?.[1]).toMatch(
            /border-radius:\s*calc\(var\(--r-card\) \+ var\(--s-1\)\)/,
        );
        expect(contentRule?.[1]).not.toMatch(/discovery-radar-live/);
        expect(css).toMatch(
            /\[data-distance="near"\]\[data-sector="7"\][\s\S]*grid-row:\s*10/,
        );
    });
});
