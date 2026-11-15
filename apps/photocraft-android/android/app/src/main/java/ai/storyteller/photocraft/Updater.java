package ai.storyteller.photocraft;

import android.app.Activity;
import android.app.AlertDialog;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.SharedPreferences;
import android.content.pm.PackageInstaller;
import android.net.Uri;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.provider.Settings;
import android.util.Log;
import android.widget.ProgressBar;
import android.widget.Toast;

import androidx.core.content.ContextCompat;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Updates from PIRA's GitHub releases.
 *
 * <p>Releases are tagged {@code v<major>.<minor>.<patch>[-beta[.<n>]]} (the PIRA_VERSION file;
 * pre-releases included) with the APK attached. At startup (unless Help › Check for Updates at
 * Startup is off) and from Help › Check for Updates…, the newest such release is compared with
 * this build's version name ({@code 0.2.0-beta-android}); when it is newer, a dialog offers to
 * install it, wait, or skip that version. A beta comes before its release: 0.2.0-beta (beta 1) <
 * 0.2.0-beta.2 < 0.2.0. Installing downloads the APK into the cache and hands it to Android's
 * package installer, which asks the user to confirm (and, the first time, to allow PIRA to
 * install apps). Updates only install when they are signed with the same key as this build.
 *
 * <p>The repository is public, so the check needs no account or token: GitHub answers anyone,
 * up to 60 requests an hour from one network.
 */
final class Updater {
    private static final String TAG = "PIRA";
    /** The repository whose releases are the updates (never PhotoCraft's: a different app). */
    static final String REPO = "PIRA-Painting-Illustration/PIRA";
    private static final String API_HOST = "api.github.com";
    private static final String API = "https://" + API_HOST + "/repos/" + REPO + "/releases?per_page=20";
    /** Groups: 1-3 major, minor, patch; 4 "-beta…"; 5 the beta number. */
    private static final String VERSION = "(\\d+)\\.(\\d+)\\.(\\d+)(-beta(?:\\.(\\d+))?)?";
    private static final Pattern TAG_VERSION = Pattern.compile("^v" + VERSION + "$");
    private static final Pattern NAME_VERSION = Pattern.compile("^" + VERSION);
    private static final String INSTALL_ACTION = "art.pira.UPDATE_INSTALLED";

    private final Activity activity;
    private final File filesDir;
    private final Handler main = new Handler(Looper.getMainLooper());
    private final ExecutorService net = Executors.newSingleThreadExecutor();
    private boolean busy;
    private boolean receiverRegistered;

    /** A newer release: its tag, title, notes and APK. */
    private static final class Release {
        String tag;
        String title;
        String notes;
        String apkUrl;
        long apkSize;
        int[] version;
    }

    Updater(Activity activity) {
        this.activity = activity;
        this.filesDir = activity.getFilesDir();
    }

    /** Whether the startup check is on (Help › Check for Updates at Startup; Rust owns the file). */
    boolean checkAtStartup() {
        return !new File(filesDir, "update-check-off").exists();
    }

    /**
     * Look for a newer release. {@code manual}: the user asked (Help › Check for Updates…), so
     * say so when there is none or the check fails, and offer a skipped version again.
     */
    void check(boolean manual) {
        if (busy) {
            return;
        }
        busy = true;
        if (manual) {
            toast("Checking for updates…");
        }
        net.execute(() -> {
            Release r = null;
            String error = null;
            try {
                r = newest();
            } catch (Exception e) {
                Log.w(TAG, "update check", e);
                error = e.getMessage();
            }
            final Release found = r;
            final String failed = error;
            main.post(() -> {
                busy = false;
                if (activity.isFinishing()) {
                    return;
                }
                if (failed != null) {
                    if (manual) {
                        toast("Couldn't check for updates: " + failed);
                    }
                } else if (found == null) {
                    if (manual) {
                        toast("PIRA is up to date (" + currentName() + ").");
                    }
                } else if (manual || !found.tag.equals(prefs().getString("skipped", ""))) {
                    offer(found);
                }
            });
        });
    }

