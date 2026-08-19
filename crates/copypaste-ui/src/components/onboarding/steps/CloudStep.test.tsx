import { describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { CloudStep } from "@/components/onboarding/steps/CloudStep";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { en } from "@/i18n";
import type { CloudStatusData } from "@/lib/ipc";
import { withUser } from "@/test/harness";

const getCloudStatus = vi.fn();
const cloudSignIn = vi.fn();
const cloudSignUp = vi.fn();
const cloudSetEndpoint = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getCloudStatus: () => getCloudStatus(),
    cloudSignIn: (credentials: unknown) => cloudSignIn(credentials),
    cloudSignUp: (credentials: unknown) => cloudSignUp(credentials),
    cloudSetEndpoint: (url: string, anonKey: string) =>
      cloudSetEndpoint(url, anonKey),
    cloudSignOut: vi.fn(),
    syncCloudNow: vi.fn(),
  };
});

function status(over: Partial<CloudStatusData> = {}): CloudStatusData {
  return {
    configured: false,
    signed_in: false,
    key_ready: false,
    email: null,
    last_sync_ms: null,
    last_error: null,
    poll_interval_secs: 60,
    unreadable_uploads: 0,
    ...over,
  };
}

function props(over: Partial<OnboardingStepProps> = {}): OnboardingStepProps {
  return {
    id: "cloud",
    platform: "desktop",
    optional: true,
    index: 2,
    total: 3,
    continue: vi.fn(),
    skip: vi.fn(),
    skipRemaining: vi.fn(),
    ...over,
  };
}

describe("CloudStep", () => {
  it("keeps local-only as Continue and hides project keys until Advanced", async () => {
    getCloudStatus.mockResolvedValue(status());
    const step = props();
    const { user } = withUser(<CloudStep {...step} />);

    expect(
      await screen.findByRole("heading", { name: en.onboarding.cloud.title }),
    ).toBeTruthy();
    expect(screen.queryByLabelText(en.onboarding.cloud.url)).toBeNull();
    expect(screen.queryByLabelText(en.settings.sync.cloud.email)).toBeNull();

    await user.click(screen.getByText(en.onboarding.cloud.advanced));
    expect(screen.getByLabelText(en.onboarding.cloud.url)).toBeTruthy();

    await user.click(screen.getByRole("button", { name: en.onboarding.cloud.action }));
    expect(step.continue).toHaveBeenCalledOnce();
  });

  it("signs in on the hosted path without asking for a URL", async () => {
    getCloudStatus.mockResolvedValue(status({ configured: true }));
    cloudSignIn.mockResolvedValue(
      status({ configured: true, signed_in: true, key_ready: true }),
    );
    const step = props();
    const { user } = withUser(<CloudStep {...step} />);

    await screen.findByText(en.onboarding.cloud.hosted);
    expect(screen.queryByLabelText(en.onboarding.cloud.url)).toBeNull();
    await user.type(screen.getByLabelText(en.settings.sync.cloud.email), "me@example.com");
    await user.type(screen.getByLabelText(en.settings.sync.cloud.password), "secret");
    await user.type(screen.getByLabelText(en.settings.sync.cloud.passphrase), "passphrase");
    await user.click(screen.getByRole("button", { name: en.onboarding.cloud.signIn }));

    await waitFor(() =>
      expect(cloudSignIn).toHaveBeenCalledWith({
        email: "me@example.com",
        password: "secret",
        passphrase: "passphrase",
      }),
    );
    await waitFor(() => expect(step.continue).toHaveBeenCalledOnce());
  });
});
