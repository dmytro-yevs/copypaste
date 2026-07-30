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
import { cn } from "@/lib/cn";

export function StorageTab() {
  const status = useStatus();
  const clear = useClearHistory();
  const [confirming, setConfirming] = useState(false);

  return (
    <div className="flex flex-col">
      <Row title="Items stored" description="Held by the background service for this device.">
        <span className="text-sm tabular-nums text-muted-foreground">
          {status.data ? status.data.item_count.toLocaleString() : "—"}
        </span>
      </Row>

      <Row
        title="How long items are kept"
        description="The service enforces its own age and count limits. It exposes no command to read or change them, so this screen would be guessing if it showed a number."
      >
        <Badge variant="secondary">Set by the service</Badge>
      </Row>

      <Row
        title="Clear history"
        description="Deletes every unpinned item on this device. Pinned items are kept, and paired devices keep their own copies."
      >
        <Button
          variant="outline"
          size="sm"
          className="text-err-strong"
          disabled={clear.isPending}
          onClick={() => setConfirming(true)}
        >
          <Trash2 aria-hidden="true" />
          Clear history
        </Button>
      </Row>

      <AlertDialog open={confirming} onOpenChange={setConfirming}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Clear all clipboard history?</AlertDialogTitle>
            <AlertDialogDescription>
              This permanently deletes every unpinned item on this device.
              Pinned items are kept. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={() => {
                clear.mutate();
                setConfirming(false);
              }}
            >
              Clear all
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
