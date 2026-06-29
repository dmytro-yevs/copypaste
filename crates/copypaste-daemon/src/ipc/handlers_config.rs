//! Config get/set IPC handlers (split from ipc god-module, ra15.1).
use super::*;

impl IpcServer {
    pub(crate) async fn dispatch_config(&self, req: Request) -> Response {
        match req.method.as_str() {
            "get_config" => {
                // Never ship account credentials over IPC. `get_config` feeds
                // the UI settings form and the CLI's read-merge-write in
                // `cloud setup`; neither needs the raw GoTrue password or email
                // back (the CLI re-supplies both on every `set_config`, the UI
                // does not surface them at all). `build_config_response` maps
                // the internal config to a typed `AppConfigResponse` that has no
                // field for either secret — only `*_set` presence flags — so a
                // leak is structurally impossible. The Supabase anon/public key
                // is, by design, a publishable key and is kept so the UI can
                // prefill the settings field.
                //
                // Fix HIGH #3: read_config() does blocking fs I/O (reads
                // config.json + config.toml); run it on the blocking thread
                // pool so the async worker is never stalled by disk I/O.
                let join = tokio::task::spawn_blocking(read_config).await;
                match join {
                    // Build the typed, redacted wire response. `AppConfigResponse`
                    // has no field that can carry a credential, so secrets cannot
                    // leak here even if a new secret field is later added to the
                    // internal `AppConfig` (CopyPaste-c4q2.18).
                    Ok(cfg) => match serde_json::to_value(build_config_response(&cfg)) {
                        Ok(v) => Response::ok(req.id, v),
                        Err(e) => Response::err(req.id, e.to_string()),
                    },
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("get_config blocking task failed: {e}"),
                    ),
                }
            }
            "set_config" => {
                let incoming: AppConfig = match serde_json::from_value(req.params.clone()) {
                    Ok(c) => c,
                    Err(e) => return Response::err(req.id, format!("invalid config: {e}")),
                };
                // Capture the requested lan_visibility toggle BEFORE we move
                // `incoming` into the blocking task, so we can hot-apply it to
                // the running DiscoveryService after the persist succeeds.
                let requested_lan_visibility = incoming.lan_visibility;
                let discovery_for_lan = self.discovery.clone();
                // Capture p2p_enabled so we can log a restart-required notice
                // after the persist succeeds. Runtime start/stop of the full P2P
                // transport stack (start_p2p) is not feasible without a large
                // refactor (CopyPaste-bjh); the persisted value is honoured on
                // the NEXT daemon restart. `None` means the caller did not send
                // the field — no change, no notice needed.
                let requested_p2p_enabled = incoming.p2p_enabled;
                // CopyPaste-44rq.67: capture whether the caller explicitly
                // cleared the relay URL (the empty-string sentinel) BEFORE
                // `incoming` is moved into the blocking task, so the running
                // relay orchestrator can be shut down after the persist succeeds.
                // `None` (omitted) is NOT a clear — only `Some("")`/whitespace.
                #[cfg(feature = "relay-sync")]
                let relay_cleared = matches!(
                    incoming.relay_url.as_deref(),
                    Some(s) if s.trim().is_empty()
                );
                #[cfg(feature = "relay-sync")]
                let relay_handle_for_clear = self.relay_handle.clone();
                // MERGE, don't overwrite. `get_config` redacts the secret
                // fields (`supabase_password`, `supabase_email`) to `*_set`
                // booleans and drops the real values, so a UI/CLI
                // read-modify-write deserialises them as `None`. A blind
                // whole-struct write would then persist null and silently WIPE
                // the stored Supabase credentials, breaking cloud sync. Merge
                // the incoming config onto the persisted one, preserving any
                // secret the caller did not supply.
                //
                // Fix HIGH #3: read_config()/write_config()/update_core_config()
                // all do blocking fs I/O; run them on the blocking thread pool.
                let core_config_arc = self.core_config.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let mut merged = merge_config(read_config(), incoming);
                    // Item 1 (keychain supabase_password): if the caller supplied a
                    // new password, migrate it to the macOS Keychain and remove it
                    // from the config struct so it is NOT written to config.json in
                    // plain text. On failure (non-macOS, unsigned build without
                    // Keychain access) we keep the existing config.json behaviour as
                    // a fallback — the password stays in merged and is written to
                    // the 0600 config.json, same as before the fix.
                    if let Some(ref pw) = merged.supabase_password.clone() {
                        match crate::keychain::store_supabase_password_to_keychain(pw) {
                            Ok(()) => {
                                // Only drop the plaintext from config.json once the
                                // Keychain ACTUALLY returns it. Under the ephemeral-key
                                // bypass (CI / unsigned dev builds) `store_*` is a no-op
                                // that still returns Ok(()); a blind strip would then
                                // silently lose the secret from both stores. The
                                // read-back confirms real persistence before we delete
                                // the on-disk copy.
                                if crate::keychain::read_supabase_password_from_keychain()
                                    .as_deref()
                                    == Some(pw.as_str())
                                {
                                    tracing::info!(
                                        "supabase_password migrated to Keychain; \
                                         removing from config.json"
                                    );
                                    merged.supabase_password = None;
                                } else {
                                    tracing::debug!(
                                        "supabase_password Keychain store is a no-op \
                                         (ephemeral/bypass mode); keeping it in config.json"
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    "supabase_password Keychain store failed; \
                                     falling back to config.json persistence"
                                );
                                // Leave merged.supabase_password as-is so
                                // write_config below persists it to the 0600
                                // config.json (existing behaviour pre-fix).
                            }
                        }
                    }
                    // Persist IPC fields (Supabase creds, p2p_enabled) to config.json.
                    write_config(&merged)?;
                    // Persist limit fields to config.toml AND return the new
                    // core config for hot-reload in the caller.
                    let new_core = update_core_config(&merged)?;
                    Ok::<_, anyhow::Error>((merged, new_core))
                })
                .await;
                match join {
                    Ok(Ok((_merged, new_core))) => {
                        if let Some(ref arc) = core_config_arc {
                            if let Ok(mut guard) = arc.write() {
                                *guard = new_core;
                            }
                        }
                        // Hot-apply lan_visibility: stop or restart mDNS-SD
                        // without a full daemon restart.
                        //
                        // When the caller explicitly sets `lan_visibility: false`,
                        // stop advertisement and browsing immediately so the device
                        // disappears from the LAN straight away. When it is
                        // re-enabled (`Some(true)`), restart mDNS so the device
                        // becomes visible again without requiring a restart. When
                        // the caller omits the field (`None`), do nothing.
                        if let Some(visible) = requested_lan_visibility {
                            if let Some(ref disc) = discovery_for_lan {
                                if visible {
                                    tracing::info!(
                                        "lan_visibility set to true — restarting mDNS-SD"
                                    );
                                    let disc_for_task = Arc::clone(disc);
                                    tokio::spawn(async move {
                                        match disc_for_task.start().await {
                                            Ok(_handle) => {
                                                tracing::info!(
                                                    "mDNS-SD restarted (lan_visibility on)"
                                                );
                                                // The handle is intentionally dropped here:
                                                // the background browse loop keeps running via
                                                // the abort_handle retained in DiscoveryService.
                                            }
                                            Err(e) => tracing::warn!(
                                                "mDNS-SD restart failed after \
                                                 lan_visibility toggle: {e}"
                                            ),
                                        }
                                    });
                                } else {
                                    tracing::info!(
                                        "lan_visibility set to false — stopping mDNS-SD"
                                    );
                                    disc.stop();
                                }
                            }
                        }
                        // CopyPaste-bjh: p2p_enabled is persisted to config.json
                        // here and honoured at the NEXT daemon startup (A-SET-4).
                        // Hot-apply (runtime start/stop of start_p2p) is not
                        // implemented; inform operators so they know a restart is
                        // needed for the toggle to take effect.
                        if let Some(enabled) = requested_p2p_enabled {
                            tracing::info!(
                                p2p_enabled = enabled,
                                "p2p_enabled persisted — change takes effect on next daemon restart"
                            );
                        }
                        // CopyPaste-44rq.67: the user cleared the relay URL —
                        // tear down the running orchestrator now (config.toml was
                        // already set to relay_url=None above). Taking the handle
                        // out of the slot and calling `shutdown()` stops the push
                        // and receive loops within one poll cycle, so relay sync
                        // is disabled at runtime without a daemon restart. Unlike
                        // p2p_enabled (which needs a restart), the relay handle is
                        // explicitly shareable for exactly this purpose.
                        #[cfg(feature = "relay-sync")]
                        if relay_cleared {
                            if let Some(handle) = relay_handle_for_clear.lock().await.take() {
                                tracing::info!(
                                    "relay_url cleared — shutting down relay orchestrator"
                                );
                                handle.shutdown();
                            } else {
                                tracing::debug!(
                                    "relay_url cleared but no relay orchestrator was running"
                                );
                            }
                        }
                        Response::ok(req.id, serde_json::json!({"saved": true}))
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("set_config blocking task failed: {e}"),
                    ),
                }
            }
            _ => self.dispatch_sync(req).await,
        }
    }
}
