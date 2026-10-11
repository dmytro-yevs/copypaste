#include <QCoreApplication>
#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusMetaType>
#include <QDBusPendingCallWatcher>
#include <QDBusServer>
#include <QDir>
#include <QEventLoop>
#include <QMap>
#include <QTimer>
#include <QXmlStreamReader>

#include <optional>

#include "../src/copypasteclipboardbridge.h"

bool mayReplyForOwnerEpoch(qulonglong capturedEpoch, qulonglong currentEpoch, bool cancelled)
{
    return !cancelled && capturedEpoch == currentEpoch;
}

namespace KWin
{
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
} // namespace KWin

bool hasSnapshotOutSignature(const QString &xml)
{
    QXmlStreamReader reader(xml);
    bool inSnapshot = false;
    QStringList outputTypes;

    while (!reader.atEnd()) {
        reader.readNext();
        if (reader.isStartElement() && reader.name() == QLatin1String("method")) {
            inSnapshot = reader.attributes().value(QLatin1String("name")) == QLatin1String("Snapshot");
        } else if (inSnapshot && reader.isStartElement() && reader.name() == QLatin1String("arg") &&
                   reader.attributes().value(QLatin1String("direction")) == QLatin1String("out")) {
            outputTypes.append(reader.attributes().value(QLatin1String("type")).toString());
        } else if (inSnapshot && reader.isEndElement() && reader.name() == QLatin1String("method")) {
            return outputTypes == QStringList{QStringLiteral("t"), QStringLiteral("as"), QStringLiteral("(suus)")};
        }
    }

    return false;
}

QDBusMessage snapshotReply(const QDBusConnection &client)
{
    const auto pending = client.asyncCall(QDBusMessage::createMethodCall(QString(),
                                                                           QStringLiteral("/app/copypaste/Clipboard"),
                                                                           QStringLiteral("app.copypaste.Clipboard"),
                                                                           QStringLiteral("Snapshot")),
                                          1'000);
    QDBusPendingCallWatcher watcher(pending);
    QEventLoop replyLoop;
    QObject::connect(&watcher, &QDBusPendingCallWatcher::finished, &replyLoop, &QEventLoop::quit);
    QTimer::singleShot(1'000, &replyLoop, &QEventLoop::quit);
    replyLoop.exec();
    return watcher.reply();
}

bool hasSnapshotValues(const QDBusMessage &reply, qulonglong generation, const QStringList &mimeTypes,
                       const KWin::CopyPasteWriterIdentity &expectedIdentity)
{
    const auto arguments = reply.arguments();
    if (reply.type() == QDBusMessage::ErrorMessage || reply.signature() != QLatin1String("tas(suus)") || arguments.size() != 3 ||
        arguments.at(0).toULongLong() != generation || arguments.at(1).toStringList() != mimeTypes) {
        return false;
    }
    const auto identity = qdbus_cast<KWin::CopyPasteWriterIdentity>(arguments.at(2));
    return identity.status == expectedIdentity.status && identity.pid == expectedIdentity.pid && identity.uid == expectedIdentity.uid &&
           identity.appId == expectedIdentity.appId;
}

namespace KWin
{
class ClipboardFixture final : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "app.copypaste.Clipboard")

public Q_SLOTS:
    Q_SCRIPTABLE void Version(uint &version)
    {
        version = 2;
    }

#if defined(COPYPASTE_TEST_UNQUALIFIED_SNAPSHOT)
    Q_SCRIPTABLE void Snapshot(qulonglong &generation, QStringList &mimeTypes, CopyPasteWriterIdentity &identity)
#else
    Q_SCRIPTABLE void Snapshot(qulonglong &generation, QStringList &mimeTypes, KWin::CopyPasteWriterIdentity &identity)
#endif
    {
        if (!m_hasSelection) {
            generation = 0;
            mimeTypes.clear();
            identity = {};
            return;
        }
        generation = 7;
        mimeTypes = {QStringLiteral("text/plain")};
        identity = {QStringLiteral("verified"), 101, 1000, QStringLiteral("org.example.Writer")};
    }

    Q_SCRIPTABLE QByteArray Read(qulonglong, const QString &, uint)
    {
        return {};
    }

    Q_SCRIPTABLE qulonglong Write(const QMap<QString, QByteArray> &)
    {
        return 8;
    }

Q_SIGNALS:
    Q_SCRIPTABLE void OwnerChanged(qulonglong generation, const QStringList &mimeTypes,
                                   const KWin::CopyPasteWriterIdentity &identity);

public:
    void setHasSelection(bool hasSelection)
    {
        m_hasSelection = hasSelection;
    }

private:
    bool m_hasSelection = false;
};
} // namespace KWin

