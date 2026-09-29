import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { CAPTURE_KEY, useCaptureState } from "@/hooks/useCapture";
import { toFriendly } from "@/lib/errors";
import { t } from "@/i18n";
import { captureSetupInstructions, type CaptureSnapshot } from "@/lib/ipc";
import { usePrefs } from "@/store/prefs";

class CheckpointError extends Error {}

export function useAndroidCaptureSetup() {
  const capture = useCaptureState();
  const client = useQueryClient();
  const method = usePrefs((state) => state.onboarding.captureSetupMethod);
  const checkpoint = usePrefs((state) => state.checkpointOnboarding);
  const instructions = useQuery({
    queryKey: ["capture", "setup-instructions"],
    queryFn: captureSetupInstructions,
    enabled: method === "adb",
    retry: false,
  });
  const action = useMutation({
    mutationFn: async ({ run, nextMethod = method }: {
      run?: () => Promise<void | CaptureSnapshot>;
      stage?: "commands" | "verify";
      nextMethod?: "shizuku" | "adb";
    }) => {
      if (!await checkpoint({ captureSetupMethod: nextMethod })) {
        throw new CheckpointError(t("onboarding.capture.setup.saveFailed"));
      }
      const fresh = await run?.();
      if (fresh) {
        client.setQueryData(CAPTURE_KEY, fresh);
      }
    },
  });
  return { capture, method, instructions, action, error: action.error instanceof CheckpointError ? action.error.message : action.error ? toFriendly(action.error) : null };
}
