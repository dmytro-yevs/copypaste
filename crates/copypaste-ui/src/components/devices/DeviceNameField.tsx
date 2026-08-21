import { useEffect, useId, useState } from "react";
import { LoaderCircle, Pencil } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useRenameDevice } from "@/hooks/useDevices";
import { statusDeviceName, useStatus } from "@/hooks/useStatus";
import { useTranslation } from "@/i18n";

export function DeviceNameField() {
  const { t } = useTranslation();
  const inputId = useId();
  const status = useStatus(statusDeviceName);
  const rename = useRenameDevice();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const current = status.data ?? "";
  const next = name.trim();

  useEffect(() => {
    if (!rename.isPending) setName(current);
  }, [current, rename.isPending]);

  const disabled = rename.isPending || status.isPending || status.isError;
  const unchanged = !next || next === current;

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (rename.isPending) return;
        setOpen(nextOpen);
      }}
    >
      <Button
        type="button"
        size="icon-sm"
        variant="ghost"
        aria-label={t("devices.own.rename.open")}
        title={t("devices.own.rename.open")}
        disabled={disabled}
        onClick={() => setOpen(true)}
      >
        <Pencil aria-hidden="true" />
      </Button>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("devices.own.rename.title")}</DialogTitle>
          <DialogDescription>
            {t("devices.own.rename.description")}
          </DialogDescription>
        </DialogHeader>
        <form
          className="flex flex-col gap-s-4"
          onSubmit={(event) => {
            event.preventDefault();
            if (unchanged) return;
            rename.mutate(next, {
              onSuccess: () => setOpen(false),
              onError: () => setName(current),
            });
          }}
        >
          <div className="flex flex-col gap-s-2">
            <Label htmlFor={inputId}>{t("devices.own.rename.label")}</Label>
            <Input
              id={inputId}
              value={name}
              maxLength={128}
              disabled={disabled}
              onChange={(event) => setName(event.target.value)}
            />
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              disabled={rename.isPending}
              onClick={() => {
                setName(current);
                setOpen(false);
              }}
            >
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={disabled || unchanged}>
              {rename.isPending ? (
                <>
                  <LoaderCircle
                    className="animate-spin motion-reduce:animate-none"
                    aria-hidden="true"
                  />
                  {t("devices.own.rename.saving")}
                </>
              ) : (
                t("devices.own.rename.action")
              )}
            </Button>
          </DialogFooter>
          <span role="status" aria-live="polite" className="sr-only">
            {rename.isPending ? t("devices.own.rename.saving") : ""}
          </span>
        </form>
      </DialogContent>
    </Dialog>
  );
}
