# CopyPaste — Робочий статус

_Сесія v0.5.2-fixes · оновлено 2026-05-30_

## Реліз
- **Гілка:** `v0.5.2-fixes` — **HEAD `1ffd215`** (батч-2 хвилі 3 ще мерджиться)
- **Статус:** 🔴 ЗАБЛОКОВАНО — не мерджити в `main`, не тегати, поки: всі фікси готові → зелений CI → перевірка на РЕАЛЬНОМУ залізі (macOS+Android) → твій sign-off.
- **Гейт зараз:** clippy `--workspace -D warnings` PASS · `tsc --noEmit` PASS. (Локально; CI ще не тригернуто.)

## ✅ Змерджено в `v0.5.2-fixes`

**Хвиля 1 (8 фіксів, @4e43a85):**
- Android: стабільний підпис (нема конфлікту встановлення), sync (econnrefused / дубль×3 / тап-копія / список девайсів), bg-capture нотифікація, рендер картинок + налаштування, система crash-логів (adb-pullable).
- macOS: rich device info, авто-QR, a11y-банер, картинки в popup/history, popup auto-close + Esc-fix, P2P-увімкнення демона, кнопка Restart daemon.

**Хвиля 2 (5 фіксів, @76c2228):**
- APK −13.4MB (release-size profile + abiFilters) → ~28MB.
- Android image-CAPTURE (detect→downscale≤1024px→PNG≤2MB→store).
- Android Settings: секції General/Display/Storage-Limits/Sync + паритет-рядки.
- macOS Devices: один список (поточний девайс перший), fingerprint на всю ширину, QR 190px, «вже спаровано» банер, rich info.
- Desktop Settings: візуал (слайдери/описи/Open-popup), Storage/Limits секція, p2p/wifi паритет.

## 🔵 Хвиля 3/4/5 — В РОБОТІ (8 агентів, off 76c2228)
Android: image-capture wiring ✅, repo limit-enforcement+DB-cap+pin/clear/bulk, history UI pin/clear/bulk.
macOS: sync-status чип, history bulk, copy sound+notification. Daemon: supabase email/pass + cloud-config. Core: local-DB cap.

## 🔴 Спостерігати
- **A1** Android краш — користувач більше НЕ ловить (ймовірно пофікшено хвилями 1-2). Логи дістануть трейс, якщо повториться.

## 🟡 Функціонал / паритет (черга)
- **W5-1** Android обрізаний: нема pin, clear-all, delete/unpin/search — привести до macOS.
- **W5-2** Sync-індикатор (обидві): чи працює синхронізація + к-сть девайсів.
- **W5-3** Bulk-екшени (обидві): мульти-вибір → масове видалення/pin/копія.
- **W5-4** Android: повний паритет усіх налаштувань.
- **W4-6** Сповіщення + звук при копіюванні (Maccy-стиль, налаштовуване).
- **Android UI-restyle** — паритет теми (спек готовий).

## 🟢 Покращення / design
- **Local-DB cap** (роздування при Supabase) — design готовий, ЧЕКАЄ рішення про підхід.
- **X5** Єдиний cloud-config macOS↔Android.
- **X1 daemon-доповнення** — per-peer app_version/os/last_seen.
- **supabase_email/password на macOS** — треба daemon-зміна (нема в AppConfig).
- **APK −2.5MB** — material-icons-core (треба .kt swap 9 іконок).

## ⚪ Старіший беклог
Audit-backlog (MEDIUM/LOW + relay), hardcode-audit (shared-const), lamport design-debt, realtime anon_key, dual 16MiB frame const, cli vacuum keychain-const.

---
*Дзеркало памʼяті оркестратора. Повні деталі — у `~/.claude/.../memory/`.*
