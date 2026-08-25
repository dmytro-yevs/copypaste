import { translucencyAttribute } from "@/lib/appearancePrefs";
import {
  applyAppearance,
  resolveTheme,
  type Appearance,
} from "@/lib/theme";
import styles from "./themeTransition.module.css";

type ThemeViewTransition = {
  ready: Promise<void>;
  finished: Promise<void>;
};

type TransitionDocument = {
  startViewTransition?: (callback: () => void) => ThemeViewTransition;
};

type Origin = {
  x: number;
  y: number;
  radius: number;
};

type Motion = {
  revealMs: number;
  fadeMs: number;
  revealEasing: string;
};

let transitionTail = Promise.resolve();

function originFrom(trigger: HTMLElement): Origin {
  const bounds = trigger.getBoundingClientRect();
  const width = Math.max(document.documentElement.clientWidth, window.innerWidth);
  const height = Math.max(document.documentElement.clientHeight, window.innerHeight);
  const x = Math.min(width, Math.max(0, bounds.left + bounds.width / 2));
  const y = Math.min(height, Math.max(0, bounds.top + bounds.height / 2));
  return {
    x,
    y,
    radius: Math.hypot(Math.max(x, width - x), Math.max(y, height - y)),
  };
}

function cssTimeMs(name: string): number {
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
  if (value.endsWith("ms")) return Number.parseFloat(value);
  if (value.endsWith("s")) return Number.parseFloat(value) * 1000;
  return 0;
}

function motion(): Motion {
  return {
    revealMs: cssTimeMs("--dur-theme") + cssTimeMs("--dur-fast"),
    fadeMs: cssTimeMs("--dur-fast"),
    revealEasing: getComputedStyle(document.documentElement)
      .getPropertyValue("--ease-linear")
      .trim(),
  };
}

function reducedMotion(): boolean {
  return typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function matchesDocument(target: Appearance): boolean {
  const root = document.documentElement;
  return root.dataset.mode === target.theme &&
    root.dataset.colorScheme === resolveTheme(target.theme) &&
    root.dataset.theme === target.colorTheme &&
    root.dataset.translucency === translucencyAttribute(target.translucency);
}

function nextFrame(): Promise<void> {
  return new Promise((resolve) => {
    window.requestAnimationFrame(() => resolve());
  });
}

function waitForTransition(
  element: HTMLElement,
  property: "transform" | "opacity",
  maximumMs: number,
): Promise<void> {
  return new Promise((resolve) => {
    let settled = false;
    const finish = () => {
      if (settled) return;
      settled = true;
      window.clearTimeout(timeout);
      element.removeEventListener("transitionend", onEnd);
      element.removeEventListener("transitioncancel", onEnd);
      resolve();
    };
    const onEnd = (event: Event) => {
      const transition = event as TransitionEvent;
      if (event.target === element && transition.propertyName === property) finish();
    };
    const timeout = window.setTimeout(finish, maximumMs);
    element.addEventListener("transitionend", onEnd);
    element.addEventListener("transitioncancel", onEnd);
  });
}

async function nativeTransition(
  origin: Origin,
  commit: () => void,
  timing: Motion,
): Promise<void> {
  const root = document.documentElement;
  const transitionDocument = document as unknown as TransitionDocument;
  const startViewTransition = transitionDocument.startViewTransition;
  if (!startViewTransition || typeof root.animate !== "function") {
    throw new Error("Native View Transitions are unavailable");
  }

  root.dataset.themeTransition = "native";
  let transition: ThemeViewTransition | undefined;
  try {
    transition = startViewTransition.call(document, commit);
    await transition.ready;
    const animation = root.animate(
      {
        clipPath: [
          `circle(0 at ${origin.x}px ${origin.y}px)`,
          `circle(${origin.radius}px at ${origin.x}px ${origin.y}px)`,
        ],
      },
      {
        duration: timing.revealMs,
        easing: timing.revealEasing,
        fill: "both",
        pseudoElement: "::view-transition-new(root)",
      } as KeyframeAnimationOptions,
    );
    await Promise.all([animation.finished, transition.finished]);
  } catch (error) {
    if (transition) {
      try {
        await transition.finished;
      } catch (finishError) {
        console.error(
          "[copypaste] native theme transition finalization failed",
          finishError,
        );
      }
    }
    throw error;
  } finally {
    delete root.dataset.themeTransition;
  }
}

async function veilTransition(
  origin: Origin,
  target: Appearance,
  commit: () => void,
  timing: Motion,
): Promise<void> {
  const veil = document.createElement("div");
  veil.className = `${styles.veil} theme-scope`;
  veil.dataset.colorScheme = resolveTheme(target.theme);
  veil.dataset.mode = target.theme;
  veil.dataset.theme = target.colorTheme;
  veil.dataset.translucency = translucencyAttribute(target.translucency);
  veil.dataset.themeTransitionVeil = "true";
  veil.setAttribute("aria-hidden", "true");
  veil.style.setProperty("--theme-transition-x", `${origin.x}px`);
  veil.style.setProperty("--theme-transition-y", `${origin.y}px`);
  veil.style.setProperty(
    "--theme-transition-diameter",
    `${origin.radius * 2}px`,
  );

  document.body.appendChild(veil);
  try {
    const revealed = waitForTransition(
      veil,
      "transform",
      timing.revealMs + timing.fadeMs,
    );
    await nextFrame();
    veil.dataset.phase = "reveal";
    await revealed;

    commit();
    await nextFrame();

    const faded = waitForTransition(veil, "opacity", timing.fadeMs * 2);
    veil.dataset.phase = "fade";
    await faded;
  } finally {
    veil.parentNode?.removeChild(veil);
  }
}

async function runTransition(
  origin: Origin,
  target: () => Appearance,
  update: () => void,
): Promise<void> {
  const next = target();
  if (matchesDocument(next)) return;

  let committed = false;
  const commit = () => {
    if (committed) return;
    update();
    applyAppearance(next);
    committed = true;
  };
  const timing = motion();

  if (reducedMotion() || timing.revealMs === 0) {
    commit();
    return;
  }

  const transitionDocument = document as unknown as TransitionDocument;
  if (transitionDocument.startViewTransition) {
    try {
      await nativeTransition(origin, commit, timing);
      return;
    } catch (error) {
      console.error("[copypaste] native theme transition failed", error);
      if (committed) return;
    }
  }

  try {
    await veilTransition(origin, next, commit, timing);
  } catch (error) {
    console.error("[copypaste] theme veil transition failed", error);
    commit();
  }
}

export function changeAppearanceFrom(
  trigger: HTMLElement,
  target: () => Appearance,
  update: () => void,
): Promise<void> {
  const origin = originFrom(trigger);
  const queued = transitionTail.then(() => runTransition(origin, target, update));
  transitionTail = queued.catch((error) => {
    console.error("[copypaste] theme transition failed", error);
    update();
    applyAppearance(target());
  });
  return transitionTail;
}
