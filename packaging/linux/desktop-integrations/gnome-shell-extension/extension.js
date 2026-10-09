import Gio from 'gi://Gio';
import GIRepository from 'gi://GIRepository';
import GLib from 'gi://GLib';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {ClipboardBridge} from './clipboard_bridge.js';

const COMPANION_BUS_NAME = 'app.copypaste.GnomeIntegration';
const HOST_BUS_NAME = 'app.copypaste.CopyPaste';
const HOST_OBJECT_PATH = '/app/copypaste/WaylandIntegration';
const HOST_INTERFACE = 'app.copypaste.WaylandIntegration';
const QUICK_PASTE_APPLICATION_ID = 'com.copypaste.CopyPaste';
const QUICK_PASTE_ROLE = 'copypaste-quick-paste';
const QUICK_PASTE_TITLE = 'CopyPaste Quick Paste';
const AWAIT_TIMEOUT_MS = 605000;
const BEGIN_TIMEOUT_MS = 125000;
const PASTE_TIMEOUT_MS = 3000;

export default class CopyPasteQuickPasteExtension extends Extension {
    enable() {
        this._bus = Gio.bus_get_sync(Gio.BusType.SESSION, null);
        this._enabled = true;
        this._generation = 1;
        this._ownsBusName = false;
        this._hostAvailable = false;
        this._awaitingShortcut = false;
        this._awaitSourceId = 0;
        this._awaitRetrySeconds = 1;
        this._activeTransaction = null;
        this._calls = new Set();
        this._windowCreatedId = global.display.connect('window-created', (_display, window) => {
            if (this._activeTransaction && this._isQuickPasteWindow(window))
                this._activeTransaction.presentationWindow = window;
        });
        this._loadClipboardBridge();
        this._busOwnerId = Gio.bus_own_name_on_connection(
            this._bus,
            COMPANION_BUS_NAME,
            Gio.BusNameOwnerFlags.NONE,
            () => {
                this._ownsBusName = true;
                this._armAwait();
            },
            () => this._stopForLostCompanionName()
        );
        this._hostWatchId = Gio.bus_watch_name_on_connection(
            this._bus,
            HOST_BUS_NAME,
            Gio.BusNameWatcherFlags.NONE,
            () => this._hostAppeared(),
            () => this._hostVanished()
        );
        this._cancelSignalId = this._bus.signal_subscribe(
            HOST_BUS_NAME,
            HOST_INTERFACE,
            'TransactionCancelled',
            HOST_OBJECT_PATH,
            null,
            Gio.DBusSignalFlags.NONE,
            (_connection, _sender, _path, _interface, _signal, parameters) => {
                const [transactionId] = parameters.deep_unpack();
                if (this._activeTransaction?.id === transactionId) {
                    this._cancelTransaction(this._activeTransaction);
                }
            }
        );
    }

    disable() {
        this._enabled = false;
        this._generation += 1;
        this._ownsBusName = false;
        this._hostAvailable = false;
        this._cancelActiveTransaction(false);
        this._clearRetry();
        this._cancelCalls();
        this._clipboardBridge?.destroy();
        this._clipboardBridge = null;
        if (this._cancelSignalId)
            this._bus.signal_unsubscribe(this._cancelSignalId);
        if (this._windowCreatedId)
            global.display.disconnect(this._windowCreatedId);
        if (this._hostWatchId)
            Gio.bus_unwatch_name(this._hostWatchId);
        if (this._busOwnerId)
            Gio.bus_unown_name(this._busOwnerId);
        this._cancelSignalId = 0;
        this._windowCreatedId = 0;
        this._hostWatchId = 0;
        this._busOwnerId = 0;
        this._awaitingShortcut = false;
        this._activeTransaction = null;
        this._bus = null;
    }

    _hostAppeared() {
        if (!this._enabled)
            return;
        this._hostAvailable = true;
        this._awaitRetrySeconds = 1;
        this._armAwait();
    }

    async _loadClipboardBridge() {
        try {
            GIRepository.Repository.prepend_search_path(`${this.path}/native/typelib`);
            GIRepository.Repository.prepend_library_path(`${this.path}/native/lib`);
            const factory = await import('gi://CopyPasteClipboard?version=1.0');
            if (this._enabled)
                this._clipboardBridge = new ClipboardBridge(this._bus, factory);
        } catch (_error) {
            // Clipboard capture stays unavailable until the packaged native shim loads.
        }
    }

    _hostVanished() {
        if (!this._enabled)
            return;
        this._generation += 1;
        this._hostAvailable = false;
        this._awaitingShortcut = false;
        this._activeTransaction = null;
        this._clearRetry();
        this._cancelCalls();
    }

    _stopForLostCompanionName() {
        if (!this._enabled)
            return;
        this._generation += 1;
        this._ownsBusName = false;
        this._awaitingShortcut = false;
        this._activeTransaction = null;
        this._clearRetry();
        this._cancelCalls();
    }

    _beginQuickPaste() {
        if (!this._canCallHost() || this._activeTransaction) {
            this._armAwait();
            return;
        }
        const focusedWindow = global.display.focus_window;
        if (!focusedWindow) {
            this._armAwait();
            return;
        }
        const pending = {
            id: GLib.uuid_string_random(),
            window: focusedWindow,
            generation: this._generation,
        };
        this._activeTransaction = pending;
        this._callHost('BeginQuickPaste', pending.id, '(b)', BEGIN_TIMEOUT_MS, pending.generation, result => {
            if (!this._isActive(pending))
                return;
            this._capturePresentationWindow(pending);
            if (!result) {
                this._cancelTransaction(pending);
                return;
            }
            this._restoreThenPaste(pending);
        }, () => this._cancelTransaction(pending));
    }

