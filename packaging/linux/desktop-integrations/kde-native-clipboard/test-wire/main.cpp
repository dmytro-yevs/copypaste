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

#include <optional>

struct WriterIdentity {
    QString status;
    quint32 pid = 0;
    quint32 uid = 0;
    QString appId;
};
Q_DECLARE_METATYPE(WriterIdentity)

bool mayReplyForOwnerEpoch(qulonglong capturedEpoch, qulonglong currentEpoch, bool cancelled)
{
    return !cancelled && capturedEpoch == currentEpoch;
}

QDBusArgument &operator<<(QDBusArgument &argument, const WriterIdentity &identity)
{
    argument.beginStructure();
    argument << identity.status << identity.pid << identity.uid << identity.appId;
    argument.endStructure();
    return argument;
}

const QDBusArgument &operator>>(const QDBusArgument &argument, WriterIdentity &identity)
{
    argument.beginStructure();
    argument >> identity.status >> identity.pid >> identity.uid >> identity.appId;
    argument.endStructure();
    return argument;
}

class ClipboardFixture final : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "app.copypaste.Clipboard")

public Q_SLOTS:
    Q_SCRIPTABLE void Version(uint &version)
    {
        version = 2;
    }

    Q_SCRIPTABLE void Snapshot(qulonglong &generation, QStringList &mimeTypes, WriterIdentity &identity)
    {
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
    Q_SCRIPTABLE void OwnerChanged(qulonglong generation, const QStringList &mimeTypes, const WriterIdentity &identity);
};

int main(int argc, char **argv)
{
    QCoreApplication application(argc, argv);
    qDBusRegisterMetaType<WriterIdentity>();
    ClipboardFixture fixture;
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
    const auto client = QDBusConnection::connectToBus(server.address(), connectionName);
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
    const auto reply = watcher.reply();
    const auto arguments = reply.arguments();
    if (reply.type() == QDBusMessage::ErrorMessage || reply.signature() != QLatin1String("tas(suus)") || arguments.size() != 3 ||
        arguments.at(0).toULongLong() != 7 || arguments.at(1).toStringList() != QStringList{QStringLiteral("text/plain")}) {
        return 5;
    }
    const auto identity = qdbus_cast<WriterIdentity>(arguments.at(2));
    if (identity.status != QLatin1String("verified") || identity.pid != 101 || identity.uid != 1000 ||
        identity.appId != QLatin1String("org.example.Writer")) {
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
    for (const auto &member : {QStringLiteral("Version"), QStringLiteral("Snapshot"), QStringLiteral("Read"),
                               QStringLiteral("Write"), QStringLiteral("OwnerChanged")}) {
        if (!xml.contains(QStringLiteral("\"") + member + QStringLiteral("\""))) {
            return 9;
        }
    }
    QDBusConnection::disconnectFromBus(connectionName);
    return 0;
}

#include "main.moc"
