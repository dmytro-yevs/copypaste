/*
    SPDX-FileCopyrightText: 2026 CopyPaste contributors
    SPDX-License-Identifier: GPL-2.0-or-later
*/
#include "copypasteclipboardbridge.h"

#include "wayland/abstract_data_source.h"
#include "wayland/clientconnection.h"
#include "wayland/display.h"
#include "wayland/seat.h"
#include "wayland/surface.h"
#include "wayland_server.h"
#include "window.h"
#include "workspace.h"
#include "xdgshellwindow.h"

#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusMetaType>
#include <QSet>
#include <QSocketNotifier>
#include <QThread>
#include <QTimer>

#include <chrono>
#include <algorithm>
#include <cerrno>
#include <climits>
#include <cstring>
#include <fcntl.h>
#include <poll.h>
#include <sys/types.h>
#include <unistd.h>

namespace KWin
{
namespace
{
constexpr auto kDaemonBusName = "app.copypaste.Daemon";
constexpr auto kGuiBusName = "app.copypaste.CopyPaste";
constexpr auto kObjectPath = "/app/copypaste/Clipboard";
constexpr uint kVersion = 2;
constexpr uint kMaximumBytes = 4 * 1024 * 1024;
constexpr uint kMaximumWriteBytes = 32 * 1024 * 1024;
constexpr uint kMaximumMimeTypes = 64;
constexpr uint kMaximumMimeBytes = 255;
constexpr int kReadTimeoutMs = 2'000;
constexpr int kWriteTimeoutMs = 2'000;
constexpr int kAuthorizerTimeoutMs = 250;
constexpr qsizetype kMaximumPendingWrites = 8;

class MemoryDataSource final : public AbstractDataSource
{
public:
    explicit MemoryDataSource(QMap<QString, QByteArray> payloads, QObject *parent)
        : AbstractDataSource(parent)
        , m_payloads(std::move(payloads))
    {
    }

    void requestData(const QString &mimeType, qint32 fd) override
    {
        const auto payload = m_payloads.constFind(mimeType);
        if (payload == m_payloads.cend()) {
            close(fd);
            return;
        }
        if (m_pendingWrites.size() >= kMaximumPendingWrites) {
            close(fd);
            return;
        }
        new PendingWrite(fd, *payload, this);
    }

    void cancel() override
    {
        const auto pendingWrites = m_pendingWrites;
        for (auto *pendingWrite : pendingWrites) {
            pendingWrite->cancel();
        }
    }

    QStringList mimeTypes() const override
    {
        return m_payloads.keys();
    }

private:
    class PendingWrite final : public QObject
    {
    public:
        PendingWrite(int fd, QByteArray bytes, MemoryDataSource *source)
            : QObject(source)
            , m_fd(fd)
            , m_bytes(std::move(bytes))
            , m_notifier(fd, QSocketNotifier::Write, this)
        {
            source->m_pendingWrites.insert(this);
            connect(this, &QObject::destroyed, source, [source, this] { source->m_pendingWrites.remove(this); });
            const int flags = fcntl(m_fd, F_GETFL);
            if (flags == -1 || fcntl(m_fd, F_SETFL, flags | O_NONBLOCK) == -1) {
                finish();
                return;
            }
            connect(&m_notifier, &QSocketNotifier::activated, this, &PendingWrite::writeMore);
            QTimer::singleShot(kWriteTimeoutMs, this, &PendingWrite::cancel);
            QMetaObject::invokeMethod(this, &PendingWrite::writeMore, Qt::QueuedConnection);
        }

        ~PendingWrite() override
        {
            if (m_fd != -1) {
                close(m_fd);
            }
        }

    public:
        void cancel()
        {
            finish();
        }

    private:
        void writeMore()
        {
            while (m_offset < m_bytes.size()) {
                const auto written = write(m_fd, m_bytes.constData() + m_offset, m_bytes.size() - m_offset);
                if (written > 0) {
                    m_offset += written;
                    continue;
                }
                if (written == -1 && errno == EINTR) {
                    continue;
                }
                if (written == -1 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
                    return;
                }
                finish();
                return;
            }
            finish();
        }

        void finish()
        {
            m_notifier.setEnabled(false);
            if (m_fd != -1) {
                close(m_fd);
                m_fd = -1;
            }
            deleteLater();
        }

        int m_fd;
        QByteArray m_bytes;
        qsizetype m_offset = 0;
        QSocketNotifier m_notifier;
    };