    _restoreThenPaste(pending) {
        try {
            pending.window.activate(global.get_current_time());
        } catch (_error) {
            this._cancelTransaction(pending);
            return;
        }
        GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
            if (!this._isActive(pending))
                return GLib.SOURCE_REMOVE;
            if (global.display.focus_window !== pending.window) {
                this._cancelTransaction(pending);
                return GLib.SOURCE_REMOVE;
            }
            this._callHost('PasteIntoRestoredWindow', pending.id, '(b)', PASTE_TIMEOUT_MS, pending.generation, () => {
                this._finishTransaction(pending);
            }, () => this._cancelTransaction(pending));
            return GLib.SOURCE_REMOVE;
        });
    }

    _cancelActiveTransaction(rearm = true) {
        if (this._activeTransaction)
            this._cancelTransaction(this._activeTransaction, rearm);
    }

    _cancelTransaction(pending, rearm = true) {
        if (this._activeTransaction === pending)
            this._activeTransaction = null;
        this._restoreCopyOnlyFocus(pending);
        if (this._bus && this._hostAvailable) {
            this._bus.call(
                HOST_BUS_NAME,
                HOST_OBJECT_PATH,
                HOST_INTERFACE,
                'CancelQuickPaste',
                new GLib.Variant('(s)', [pending.id]),
                null,
                Gio.DBusCallFlags.NONE,
                -1,
                null,
                (connection, result) => {
                    try { connection.call_finish(result); } catch (_error) {}
                }
            );
        }
        if (rearm)
            this._armAwait();
    }

    _finishTransaction(pending) {
        if (this._activeTransaction !== pending)
            return;
        this._activeTransaction = null;
        this._armAwait();
    }

    _capturePresentationWindow(pending) {
        const focusedWindow = global.display.focus_window;
        if (this._isQuickPasteWindow(focusedWindow))
            pending.presentationWindow = focusedWindow;
    }

    _isQuickPasteWindow(window) {
        return window &&
            window.get_gtk_application_id?.() === QUICK_PASTE_APPLICATION_ID &&
            (window.get_role?.() === QUICK_PASTE_ROLE ||
                window.get_title?.() === QUICK_PASTE_TITLE);
    }

    _restoreCopyOnlyFocus(pending) {
        if (!pending.presentationWindow || global.display.focus_window !== pending.presentationWindow)
            return;
        try {
            pending.window.activate(global.get_current_time());
        } catch (_error) {}
    }

    _armAwait(delaySeconds = 0) {
        if (!this._canCallHost() || this._awaitingShortcut || this._activeTransaction || this._awaitSourceId)
            return;
        if (delaySeconds === 0) {
            this._awaitQuickPaste();
            return;
        }
        const generation = this._generation;
        this._awaitSourceId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, delaySeconds, () => {
            this._awaitSourceId = 0;
            if (this._isCurrent(generation))
                this._armAwait();
            return GLib.SOURCE_REMOVE;
        });
    }

    _awaitQuickPaste() {
        if (!this._canCallHost() || this._awaitingShortcut || this._activeTransaction)
            return;
        const generation = this._generation;
        this._awaitingShortcut = true;
        this._call(
            'AwaitQuickPaste',
            new GLib.Variant('()', []),
            '(b)',
            AWAIT_TIMEOUT_MS,
            generation,
            result => {
                this._awaitingShortcut = false;
                this._awaitRetrySeconds = 1;
                const [triggered] = result.deep_unpack();
                if (triggered)
                    this._beginQuickPaste();
                else
                    this._armAwait(1);
            },
            () => {
                this._awaitingShortcut = false;
                if (this._canCallHost())
                    this._scheduleAwaitRetry();
            }
        );
    }

    _callHost(method, transactionId, replyType, timeout, generation, onSuccess, onFailure) {
        this._call(
            method,
            new GLib.Variant('(s)', [transactionId]),
            replyType,
            timeout,
            generation,
            result => onSuccess(result.deep_unpack()[0]),
            onFailure
        );
    }

    _scheduleAwaitRetry() {
        const delaySeconds = this._awaitRetrySeconds;
        this._awaitRetrySeconds = Math.min(delaySeconds * 2, 60);
        this._armAwait(delaySeconds);
    }

    _call(method, parameters, replyType, timeout, generation, onSuccess, onFailure) {
        const cancellable = new Gio.Cancellable();
        this._calls.add(cancellable);
        this._bus.call(
            HOST_BUS_NAME,
            HOST_OBJECT_PATH,
            HOST_INTERFACE,
            method,
            parameters,
            new GLib.VariantType(replyType),
            Gio.DBusCallFlags.NONE,
            timeout,
            cancellable,
            (connection, result) => {
                this._calls.delete(cancellable);
                if (!this._isCurrent(generation))
                    return;
                try {
                    onSuccess(connection.call_finish(result));
                } catch (_error) {
                    onFailure();
                }
            }
        );
    }

    _canCallHost() {
        return this._enabled && this._ownsBusName && this._hostAvailable && this._bus;
    }

    _isCurrent(generation) {
        return this._enabled && this._generation === generation && this._bus;
    }

    _isActive(pending) {
        return this._isCurrent(pending.generation) && this._activeTransaction === pending;
    }

    _clearRetry() {
        if (this._awaitSourceId)
            GLib.Source.remove(this._awaitSourceId);
        this._awaitSourceId = 0;
    }

    _cancelCalls() {
        for (const cancellable of this._calls)
            cancellable.cancel();
        this._calls.clear();
    }
}
