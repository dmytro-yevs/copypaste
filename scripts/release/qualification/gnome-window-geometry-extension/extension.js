import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const BUS_NAME = 'app.copypaste.QualificationGeometry';
const PATH = '/app/copypaste/QualificationGeometry';
const INTERFACE = 'app.copypaste.QualificationGeometry';
const XML = `<node><interface name="${INTERFACE}">
<method name="Version"><arg name="version" type="u" direction="out"/></method>
<method name="Lookup"><arg name="pid" type="u" direction="in"/><arg name="title" type="s" direction="in"/>
<arg name="frameX" type="i" direction="out"/><arg name="frameY" type="i" direction="out"/><arg name="frameWidth" type="i" direction="out"/><arg name="frameHeight" type="i" direction="out"/>
<arg name="bufferX" type="i" direction="out"/><arg name="bufferY" type="i" direction="out"/><arg name="bufferWidth" type="i" direction="out"/><arg name="bufferHeight" type="i" direction="out"/><arg name="stacking" type="i" direction="out"/></method>
<method name="PointBelongsTo"><arg name="pid" type="u" direction="in"/><arg name="title" type="s" direction="in"/><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/><arg name="owned" type="b" direction="out"/></method>
</interface></node>`;

function rectValues(rect) {
    const values = [rect?.x, rect?.y, rect?.width, rect?.height];
    return values.every(Number.isInteger) && values[2] > 0 && values[3] > 0 ? values : null;
}

export default class QualificationWindowGeometryExtension extends Extension {
    enable() {
        // The private headless Mutter seat has no libinput keyboard; keep one
        // virtual keyboard alive so GTK Wayland clients receive seat keyboard
        // capability during the qualified desktop session.
        const seat = Clutter.get_default_backend().get_default_seat();
        this._virtualKeyboard = seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
        this._desktopReady = false;
        this._startupCompleteId = 0;
        this._overviewHiddenId = 0;
        if (Main.layoutManager._startingUp) {
            this._startupCompleteId = Main.layoutManager.connect('startup-complete', () => this._prepareDesktop());
        } else {
            this._prepareDesktop();
        }
        this._connection = Gio.bus_get_sync(Gio.BusType.SESSION, null);
        this._impl = Gio.DBusExportedObject.wrapJSObject(XML, {
            VersionAsync: (_params, invocation) => {
                if (!this._desktopReady)
                    return invocation.return_dbus_error(`${INTERFACE}.Error.NotReady`, 'GNOME desktop is not ready');
                invocation.return_value(new GLib.Variant('(u)', [1]));
            },
            LookupAsync: (params, invocation) => this._lookup(params, invocation),
            PointBelongsToAsync: (params, invocation) => this._pointBelongsTo(params, invocation),
        });
        this._impl.export(this._connection, PATH);
        this._owner = Gio.bus_own_name_on_connection(this._connection, BUS_NAME, Gio.BusNameOwnerFlags.NONE, null, null);
    }

    disable() {
        if (this._startupCompleteId)
            Main.layoutManager.disconnect(this._startupCompleteId);
        this._startupCompleteId = 0;
        if (this._overviewHiddenId)
            Main.overview.disconnect(this._overviewHiddenId);
        this._overviewHiddenId = 0;
        this._desktopReady = false;
        this._virtualKeyboard?.run_dispose();
        this._virtualKeyboard = null;
        if (this._owner)
            Gio.bus_unown_name(this._owner);
        this._owner = 0;
        this._impl?.unexport();
        this._impl = null;
        this._connection = null;
    }

    _prepareDesktop() {
        if (this._startupCompleteId)
            Main.layoutManager.disconnect(this._startupCompleteId);
        this._startupCompleteId = 0;
        if (this._desktopReady || this._overviewHiddenId)
            return;
        if (Main.overview.visible || Main.overview.visibleTarget) {
            this._overviewHiddenId = Main.overview.connect('hidden', () => {
                Main.overview.disconnect(this._overviewHiddenId);
                this._overviewHiddenId = 0;
                this._desktopReady = true;
            });
            Main.overview.hide();
        } else {
            this._desktopReady = true;
        }
    }

    _lookup(params, invocation) {
        const [pid, title] = params;
        if (!Number.isInteger(pid) || pid <= 0 || typeof title !== 'string' || !title)
            return invocation.return_dbus_error(`${INTERFACE}.Error.Invalid`, 'invalid window identity');
        const matches = global.get_window_actors()
            .map(actor => actor.meta_window)
            .filter(window => window && window.get_pid?.() === pid && window.get_title?.() === title);
        if (matches.length !== 1)
            return invocation.return_dbus_error(`${INTERFACE}.Error.Ambiguous`, 'window identity is not unique');
        try {
            const frame = rectValues(matches[0].get_frame_rect());
            const buffer = rectValues(matches[0].get_buffer_rect());
            if (!frame || !buffer)
                throw new Error('invalid geometry');
            const stacking = this._stacking().indexOf(matches[0]);
            if (stacking < 0)
                throw new Error('missing stacking');
            invocation.return_value(new GLib.Variant('(iiiiiiiii)', [...frame, ...buffer, stacking]));
        } catch (_error) {
            invocation.return_dbus_error(`${INTERFACE}.Error.Unavailable`, 'window geometry is unavailable');
        }
    }

    _pointBelongsTo(params, invocation) {
        const [pid, title, x, y] = params;
        const windows = this._stacking();
        const index = windows.findIndex(window => window.get_pid?.() === pid && window.get_title?.() === title);
        if (index < 0 || windows.filter(window => window.get_pid?.() === pid && window.get_title?.() === title).length !== 1)
            return invocation.return_dbus_error(`${INTERFACE}.Error.Ambiguous`, 'window identity is not unique');
        const target = rectValues(windows[index].get_buffer_rect());
        const contains = rect => rect && x >= rect[0] && y >= rect[1] && x < rect[0] + rect[2] && y < rect[1] + rect[3];
        invocation.return_value(new GLib.Variant('(b)', [contains(target) && !windows.slice(index + 1).some(window => contains(rectValues(window.get_buffer_rect())))]));
    }

    _stacking() {
        return global.display.sort_windows_by_stacking(
            global.get_window_actors().map(actor => actor.meta_window).filter(window => window?.showing_on_its_workspace?.())
        );
    }
}
