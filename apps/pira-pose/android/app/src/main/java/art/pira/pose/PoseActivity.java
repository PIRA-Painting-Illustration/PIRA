package art.pira.pose;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.provider.OpenableColumns;
import android.util.Log;

import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;

import com.google.androidgamesdk.GameActivity;

import org.json.JSONObject;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * PIRA Pose's Activity: GameActivity runs the Rust app (libpira_pose.so); this class adds the
 * document pickers, driven by small JSON files in {@code files/bridge} (see {@code src/android.rs}
 * for the protocol).
 */
public class PoseActivity extends GameActivity {
    private static final String TAG = "PIRAPose";
    private static final int REQ_OPEN = 2001;
    private static final int REQ_SAVE = 2002;
    private static final long POLL_MS = 150;

    private File bridge;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final ExecutorService io = Executors.newSingleThreadExecutor();
    /** The request a picker is open for (-1: none), and its save details. */
    private long pendingId = -1;
    private String pendingName = "";
    private String pendingData = "";

    private final Runnable poll = new Runnable() {
        @Override
        public void run() {
            try {
                checkRequest();
            } catch (Exception e) {
                Log.e(TAG, "bridge", e);
            }
            handler.postDelayed(this, POLL_MS);
        }
    };

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        bridge = new File(getFilesDir(), "bridge");
        //noinspection ResultOfMethodCallIgnored
        bridge.mkdirs();
        if (state != null) {
            pendingId = state.getLong("pendingId", -1);
            pendingName = state.getString("pendingName", "");
            pendingData = state.getString("pendingData", "");
        }
        hideSystemBars();
    }

    @Override
    protected void onResume() {
        super.onResume();
        hideSystemBars();
        handler.removeCallbacks(poll);
        handler.post(poll);
    }

    @Override
    protected void onPause() {
        handler.removeCallbacks(poll);
        super.onPause();
    }

    @Override
    protected void onSaveInstanceState(Bundle out) {
        super.onSaveInstanceState(out);
        out.putLong("pendingId", pendingId);
        out.putString("pendingName", pendingName);
        out.putString("pendingData", pendingData);
    }

    /**
     * winit's event loop can't be made twice in one process, so once the app is finished (Back),
     * end the process: the next launch starts a fresh one.
     */
    @Override
    protected void onDestroy() {
        if (isFinishing()) {
            io.shutdown();
            android.os.Process.killProcess(android.os.Process.myPid());
        }
        super.onDestroy();
    }

    private void hideSystemBars() {
        WindowInsetsControllerCompat c = WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
        c.hide(WindowInsetsCompat.Type.systemBars());
        c.setSystemBarsBehavior(WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
    }

    // ---- Document pickers -------------------------------------------------------------------

    private void checkRequest() throws Exception {
        File f = new File(bridge, "request.json");
        if (pendingId != -1 || !f.isFile()) {
            return;
        }
        JSONObject r = new JSONObject(readText(f));
        //noinspection ResultOfMethodCallIgnored
        f.delete();
        long id = r.getLong("id");
        String kind = r.getString("kind");
        if ("open".equals(kind)) {
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType(r.optString("mime", "*/*"));
            intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
            start(id, intent, REQ_OPEN);
        } else if ("save".equals(kind)) {
            pendingName = r.optString("name", "Untitled");
            pendingData = r.optString("data", "");
            String uri = r.optString("uri", "");
            if (!uri.isEmpty()) {
                // Save to the document the scene came from (or was last saved to).
                final String data = pendingData;
                final String name = pendingName;
                io.execute(() -> copyOut(id, new File(data), Uri.parse(uri), name));
                return;
            }
            Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE)
                    .setType(r.optString("mime", "application/octet-stream"))
                    .putExtra(Intent.EXTRA_TITLE, pendingName);
            start(id, intent, REQ_SAVE);
        } else {
            answerError(id, "unknown request " + kind);
        }
    }

    private void start(long id, Intent intent, int code) {
        pendingId = id;
        try {
            startActivityForResult(intent, code);
        } catch (ActivityNotFoundException e) {
            pendingId = -1;
            answerError(id, "no document picker on this device");
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != REQ_OPEN && requestCode != REQ_SAVE) {
            return;
        }
        final long id = pendingId;
        pendingId = -1;
        if (id == -1) {
            return;
        }
        final Uri uri = data == null ? null : data.getData();
        if (resultCode != Activity.RESULT_OK || uri == null) {
            answerCancelled(id);
            return;
        }
        keepAccess(uri, data.getFlags());
        if (requestCode == REQ_OPEN) {
            io.execute(() -> copyIn(id, uri));
        } else {
            final String src = pendingData;
            final String name = displayName(uri, pendingName);
            io.execute(() -> copyOut(id, new File(src), uri, name));
        }
    }

    /** Copies a picked document into the bridge inbox and answers with its path. */
    private void copyIn(long id, Uri uri) {
        String name = displayName(uri, "document");
        File dest = new File(bridge, "inbox/" + id + "-" + safeName(name));
        //noinspection ResultOfMethodCallIgnored
        dest.getParentFile().mkdirs();
        try (InputStream in = getContentResolver().openInputStream(uri); OutputStream out = new FileOutputStream(dest)) {
            if (in == null) {
                throw new IOException("the provider returned no data");
            }
            copy(in, out);
            answer(new JSONObject().put("id", id).put("path", dest.getAbsolutePath()).put("name", name).put("uri", uri.toString()));
        } catch (Exception e) {
            Log.e(TAG, "open " + uri, e);
            answerError(id, "couldn't open " + name + ": " + e.getMessage());
        }
    }

    /** Writes the file Rust prepared to the document and answers. */
    private void copyOut(long id, File src, Uri uri, String name) {
        try (InputStream in = new FileInputStream(src); OutputStream out = getContentResolver().openOutputStream(uri, "wt")) {
            if (out == null) {
                throw new IOException("the provider gave nowhere to write");
            }
            copy(in, out);
            answer(new JSONObject().put("id", id).put("name", name).put("uri", uri.toString()));
        } catch (Exception e) {
            Log.e(TAG, "save " + uri, e);
            answerError(id, "couldn't save " + name + ": " + e.getMessage());
        } finally {
            //noinspection ResultOfMethodCallIgnored
            src.delete();
        }
    }

    /** Keeps access to a picked document past this run, where the provider allows it. */
    private void keepAccess(Uri uri, int flags) {
        int keep = flags & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
        try {
            getContentResolver().takePersistableUriPermission(uri, keep);
        } catch (SecurityException e) {
            // Not persistable: the grant lasts while the app runs.
        }
    }

    private String displayName(Uri uri, String fallback) {
        try (Cursor c = getContentResolver().query(uri, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst()) {
                String n = c.getString(0);
                if (n != null && !n.isEmpty()) {
                    return n;
                }
            }
        } catch (Exception e) {
            Log.w(TAG, "display name", e);
        }
        return fallback;
    }

    private static String safeName(String name) {
        String s = name.replaceAll("[/\\\\:*?\"<>|]", "_");
        return s.isEmpty() ? "document" : s;
    }

    private void answerCancelled(long id) {
        try {
            answer(new JSONObject().put("id", id).put("cancelled", true));
        } catch (Exception e) {
            Log.e(TAG, "answer", e);
        }
    }

    private void answerError(long id, String error) {
        try {
            answer(new JSONObject().put("id", id).put("error", error));
        } catch (Exception e) {
            Log.e(TAG, "answer", e);
        }
    }

    private void answer(JSONObject a) throws IOException {
        File tmp = new File(bridge, "answer.json.tmp");
        try (OutputStream out = new FileOutputStream(tmp)) {
            out.write(a.toString().getBytes(StandardCharsets.UTF_8));
        }
        if (!tmp.renameTo(new File(bridge, "answer.json"))) {
            throw new IOException("couldn't write the answer");
        }
    }

    private static String readText(File f) throws IOException {
        try (InputStream in = new FileInputStream(f)) {
            java.io.ByteArrayOutputStream b = new java.io.ByteArrayOutputStream();
            copy(in, b);
            return b.toString("UTF-8");
        }
    }

    private static void copy(InputStream in, OutputStream out) throws IOException {
        byte[] buf = new byte[1 << 16];
        int n;
        while ((n = in.read(buf)) > 0) {
            out.write(buf, 0, n);
        }
    }
}