int main(int argc, char **argv)
{
    QCoreApplication application(argc, argv);
    qDBusRegisterMetaType<KWin::CopyPasteWriterIdentity>();
    qDBusRegisterMetaType<QMap<QString, QByteArray>>();
    KWin::ClipboardFixture fixture;
    QDBusServer server(QStringLiteral("unix:tmpdir=%1").arg(QDir::tempPath()));
    if (!server.isConnected()) {
        return 1;
    }

    std::optional<QDBusConnection> peerConnection;
    bool registered = false;
    QEventLoop setupLoop;
    QObject::connect(&server, &QDBusServer::newConnection, &application, [&fixture, &peerConnection, &registered, &setupLoop](const QDBusConnection &connection) {
        peerConnection.emplace(connection);
        registered = peerConnection->registerObject(QStringLiteral("/app/copypaste/Clipboard"), &fixture,
                                                    QDBusConnection::ExportScriptableSlots | QDBusConnection::ExportScriptableSignals);
        setupLoop.quit();
    });
    QTimer::singleShot(1'000, &setupLoop, &QEventLoop::quit);

    const QString connectionName = QStringLiteral("copypaste-kwin-wire-fixture-client");
    const auto client = QDBusConnection::connectToPeer(server.address(), connectionName);
    if (!client.isConnected()) {
        return 2;
    }
    setupLoop.exec();
    if (!registered) {
        return 3;
    }

    const auto versionPending = client.asyncCall(QDBusMessage::createMethodCall(QString(),
                                                                                   QStringLiteral("/app/copypaste/Clipboard"),
                                                                                   QStringLiteral("app.copypaste.Clipboard"),
                                                                                   QStringLiteral("Version")),
                                                 1'000);
    QDBusPendingCallWatcher versionWatcher(versionPending);
    QEventLoop versionReplyLoop;
    QObject::connect(&versionWatcher, &QDBusPendingCallWatcher::finished, &versionReplyLoop, &QEventLoop::quit);
    QTimer::singleShot(1'000, &versionReplyLoop, &QEventLoop::quit);
    versionReplyLoop.exec();
    const auto versionReply = versionWatcher.reply();
    if (versionReply.type() == QDBusMessage::ErrorMessage || versionReply.signature() != QLatin1String("u") ||
        versionReply.arguments().size() != 1 || versionReply.arguments().constFirst().toUInt() != 2) {
        return 4;
    }

    if (!hasSnapshotValues(snapshotReply(client), 0, {}, {})) {
        return 5;
    }
    fixture.setHasSelection(true);
    const KWin::CopyPasteWriterIdentity expectedIdentity{QStringLiteral("verified"), 101, 1000, QStringLiteral("org.example.Writer")};
    if (!hasSnapshotValues(snapshotReply(client), 7, {QStringLiteral("text/plain")}, expectedIdentity)) {
        return 6;
    }
    if (mayReplyForOwnerEpoch(7, 8, false) || mayReplyForOwnerEpoch(7, 7, true) || !mayReplyForOwnerEpoch(7, 7, false)) {
        return 7;
    }
    const auto introspection = client.asyncCall(QDBusMessage::createMethodCall(QString(),
                                                                                 QStringLiteral("/app/copypaste/Clipboard"),
                                                                                 QStringLiteral("org.freedesktop.DBus.Introspectable"),
                                                                                 QStringLiteral("Introspect")),
                                                1'000);
    QDBusPendingCallWatcher introspectionWatcher(introspection);
    QEventLoop introspectionLoop;
    QObject::connect(&introspectionWatcher, &QDBusPendingCallWatcher::finished, &introspectionLoop, &QEventLoop::quit);
    QTimer::singleShot(1'000, &introspectionLoop, &QEventLoop::quit);
    introspectionLoop.exec();
    const auto introspectionReply = introspectionWatcher.reply();
    if (introspectionReply.type() == QDBusMessage::ErrorMessage || introspectionReply.arguments().size() != 1) {
        return 8;
    }
    const auto xml = introspectionReply.arguments().constFirst().toString();
    if (!hasSnapshotOutSignature(xml)) {
        return 9;
    }
    for (const auto &member : {QStringLiteral("Version"), QStringLiteral("Snapshot"), QStringLiteral("Read"),
                               QStringLiteral("Write"), QStringLiteral("OwnerChanged")}) {
        if (!xml.contains(QStringLiteral("\"") + member + QStringLiteral("\""))) {
            return 10;
        }
    }
    QDBusConnection::disconnectFromPeer(connectionName);
    return 0;
}

#include "main.moc"
