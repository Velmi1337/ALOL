#!/usr/bin/env python3
"""Check computed GTK colors under light/dark system themes; needs a display."""
import ctypes as c
import os
from pathlib import Path
import re
import unittest

P = c.c_void_p


class Color(c.Structure):
    _fields_ = [(name, c.c_double) for name in ("red", "green", "blue", "alpha")]


def bind(lib, name, result, *args):
    function = getattr(lib, name)
    function.restype = result
    function.argtypes = list(args)
    return function


def luminance(color):
    values = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
              for v in (color.red, color.green, color.blue)]
    return sum(a * b for a, b in zip(values, (0.2126, 0.7152, 0.0722)))


class Theme(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.gtk = c.CDLL("libgtk-3.so.0")
        cls.gdk = c.CDLL("libgdk-3.so.0")
        cls.object = c.CDLL("libgobject-2.0.so.0")
        cls.pango = c.CDLL("libpango-1.0.so.0")
        if not bind(cls.gtk, "gtk_init_check", c.c_int, P, P)(None, None):
            raise unittest.SkipTest("Needs a graphical session")
        css = re.search(r'const CSS: &str = r#"(.*?)"#;',
                        (Path(__file__).resolve().parents[1] / "src/gui.rs").read_text(), re.S)[1].encode()
        if os.environ.get("ALOL_TEST_UI_THEME") == "light":
            source = (Path(__file__).resolve().parents[1] / "src/gui.rs").read_text()
            colors = re.search(r'const LIGHT_COLORS: &str = r#"(.*?)"#;', source, re.S)[1]
            css = (colors + "\n@define-color theme_bg_color" + css.decode().split("@define-color theme_bg_color", 1)[1]).encode()
        provider = bind(cls.gtk, "gtk_css_provider_new", P)()
        error = P()
        ok = bind(cls.gtk, "gtk_css_provider_load_from_data", c.c_int, P, c.c_char_p, c.c_ssize_t, P)(provider, css, -1, c.byref(error))
        if not ok or error.value:
            raise AssertionError("GTK rejected ALOL CSS")
        screen = bind(cls.gdk, "gdk_screen_get_default", P)()
        bind(cls.gtk, "gtk_style_context_add_provider_for_screen", None, P, P, c.c_uint)(screen, provider, 600)
        bind(cls.object, "g_object_unref", None, P)(provider)
        cls.context = staticmethod(bind(cls.gtk, "gtk_widget_get_style_context", P, P))
        cls.get_color = staticmethod(bind(cls.gtk, "gtk_style_context_get_color", None, P, c.c_uint, P))
        cls.get_background = staticmethod(bind(cls.gtk, "gtk_style_context_get_background_color", None, P, c.c_uint, P))

    def assert_readable(self, text_widget, background_widget):
        foreground, background = Color(), Color()
        self.get_color(self.context(text_widget), 0, c.byref(foreground))
        self.get_background(self.context(background_widget), 0, c.byref(background))
        self.assertGreater(background.alpha, 0.99)
        light, dark = sorted((luminance(foreground), luminance(background)), reverse=True)
        self.assertGreaterEqual((light + 0.05) / (dark + 0.05), 4.5)

    def test_popup_menu_text_has_contrast_on_its_own_css_root(self):
        menu = bind(self.gtk, "gtk_menu_new", P)()
        item = bind(self.gtk, "gtk_menu_item_new_with_label", P, c.c_char_p)("№1 · Phoenix".encode())
        bind(self.gtk, "gtk_menu_shell_append", None, P, P)(menu, item)
        label = bind(self.gtk, "gtk_bin_get_child", P, P)(item)
        self.assert_readable(label, menu)

    def test_treeview_popup_remains_readable(self):
        tree = bind(self.gtk, "gtk_tree_view_new", P)()
        self.assert_readable(tree, tree)

    def test_expander_title_is_readable(self):
        window = bind(self.gtk, "gtk_window_new", P, c.c_int)(0)
        expander = bind(self.gtk, "gtk_expander_new", P, c.c_char_p)("Установка и обслуживание".encode())
        bind(self.gtk, "gtk_container_add", None, P, P)(window, expander)
        label = bind(self.gtk, "gtk_expander_get_label_widget", P, P)(expander)
        self.assert_readable(label, window)
        bind(self.gtk, "gtk_widget_destroy", None, P)(window)

    def test_app_heading_style_does_not_enlarge_window_title(self):
        header = bind(self.gtk, "gtk_header_bar_new", P)()
        label = bind(self.gtk, "gtk_label_new", P, c.c_char_p)(b"ALOL")
        bind(self.gtk, "gtk_container_add", None, P, P)(header, label)
        bind(self.gtk, "gtk_style_context_add_class", None, P, c.c_char_p)(self.context(label), b"title")
        description = P()
        bind(self.gtk, "gtk_style_context_get", None, P, c.c_uint, c.c_char_p)(self.context(label), 0, b"font", c.byref(description), P())
        size = bind(self.pango, "pango_font_description_get_size", c.c_int, P)(description)
        self.assertLessEqual(size / 1024, 16)
        bind(self.pango, "pango_font_description_free", None, P)(description)


if __name__ == "__main__":
    unittest.main(verbosity=2)
