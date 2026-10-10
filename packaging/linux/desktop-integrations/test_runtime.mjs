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
    const ShortcutBridge = class { destroy() {} };
    const ExtensionClass = new Function('Gio', 'GLib', 'Extension', 'ClipboardBridge', 'ShortcutBridge', 'global', `${source}; return CopyPasteQuickPasteExtension;`)(
        Gio,
        GLib,
        Extension,
        ClipboardBridge,
        ShortcutBridge,
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

function loadShortcutBridge() {
    const source = fs.readFileSync(new URL('gnome-shell-extension/shortcut_bridge.js', root), 'utf8')
        .replace(/^import .*;\n/gm, '')
        .replace('export class ', 'class ');
    const state = {active: new Map(), allowed: [], calls: [], owner: ':1.host', pendingDbus: [], signals: [], uid: 1000, ungrabs: 0, unexports: 0, watches: 0};
    const bus = {
        call(...args) {
            const method = args[3];
            state.calls.push(method);
            const reply = method === 'GetNameOwner' ? [state.owner] : [state.uid];
            const respond = () => args[9](bus, {value: {deep_unpack: () => reply}});
            if (state.deferNextDbus) {
                state.deferNextDbus = false;
                state.pendingDbus.push(respond);
            } else {
                respond();
            }
        },
        call_finish(result) {
            return result.value;
        },
    };
    class Variant {
        constructor(_type, value) { this.value = value; }
        deep_unpack() { return this.value; }
    }
    const display = {
        connect(signal, callback) {
            assert.equal(signal, 'accelerator-activated');
            state.activated = callback;
            return 31;
        },
        disconnect(id) { state.disconnected = id; },
        grab_accelerator(accelerator) {
            if (state.active.has(accelerator))
                return 0;
            const action = state.active.size + 1;
            state.active.set(accelerator, action);
            return action;
        },
        ungrab_accelerator(action) {
            for (const [accelerator, candidate] of state.active) {
                if (candidate === action)
                    state.active.delete(accelerator);
            }
            state.ungrabs += 1;
        },
    };
    const Gio = {
        BusNameWatcherFlags: {NONE: 0},
        DBusCallFlags: {NONE: 0},
        Credentials: class { get_unix_user() { return 1000; } },
        VariantType: class { constructor(type) { this.type = type; } },
        DBusExportedObject: {
            wrapJSObject(_xml, handlers) {
                state.handlers = handlers;
                return {
                    export(_connection, path) { state.path = path; },
                    unexport() { state.unexports += 1; },
                    emit_signal(name, value) { state.signals.push({name, value: value.deep_unpack()}); },
                };
            },
        },
        bus_watch_name_on_connection(_bus, _name, _flags, appeared, vanished) {
            state.hostAppeared = appeared;
            state.hostVanished = vanished;
            return 32;
        },
        bus_unwatch_name(id) { state.unwatched = id; },
    };
    const GLib = {Variant, VariantType: class { constructor(type) { this.type = type; } }};
    const Meta = {
        KeyBindingFlags: {NONE: 0},
        external_binding_name_for_action: action => `external-${action}`,
    };
    const Shell = {ActionMode: {NONE: 0, NORMAL: 1, OVERVIEW: 2}};
    const Main = {wm: {allowKeybinding(name, modes) {
        state.allowed.push({name, modes});
        if (state.throwAllowKeybinding)
            throw new Error('Shell rejected keybinding mode');
    }}};
    const ShortcutBridge = new Function('Gio', 'GLib', 'Meta', 'Shell', 'Main', 'global', `${source}; return ShortcutBridge;`)(
        Gio, GLib, Meta, Shell, Main, {display}
    );
    return {bus, state, bridge: new ShortcutBridge(bus)};
}

function loadClipboardBridge({nativeAvailable = true, exposeAvailability = true} = {}) {
    const source = fs.readFileSync(new URL('gnome-shell-extension/clipboard_bridge.js', root), 'utf8')
        .replace(/^import .*;\n/gm, '')
        .replace('export class ', 'class ');
    const state = {
        calls: [],
        daemonOwner: ':1.daemon',
        guiOwner: ':1.gui',
        inventoryCalls: 0,
    };
    class Variant {
        constructor(_type, value) { this.value = value; }
        deep_unpack() { return this.value; }
    }
    const selection = {
        connect() { return 1; },
        disconnect() {},
        get_mimetypes() {
            state.inventoryCalls += 1;
            return ['text/plain'];
        },
    };
    const connection = {
        call(...args) {
            const method = args[3];
            const [name] = args[4].deep_unpack();
            state.calls.push({method, name});
            let reply;
            if (method === 'GetNameOwner') {
                reply = [name === 'app.copypaste.Daemon' ? state.daemonOwner :
                    name === 'app.copypaste.CopyPaste' ? state.guiOwner : ''];
            } else if (method === 'GetConnectionUnixUser') {
                reply = [1000];
            } else {
                throw new Error(`unexpected D-Bus call: ${method}`);
            }
            args[9](connection, {value: new Variant('', reply)});
        },
        call_finish(result) { return result.value; },
    };
    const Gio = {
        DBusCallFlags: {NONE: 0},
        Credentials: class { get_unix_user() { return 1000; } },
        VariantType: class { constructor(type) { this.type = type; } },
        DBusExportedObject: {
            wrapJSObject(_xml, handlers) {
                state.handlers = handlers;
                return {export() {}, unexport() {}, emit_signal() {}};
            },
        },
    };
    const GLib = {Variant, VariantType: class { constructor(type) { this.type = type; } }};
    const Meta = {SelectionType: {CLIPBOARD: 1}};
    const factory = {};
    if (exposeAvailability)
        factory.clipboard_source_is_available = receivedSelection => receivedSelection === selection && nativeAvailable;
    const ClipboardBridge = new Function('Gio', 'GLib', 'Meta', 'global', `${source}; return ClipboardBridge;`)(
        Gio, GLib, Meta, {display: {get_selection: () => selection}}
    );
    return {bridge: new ClipboardBridge(connection, factory), state};
}

async function invokeClipboard(runtime, method = 'VersionAsync', params = [], sender = ':1.daemon') {
    let reply;
    const invocation = {
        get_sender: () => sender,
        return_value: value => { reply = {value: value.deep_unpack()}; },
        return_dbus_error: (_name, code) => { reply = {error: code}; },
    };
    await runtime.state.handlers[method](params, invocation);
    return reply;
}

async function invokeShortcut(runtime, method, params, sender = ':1.host') {
    let reply;
    const invocation = {
        get_sender: () => sender,
        return_value: value => { reply = {value: value.deep_unpack()}; },
        return_dbus_error: (_name, code) => { reply = {error: code}; },
    };
    await runtime.state.handlers[method](params, invocation);
    return reply;
}

{
    const runtime = loadGnome();
    runtime.extension.enable();
    runtime.extension._ownCompanionName();
    runtime.callbacks.ownAppeared();
    assert.equal(runtime.calls.length, 0, 'an absent host must not receive an await call');
    runtime.callbacks.hostAppeared();
    assert.equal(runtime.calls.at(-1).method, 'AwaitQuickPaste');
    assert.equal(runtime.calls.at(-1).timeout, 605000);
}

{
    const runtime = loadGnome();
    runtime.extension.enable();
    runtime.extension._ownCompanionName();
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
    runtime.extension._ownCompanionName();
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
    runtime.extension._ownCompanionName();
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
    runtime.extension._ownCompanionName();
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

{
    const runtime = loadShortcutBridge();
    const version = await invokeShortcut(runtime, 'VersionAsync', []);
    assert.deepEqual(version.value, [1], 'the public shortcut bridge reports its wire version');
    assert.deepEqual(runtime.state.calls, [], 'Version does not require host ownership');
    const unauthorized = await invokeShortcut(runtime, 'RegisterShortcutAsync', ['quick-paste', '<Super>V'], ':1.other');
    assert.equal(unauthorized.error, 'AccessDenied', 'only the current host name owner may register shortcuts');
    assert.equal(runtime.state.active.size, 0, 'rejected callers cannot reserve accelerators');
    const registered = await invokeShortcut(runtime, 'RegisterShortcutAsync', ['quick-paste', '<Super>V']);
    assert.deepEqual(registered.value, [true, '<Super>V'], 'a successful Mutter grab reports the exact accepted accelerator');
    assert.deepEqual(runtime.state.allowed, [{name: 'external-1', modes: 3}], 'registered shortcuts are enabled only in unlocked Shell modes');
    const collision = await invokeShortcut(runtime, 'RegisterShortcutAsync', ['other', '<Super>V']);
    assert.deepEqual(collision.value, [false, ''], 'a reserved accelerator collision must not claim registration');
    runtime.state.activated(null, 1);
    assert.deepEqual(runtime.state.signals, [{name: 'Activated', value: ['quick-paste']}], 'accelerator activation only emits the host-facing signal');
    runtime.state.hostVanished();
    assert.equal(runtime.state.active.size, 0, 'host name loss clears every Mutter grab');
    assert.equal(runtime.state.ungrabs, 1, 'host name loss ungrabs the active shortcut exactly once');
    const second = await invokeShortcut(runtime, 'RegisterShortcutAsync', ['quick-paste', '<Super>C']);
    assert.deepEqual(second.value, [true, '<Super>C']);
    runtime.bridge.destroy();
    assert.equal(runtime.state.active.size, 0, 'destroy clears shortcut grabs');
    assert.equal(runtime.state.unwatched, 32, 'destroy removes the host owner watcher');
    assert.equal(runtime.state.unexports, 1, 'destroy unexports the D-Bus object');
    runtime.state.activated(null, 1);
    assert.equal(runtime.state.signals.length, 1, 'destroyed bridges never emit late activations');
}

{
    const runtime = loadClipboardBridge({nativeAvailable: false});
    const reply = await invokeClipboard(runtime);
    assert.equal(reply.error, 'Unavailable', 'Version rejects a loaded native shim without the patched Mutter ABI');
    assert.equal(runtime.state.inventoryCalls, 0, 'Version checks native availability without reading the clipboard snapshot');
}

{
    const runtime = loadClipboardBridge({exposeAvailability: false});
    const reply = await invokeClipboard(runtime);
    assert.equal(reply.error, 'Unavailable', 'Version rejects a shim that does not export native availability');
}

{
    const runtime = loadClipboardBridge();
    assert.deepEqual((await invokeClipboard(runtime)).value, [2], 'the daemon owner may complete the native v2 handshake');
    assert.deepEqual((await invokeClipboard(runtime, 'VersionAsync', [], ':1.gui')).value, [2], 'the GUI owner may complete the native v2 handshake');
    assert.equal((await invokeClipboard(runtime, 'VersionAsync', [], ':1.other')).error, 'AccessDenied', 'foreign session peers cannot probe the clipboard bridge');
    assert.equal((await invokeClipboard(runtime, 'SnapshotAsync', [], ':1.gui')).error, 'AccessDenied', 'the GUI owner cannot access clipboard payload metadata');
}

{
    const runtime = loadShortcutBridge();
    runtime.state.deferNextDbus = true;
    const pending = invokeShortcut(runtime, 'RegisterShortcutAsync', ['quick-paste', '<Super>V']);
    runtime.state.hostVanished();
    runtime.state.pendingDbus.shift()();
    const result = await pending;
    assert.equal(result.error, 'Unavailable', 'an in-flight registration cannot re-grab after host owner loss');
    assert.equal(runtime.state.active.size, 0, 'owner loss keeps accelerators released after delayed authorization');
}

{
    const runtime = loadShortcutBridge();
    runtime.state.deferNextDbus = true;
    const pending = invokeShortcut(runtime, 'RegisterShortcutAsync', ['quick-paste', '<Super>V']);
    runtime.bridge.destroy();
    runtime.state.pendingDbus.shift()();
    const result = await pending;
    assert.equal(result.error, 'Unavailable', 'destroyed bridges reject authorization that completes late');
    assert.equal(runtime.state.active.size, 0, 'destroyed bridges never restore delayed grabs');
}

{
    const runtime = loadShortcutBridge();
    runtime.state.throwAllowKeybinding = true;
    const result = await invokeShortcut(runtime, 'RegisterShortcutAsync', ['quick-paste', '<Super>V']);
    assert.deepEqual(result.value, [false, ''], 'a failed Shell permission step does not report a registered shortcut');
    assert.equal(runtime.state.active.size, 0, 'a failed Shell permission step releases the accelerator');
    assert.equal(runtime.state.ungrabs, 1, 'accelerator release runs even when allowKeybinding throws');
}

{
    const runtime = loadShortcutBridge();
    await invokeShortcut(runtime, 'RegisterShortcutAsync', ['quick-paste', '<Super>V']);
    runtime.state.throwAllowKeybinding = true;
    const removed = await invokeShortcut(runtime, 'UnregisterShortcutAsync', ['quick-paste']);
    assert.deepEqual(removed.value, [true], 'unregistration still succeeds when disabling Shell handling throws');
    assert.equal(runtime.state.active.size, 0, 'unregistration always releases the accelerator after allowKeybinding throws');
    assert.equal(runtime.state.ungrabs, 1, 'unregistration attempts Mutter ungrab independently of Shell mode cleanup');
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
    const deadline = runtime.timers.find(timer => timer.singleShot === true);
    assert.ok(deadline, 'a Begin transaction must have one deadline');
    deadline._callback();
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
