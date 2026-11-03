#!/usr/bin/env python3
"""Exercise the real GTK app with a temporary GTK test module and fake release API."""
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import time
import unittest
import hashlib
import zipfile

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
static int on_screen(GtkWidget *w, GtkWidget *main) {
    int x, y;
    return w && gtk_widget_get_mapped(w) && gtk_widget_translate_coordinates(w, main, 0, 0, &x, &y) &&
        y >= 0 && y + gtk_widget_get_allocated_height(w) <= gtk_widget_get_allocated_height(main);
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
    if ((mode == 6 || mode == 7 || mode == 8) && ticks == 4) {
        if (mode == 6 || mode == 8) {
            GtkWidget *notebook = find(main, "alol-notebook", NULL);
            gtk_notebook_set_current_page(GTK_NOTEBOOK(notebook), 1);
            GtkWidget *toggle = find(main, NULL, "Широкий экран");
            gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(toggle), FALSE);
            if (mode == 8) {
                gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(find(main, "alol-fps-hud", NULL)), TRUE);
                gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(find(main, "alol-game-mode", NULL)), TRUE);
                if (find(main, "alol-fps-limit", NULL) || find(main, "alol-alt-tab", NULL) || find(main, NULL, "Больше FPS") || find(main, NULL, "Окна поверх игры тормозят · окно + 60 FPS")) unexpected = 1;
            }
        } else {
            gtk_button_clicked(GTK_BUTTON(find(main, NULL, "Помощь")));
        }
    }
    if ((mode == 6 || mode == 8) && ticks == 6) {
        gtk_button_clicked(GTK_BUTTON(find(main, NULL, "Сохранить настройки")));
    }
    if (mode == 9 && ticks == 4) {
        gtk_notebook_set_current_page(GTK_NOTEBOOK(find(main, "alol-notebook", NULL)), 2);
        gtk_expander_set_expanded(GTK_EXPANDER(find(main, "alol-mod-import", NULL)), TRUE);
        gtk_entry_set_text(GTK_ENTRY(find(main, "alol-mod-file", NULL)), getenv("ALOL_GUI_MOD_ARCHIVE"));
        gtk_button_clicked(GTK_BUTTON(find(main, NULL, "Проверить")));
    }
    if (mode == 10 && ticks == 4) {
        gtk_notebook_set_current_page(GTK_NOTEBOOK(find(main, "alol-notebook", NULL)), 1);
        gtk_button_clicked(GTK_BUTTON(find(main, NULL, "Создать и выбрать ALOL-Proton")));
    }
    if (mode == 10 && ticks == 10) {
        gtk_button_clicked(GTK_BUTTON(find(main, NULL, "Сохранить настройки")));
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
        fprintf(report, "{\"popup\":%d,\"unexpected\":%d,\"clicked\":%d,\"notes\":%d,\"transient\":%d,\"icon\":%d,\"identity\":%d,\"duplicate\":%d,\"width\":%d,\"height\":%d,\"retry\":%d,\"save_visible\":%d,\"play_visible\":%d,\"help\":%d,\"page\":%d,\"mod_plan\":%d}",
            dialog != NULL, unexpected, clicked, dialog ? has_notes(dialog) : 0,
            dialog && gtk_window_get_transient_for(GTK_WINDOW(dialog)) == GTK_WINDOW(main),
            icon && gdk_pixbuf_get_width(icon) == 256,
            !strcmp(g_get_prgname(), getenv("ALOL_GUI_APP_ID")), find(main, NULL, "Установить лаунчер") != NULL,
            width, height, dialog && find(dialog, NULL, "Проверить снова") != NULL,
            on_screen(find(main, NULL, "Сохранить настройки"), main),
            on_screen(find(main, NULL, "Установить Arizona"), main),
            gtk_expander_get_expanded(GTK_EXPANDER(find(main, "alol-help", NULL))),
            gtk_notebook_get_current_page(GTK_NOTEBOOK(find(main, "alol-notebook", NULL))),
            strstr(gtk_label_get_text(GTK_LABEL(find(main, "alol-mod-preview", NULL))), "Распознано файлов: 3") != NULL);
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

    def scenario(self, mode, policy="startup", notify=True, cached=False, standalone=False, real_runtime=False):
        with tempfile.TemporaryDirectory(prefix="alol-gui-") as directory:
            root = Path(directory)
            config = root / "config/alol/settings.conf"
            config.parent.mkdir(parents=True)
            config.write_text(f"update_policy={policy}\nupdate_notifications={str(notify).lower()}\n")
            if mode == 10:
                recipe=json.loads((ROOT/'runtime/base.json').read_text())
                runtime=root/'data/alol/runtimes'/recipe['runtime_name']
                (runtime/'files/bin').mkdir(parents=True)
                for name in ('files/bin/wine','toolmanifest.vdf','compatibilitytool.vdf'):
                    (runtime/name).write_text('fixture')
                (runtime/'proton').write_bytes((ROOT/'runtime/proton_wrapper.py').read_bytes())
                (runtime/'proton.alol-upstream').write_bytes(b'fixture engine')
                marker={'recipe':recipe,'upstream_proton_sha256':hashlib.sha256(b'fixture engine').hexdigest(),
                        'tool_vdf_sha256':hashlib.sha256(b'fixture').hexdigest()}
                if real_runtime:
                    prepared=ROOT/'.dev-data/data/alol/runtimes'/recipe['runtime_name']
                    for relative in [*recipe['engine_files'],'ALOL-WINE-SOURCE.tar.gz']:
                        target=runtime/relative;target.parent.mkdir(parents=True,exist_ok=True)
                        shutil.copyfile(prepared/relative,target)
                    marker['keyboard_patch']={'protocol':'xkb-state-v1'}
                (runtime/'alol-runtime.json').write_text(json.dumps(marker))
                with config.open('a') as stream:stream.write('runner=wine\n')
            if mode == 9:
                prefix=root/'data/prefixes/wine'
                launcher=prefix/'drive_c/Arizona Games Launcher'
                game=launcher/'bin/arizona'; game.mkdir(parents=True)
                (launcher/'Arizona Games Launcher.exe').touch()
                (game/'gta_sa.exe').write_bytes(b'game fixture')
                (game/'MoonLoader.asi').write_bytes(b'loader fixture')
                with config.open('a') as stream: stream.write(f'runner=wine\nprefix_root={root}/data/prefixes\n')
                with zipfile.ZipFile(root/'mod.zip','w') as z:
                    for name in ('mod.lua','lib/helper.lua','config/options.ini'):z.writestr(name,b'-- fixture')
            installed_entry=root/'data/applications/arizona launcher.exe.desktop'
            if mode == 8:
                installed_entry.parent.mkdir(parents=True)
                installed_entry.write_text('[Desktop Entry]\nType=Application\nName=Arizona\nExec=/usr/bin/alol launch\nIcon=alol\nNoDisplay=true\nX-ALOL-WindowMetadata=true\n')
            release = {"tag_name": "v99.0.0", "body": "Fixture changes\nAn isolated release preview.", "assets": []}
            metadata = root / "release.json"
            metadata.write_text(json.dumps(release))
            tools = root / "bin"
            tools.mkdir()
            if mode == 10:
                umu=tools/'umu-run';umu.write_text('#!/bin/sh\nexit 0\n');umu.chmod(0o755)
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
                       XDG_CACHE_HOME=str(root / "cache"), ALOL_DESKTOP_DATA_HOME=str(root / "data"),
                       ALOL_GUI_APP_ID="alol" if standalone else "alol-dev", XDG_DATA_DIRS=str(root / "system") + ":/usr/local/share:/usr/share",
                       GTK_MODULES=str(self.module), GTK_THEME="Adwaita:dark",
                       PATH=str(tools) + os.pathsep + os.environ["PATH"], ALOL_GUI_MODE=str(mode),
                       ALOL_GUI_REPORT=str(report), ALOL_GUI_RELEASE=str(metadata), ALOL_GUI_REQUEST=str(root / "requested"))
            if mode == 9: env['ALOL_GUI_MOD_ARCHIVE']=str(root/'mod.zip')
            if os.environ.get("ALOL_GUI_PREVIEW") and mode == 0:
                env["ALOL_GUI_SCREENSHOT"] = os.environ["ALOL_GUI_PREVIEW"]
            executable = ROOT / "target/release/alol"
            if standalone:
                executable = root / "alol"
                shutil.copy2(ROOT / "target/release/alol", executable)
            result = subprocess.run([str(executable)], env=env,
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
            if mode not in (3, 6, 7, 8, 9, 10):
                self.assertEqual(data["popup"], 1, data)
                self.assertEqual(data["notes"], 1)
                self.assertEqual(data["transient"], 1)
            self.assertEqual((root / "requested").exists(), not cached and mode not in (3, 6, 7, 8, 9, 10))
            if mode == 10:
                saved=config.read_text()
                if real_runtime:
                    self.assertIn(f'proton={runtime}\n',saved)
                    self.assertIn('runner=proton\n',saved)
                else:
                    self.assertIn('proton=GE-Proton\n',saved)
                    self.assertIn('runner=wine\n',saved)
            if mode == 9:
                self.assertEqual(data['mod_plan'],1)
                self.assertFalse((game/'moonloader/mod.lua').exists())
            arizona_class = "steam_app_alolarizona" if standalone else "steam_app_alolarizonadev"
            metadata = (root / "data/applications" / f"{arizona_class}.desktop").read_text()
            self.assertIn("NoDisplay=true\n", metadata)
            arizona_icon = root / "data/icons/hicolor/256x256/apps/alol-arizona.png"
            self.assertIn(f"Icon={arizona_icon}\n",metadata)
            self.assertEqual(arizona_icon.read_bytes(),(ROOT/'assets/arizona.png').read_bytes())
            self.assertNotEqual(arizona_icon.read_bytes(),(ROOT/'assets/alol.png').read_bytes())
            self.assertFalse((root / "data/applications/steam_app_default.desktop").exists())
            if standalone:
                locations = [root / "data", Path("/usr/local/share"), Path("/usr/share")]
                self.assertTrue(any((d / "applications/alol.desktop").is_file() for d in locations))
                self.assertTrue(any((d / "icons/hicolor/256x256/apps/alol.png").is_file() for d in locations))
            else:
                self.assertFalse((root / "data/applications/alol.desktop").exists())
                self.assertFalse((root / "data/icons/hicolor/256x256/apps/alol.png").exists())
                entry = (root / "data/applications/alol-dev.desktop").read_text()
                self.assertIn("NoDisplay=true\n", entry)
                self.assertIn("StartupWMClass=alol-dev\n", entry)
                self.assertIn(str(ROOT / "assets/alol.png"), entry)
            if mode == 3:
                saved = config.read_text()
                self.assertIn("update_policy=weekly\n", saved)
                self.assertIn("update_notifications=false\n", saved)
            if mode in (6, 8):
                options = json.loads((root / "config/alol/game-options.json").read_text())
                self.assertFalse(options["options"]["wideScreen"])
                self.assertEqual(data["save_visible"], 1)
                if mode == 8:
                    saved=config.read_text()
                    self.assertIn('fps_hud=true\n',saved)
                    self.assertIn('game_mode=true\n',saved)
                    for removed in ('alt_tab=', 'fps_limit=', 'game_vsync='):
                        self.assertNotIn(removed,saved)
                    self.assertFalse(options['options']['shitPc'])
                    self.assertTrue(options['options']['newGraphics'])
                    self.assertTrue(options['options']['grass'])
                    self.assertIn('Exec=/usr/bin/alol launch\n',installed_entry.read_text())
                    self.assertIn(f'Icon={arizona_icon}\n',installed_entry.read_text())
            if mode == 7:
                self.assertEqual(data["help"], 1)
                self.assertEqual(data["page"], 1)
            if mode == 0:
                self.assertEqual(data["play_visible"], 1)
            if mode == 5:
                self.assertEqual(data["retry"], 1, data)

    def test_game_settings_save_from_fixed_action_bar(self):
        self.scenario(6, policy="manual")

    def test_help_opens_recovery_section(self):
        self.scenario(7, policy="manual")

    def test_fps_and_game_mode_controls_keep_graphics_and_removed_controls_absent(self):
        self.scenario(8, policy="manual")

    def test_zip_preview_shows_plan_without_installing(self):
        self.scenario(9, policy="manual")

    def test_failed_runtime_integrity_keeps_runner_and_choice_before_save(self):
        self.scenario(10, policy="manual")

    def test_verified_runtime_updates_runner_before_save(self):
        recipe=json.loads((ROOT/'runtime/base.json').read_text())
        runtime=ROOT/'.dev-data/data/alol/runtimes'/recipe['runtime_name']
        if not runtime.is_dir():self.skipTest('Optional local prepared runtime required')
        self.scenario(10,policy='manual',real_runtime=True)

    def test_startup_shows_release_dialog(self):
        self.scenario(0)

    def test_standalone_copy_keeps_desktop_integration(self):
        self.scenario(0, standalone=True)

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
