import path from "node:path";

import { writeRedacted, writeSafeScreenshot } from "./redact.js";

export interface SetupEvidenceProbe {
  shell(): Promise<unknown>;
  hierarchy(): Promise<unknown>;
  screenshot(): Promise<string>;
  logs(): Promise<unknown>;
}

function slug(name: string): string {
  return (
    name
      .replace(/[^a-zA-Z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 120) || "failure"
  );
}

async function required<T>(name: string, action: () => Promise<T>): Promise<T> {
  try {
    return await action();
  } catch {
    throw new Error(`required setup evidence ${name} was unavailable`);
  }
}

export async function writeSetupFailureEvidence(
  suite: string,
  stage: string,
  probe: SetupEvidenceProbe,
  out: string,
): Promise<void> {
  const folder = path.join(out, "failures", slug(suite));
  const evidence = {
    suite,
    stage,
    shell: await required("shell state", () => probe.shell()),
    hierarchy: await required("hierarchy", () => probe.hierarchy()),
    logs: await required("bounded logs", () => probe.logs()),
  };
  const screenshot = await required("screenshot", () => probe.screenshot());
  await required("JSON write", async () =>
    writeRedacted(path.join(folder, `${slug(stage)}.json`), evidence),
  );
  await required("screenshot write", async () =>
    writeSafeScreenshot(path.join(folder, `${slug(stage)}.png`), screenshot),
  );
}