    QMap<QString, QByteArray> m_payloads;
    QSet<PendingWrite *> m_pendingWrites;
};

struct ReadResult {
    QByteArray bytes;
    QString error;
};

ReadResult readBounded(int fd, uint maxBytes, const std::shared_ptr<std::atomic_bool> &cancelled)
{
    QByteArray bytes;
    bytes.reserve(qMin(maxBytes, 64 * 1024u));
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(kReadTimeoutMs);
    char buffer[16 * 1024];
    while (true) {
        if (cancelled->load()) {
            close(fd);
            return {{}, QStringLiteral("Unavailable")};
        }
        const auto now = std::chrono::steady_clock::now();
        if (now >= deadline) {
            close(fd);
            return {{}, QStringLiteral("Unavailable")};
        }
        const auto remaining = std::chrono::duration_cast<std::chrono::milliseconds>(deadline - now).count();
        pollfd event{fd, POLLIN | POLLHUP, 0};
        const auto polled = poll(&event, 1, qMin<qint64>(100, qMax<qint64>(1, remaining)));
        if (polled == 0) {
            continue;
        }
        if (polled < 0) {
            if (errno == EINTR) {
                continue;
            }
            close(fd);
            return {{}, QStringLiteral("Unavailable")};
        }
        const auto readCount = read(fd, buffer, sizeof(buffer));
        if (readCount > 0) {
            if (bytes.size() + readCount > maxBytes) {
                close(fd);
                return {{}, QStringLiteral("TooLarge")};
            }
            bytes.append(buffer, readCount);
            continue;
        }
        if (readCount == 0) {
            close(fd);
            return {std::move(bytes), {}};
        }
        if (errno != EINTR && errno != EAGAIN && errno != EWOULDBLOCK) {
            close(fd);
            return {{}, QStringLiteral("Unavailable")};
        }
    }
}
} // namespace

QDBusArgument &operator<<(QDBusArgument &argument, const CopyPasteWriterIdentity &identity)
{
    argument.beginStructure();
    argument << identity.status << identity.pid << identity.uid << identity.appId;
    argument.endStructure();
    return argument;
}

const QDBusArgument &operator>>(const QDBusArgument &argument, CopyPasteWriterIdentity &identity)
{
    argument.beginStructure();
    argument >> identity.status >> identity.pid >> identity.uid >> identity.appId;
    argument.endStructure();
    return argument;
}

CopyPasteClipboardBridge::CopyPasteClipboardBridge(QObject *parent)
    : QObject(parent)
{
    qDBusRegisterMetaType<CopyPasteWriterIdentity>();
    qDBusRegisterMetaType<QMap<QString, QByteArray>>();
    auto bus = QDBusConnection::sessionBus();
    bus.registerObject(QLatin1String(kObjectPath), this,
                       QDBusConnection::ExportScriptableSlots | QDBusConnection::ExportScriptableSignals);
    if (const auto interface = bus.interface()) {
        connect(interface, &QDBusConnectionInterface::serviceOwnerChanged, this,
                [this](const QString &name, const QString &oldOwner, const QString &newOwner) {
                    if (name == QLatin1String(kDaemonBusName) && oldOwner != newOwner) {
                        ++m_daemonOwnerEpoch;
                        cancelPendingTransfers();
                    }
                });
    }
    connect(waylandServer()->seat(), &SeatInterface::selectionChanged, this, &CopyPasteClipboardBridge::selectionChanged);
    selectionChanged(waylandServer()->seat()->selection());
}

void CopyPasteClipboardBridge::Version(uint &version)
{
    if (!authorize(true)) {
        return;
    }
    version = kVersion;
}

void CopyPasteClipboardBridge::Snapshot(qulonglong &generation, QStringList &mimeTypes, CopyPasteWriterIdentity &identity)
{
    if (!authorize()) {
        return;
    }
    if (!hasValidInventory(m_selection.source)) {
        fail(QStringLiteral("Unavailable"));
        return;
    }
    snapshot(generation, mimeTypes, identity);
}

QByteArray CopyPasteClipboardBridge::Read(qulonglong generation, const QString &mimeType, uint maxBytes)
{
    if (!authorize()) {
        return {};
    }
    const auto source = m_selection.source;
    if (!isCurrent(generation, source)) {
        fail(QStringLiteral("StaleSelection"));
        return {};
    }
    if (!isValidMimeType(mimeType) || !source->mimeTypes().contains(mimeType)) {
        fail(QStringLiteral("UnsupportedMime"));
        return {};
    }
    if (maxBytes == 0 || maxBytes > kMaximumBytes) {
        fail(QStringLiteral("TooLarge"));
        return {};
    }
    if (m_pendingRead) {
        fail(QStringLiteral("Unavailable"));
        return {};
    }
    int fds[2];
    if (pipe2(fds, O_CLOEXEC) != 0) {
        fail(QStringLiteral("Unavailable"));
        return {};
    }
    source->requestData(mimeType, fds[1]);
    setDelayedReply(true);
    const QDBusMessage request = message();
    const QDBusConnection bus = connection();
    const QPointer<CopyPasteClipboardBridge> bridge(this);
    const auto cancelled = std::make_shared<std::atomic_bool>(false);
    const auto daemonOwnerEpoch = m_daemonOwnerEpoch;
    m_pendingRead = cancelled;
    auto *worker = QThread::create([bridge, request, bus, generation, source, maxBytes, cancelled, daemonOwnerEpoch, readFd = fds[0]]() mutable {
        ReadResult result = readBounded(readFd, maxBytes, cancelled);
        QMetaObject::invokeMethod(bridge, [bridge, request, bus, generation, source, cancelled, daemonOwnerEpoch, result = std::move(result)]() {
            if (!bridge) {
                return;
            }
            if (bridge->m_pendingRead == cancelled) {
                bridge->m_pendingRead.reset();
            }
            if (!bridge->isCurrent(generation, source)) {
                bus.send(request.createErrorReply(QStringLiteral("app.copypaste.Clipboard.Error.StaleSelection"), QStringLiteral("StaleSelection")));
            } else if (cancelled->load() || bridge->m_daemonOwnerEpoch != daemonOwnerEpoch) {
                bus.send(request.createErrorReply(QStringLiteral("app.copypaste.Clipboard.Error.Unavailable"), QStringLiteral("Unavailable")));
            } else if (!result.error.isEmpty()) {
                bus.send(request.createErrorReply(QStringLiteral("app.copypaste.Clipboard.Error.") + result.error, result.error));
            } else {
                bus.send(request.createReply(QVariant::fromValue(result.bytes)));
            }
        }, Qt::QueuedConnection);
    });
    connect(worker, &QThread::finished, worker, &QObject::deleteLater);
    worker->start();
    return {};
}

qulonglong CopyPasteClipboardBridge::Write(const QMap<QString, QByteArray> &payloads)
{
    if (!authorize()) {
        return 0;
    }
    if (payloads.isEmpty() || payloads.size() > kMaximumMimeTypes) {
        fail(QStringLiteral("TooLarge"));
        return 0;
    }
    quint64 totalBytes = 0;
    for (auto it = payloads.cbegin(); it != payloads.cend(); ++it) {
        if (!isValidMimeType(it.key())) {
            fail(QStringLiteral("UnsupportedMime"));
            return 0;
        }
        if (it.value().size() > kMaximumBytes) {
            fail(QStringLiteral("TooLarge"));
            return 0;
        }
        totalBytes += it.value().size();
    }
    if (totalBytes > kMaximumWriteBytes) {
        fail(QStringLiteral("TooLarge"));
        return 0;
    }
    auto *source = new MemoryDataSource(payloads, this);
    waylandServer()->seat()->setSelection(source, waylandServer()->display()->nextSerial());
    // setSelection synchronously emits selectionChanged. The source has no client,
    // so its status remains no-client and callers will not attribute it to another app.
    return m_generation;
}

bool CopyPasteClipboardBridge::authorize(bool allowGui)
{
    if (!calledFromDBus()) {
        return true;
    }
    const auto bus = connection();
    const auto sender = message().service();
    if (sender.isEmpty()) {
        fail(QStringLiteral("AccessDenied"));
        return false;
    }
    const auto ownsName = [&bus, &sender](const char *name) {
        QDBusMessage ownerRequest = QDBusMessage::createMethodCall(QStringLiteral("org.freedesktop.DBus"),
                                                                    QStringLiteral("/org/freedesktop/DBus"),
                                                                    QStringLiteral("org.freedesktop.DBus"),
                                                                    QStringLiteral("GetNameOwner"));
        ownerRequest.setArguments({QLatin1String(name)});
        const auto owner = bus.call(ownerRequest, QDBus::Block, kAuthorizerTimeoutMs);
        return owner.type() != QDBusMessage::ErrorMessage && owner.arguments().size() == 1 &&
            owner.arguments().constFirst().toString() == sender;
    };
    if (!ownsName(kDaemonBusName) && (!allowGui || !ownsName(kGuiBusName))) {
        fail(QStringLiteral("AccessDenied"));
        return false;
    }
    QDBusMessage uidRequest = QDBusMessage::createMethodCall(QStringLiteral("org.freedesktop.DBus"),
                                                              QStringLiteral("/org/freedesktop/DBus"),
                                                              QStringLiteral("org.freedesktop.DBus"),
                                                              QStringLiteral("GetConnectionUnixUser"));
    uidRequest.setArguments({sender});
    const auto uidReply = bus.call(uidRequest, QDBus::Block, kAuthorizerTimeoutMs);
    if (uidReply.type() == QDBusMessage::ErrorMessage || uidReply.arguments().size() != 1 ||
        uidReply.arguments().constFirst().toUInt() != getuid()) {
        fail(QStringLiteral("AccessDenied"));
        return false;
    }
    return true;
}

void CopyPasteClipboardBridge::fail(const QString &code)
{
    sendErrorReply(QStringLiteral("app.copypaste.Clipboard.Error.") + code, code);
}

bool CopyPasteClipboardBridge::isValidMimeType(const QString &mimeType) const
{
    return !mimeType.isEmpty() && mimeType.toUtf8().size() <= kMaximumMimeBytes;
}

bool CopyPasteClipboardBridge::hasValidInventory(const AbstractDataSource *source) const
{
    if (!source) {
        return true;
    }
    const auto mimeTypes = source->mimeTypes();
    return mimeTypes.size() <= kMaximumMimeTypes && std::all_of(mimeTypes.cbegin(), mimeTypes.cend(), [this](const QString &mimeType) {
        return isValidMimeType(mimeType);
    });
}

bool CopyPasteClipboardBridge::isCurrent(qulonglong generation, const AbstractDataSource *source) const
{
    return generation == m_generation && source && source == m_selection.source && source == waylandServer()->seat()->selection();
}

void CopyPasteClipboardBridge::cancelPendingTransfers()
{
    if (m_pendingRead) {
        m_pendingRead->store(true);
        m_pendingRead.reset();
    }
    if (auto *memorySource = dynamic_cast<MemoryDataSource *>(m_selection.source.data())) {
        memorySource->cancel();
    }
}

CopyPasteWriterIdentity CopyPasteClipboardBridge::identityFor(AbstractDataSource *source) const
{
    if (!source || !source->client()) {
        return {QStringLiteral("no-client"), 0, 0, {}};
    }
    auto *client = waylandServer()->display()->getConnection(source->client());
    if (!client) {
        return {QStringLiteral("no-client"), 0, 0, {}};
    }
    QSet<QString> appIds;
    bool hasMatchingToplevel = false;
    bool hasMissingAppId = false;
    if (!workspace()) {
        return {QStringLiteral("no-app-id"), static_cast<quint32>(client->processId()), static_cast<quint32>(client->userId()), {}};
    }
    for (auto *window : workspace()->windows()) {
        auto *toplevel = qobject_cast<XdgToplevelWindow *>(window);
        if (!toplevel || !toplevel->surface() || toplevel->surface()->client() != client) {
            continue;
        }
        hasMatchingToplevel = true;
        const QString appId = toplevel->rawAppId();
        if (appId.isEmpty()) {
            hasMissingAppId = true;
        } else {
            appIds.insert(appId);
        }
    }
    const auto pid = client->processId() > 0 ? static_cast<quint32>(client->processId()) : 0;
    const auto uid = static_cast<quint32>(client->userId());
    if (!hasMatchingToplevel || hasMissingAppId || appIds.isEmpty()) {
        return {QStringLiteral("no-app-id"), pid, uid, {}};
    }
    if (appIds.size() != 1) {
        return {QStringLiteral("ambiguous"), pid, uid, {}};
    }
    return {QStringLiteral("verified"), pid, uid, *appIds.cbegin()};
}

void CopyPasteClipboardBridge::snapshot(qulonglong &generation, QStringList &mimeTypes, CopyPasteWriterIdentity &identity) const
{
    generation = m_generation;
    identity = m_selection.identity;
    if (!m_selection.source) {
        mimeTypes.clear();
        return;
    }
    mimeTypes = m_selection.source->mimeTypes();
}

void CopyPasteClipboardBridge::selectionChanged(AbstractDataSource *source)
{
    cancelPendingTransfers();
    const auto oldSource = m_selection.source;
    if (oldSource && oldSource != source) {
        if (auto *memorySource = dynamic_cast<MemoryDataSource *>(oldSource.data())) {
            memorySource->deleteLater();
        }
    }
    ++m_generation;
    m_selection.source = source;
    m_selection.identity = identityFor(source);
    qulonglong generation = 0;
    QStringList mimeTypes;
    CopyPasteWriterIdentity identity;
    snapshot(generation, mimeTypes, identity);
    Q_EMIT OwnerChanged(generation, hasValidInventory(source) ? mimeTypes : QStringList{}, identity);
}
} // namespace KWin

#include "copypasteclipboardbridge.moc"
