#!/usr/bin/env python3
"""Exercise the real GTK app with a temporary GTK test module and fake release API."""
import json
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
PROBE = r'''
#include <gtk/gtk.h>
#include <gmodule.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static int ticks, clicked, unexpected;
static GtkWidget *find(GtkWidget *w, const char *name, const char *button) {
    if (name && !strcmp(gtk_widget_get_name(w), name)) return w;
    if (button && GTK_IS_BUTTON(w) && gtk_button_get_label(GTK_BUTTON(w)) &&
        !strcmp(gtk_button_get_label(GTK_BUTTON(w)), button)) return w;
    if (GTK_IS_CONTAINER(w)) {
        GList *children = gtk_container_get_children(GTK_CONTAINER(w));
        for (GList *p = children; p; p = p->next) {
            GtkWidget *result = find(p->data, name, button);
            if (result) { g_list_free(children); return result; }
        }
        g_list_free(children);
    }
    return NULL;
}
static int has_notes(GtkWidget *w) {
    if (GTK_IS_TEXT_VIEW(w)) {
        GtkTextBuffer *b = gtk_text_view_get_buffer(GTK_TEXT_VIEW(w));
        GtkTextIter start, end;
        gtk_text_buffer_get_bounds(b, &start, &end);
        char *text = gtk_text_buffer_get_text(b, &start, &end, FALSE);
        int found = strstr(text, "Fixture changes") != NULL;
        g_free(text);
        return found;
    }
    if (GTK_IS_CONTAINER(w)) {
        GList *children = gtk_container_get_children(GTK_CONTAINER(w));
        int found = 0;
        for (GList *p = children; p && !found; p = p->next) found = has_notes(p->data);
        g_list_free(children);
        return found;
    }
    return 0;
}
static gboolean inspect(gpointer unused) {
    ticks++;
    int mode = atoi(getenv("ALOL_GUI_MODE"));
    GtkWidget *main = NULL, *dialog = NULL;
    GList *windows = gtk_window_list_toplevels();
    for (GList *p = windows; p; p = p->next) {
        GtkWidget *w = p->data;
        if (!gtk_widget_get_visible(w)) continue;
        if (GTK_IS_DIALOG(w)) dialog = w;
        else if (gtk_window_get_title(GTK_WINDOW(w)) &&
                 strstr(gtk_window_get_title(GTK_WINDOW(w)), "Arizona Launcher")) main = w;
    }
    g_list_free(windows);
    if (!main) return G_SOURCE_CONTINUE;
    if ((mode == 1 || mode == 2) && !clicked && dialog) unexpected = 1;
    if ((mode == 1 || mode == 2) && ticks == 6) {
        GtkWidget *button = find(main, NULL, "Проверить обновления");
        if (!button) button = find(main, NULL, "Обновить ALOL");
        if (button) { clicked = 1; gtk_button_clicked(GTK_BUTTON(button)); }
    }
    if (mode == 3 && ticks == 4) {
        GtkWidget *policy = find(main, "alol-update-policy", NULL);
        GtkWidget *notify = find(main, "alol-update-notifications", NULL);
        if (policy) gtk_combo_box_set_active(GTK_COMBO_BOX(policy), 2);
        if (notify) gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(notify), FALSE);
    }
    if (mode == 5 && dialog && !clicked && ticks >= 6) {
        clicked = 1;
        gtk_dialog_response(GTK_DIALOG(dialog), GTK_RESPONSE_ACCEPT);
    }
    if (ticks < 12 || ((mode == 1 || mode == 2) && ticks < 16)) return G_SOURCE_CONTINUE;
    int width = 0, height = 0;
    gtk_window_get_default_size(GTK_WINDOW(main), &width, &height);
    GdkPixbuf *icon = gtk_window_get_icon(GTK_WINDOW(main));
    FILE *report = fopen(getenv("ALOL_GUI_REPORT"), "w");
    if (report) {
        fprintf(report, "{\"popup\":%d,\"unexpected\":%d,\"clicked\":%d,\"notes\":%d,\"transient\":%d,\"icon\":%d,\"identity\":%d,\"duplicate\":%d,\"width\":%d,\"height\":%d,\"retry\":%d}",
            dialog != NULL, unexpected, clicked, dialog ? has_notes(dialog) : 0,
            dialog && gtk_window_get_transient_for(GTK_WINDOW(dialog)) == GTK_WINDOW(main),
            icon && gdk_pixbuf_get_width(icon) == 256,
            !strcmp(g_get_prgname(), "alol"), find(main, NULL, "Установить лаунчер") != NULL,
            width, height, dialog && find(dialog, NULL, "Проверить снова") != NULL);
        fclose(report);
    }
    if (dialog && getenv("ALOL_GUI_SCREENSHOT")) {
        int w = gtk_widget_get_allocated_width(dialog), h = gtk_widget_get_allocated_height(dialog);
        cairo_surface_t *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, w, h);
        cairo_t *cr = cairo_create(surface);
        gtk_widget_draw(dialog, cr);
        cairo_surface_write_to_png(surface, getenv("ALOL_GUI_SCREENSHOT"));
        cairo_destroy(cr);
        cairo_surface_destroy(surface);
    }
    if (dialog) gtk_dialog_response(GTK_DIALOG(dialog), GTK_RESPONSE_CANCEL);
    gtk_main_quit();
    return G_SOURCE_REMOVE;
}
G_MODULE_EXPORT void gtk_module_init(gint *argc, gchar ***argv) {
    g_timeout_add(100, inspect, NULL);
}
'''


