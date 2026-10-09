import assert from 'node:assert/strict';
import fs from 'node:fs';

const root = new URL('.', import.meta.url);

function loadGnome({focusWindow = null} = {}) {
    const source = fs.readFileSync(new URL('gnome-shell-extension/extension.js', root), 'utf8')
        .replace(/^import .*;\n/gm, '')
        .replace('export default class ', 'class ');
    const callbacks = {};
    const calls = [];
    const timers = [];
    const bus = {
        call(...args) {
            calls.push({method: args[3], timeout: args[7], cancellable: args[8], callback: args[9]});
        },
        call_finish(result) {
            if (result.error)
                throw result.error;
            return result.value;
        },
        signal_subscribe() {
            return 3;
        },
        signal_unsubscribe() {},
    };
    class Variant {
        constructor(_type, value) {
            this.value = value;
        }

        deep_unpack() {
            return this.value;
        }
    }
    const Gio = {
        BusType: {SESSION: 1},
        BusNameOwnerFlags: {NONE: 0},
        BusNameWatcherFlags: {NONE: 0},
        DBusSignalFlags: {NONE: 0},
        DBusCallFlags: {NONE: 0, NO_REPLY_EXPECTED: 1},
        Cancellable: class {
            cancel() {
                this.cancelled = true;
            }
        },
        VariantType: class {
            constructor(type) {
                this.type = type;
            }
        },
        bus_get_sync: () => bus,
        bus_own_name_on_connection: (_bus, _name, _flags, appeared, vanished) => {
            callbacks.ownAppeared = appeared;
            callbacks.ownVanished = vanished;
            return 1;
        },
        bus_watch_name_on_connection: (_bus, _name, _flags, appeared, vanished) => {
            callbacks.hostAppeared = appeared;
            callbacks.hostVanished = vanished;
            return 2;
        },
        bus_unown_name() {},
        bus_unwatch_name() {},
    };
    const GLib = {
        Variant,
        VariantType: class {
            constructor(type) {
                this.type = type;
            }
        },
        PRIORITY_DEFAULT: 0,
        PRIORITY_DEFAULT_IDLE: 0,
        SOURCE_REMOVE: false,
        uuid_string_random: () => '00000000-0000-4000-8000-000000000000',
        idle_add: (_priority, callback) => timers.push(callback),
        timeout_add_seconds: (_priority, seconds, callback) => {
            timers.push(callback);
            return timers.length;
        },
        Source: {remove() {}},
    };
    const Extension = class {};
    const shell = {display: {focus_window: focusWindow, connect: () => 4, disconnect() {}}, get_current_time: () => 1};
    const ClipboardBridge = class { destroy() {} };
    const ExtensionClass = new Function('Gio', 'GLib', 'Extension', 'ClipboardBridge', 'global', `${source}; return CopyPasteQuickPasteExtension;`)(
        Gio,
        GLib,
        Extension,
        ClipboardBridge,
        shell
    );
    return {callbacks, calls, timers, shell, extension: new ExtensionClass()};
}

function completeGnome(bus, call, value) {
    call.callback(bus, {value: {deep_unpack: () => value}});
}

function failGnome(bus, call, message) {
    call.callback(bus, {error: new Error(message)});
}

{
    const runtime = loadGnome();
    runtime.extension.enable();
    runtime.callbacks.ownAppeared();
    assert.equal(runtime.calls.length, 0, 'an absent host must not receive an await call');
    runtime.callbacks.hostAppeared();
    assert.equal(runtime.calls.at(-1).method, 'AwaitQuickPaste');
    assert.equal(runtime.calls.at(-1).timeout, 605000);
}

{
    const runtime = loadGnome();
    runtime.extension.enable();
    runtime.callbacks.ownAppeared();
    runtime.callbacks.hostAppeared();
    const firstAwait = runtime.calls.at(-1);
    failGnome(runtime.extension._bus, firstAwait, 'G_DBUS_ERROR_NO_REPLY');
    assert.equal(runtime.timers.length, 1, 'an await timeout must schedule a bounded retry');
    runtime.timers.shift()();
    assert.equal(runtime.calls.at(-1).method, 'AwaitQuickPaste');
}

{
    const runtime = loadGnome();
    runtime.extension.enable();
    runtime.callbacks.ownAppeared();
    runtime.callbacks.hostAppeared();
    const staleAwait = runtime.calls.at(-1);
    runtime.callbacks.hostVanished();
    assert.equal(staleAwait.cancellable.cancelled, true, 'host loss must cancel its pending await');
    runtime.callbacks.hostAppeared();
    const restartedAwait = runtime.calls.at(-1);
    assert.notEqual(restartedAwait, staleAwait, 'host restart must create a fresh await');
    runtime.extension.disable();
    completeGnome(runtime.extension._bus ?? {}, restartedAwait, [true]);
    assert.equal(runtime.calls.filter(call => call.method === 'BeginQuickPaste').length, 0, 'a late callback after disable must do nothing');
}

{
    const source = {activate() {}};
    const runtime = loadGnome({focusWindow: source});
    source.activate = () => { runtime.shell.display.focus_window = {}; };
    runtime.extension.enable();
    runtime.callbacks.ownAppeared();
    runtime.callbacks.hostAppeared();
    completeGnome(runtime.extension._bus, runtime.calls.at(-1), [true]);
    completeGnome(runtime.extension._bus, runtime.calls.at(-1), [true]);
    runtime.timers.shift()();
    assert.equal(runtime.calls.some(call => call.method === 'PasteIntoRestoredWindow'), false, 'refused focus must never paste');
    assert.equal(runtime.calls.some(call => call.method === 'CancelQuickPaste'), true, 'refused focus must cancel');
}

