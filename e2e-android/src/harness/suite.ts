import { beforeAll } from "vitest";

type CaptureSetupFailure = (suite: string, stage?: string) => Promise<void>;

const captureSetupFailure: CaptureSetupFailure = async (suite, stage) => {
  const evidence = await import("./evidence.js");
  await evidence.captureSetupFailure(suite, stage);
};

export async function runSuiteSetup(
  suite: string,
  setup: () => Promise<void>,
  capture: CaptureSetupFailure = captureSetupFailure,
): Promise<void> {
  try {
    await setup();
  } catch (primary) {
    try {
      await capture(suite, "before-all");
    } catch (evidenceFailure) {
      throw new AggregateError(
        [primary, evidenceFailure],
        `${primary instanceof Error ? primary.message : String(primary)} ` +
          "(required setup evidence also failed)",
      );
    }
    throw primary;
  }
}

export function beforeAllWithEvidence(
  suite: string,
  setup: () => Promise<void>,
  timeout?: number,
): void {
  beforeAll(() => runSuiteSetup(suite, setup), timeout);
}
