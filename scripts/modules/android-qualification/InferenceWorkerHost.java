package com.copypaste.qualification;

import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.os.IBinder;
import android.os.ParcelFileDescriptor;
import com.copypaste.app.IInferenceWorker;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;

/** Qualification supplies the same typed launcher upcalls as the product host. */
final class InferenceWorkerHost {
    private static Context context;
    private static final AtomicLong next = new AtomicLong(1);
    private static final ConcurrentHashMap<Long, Session> sessions = new ConcurrentHashMap<>();
    static void initialize(Context value) { context = value; }
    static long[] openWorker() {
        Session session = new Session();
        ParcelFileDescriptor[] pair = null;
        try {
            pair = ParcelFileDescriptor.createSocketPair();
            session.bound = context.bindService(new Intent(context, InferenceWorkerService.class), session, Context.BIND_AUTO_CREATE);
            if (!session.bound || !session.connected.await(30, TimeUnit.SECONDS) || session.remote == null) throw new IllegalStateException("Worker binding failed");
            if (session.remote.open(pair[1]) <= 0) throw new IllegalStateException("Invalid worker identity");
            long id = next.getAndIncrement();
            sessions.put(id, session);
            return new long[] { pair[0].detachFd(), id };
        } catch (Exception error) {
            session.close();
            return new long[] { -1, 0 };
        } finally {
            if (pair != null) for (ParcelFileDescriptor item : pair) {
                try { item.close(); } catch (Exception ignored) { }
            }
        }
    }
    static void closeWorker(long id) {
        Session session = sessions.remove(id);
        if (session != null) session.close();
    }
    private static final class Session implements ServiceConnection {
        final CountDownLatch connected = new CountDownLatch(1);
        final CountDownLatch died = new CountDownLatch(1);
        final AtomicBoolean closed = new AtomicBoolean();
        volatile IInferenceWorker remote;
        volatile boolean bound;
        @Override public void onServiceConnected(ComponentName name, IBinder binder) {
            remote = IInferenceWorker.Stub.asInterface(binder);
            try { binder.linkToDeath(() -> died.countDown(), 0); }
            catch (Exception ignored) { died.countDown(); }
            connected.countDown();
            if (closed.get()) new Thread(this::terminate, "qualification-cancel").start();
        }
        @Override public void onServiceDisconnected(ComponentName name) { died.countDown(); connected.countDown(); }
        @Override public void onNullBinding(ComponentName name) { connected.countDown(); }
        @Override public void onBindingDied(ComponentName name) { died.countDown(); connected.countDown(); }
        void close() { if (closed.compareAndSet(false, true)) terminate(); }
        void terminate() {
            try {
                if (remote != null) {
                    try { remote.stop(); } catch (Exception ignored) { }
                    if (!died.await(5, TimeUnit.SECONDS)) throw new IllegalStateException("Worker did not terminate");
                }
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
                throw new IllegalStateException("Worker teardown interrupted", error);
            } finally {
                if (bound) {
                    bound = false;
                    try { context.unbindService(this); } catch (IllegalArgumentException ignored) { }
                }
            }
        }
    }
}