    // ---- Finding the release ---------------------------------------------------------------

    private String currentName() {
        try {
            String n = activity.getPackageManager().getPackageInfo(activity.getPackageName(), 0).versionName;
            return n == null ? "" : n.replace("-android", "");
        } catch (Exception e) {
            return "";
        }
    }

    /**
     * A version as {major, minor, patch, release, beta}: {@code release} is 0 for a beta and 1
     * for the release itself, so a beta sorts before its release; {@code beta} is the beta
     * number ({@code -beta} alone is beta 1). Null when {@code s} isn't a PIRA version.
     */
    static int[] parse(Pattern p, String s) {
        Matcher m = p.matcher(s == null ? "" : s);
        if (!m.find()) {
            return null;
        }
        try {
            boolean beta = m.group(4) != null;
            return new int[] {
                Integer.parseInt(m.group(1)),
                Integer.parseInt(m.group(2)),
                Integer.parseInt(m.group(3)),
                beta ? 0 : 1,
                beta ? (m.group(5) == null ? 1 : Integer.parseInt(m.group(5))) : 0,
            };
        } catch (NumberFormatException e) {
            return null;
        }
    }

    static int compare(int[] a, int[] b) {
        for (int i = 0; i < 5; i++) {
            if (a[i] != b[i]) {
                return Integer.compare(a[i], b[i]);
            }
        }
        return 0;
    }

    /** The newest release newer than this build, with an APK; null when there is none. */
    private Release newest() throws IOException, org.json.JSONException {
        int[] current = parse(NAME_VERSION, currentName());
        if (current == null) {
            throw new IOException("unknown installed version");
        }
        JSONArray releases = new JSONArray(new String(get(API), StandardCharsets.UTF_8));
        Release best = null;
        for (int i = 0; i < releases.length(); i++) {
            JSONObject rel = releases.getJSONObject(i);
            if (rel.optBoolean("draft")) {
                continue;
            }
            String tag = rel.optString("tag_name");
            int[] v = parse(TAG_VERSION, tag);
            if (v == null || compare(v, current) <= 0 || (best != null && compare(v, best.version) <= 0)) {
                continue;
            }
            JSONArray assets = rel.optJSONArray("assets");
            for (int j = 0; assets != null && j < assets.length(); j++) {
                JSONObject a = assets.getJSONObject(j);
                if (a.optString("name").endsWith(".apk")) {
                    Release r = new Release();
                    r.tag = tag;
                    r.title = rel.optString("name", tag);
                    r.notes = rel.optString("body", "");
                    // The API's asset link, which answers application/octet-stream with the file.
                    r.apkUrl = a.getString("url");
                    r.apkSize = a.optLong("size", -1);
                    r.version = v;
                    best = r;
                    break;
                }
            }
        }
        return best;
    }

    /**
     * GET over HTTPS. Redirects (GitHub sends asset downloads to its file host) are followed by
     * hand, so none of them can step down to plain HTTP.
     */
    private HttpURLConnection open(String url) throws IOException {
        boolean listing = url.equals(API);
        for (int hop = 0; hop < 5; hop++) {
            URL u = new URL(url);
            if (!"https".equals(u.getProtocol())) {
                throw new IOException("refused a download that isn't HTTPS");
            }
            HttpURLConnection c = (HttpURLConnection) u.openConnection();
            c.setConnectTimeout(15000);
            c.setReadTimeout(30000);
            c.setInstanceFollowRedirects(false);
            c.setRequestProperty("User-Agent", "PIRA-Android/" + currentName());
            c.setRequestProperty("Accept", listing ? "application/vnd.github+json" : "application/octet-stream");
            if (API_HOST.equals(u.getHost())) {
                c.setRequestProperty("X-GitHub-Api-Version", "2022-11-28");
            }
            int code = c.getResponseCode();
            if (code == 200) {
                return c;
            }
            String next = c.getHeaderField("Location");
            c.disconnect();
            if (code >= 300 && code < 400 && next != null) {
                url = new URL(u, next).toString();
                continue;
            }
            if (code == 403 || code == 429) {
                throw new IOException("GitHub is limiting requests from this network; try again in an hour");
            }
            if (code == 404 && listing) {
                throw new IOException("PIRA's releases page couldn't be found");
            }
            throw new IOException("GitHub answered " + code);
        }
        throw new IOException("too many redirects");
    }

