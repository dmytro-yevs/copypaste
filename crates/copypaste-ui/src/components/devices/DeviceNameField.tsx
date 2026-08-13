import { useEffect, useId, useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useRenameDevice } from "@/hooks/useDevices";
import { statusDeviceName, useStatus } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";

export function DeviceNameField() {
  const { t } = useTranslation();
  const inputId = useId();
  const status = useStatus(statusDeviceName);
  const rename = useRenameDevice();
  const [name, setName] = useState("");
  const current = status.data ?? "";
  const next = name.trim();

  useEffect(() => {
    if (!rename.isPending) setName(current);
  }, [current, rename.isPending]);

  return (
    <form
      className="flex w-full max-w-md items-center gap-s-2"
      onSubmit={(event) => {
        event.preventDefault();
        if (!next || next === current) return;
        rename.mutate(next, { onError: () => setName(current) });
      }}
    >
      <Label htmlFor={inputId} className="sr-only">
        {t("devices.own.rename.label")}
      </Label>
      <Input
        id={inputId}
        value={name}
        maxLength={128}
        disabled={rename.isPending || status.isPending || status.isError}
        onChange={(event) => setName(event.target.value)}
      />
      <Button
        type="submit"
        size="sm"
        variant="outline"
        disabled={rename.isPending || !next || next === current}
      >
        {t(
          rename.isPending
            ? "devices.own.rename.saving"
            : "devices.own.rename.action",
        )}
      </Button>
      <span role="status" aria-live="polite" className="sr-only">
        {rename.isPending ? t("devices.own.rename.saving") : ""}
      </span>
    </form>
  );
}
