const BUS_NAME = 'app.copypaste.QualificationGeometry';
const PATH = '/app/copypaste/QualificationGeometry';
const INTERFACE = 'app.copypaste.QualificationGeometry';

function rectangle(rectangle, stacking) {
    const values = [rectangle?.x, rectangle?.y, rectangle?.width, rectangle?.height];
    return values.every(Number.isInteger) && Number.isInteger(stacking) && values[2] > 0 && values[3] > 0 ? [...values, stacking].join(',') : null;
}

function identifier(window) {
    const value = window?.internalId;
    return value ? value.toString() : '';
}

function report(window) {
    const id = identifier(window);
    const frame = rectangle(window?.frameGeometry, window?.stackingOrder);
    const buffer = rectangle(window?.bufferGeometry, window?.stackingOrder);
    if (!id || !Number.isInteger(window?.pid) || window.pid <= 0 || !window.caption || !frame || !buffer)
        return;
    callDBus(BUS_NAME, PATH, INTERFACE, 'Report', window.pid, window.caption, id, frame, buffer);
}

function remove(window) {
    const id = identifier(window);
    if (id)
        callDBus(BUS_NAME, PATH, INTERFACE, 'Remove', id);
}

for (const window of workspace.windowList())
    report(window);
workspace.windowAdded.connect(report);
workspace.windowRemoved.connect(remove);

const refresh = new QTimer();
refresh.interval = 250;
refresh.timeout.connect(() => {
    for (const window of workspace.windowList())
        report(window);
});
refresh.start();
