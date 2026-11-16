package com.copypaste.app.sms;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/** Wakes the CopyPaste app when an SMS arrives; the message itself stays here. */
public final class SmsReceivedReceiver extends BroadcastReceiver {
    static final String ACTION_INBOX_CHANGED = "com.copypaste.app.action.SMS_INBOX_CHANGED";

    @Override
    public void onReceive(Context context, Intent intent) {
        String host = context.getPackageName().replaceFirst("\\.sms$", "");
        context.sendBroadcast(
                new Intent(ACTION_INBOX_CHANGED).setPackage(host),
                SmsInboxProvider.BRIDGE_PERMISSION);
    }
}
