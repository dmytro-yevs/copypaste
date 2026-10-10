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

#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusMetaType>
#include <QFileInfo>
#include <QSet>
#include <QSocketNotifier>
#include <QThread>

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
constexpr auto kObjectPath = "/app/copypaste/Clipboard";
constexpr uint kVersion = 2;
constexpr uint kMaximumBytes = 4 * 1024 * 1024;
constexpr uint kMaximumWriteBytes = 32 * 1024 * 1024;
constexpr uint kMaximumMimeTypes = 64;
constexpr uint kMaximumMimeBytes = 255;
constexpr int kReadTimeoutMs = 2'000;

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
        new PendingWrite(fd, *payload, this);
    }

    void cancel() override
    {
    }

    QStringList mimeTypes() const override
    {
        return m_payloads.keys();
    }

private:
    class PendingWrite final : public QObject
    {
    public:
        PendingWrite(int fd, QByteArray bytes, QObject *parent)
            : QObject(parent)
            , m_fd(fd)
            , m_bytes(std::move(bytes))
            , m_notifier(fd, QSocketNotifier::Write, this)
        {
            const int flags = fcntl(m_fd, F_GETFL);
            if (flags == -1 || fcntl(m_fd, F_SETFL, flags | O_NONBLOCK) == -1) {
                finish();
                return;
            }
            connect(&m_notifier, &QSocketNotifier::activated, this, &PendingWrite::writeMore);
            QMetaObject::invokeMethod(this, &PendingWrite::writeMore, Qt::QueuedConnection);
        }

        ~PendingWrite() override
        {
            if (m_fd != -1) {
                close(m_fd);
            }
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

QDBusArgument &operator<<(QDBusArgument &argument, const CopyPasteClipboardSnapshot &snapshot)
{
    argument.beginStructure();
    argument << snapshot.generation << snapshot.mimeTypes << snapshot.identity;
    argument.endStructure();
    return argument;
}

const QDBusArgument &operator>>(const QDBusArgument &argument, CopyPasteClipboardSnapshot &snapshot)
{
    argument.beginStructure();
    argument >> snapshot.generation >> snapshot.mimeTypes >> snapshot.identity;
    argument.endStructure();
    return argument;
}

CopyPasteClipboardBridge::CopyPasteClipboardBridge(QObject *parent)
    : QObject(parent)
{
    qDBusRegisterMetaType<CopyPasteWriterIdentity>();
    qDBusRegisterMetaType<CopyPasteClipboardSnapshot>();
    const auto bus = QDBusConnection::sessionBus();
    bus.registerObject(QLatin1String(kObjectPath), this,
                       QDBusConnection::ExportScriptableSlots | QDBusConnection::ExportScriptableSignals);
    if (const auto interface = bus.interface()) {
        connect(interface, &QDBusConnectionInterface::serviceOwnerChanged, this,
                [this](const QString &name, const QString &, const QString &newOwner) {
                    if (name == QLatin1String(kDaemonBusName) && newOwner.isEmpty()) {
                        cancelPendingReads();
                    }
                });
    }
    connect(waylandServer()->seat(), &SeatInterface::selectionChanged, this, &CopyPasteClipboardBridge::selectionChanged);
    selectionChanged(waylandServer()->seat()->selection());
}

uint CopyPasteClipboardBridge::Version() const
{
    return kVersion;
}

CopyPasteClipboardSnapshot CopyPasteClipboardBridge::Snapshot()
{
    if (!authorize()) {
        return {};
    }
    if (!hasValidInventory(m_selection.source)) {
        fail(QStringLiteral("Unavailable"));
        return {};
    }
    return snapshot();
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
    m_pendingRead = cancelled;
    auto *worker = QThread::create([bridge, request, bus, generation, source, maxBytes, cancelled, readFd = fds[0]]() mutable {
        ReadResult result = readBounded(readFd, maxBytes, cancelled);
        QMetaObject::invokeMethod(bridge, [bridge, request, bus, generation, source, cancelled, result = std::move(result)]() {
            if (!bridge) {
                return;
            }
            if (bridge->m_pendingRead == cancelled) {
                bridge->m_pendingRead.reset();
            }
            if (!bridge->isCurrent(generation, source)) {
                bus.send(request.createErrorReply(QStringLiteral("app.copypaste.Clipboard.Error.StaleSelection"), QStringLiteral("StaleSelection")));
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

bool CopyPasteClipboardBridge::authorize()
{
    if (!calledFromDBus()) {
        return true;
    }
    const auto bus = connection();
    const auto interface = bus.interface();
    const auto sender = message().sender();
    if (!interface || sender.isEmpty() || interface->serviceOwner(QLatin1String(kDaemonBusName)) != sender ||
        interface->serviceUid(sender) != getuid()) {
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

void CopyPasteClipboardBridge::cancelPendingReads()
{
    if (m_pendingRead) {
        m_pendingRead->store(true);
        m_pendingRead.reset();
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
    if (!workspace()) {
        return {QStringLiteral("no-app-id"), static_cast<quint32>(client->processId()), static_cast<quint32>(client->userId()), {}};
    }
    for (auto *window : workspace()->windows()) {
        if (window->surface() && window->surface()->client() == client && !window->desktopFileName().isEmpty()) {
            appIds.insert(window->desktopFileName());
        }
    }
    const auto pid = client->processId() > 0 ? static_cast<quint32>(client->processId()) : 0;
    const auto uid = static_cast<quint32>(client->userId());
    if (appIds.isEmpty()) {
        return {QStringLiteral("no-app-id"), pid, uid, {}};
    }
    if (appIds.size() != 1) {
        return {QStringLiteral("ambiguous"), pid, uid, {}};
    }
    return {QStringLiteral("verified"), pid, uid, *appIds.cbegin()};
}

CopyPasteClipboardSnapshot CopyPasteClipboardBridge::snapshot() const
{
    if (!m_selection.source) {
        return {m_generation, {}, m_selection.identity};
    }
    return {m_generation, m_selection.source->mimeTypes(), m_selection.identity};
}

void CopyPasteClipboardBridge::selectionChanged(AbstractDataSource *source)
{
    cancelPendingReads();
    const auto oldSource = m_selection.source;
    if (oldSource && oldSource != source) {
        if (auto *memorySource = dynamic_cast<MemoryDataSource *>(oldSource.data())) {
            memorySource->deleteLater();
        }
    }
    ++m_generation;
    m_selection.source = source;
    m_selection.identity = identityFor(source);
    const auto state = snapshot();
    Q_EMIT OwnerChanged(state.generation, hasValidInventory(source) ? state.mimeTypes : QStringList{}, state.identity);
}
} // namespace KWin

#include "copypasteclipboardbridge.moc"
