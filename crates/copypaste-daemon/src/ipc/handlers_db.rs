//! Database admin IPC handlers: reset/vacuum/stats/backup/restore (split from ipc god-module, ra15.1).
use super::*;

impl IpcServer {
    pub(crate) async fn dispatch_db(&self, req: Request) -> Response {
        match req.method.as_str() {
            // ------------------------------------------------------------------
            // Destructive recovery: wipe + recreate the clipboard database.
            //
            // This is the explicit escape hatch for a daemon stuck in DEGRADED
            // mode because `clipboard.db` cannot be decrypted (key mismatch /
            // "file is not a database"). UNLIKE every other DB-touching method,
            // this one is NOT gated behind the `ready` flag — recovering FROM
            // degraded mode is its entire reason to exist, so it must run while
            // `ready = false`. It therefore appears BEFORE the readiness gate in
            // spirit (the gate's `requires_db` allow-list deliberately omits it).
            // ------------------------------------------------------------------
            "reset_database" => {
                // Guard #1: an explicit confirm flag is mandatory so a stray or
                // replayed call can never erase the user's history by accident.
                let confirm = req
                    .params
                    .get("confirm")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !confirm {
                    tracing::warn!(
                        "reset_database rejected: missing confirm=true — refusing \
                         to wipe the clipboard database without explicit confirmation"
                    );
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "reset_database requires confirm=true",
                    );
                }

                let db_path = crate::paths::db_path();
                tracing::warn!(
                    db_path = %db_path.display(),
                    "reset_database INVOKED: WIPING and RECREATING the clipboard \
                     database. All local clipboard history will be PERMANENTLY \
                     DELETED. This is the user-confirmed recovery escape hatch for \
                     a daemon stuck in degraded mode (undecryptable DB)."
                );

                // Resolve the key for the FRESH database. Prefer the real
                // device key from the Keychain (so the new DB re-opens normally
                // on the next restart); if that is unreachable (the very reason
                // we may be degraded), fall back to the key this server already
                // holds. Either way the fresh empty DB is self-consistent and
                // immediately usable this session.
                let fresh_key: zeroize::Zeroizing<[u8; 32]> = {
                    #[cfg(target_os = "macos")]
                    {
                        match crate::keychain::load_or_create() {
                            Ok(kp) => {
                                tracing::info!(
                                    "reset_database: using the device Keychain key for the \
                                     fresh database"
                                );
                                kp.local_enc_key()
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    "reset_database: Keychain key unavailable; recreating the \
                                     fresh database with the daemon's current in-memory key"
                                );
                                zeroize::Zeroizing::new(**self.local_key)
                            }
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        zeroize::Zeroizing::new(**self.local_key)
                    }
                };

                // Do the destructive filesystem work + reopen on a blocking
                // thread (rusqlite is sync). We hold the DB mutex for the whole
                // operation so no other request can touch the handle mid-swap.
                let db_arc = self.db.clone();
                let path_for_task = db_path.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let mut guard = db_arc.blocking_lock();

                    // CopyPaste-2lc9 (belt #2): checkpoint the OLD connection
                    // before we swap it out. Flushing outstanding WAL frames to
                    // the main file minimises the window during which a stale
                    // WAL file can survive the delete in step 2 and be replayed
                    // onto the fresh DB created in step 3 — which would trigger
                    // the "duplicate column name: content_hash" race inside
                    // `apply_migrations`. A failed checkpoint is non-fatal: the
                    // WAL file is still deleted below; the checkpoint only
                    // tightens the race window, it does not eliminate the
                    // `wal_checkpoint(TRUNCATE)` call at the top of
                    // `apply_migrations` (which is the authoritative fix).
                    if let Err(e) = guard
                        .conn()
                        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                    {
                        tracing::warn!(
                            error = %e,
                            "reset_database: wal_checkpoint(TRUNCATE) on old \
                             handle failed; WAL will still be removed in step 2"
                        );
                    }

                    // 1. Close the current connection. Swapping in a throwaway
                    //    in-memory DB drops the old `Database` (and its open
                    //    file handles / WAL) so the files can be removed cleanly.
                    *guard = Database::open_in_memory()
                        .map_err(|e| format!("failed to open transient in-memory DB: {e}"))?;

                    // 2. Delete clipboard.db and its WAL/SHM siblings. A missing
                    //    file is fine (NotFound is not an error here).
                    for suffix in ["", "-wal", "-shm"] {
                        let mut p = path_for_task.clone().into_os_string();
                        p.push(suffix);
                        let p = std::path::PathBuf::from(p);
                        match std::fs::remove_file(&p) {
                            Ok(()) => {
                                tracing::warn!(file = %p.display(), "reset_database: deleted")
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                            Err(e) => {
                                return Err(format!("failed to delete {}: {e}", p.display()));
                            }
                        }
                    }

                    // 3. Recreate a fresh empty encrypted DB with the chosen key
                    //    using the SAME open/migrate path a clean install uses.
                    let fresh = Database::open(&path_for_task, &fresh_key)
                        .map_err(|e| format!("failed to create fresh database: {e}"))?;

                    // 4. Ensure the additive audit table the IPC layer relies on
                    //    exists, matching the normal `serve()` startup path.
                    if let Err(e) = ensure_revoked_devices_table(fresh.conn()) {
                        tracing::warn!("reset_database: ensure_revoked_devices_table failed: {e}");
                    }

                    // 5. Install the fresh DB as the live handle.
                    *guard = fresh;
                    Ok::<(), String>(())
                })
                .await;

                match join {
                    Ok(Ok(())) => {
                        // Bring the daemon OUT of degraded mode IN-PLACE: the new
                        // empty DB is live, so flip readiness on and clear the
                        // degraded reason. Subsequent history_page / status calls
                        // now succeed without a process restart.
                        self.ready.store(true, Ordering::Relaxed);
                        *self
                            .degraded_reason
                            .lock()
                            .unwrap_or_else(|p| p.into_inner()) = None;
                        tracing::warn!(
                            db_path = %db_path.display(),
                            "reset_database COMPLETE: fresh empty database created, daemon \
                             recovered in-place (no longer degraded, ready=true)"
                        );
                        // ResetDatabaseResponse carries both `reset` and `ready`.
                        // `reset` is kept because the TypeScript ResetDatabaseResult
                        // interface declares it and UI callers read `data.reset`.
                        Response::ok(req.id, serde_json::json!({ "reset": true, "ready": true }))
                    }
                    Ok(Err(msg)) => {
                        tracing::error!(
                            db_path = %db_path.display(),
                            error = %msg,
                            "reset_database FAILED: the clipboard database could not be \
                             wiped/recreated. The daemon remains in its prior state."
                        );
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("reset_database blocking task failed: {e}"),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // Database maintenance
            // ------------------------------------------------------------------
            "vacuum" => {
                // Parse optional flags; both default to false so a bare `{}`
                // params object runs the full VACUUM + REINDEX path.
                let reindex_only = req
                    .params
                    .get("reindex_only")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let dry_run = req
                    .params
                    .get("dry_run")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let db_arc = self.db.clone();
                let db_path = crate::paths::db_path();
                let join = tokio::task::spawn_blocking(move || {
                    let guard = db_arc.blocking_lock();

                    // Stat the file before any writes so we can report
                    // reclaimed bytes.  The stat uses the filesystem path, not
                    // in-memory pages, so it accurately reflects WAL state.
                    let size_before = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

                    if dry_run {
                        // Verify the connection is healthy by running a cheap
                        // read-only statement; does NOT mutate anything.
                        guard
                            .conn()
                            .execute_batch("SELECT COUNT(*) FROM clipboard_items")
                            .map_err(|e| format!("dry-run DB probe failed: {e}"))?;
                        return Ok((size_before, size_before));
                    }

                    if !reindex_only {
                        // Flush WAL pages into the main file before VACUUM so
                        // the "after" size reflects the fully compacted state.
                        // A failed checkpoint is non-fatal — log and continue.
                        if let Err(e) = guard
                            .conn()
                            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
                        {
                            tracing::warn!(
                                error = %e,
                                "vacuum: wal_checkpoint(TRUNCATE) failed; \
                                 continuing with VACUUM (after-size may be inflated)"
                            );
                        }
                        guard
                            .conn()
                            .execute_batch("VACUUM")
                            .map_err(|e| format!("VACUUM failed: {e}"))?;
                    }

                    guard
                        .conn()
                        .execute_batch("REINDEX")
                        .map_err(|e| format!("REINDEX failed: {e}"))?;

                    // Drop the guard so the OS flushes pending writes before
                    // we stat the file for the "after" size.
                    drop(guard);

                    let size_after = std::fs::metadata(&db_path)
                        .map(|m| m.len())
                        .unwrap_or(size_before);

                    Ok::<(u64, u64), String>((size_before, size_after))
                })
                .await;

                match join {
                    Ok(Ok((size_before, size_after))) => {
                        let reclaimed = size_before as i64 - size_after as i64;
                        tracing::info!(
                            size_before,
                            size_after,
                            reclaimed,
                            reindex_only,
                            dry_run,
                            "vacuum: completed"
                        );
                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "ok": true,
                                "size_before": size_before,
                                "size_after": size_after,
                                "reclaimed": reclaimed,
                            }),
                        )
                    }
                    Ok(Err(msg)) => {
                        tracing::error!(error = %msg, "vacuum: operation failed");
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("vacuum blocking task failed: {e}"),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // db_stats — lightweight storage summary (CopyPaste-40gl)
            //
            // Used by the macOS UI settings panel (SettingsView.gq51) to show
            // item count and approximate on-disk size without the full `stats`
            // computation. Returns { item_count, size_bytes }.
            // ------------------------------------------------------------------
            "db_stats" => {
                let db_arc = self.db.clone();
                let db_path = crate::paths::db_path();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // Count all rows including tombstones — same contract as
                    // COUNT(*) so the number is consistent with `stats`.
                    let item_count: u64 = db
                        .conn()
                        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| {
                            r.get::<_, i64>(0)
                        })
                        .map(|n| n.max(0) as u64)
                        .unwrap_or(0);
                    // File size from the filesystem — excludes the WAL file so
                    // it matches what a user sees in Finder / du. Returns 0 when
                    // the file doesn't exist yet (fresh install).
                    let size_bytes: u64 = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
                    (item_count, size_bytes)
                })
                .await;
                match join {
                    Ok((item_count, size_bytes)) => Response::ok(
                        req.id,
                        serde_json::json!({ "item_count": item_count, "size_bytes": size_bytes }),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("db_stats blocking task failed: {e}"),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // db_backup — in-process SQLCipher backup (CopyPaste-x94p)
            //
            // Uses `VACUUM INTO '<dest>'` which produces a consistent, hot
            // copy encrypted with the same key as the source database. The
            // daemon does NOT need to stop. The destination file must not
            // already exist (refuses overwrite for safety).
            // ------------------------------------------------------------------
            "db_backup" => {
                let dest_path = match req.params.get("dest_path").and_then(|v| v.as_str()) {
                    Some(s) if !s.is_empty() => s.to_string(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "db_backup requires a non-empty dest_path",
                        )
                    }
                };

                // Refuse to overwrite an existing file so a mis-aimed backup
                // cannot silently clobber a good previous backup.
                if std::path::Path::new(&dest_path).exists() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("db_backup: dest_path already exists: {dest_path}"),
                    );
                }

                let db_arc = self.db.clone();
                let dest_path_clone = dest_path.clone();
                let join = tokio::task::spawn_blocking(move || {
                    // Acquire the write-mutex so no other IPC call can mutate
                    // the DB mid-backup. `VACUUM INTO` takes a consistent
                    // snapshot of all non-empty pages; holding the lock ensures
                    // the snapshot is atomic from the daemon's perspective.
                    let guard = db_arc.blocking_lock();

                    // Ensure the parent directory exists so the error message
                    // is clear if it doesn't.
                    if let Some(parent) = std::path::Path::new(&dest_path_clone).parent() {
                        if !parent.as_os_str().is_empty() && !parent.exists() {
                            return Err(format!(
                                "db_backup: parent directory does not exist: {}",
                                parent.display()
                            ));
                        }
                    }

                    // VACUUM INTO copies all live pages into dest, encrypted
                    // with the same SQLCipher key as the source. This is the
                    // same mechanism the shell script used via `sqlcipher .backup`,
                    // but done in-process without stopping the daemon.
                    guard
                        .conn()
                        .execute_batch(&format!(
                            "VACUUM INTO '{}'",
                            dest_path_clone.replace('\'', "''")
                        ))
                        .map_err(|e| format!("VACUUM INTO failed: {e}"))?;

                    // Set restrictive permissions on the backup file so other
                    // local users cannot read the encrypted database.
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(meta) = std::fs::metadata(&dest_path_clone) {
                            let mut perms = meta.permissions();
                            perms.set_mode(0o600);
                            let _ = std::fs::set_permissions(&dest_path_clone, perms);
                        }
                    }

                    let size_bytes = std::fs::metadata(&dest_path_clone)
                        .map(|m| m.len())
                        .unwrap_or(0);

                    Ok::<u64, String>(size_bytes)
                })
                .await;