@unittest.skipUnless(os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"), "Needs a graphical session")
class App(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.module_dir = tempfile.TemporaryDirectory(prefix="alol-gtk-probe-")
        cls.module = Path(cls.module_dir.name) / "probe.so"
        flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "gtk+-3.0"], text=True))
        subprocess.run(["cc", "-shared", "-fPIC", "-Wno-deprecated-declarations", "-x", "c", "-",
                        "-o", str(cls.module), *flags], input=PROBE, text=True, check=True, capture_output=True)

    @classmethod
    def tearDownClass(cls):
        cls.module_dir.cleanup()

    def scenario(self, mode, policy="startup", notify=True, cached=False):
        with tempfile.TemporaryDirectory(prefix="alol-gui-") as directory:
            root = Path(directory)
            config = root / "config/alol/settings.conf"
            config.parent.mkdir(parents=True)
            config.write_text(f"update_policy={policy}\nupdate_notifications={str(notify).lower()}\n")
            release = {"tag_name": "v99.0.0", "body": "Fixture changes\nAn isolated release preview.", "assets": []}
            metadata = root / "release.json"
            metadata.write_text(json.dumps(release))
            tools = root / "bin"
            tools.mkdir()
            curl = tools / "curl"
            curl.write_text("#!/usr/bin/python3\nimport os,pathlib,shutil,sys\na=sys.argv\n"
                            "pathlib.Path(os.environ['ALOL_GUI_REQUEST']).touch()\n"
                            "shutil.copyfile(os.environ['ALOL_GUI_RELEASE'],a[a.index('--output')+1])\n")
            curl.chmod(0o755)
            if cached:
                cache = root / "data/alol/updates/release.json"
                cache.parent.mkdir(parents=True)
                cache.write_text(json.dumps({"checked_at": int(time.time()), "release": release}))
            report = root / "report.json"
            env = dict(os.environ, XDG_CONFIG_HOME=str(root / "config"), XDG_DATA_HOME=str(root / "data"),
                       XDG_CACHE_HOME=str(root / "cache"), XDG_DATA_DIRS=str(root / "system") + ":/usr/local/share:/usr/share",
                       GTK_MODULES=str(self.module), GTK_THEME="Adwaita:dark",
                       PATH=str(tools) + os.pathsep + os.environ["PATH"], ALOL_GUI_MODE=str(mode),
                       ALOL_GUI_REPORT=str(report), ALOL_GUI_RELEASE=str(metadata), ALOL_GUI_REQUEST=str(root / "requested"))
            if os.environ.get("ALOL_GUI_PREVIEW") and mode == 0:
                env["ALOL_GUI_SCREENSHOT"] = os.environ["ALOL_GUI_PREVIEW"]
            result = subprocess.run([str(ROOT / "target/release/alol")], env=env,
                                    capture_output=True, text=True, timeout=8)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertTrue(report.is_file(), result.stderr)
            data = json.loads(report.read_text())
            self.assertEqual(data["icon"], 1)
            self.assertEqual(data["identity"], 1)
            self.assertEqual(data["duplicate"], 0)
            self.assertLessEqual(data["width"], 860)
            self.assertLessEqual(data["height"], 760)
            self.assertEqual(data["unexpected"], 0)
            if mode != 3:
                self.assertEqual(data["popup"], 1, data)
                self.assertEqual(data["notes"], 1)
                self.assertEqual(data["transient"], 1)
            self.assertEqual((root / "requested").exists(), not cached and mode != 3)
            locations = [root / "data", Path("/usr/local/share"), Path("/usr/share")]
            self.assertTrue(any((d / "applications/alol.desktop").is_file() for d in locations))
            self.assertTrue(any((d / "icons/hicolor/256x256/apps/alol.png").is_file() for d in locations))
            if mode == 3:
                saved = config.read_text()
                self.assertIn("update_policy=weekly\n", saved)
                self.assertIn("update_notifications=false\n", saved)
            if mode == 5:
                self.assertEqual(data["retry"], 1, data)

    def test_startup_shows_release_dialog(self):
        self.scenario(0)

    def test_manual_policy_waits_for_user_check(self):
        self.scenario(1, policy="manual")

    def test_disabled_popup_still_allows_manual_update_dialog(self):
        self.scenario(2, notify=False)

    def test_preferences_save_immediately(self):
        self.scenario(3, policy="manual", notify=True)

    def test_daily_policy_uses_cached_release(self):
        self.scenario(4, policy="daily", cached=True)

    def test_failed_install_keeps_dialog_open_for_retry(self):
        self.scenario(5)


if __name__ == "__main__":
    unittest.main(verbosity=2)
