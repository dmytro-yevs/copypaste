import { useCallback, useEffect, useRef, useState } from "react";
import { invokeNative } from "@/lib/nativeInvoke";

import { StateView } from "@/components/shared/StateView";
import { Button, Dialog, Input } from "@/components/ui";
import type {
  SecureInviteView,
  SecurePairingView,
  SecureSasView,
} from "@/generated/ipc";
import styles from "./nativePresentation.module.css";

type Revealed =
  | { kind: "invite"; value: SecureInviteView; expiresAt: number }
  | { kind: "sas"; value: SecureSasView; expiresAt: number };

/** Mounted only in the protected, label-checked `pairing` WebView. */
export default function ProtectedPairingSurface() {
  const [view, setView] = useState<SecurePairingView | null>(null);
  const [revealed, setRevealed] = useState<Revealed | null>(null);
  const [code, setCode] = useState("");
  const [addr, setAddr] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());
  const generation = useRef<number | null>(null);
  const revealEpoch = useRef(0);
  const refreshIssued = useRef(0);

  const refresh = useCallback(async () => {
    const issued = ++refreshIssued.current;
    try {
      const next = await invokeNative("pair_secure_state");
      if (issued !== refreshIssued.current) return;
      if (generation.current !== next.generation) {
        generation.current = next.generation;
        revealEpoch.current += 1;
        setRevealed(null);
      }
      setView((old) => {
        if (old?.phase !== next.phase) {
          revealEpoch.current += 1;
          setRevealed(null);
        }
        return next;
      });
    } catch {
      if (issued !== refreshIssued.current) return;
      setError("This protected pairing session is unavailable. Close it and try again.");
      revealEpoch.current += 1;
      setRevealed(null);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const poll = window.setInterval(() => { void refresh(); }, 500);
    const expiry = window.setInterval(() => setNow(Date.now()), 250);
    const pageGone = () => {
      revealEpoch.current += 1;
      setRevealed(null);
      setCode("");
      setAddr("");
    };
    const hidden = () => { if (document.hidden) pageGone(); };
    window.addEventListener("pagehide", pageGone);
    document.addEventListener("visibilitychange", hidden);
    return () => {
      window.clearInterval(poll);
      window.clearInterval(expiry);
      window.removeEventListener("pagehide", pageGone);
      document.removeEventListener("visibilitychange", hidden);
      revealEpoch.current += 1;
      refreshIssued.current += 1;
    };
  }, [refresh]);

  useEffect(() => {
    if (revealed && now >= revealed.expiresAt) {
      revealEpoch.current += 1;
      setRevealed(null);
    }
  }, [now, revealed]);

  const run = async (action: () => Promise<unknown>) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await action();
      await refresh();
    } catch {
      setError("The pairing action could not complete. Check both devices and try again.");
      setRevealed(null);
    } finally {
      setBusy(false);
    }
  };

  const close = () => {
    if (busy) return;
    revealEpoch.current += 1;
    setRevealed(null);
    setCode("");
    setAddr("");
    void invokeNative("pair_secure_close").catch(() => {
      setError("The pairing window could not close. Try again.");
    });
  };
  const revealInvite = () => run(async () => {
    const expected = generation.current;
    if (expected === null) return;
    const epoch = revealEpoch.current;
    const requestedAt = Date.now();
    const value = await invokeNative("pair_secure_reveal_invite", { generation: expected });
    if (epoch !== revealEpoch.current || value.generation !== generation.current || value.ceremony_id !== view?.ceremony.ceremony_id) return;
    const expiresAt = requestedAt + value.expires_in_ms;
    if (Date.now() >= expiresAt) return;
    setRevealed({ kind: "invite", value, expiresAt });
  });
  const revealSas = () => run(async () => {
    const expected = generation.current;
    if (expected === null) return;
    const epoch = revealEpoch.current;
    const requestedAt = Date.now();
    const value = await invokeNative("pair_secure_reveal_sas", { generation: expected });
    if (epoch !== revealEpoch.current || value.generation !== generation.current || value.ceremony_id !== view?.ceremony.ceremony_id) return;
    const expiresAt = requestedAt + value.expires_in_ms;
    if (Date.now() >= expiresAt) return;
    setRevealed({ kind: "sas", value, expiresAt });
  });
  const join = () => run(async () => {
    const expected = generation.current;
    if (expected === null) return;
    const enteredCode = code;
    const enteredAddr = addr;
    setCode("");
    setAddr("");
    await invokeNative("pair_secure_join", { generation: expected, code: enteredCode, addr: enteredAddr });
  });
  const decide = (accept: boolean) => run(async () => {
    const expected = generation.current;
    if (expected === null) return;
    setRevealed(null);
    await invokeNative("pair_secure_decide", { generation: expected, accept });
  });

  const phase = view?.phase ?? "loading";
  const active = phase === "join" || phase === "invite" || phase === "progress" || phase === "confirm";
  const title = phase === "join" ? "Join another device" : phase === "confirm" ? "Compare security codes" : "Connect a device";
  const visible = revealed && now < revealed.expiresAt ? revealed : null;
  const ceremony = view?.ceremony;

  return (
    <Dialog
      open
      onOpenChange={(open) => { if (!open) close(); }}
      title={title}
      description="Show pairing details only when the other device is ready."
      showCloseButton={false}
      contentProps={{
        className: styles.dialog,
        onCopy: (event) => event.preventDefault(),
        onCut: (event) => event.preventDefault(),
        onDragStart: (event) => event.preventDefault(),
        onContextMenu: (event) => event.preventDefault(),
        onEscapeKeyDown: (event) => { event.preventDefault(); close(); },
        onPointerDownOutside: (event) => event.preventDefault(),
      }}
      footer={<>
        {active && <Button type="button" variant="secondary" disabled={busy} onClick={close}>Cancel pairing</Button>}
        {!active && <Button type="button" onClick={close}>Done</Button>}
      </>}
    >
      {error && <StateView mode="error" placement="inline" title={error} />}
      {phase === "loading" && <StateView mode="loading" title="Opening pairing" />}
      {phase === "join" && (
        <form className={styles.form} onSubmit={(event) => { event.preventDefault(); void join(); }}>
          <label>Pairing code<Input type="password" autoComplete="off" value={code} onChange={(event) => setCode(event.target.value)} required /></label>
          <label>Pairing address<Input type="password" autoComplete="off" value={addr} onChange={(event) => setAddr(event.target.value)} required /></label>
          <Button type="submit" disabled={busy || !code || !addr}>Join</Button>
        </form>
      )}
      {phase === "invite" && (
        <div className={styles.stack}>
          {!visible || visible.kind !== "invite" ? (
            <StateView mode="info" title="Ready to show a pairing code" description="Reveal it only when the other device is ready to scan or enter it." actions={<Button type="button" disabled={busy} onClick={() => void revealInvite()}>Reveal code</Button>} />
          ) : (
            <div className={styles.secret}>
              <img className={styles.qr} draggable={false} src={`data:image/svg+xml,${encodeURIComponent(visible.value.qr_svg)}`} alt="Pairing QR code" />
              <p>Code: <strong>{visible.value.code}</strong></p>
              <p>Address: <strong>{visible.value.address}</strong></p>
            </div>
          )}
        </div>
      )}
      {phase === "progress" && <StateView mode="loading" title="Connecting devices" description="Keep both devices open while the secure connection is established." />}
      {phase === "confirm" && (
        <div className={styles.stack}>
          {!visible || visible.kind !== "sas" ? (
            <StateView mode="warning" title="Compare security codes" description="Show the security code and compare it on both devices before confirming." actions={<Button type="button" disabled={busy} onClick={() => void revealSas()}>Show security code</Button>} />
          ) : (
            <div className={styles.secret}>
              <p>Security code</p>
              <output className={styles.sas}>{visible.value.sas}</output>
              <p>Does this match the other device?</p>
              <div className={styles.actions}>
                <Button type="button" disabled={busy} variant="secondary" onClick={() => void decide(false)}>No, reject</Button>
                <Button type="button" disabled={busy} onClick={() => void decide(true)}>Yes, confirm</Button>
              </div>
            </div>
          )}
        </div>
      )}
      {phase === "terminal" && <StateView mode={ceremony?.state === "confirmed" ? "success" : "warning"} title={ceremony?.state === "confirmed" ? "Device connected" : "Pairing ended"} description={ceremony?.state === "confirmed" ? "The device is ready to sync." : "No device was paired."} />}
    </Dialog>
  );
}