{
    let restored = false;
    const source = {activate() { restored = true; }};
    const runtime = loadGnome({focusWindow: source});
    runtime.extension.enable();
    runtime.callbacks.ownAppeared();
    runtime.callbacks.hostAppeared();
    completeGnome(runtime.extension._bus, runtime.calls.at(-1), [true]);
    runtime.shell.display.focus_window = {
        get_gtk_application_id: () => 'com.copypaste.CopyPaste',
        get_role: () => null,
        get_title: () => 'CopyPaste Quick Paste',
    };
    completeGnome(runtime.extension._bus, runtime.calls.at(-1), [false]);
    assert.equal(restored, true, 'a copy-only GNOME cancellation restores the captured focus');
    assert.equal(runtime.calls.some(call => call.method === 'PasteIntoRestoredWindow'), false, 'a copy-only GNOME cancellation never pastes');
}

{
    const runtime = loadGnome();
    const roleAbsentWaylandWindow = {
        get_gtk_application_id: () => 'com.copypaste.CopyPaste',
        get_role: () => null,
        get_title: () => 'CopyPaste Quick Paste',
    };
    assert.equal(runtime.extension._isQuickPasteWindow(roleAbsentWaylandWindow), true, 'Wayland title fallback must identify the native transient');
    assert.equal(runtime.extension._isQuickPasteWindow({...roleAbsentWaylandWindow, get_gtk_application_id: () => 'other.app'}), false, 'title alone must not identify a foreign window');
    assert.equal(runtime.extension._isQuickPasteWindow({...roleAbsentWaylandWindow, get_title: () => 'Other'}), false, 'app ID alone must not identify the main window');
}

function loadKde({activateWindow} = {}) {
    const source = fs.readFileSync(new URL('kde-kwin-script/contents/code/main.js', root), 'utf8');
    const calls = [];
    const signals = {};
    const timers = [];
    const QTimer = class {
        constructor() { this.timeout = {connect: callback => { this._callback = callback; }}; timers.push(this); }
        start() { this.started = true; }
        stop() { this.stopped = true; }
    };
    const workspace = {
        activeWindow: null,
        activateWindow(window) {
            activateWindow?.(window, workspace, signals);
        },
        windowAdded: {connect: callback => { signals.added = callback; }},
        windowActivated: {connect: callback => { signals.activated = callback; }},
        windowRemoved: {connect: callback => { signals.removed = callback; }},
    };
    const callDBus = (_service, _path, _interface, method, ...args) => {
        const callback = args.pop();
        calls.push({method, args, callback});
    };
    new Function('callDBus', 'workspace', 'QTimer', source)(callDBus, workspace, QTimer);
    return {calls, signals, workspace, timers};
}

{
    const runtime = loadKde();
    const failedAwait = runtime.calls.at(-1);
    assert.equal(failedAwait.method, 'AwaitQuickPaste');
    runtime.signals.added({desktopFileName: 'com.copypaste.CopyPaste', deleted: false});
    assert.notEqual(runtime.calls.at(-1), failedAwait, 'a host window arrival must replace an await whose D-Bus error has no callback');
}

{
    const source = {deleted: false};
    const runtime = loadKde({activateWindow: (_window, workspace) => { workspace.activeWindow = {deleted: false}; }});
    runtime.workspace.activeWindow = source;
    runtime.calls.at(-1).callback(true);
    runtime.calls.at(-1).callback(true);
    assert.equal(runtime.calls.some(call => call.method === 'PasteIntoRestoredWindow'), false, 'KWin must not paste without restored focus');
    runtime.signals.removed(source);
    assert.equal(runtime.calls.some(call => call.method === 'CancelQuickPaste'), true, 'a destroyed restore target must cancel');
}

{
    const source = {deleted: false};
    const runtime = loadKde({activateWindow: () => {}});
    runtime.workspace.activeWindow = source;
    runtime.calls.at(-1).callback(true);
    runtime.calls.at(-1).callback(true);
    runtime.workspace.activeWindow = source;
    runtime.signals.activated(source);
    assert.equal(runtime.calls.some(call => call.method === 'PasteIntoRestoredWindow'), true, 'KWin must paste only after the restore activation event');
}

{
    const source = {deleted: false};
    const runtime = loadKde({activateWindow: () => {}});
    runtime.workspace.activeWindow = source;
    runtime.calls.at(-1).callback(true);
    assert.equal(runtime.timers.length, 1, 'a Begin transaction must have one deadline');
    runtime.timers[0]._callback();
    assert.equal(runtime.calls.some(call => call.method === 'CancelQuickPaste'), true, 'a missing Begin/Paste reply must cancel');
    assert.equal(runtime.calls.at(-1).method, 'AwaitQuickPaste', 'deadline recovery must rearm Await');
}

{
    const source = {deleted: false};
    let restored = false;
    const runtime = loadKde({activateWindow: window => { if (window === source) restored = true; }});
    runtime.workspace.activeWindow = source;
    runtime.calls.at(-1).callback(true);
    const quickPasteWindow = {desktopFileName: 'com.copypaste.CopyPaste', caption: 'CopyPaste Quick Paste', deleted: false};
    runtime.signals.added(quickPasteWindow);
    runtime.workspace.activeWindow = quickPasteWindow;
    runtime.calls.at(-1).callback(false);
    assert.equal(restored, true, 'a copy-only cancellation restores the captured focus');
    assert.equal(runtime.calls.some(call => call.method === 'PasteIntoRestoredWindow'), false, 'a copy-only cancellation never pastes');
}
