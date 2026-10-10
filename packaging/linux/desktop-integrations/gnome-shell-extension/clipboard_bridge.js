import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';

const DAEMON_BUS_NAME = 'app.copypaste.Daemon';
const PATH = '/app/copypaste/Clipboard';
const INTERFACE = 'app.copypaste.Clipboard';
const MAX_BYTES = 4 * 1024 * 1024;
const MAX_WRITE_TOTAL_BYTES = 32 * 1024 * 1024;
const MAX_MIMES = 64;
const MAX_MIME_BYTES = 255;
const PROTOCOL_VERSION = 2;
const XML = `<node><interface name="${INTERFACE}">
<method name="Version"><arg name="version" type="u" direction="out"/></method>
<method name="Snapshot"><arg name="sequence" type="t" direction="out"/><arg name="mimes" type="as" direction="out"/><arg name="identity" type="(suus)" direction="out"/></method>
<method name="Read"><arg name="sequence" type="t" direction="in"/><arg name="mime" type="s" direction="in"/><arg name="maxBytes" type="u" direction="in"/><arg name="bytes" type="ay" direction="out"/></method>
<method name="Write"><arg name="payloads" type="a{say}" direction="in"/><arg name="sequence" type="t" direction="out"/></method>
<signal name="OwnerChanged"><arg name="sequence" type="t"/><arg name="mimes" type="as"/><arg name="identity" type="(suus)"/></signal>
</interface></node>`;

export class ClipboardBridge {
    constructor(connection, sourceFactory) {
        this._connection = connection;
        this._selection = global.display.get_selection();
        this._sequence = 0;
        this._reads = new Set();
        this._source = null;
        this._sourceFactory = sourceFactory;
        this._destroyed = false;
        this._replied = new WeakSet();
        this._impl = Gio.DBusExportedObject.wrapJSObject(XML, {
            VersionAsync: (_params, invocation) => this._versionAsync(invocation),
            SnapshotAsync: (params, invocation) => this._snapshotAsync(params, invocation),
            ReadAsync: (params, invocation) => this._readAsync(params, invocation),
            WriteAsync: (params, invocation) => this._writeAsync(params, invocation),
        });
        this._impl.export(connection, PATH);
        this._identity = null;
        this._identity = this._writerIdentity(this._currentOwner());
        this._ownerChangedId = this._selection.connect('owner-changed', (_selection, selectionType, source) => {
            if (selectionType !== Meta.SelectionType.CLIPBOARD)
                return;
            this._sequence += 1;
            this._identity = this._writerIdentity(source);
            this._cancelReads();
            this._emitOwnerChanged();
        });
    }

    destroy() {
        this._destroyed = true;
        this._cancelReads();
        if (this._ownerChangedId)
            this._selection.disconnect(this._ownerChangedId);
        this._ownerChangedId = 0;
        this._impl.unexport();
    }

    async _versionAsync(invocation) {
        if (!await this._authorized(invocation))
            return;
        if (this._destroyed)
            return this._error(invocation, 'Unavailable');
        this._reply(invocation, new GLib.Variant('(u)', [PROTOCOL_VERSION]));
    }

    async _snapshotAsync(_params, invocation) {
        if (!await this._authorized(invocation))
            return;
        const mimes = this._inventory();
        if (!mimes)
            return this._error(invocation, 'Unavailable');
        this._reply(invocation, new GLib.Variant('(tas(suus))', [BigInt(this._sequence), mimes, this._identityTuple()]));
    }

    async _readAsync(params, invocation) {
        if (!await this._authorized(invocation))
            return;
        if (this._destroyed)
            return this._error(invocation, 'Unavailable');
        const [sequence, mime, maxBytes] = params;
        if (Number(sequence) !== this._sequence)
            return this._error(invocation, 'StaleSelection');
        const mimes = this._inventory();
        if (!mimes)
            return this._error(invocation, 'Unavailable');
        if (!this._validMime(mime) || !mimes.includes(mime))
            return this._error(invocation, 'UnsupportedMime');
        if (maxBytes < 1 || maxBytes > MAX_BYTES)
            return this._error(invocation, 'TooLarge');
        const cancellable = new Gio.Cancellable();
        this._reads.add(cancellable);
        const output = Gio.MemoryOutputStream.new_resizable();
        this._selection.transfer_async(Meta.SelectionType.CLIPBOARD, mime, maxBytes + 1, output, cancellable, (selection, result) => {
            this._reads.delete(cancellable);
            try {
                selection.transfer_finish(result);
                if (this._destroyed || Number(sequence) !== this._sequence)
                    return this._error(invocation, 'StaleSelection');
                output.close(null);
                const bytes = output.steal_as_bytes().toArray();
                if (bytes.length > maxBytes)
                    return this._error(invocation, 'TooLarge');
                this._reply(invocation, new GLib.Variant('(ay)', [bytes]));
            } catch (_error) {
                this._error(invocation, 'Unavailable');
            }
        });
    }

