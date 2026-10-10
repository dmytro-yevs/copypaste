/*
    SPDX-FileCopyrightText: 2026 CopyPaste contributors
    SPDX-License-Identifier: GPL-2.0-or-later
*/
#pragma once

#include <QDBusContext>
#include <QByteArray>
#include <QMap>
#include <QObject>
#include <QPointer>
#include <QStringList>
#include <atomic>
#include <memory>

class QDBusArgument;

namespace KWin
{
class AbstractDataSource;

// This type is deliberately small: app identity is accepted by callers only
// when status is "verified". The compositor supplies pid and uid directly.
struct CopyPasteWriterIdentity {
    QString status;
    quint32 pid = 0;
    quint32 uid = 0;
    QString appId;
};

struct CopyPasteClipboardSnapshot {
    qulonglong generation = 0;
    QStringList mimeTypes;
    CopyPasteWriterIdentity identity;
};

QDBusArgument &operator<<(QDBusArgument &argument, const CopyPasteWriterIdentity &identity);
const QDBusArgument &operator>>(const QDBusArgument &argument, CopyPasteWriterIdentity &identity);
QDBusArgument &operator<<(QDBusArgument &argument, const CopyPasteClipboardSnapshot &snapshot);
const QDBusArgument &operator>>(const QDBusArgument &argument, CopyPasteClipboardSnapshot &snapshot);

class CopyPasteClipboardBridge final : public QObject, protected QDBusContext
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "app.copypaste.Clipboard")

public:
    explicit CopyPasteClipboardBridge(QObject *parent = nullptr);

public Q_SLOTS:
    uint Version() const;
    CopyPasteClipboardSnapshot Snapshot();
    QByteArray Read(qulonglong generation, const QString &mimeType, uint maxBytes);
    qulonglong Write(const QMap<QString, QByteArray> &payloads);

Q_SIGNALS:
    void OwnerChanged(qulonglong generation, const QStringList &mimeTypes,
                      const KWin::CopyPasteWriterIdentity &identity);

private:
    struct SelectionState {
        QPointer<AbstractDataSource> source;
        CopyPasteWriterIdentity identity;
    };

    bool authorize();
    void fail(const QString &code);
    bool isValidMimeType(const QString &mimeType) const;
    bool hasValidInventory(const AbstractDataSource *source) const;
    bool isCurrent(qulonglong generation, const AbstractDataSource *source) const;
    void cancelPendingReads();
    CopyPasteWriterIdentity identityFor(AbstractDataSource *source) const;
    CopyPasteClipboardSnapshot snapshot() const;
    void selectionChanged(AbstractDataSource *source);

    qulonglong m_generation = 0;
    SelectionState m_selection;
    std::shared_ptr<std::atomic_bool> m_pendingRead;
};
} // namespace KWin

Q_DECLARE_METATYPE(KWin::CopyPasteWriterIdentity)
Q_DECLARE_METATYPE(KWin::CopyPasteClipboardSnapshot)