                match join {
                    Ok(Ok(size_bytes)) => {
                        tracing::info!(
                            dest = %dest_path,
                            size_bytes,
                            "db_backup: backup created successfully"
                        );
                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "ok": true,
                                "dest_path": dest_path,
                                "size_bytes": size_bytes,
                            }),
                        )
                    }
                    Ok(Err(msg)) => {
                        tracing::error!(error = %msg, "db_backup: failed");
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("db_backup blocking task failed: {e}"),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // db_restore — replace the live DB with a backup (CopyPaste-8wbt,
            // crh3.6, crh3.2)
            //
            // VALIDATE-then-SWAP. The previous implementation moved/deleted the
            // live DB *before* checking the backup could be opened, so a
            // wrong-key or corrupt backup permanently destroyed the user's
            // history. The flow is now:
            //   PHASE A (no live mutation): copy the backup to a throwaway
            //     staging file, open it with the real device key, run an
            //     integrity_check and a schema sanity check. Any failure aborts
            //     with the live DB completely untouched.
            //   PHASE B (only after A succeeds): quiesce, move the live DB aside
            //     (BOTH force and non-force, so a late failure can roll back),
            //     copy the validated backup into place, reopen. On any Phase-B
            //     failure the aside files are moved back and the original DB is
            //     reopened. `force` only controls whether the aside safety copy
            //     is deleted on success.
            // Degraded mode (crh3.6): the key is resolved from the Keychain, not
            // the daemon's throwaway in-memory key. crh3.2: the r2d2 read pool is
            // rebuilt against the restored file so reads stop serving stale data.
            // ------------------------------------------------------------------
            "db_restore" => {
                let confirm = req
                    .params
                    .get("confirm")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !confirm {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "db_restore requires confirm=true",
                    );
                }

                let src_path = match req.params.get("src_path").and_then(|v| v.as_str()) {
                    Some(s) if !s.is_empty() => s.to_string(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "db_restore requires a non-empty src_path",
                        )
                    }
                };

                if !std::path::Path::new(&src_path).is_file() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("db_restore: backup file not found: {src_path}"),
                    );
                }

                let force = req
                    .params
                    .get("force")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let db_path = crate::paths::db_path();

                // Resolve the REAL device key for validating + opening the
                // restored DB (CopyPaste-crh3.6). The daemon's in-memory key is
                // a throwaway dummy when degraded (Keychain locked at startup),
                // which can NEVER open a backup encrypted with the real device
                // key. Mirror `reset_database`: load the key from the Keychain.
                // If the daemon is degraded AND the Keychain is still
                // unreachable, reject with ipc_not_ready and make NO filesystem
                // change (the only safe outcome).
                let restore_key: zeroize::Zeroizing<[u8; 32]> = {
                    #[cfg(target_os = "macos")]
                    {
                        match crate::keychain::load_or_create() {
                            Ok(kp) => kp.local_enc_key(),
                            Err(e) => {
                                if !self.ready.load(Ordering::Relaxed) {
                                    tracing::warn!(
                                        error = %e,
                                        "db_restore: refusing — daemon is degraded and the \
                                         Keychain key is unreachable; no filesystem change made"
                                    );
                                    return Response::err_with_code(
                                        req.id,
                                        ERR_CODE_IPC_NOT_READY,
                                        ERR_IPC_NOT_READY,
                                    );
                                }
                                tracing::warn!(
                                    error = %e,
                                    "db_restore: Keychain unavailable; validating with the \
                                     daemon's current in-memory key"
                                );
                                zeroize::Zeroizing::new(**self.local_key)
                            }
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        zeroize::Zeroizing::new(**self.local_key)
                    }
                };

                tracing::warn!(
                    src_path = %src_path,
                    db_path = %db_path.display(),
                    force,
                    "db_restore: validating backup before any destructive change"
                );

                let db_arc = self.db.clone();
                let src_for_task = src_path.clone();
                let db_path_for_task = db_path.clone();
                // Clone the key for the blocking task; the outer `restore_key`
                // is reused after the join to rebuild the read pool (crh3.2).
                let key_for_task: zeroize::Zeroizing<[u8; 32]> =
                    zeroize::Zeroizing::new(*restore_key);
                let join = tokio::task::spawn_blocking(move || -> Result<(), String> {
                    // Hold the write lock across validate+swap so no concurrent
                    // write lands on the about-to-be-replaced handle. The old
                    // handle is kept live until the swap succeeds: on a
                    // rolled-back failure it remains valid (its inode is moved
                    // aside and back, never replaced), so `guard` is always a
                    // usable DB.
                    let mut guard = db_arc.blocking_lock();
                    let restored = restore_database_file(
                        std::path::Path::new(&src_for_task),
                        &db_path_for_task,
                        &key_for_task,
                        force,
                    )?;
                    *guard = restored;
                    Ok(())
                })
                .await;

                match join {
                    Ok(Ok(())) => {
                        // Mark daemon as ready (in case it was degraded before).
                        self.ready.store(true, Ordering::Relaxed);
                        *self
                            .degraded_reason
                            .lock()
                            .unwrap_or_else(|p| p.into_inner()) = None;
                        // CopyPaste-crh3.2: the r2d2 read pool still holds file
                        // descriptors to the OLD database inode. Rebuild it
                        // against the restored file so reads stop serving
                        // pre-restore data. On failure, drop to None — reads then
                        // fall back to the write handle, which IS the restored DB.
                        let rebuilt = copypaste_core::open_pool(&db_path, &restore_key, 4)
                            .ok()
                            .map(Arc::new);
                        *self.read_pool.lock().unwrap_or_else(|p| p.into_inner()) = rebuilt;
                        tracing::warn!(
                            src_path = %src_path,
                            db_path = %db_path.display(),
                            "db_restore: COMPLETE — restored database is live"
                        );
                        Response::ok(req.id, serde_json::json!({ "ok": true, "ready": true }))
                    }
                    Ok(Err(msg)) => {
                        tracing::error!(
                            error = %msg,
                            "db_restore: FAILED — prior database preserved"
                        );
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("db_restore blocking task failed: {e}"),
                    ),
                }
            }

            _ => self.dispatch_peers(req).await,
        }
    }
}
