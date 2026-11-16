package com.copypaste.app.sms;

import android.Manifest;
import android.app.AppOpsManager;
import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Context;
import android.content.pm.PackageManager;
import android.database.ContentObserver;
import android.database.Cursor;
import android.net.Uri;
import android.os.Binder;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.Process;
import android.provider.Telephony;

/**
 * Read-only bridge to new inbox messages for the CopyPaste app, which holds no
 * SMS permission itself. Only packages signed with the CopyPaste key can read
 * it; the caller chooses the starting message and never receives older ones.
 */
public final class SmsInboxProvider extends ContentProvider {
    static final String BRIDGE_PERMISSION = "com.copypaste.app.permission.SMS_BRIDGE";
    private static final String OTP_OP = "android:read_otp_sms";
    private static final int PAGE_LIMIT = 32;

    private ContentObserver inboxObserver;

    @Override
    public boolean onCreate() {
        return true;
    }

    @Override
    public Cursor query(Uri uri, String[] projection, String selection, String[] selectionArgs,
                        String sortOrder) {
        Context context = requireContext();
        if (!granted(context)) return null;
        observeInbox(context, uri);
        long after = parameter(uri, "after");
        long since = parameter(uri, "since");
        // The inbox is read as this app, never on behalf of the caller's identity.
        long identity = Binder.clearCallingIdentity();
        try {
            return context.getContentResolver().query(
                    Telephony.Sms.Inbox.CONTENT_URI,
                    new String[] {Telephony.Sms._ID, Telephony.Sms.BODY},
                    Telephony.Sms._ID + " > ? AND " + Telephony.Sms.DATE + " >= ?",
                    new String[] {Long.toString(after), Long.toString(since)},
                    Telephony.Sms._ID + " ASC LIMIT " + PAGE_LIMIT);
        } finally {
            Binder.restoreCallingIdentity(identity);
        }
    }

    /** Reports whether this app currently holds the SMS access it needs. */
    @Override
    public Bundle call(String method, String arg, Bundle extras) {
        Context context = requireContext();
        context.enforceCallingPermission(BRIDGE_PERMISSION, "CopyPaste SMS bridge");
        if (!"access".equals(method)) return null;
        Bundle result = new Bundle();
        result.putBoolean("granted", granted(context));
        return result;
    }

    static boolean granted(Context context) {
        if (context.checkSelfPermission(Manifest.permission.READ_SMS) != PackageManager.PERMISSION_GRANTED
                || context.checkSelfPermission(Manifest.permission.RECEIVE_SMS) != PackageManager.PERMISSION_GRANTED) {
            return false;
        }
        AppOpsManager appOps = context.getSystemService(AppOpsManager.class);
        try {
            return appOps.checkOpNoThrow(OTP_OP, Process.myUid(), context.getPackageName())
                    == AppOpsManager.MODE_ALLOWED;
        } catch (IllegalArgumentException unsupported) {
            return true;
        }
    }

    /** Forwards inbox changes, including hash-addressed SMS that never broadcast. */
    private synchronized void observeInbox(Context context, Uri uri) {
        if (inboxObserver != null) return;
        Uri messages = uri.buildUpon().clearQuery().build();
        inboxObserver = new ContentObserver(new Handler(Looper.getMainLooper())) {
            @Override
            public void onChange(boolean selfChange) {
                context.getContentResolver().notifyChange(messages, null);
            }
        };
        context.getContentResolver().registerContentObserver(Telephony.Sms.CONTENT_URI, true, inboxObserver);
    }

    private static long parameter(Uri uri, String name) {
        try {
            return Math.max(0L, Long.parseLong(uri.getQueryParameter(name)));
        } catch (RuntimeException invalid) {
            return Long.MAX_VALUE;
        }
    }

    @Override
    public String getType(Uri uri) {
        return null;
    }

    @Override
    public Uri insert(Uri uri, ContentValues values) {
        throw new UnsupportedOperationException();
    }

    @Override
    public int delete(Uri uri, String selection, String[] selectionArgs) {
        throw new UnsupportedOperationException();
    }

    @Override
    public int update(Uri uri, ContentValues values, String selection, String[] selectionArgs) {
        throw new UnsupportedOperationException();
    }
}
