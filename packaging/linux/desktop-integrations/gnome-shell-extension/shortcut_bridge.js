import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const HOST_BUS_NAME = 'app.copypaste.CopyPaste';
const PATH = '/app/copypaste/GnomeShortcuts';
const INTERFACE = 'app.copypaste.GnomeShortcuts';
const MAX_ID_BYTES = 128;
const MAX_ACCELERATOR_BYTES = 256;
const UNLOCKED_ACTION_MODES = Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW;
const XML = `<node><interface name="${INTERFACE}">
<method name="RegisterShortcut"><arg name="id" type="s" direction="in"/><arg name="accelerator" type="s" direction="in"/><arg name="registered" type="b" direction="out"/><arg name="triggerDescription" type="s" direction="out"/></method>
<method name="UnregisterShortcut"><arg name="id" type="s" direction="in"/><arg name="removed" type="b" direction="out"/></method>
<method name="Version"><arg name="version" type="u" direction="out"/></method>
<signal name="Activated"><arg name="id" type="s"/></signal>
</interface></node>`;

export class ShortcutBridge {
    constructor(connection) {
        this._connection = connection;
        this._bindings = new Map();
        this._destroyed = false;
        this._generation = 1;
        this._replied = new WeakSet();
        this._impl = Gio.DBusExportedObject.wrapJSObject(XML, {
            RegisterShortcutAsync: (params, invocation) => this._registerShortcutAsync(params, invocation),
            UnregisterShortcutAsync: (params, invocation) => this._unregisterShortcutAsync(params, invocation),
            VersionAsync: (_params, invocation) => this._reply(invocation, new GLib.Variant('(u)', [1])),
        });
        this._impl.export(connection, PATH);
        this._acceleratorActivatedId = global.display.connect('accelerator-activated', (_display, action) => {
            this._activated(action);
        });
        this._hostWatchId = Gio.bus_watch_name_on_connection(
            connection,
            HOST_BUS_NAME,
            Gio.BusNameWatcherFlags.NONE,
            null,
            () => {
                this._generation += 1;
                this._clearBindings();
            }
        );
    }

    destroy() {
        if (this._destroyed)
            return;
        this._destroyed = true;
        this._generation += 1;
        this._clearBindings();
        if (this._acceleratorActivatedId)
            global.display.disconnect(this._acceleratorActivatedId);
        if (this._hostWatchId)
            Gio.bus_unwatch_name(this._hostWatchId);
        this._acceleratorActivatedId = 0;
        this._hostWatchId = 0;
        this._impl.unexport();
    }

    async _registerShortcutAsync(params, invocation) {
        const generation = this._generation;
        if (!await this._authorized(invocation))
            return;
        if (!this._isCurrent(generation))
            return this._error(invocation, 'Unavailable');
        const [id, accelerator] = params;
        if (!this._valid(id, MAX_ID_BYTES) || !this._valid(accelerator, MAX_ACCELERATOR_BYTES))
            return this._reply(invocation, new GLib.Variant('(bs)', [false, '']));
        const previous = this._bindings.get(id);
        if (previous?.accelerator === accelerator)
            return this._reply(invocation, new GLib.Variant('(bs)', [true, previous.triggerDescription]));

        if (previous)
            this._removeBinding(id, previous);
        const binding = this._grab(id, accelerator);
        if (!binding) {
            if (previous)
                this._restoreBinding(id, previous);
            return this._reply(invocation, new GLib.Variant('(bs)', [false, '']));
        }
        this._bindings.set(id, binding);
        this._reply(invocation, new GLib.Variant('(bs)', [true, binding.triggerDescription]));
    }

    async _unregisterShortcutAsync(params, invocation) {
        const generation = this._generation;
        if (!await this._authorized(invocation))
            return;
        if (!this._isCurrent(generation))
            return this._error(invocation, 'Unavailable');
        const [id] = params;
        if (!this._valid(id, MAX_ID_BYTES))
            return this._reply(invocation, new GLib.Variant('(b)', [false]));
        const binding = this._bindings.get(id);
        if (!binding)
            return this._reply(invocation, new GLib.Variant('(b)', [false]));
        this._removeBinding(id, binding);
        this._reply(invocation, new GLib.Variant('(b)', [true]));
    }

    _grab(id, accelerator) {
        let action;
        let name;
        try {
            action = global.display.grab_accelerator(accelerator, Meta.KeyBindingFlags.NONE);
            if (!action)
                return null;
            name = Meta.external_binding_name_for_action(action);
            if (!name) {
                global.display.ungrab_accelerator(action);
                return null;
            }
            Main.wm.allowKeybinding(name, UNLOCKED_ACTION_MODES);
        } catch (_error) {
            if (action)
                global.display.ungrab_accelerator(action);
            return null;
        }
        return {action, name, accelerator, id, triggerDescription: accelerator};
    }

    _restoreBinding(id, binding) {
        const restored = this._grab(id, binding.accelerator);
        if (restored)
            this._bindings.set(id, restored);
    }

    _removeBinding(id, binding) {
        this._bindings.delete(id);
        try {
            Main.wm.allowKeybinding(binding.name, Shell.ActionMode.NONE);
        } catch (_error) {}
        try {
            global.display.ungrab_accelerator(binding.action);
        } catch (_error) {}
    }

    _clearBindings() {
        for (const [id, binding] of [...this._bindings])
            this._removeBinding(id, binding);
    }

    _activated(action) {
        if (this._destroyed)
            return;
        for (const binding of this._bindings.values()) {
            if (binding.action === action) {
                this._impl.emit_signal('Activated', new GLib.Variant('(s)', [binding.id]));
                return;
            }
        }
    }

    async _authorized(invocation) {
        if (this._destroyed) {
            this._error(invocation, 'Unavailable');
            return false;
        }
        try {
            const sender = invocation.get_sender();
            const owner = await this._busCall('GetNameOwner', new GLib.Variant('(s)', [HOST_BUS_NAME]), '(s)');
            const uid = await this._busCall('GetConnectionUnixUser', new GLib.Variant('(s)', [sender]), '(u)');
            const currentOwner = await this._busCall('GetNameOwner', new GLib.Variant('(s)', [HOST_BUS_NAME]), '(s)');
            if (this._destroyed)
                return this._error(invocation, 'Unavailable'), false;
            if (owner.deep_unpack()[0] !== sender || currentOwner.deep_unpack()[0] !== sender ||
                uid.deep_unpack()[0] !== new Gio.Credentials().get_unix_user())
                throw new Error('unauthorized');
            return true;
        } catch (_error) {
            this._error(invocation, this._destroyed ? 'Unavailable' : 'AccessDenied');
            return false;
        }
    }

    _busCall(method, parameters, replyType) {
        return new Promise((resolve, reject) => this._connection.call(
            'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', method,
            parameters, new GLib.VariantType(replyType), Gio.DBusCallFlags.NONE, 3000, null,
            (connection, result) => {
                try { resolve(connection.call_finish(result)); } catch (error) { reject(error); }
            }
        ));
    }

    _valid(value, maxBytes) {
        return typeof value === 'string' && value.length > 0 && new TextEncoder().encode(value).length <= maxBytes;
    }

    _isCurrent(generation) {
        return !this._destroyed && this._generation === generation;
    }

    _error(invocation, code) {
        if (this._replied.has(invocation))
            return;
        this._replied.add(invocation);
        invocation.return_dbus_error(`${INTERFACE}.Error.${code}`, code);
    }

    _reply(invocation, value) {
        if (this._replied.has(invocation))
            return;
        this._replied.add(invocation);
        invocation.return_value(value);
    }
}
