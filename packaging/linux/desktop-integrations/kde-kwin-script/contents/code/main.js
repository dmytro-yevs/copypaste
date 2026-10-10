const HOST_BUS_NAME = 'app.copypaste.CopyPaste';
const HOST_OBJECT_PATH = '/app/copypaste/WaylandIntegration';
const HOST_INTERFACE = 'app.copypaste.WaylandIntegration';
const HOST_DESKTOP_FILE_NAME = 'com.copypaste.CopyPaste';
const QUICK_PASTE_CAPTION = 'CopyPaste Quick Paste';

let activeTransaction = null;
let awaitingShortcut = false;
let awaitGeneration = 0;
let restorePending = null;
let deadlineTimer = null;
let qualificationTimer = null;
let qualificationClose = null;

function newTransactionId() {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, character => {
        const random = Math.floor(Math.random() * 16);
        const value = character === 'x' ? random : (random & 0x3) | 0x8;
        return value.toString(16);
    });
}

function callHost(method, transactionId, callback) {
    callDBus(
        HOST_BUS_NAME,
        HOST_OBJECT_PATH,
        HOST_INTERFACE,
        method,
        transactionId,
        callback
    );
}

function isHostWindow(window) {
    return window && !window.deleted && window.desktopFileName === HOST_DESKTOP_FILE_NAME;
}

function isQuickPasteWindow(window) {
    return isHostWindow(window) && window.caption === QUICK_PASTE_CAPTION;
}

function qualificationHostWindow(pid) {
    return workspace.windowList().find(window => isHostWindow(window) && window.pid === pid);
}

function reportQualification(transaction, pid, mapped) {
    callDBus(HOST_BUS_NAME, HOST_OBJECT_PATH, HOST_INTERFACE,
        'QualificationObserved', transaction, pid, HOST_DESKTOP_FILE_NAME,
        'main', mapped, () => {});
}

function awaitQualification() {
    callDBus(HOST_BUS_NAME, HOST_OBJECT_PATH, HOST_INTERFACE,
        'AwaitQualification', (transaction, action, pid) => {
            if (!transaction || !action || !pid || qualificationClose?.transaction === transaction)
                return;
            const window = qualificationHostWindow(pid);
            if (!window)
                return;
            if (action === 'close-main') {
                qualificationClose = {transaction, pid, window};
                reportQualification(transaction, pid, true);
                window.closeWindow();
            } else if (action === 'quick-paste' && workspace.activeWindow === window) {
                beginQuickPaste();
            }
        });
}

function recoverAwaitForHostWindow(window) {
    if (!isHostWindow(window) || activeTransaction)
        return;
    awaitGeneration += 1;
    awaitingShortcut = false;
    awaitQuickPaste();
}

function cancelTransaction(pending) {
    if (activeTransaction !== pending)
        return;
    restoreFocusWithoutPaste(pending);
    finishTransaction(pending);
    callHost('CancelQuickPaste', pending.id, () => {});
    awaitQuickPaste();
}

function restoreFocusWithoutPaste(pending) {
    if (!pending.window || pending.window.deleted || !pending.presentationWindow || pending.presentationWindow.deleted)
        return;
    if (workspace.activeWindow === pending.presentationWindow)
        workspace.activateWindow(pending.window);
}

function finishTransaction(pending) {
    if (activeTransaction !== pending)
        return false;
    activeTransaction = null;
    if (restorePending === pending)
        restorePending = null;
    if (deadlineTimer)
        deadlineTimer.stop();
    deadlineTimer = null;
    return true;
}

function armDeadline(pending) {
    deadlineTimer = new QTimer();
    deadlineTimer.interval = 120000;
    deadlineTimer.singleShot = true;
    deadlineTimer.timeout.connect(() => cancelTransaction(pending));
    deadlineTimer.start();
}

function pasteIntoRestoredWindow(pending) {
    if (restorePending !== pending)
        return;
    if (activeTransaction !== pending || !pending.window || pending.window.deleted || workspace.activeWindow !== pending.window) {
        cancelTransaction(pending);
        return;
    }
    restorePending = null;
    callHost('PasteIntoRestoredWindow', pending.id, () => {
        if (activeTransaction === pending) {
            finishTransaction(pending);
            awaitQuickPaste();
        }
    });
}

function restoreThenPaste(pending) {
    try {
        if (!pending.window || pending.window.deleted) {
            cancelTransaction(pending);
            return;
        }
        restorePending = pending;
        workspace.activateWindow(pending.window);
        if (workspace.activeWindow === pending.window)
            pasteIntoRestoredWindow(pending);
    } catch (_error) {
        cancelTransaction(pending);
    }
}

function beginQuickPaste() {
    if (activeTransaction)
        return;
    const focusedWindow = workspace.activeWindow;
    if (!focusedWindow || focusedWindow.deleted) {
        awaitQuickPaste();
        return;
    }
    const pending = {
        id: newTransactionId(),
        window: focusedWindow,
        presentationWindow: null,
    };
    activeTransaction = pending;
    armDeadline(pending);
    callHost('BeginQuickPaste', pending.id, accepted => {
        if (activeTransaction !== pending)
            return;
        if (!accepted) {
            cancelTransaction(pending);
            return;
        }
        restoreThenPaste(pending);
    });
}

function awaitQuickPaste() {
    if (awaitingShortcut || activeTransaction)
        return;
    const generation = ++awaitGeneration;
    awaitingShortcut = true;
    callDBus(
        HOST_BUS_NAME,
        HOST_OBJECT_PATH,
        HOST_INTERFACE,
        'AwaitQuickPaste',
        triggered => {
            if (generation !== awaitGeneration)
                return;
            awaitingShortcut = false;
            if (triggered)
                beginQuickPaste();
        }
    );
}

workspace.windowAdded.connect(window => {
    if (qualificationClose && isHostWindow(window) && window.pid === qualificationClose.pid) {
        reportQualification(qualificationClose.transaction, qualificationClose.pid, true);
        qualificationClose = null;
    }
    if (activeTransaction && isQuickPasteWindow(window)) {
        activeTransaction.presentationWindow = window;
        workspace.activateWindow(window);
    }
    recoverAwaitForHostWindow(window);
});
workspace.windowActivated.connect(window => {
    if (restorePending && window === restorePending.window)
        pasteIntoRestoredWindow(restorePending);
    else
        recoverAwaitForHostWindow(window);
});
workspace.windowRemoved.connect(window => {
    if (qualificationClose?.window === window) {
        reportQualification(qualificationClose.transaction, qualificationClose.pid, false);
    }
    if (activeTransaction?.window === window)
        cancelTransaction(activeTransaction);
});

awaitQuickPaste();
qualificationTimer = new QTimer();
qualificationTimer.interval = 250;
qualificationTimer.timeout.connect(awaitQualification);
qualificationTimer.start();