    private byte[] get(String url) throws IOException {
        HttpURLConnection c = open(url);
        try (InputStream in = c.getInputStream()) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[16384];
            for (int n; (n = in.read(buf)) > 0; ) {
                out.write(buf, 0, n);
            }
            return out.toByteArray();
        } finally {
            c.disconnect();
        }
    }

    // ---- Asking ----------------------------------------------------------------------------

    /** The release notes' "New in this build" section (or their start), for the dialog. */
    static String summary(String notes) {
        String s = notes.replace("\r", "");
        int at = s.indexOf("## New in this build");
        if (at >= 0) {
            s = s.substring(s.indexOf('\n', at) + 1);
            int end = s.indexOf("\n## ");
            if (end >= 0) {
                s = s.substring(0, end);
            }
        }
        s = s.replaceAll("\\*\\*|`", "").replaceAll("(?m)^- ", "• ").replaceAll("\\n[ \\t]*(?![•\\n])", " ").trim();
        return s.length() > 900 ? s.substring(0, 900) + "…" : s;
    }

    private void offer(Release r) {
        String name = r.tag.substring(1);
        new AlertDialog.Builder(activity, AlertDialog.THEME_DEVICE_DEFAULT_DARK)
                .setTitle("PIRA " + name + " is available")
                .setMessage("You have " + currentName() + ".\n\n" + summary(r.notes) + "\n\nDownload and install it now? Save your work first: PIRA closes to update.")
                .setPositiveButton("Install", (d, w) -> download(r))
                .setNegativeButton("Later", null)
                .setNeutralButton("Skip This Version", (d, w) -> prefs().edit().putString("skipped", r.tag).apply())
                .show();
    }

    private SharedPreferences prefs() {
        return activity.getSharedPreferences("updates", Context.MODE_PRIVATE);
    }

    // ---- Downloading and installing --------------------------------------------------------

    private void download(Release r) {
        ProgressBar bar = new ProgressBar(activity, null, android.R.attr.progressBarStyleHorizontal);
        bar.setMax(1000);
        bar.setPadding(48, 24, 48, 0);
        final boolean[] cancelled = {false};
        AlertDialog dialog = new AlertDialog.Builder(activity, AlertDialog.THEME_DEVICE_DEFAULT_DARK)
                .setTitle("Downloading PIRA " + r.tag.substring(1))
                .setView(bar)
                .setCancelable(false)
                .setNegativeButton("Cancel", (d, w) -> cancelled[0] = true)
                .show();
        busy = true;
        net.execute(() -> {
            File apk = new File(activity.getCacheDir(), "update.apk");
            String error = null;
            try {
                HttpURLConnection c = open(r.apkUrl);
                long total = r.apkSize > 0 ? r.apkSize : c.getContentLengthLong();
                long done = 0;
                try (InputStream in = c.getInputStream(); OutputStream out = new FileOutputStream(apk)) {
                    byte[] buf = new byte[65536];
                    for (int n; (n = in.read(buf)) > 0; ) {
                        if (cancelled[0]) {
                            throw new IOException("cancelled");
                        }
                        out.write(buf, 0, n);
                        done += n;
                        if (total > 0) {
                            final int p = (int) (done * 1000 / total);
                            main.post(() -> bar.setProgress(p));
                        }
                    }
                } finally {
                    c.disconnect();
                }
                if (r.apkSize > 0 && done != r.apkSize) {
                    throw new IOException("the download was incomplete");
                }
            } catch (Exception e) {
                Log.w(TAG, "update download", e);
                error = e.getMessage();
                //noinspection ResultOfMethodCallIgnored
                apk.delete();
            }
            final String failed = error;
            main.post(() -> {
                busy = false;
                dialog.dismiss();
                if (cancelled[0]) {
                    return;
                }
                if (failed != null) {
                    toast("The update couldn't be downloaded: " + failed);
                    return;
                }
                install(apk);
            });
        });
    }

    /** The APK left by a download while the user was allowing PhotoCraft to install apps. */
    void resumePendingInstall() {
        File apk = new File(activity.getCacheDir(), "update.apk");
        if (prefs().getBoolean("awaiting-permission", false) && apk.isFile()
                && activity.getPackageManager().canRequestPackageInstalls()) {
            prefs().edit().putBoolean("awaiting-permission", false).apply();
            install(apk);
        }
    }

    private void install(File apk) {
        if (!activity.getPackageManager().canRequestPackageInstalls()) {
            // Android asks once per app: send the user to the switch, and carry on when they're back.
            prefs().edit().putBoolean("awaiting-permission", true).apply();
            toast("Allow PIRA to install apps, then come back to finish the update.");
            activity.startActivity(new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                    Uri.parse("package:" + activity.getPackageName())));
            return;
        }
        net.execute(() -> {
            try {
                PackageInstaller installer = activity.getPackageManager().getPackageInstaller();
                PackageInstaller.SessionParams params = new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
                params.setAppPackageName(activity.getPackageName());
                int id = installer.createSession(params);
                try (PackageInstaller.Session session = installer.openSession(id)) {
                    try (InputStream in = new FileInputStream(apk); OutputStream out = session.openWrite("photocraft.apk", 0, apk.length())) {
                        byte[] buf = new byte[65536];
                        for (int n; (n = in.read(buf)) > 0; ) {
                            out.write(buf, 0, n);
                        }
                        session.fsync(out);
                    }
                    main.post(this::registerReceiver);
                    Intent status = new Intent(INSTALL_ACTION).setPackage(activity.getPackageName());
                    int flags = PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= 31 ? PendingIntent.FLAG_MUTABLE : 0);
                    session.commit(PendingIntent.getBroadcast(activity, id, status, flags).getIntentSender());
                }
            } catch (Exception e) {
                Log.w(TAG, "update install", e);
                main.post(() -> toast("The update couldn't be installed: " + e.getMessage()));
            }
        });
    }

    private void registerReceiver() {
        if (receiverRegistered) {
            return;
        }
        receiverRegistered = true;
        ContextCompat.registerReceiver(activity.getApplicationContext(), new BroadcastReceiver() {
            @Override
            public void onReceive(Context context, Intent intent) {
                int status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE);
                if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
                    Intent confirm = intent.getParcelableExtra(Intent.EXTRA_INTENT);
                    if (confirm != null) {
                        confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                        context.startActivity(confirm);
                    }
                } else if (status != PackageInstaller.STATUS_SUCCESS) {
                    String msg = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE);
                    Log.w(TAG, "update install: " + status + " " + msg);
                    if (status != PackageInstaller.STATUS_FAILURE_ABORTED) {
                        Toast.makeText(context, status == PackageInstaller.STATUS_FAILURE_CONFLICT || status == PackageInstaller.STATUS_FAILURE_INCOMPATIBLE
                                ? "The update is signed with a different key than this copy of PIRA: uninstall it and install the update by hand."
                                : "The update couldn't be installed" + (msg == null ? "." : ": " + msg), Toast.LENGTH_LONG).show();
                    }
                }
            }
        }, new IntentFilter(INSTALL_ACTION), ContextCompat.RECEIVER_NOT_EXPORTED);
    }

    private void toast(String text) {
        Toast.makeText(activity, text, Toast.LENGTH_LONG).show();
    }
}
