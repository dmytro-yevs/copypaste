/**
 * Three of the four actions here are irreversible in some direction, so each
 * one that is asks first and says what it costs (AGENTS.md rule 4):
 *
 * * Export is not destructive but it *leaks* — every clip as readable text —
 *   and the opt-in for flagged items sits inside the dialog, the second ask.
 * * Import parses first, then asks with the real item count before storing.
 * * Restore replaces everything, pinned included, so the dialog names what goes
 *   and its affirmative names the next step rather than saying "OK" (A11Y-3).
 *
 * No path enters this component, so none can reach the DOM (INV-12).
 */
import { useState } from "react";
import { Archive, Download, RotateCcw, Trash2, Upload, X } from "lucide-react";

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
import { Button, buttonVariants } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Row } from "@/components/settings/Row";
import { useDeferredDelete } from "@/hooks/useDeferredDelete";
import { statusItemCount, useStatus } from "@/hooks/useStatus";
import {
  useBackupDatabase,
  useExportHistory,
  useImportHistory,
  useRestoreDatabase,
} from "@/hooks/useServiceConfig";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import type { ImportPreview } from "@/lib/ipc";

type Confirming = "none" | "clear" | "export" | "restore";

export function StorageTab() {
  const { t } = useTranslation();
  const status = useStatus(statusItemCount);
  // Its own instance rather than one shared with the history list (DMY-168).
  // Only one screen is mounted at a time, so the two can never both hold a
  // pending clear, and a module-level store would buy nothing here.
  const { removeAll } = useDeferredDelete();
  const exportHistory = useExportHistory();
  const importHistory = useImportHistory();
  const backup = useBackupDatabase();
  const restore = useRestoreDatabase();

  const [confirming, setConfirming] = useState<Confirming>("none");
  const [includeSensitive, setIncludeSensitive] = useState(false);
  const [pendingImport, setPendingImport] = useState<ImportPreview | null>(null);

  const close = () => setConfirming("none");

  return (
    <div className="flex flex-col">
      <Row
        title={t("settings.storage.stored.title")}
        description={t("settings.storage.stored.description")}
      >
        <span className="text-sm tabular-nums text-muted-foreground">
          {status.data !== undefined
            ? status.data.toLocaleString()
            : t("common.noValue")}
        </span>
      </Row>

      <Row
        title={t("settings.transfer.export.title")}
        description={t("settings.transfer.export.description")}
      >
        <Button
          variant="outline"
          size="sm"
          disabled={exportHistory.isPending}
          onClick={() => {
            // Reset every time: an opt-in that remembers is not an opt-in.
            setIncludeSensitive(false);
            setConfirming("export");
          }}
        >
          <Download aria-hidden="true" />
          {t("settings.transfer.export.action")}
        </Button>
      </Row>

      <Row
        title={t("settings.transfer.import.title")}
        description={t("settings.transfer.import.description")}
      >
        <Button
          variant="outline"
          size="sm"
          disabled={importHistory.isPending}
          onClick={() => {
            importHistory.prepare.mutate(undefined, {
              onSuccess: (preview) => setPendingImport(preview),
            });
          }}
        >
          <Upload aria-hidden="true" />
          {t("settings.transfer.import.action")}
        </Button>
      </Row>

      <Row
        title={t("settings.transfer.backup.title")}
        description={t("settings.transfer.backup.description")}
      >
        <Button
          variant="outline"
          size="sm"
          disabled={backup.isPending}
          onClick={() => backup.mutate()}
        >
          <Archive aria-hidden="true" />
          {t("settings.transfer.backup.action")}
        </Button>
      </Row>

      <Row
        title={t("settings.transfer.restore.title")}
        description={t("settings.transfer.restore.description")}
      >
        <Button
          variant="outline"
          size="sm"
          className="text-err-strong"
          disabled={restore.isPending}
          onClick={() => setConfirming("restore")}
        >
          <RotateCcw aria-hidden="true" />
          {t("settings.transfer.restore.action")}
        </Button>
      </Row>

      <Row
        title={t("settings.storage.clear.title")}
        description={t("settings.storage.clear.description")}
      >
        <Button
          variant="outline"
          size="sm"
          className="text-err-strong"
          onClick={() => setConfirming("clear")}
        >
          <Trash2 aria-hidden="true" />
          {t("settings.storage.clear.action")}
        </Button>
      </Row>

      <AlertDialog open={confirming === "clear"} onOpenChange={close}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("history.clear.title")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("history.clear.body")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>
              <X aria-hidden="true" />
              {t("common.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={() => {
                void removeAll();
                close();
              }}
            >
              <Trash2 aria-hidden="true" />
              {t("history.clear.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog open={confirming === "export"} onOpenChange={close}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("settings.transfer.export.dialogTitle")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("settings.transfer.export.dialogBody")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="flex flex-col gap-s-1">
            <div className="flex items-center gap-s-2">
              <Checkbox
                id="export-include-sensitive"
                checked={includeSensitive}
                onCheckedChange={(checked) => setIncludeSensitive(checked === true)}
              />
              <Label htmlFor="export-include-sensitive">
                {t("settings.transfer.export.includeSensitive")}
              </Label>
            </div>
            <p className="text-xs text-muted-foreground">
              {t("settings.transfer.export.includeSensitiveHint")}
            </p>
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel>
              <X aria-hidden="true" />
              {t("common.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                exportHistory.mutate(includeSensitive);
                close();
              }}
            >
              <Download aria-hidden="true" />
              {t("settings.transfer.export.confirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog
        open={pendingImport !== null}
        onOpenChange={(open) => {
          if (open || pendingImport === null) return;
          importHistory.cancel.mutate(pendingImport.token);
          setPendingImport(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("settings.transfer.import.dialogTitle", {
                count: pendingImport?.item_count ?? 0,
              })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("settings.transfer.import.dialogBody", {
                count: pendingImport?.item_count ?? 0,
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>
              <X aria-hidden="true" />
              {t("common.cancel")}
            </AlertDialogCancel>
            <Button
              disabled={importHistory.apply.isPending}
              onClick={() => {
                if (pendingImport === null) return;
                importHistory.apply.mutate(pendingImport.token);
                setPendingImport(null);
              }}
            >
              <Upload aria-hidden="true" />
              {t("settings.transfer.import.confirm")}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog open={confirming === "restore"} onOpenChange={close}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("settings.transfer.restore.dialogTitle")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("settings.transfer.restore.dialogBody")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <p className="text-xs text-muted-foreground">
            {t("settings.transfer.restore.dialogSafety")}
          </p>
          <AlertDialogFooter>
            <AlertDialogCancel>
              <X aria-hidden="true" />
              {t("common.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={() => {
                restore.mutate();
                close();
              }}
            >
              <RotateCcw aria-hidden="true" />
              {t("settings.transfer.restore.confirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