    async _writeAsync(params, invocation) {
        if (!await this._authorized(invocation))
            return;
        if (this._destroyed)
            return this._error(invocation, 'Unavailable');
        const [values] = params;
        const payloads = new Map(Object.entries(values));
        let total = 0;
        if (payloads.size < 1 || payloads.size > MAX_MIMES)
            return this._error(invocation, 'TooLarge');
        for (const [mime, bytes] of payloads) {
            if (!this._validMime(mime))
                return this._error(invocation, 'UnsupportedMime');
            if (bytes.length > MAX_BYTES)
                return this._error(invocation, 'TooLarge');
            total += bytes.length;
        }
        if (total > MAX_WRITE_TOTAL_BYTES)
            return this._error(invocation, 'TooLarge');
        try {
            this._source = this._sourceFactory.clipboard_source_new(new GLib.Variant('a{say}', values));
        } catch (_error) {
            return this._error(invocation, 'Unavailable');
        }
        this._selection.set_owner(Meta.SelectionType.CLIPBOARD, this._source);
        this._reply(invocation, new GLib.Variant('(t)', [BigInt(this._sequence)]));
    }

    async _authorized(invocation) {
        if (this._destroyed) {
            this._error(invocation, 'Unavailable');
            return false;
        }
        try {
            const sender = invocation.get_sender();
            const owner = await this._busCall('GetNameOwner', new GLib.Variant('(s)', [DAEMON_BUS_NAME]), '(s)');
            const uid = await this._busCall('GetConnectionUnixUser', new GLib.Variant('(s)', [sender]), '(u)');
            if (owner.deep_unpack()[0] !== sender || uid.deep_unpack()[0] !== new Gio.Credentials().get_unix_user())
                throw new Error('unauthorized');
            return true;
        } catch (_error) {
            this._error(invocation, 'AccessDenied');
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

    _inventory() {
        const values = this._selection.get_mimetypes(Meta.SelectionType.CLIPBOARD) ?? [];
        if (values.length > MAX_MIMES || values.some(mime => !this._validMime(mime)))
            return null;
        return [...values];
    }

    _writerIdentity(source) {
        if (!source || !this._sourceFactory?.clipboard_source_writer_identity)
            return null;
        try {
            const value = this._sourceFactory.clipboard_source_writer_identity(source);
            if (!value)
                return null;
            const [status, pid, uid, appId] = value.deep_unpack();
            if (typeof status !== 'string' || !status || !Number.isInteger(uid) ||
                !Number.isInteger(pid) || typeof appId !== 'string')
                return null;
            return [status, pid, uid, appId];
        } catch (_error) {
            return null;
        }
    }

    _currentOwner() {
        if (!this._sourceFactory?.clipboard_selection_owner)
            return null;
        try {
            return this._sourceFactory.clipboard_selection_owner(this._selection);
        } catch (_error) {
            return null;
        }
    }

    _identityTuple() {
        return this._identity ?? ['no-client', 0, 0, ''];
    }

    _validMime(mime) {
        return typeof mime === 'string' && mime.length > 0 && new TextEncoder().encode(mime).length <= MAX_MIME_BYTES;
    }

    _emitOwnerChanged() {
        const mimes = this._inventory();
        if (mimes && !this._destroyed)
            this._impl.emit_signal('OwnerChanged', new GLib.Variant('(tas(suus))', [BigInt(this._sequence), mimes, this._identityTuple()]));
    }

    _cancelReads() {
        for (const cancellable of this._reads)
            cancellable.cancel();
        this._reads.clear();
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
