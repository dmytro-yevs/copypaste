import { Suspense } from "react";

import { Boundary } from "@/app/shell/Boundary";
import { useTranslation } from "@/i18n";
import { useUi } from "@/store/ui";
import { StateView } from "@/components/shared/StateView";
import { screenRegistry } from "./screenRegistry";

export function ScreenRouter({ pushLive }: { pushLive: boolean }) {
  const { t } = useTranslation();
  const view = useUi((state) => state.view);
  const screen = screenRegistry[view];
  const label = t(screen.label);
  return (
    <Boundary key={view} label={label} layout="screen" onReset={screen.reset}>
      <Suspense
        fallback={
          <StateView
            mode="loading"
            placement="screen"
            title={label}
            aria-label={t("shell.loading", { region: label })}
          />
        }
      >
        {screen.render(pushLive)}
      </Suspense>
    </Boundary>
  );
}
