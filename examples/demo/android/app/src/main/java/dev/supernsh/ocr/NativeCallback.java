package dev.supernsh.ocr;

import com.google.android.gms.tasks.OnFailureListener;
import com.google.android.gms.tasks.OnSuccessListener;

/** Turns ML Kit's task listeners into native calls (see platform/android.rs). */
public final class NativeCallback implements OnSuccessListener<Object>, OnFailureListener {
    private long pointer;

    public NativeCallback(long pointer) { this.pointer = pointer; }

    private native void succeeded(long pointer, Object text);
    private native void failed(long pointer, Exception error);

    @Override public synchronized void onSuccess(Object text) {
        if (pointer != 0) { long p = pointer; pointer = 0; succeeded(p, text); }
    }

    @Override public synchronized void onFailure(Exception error) {
        if (pointer != 0) { long p = pointer; pointer = 0; failed(p, error); }
    }
}
