import { useState } from "react";
import { Trash2 } from "lucide-react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { Row } from "@/components/settings/Row";
import { useClearHistory, useStatus } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";

export function StorageTab() {
  const { t } = useTranslation();
  const status = useStatus();
  const clear = useClearHistory();
  const [confirming, setConfirming] = useState(false);

  return (
    <div className="flex flex-col">
      <Row
        title={t("settings.storage.stored.title")}
        description={t("settings.storage.stored.description")}
      >
        <span className="text-sm tabular-nums text-muted-foreground">
          {status.data
            ? status.data.item_count.toLocaleString()
            : t("common.noValue")}
        </span>
      </Row>

      <Row
        title={t("settings.storage.retention.title")}
        description={t("settings.storage.retention.description")}
      >
        <Badge variant="secondary">{t("settings.storage.retention.badge")}</Badge>
      </Row>

      <Row
        title={t("settings.storage.clear.title")}
        description={t("settings.storage.clear.description")}
      >
        <Button
          variant="outline"
          size="sm"
          className="text-err-strong"
          disabled={clear.isPending}
          onClick={() => setConfirming(true)}
        >
          <Trash2 aria-hidden="true" />
          {t("settings.storage.clear.action")}
        </Button>
      </Row>

      <AlertDialog open={confirming} onOpenChange={setConfirming}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("history.clear.title")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("history.clear.body")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={() => {
                clear.mutate();
                setConfirming(false);
              }}
            >
              {t("history.clear.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
