//! Small GTK 3 ABI surface. Widgets and signal data live on GTK's main thread.
//! Native widgets, no webview, Cargo dependencies, or bundled toolkit.
use crate::core::{self, Result, Settings};
use crate::desktop;
use crate::game::{self, PlayerProfile, Server};
use crate::game_options::{self, GameOptions};
use crate::logs;
use crate::mods::{self, CatalogMod, InstalledMod};
use crate::updates::{self, Release};
use std::{
    cell::Cell,
    ffi::{CStr, CString, c_char, c_int, c_uint, c_void},
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
    ptr,
    sync::mpsc::{self, Receiver},
};

type W = *mut c_void;
type Callback = Option<unsafe extern "C" fn()>;
#[link(name = "gtk-3")]
unsafe extern "C" {
    fn gtk_init_check(argc: *mut c_int, argv: *mut *mut *mut c_char) -> c_int;
    fn gtk_settings_get_default() -> W;
    fn gtk_main();
    fn gtk_main_quit();
    fn gtk_window_new(kind: c_int) -> W;
    fn gtk_window_set_title(window: W, title: *const c_char);
    fn gtk_window_set_default_size(window: W, width: c_int, height: c_int);
    fn gtk_window_set_position(window: W, position: c_int);
    fn gtk_window_set_icon_name(window: W, name: *const c_char);
    fn gtk_window_set_icon(window: W, pixbuf: W);
    fn gtk_window_set_default_icon(pixbuf: W);
    fn gtk_window_set_transient_for(window: W, parent: W);
    fn gtk_window_set_modal(window: W, modal: c_int);
    fn gtk_window_set_destroy_with_parent(window: W, destroy: c_int);
    fn gtk_window_set_deletable(window: W, deletable: c_int);
    fn gtk_accel_group_new() -> W;
    fn gtk_window_add_accel_group(window: W, group: W);
    fn gtk_widget_add_accelerator(
        widget: W,
        signal: *const c_char,
        group: W,
        key: c_uint,
        modifiers: c_uint,
        flags: c_uint,
    );
    fn gtk_box_new(orientation: c_int, spacing: c_int) -> W;
    fn gtk_box_set_homogeneous(container: W, homogeneous: c_int);
    fn gtk_box_pack_start(container: W, child: W, expand: c_int, fill: c_int, padding: c_uint);
    fn gtk_box_reorder_child(container: W, child: W, position: c_int);
    fn gtk_orientable_set_orientation(orientable: W, orientation: c_int);
    fn gtk_container_add(container: W, child: W);
    fn gtk_container_child_set(container: W, child: W, first_property: *const c_char, ...);
    fn gtk_container_set_border_width(container: W, width: c_uint);
    fn gtk_widget_show_all(widget: W);
    fn gtk_widget_destroy(widget: W);
    fn gtk_widget_set_name(widget: W, name: *const c_char);
    fn gtk_widget_set_sensitive(widget: W, sensitive: c_int);
    fn gtk_widget_set_visible(widget: W, visible: c_int);
    fn gtk_widget_set_no_show_all(widget: W, no_show: c_int);
    fn gtk_widget_grab_focus(widget: W);
    fn gtk_widget_set_tooltip_text(widget: W, text: *const c_char);
    fn gtk_widget_set_size_request(widget: W, width: c_int, height: c_int);
    fn gtk_widget_set_hexpand(widget: W, expand: c_int);
    fn gtk_widget_set_valign(widget: W, align: c_int);
    fn gtk_widget_get_allocated_width(widget: W) -> c_int;
    fn gtk_widget_get_allocated_height(widget: W) -> c_int;
    fn gtk_widget_draw(widget: W, cairo: W);
    fn gtk_widget_get_style_context(widget: W) -> W;
    fn gtk_style_context_add_class(context: W, class: *const c_char);
    fn gtk_style_context_remove_class(context: W, class: *const c_char);
    fn gtk_style_context_has_class(context: W, class: *const c_char) -> c_int;
    fn gtk_size_group_new(mode: c_int) -> W;
    fn gtk_size_group_add_widget(group: W, widget: W);
    fn gtk_label_new(text: *const c_char) -> W;
    fn gtk_label_set_text(label: W, text: *const c_char);
    fn gtk_label_set_xalign(label: W, align: f32);
    fn gtk_label_set_line_wrap(label: W, wrap: c_int);
    fn gtk_label_set_max_width_chars(label: W, width: c_int);
    fn gtk_label_set_ellipsize(label: W, mode: c_int);
    fn gtk_image_new_from_pixbuf(pixbuf: W) -> W;
    fn gtk_button_new_with_label(label: *const c_char) -> W;
    fn gtk_button_set_label(button: W, label: *const c_char);
    fn gtk_button_set_image(button: W, image: W);
    fn gtk_button_set_always_show_image(button: W, show: c_int);
    fn gtk_button_set_image_position(button: W, position: c_int);
    fn gtk_entry_new() -> W;
    fn gtk_entry_set_text(entry: W, text: *const c_char);
    fn gtk_entry_get_text(entry: W) -> *const c_char;
    fn gtk_entry_set_placeholder_text(entry: W, text: *const c_char);
    fn gtk_switch_new() -> W;
    fn gtk_switch_get_active(switch: W) -> c_int;
    fn gtk_switch_set_active(switch: W, active: c_int);
    fn gtk_widget_get_parent(widget: W) -> W;
    fn gtk_widget_get_accessible(widget: W) -> W;
    fn gtk_combo_box_text_new() -> W;
    fn gtk_combo_box_text_new_with_entry() -> W;
    fn gtk_combo_box_text_append_text(combo: W, text: *const c_char);
    fn gtk_combo_box_text_remove_all(combo: W);
    fn gtk_list_box_new() -> W;
    fn gtk_list_box_insert(list: W, child: W, position: c_int);
    fn gtk_list_box_get_selected_row(list: W) -> W;
    fn gtk_list_box_get_row_at_index(list: W, index: c_int) -> W;
    fn gtk_list_box_row_get_index(row: W) -> c_int;
    fn gtk_list_box_select_row(list: W, row: W);
    fn gtk_combo_box_set_active(combo: W, index: c_int);
    fn gtk_combo_box_get_active(combo: W) -> c_int;
    fn gtk_bin_get_child(bin: W) -> W;
    fn gtk_notebook_new() -> W;
    fn gtk_notebook_append_page(notebook: W, child: W, label: W) -> c_int;
    fn gtk_notebook_set_current_page(notebook: W, page: c_int);
    fn gtk_notebook_set_show_tabs(notebook: W, show: c_int);
    fn gtk_notebook_set_show_border(notebook: W, show: c_int);
    fn gtk_scrolled_window_new(hadjustment: W, vadjustment: W) -> W;
    fn gtk_scrolled_window_set_policy(window: W, horizontal: c_int, vertical: c_int);
    fn gtk_scrolled_window_get_vadjustment(window: W) -> W;
    fn gtk_adjustment_get_value(adjustment: W) -> f64;
    fn gtk_adjustment_get_upper(adjustment: W) -> f64;
    fn gtk_adjustment_get_page_size(adjustment: W) -> f64;
    fn gtk_adjustment_set_value(adjustment: W, value: f64);
    fn gtk_text_view_new() -> W;
    fn gtk_text_view_get_buffer(view: W) -> W;
    fn gtk_text_view_set_editable(view: W, editable: c_int);
    fn gtk_text_view_set_cursor_visible(view: W, visible: c_int);
    fn gtk_text_view_set_monospace(view: W, monospace: c_int);
    fn gtk_text_view_set_wrap_mode(view: W, mode: c_int);
    fn gtk_text_buffer_set_text(buffer: W, text: *const c_char, length: c_int);
    fn gtk_spinner_new() -> W;
    fn gtk_spinner_start(spinner: W);
    fn gtk_spinner_stop(spinner: W);
    fn gtk_expander_new(label: *const c_char) -> W;
    fn gtk_expander_set_expanded(expander: W, expanded: c_int);
    fn gtk_css_provider_new() -> W;
    fn gtk_css_provider_load_from_data(
        provider: W,
        data: *const c_char,
        length: isize,
        error: *mut W,
    ) -> c_int;
    fn gtk_style_context_add_provider_for_screen(screen: W, provider: W, priority: c_uint);
    fn gtk_file_chooser_dialog_new(
        title: *const c_char,
        parent: W,
        action: c_int,
        first: *const c_char,
        ...
    ) -> W;
    fn gtk_file_chooser_get_filename(chooser: W) -> *mut c_char;
    fn gtk_dialog_run(dialog: W) -> c_int;
    fn gtk_dialog_add_button(dialog: W, text: *const c_char, response: c_int) -> W;
    fn gtk_dialog_new() -> W;
    fn gtk_dialog_get_content_area(dialog: W) -> W;
    fn gtk_dialog_get_widget_for_response(dialog: W, response: c_int) -> W;
    fn gtk_dialog_set_response_sensitive(dialog: W, response: c_int, sensitive: c_int);
}
#[link(name = "gdk-3")]
unsafe extern "C" {
    fn gdk_screen_get_default() -> W;
    fn gdk_set_program_class(class: *const c_char);
    fn gdk_atom_intern_static_string(name: *const c_char) -> W;
    fn gdk_display_get_default() -> W;
    fn gdk_display_get_primary_monitor(display: W) -> W;
    fn gdk_display_get_monitor(display: W, index: c_int) -> W;
    fn gdk_monitor_get_workarea(monitor: W, rectangle: *mut Rectangle);
}
#[link(name = "atk-1.0")]
unsafe extern "C" {
    fn atk_object_set_name(object: W, name: *const c_char);
}
#[link(name = "fontconfig")]
unsafe extern "C" {
    fn FcConfigAppFontAddFile(config: W, file: *const u8) -> c_int;
}
#[repr(C)]
#[derive(Default)]
struct Rectangle {
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
}
#[link(name = "gdk_pixbuf-2.0")]
unsafe extern "C" {
    fn gdk_pixbuf_loader_new_with_type(format: *const c_char, error: *mut W) -> W;
    fn gdk_pixbuf_loader_write(loader: W, bytes: *const u8, count: usize, error: *mut W) -> c_int;
    fn gdk_pixbuf_loader_close(loader: W, error: *mut W) -> c_int;
    fn gdk_pixbuf_loader_get_pixbuf(loader: W) -> W;
    fn gdk_pixbuf_scale_simple(pixbuf: W, width: c_int, height: c_int, interpolation: c_int) -> W;
}
#[link(name = "gtk-3")]
unsafe extern "C" {
    fn gtk_clipboard_get(selection: W) -> W;
    fn gtk_clipboard_set_text(clipboard: W, text: *const c_char, length: c_int);
}
#[link(name = "cairo")]
unsafe extern "C" {
    fn cairo_image_surface_create(format: c_int, width: c_int, height: c_int) -> W;
    fn cairo_create(surface: W) -> W;
    fn cairo_destroy(cairo: W);
    fn cairo_status(cairo: W) -> c_int;
    fn cairo_surface_destroy(surface: W);
    fn cairo_surface_write_to_png(surface: W, path: *const c_char) -> c_int;
}
#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn g_signal_connect_data(
        instance: W,
        signal: *const c_char,
        callback: Callback,
        data: W,
        destroy: W,
        flags: c_uint,
    ) -> u64;
    fn g_object_unref(object: W);
    fn g_object_set(object: W, first_property: *const c_char, ...);
    fn g_signal_handler_block(instance: W, handler: u64);
    fn g_signal_handler_unblock(instance: W, handler: u64);
}
#[link(name = "glib-2.0")]
unsafe extern "C" {
    fn g_timeout_add(
        interval: c_uint,
        callback: Option<unsafe extern "C" fn(W) -> c_int>,
        data: W,
    ) -> c_uint;
    fn g_source_remove(tag: c_uint) -> c_int;
    fn g_free(pointer: W);
    fn g_get_monotonic_time() -> i64;
    fn g_set_prgname(name: *const c_char);
    fn g_set_application_name(name: *const c_char);
}
unsafe extern "C" {
    fn kill(pid: c_int, signal: c_int) -> c_int;
}

fn c(text: &str) -> CString {
    CString::new(text.replace('\0', "")).expect("NUL removed")
}
fn label(text: &str, class: &str) -> W {
    unsafe {
        let w = gtk_label_new(c(text).as_ptr());
        gtk_label_set_xalign(w, 0.0);
        gtk_label_set_line_wrap(w, 1);
        style(w, class);
        w
    }
}
fn style(w: W, class: &str) {
    unsafe {
        gtk_style_context_add_class(gtk_widget_get_style_context(w), c(class).as_ptr());
    }
}
fn pack(container: W, child: W, expand: bool) {
    unsafe {
        gtk_box_pack_start(container, child, expand.into(), 1, 0);
    }
}
fn box_new(vertical: bool, spacing: i32) -> W {
    unsafe { gtk_box_new(vertical.into(), spacing) }
}
fn settings_group(parent: W, title: &str, description: &str) -> W {
    let group = box_new(true, 8);
    style(group, "settings-group");
    pack(group, label(title, "group-title"), false);
    if !description.is_empty() {
        pack(group, label(description, "caption"), false);
    }
    let body = box_new(true, 4);
    style(body, "settings-surface");
    pack(group, body, false);
    pack(parent, group, false);
    body
}
fn mod_row(name: &str, version: &str, description: &str, state: Option<&str>) -> W {
    let row = box_new(true, 6);
    style(row, "mod-row");
    let heading = box_new(false, 12);
    pack(
        heading,
        label(&format!("{name} {version}"), "mod-title"),
        true,
    );
    if let Some(state) = state {
        pack(heading, label(state, "status-chip"), false);
    }
    pack(row, heading, false);
    if !description.is_empty() {
        let text = description
            .chars()
            .take(250)
            .collect::<String>()
            .replace('\n', " ");
        pack(row, label(&text, "caption"), false);
        unsafe {
            gtk_widget_set_tooltip_text(row, c(description).as_ptr());
        }
    }
    row
}
fn disclosure(parent: W, title: &str) -> (W, W) {
    unsafe {
        let expander = gtk_expander_new(c(title).as_ptr());
        let body = box_new(true, 10);
        gtk_expander_set_expanded(expander, 0);
        gtk_container_add(expander, body);
        pack(parent, expander, false);
        (expander, body)
    }
}
/// Equal columns for action buttons, without stretching input fields or labels.
fn action_row() -> W {
    let row = box_new(false, 10);
    unsafe {
        gtk_box_set_homogeneous(row, 1);
    }
    style(row, "actions");
    row
}
fn notebook_page(notebook: W, page: W, title: &str, tabs: W) {
    let tab = label(title, "tab-label");
    unsafe {
        gtk_label_set_xalign(tab, 0.5);
        gtk_size_group_add_widget(tabs, tab);
        gtk_notebook_append_page(notebook, page, tab);
        gtk_container_child_set(
            notebook,
            page,
            c"tab-expand".as_ptr(),
            1 as c_int,
            c"tab-fill".as_ptr(),
            1 as c_int,
            ptr::null::<c_char>(),
        );
    }
}
fn set_label(w: W, text: &str) {
    unsafe {
        gtk_label_set_text(w, c(text).as_ptr());
    }
}
fn entry(text: &str) -> W {
    unsafe {
        let w = gtk_entry_new();
        gtk_entry_set_text(w, c(text).as_ptr());
        w
    }
}
fn entry_text(w: W) -> String {
    unsafe {
        CStr::from_ptr(gtk_entry_get_text(w))
            .to_string_lossy()
            .trim()
            .into()
    }
}
fn check(parent: W, text: &str, active: bool) -> W {
    unsafe {
        let row = box_new(false, 20);
        style(row, "preference-row");
        let title = label(text, "preference-title");
        pack(row, title, true);
        let w = gtk_switch_new();
        gtk_switch_set_active(w, active.into());
        gtk_widget_set_valign(w, 3);
        gtk_widget_set_size_request(w, 52, 32);
        atk_object_set_name(gtk_widget_get_accessible(w), c(text).as_ptr());
        pack(row, w, false);
        pack(parent, row, false);
        w
    }
}
fn checked(w: W) -> bool {
    unsafe { gtk_switch_get_active(w) != 0 }
}

#[derive(Clone, Copy)]
enum Action {
    About,
    ExternalLink(&'static str),
    Navigate(c_int),
    Project(crate::project::Project),
    Work(&'static str),
    Play,
    Save,
    Folder,
    Stop,
    Browse(W, bool),
    ChooseInstaller,
    Website,
    ModFilter,
    ModSelection,
    ModInstall,
    ModFile,
    ModImport,
    ModPreview,
    RuntimePrepare,
    ModManage(&'static str),
    PlayerChanged,
    Update,
    UpdatePreferences,
    Appearance,
    QuickPreferences,
    SettingsChanged,
    Help,
    CopyLog,
    LogFilter,
    LogPause,
    LogClear,
    LogFolder,
}
struct Signal {
    app: *mut App,
    action: Action,
}
struct Fields {
    ui_theme: W,
    runner: W,
    proton: W,
    wine: W,
    installer: W,
    executable: W,
    prefix: W,
    wined3d: W,
    disable_gpu: W,
    discrete_gpu: W,
    debug: W,
    no_sync: W,
    launcher_compatibility: W,
    input_compatibility: W,
    fps_hud: W,
    game_mode: W,
    update_policy: W,
    update_notifications: W,
}
struct App {
    about_dialog: W,
    theme_provider: W,
    chrome: Box<Chrome>,
    settings: Settings,
    project_buttons: Vec<(crate::project::Project, W)>,
    catalog_title: W,
    window: W,
    notebook: W,
    status: W,
    runtime: W,
    hero_title: W,
    hero: W,
    hero_step: W,
    help_expander: W,
    settings_hint: W,
    settings_save_bar: W,
    detail: W,
    play: W,
    update_game: W,
    game_folder: W,
    spinner: W,
    stop: W,
    settings_page: W,
    log: LogPanel,
    settings_book: W,
    fields: Fields,
    buttons: Vec<W>,
    utility_buttons: W,
    signals: Vec<SignalHolder>,
    child: Option<Child>,
    action: String,
    stop_at: Option<i64>,
    mods_page: W,
    mod_list: W,
    mod_search: W,
    mod_file: W,
    mod_target: W,
    mod_preview: W,
    installed_list: W,
    mod_book: W,
    mod_install: W,
    mod_selection: W,
    installed_count: W,
    mod_manage: Vec<(&'static str, W)>,
    mod_count: W,
    catalog_items: Vec<CatalogMod>,
    installed_items: Vec<InstalledMod>,
    player: PlayerProfile,
    server_rows: Vec<Server>,
    server_combo: W,
    nickname_entry: W,
    nickname_controls: W,
    player_title: W,
    game_memory: W,
    game_checks: Vec<(&'static str, W)>,
    player_controls: W,
    player_hint: W,
    player_dirty: bool,
    player_changed_at: i64,
    updating_player: bool,
    player_signal: u64,
    capture: Option<Capture>,
    update_label: W,
    update_button: W,
    update_receiver: Option<Receiver<Result<Option<Release>>>>,
    update_release: Option<Release>,
    update_installed: bool,
    executable: std::path::PathBuf,
    update_dialog: W,
    update_dialog_status: W,
    update_prompt_pending: bool,
    update_manual: bool,
}
struct Capture {
    output: std::path::PathBuf,
    page: usize,
    error: Option<String>,
}
// Heap allocations keep signal pointers stable as the vector grows.
struct SignalHolder(Box<Signal>);

struct LogPanel {
    buffer: W,
    search: W,
    level: W,
    session: W,
    follow: W,
    summary: W,
    pause_button: W,
    adjustment: W,
    source: String,
    paused: bool,
    scroll_to: Option<f64>,
    reader: core::LogTail,
}
impl LogPanel {
    fn visible(&self) -> logs::View {
        logs::view(
            &self.source,
            &entry_text(self.search),
            unsafe { gtk_combo_box_get_active(self.level) },
            unsafe { gtk_combo_box_get_active(self.session) } == 0,
        )
    }
    fn render(&mut self) {
        let view = self.visible();
        unsafe {
            self.scroll_to = Some(if checked(self.follow) {
                f64::INFINITY
            } else {
                gtk_adjustment_get_value(self.adjustment)
            });
            gtk_text_buffer_set_text(self.buffer, c(&view.text).as_ptr(), -1);
        }
        let state = if self.paused {
            " · На паузе"
        } else {
            ""
        };
        let bounded = if core::log_file()
            .metadata()
            .is_ok_and(|m| m.len() > 48 * 1024)
        {
            " · Показан конец файла"
        } else {
            ""
        };
        set_label(
            self.summary,
            &if view.total == 0 {
                format!("Журнал пуст. Здесь появятся результаты операций.{state}")
            } else {
                format!(
                    "Строк: {} / {} · Ошибки: {} · Предупреждения: {}{state}{bounded}",
                    view.shown, view.total, view.errors, view.warnings
                )
            },
        );
    }
    fn poll(&mut self) {
        if let Some(target) = self.scroll_to.take() {
            unsafe {
                let bottom = (gtk_adjustment_get_upper(self.adjustment)
                    - gtk_adjustment_get_page_size(self.adjustment))
                .max(0.0);
                gtk_adjustment_set_value(self.adjustment, target.min(bottom));
            }
        }
        if !self.paused
            && let Some(raw) = self.reader.changed()
        {
            self.source = logs::normalize(&raw);
            self.render();
        }
    }
}

/// Stable signal data for navigation and adaptive layout, independent of workers.
struct Chrome {
    window: W,
    shell: W,
    rail: W,
    rail_brand: W,
    rail_meta: W,
    items: W,
    title: W,
    subtitle: W,
    buttons: Vec<W>,
    compact: Cell<bool>,
}
impl Chrome {
    fn select(&self, page: usize) {
        let titles = ["Играть", "Настройки", "Моды", "Журнал"];
        set_label(self.title, titles.get(page).unwrap_or(&"ALOL"));
        for (index, button) in self.buttons.iter().enumerate() {
            unsafe {
                let context = gtk_widget_get_style_context(*button);
                if index == page {
                    gtk_style_context_add_class(context, c"nav-selected".as_ptr());
                } else {
                    gtk_style_context_remove_class(context, c"nav-selected".as_ptr());
                }
            }
        }
    }
    fn resize(&self, width: i32) {
        let compact = width < 840;
        if self.compact.replace(compact) == compact {
            return;
        }
        unsafe {
            gtk_orientable_set_orientation(self.shell, compact.into());
            gtk_orientable_set_orientation(self.items, (!compact).into());
            gtk_box_set_homogeneous(self.items, compact.into());
            gtk_widget_set_size_request(self.rail, if compact { -1 } else { 96 }, -1);
            gtk_widget_set_visible(self.rail_brand, (!compact).into());
            gtk_widget_set_visible(self.rail_meta, (!compact).into());
            let context = gtk_widget_get_style_context(self.window);
            if compact {
                gtk_style_context_add_class(context, c"compact".as_ptr());
            } else {
                gtk_style_context_remove_class(context, c"compact".as_ptr());
            }
        }
    }
}
unsafe extern "C" fn navigation_changed(_: W, _: W, page: c_uint, data: W) {
    unsafe { (&*data.cast::<Chrome>()).select(page as usize) };
}
unsafe extern "C" fn layout_changed(_: W, allocation: *const Rectangle, data: W) {
    unsafe { (&*data.cast::<Chrome>()).resize((*allocation).width) };
}

impl App {
    fn show_about(&mut self) {
        if !self.about_dialog.is_null() {
            return;
        }
        unsafe {
            let dialog = gtk_dialog_new();
            self.about_dialog = dialog;
            gtk_widget_set_name(dialog, c"alol-about".as_ptr());
            gtk_window_set_title(dialog, c"О проекте ALOL".as_ptr());
            gtk_window_set_transient_for(dialog, self.window);
            gtk_window_set_destroy_with_parent(dialog, 1);
            gtk_window_set_modal(dialog, 1);
            let (width, height) = fit_window_size(520, 440);
            gtk_window_set_default_size(dialog, width, height);
            let content = gtk_dialog_get_content_area(dialog);
            gtk_container_set_border_width(content, 24);
            let identity = box_new(false, 16);
            pack(identity, logo(56), false);
            let heading = box_new(true, 4);
            pack(
                heading,
                label(&format!("ALOL {}", env!("CARGO_PKG_VERSION")), "page-title"),
                false,
            );
            pack(
                heading,
                label("Arizona RP и Родина RP на Linux", "caption"),
                false,
            );
            pack(identity, heading, true);
            pack(content, identity, false);
            pack(
                content,
                label(
                    "Автор проекта — Velmi1337. Исходники, другие проекты и новости ALOL:",
                    "body",
                ),
                false,
            );
            let links = box_new(true, 8);
            style(links, "about-links");
            for (title, url) in [
                ("Velmi1337 · GitHub", "https://github.com/Velmi1337"),
                ("Сайт автора", "https://velmi1337.github.io/"),
                ("Новости ALOL · @alolinux", "https://t.me/alolinux"),
            ] {
                let button = self.button(links, title, Action::ExternalLink(url), false);
                gtk_widget_set_tooltip_text(button, c(url).as_ptr());
                gtk_button_set_image(
                    button,
                    symbol(
                        "external",
                        if self.settings.ui_theme == "light" {
                            "#444746"
                        } else {
                            "#c4c7c5"
                        },
                        20,
                    ),
                );
                gtk_button_set_image_position(button, 1);
                gtk_button_set_always_show_image(button, 1);
            }
            pack(content, links, false);
            pack(
                content,
                label("Код — Apache-2.0 · Google Sans — OFL-1.1", "caption"),
                false,
            );
            gtk_dialog_add_button(dialog, c"Закрыть".as_ptr(), -7);
            g_signal_connect_data(
                dialog,
                c"response".as_ptr(),
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(W, c_int, W),
                    unsafe extern "C" fn(),
                >(about_response)),
                (self as *mut App).cast(),
                ptr::null_mut(),
                0,
            );
            gtk_widget_show_all(dialog);
        }
    }
    fn apply_appearance(&self) {
        let light = self.settings.ui_theme == "light";
        let ink = if light { "#444746" } else { "#c4c7c5" };
        unsafe {
            gtk_css_provider_load_from_data(
                self.theme_provider,
                c(&theme_css(light)).as_ptr(),
                -1,
                ptr::null_mut(),
            );
            g_object_set(
                gtk_settings_get_default(),
                c"gtk-application-prefer-dark-theme".as_ptr(),
                (!light) as c_int,
                ptr::null::<c_char>(),
            );
            for (button, icon) in self
                .chrome
                .buttons
                .iter()
                .zip(["play", "tune", "grid", "notes"])
            {
                gtk_button_set_image(*button, symbol(icon, ink, 24));
            }
            for (project, button) in &self.project_buttons {
                gtk_button_set_image(
                    *button,
                    symbol(
                        if *project == crate::project::Project::Arizona {
                            "sun"
                        } else {
                            "home"
                        },
                        ink,
                        22,
                    ),
                );
            }
        }
    }
    fn load_project_controls(&mut self) -> Result<()> {
        let options = GameOptions::load(&self.settings)?;
        self.updating_player = true;
        unsafe {
            gtk_combo_box_set_active(
                self.game_memory,
                [1024, 2048, 3072, 4096]
                    .iter()
                    .position(|m| *m == options.memory)
                    .unwrap_or(1) as i32,
            );
            for (id, widget) in &self.game_checks {
                let available = game_options::OPTIONS
                    .iter()
                    .any(|o| o.id == *id && o.supports(self.settings.project));
                gtk_switch_set_active(*widget, options.enabled(id).into());
                let row = gtk_widget_get_parent(*widget);
                gtk_widget_set_no_show_all(row, (!available).into());
                gtk_widget_set_visible(row, available.into());
                gtk_widget_set_no_show_all(*widget, (!available).into());
                gtk_widget_set_visible(*widget, available.into());
            }
            for (project, widget) in &self.project_buttons {
                let context = gtk_widget_get_style_context(*widget);
                if *project == self.settings.project {
                    gtk_style_context_add_class(context, c"project-selected".as_ptr());
                } else {
                    gtk_style_context_remove_class(context, c"project-selected".as_ptr());
                }
            }
            gtk_entry_set_text(self.nickname_entry, c(&self.player.nickname).as_ptr());
        }
        self.updating_player = false;
        let arizona = self.settings.project == crate::project::Project::Arizona;
        unsafe {
            gtk_widget_set_no_show_all(self.nickname_controls, (!arizona).into());
            gtk_widget_set_visible(self.nickname_controls, arizona.into());
        }
        set_label(
            self.player_title,
            if arizona {
                "Сервер и персонаж"
            } else {
                "Выбери сервер"
            },
        );
        set_label(
            self.chrome.subtitle,
            &format!("{} · На Linux", self.settings.project.name()),
        );
        self.update_player_hint();
        if !self.catalog_title.is_null() {
            set_label(
                self.catalog_title,
                &format!("Каталог · {}", self.settings.project.name()),
            );
        }
        Ok(())
    }

    fn switch_project(&mut self, project: crate::project::Project) -> Result<()> {
        if project == self.settings.project {
            return Ok(());
        }
        if self.child.is_some() {
            return Err("Сначала завершите игру или операцию лаунчера".into());
        }
        self.save()?;
        let mut next = self.settings.clone();
        next.project = project;
        let player = PlayerProfile::load(&next)?;
        GameOptions::load(&next)?;
        game::servers(project)?;
        next.save()?;
        self.settings = next;
        self.player = player;
        self.player_dirty = false;
        self.reload_server_choices()?;
        self.load_project_controls()?;
        unsafe {
            gtk_entry_set_text(self.mod_file, c"".as_ptr());
            gtk_entry_set_text(self.mod_search, c"".as_ptr());
        }
        set_label(
            self.mod_preview,
            "Выберите архив для выбранной игры и проверьте план установки.",
        );
        self.reload_mods();
        self.refresh();
        set_label(
            self.settings_hint,
            &format!(
                "Выбрана {}. Её настройки сохраняются отдельно.",
                project.name()
            ),
        );
        Ok(())
    }
    fn button(&mut self, parent: W, title: &str, action: Action, primary: bool) -> W {
        unsafe {
            let button = gtk_button_new_with_label(c(title).as_ptr());
            if primary {
                style(button, "primary");
            }
            gtk_widget_set_valign(button, 3); // GTK_ALIGN_CENTER
            gtk_widget_set_size_request(button, -1, 48);
            let equal_columns = gtk_style_context_has_class(
                gtk_widget_get_style_context(parent),
                c"actions".as_ptr(),
            ) != 0;
            if !equal_columns
                && matches!(
                    action,
                    Action::Browse(_, _)
                        | Action::ModFilter
                        | Action::ModFile
                        | Action::ModImport
                        | Action::Work("servers-refresh" | "mods-refresh")
                        | Action::Stop
                )
            {
                gtk_widget_set_size_request(button, 156, 44);
                gtk_size_group_add_widget(self.utility_buttons, button);
            }
            let mut signal = SignalHolder(Box::new(Signal { app: self, action }));
            let pointer: W = (&mut *signal.0 as *mut Signal).cast();
            g_signal_connect_data(
                button,
                c"clicked".as_ptr(),
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(W, W),
                    unsafe extern "C" fn(),
                >(clicked)),
                pointer,
                ptr::null_mut(),
                0,
            );
            self.signals.push(signal);
            pack(parent, button, equal_columns);
            if matches!(
                action,
                Action::Work(_)
                    | Action::Project(_)
                    | Action::Play
                    | Action::Save
                    | Action::ChooseInstaller
                    | Action::Update
            ) {
                self.buttons.push(button);
            }
            button
        }
    }

    fn file_field(&mut self, parent: W, title: &str, value: &str, folder: bool) -> W {
        pack(parent, label(title, "field-label"), false);
        let row = box_new(false, 10);
        let field = entry(value);
        unsafe {
            gtk_widget_set_hexpand(field, 1);
        }
        pack(row, field, true);
        self.button(row, "Обзор…", Action::Browse(field, folder), false);
        pack(parent, row, false);
        field
    }

    fn collect(&self) -> Result<Settings> {
        let f = &self.fields;
        let s = Settings {
            project: self.settings.project,
            ui_theme: self.settings.ui_theme.clone(),
            runner: if unsafe { gtk_combo_box_get_active(f.runner) } == 1 {
                "wine"
            } else {
                "proton"
            }
            .into(),
            proton: entry_text(f.proton),
            wine: entry_text(f.wine),
            installer: entry_text(f.installer),
            executable: entry_text(f.executable),
            prefix_root: entry_text(f.prefix),
            wined3d: checked(f.wined3d),
            disable_gpu: checked(f.disable_gpu),
            discrete_gpu: checked(f.discrete_gpu),
            debug: checked(f.debug),
            no_sync: checked(f.no_sync),
            launcher_compatibility: checked(f.launcher_compatibility),
            input_compatibility: checked(f.input_compatibility),
            fps_hud: checked(f.fps_hud),
            game_mode: checked(f.game_mode),
            update_policy: ["startup", "daily", "weekly", "manual"]
                .get(unsafe { gtk_combo_box_get_active(f.update_policy) } as usize)
                .ok_or("Выберите режим проверки обновлений")?
                .to_string(),
            update_notifications: checked(f.update_notifications),
        };
        s.validate()?;
        Ok(s)
    }

    fn save(&mut self) -> Result<()> {
        let s = self.collect()?;
        s.save()?;
        self.settings = s;
        self.game_preferences()?.save()?;
        unsafe {
            gtk_widget_set_visible(self.settings_save_bar, 0);
        }
        self.persist_player(true)?;
        set_label(
            self.settings_hint,
            "Настройки сохранены. Они применятся при следующем запуске игры.",
        );
        self.refresh();
        Ok(())
    }

    fn game_preferences(&self) -> Result<GameOptions> {
        let selected = unsafe { gtk_combo_box_get_active(self.game_memory) };
        let memory = *[1024, 2048, 3072, 4096]
            .get(selected as usize)
            .ok_or("Выберите память игры")?;
        Ok(GameOptions {
            project: self.settings.project,
            memory,
            values: self
                .game_checks
                .iter()
                .filter(|(id, _)| {
                    game_options::OPTIONS
                        .iter()
                        .any(|o| o.id == *id && o.supports(self.settings.project))
                })
                .map(|(id, w)| ((*id).into(), checked(*w)))
                .collect(),
        })
    }
    fn save_quick_preferences(&mut self) -> Result<()> {
        let mut saved = Settings::load()?;
        self.game_preferences()?.save()?;
        macro_rules! sync_toggles {
            ($($field:ident),*) => {{
                $(saved.$field = checked(self.fields.$field);)*
                saved.save()?;
                $(self.settings.$field = saved.$field;)*
            }};
        }
        sync_toggles!(
            wined3d,
            disable_gpu,
            discrete_gpu,
            debug,
            no_sync,
            launcher_compatibility,
            input_compatibility,
            fps_hud,
            game_mode
        );
        set_label(self.status, "Сохранено · применится при следующем запуске");
        Ok(())
    }

    fn refresh(&self) {
        let s = &self.settings;
        let installed = s.find_executable();
        let game_ready = game::game_executable(s).is_ok();
        unsafe {
            gtk_widget_set_visible(self.hero, (!game_ready).into());
            gtk_widget_set_visible(self.player_controls, game_ready.into());
        }
        set_label(
            self.hero_title,
            if game_ready {
                "Время играть."
            } else if installed.is_some() {
                "Осталось скачать игру"
            } else {
                "Начнём с установки"
            },
        );
        set_label(
            self.hero_step,
            if game_ready {
                s.project.name()
            } else if installed.is_some() {
                "ШАГ 2 ИЗ 3 · ФАЙЛЫ ИГРЫ"
            } else {
                "ШАГ 1 ИЗ 3 · УСТАНОВКА"
            },
        );
        let prepared = s.prefix_ready();
        let runtime_name = if s.runner == "wine" {
            s.wine.clone()
        } else {
            std::path::Path::new(&s.proton)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into()
        };
        set_label(
            self.runtime,
            &format!(
                "{}  ·  {}",
                runtime_name,
                if s.wined3d {
                    "OpenGL"
                } else if s.runner == "wine" {
                    "Драйвер Wine"
                } else {
                    "Vulkan"
                }
            ),
        );
        set_label(
            self.detail,
            if game_ready {
                if s.project == crate::project::Project::Rodina {
                    "Выбери сервер. Твой персонаж ждёт внутри игры."
                } else {
                    "Твой сервер, твой персонаж. Всё готово к новой истории."
                }
            } else if installed.is_some() {
                "Откроем официальный лаунчер на выбранной игре. После загрузки играйте прямо отсюда."
            } else if prepared {
                "Всё подготовлено. Выберите установщик Arizona — остальное сделаем за вас."
            } else {
                "Выберите установщик Arizona Games. ALOL подготовит окружение для обеих игр."
            },
        );
        unsafe {
            gtk_button_set_label(
                self.play,
                c(if game_ready {
                    "Играть"
                } else if installed.is_some() {
                    "Скачать игру"
                } else {
                    if s.project == crate::project::Project::Arizona {
                        "Установить Arizona"
                    } else {
                        "Установить лаунчер"
                    }
                })
                .as_ptr(),
            );
        }
        if self.child.is_none() {
            set_label(
                self.status,
                if game_ready {
                    "Готов к запуску"
                } else if installed.is_some() {
                    "Следующий шаг — скачать файлы игры"
                } else if prepared {
                    "Можно установить общий лаунчер"
                } else {
                    "Требуется установка"
                },
            );
        }
        self.refresh_actions();
    }

    fn refresh_actions(&self) {
        let idle = self.child.is_none() && !self.update_installed;
        unsafe {
            gtk_widget_set_sensitive(
                self.update_game,
                (idle && self.settings.find_executable().is_some()).into(),
            );
            gtk_widget_set_sensitive(
                self.game_folder,
                (idle && self.settings.prefix_ready()).into(),
            );
        }
    }

    fn start(&mut self, action: &str) -> Result<()> {
        self.start_args(action, &[])
    }

    fn start_args(&mut self, action: &str, args: &[String]) -> Result<()> {
        if self.child.is_some() {
            return Err("Дождитесь завершения текущей операции".into());
        }
        if action == "install"
            && !std::path::Path::new(&entry_text(self.fields.installer)).is_file()
            && !self.choose_installer()
        {
            return Ok(());
        }
        self.save()?;
        let file = core::open_log(action, self.settings.project)?;
        let stderr = file.try_clone().map_err(|e| e.to_string())?;
        let mut cmd = Command::new(&self.executable);
        cmd.args(["--project", self.settings.project.id(), action])
            .args(args)
            .stdout(Stdio::from(file))
            .stderr(Stdio::from(stderr))
            .stdin(Stdio::null())
            .process_group(0);
        self.child = Some(cmd.spawn().map_err(|e| {
            logs::failure(action, &e.to_string());
            e.to_string()
        })?);
        self.action = action.into();
        self.stop_at = None;
        set_label(
            self.status,
            match action {
                "prepare" => "Подготовка окружения, шрифтов и Visual C++…",
                "install" => "Установка Arizona и необходимых компонентов…",
                "launch" => "Официальный лаунчер работает · закройте его для завершения",
                "play" => "Запуск игры на выбранном сервере…",
                "servers-refresh" => "Обновление списка серверов выбранной игры…",
                "repair" => "Восстановление окружения…",
                "repair-game" => "Исправление загрузки шрифтов и DLL игры…",
                "repair-language" => "Исправление русского текста игры…",
                "mods-refresh" => "Загрузка официального каталога модов…",
                "mods-install" | "mods-add" => "Установка мода с резервным копированием…",
                "mods-enable" | "mods-disable" | "mods-remove" => "Применение изменений мода…",
                "libraries" => "Установка шрифтов и Visual C++…",
                "winecfg" => "Открыты настройки Wine",
                "doctor" => "Диагностика…",
                "self-update" => "Загрузка и установка обновления ALOL…",
                "runtime-prepare" => {
                    "Подготовка экспериментального ALOL-Proton · база около 539 МБ…"
                }
                _ => "Выполняется…",
            },
        );
        unsafe {
            for b in &self.buttons {
                gtk_widget_set_sensitive(*b, 0);
            }
            gtk_widget_set_sensitive(self.settings_page, 0);
            gtk_widget_set_sensitive(self.mods_page, 0);
            gtk_widget_set_sensitive(self.player_controls, 0);
            // Do not interrupt a privileged package transaction midway through installation.
            gtk_widget_set_sensitive(self.stop, (action != "self-update").into());
            gtk_widget_set_visible(self.stop, 1);
            gtk_spinner_start(self.spinner);
            if action == "doctor" {
                gtk_notebook_set_current_page(self.notebook, 3);
            }
        }
        Ok(())
    }

    fn dispatch(&mut self, action: Action) -> Result<()> {
        if self.capture.is_some() {
            return Ok(());
        }
        match action {
            Action::About => self.show_about(),
            Action::ExternalLink(url) => {
                Command::new("xdg-open")
                    .arg(url)
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Action::Appearance => {
                let theme = if unsafe { gtk_combo_box_get_active(self.fields.ui_theme) } == 1 {
                    "light"
                } else {
                    "dark"
                };
                let mut saved = Settings::load()?;
                saved.ui_theme = theme.into();
                saved.save()?;
                self.settings.ui_theme = theme.into();
                self.apply_appearance();
            }
            Action::Navigate(page) => {
                unsafe {
                    gtk_notebook_set_current_page(self.notebook, page);
                }
                if page == 3 {
                    self.log.render();
                }
            }
            Action::Project(project) => self.switch_project(project)?,
            Action::RuntimePrepare => self.start_args("runtime-prepare", &["--select".into()])?,
            Action::QuickPreferences if self.updating_player => (),
            Action::QuickPreferences => self.save_quick_preferences()?,
            Action::SettingsChanged if self.updating_player => (),
            Action::SettingsChanged => {
                set_label(
                    self.settings_hint,
                    "Есть изменения путей или окружения. Сохраните их для применения.",
                );
                unsafe {
                    gtk_widget_set_visible(self.settings_save_bar, 1);
                }
            }
            Action::Help => unsafe {
                gtk_expander_set_expanded(self.help_expander, 1);
                gtk_notebook_set_current_page(self.settings_book, 3);
                gtk_notebook_set_current_page(self.notebook, 1);
                gtk_widget_grab_focus(self.help_expander);
            },
            Action::CopyLog => unsafe {
                let clipboard =
                    gtk_clipboard_get(gdk_atom_intern_static_string(c"CLIPBOARD".as_ptr()));
                gtk_clipboard_set_text(clipboard, c(&self.log.visible().text).as_ptr(), -1);
                set_label(self.status, "Скопированы видимые записи журнала");
            },
            Action::LogFilter => self.log.render(),
            Action::LogPause => {
                self.log.paused = !self.log.paused;
                unsafe {
                    gtk_button_set_label(
                        self.log.pause_button,
                        c(if self.log.paused {
                            "Продолжить"
                        } else {
                            "Пауза"
                        })
                        .as_ptr(),
                    );
                }
                self.log.poll();
                self.log.render();
            }
            Action::LogClear => {
                logs::clear()?;
                self.log.source.clear();
                self.log.reader = core::LogTail::default();
                self.log.render();
                set_label(self.status, "Журнал ALOL и его архив очищены");
            }
            Action::LogFolder => {
                let folder = core::log_file().parent().unwrap().to_path_buf();
                std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                Command::new("xdg-open")
                    .arg(folder)
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Action::UpdatePreferences => {
                let current = self.collect()?;
                let mut saved = Settings::load()?;
                saved.update_policy = current.update_policy;
                saved.update_notifications = current.update_notifications;
                saved.save()?;
                self.settings.update_policy = saved.update_policy;
                self.settings.update_notifications = saved.update_notifications;
                if !self.settings.update_notifications {
                    self.update_prompt_pending = false;
                }
                set_label(self.status, "Настройки обновлений сохранены");
            }
            Action::Update => {
                if self.child.is_some() {
                    return Ok(());
                }
                if self.update_installed {
                    self.persist_player(true)?;
                    Command::new(&self.executable)
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    unsafe {
                        gtk_main_quit();
                    }
                } else if self.update_release.is_some() {
                    self.show_update_dialog();
                } else {
                    self.check_updates(true);
                }
            }
            Action::Work(op) => self.start(op)?,
            Action::PlayerChanged => {
                if !self.updating_player {
                    self.persist_player(false)?;
                }
            }
            Action::ModFilter => self.reload_mods(),
            Action::ModSelection => self.refresh_mod_actions(),
            Action::ModInstall => {
                let row = unsafe { gtk_list_box_get_selected_row(self.mod_list) };
                if row.is_null() {
                    return Err("Выберите официальный мод из списка".into());
                }
                let index = unsafe { gtk_list_box_row_get_index(row) } as usize;
                let id = self
                    .catalog_items
                    .get(index)
                    .ok_or("Мод не найден")?
                    .id
                    .clone();
                self.start_args("mods-install", &[id])?;
            }
            Action::ModFile => {
                self.dispatch(Action::Browse(self.mod_file, false))?;
            }
            Action::ModImport | Action::ModPreview => {
                let file = entry_text(self.mod_file);
                let selected = unsafe { gtk_combo_box_get_active(self.mod_target) };
                let mode = ["auto", "root", "modloader", "cleo", "moonloader"]
                    .get(selected as usize)
                    .copied()
                    .unwrap_or("auto");
                self.start_args(
                    if matches!(action, Action::ModPreview) {
                        "mods-preview"
                    } else {
                        "mods-add"
                    },
                    &[file, mode.into()],
                )?;
            }
            Action::ModManage(operation) => {
                let row = unsafe { gtk_list_box_get_selected_row(self.installed_list) };
                if row.is_null() {
                    return Err("Выберите установленный мод".into());
                }
                let index = unsafe { gtk_list_box_row_get_index(row) };
                let id = self
                    .installed_items
                    .get(index as usize)
                    .ok_or("Выберите установленный мод")?
                    .id
                    .clone();
                self.start_args(operation, &[id])?;
            }
            Action::ChooseInstaller => {
                if self.choose_installer() {
                    self.save()?;
                    set_label(
                        self.status,
                        "Установщик выбран. Нажмите кнопку установки на главном экране.",
                    );
                }
            }
            Action::Website => {
                Command::new("xdg-open")
                    .arg(self.settings.project.website())
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Action::Play => {
                let s = self.collect()?;
                self.persist_player(true)?;
                let operation = if game::game_executable(&s).is_ok() {
                    self.player.validate_launch()?;
                    "play"
                } else if s.find_executable().is_some() {
                    "launch"
                } else {
                    "install"
                };
                self.start(operation)?;
            }
            Action::Save => {
                self.save()?;
                set_label(self.status, "Настройки сохранены");
            }
            Action::Folder => {
                let path = self
                    .settings
                    .find_executable()
                    .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
                    .unwrap_or_else(|| self.settings.prefix());
                std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
                Command::new("xdg-open")
                    .arg(path)
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Action::Stop => {
                if let Some(child) = &self.child {
                    // The worker and all of its children have a dedicated process group.
                    unsafe {
                        if kill(-(child.id() as i32), 15) != 0 {
                            return Err(std::io::Error::last_os_error().to_string());
                        }
                    }
                    self.stop_at = Some(unsafe { g_get_monotonic_time() });
                    set_label(self.status, "Остановка…");
                }
            }
            Action::Browse(field, folder) => unsafe {
                let dialog = gtk_file_chooser_dialog_new(
                    c"Выберите путь".as_ptr(),
                    self.window,
                    if folder { 2 } else { 0 },
                    ptr::null::<c_char>(),
                );
                gtk_dialog_add_button(dialog, c"Отмена".as_ptr(), -6);
                gtk_dialog_add_button(dialog, c"Выбрать".as_ptr(), -3);
                if gtk_dialog_run(dialog) == -3 {
                    let path = gtk_file_chooser_get_filename(dialog);
                    if !path.is_null() {
                        gtk_entry_set_text(field, path);
                        g_free(path.cast());
                    }
                }
                gtk_widget_destroy(dialog);
            },
        }
        Ok(())
    }

    fn choose_installer(&self) -> bool {
        unsafe {
            let dialog = gtk_file_chooser_dialog_new(
                c"Выберите скачанный установщик Arizona (.exe)".as_ptr(),
                self.window,
                0,
                ptr::null::<c_char>(),
            );
            gtk_dialog_add_button(dialog, c"Отмена".as_ptr(), -6);
            gtk_dialog_add_button(dialog, c"Выбрать".as_ptr(), -3);
            let mut selected = false;
            if gtk_dialog_run(dialog) == -3 {
                let path = gtk_file_chooser_get_filename(dialog);
                if !path.is_null() {
                    gtk_entry_set_text(self.fields.installer, path);
                    g_free(path.cast());
                    selected = true;
                }
            }
            gtk_widget_destroy(dialog);
            selected
        }
    }

    fn reload_server_choices(&mut self) -> Result<()> {
        self.server_rows = game::servers(self.settings.project)?;
        self.updating_player = true;
        unsafe {
            if self.player_signal != 0 {
                g_signal_handler_block(self.server_combo, self.player_signal);
            }
            gtk_combo_box_text_remove_all(self.server_combo);
            let mut selected = -1;
            for (index, s) in self.server_rows.iter().enumerate() {
                gtk_combo_box_text_append_text(self.server_combo, c(&s.label()).as_ptr());
                if s.id == self.player.server_id {
                    selected = index as i32;
                }
            }
            gtk_combo_box_set_active(self.server_combo, selected);
            if self.player_signal != 0 {
                g_signal_handler_unblock(self.server_combo, self.player_signal);
            }
        }
        self.updating_player = false;
        self.update_player_hint();
        Ok(())
    }

    fn update_player_hint(&self) {
        let arizona = self.settings.project == crate::project::Project::Arizona;
        let selected = unsafe { gtk_combo_box_get_active(self.server_combo) };
        let text = self
            .server_rows
            .get(selected as usize)
            .map(|s| format!("{}:{}", s.host, s.port))
            .unwrap_or_else(|| "Выберите сервер или обновите список.".into());
        unsafe {
            gtk_widget_set_tooltip_text(self.player_hint, c(&text).as_ptr());
        }
        set_label(
            self.player_hint,
            if arizona {
                "Никнейм сохраняется для каждого сервера."
            } else {
                "Никнейм и персонаж выбираются в самой игре."
            },
        );
    }

    fn persist_player(&mut self, force: bool) -> Result<()> {
        if self.nickname_entry.is_null() || self.updating_player {
            return Ok(());
        }
        let arizona = self.settings.project == crate::project::Project::Arizona;
        let typed = if arizona {
            entry_text(self.nickname_entry)
        } else {
            String::new()
        };
        let selected = unsafe { gtk_combo_box_get_active(self.server_combo) };
        let selected_id = self
            .server_rows
            .get(selected as usize)
            .map(|s| s.id.clone());
        let mut changed = false;
        if let Some(id) = selected_id
            && id != self.player.server_id
        {
            self.player.set_nickname(typed);
            self.player.select(&id);
            unsafe {
                gtk_entry_set_text(self.nickname_entry, c(&self.player.nickname).as_ptr());
            }
            changed = true;
        } else if arizona && typed != self.player.nickname {
            self.player.set_nickname(typed);
            changed = true;
        }
        if changed {
            self.player_dirty = true;
            self.player_changed_at = unsafe { g_get_monotonic_time() };
            self.update_player_hint();
        }
        if self.player_dirty
            && (force || unsafe { g_get_monotonic_time() } - self.player_changed_at >= 700_000)
        {
            self.player.save()?;
            self.player_dirty = false;
        }
        Ok(())
    }

    fn build_player_controls(&mut self) -> Result<W> {
        let controls = box_new(true, 6);
        self.player_controls = controls;
        style(controls, "section-card");
        self.player_title = label("Сервер и персонаж", "section-title");
        pack(controls, self.player_title, false);
        pack(controls, label("Сервер", "field-label"), false);
        let row = box_new(false, 10);
        self.server_combo = unsafe { gtk_combo_box_text_new() };
        pack(row, self.server_combo, true);
        self.button(
            row,
            "Обновить серверы",
            Action::Work("servers-refresh"),
            false,
        );
        pack(controls, row, false);
        self.nickname_controls = box_new(true, 6);
        unsafe {
            gtk_widget_set_name(self.nickname_controls, c"alol-nickname-controls".as_ptr());
        }
        pack(
            self.nickname_controls,
            label("Никнейм", "field-label"),
            false,
        );
        self.nickname_entry = entry(&self.player.nickname);
        unsafe {
            gtk_widget_set_name(self.nickname_entry, c"alol-nickname".as_ptr());
        }
        unsafe {
            gtk_entry_set_placeholder_text(self.nickname_entry, c"Nick_Name".as_ptr());
        }
        pack(self.nickname_controls, self.nickname_entry, false);
        pack(controls, self.nickname_controls, false);
        self.player_hint = label("", "caption");
        pack(controls, self.player_hint, false);
        self.reload_server_choices()?;
        self.connect_action(self.nickname_entry, "activate", Action::Play);
        // Combo changes are handled synchronously so a nickname typed immediately
        // after choosing another server cannot overwrite the previous server's nick.
        let mut signal = SignalHolder(Box::new(Signal {
            app: self,
            action: Action::PlayerChanged,
        }));
        let pointer = (&mut *signal.0 as *mut Signal).cast();
        unsafe {
            self.player_signal = g_signal_connect_data(
                self.server_combo,
                c"changed".as_ptr(),
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(W, W),
                    unsafe extern "C" fn(),
                >(clicked)),
                pointer,
                ptr::null_mut(),
                0,
            );
        }
        self.signals.push(signal);
        Ok(controls)
    }

    fn build_mods(&mut self) -> W {
        unsafe {
            let page = box_new(true, 10);
            self.mods_page = page;
            self.mod_book = gtk_notebook_new();
            gtk_widget_set_name(self.mod_book, c"alol-mod-book".as_ptr());
            style(self.mod_book, "secondary-tabs");
            pack(page, self.mod_book, true);
            let tabs = gtk_size_group_new(1);
            let catalog = box_new(true, 10);
            style(catalog, "pane");
            notebook_page(self.mod_book, catalog, "Каталог", tabs);
            let search_row = box_new(false, 8);
            self.mod_search = entry("");
            style(self.mod_search, "search-field");
            gtk_widget_set_name(self.mod_search, c"alol-mod-search".as_ptr());
            gtk_entry_set_placeholder_text(self.mod_search, c"Найти мод…".as_ptr());
            pack(search_row, self.mod_search, true);
            self.button(
                search_row,
                "Обновить каталог",
                Action::Work("mods-refresh"),
                false,
            );
            self.connect_action(self.mod_search, "activate", Action::ModFilter);
            self.connect_action(self.mod_search, "changed", Action::ModFilter);
            pack(catalog, search_row, false);
            self.catalog_title = label("", "caption");
            pack(catalog, self.catalog_title, false);
            self.mod_count = label("", "caption");
            pack(catalog, self.mod_count, false);
            self.mod_list = gtk_list_box_new();
            gtk_widget_set_name(self.mod_list, c"alol-mod-list".as_ptr());
            let scroll = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
            gtk_scrolled_window_set_policy(scroll, 2, 1);
            gtk_widget_set_size_request(scroll, -1, 120);
            gtk_container_add(scroll, self.mod_list);
            pack(catalog, scroll, true);
            let selection = box_new(false, 12);
            style(selection, "selection-bar");
            self.mod_selection = label("Выберите мод", "body");
            pack(selection, self.mod_selection, true);
            gtk_label_set_max_width_chars(self.mod_selection, 28);
            gtk_label_set_ellipsize(self.mod_selection, 3);
            self.mod_install = self.button(
                selection,
                "Установить выбранный мод",
                Action::ModInstall,
                true,
            );
            gtk_widget_set_name(self.mod_install, c"alol-mod-install".as_ptr());
            pack(catalog, selection, false);
            self.connect_action(self.mod_list, "row-selected", Action::ModSelection);

            let installed = box_new(true, 10);
            style(installed, "pane");
            notebook_page(self.mod_book, installed, "Мои моды", tabs);
            self.installed_count = label("", "caption");
            pack(installed, self.installed_count, false);
            self.installed_list = gtk_list_box_new();
            gtk_widget_set_name(self.installed_list, c"alol-installed-mods".as_ptr());
            let scroll = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
            gtk_scrolled_window_set_policy(scroll, 2, 1);
            gtk_container_add(scroll, self.installed_list);
            pack(installed, scroll, true);
            pack(
                installed,
                label(
                    "Отключение и удаление восстанавливают исходные файлы игры.",
                    "caption",
                ),
                false,
            );
            let row = action_row();
            for (title, operation) in [
                ("Включить", "mods-enable"),
                ("Отключить", "mods-disable"),
                ("Удалить", "mods-remove"),
            ] {
                let button = self.button(row, title, Action::ModManage(operation), false);
                self.mod_manage.push((operation, button));
            }
            pack(installed, row, false);
            self.connect_action(self.installed_list, "row-selected", Action::ModSelection);

            let local = box_new(true, 10);
            style(local, "pane");
            notebook_page(self.mod_book, local, "Добавить", tabs);
            pack(local, label("Свой мод", "section-title"), false);
            let (expander, import) = disclosure(local, "Файл или архив");
            gtk_widget_set_name(expander, c"alol-mod-import".as_ptr());
            gtk_expander_set_expanded(expander, 1);
            let row = box_new(false, 8);
            self.mod_file = entry("");
            gtk_widget_set_name(self.mod_file, c"alol-mod-file".as_ptr());
            gtk_entry_set_placeholder_text(self.mod_file, c"ZIP, RAR, 7z, ASI, CS, Lua…".as_ptr());
            pack(row, self.mod_file, true);
            self.button(row, "Выбрать файл", Action::ModFile, false);
            pack(import, row, false);
            pack(
                import,
                label(
                    "Расположение определяется автоматически. Проверка покажет файлы и зависимости до установки.",
                    "caption",
                ),
                false,
            );
            let (_, advanced) = disclosure(import, "Дополнительно: расположение файлов");
            self.mod_target = gtk_combo_box_text_new();
            for text in [
                "Определить автоматически",
                "Корень игры",
                "Mod Loader",
                "CLEO",
                "MoonLoader",
            ] {
                gtk_combo_box_text_append_text(self.mod_target, c(text).as_ptr());
            }
            gtk_combo_box_set_active(self.mod_target, 0);
            pack(advanced, self.mod_target, false);
            self.mod_preview = label("Выберите файл, затем нажмите «Проверить».", "caption");
            gtk_widget_set_name(self.mod_preview, c"alol-mod-preview".as_ptr());
            let preview_scroll = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
            gtk_scrolled_window_set_policy(preview_scroll, 2, 1);
            gtk_container_add(preview_scroll, self.mod_preview);
            pack(local, preview_scroll, true);
            let row = action_row();
            self.button(row, "Проверить", Action::ModPreview, false);
            self.button(row, "Добавить мод", Action::ModImport, true);
            pack(local, row, false);
            g_object_unref(tabs);
            page
        }
    }

    fn refresh_mod_actions(&self) {
        let idle = self.child.is_none() && !self.update_installed;
        let ready = mods::game_dir(&self.settings).is_ok();
        unsafe {
            let row = gtk_list_box_get_selected_row(self.mod_list);
            let selected = if row.is_null() {
                None
            } else {
                self.catalog_items
                    .get(gtk_list_box_row_get_index(row) as usize)
            };
            if !self.mod_install.is_null() {
                gtk_widget_set_sensitive(
                    self.mod_install,
                    (idle && ready && selected.is_some()).into(),
                );
                set_label(
                    self.mod_selection,
                    if !ready {
                        "Сначала скачайте выбранную игру"
                    } else {
                        selected
                            .map(|m| m.name.as_str())
                            .unwrap_or("Выберите мод из списка")
                    },
                );
            }
            let row = gtk_list_box_get_selected_row(self.installed_list);
            let selected = if row.is_null() {
                None
            } else {
                self.installed_items
                    .get(gtk_list_box_row_get_index(row) as usize)
            };
            for (operation, button) in &self.mod_manage {
                let allowed = selected.is_some_and(|m| match *operation {
                    "mods-enable" => !m.active,
                    "mods-disable" => m.active,
                    _ => true,
                });
                gtk_widget_set_sensitive(*button, (idle && ready && allowed).into());
            }
        }
    }

    fn reload_mods(&mut self) {
        if self.mod_list.is_null() {
            return;
        }
        let selected_catalog = unsafe {
            let row = gtk_list_box_get_selected_row(self.mod_list);
            if row.is_null() {
                None
            } else {
                self.catalog_items
                    .get(gtk_list_box_row_get_index(row) as usize)
                    .map(|m| m.id.clone())
            }
        };
        let selected_installed = unsafe {
            let row = gtk_list_box_get_selected_row(self.installed_list);
            if row.is_null() {
                None
            } else {
                self.installed_items
                    .get(gtk_list_box_row_get_index(row) as usize)
                    .map(|m| m.id.clone())
            }
        };
        let query = entry_text(self.mod_search).to_lowercase();
        let mut error = None;
        let all = match mods::catalog(&self.settings) {
            Ok(v) => v,
            Err(e) => {
                error = Some(e);
                Vec::new()
            }
        };
        let total = all.len();
        self.catalog_items = all
            .into_iter()
            .filter(|m| {
                format!("{} {} {}", m.name, m.description, m.category)
                    .to_lowercase()
                    .contains(&query)
            })
            .collect();
        let mut installed_error = None;
        self.installed_items = match mods::installed(&self.settings) {
            Ok(v) => v,
            Err(e) => {
                installed_error = Some(e);
                Vec::new()
            }
        };
        unsafe {
            for list in [self.mod_list, self.installed_list] {
                loop {
                    let row = gtk_list_box_get_row_at_index(list, 0);
                    if row.is_null() {
                        break;
                    }
                    gtk_widget_destroy(row);
                }
            }
            for m in &self.catalog_items {
                let status = self
                    .installed_items
                    .iter()
                    .find(|i| i.id == format!("official-{}", m.id))
                    .map(|i| {
                        if i.active {
                            "Включён"
                        } else {
                            "Отключён"
                        }
                    });
                let row = mod_row(&m.name, &m.version, &m.description, status);
                gtk_list_box_insert(self.mod_list, row, -1);
            }
            for m in &self.installed_items {
                let source = if m.source == "official" {
                    "Из каталога"
                } else {
                    "Из файла или архива"
                };
                gtk_list_box_insert(
                    self.installed_list,
                    mod_row(
                        &m.name,
                        &m.version,
                        source,
                        Some(if m.active {
                            "Включён"
                        } else {
                            "Отключён"
                        }),
                    ),
                    -1,
                );
            }
            let index = self
                .catalog_items
                .iter()
                .position(|m| Some(&m.id) == selected_catalog.as_ref())
                .unwrap_or(0);
            let row = gtk_list_box_get_row_at_index(self.mod_list, index as c_int);
            if !row.is_null() {
                gtk_list_box_select_row(self.mod_list, row);
            }
            let index = self
                .installed_items
                .iter()
                .position(|m| Some(&m.id) == selected_installed.as_ref())
                .unwrap_or(0);
            let row = gtk_list_box_get_row_at_index(self.installed_list, index as c_int);
            if !row.is_null() {
                gtk_list_box_select_row(self.installed_list, row);
            }
            gtk_widget_show_all(self.mod_list);
            gtk_widget_show_all(self.installed_list);
        }
        set_label(
            self.mod_count,
            &error.unwrap_or_else(|| {
                if total == 0 {
                    "Каталог ещё не загружен. Нажмите «Обновить каталог».".into()
                } else if self.catalog_items.is_empty() {
                    "По этому запросу ничего не найдено.".into()
                } else {
                    format!("Найдено: {} из {total}", self.catalog_items.len())
                }
            }),
        );
        set_label(
            self.installed_count,
            &installed_error.unwrap_or_else(|| {
                if self.installed_items.is_empty() {
                    "Пока нет установленных модов. Выберите мод в каталоге или добавьте свой архив."
                        .into()
                } else {
                    format!(
                        "Модов: {} · Включено: {}",
                        self.installed_items.len(),
                        self.installed_items.iter().filter(|m| m.active).count()
                    )
                }
            }),
        );
        self.refresh_mod_actions();
    }

    fn tick(&mut self) {
        self.poll_updates();
        if let Err(e) = self.persist_player(false) {
            set_label(self.status, &format!("Не сохранить ник и сервер: {e}"));
        }
        if let Some(child) = &mut self.child {
            if self
                .stop_at
                .is_some_and(|t| unsafe { g_get_monotonic_time() } - t > 3_000_000)
            {
                unsafe {
                    kill(-(child.id() as i32), 9);
                }
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stopped = self.stop_at.take().is_some();
                    self.child = None;
                    logs::finish(&self.action, status.success(), stopped);
                    if self.action == "mods-preview" {
                        set_label(
                            self.mod_preview,
                            &logs::view(&logs::normalize(&core::tail_log()), "", 0, true).text,
                        );
                    }
                    if self.action == "self-update" {
                        if status.success() {
                            self.update_installed = true;
                            set_label(
                                self.update_label,
                                "Обновление установлено. Перезапустите ALOL.",
                            );
                            unsafe {
                                gtk_button_set_label(self.update_button, c"Перезапустить".as_ptr());
                            }
                        } else {
                            self.update_release = None;
                            set_label(
                                self.update_label,
                                "Обновление не установлено. Подробности в журнале.",
                            );
                            unsafe {
                                gtk_button_set_label(
                                    self.update_button,
                                    c"Повторить проверку".as_ptr(),
                                );
                            }
                        }
                        self.finish_update_dialog(status.success());
                    }
                    if let Ok(mut s) = Settings::load() {
                        // A worker is pinned to the visible project even if another
                        // CLI instance changes the saved selection while it runs.
                        s.project = self.settings.project;
                        self.updating_player = true;
                        unsafe {
                            gtk_entry_set_text(self.fields.proton, c(&s.proton).as_ptr());
                            gtk_combo_box_set_active(
                                self.fields.runner,
                                if s.runner == "wine" { 1 } else { 0 },
                            );
                        }
                        self.updating_player = false;
                        self.settings = s;
                    }
                    self.refresh();
                    self.reload_mods();
                    if self.action == "servers-refresh"
                        && let Err(e) = self.reload_server_choices()
                    {
                        set_label(self.status, &e);
                    }
                    set_label(
                        self.status,
                        if stopped {
                            "Операция остановлена · подробности в журнале"
                        } else if status.success() {
                            if self.action == "play" {
                                "Игра закрыта · можно подключиться снова"
                            } else {
                                "Готово · подробности в журнале"
                            }
                        } else {
                            "Операция не выполнена · откройте журнал"
                        },
                    );
                    unsafe {
                        for b in &self.buttons {
                            gtk_widget_set_sensitive(
                                *b,
                                (!self.update_installed || *b == self.update_button).into(),
                            );
                        }
                        gtk_widget_set_sensitive(
                            self.settings_page,
                            (!self.update_installed).into(),
                        );
                        gtk_widget_set_sensitive(self.mods_page, (!self.update_installed).into());
                        gtk_widget_set_sensitive(
                            self.player_controls,
                            (!self.update_installed).into(),
                        );
                        gtk_widget_set_sensitive(self.stop, 0);
                        gtk_widget_set_visible(self.stop, 0);
                        gtk_spinner_stop(self.spinner);
                        if !status.success() && !stopped {
                            gtk_notebook_set_current_page(self.notebook, 3);
                        }
                    }
                    self.refresh_actions();
                    self.refresh_mod_actions();
                }
                Err(e) => set_label(self.status, &format!("Ошибка ожидания: {e}")),
                Ok(None) => (),
            }
        }
        self.log.poll();
        if self.update_prompt_pending && self.child.is_none() && self.update_dialog.is_null() {
            self.update_prompt_pending = false;
            self.show_update_dialog();
        }
    }

    fn check_updates(&mut self, force: bool) {
        if self.update_receiver.is_some() || self.update_installed {
            return;
        }
        self.update_release = None;
        self.update_manual = force;
        set_label(self.update_label, "Проверка обновлений ALOL…");
        unsafe {
            gtk_widget_set_sensitive(self.update_button, 0);
        }
        let (sender, receiver) = mpsc::channel();
        self.update_receiver = Some(receiver);
        let policy = self.settings.update_policy.clone();
        // One bounded check per launch or button click; GTK is only touched by the main thread.
        std::thread::spawn(move || {
            let _ = sender.send(if force {
                updates::check(true)
            } else {
                updates::check_scheduled(&policy)
            });
        });
    }

    fn poll_updates(&mut self) {
        let result = match self.update_receiver.as_ref().map(Receiver::try_recv) {
            Some(Ok(result)) => result,
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                Err("Проверка обновлений прервана".into())
            }
            _ => return,
        };
        self.update_receiver = None;
        match result {
            Ok(Some(release)) => {
                self.update_prompt_pending = self.update_manual
                    || (self.settings.update_notifications
                        && self.settings.update_policy != "manual");
                set_label(
                    self.update_label,
                    &format!(
                        "Доступна ALOL {} · у вас {}",
                        release.version,
                        env!("CARGO_PKG_VERSION")
                    ),
                );
                self.update_release = Some(release);
                unsafe {
                    gtk_button_set_label(self.update_button, c"Обновить ALOL".as_ptr());
                }
            }
            Ok(None) => {
                set_label(
                    self.update_label,
                    &format!("ALOL {} · актуальная версия", env!("CARGO_PKG_VERSION")),
                );
                unsafe {
                    gtk_button_set_label(self.update_button, c"Проверить обновления".as_ptr());
                }
            }
            Err(_) => {
                set_label(
                    self.update_label,
                    &format!(
                        "ALOL {} · не удалось проверить обновления",
                        env!("CARGO_PKG_VERSION")
                    ),
                );
                unsafe {
                    gtk_button_set_label(self.update_button, c"Повторить проверку".as_ptr());
                }
            }
        }
        unsafe {
            gtk_widget_set_sensitive(self.update_button, self.child.is_none().into());
        }
    }

    fn connect_action(&mut self, widget: W, name: &str, action: Action) {
        let mut signal = SignalHolder(Box::new(Signal { app: self, action }));
        let pointer = (&mut *signal.0 as *mut Signal).cast();
        unsafe {
            let callback = if name.starts_with("notify::") || name == "row-selected" {
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(W, W, W),
                    unsafe extern "C" fn(),
                >(notify_action))
            } else {
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(W, W),
                    unsafe extern "C" fn(),
                >(clicked))
            };
            g_signal_connect_data(
                widget,
                c(name).as_ptr(),
                callback,
                pointer,
                ptr::null_mut(),
                0,
            );
        }
        self.signals.push(signal);
    }

    fn show_update_dialog(&mut self) {
        if !self.update_dialog.is_null() || self.child.is_some() {
            return;
        }
        let Some(release) = &self.update_release else {
            return;
        };
        unsafe {
            let dialog = gtk_dialog_new();
            self.update_dialog = dialog;
            gtk_window_set_title(dialog, c"Обновление ALOL".as_ptr());
            gtk_window_set_transient_for(dialog, self.window);
            gtk_window_set_destroy_with_parent(dialog, 1);
            gtk_window_set_modal(dialog, 1);
            gtk_window_set_position(dialog, 4);
            let (width, height) = fit_window_size(620, 500);
            gtk_window_set_default_size(dialog, width, height);
            let content = gtk_dialog_get_content_area(dialog);
            gtk_container_set_border_width(content, 20);
            pack(
                content,
                label(&format!("Доступна ALOL {}", release.version), "alol-title"),
                false,
            );
            pack(
                content,
                label(
                    &format!("Установлена версия {}", env!("CARGO_PKG_VERSION")),
                    "caption",
                ),
                false,
            );
            pack(content, label("Что нового", "field-label"), false);
            let notes = gtk_text_view_new();
            style(notes, "release-notes");
            gtk_text_view_set_editable(notes, 0);
            gtk_text_view_set_cursor_visible(notes, 0);
            gtk_text_view_set_wrap_mode(notes, 3);
            let text = if release.notes.trim().is_empty() {
                "Описание доступно на странице релиза."
            } else {
                &release.notes
            };
            gtk_text_buffer_set_text(gtk_text_view_get_buffer(notes), c(text).as_ptr(), -1);
            let scroller = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
            gtk_scrolled_window_set_policy(scroller, 2, 1);
            gtk_widget_set_size_request(scroller, -1, 120);
            gtk_container_add(scroller, notes);
            pack(content, scroller, true);
            self.update_dialog_status = label(
                "При обновлении сохраняются настройки, игра и моды.",
                "caption",
            );
            pack(content, self.update_dialog_status, false);
            gtk_dialog_add_button(dialog, c"Открыть релиз".as_ptr(), 1);
            gtk_dialog_add_button(dialog, c"Позже".as_ptr(), -6);
            let install = gtk_dialog_add_button(dialog, c"Обновить".as_ptr(), -3);
            style(install, "primary");
            g_signal_connect_data(
                dialog,
                c"response".as_ptr(),
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(W, c_int, W),
                    unsafe extern "C" fn(),
                >(update_response)),
                (self as *mut App).cast(),
                ptr::null_mut(),
                0,
            );
            gtk_widget_show_all(dialog);
        }
    }

    fn respond_update(&mut self, response: c_int) -> Result<()> {
        if self.capture.is_some() || (self.child.is_some() && self.action == "self-update") {
            return Ok(());
        }
        if response == 1 {
            let url = self
                .update_release
                .as_ref()
                .map(|r| {
                    format!(
                        "https://github.com/Velmi1337/ALOL/releases/tag/v{}",
                        r.version
                    )
                })
                .unwrap_or_else(|| "https://github.com/Velmi1337/ALOL/releases/latest".into());
            Command::new("xdg-open")
                .arg(url)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        if response == -3 && self.update_installed {
            return self.dispatch(Action::Update);
        }
        if response == -3
            && let Some(release) = &self.update_release
        {
            let version = release.version.clone();
            self.start_args("self-update", &[version])?;
            if self.child.is_some() {
                set_label(
                    self.update_dialog_status,
                    "Загрузка и установка обновления… Для системного пакета подтвердите запрос прав.",
                );
                unsafe {
                    for response in [1, -6, -3] {
                        gtk_dialog_set_response_sensitive(self.update_dialog, response, 0);
                    }
                    gtk_window_set_deletable(self.update_dialog, 0);
                }
            }
            return Ok(());
        }
        let dialog = self.update_dialog;
        self.update_dialog = ptr::null_mut();
        self.update_dialog_status = ptr::null_mut();
        if !dialog.is_null() {
            unsafe {
                gtk_widget_destroy(dialog);
            }
        }
        if response == -3 {
            self.check_updates(true);
        }
        Ok(())
    }

    fn finish_update_dialog(&mut self, success: bool) {
        if self.update_dialog.is_null() {
            return;
        }
        set_label(
            self.update_dialog_status,
            if success {
                "Обновление установлено. Нажмите «Перезапустить», чтобы открыть новую версию."
            } else {
                "Обновление не установлено. Подробности во вкладке «Журнал». Можно повторить проверку."
            },
        );
        unsafe {
            gtk_window_set_deletable(self.update_dialog, 1);
            for response in [1, -6, -3] {
                gtk_dialog_set_response_sensitive(self.update_dialog, response, 1);
            }
            gtk_button_set_label(
                gtk_dialog_get_widget_for_response(self.update_dialog, -3),
                c(if success {
                    "Перезапустить"
                } else {
                    "Проверить снова"
                })
                .as_ptr(),
            );
            gtk_button_set_label(
                gtk_dialog_get_widget_for_response(self.update_dialog, -6),
                c"Закрыть".as_ptr(),
            );
        }
    }
}

unsafe extern "C" fn update_response(_: W, response: c_int, data: W) {
    let app = unsafe { &mut *(data as *mut App) };
    if let Err(error) = app.respond_update(response) {
        set_label(app.status, &error);
        if !app.update_dialog_status.is_null() {
            set_label(app.update_dialog_status, &error);
        }
    }
}

fn bounded_size(width: i32, height: i32, area_width: i32, area_height: i32) -> (i32, i32) {
    (
        width.min(area_width.saturating_sub(64).max(1)),
        height.min(area_height.saturating_sub(96).max(1)),
    )
}

fn fit_window_size(width: i32, height: i32) -> (i32, i32) {
    unsafe {
        let display = gdk_display_get_default();
        let mut monitor = gdk_display_get_primary_monitor(display);
        if monitor.is_null() {
            monitor = gdk_display_get_monitor(display, 0);
        }
        if monitor.is_null() {
            return (width, height);
        }
        let mut area = Rectangle::default();
        gdk_monitor_get_workarea(monitor, &mut area);
        bounded_size(width, height, area.width, area.height)
    }
}

fn window_icon(window: W) {
    unsafe {
        let loader = gdk_pixbuf_loader_new_with_type(c"png".as_ptr(), ptr::null_mut());
        if loader.is_null() {
            return;
        }
        if gdk_pixbuf_loader_write(
            loader,
            desktop::ICON.as_ptr(),
            desktop::ICON.len(),
            ptr::null_mut(),
        ) != 0
            && gdk_pixbuf_loader_close(loader, ptr::null_mut()) != 0
        {
            let pixbuf = gdk_pixbuf_loader_get_pixbuf(loader);
            if !pixbuf.is_null() {
                gtk_window_set_default_icon(pixbuf);
                gtk_window_set_icon(window, pixbuf);
            }
        }
        g_object_unref(loader);
    }
}

fn logo(size: c_int) -> W {
    unsafe {
        let loader = gdk_pixbuf_loader_new_with_type(c"png".as_ptr(), ptr::null_mut());
        if loader.is_null() {
            return label("A", "brand");
        }
        let mut result = ptr::null_mut();
        if gdk_pixbuf_loader_write(
            loader,
            desktop::ICON.as_ptr(),
            desktop::ICON.len(),
            ptr::null_mut(),
        ) != 0
            && gdk_pixbuf_loader_close(loader, ptr::null_mut()) != 0
        {
            let pixbuf = gdk_pixbuf_loader_get_pixbuf(loader);
            if !pixbuf.is_null() {
                let scaled = gdk_pixbuf_scale_simple(pixbuf, size, size, 2);
                if !scaled.is_null() {
                    result = gtk_image_new_from_pixbuf(scaled);
                    g_object_unref(scaled);
                }
            }
        }
        g_object_unref(loader);
        if result.is_null() {
            label("A", "brand")
        } else {
            result
        }
    }
}

/// Small vector symbols stay sharp without depending on the desktop icon theme.
fn vector_image(svg: &str, size: i32) -> W {
    unsafe {
        let loader = gdk_pixbuf_loader_new_with_type(c"svg".as_ptr(), ptr::null_mut());
        if loader.is_null() {
            return label("", "caption");
        }
        let mut result = ptr::null_mut();
        if gdk_pixbuf_loader_write(loader, svg.as_ptr(), svg.len(), ptr::null_mut()) != 0
            && gdk_pixbuf_loader_close(loader, ptr::null_mut()) != 0
        {
            let pixbuf = gdk_pixbuf_loader_get_pixbuf(loader);
            if !pixbuf.is_null() {
                let scaled = gdk_pixbuf_scale_simple(pixbuf, size, size, 2);
                if !scaled.is_null() {
                    result = gtk_image_new_from_pixbuf(scaled);
                    g_object_unref(scaled);
                }
            }
        }
        g_object_unref(loader);
        if result.is_null() {
            label("", "caption")
        } else {
            result
        }
    }
}
fn symbol(name: &str, color: &str, size: i32) -> W {
    let path = match name {
        "play" => "M8 5 19 12 8 19Z",
        "tune" => "M4 7h5m5 0h6M4 17h10m5 0h1M9 4v6m5 4v6",
        "grid" => "M4 4h6v6H4ZM14 4h6v6h-6ZM4 14h6v6H4ZM14 14h6v6h-6Z",
        "notes" => "M6 3h12v18H6ZM9 7h6M9 11h6M9 15h4",
        "sun" => {
            "M12 3v2m0 14v2M3 12h2m14 0h2M5.6 5.6 7 7m10 10 1.4 1.4M5.6 18.4 7 17M17 7l1.4-1.4M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0"
        }
        "external" => "M14 4h6v6M20 4l-9 9M10 4H4v16h16v-6",
        "home" => "M3 11 12 3l9 8M5 10v11h14V10M10 21v-7h4v7",
        _ => "M5 12h14M12 5l7 7-7 7",
    };
    vector_image(
        &format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="{path}" fill="none" stroke="{color}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"#
        ),
        size,
    )
}
fn expressive_mark() -> W {
    vector_image(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="112" height="112" viewBox="0 0 112 112"><g fill="#8ab4f8"><circle cx="56" cy="28" r="24"/><circle cx="80" cy="42" r="24"/><circle cx="80" cy="70" r="24"/><circle cx="56" cy="84" r="24"/><circle cx="32" cy="70" r="24"/><circle cx="32" cy="42" r="24"/></g><circle cx="56" cy="56" r="32" fill="#a8c7fa"/><path d="M48 38Q46 37 46 40V73Q46 76 49 74L73 59Q77 56 73 53Z" fill="#062e6f"/></svg>"##,
        88,
    )
}

unsafe extern "C" fn clicked(_: W, data: W) {
    // SAFETY: SignalHolder and App outlive GTK's main loop; callbacks run there only.
    let signal = unsafe { &*(data as *const Signal) };
    let app = unsafe { &mut *signal.app };
    if let Err(error) = app.dispatch(signal.action) {
        set_label(app.status, &error);
    }
}
unsafe extern "C" fn notify_action(widget: W, _: W, data: W) {
    unsafe {
        clicked(widget, data);
    }
}
unsafe extern "C" fn about_response(dialog: W, _: c_int, data: W) {
    unsafe {
        (&mut *data.cast::<App>()).about_dialog = ptr::null_mut();
        gtk_widget_destroy(dialog);
    }
}
unsafe extern "C" fn close_requested(_: W, _: W, data: W) -> c_int {
    if !data.is_null() {
        let app = unsafe { &mut *(data as *mut App) };
        if let Err(e) = app.persist_player(true) {
            set_label(app.status, &format!("Не сохранить профиль игрока: {e}"));
            return 1;
        }
    }
    unsafe {
        gtk_main_quit();
    }
    // Defer destruction until the main loop and its timer have stopped.
    1
}
unsafe extern "C" fn tick(data: W) -> c_int {
    unsafe {
        (&mut *(data as *mut App)).tick();
    }
    1
}
unsafe extern "C" fn smoke_quit(_: W) -> c_int {
    unsafe {
        gtk_main_quit();
    }
    0
}

unsafe extern "C" fn capture_tick(data: W) -> c_int {
    let app = unsafe { &mut *(data as *mut App) };
    let capture = app
        .capture
        .as_ref()
        .expect("capture timer belongs to demo mode");
    let names = ["main.png", "settings.png", "mods.png"];
    let result = (|| {
        let (width, height) = unsafe {
            (
                gtk_widget_get_allocated_width(app.window),
                gtk_widget_get_allocated_height(app.window),
            )
        };
        if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
            return Err("Некорректный размер окна для скриншота".into());
        }
        let output = capture.output.join(names[capture.page]);
        unsafe {
            let surface = cairo_image_surface_create(0, width, height); // CAIRO_FORMAT_ARGB32
            let cairo = cairo_create(surface);
            gtk_widget_draw(app.window, cairo);
            let status = cairo_status(cairo);
            cairo_destroy(cairo);
            let saved = if status == 0 {
                cairo_surface_write_to_png(surface, c(&output.to_string_lossy()).as_ptr())
            } else {
                status
            };
            cairo_surface_destroy(surface);
            if saved != 0 {
                return Err("Не удалось сохранить PNG окна GTK".into());
            }
        }
        println!("Сохранён скриншот: {}", names[capture.page]);
        Ok(())
    })();
    let capture = app.capture.as_mut().expect("demo capture");
    if let Err(error) = result {
        capture.error = Some(error);
        unsafe {
            gtk_main_quit();
        }
        return 0;
    }
    capture.page += 1;
    if capture.page == names.len() {
        unsafe {
            gtk_main_quit();
        }
        return 0;
    }
    unsafe {
        gtk_notebook_set_current_page(app.notebook, capture.page as c_int);
    }
    1
}

const CSS: &str = r#"
/* Material 3 Expressive: semantic color roles, emphasized type and shape states.
   Native GTK adaptation; respect gtk-enable-animations and keyboard focus. */
@define-color md_error #f2b8b5;
@define-color md_surface #101418;
@define-color md_surface_container_low #181c20;
@define-color md_surface_container #1d2024;
@define-color md_surface_container_high #262a2f;
@define-color md_surface_container_highest #32363b;
@define-color md_on_surface #e3e3e3;
@define-color md_on_surface_variant #c4c7c5;
@define-color md_primary #a8c7fa;
@define-color md_on_primary #062e6f;
@define-color md_primary_container #284777;
@define-color md_on_primary_container #d3e3fd;
@define-color md_secondary_container #293541;
@define-color md_on_secondary_container #d3e3fd;
@define-color md_tertiary #a8dab5;
@define-color md_outline #8e918f;
@define-color md_outline_variant #444746;
@define-color md_hover #3b4047;
@define-color md_pressed #454c55;
@define-color md_primary_hover #bdd5fc;
@define-color md_primary_pressed #8bafe9;
@define-color md_hero #1e2a3a;
@define-color md_selected_hover #304f7e;
@define-color md_on_disabled #8e918f;
@define-color theme_bg_color @md_surface;
@define-color theme_fg_color @md_on_surface;
@define-color theme_base_color @md_surface_container;
@define-color theme_text_color @md_on_surface;
@define-color theme_selected_bg_color @md_primary_container;
@define-color theme_selected_fg_color @md_on_primary_container;
@define-color insensitive_fg_color @md_on_disabled;
* { font-family: "Google Sans", Sans; font-size: 14px; }
window, dialog, messagedialog { background: @md_surface; color: @md_on_surface; }
.main-content { padding: 20px 24px 12px 16px; }
.brand { color: @md_on_surface; font-size: 18px; font-weight: 800; }
.dev-badge { color: @md_primary; background: @md_surface_container_high; border-radius: 20px; padding: 5px 8px; font-size: 10px; font-weight: 700; }
.caption { color: @md_on_surface_variant; font-size: 12px; }
.body { color: @md_on_surface; font-size: 14px; }
.runtime { color: @md_on_surface_variant; font-size: 11px; margin-top: 6px; }
.eyebrow { color: @md_primary; font-size: 12px; font-weight: 700; }
.alol-title { color: @md_on_surface; font-size: 32px; font-weight: 600; }
.page-title { color: @md_on_surface; font-size: 32px; font-weight: 500; }
.section-title { color: @md_on_surface; font-size: 20px; font-weight: 500; }
.hero { background: @md_hero; border: none; border-radius: 28px 28px 48px 28px; padding: 16px 24px; }
.section-card { background: @md_surface_container; border: none; border-radius: 24px; padding: 20px; }
.help-content { padding: 12px 0; }
.field-label { font-size: 12px; font-weight: 600; color: @md_on_surface_variant; margin-top: 6px; }
button {
    background-image: none; background-color: @md_surface_container_highest;
    color: @md_on_surface; border: 1px solid transparent; border-radius: 26px;
    min-height: 20px; padding: 10px 18px; box-shadow: none; text-shadow: none;
    font-weight: 600;
    transition: background-color 180ms ease, border-radius 220ms ease, box-shadow 120ms ease;
}
button:hover { background-color: @md_hover; }
button:active { background-color: @md_pressed; border-radius: 12px; }
button:focus { border-color: @md_primary; box-shadow: 0 0 0 2px alpha(@md_primary, 0.24); }
button.primary { background-color: @md_primary; color: @md_on_primary; font-weight: 700; }
button.primary:hover { background-color: @md_primary_hover; }
button.primary:active { background-color: @md_primary_pressed; border-radius: 16px; }
button.play-button { font-size: 20px; min-height: 34px; padding: 14px 24px; border-radius: 32px; }
button.play-button:active { border-radius: 16px; }
button:disabled { background-color: alpha(@md_on_surface, 0.10); color: alpha(@md_on_surface, 0.45); border-color: transparent; box-shadow: none; }
button:disabled image { opacity: 0.45; }
.project-group { background: @md_surface_container_low; border-radius: 32px; padding: 4px; }
button.project-button { background-color: transparent; color: @md_on_surface_variant; border-radius: 24px; padding: 8px 18px; }
button.project-button:hover { background-color: @md_surface_container_high; }
button.project-selected { background-color: @md_primary_container; color: @md_on_primary_container; border-radius: 16px; }
button.project-selected:hover { background-color: @md_selected_hover; }
.navigation-rail { background: @md_surface; padding: 24px 8px; }
button.navigation-item { background-color: transparent; color: @md_on_surface_variant; border-radius: 24px; padding: 6px 2px; min-width: 64px; min-height: 44px; font-size: 12px; }
button.navigation-item:hover { background-color: @md_surface_container_high; }
button.navigation-item.nav-selected { background-color: transparent; color: @md_on_surface; font-weight: 700; }
button.navigation-item image { padding: 6px 16px; border-radius: 24px; }
button.navigation-item.nav-selected image { background: @md_secondary_container; }
button.navigation-item:active { border-radius: 12px; }
entry, combobox entry {
    background: @md_surface_container_high; color: @md_on_surface;
    border: none; border-bottom: 1px solid @md_outline; border-radius: 12px 12px 0 0;
    min-height: 22px; padding: 12px 14px; box-shadow: none; caret-color: @md_primary;
}
entry:focus { border-bottom-color: @md_primary; box-shadow: inset 0 -1px @md_primary; }
entry:disabled { color: alpha(@md_on_surface, 0.45); }
combobox button { background-color: @md_surface_container_high; border-bottom: 1px solid @md_outline; border-radius: 12px 12px 0 0; padding: 10px 14px; font-weight: 400; }
combobox button:hover { background-color: @md_surface_container_highest; }
notebook, notebook > stack { background: transparent; border: none; padding: 0; }
textview, textview text { background: @md_surface_container_low; color: @md_on_surface_variant; font-family: monospace; font-size: 12px; }
textview { padding: 16px; border-radius: 20px; }
checkbutton { color: @md_on_surface; padding: 8px 0; min-height: 24px; }
check { background: transparent; border: 2px solid @md_on_surface_variant; border-radius: 4px; min-width: 14px; min-height: 14px; margin-right: 8px; }
check:checked { background: @md_primary; color: @md_on_primary; border-color: @md_primary; }
checkbutton:hover { color: @md_on_primary_container; }
checkbutton:focus check { box-shadow: 0 0 0 4px alpha(@md_primary, 0.18); }
separator { background: @md_outline_variant; min-height: 1px; }
scrolledwindow { border: none; }
list { background: @md_surface_container_low; color: @md_on_surface; border-radius: 16px; }
list row { background: transparent; border: none; border-radius: 12px; margin: 2px 4px; }
list row:selected { background: @md_secondary_container; color: @md_on_secondary_container; }
list row:hover { background: @md_surface_container_highest; }
.save-bar { background: @md_surface_container_high; border: none; border-radius: 28px; padding: 12px 16px; }
.status-bar { background: @md_surface_container_low; border-radius: 20px; padding: 8px 14px; }
.status-bar label { font-size: 12px; color: @md_on_surface_variant; }
/* Popups are separate roots; explicitly style them for COSMIC and light GTK themes. */
menu, .menu, .context-menu, popover, popover.background, window.popup {
    background: @md_surface_container_high; color: @md_on_surface;
    border: 1px solid @md_outline_variant; border-radius: 12px; padding: 6px;
}
menuitem { background: transparent; color: @md_on_surface; padding: 10px 14px; border-radius: 8px; }
menuitem label, menuitem cellview, combobox cellview { color: inherit; }
menuitem:hover, menuitem:selected, menuitem:focus { background: @md_primary_container; color: @md_on_primary_container; }
menuitem:disabled { color: @md_on_disabled; }
menu arrow { color: @md_primary; background: @md_surface_container_highest; min-height: 18px; }
combobox arrow, expander arrow { color: @md_primary; }
expander > title { color: @md_on_surface_variant; padding: 12px 0; font-weight: 600; }
expander > title:hover { color: @md_primary; }
headerbar, .titlebar { background: @md_surface; color: @md_on_surface; border: none; box-shadow: none; }
headerbar .title, .titlebar .title { font-size: 14px; font-weight: 600; color: @md_on_surface; }
headerbar .subtitle, .titlebar .subtitle { color: @md_on_surface_variant; }
button.titlebutton { padding: 4px; min-height: 24px; min-width: 24px; border-radius: 20px; }
filechooser, filechooser .view, treeview.view { background: @md_surface_container; color: @md_on_surface; }
treeview.view:selected, treeview.view:selected:focus { background: @md_primary_container; color: @md_on_primary_container; }
treeview header button { background: @md_surface_container_high; color: @md_on_surface; }
tooltip, tooltip.background { background: @md_on_surface; color: @md_surface; border: none; border-radius: 8px; padding: 8px; }
tooltip label { color: @md_surface; }
scrollbar { background: transparent; }
scrollbar slider { background: @md_outline_variant; border-radius: 8px; min-width: 6px; min-height: 8px; }
scrollbar slider:hover { background: @md_outline; }
.release-notes, .release-notes text { font-family: "Google Sans", Sans; font-size: 14px; color: @md_on_surface; }
dialog .dialog-action-area { padding: 12px; }
window.compact .main-content { padding: 12px; }
window.compact .navigation-rail { padding: 8px 12px; }
window.compact button.navigation-item { min-width: 48px; padding: 6px 8px; min-height: 40px; }
window.compact .page-title { font-size: 26px; }
window.compact .alol-title { font-size: 26px; }
window.compact .hero { padding: 20px; }

.settings-group { margin-bottom: 12px; }
.group-title { color: @md_primary; font-size: 14px; font-weight: 500; padding: 4px 12px; }
.settings-group > .caption { padding: 0 12px 8px; }
.settings-surface { background: @md_surface_container_low; border-radius: 24px; padding: 12px; }
.preference-row { background: @md_surface_container; border-radius: 16px; padding: 10px 14px; min-height: 32px; }
.preference-title { color: @md_on_surface; font-size: 14px; }
switch { background: @md_surface_container_highest; border: 2px solid @md_outline; border-radius: 24px; color: transparent; }
switch:checked { background: @md_primary; border-color: @md_primary; }
switch slider { background: @md_outline; border: none; border-radius: 24px; min-width: 18px; min-height: 18px; margin: 4px; box-shadow: none; }
switch:checked slider { background: @md_on_primary; min-width: 22px; min-height: 22px; margin: 2px; }
switch:focus { box-shadow: 0 0 0 3px alpha(@md_primary, 0.24); }
switch:disabled { opacity: 0.45; }
switch image { color: transparent; }

notebook.secondary-tabs > header { background: transparent; border: none; border-bottom: 1px solid @md_outline_variant; box-shadow: none; padding: 0; margin-bottom: 12px; }
notebook.secondary-tabs > header tabs tab { background: transparent; border: none; border-bottom: 3px solid transparent; border-radius: 12px 12px 0 0; color: @md_on_surface_variant; padding: 12px 6px; margin: 0; }
notebook.secondary-tabs > header tabs tab:checked { background: alpha(@md_primary, 0.08); border-bottom-color: @md_primary; color: @md_primary; box-shadow: none; }
notebook.secondary-tabs > header tabs tab:hover { background: alpha(@md_primary, 0.12); }
notebook.secondary-tabs > header tabs tab label { font-weight: 500; }
.pane { padding: 4px; }
.search-field { border-radius: 28px; border: none; padding: 14px 20px; background: @md_surface_container_high; }
.selection-bar, .toolbar { background: @md_surface_container; border-radius: 24px; padding: 10px 12px; }
.mod-row { padding: 14px; }
.mod-title { color: @md_on_surface; font-size: 16px; font-weight: 500; }
.status-chip { background: @md_surface_container_highest; color: @md_primary; border-radius: 12px; padding: 4px 8px; font-size: 11px; }
button.destructive { color: @md_error; }
.about-links { padding: 14px 0; }
"#;
const LIGHT_COLORS: &str = r#"
@define-color md_error #b3261e;
@define-color md_surface #f8fafd;
@define-color md_surface_container_low #f0f4f9;
@define-color md_surface_container #e9eef6;
@define-color md_surface_container_high #e2e7ef;
@define-color md_surface_container_highest #dce1e9;
@define-color md_on_surface #1f1f1f;
@define-color md_on_surface_variant #444746;
@define-color md_primary #0b57d0;
@define-color md_on_primary #ffffff;
@define-color md_primary_container #d3e3fd;
@define-color md_on_primary_container #0842a0;
@define-color md_secondary_container #c2e7ff;
@define-color md_on_secondary_container #001d35;
@define-color md_tertiary #146c2e;
@define-color md_outline #747775;
@define-color md_outline_variant #c4c7c5;
@define-color md_hover #d5deec;
@define-color md_pressed #c7d3e3;
@define-color md_primary_hover #286bde;
@define-color md_primary_pressed #0842a0;
@define-color md_hero #dce9fc;
@define-color md_selected_hover #c2d7f8;
@define-color md_on_disabled #747775;
"#;
pub fn run() -> Result<()> {
    run_mode(None)
}
fn theme_css(light: bool) -> String {
    if !light {
        return CSS.into();
    }
    let (_, shared) = CSS
        .split_once("@define-color theme_bg_color")
        .expect("semantic GTK theme");
    format!("{LIGHT_COLORS}\n@define-color theme_bg_color{shared}")
}
fn register_interface_font() -> Result<()> {
    let bytes = include_bytes!("../assets/fonts/GoogleSans.ttf");
    let path = core::data_dir().join("ui/GoogleSans.ttf");
    if std::fs::read(&path).ok().as_deref() != Some(bytes) {
        mods::atomic_bytes(&path, bytes)?;
    }
    let name = c(&path.to_string_lossy());
    if unsafe { FcConfigAppFontAddFile(ptr::null_mut(), name.as_ptr().cast()) } == 0 {
        return Err("Google Sans недоступен; используется системный шрифт".into());
    }
    Ok(())
}
pub(crate) fn run_mode(capture_output: Option<std::path::PathBuf>) -> Result<()> {
    if let Err(error) = register_interface_font() {
        eprintln!("Шрифт интерфейса: {error}");
    }
    let settings = Settings::load()?;
    let player = PlayerProfile::load(&settings)?;
    let game_options = GameOptions::load(&settings)?;
    unsafe {
        g_set_prgname(c(desktop::application_id()).as_ptr());
        gdk_set_program_class(c(desktop::application_id()).as_ptr());
        g_set_application_name(c"ALOL".as_ptr());
        if gtk_init_check(ptr::null_mut(), ptr::null_mut()) == 0 {
            return Err("Нет графической сессии. Используйте alol --help для CLI.".into());
        }
        // Prefer the theme's dark variants for GTK widgets not customized by ALOL.
        g_object_set(
            gtk_settings_get_default(),
            c"gtk-application-prefer-dark-theme".as_ptr(),
            1 as c_int,
            ptr::null::<c_char>(),
        );
        let provider = gtk_css_provider_new();
        gtk_css_provider_load_from_data(
            provider,
            c(&theme_css(settings.ui_theme == "light")).as_ptr(),
            -1,
            ptr::null_mut(),
        );
        gtk_style_context_add_provider_for_screen(gdk_screen_get_default(), provider, 600);
        let window = gtk_window_new(0);
        gtk_window_set_title(
            window,
            c(if desktop::development_copy() {
                "ALOL · Разработка — Arizona Launcher On Linux"
            } else {
                "ALOL — Arizona Launcher On Linux"
            })
            .as_ptr(),
        );
        let (width, height) = fit_window_size(960, 740);
        gtk_window_set_default_size(window, width, height);
        gtk_window_set_position(window, 1);
        gtk_window_set_icon_name(window, c"alol".as_ptr());
        window_icon(window);
        if capture_output.is_none()
            && !std::env::args().any(|a| a == "--smoke-test")
            && let Err(error) =
                desktop::integrate().and_then(|_| desktop::integrate_game(settings.project))
        {
            eprintln!("Интеграция с рабочим столом: {error}");
        }
        // Desktop navigation rail becomes a compact navigation bar on small windows.
        let shell = box_new(false, 0);
        gtk_widget_set_name(shell, c"alol-shell".as_ptr());
        gtk_container_add(window, shell);
        let rail = box_new(true, 24);
        style(rail, "navigation-rail");
        let rail_brand = box_new(true, 8);
        pack(rail_brand, logo(48), false);
        let brand = label("ALOL", "brand");
        gtk_label_set_xalign(brand, 0.5);
        pack(rail_brand, brand, false);
        pack(rail, rail_brand, false);
        let navigation_items = box_new(true, 8);
        pack(rail, navigation_items, false);
        let rail_meta = box_new(true, 8);
        if desktop::development_copy() {
            let badge = label("DEV", "dev-badge");
            gtk_label_set_xalign(badge, 0.5);
            pack(rail_meta, badge, false);
        }
        let version = label(env!("CARGO_PKG_VERSION"), "caption");
        gtk_label_set_xalign(version, 0.5);
        pack(rail_meta, version, false);
        pack(rail, rail_meta, false);
        pack(shell, rail, false);
        let outer = box_new(true, 16);
        style(outer, "main-content");
        pack(shell, outer, true);
        let header = box_new(false, 16);
        let heading = box_new(true, 4);
        let page_title = label("Играть", "page-title");
        let page_subtitle = label("", "caption");
        pack(heading, page_title, false);
        pack(heading, page_subtitle, false);
        pack(header, heading, true);
        pack(outer, header, false);
        let notebook = gtk_notebook_new();
        gtk_widget_set_name(notebook, c"alol-notebook".as_ptr());
        gtk_notebook_set_show_tabs(notebook, 0);
        gtk_notebook_set_show_border(notebook, 0);
        let tab_sizes = gtk_size_group_new(1);
        pack(outer, notebook, true);
        let home = box_new(true, 12);
        gtk_container_set_border_width(home, 4);
        let hero = box_new(false, 12);
        style(hero, "hero");
        let hero_copy = box_new(true, 8);
        let hero_step = label("ARIZONA ROLE PLAY", "eyebrow");
        let hero_title = label("Можно играть", "alol-title");
        pack(hero_copy, hero_step, false);
        pack(hero_copy, hero_title, false);
        let detail = label("", "body");
        pack(hero_copy, detail, false);
        let runtime = label("", "runtime");
        pack(hero, hero_copy, true);
        pack(hero, expressive_mark(), false);
        pack(home, hero, false);
        let settings_page = box_new(true, 14);
        gtk_container_set_border_width(settings_page, 4);
        let settings_launch = box_new(true, 14);
        let settings_launcher = box_new(true, 14);
        let settings_help = box_new(true, 14);
        let log_page = box_new(true, 10);
        gtk_container_set_border_width(log_page, 4);
        let log_search = entry("");
        style(log_search, "search-field");
        gtk_widget_set_name(log_search, c"alol-log-search".as_ptr());
        gtk_entry_set_placeholder_text(log_search, c"Поиск в журнале…".as_ptr());
        let log_search_row = box_new(false, 8);
        pack(log_search_row, log_search, true);
        pack(log_page, log_search_row, false);
        let log_options = box_new(false, 8);
        let log_level = gtk_combo_box_text_new();
        gtk_widget_set_name(log_level, c"alol-log-level".as_ptr());
        for text in ["Все уровни", "Ошибки и предупреждения", "Только ошибки"]
        {
            gtk_combo_box_text_append_text(log_level, c(text).as_ptr());
        }
        gtk_combo_box_set_active(log_level, 0);
        let log_session = gtk_combo_box_text_new();
        gtk_widget_set_name(log_session, c"alol-log-session".as_ptr());
        for text in ["Последняя операция", "Весь журнал"] {
            gtk_combo_box_text_append_text(log_session, c(text).as_ptr());
        }
        gtk_combo_box_set_active(log_session, 0);
        pack(log_options, log_level, true);
        pack(log_options, log_session, true);
        let log_follow = check(log_options, "Следить", true);
        gtk_widget_set_name(log_follow, c"alol-log-follow".as_ptr());
        pack(log_page, log_options, false);
        let log_summary = label("", "caption");
        gtk_widget_set_name(log_summary, c"alol-log-summary".as_ptr());
        pack(log_page, log_summary, false);
        let view = gtk_text_view_new();
        gtk_widget_set_name(view, c"alol-log-view".as_ptr());
        gtk_text_view_set_editable(view, 0);
        gtk_text_view_set_cursor_visible(view, 0);
        gtk_text_view_set_monospace(view, 1);
        gtk_text_view_set_wrap_mode(view, 3);
        let scroller = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
        gtk_scrolled_window_set_policy(scroller, 2, 1);
        gtk_container_add(scroller, view);
        pack(log_page, scroller, true);
        let status = label("", "body");
        let spinner = gtk_spinner_new();
        let footer = box_new(false, 10);
        gtk_widget_set_size_request(status, 160, -1);
        pack(footer, spinner, false);
        pack(footer, status, true);
        gtk_label_set_max_width_chars(runtime, 26);
        gtk_label_set_ellipsize(runtime, 3);
        pack(footer, runtime, false);
        let empty = Fields {
            ui_theme: ptr::null_mut(),
            runner: ptr::null_mut(),
            proton: ptr::null_mut(),
            wine: ptr::null_mut(),
            installer: ptr::null_mut(),
            executable: ptr::null_mut(),
            prefix: ptr::null_mut(),
            wined3d: ptr::null_mut(),
            disable_gpu: ptr::null_mut(),
            discrete_gpu: ptr::null_mut(),
            debug: ptr::null_mut(),
            no_sync: ptr::null_mut(),
            launcher_compatibility: ptr::null_mut(),
            input_compatibility: ptr::null_mut(),
            fps_hud: ptr::null_mut(),
            game_mode: ptr::null_mut(),
            update_policy: ptr::null_mut(),
            update_notifications: ptr::null_mut(),
        };
        let mut app = Box::new(App {
            about_dialog: ptr::null_mut(),
            theme_provider: provider,
            chrome: Box::new(Chrome {
                window,
                shell,
                rail,
                rail_brand,
                rail_meta,
                items: navigation_items,
                title: page_title,
                subtitle: page_subtitle,
                buttons: Vec::new(),
                compact: Cell::new(width >= 840),
            }),
            settings: settings.clone(),
            project_buttons: Vec::new(),
            catalog_title: ptr::null_mut(),
            window,
            notebook,
            status,
            runtime,
            hero_title,
            hero,
            hero_step,
            help_expander: ptr::null_mut(),
            settings_save_bar: ptr::null_mut(),
            settings_hint: label(
                "Изменения применятся при следующем запуске игры.",
                "caption",
            ),
            detail,
            play: ptr::null_mut(),
            update_game: ptr::null_mut(),
            game_folder: ptr::null_mut(),
            spinner,
            stop: ptr::null_mut(),
            settings_page,
            log: LogPanel {
                buffer: gtk_text_view_get_buffer(view),
                search: log_search,
                level: log_level,
                session: log_session,
                follow: log_follow,
                summary: log_summary,
                pause_button: ptr::null_mut(),
                adjustment: gtk_scrolled_window_get_vadjustment(scroller),
                source: String::new(),
                paused: false,
                scroll_to: None,
                reader: core::LogTail::default(),
            },
            settings_book: ptr::null_mut(),
            fields: empty,
            buttons: Vec::new(),
            utility_buttons: gtk_size_group_new(1),
            signals: Vec::new(),
            child: None,
            action: String::new(),
            stop_at: None,
            mods_page: ptr::null_mut(),
            mod_list: ptr::null_mut(),
            mod_search: ptr::null_mut(),
            mod_file: ptr::null_mut(),
            mod_target: ptr::null_mut(),
            mod_preview: ptr::null_mut(),
            installed_list: ptr::null_mut(),
            mod_book: ptr::null_mut(),
            mod_install: ptr::null_mut(),
            mod_selection: ptr::null_mut(),
            installed_count: ptr::null_mut(),
            mod_manage: Vec::new(),
            mod_count: ptr::null_mut(),
            catalog_items: Vec::new(),
            installed_items: Vec::new(),
            player,
            server_rows: Vec::new(),
            server_combo: ptr::null_mut(),
            nickname_entry: ptr::null_mut(),
            nickname_controls: ptr::null_mut(),
            player_title: ptr::null_mut(),
            game_memory: ptr::null_mut(),
            game_checks: Vec::new(),
            player_controls: ptr::null_mut(),
            player_hint: ptr::null_mut(),
            player_dirty: true,
            player_changed_at: g_get_monotonic_time(),
            updating_player: false,
            player_signal: 0,
            capture: capture_output.map(|output| Capture {
                output,
                page: 0,
                error: None,
            }),
            update_label: label(&format!("ALOL {}", env!("CARGO_PKG_VERSION")), "caption"),
            update_button: ptr::null_mut(),
            update_receiver: None,
            update_release: None,
            update_installed: false,
            executable: std::env::current_exe().map_err(|e| e.to_string())?,
            update_dialog: ptr::null_mut(),
            update_dialog_status: ptr::null_mut(),
            update_prompt_pending: false,
            update_manual: false,
        });
        let about = app.button(header, "О проекте", Action::About, false);
        gtk_widget_set_name(about, c"alol-about-button".as_ptr());
        for (index, (title, icon)) in [
            ("Играть", "play"),
            ("Настройки", "tune"),
            ("Моды", "grid"),
            ("Журнал", "notes"),
        ]
        .iter()
        .enumerate()
        {
            let button = app.button(
                navigation_items,
                title,
                Action::Navigate(index as c_int),
                false,
            );
            style(button, "navigation-item");
            gtk_widget_set_name(button, c(&format!("alol-nav-{index}")).as_ptr());
            gtk_widget_set_tooltip_text(button, c(title).as_ptr());
            gtk_button_set_image(button, symbol(icon, "#e8def8", 24));
            gtk_button_set_always_show_image(button, 1);
            gtk_button_set_image_position(button, 2);
            app.chrome.buttons.push(button);
        }
        app.chrome.resize(width);
        app.chrome.select(0);
        let chrome_pointer = (&*app.chrome as *const Chrome).cast_mut().cast();
        g_signal_connect_data(
            notebook,
            c"switch-page".as_ptr(),
            Some(std::mem::transmute::<
                unsafe extern "C" fn(W, W, c_uint, W),
                unsafe extern "C" fn(),
            >(navigation_changed)),
            chrome_pointer,
            ptr::null_mut(),
            0,
        );
        g_signal_connect_data(
            window,
            c"size-allocate".as_ptr(),
            Some(std::mem::transmute::<
                unsafe extern "C" fn(W, *const Rectangle, W),
                unsafe extern "C" fn(),
            >(layout_changed)),
            chrome_pointer,
            ptr::null_mut(),
            0,
        );
        app.log.pause_button = app.button(log_search_row, "Пауза", Action::LogPause, false);
        gtk_widget_set_name(app.log.pause_button, c"alol-log-pause".as_ptr());
        let log_actions = box_new(false, 8);
        style(log_actions, "toolbar");
        app.button(log_actions, "Копировать", Action::CopyLog, false);
        let clear = app.button(log_actions, "Очистить", Action::LogClear, false);
        style(clear, "destructive");
        gtk_widget_set_name(clear, c"alol-log-clear".as_ptr());
        app.button(log_actions, "Папка журналов", Action::LogFolder, false);
        app.button(log_actions, "Диагностика", Action::Work("doctor"), false);
        pack(log_page, log_actions, false);
        app.connect_action(log_search, "changed", Action::LogFilter);
        for widget in [log_level, log_session] {
            app.connect_action(widget, "changed", Action::LogFilter);
        }
        app.connect_action(log_follow, "notify::active", Action::LogFilter);
        let update_column = box_new(true, 4);
        gtk_label_set_max_width_chars(app.update_label, 32);
        pack(update_column, app.update_label, false);
        app.update_button =
            app.button(update_column, "Проверить обновления", Action::Update, false);
        // Version and update actions live with update preferences, keeping the app bar quiet.
        let player_controls = app.build_player_controls()?;
        pack(home, player_controls, false);
        let play_actions = box_new(true, 8);
        let project_row = box_new(false, 10);
        style(project_row, "actions");
        style(project_row, "project-group");
        gtk_widget_set_name(project_row, c"alol-projects".as_ptr());
        for project in [
            crate::project::Project::Arizona,
            crate::project::Project::Rodina,
        ] {
            let widget = app.button(project_row, project.name(), Action::Project(project), false);
            gtk_widget_set_name(
                widget,
                c(&format!("alol-project-{}", project.id())).as_ptr(),
            );
            style(widget, "project-button");
            gtk_button_set_image(
                widget,
                symbol(
                    if project == crate::project::Project::Arizona {
                        "sun"
                    } else {
                        "home"
                    },
                    "#e8def8",
                    22,
                ),
            );
            gtk_button_set_always_show_image(widget, 1);
            app.project_buttons.push((project, widget));
        }
        pack(outer, project_row, false);
        gtk_box_reorder_child(outer, project_row, 1);
        app.play = app.button(play_actions, "Установить Arizona", Action::Play, true);
        style(app.play, "play-button");
        let secondary = if width < 640 {
            let row = box_new(true, 8);
            style(row, "actions");
            row
        } else {
            action_row()
        };
        app.update_game = app.button(secondary, "Обновить игру", Action::Work("launch"), false);
        app.game_folder = app.button(secondary, "Папка игры", Action::Folder, false);
        app.button(secondary, "Помощь", Action::Help, false);
        pack(play_actions, secondary, false);
        let maintenance = box_new(true, 10);
        let expander = gtk_expander_new(c"Установка и восстановление".as_ptr());
        app.help_expander = expander;
        gtk_widget_set_name(expander, c"alol-help".as_ptr());
        gtk_expander_set_expanded(expander, 0);
        gtk_container_add(expander, maintenance);
        style(maintenance, "help-content");
        let installer_row = action_row();
        app.button(
            installer_row,
            "Выбрать установщик .exe…",
            Action::ChooseInstaller,
            false,
        );
        app.button(installer_row, "Сайт выбранной игры", Action::Website, false);
        pack(maintenance, installer_row, false);
        pack(
            maintenance,
            label(
                "Скачайте .exe с сайта Arizona и выберите файл.\nProton, шрифты и Visual C++ устанавливаются автоматически.",
                "caption",
            ),
            false,
        );
        pack(
            maintenance,
            label("Если что-то не работает", "section-title"),
            false,
        );
        let row = action_row();
        app.button(
            row,
            "Подготовить Proton / Wine",
            Action::Work("prepare"),
            false,
        );
        pack(maintenance, row, false);
        let row = action_row();
        app.button(row, "Починить окружение", Action::Work("repair"), false);
        app.button(row, "Шрифты и Visual C++", Action::Work("libraries"), false);
        pack(maintenance, row, false);
        let row = action_row();
        app.button(
            row,
            "Сбросить графику GTA",
            Action::Work("reset-game"),
            false,
        );
        app.button(row, "Диагностика", Action::Work("doctor"), false);
        pack(maintenance, row, false);
        let row = action_row();
        app.button(
            row,
            "Починить запуск игры",
            Action::Work("repair-game"),
            false,
        );
        app.button(
            row,
            "Исправить русский текст",
            Action::Work("repair-language"),
            false,
        );
        pack(maintenance, row, false);
        pack(
            maintenance,
            label(
                "Починка сохраняет копии кэша. Сброс графики сохраняет копию настроек выбранной игры.\nПроверка и восстановление файлов игры доступны в официальном лаунчере.",
                "caption",
            ),
            false,
        );
        let home_scroller = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
        gtk_scrolled_window_set_policy(home_scroller, 2, 1);
        gtk_container_add(home_scroller, home);
        let home_container = box_new(true, 12);
        pack(home_container, home_scroller, true);
        pack(home_container, play_actions, false);
        notebook_page(notebook, home_container, "Играть", tab_sizes);
        let settings_container = box_new(true, 12);
        app.settings_page = settings_container;
        app.settings_book = gtk_notebook_new();
        style(app.settings_book, "secondary-tabs");
        gtk_widget_set_name(app.settings_book, c"alol-settings-book".as_ptr());
        for (body, title) in [
            (settings_page, "Игра"),
            (settings_launch, "Запуск"),
            (settings_launcher, "Лаунчер"),
            (settings_help, "Помощь"),
        ] {
            let scroll = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
            gtk_scrolled_window_set_policy(scroll, 2, 1);
            gtk_container_set_border_width(body, 4);
            gtk_container_add(scroll, body);
            notebook_page(app.settings_book, scroll, title, tab_sizes);
        }
        pack(settings_container, app.settings_book, true);
        notebook_page(notebook, settings_container, "Настройки", tab_sizes);
        let mods_page = app.build_mods();
        notebook_page(notebook, mods_page, "Моды", tab_sizes);
        notebook_page(notebook, log_page, "Журнал", tab_sizes);
        let appearance = settings_group(
            settings_launcher,
            "Оформление",
            "Тема применяется сразу и запоминается.",
        );
        let ui_theme = gtk_combo_box_text_new();
        gtk_widget_set_name(ui_theme, c"alol-ui-theme".as_ptr());
        gtk_combo_box_text_append_text(ui_theme, c"Тёмная тема".as_ptr());
        gtk_combo_box_text_append_text(ui_theme, c"Светлая тема".as_ptr());
        gtk_combo_box_set_active(ui_theme, if settings.ui_theme == "light" { 1 } else { 0 });
        pack(appearance, ui_theme, false);
        let performance = settings_group(
            settings_page,
            "Производительность",
            "Оптимизация запуска без изменения качества графики.",
        );
        let game_mode = check(performance, "Использовать GameMode", settings.game_mode);
        gtk_widget_set_name(game_mode, c"alol-game-mode".as_ptr());
        gtk_widget_set_tooltip_text(game_mode, c"GameMode временно применяет оптимизации CPU и планировщика на время игры. Текстуры, эффекты, разрешение и синхронизация не меняются. Прирост зависит от системы.".as_ptr());
        if !core::available("gamemoderun") {
            pack(
                performance,
                label(
                    "GameMode не установлен. Для использования нужен пакет gamemode; игра запускается и без него.",
                    "caption",
                ),
                false,
            );
        }
        let fps_hud = check(
            performance,
            "Показывать FPS и время кадра · DXVK",
            settings.fps_hud,
        );
        gtk_widget_set_name(fps_hud, c"alol-fps-hud".as_ptr());
        pack(
            performance,
            label(
                "Счётчик доступен при запуске через «Играть» с DXVK / Vulkan.",
                "caption",
            ),
            false,
        );
        let game_card = settings_group(settings_page, "Игра", "");
        pack(game_card, label("Память игры", "field-label"), false);
        app.game_memory = gtk_combo_box_text_new();
        gtk_widget_set_name(app.game_memory, c"alol-game-memory".as_ptr());
        for note in ["1 ГБ · экономно", "2 ГБ · рекомендуется", "3 ГБ", "4 ГБ"]
        {
            gtk_combo_box_text_append_text(app.game_memory, c(note).as_ptr());
        }
        gtk_combo_box_set_active(
            app.game_memory,
            [1024, 2048, 3072, 4096]
                .iter()
                .position(|m| *m == game_options.memory)
                .unwrap_or(1) as i32,
        );
        pack(game_card, app.game_memory, false);
        for option in game_options::OPTIONS
            .iter()
            .filter(|option| ["wideScreen", "windowed", "shitPc"].contains(&option.id))
        {
            let widget = check(game_card, option.label, game_options.enabled(option.id));
            gtk_widget_set_tooltip_text(widget, c(option.tooltip).as_ptr());
            gtk_widget_set_name(widget, c(&format!("alol-option-{}", option.id)).as_ptr());
            app.game_checks.push((option.id, widget));
        }
        let (_, graphics) = disclosure(game_card, "Дополнительная графика и интерфейс");
        for option in game_options::OPTIONS
            .iter()
            .filter(|option| !["wideScreen", "windowed", "shitPc"].contains(&option.id))
        {
            let widget = check(graphics, option.label, game_options.enabled(option.id));
            gtk_widget_set_tooltip_text(widget, c(option.tooltip).as_ptr());
            gtk_widget_set_name(widget, c(&format!("alol-option-{}", option.id)).as_ptr());
            app.game_checks.push((option.id, widget));
        }
        let compatibility = settings_group(
            settings_launch,
            "Совместимость",
            "Если всё работает, оставьте эти параметры как есть.",
        );
        let input_compatibility = check(
            compatibility,
            "Совместимый ввод клавиатуры · COSMIC / WASD",
            settings.input_compatibility,
        );
        gtk_widget_set_tooltip_text(input_compatibility, c"Использует XWayland, если доступен, и отключает перехват клавиш Linux IME. Перед игрой выберите английскую раскладку. Отключите для IME или нативного Wayland.".as_ptr());
        let launcher_compatibility = check(
            compatibility,
            "Совместимый запуск официального лаунчера",
            settings.launcher_compatibility,
        );
        gtk_widget_set_tooltip_text(launcher_compatibility, c"Убирает конфликтующие переменные dotfiles и оверлеев только для лаунчера и установщика; использует XWayland при доступном DISPLAY.".as_ptr());
        let disable_gpu = check(
            compatibility,
            "Не использовать GPU в официальном лаунчере",
            settings.disable_gpu,
        );
        gtk_widget_set_tooltip_text(
            disable_gpu,
            c"Помогает при пустом или белом окне Arizona. На графику самой игры не влияет."
                .as_ptr(),
        );
        let wined3d = check(
            compatibility,
            "Совместимая графика игры · OpenGL",
            settings.wined3d,
        );
        gtk_widget_set_tooltip_text(wined3d, c"Попробуйте, если игра не отображается или есть проблемы с Vulkan. Может снизить производительность.".as_ptr());
        let update_card = settings_group(settings_launcher, "Обновления ALOL", "");
        pack(update_card, update_column, false);
        pack(
            update_card,
            label("Когда проверять новую версию", "field-label"),
            false,
        );
        let update_policy = gtk_combo_box_text_new();
        gtk_widget_set_name(update_policy, c"alol-update-policy".as_ptr());
        for title in [
            "При каждом запуске",
            "Раз в сутки",
            "Раз в неделю",
            "Только вручную",
        ] {
            gtk_combo_box_text_append_text(update_policy, c(title).as_ptr());
        }
        gtk_combo_box_set_active(
            update_policy,
            ["startup", "daily", "weekly", "manual"]
                .iter()
                .position(|p| *p == settings.update_policy)
                .unwrap_or(0) as i32,
        );
        pack(update_card, update_policy, false);
        let update_notifications = check(
            update_card,
            "Сообщать, когда доступно обновление",
            settings.update_notifications,
        );
        gtk_widget_set_name(update_notifications, c"alol-update-notifications".as_ptr());
        pack(
            update_card,
            label("Эти два параметра сохраняются сразу.", "caption"),
            false,
        );
        let advanced_card = settings_group(
            settings_launch,
            "Дополнительно",
            "Пути и параметры запуска обычно определяются автоматически.",
        );
        let (_, advanced) = disclosure(advanced_card, "Окружение и пути к файлам");
        pack(advanced, label("Запускать через", "field-label"), false);
        let runner = gtk_combo_box_text_new();
        gtk_combo_box_text_append_text(runner, c"Proton · рекомендуется".as_ptr());
        gtk_combo_box_text_append_text(runner, c"Wine · отдельное окружение".as_ptr());
        gtk_combo_box_set_active(runner, if settings.runner == "wine" { 1 } else { 0 });
        pack(advanced, runner, false);
        pack(advanced, label("Сборка Proton", "field-label"), false);
        let combo = gtk_combo_box_text_new_with_entry();
        for item in ["GE-Proton", "UMU-Proton", "ALOL-Proton"] {
            gtk_combo_box_text_append_text(combo, c(item).as_ptr());
        }
        for path in core::proton_versions() {
            gtk_combo_box_text_append_text(combo, c(&path.to_string_lossy()).as_ptr());
        }
        let proton = gtk_bin_get_child(combo);
        gtk_entry_set_text(proton, c(&settings.proton).as_ptr());
        pack(advanced, combo, false);
        app.button(
            advanced,
            "Создать и выбрать ALOL-Proton",
            Action::RuntimePrepare,
            false,
        );
        pack(
            advanced,
            label(
                "Наш профиль Arizona и исправление индикатора EN / RU в одной сборке. Подготовка скачает готовые проверенные файлы; компиляция Wine не нужна. Требуется совместимость клавиатуры / XWayland. Старые сборки сохраняются.",
                "caption",
            ),
            false,
        );
        pack(
            advanced,
            label(
                "Сборка скачивается при первом запуске. Для её обновления снова выберите GE-Proton.",
                "caption",
            ),
            false,
        );
        pack(advanced, label("Команда Wine", "field-label"), false);
        let wine = entry(&settings.wine);
        gtk_widget_set_name(wine, c"alol-wine-command".as_ptr());
        pack(advanced, wine, false);
        let installer = app.file_field(
            advanced,
            "Установщик Arizona · .exe",
            &settings.installer,
            false,
        );
        let executable = app.file_field(
            advanced,
            "Официальный лаунчер · можно оставить пустым",
            &settings.executable,
            false,
        );
        gtk_entry_set_placeholder_text(executable, c"Найти автоматически".as_ptr());
        let prefix = app.file_field(
            advanced,
            "Где хранить окружения игры",
            &settings.prefix_root,
            true,
        );
        let discrete_gpu = check(
            advanced,
            "Использовать второй GPU · Mesa",
            settings.discrete_gpu,
        );
        let no_sync = check(advanced, "Отключить Esync / Fsync", settings.no_sync);
        let debug = check(advanced, "Подробный журнал Wine / Proton", settings.debug);
        app.button(
            advanced,
            "Открыть настройки Wine",
            Action::Work("winecfg"),
            false,
        );
        let help_card = settings_group(
            settings_help,
            "Помощь",
            "Установка, восстановление и проверка окружения без удаления игры.",
        );
        pack(help_card, expander, false);
        app.button(
            help_card,
            "Сбросить кэш лаунчера с резервной копией",
            Action::Work("clean-cache"),
            false,
        );
        pack(
            help_card,
            label(
                "Для пустого или повреждённого интерфейса Arizona. Моды, настройки и шейдеры сохраняются. На FPS игры очистка этого кэша не влияет.",
                "caption",
            ),
            false,
        );
        app.fields = Fields {
            ui_theme,
            runner,
            proton,
            wine,
            installer,
            executable,
            prefix,
            wined3d,
            disable_gpu,
            discrete_gpu,
            debug,
            no_sync,
            launcher_compatibility,
            input_compatibility,
            fps_hud,
            game_mode,
            update_policy,
            update_notifications,
        };
        app.connect_action(ui_theme, "changed", Action::Appearance);
        app.connect_action(update_policy, "changed", Action::UpdatePreferences);
        app.connect_action(
            update_notifications,
            "notify::active",
            Action::UpdatePreferences,
        );
        app.connect_action(runner, "changed", Action::SettingsChanged);
        app.connect_action(app.game_memory, "changed", Action::QuickPreferences);
        for widget in [proton, wine, installer, executable, prefix] {
            app.connect_action(widget, "changed", Action::SettingsChanged);
        }
        let mut toggles = vec![
            wined3d,
            disable_gpu,
            launcher_compatibility,
            input_compatibility,
            fps_hud,
            game_mode,
            discrete_gpu,
            no_sync,
            debug,
        ];
        toggles.extend(app.game_checks.iter().map(|(_, w)| *w));
        for widget in toggles {
            app.connect_action(widget, "notify::active", Action::QuickPreferences);
        }
        let save_row = box_new(false, 12);
        app.settings_save_bar = save_row;
        style(save_row, "save-bar");
        gtk_label_set_max_width_chars(app.settings_hint, 48);
        pack(save_row, app.settings_hint, true);
        app.button(save_row, "Сохранить настройки", Action::Save, true);
        gtk_widget_show_all(save_row);
        gtk_widget_set_no_show_all(save_row, 1);
        gtk_widget_set_visible(save_row, 0);
        pack(settings_container, save_row, false);
        style(footer, "status-bar");
        app.stop = app.button(footer, "Остановить", Action::Stop, false);
        gtk_widget_set_sensitive(app.stop, 0);
        gtk_widget_set_no_show_all(app.stop, 1);
        pack(outer, footer, false);
        let accelerators = gtk_accel_group_new();
        gtk_window_add_accel_group(window, accelerators);
        for (index, button) in app.chrome.buttons.iter().enumerate() {
            gtk_widget_add_accelerator(
                *button,
                c"clicked".as_ptr(),
                accelerators,
                b'1' as c_uint + index as c_uint,
                4,
                1,
            );
            gtk_widget_set_tooltip_text(
                *button,
                c(&format!(
                    "{} · Ctrl+{}",
                    ["Играть", "Настройки", "Моды", "Журнал"][index],
                    index + 1
                ))
                .as_ptr(),
            );
        }
        gtk_widget_add_accelerator(app.play, c"clicked".as_ptr(), accelerators, 0xff0d, 4, 1);
        gtk_widget_set_tooltip_text(app.play, c"Запустить · Ctrl+Enter".as_ptr());
        g_object_unref(accelerators);
        app.refresh();
        app.reload_mods();
        app.tick();
        g_signal_connect_data(
            window,
            c"delete-event".as_ptr(),
            Some(std::mem::transmute::<
                unsafe extern "C" fn(W, W, W) -> c_int,
                unsafe extern "C" fn(),
            >(close_requested)),
            (&mut *app as *mut App).cast(),
            ptr::null_mut(),
            0,
        );
        let timer = g_timeout_add(400, Some(tick), (&mut *app as *mut App).cast());
        gtk_widget_set_name(window, c"alol".as_ptr());
        gtk_widget_show_all(window);
        app.load_project_controls()?;
        app.apply_appearance();
        app.chrome.compact.set(width >= 840);
        app.chrome.resize(width);
        app.refresh();
        if app.capture.is_some() {
            set_label(app.status, "Демонстрационный профиль · Nick_Name");
        }
        // Allow GTK to map and allocate each page before drawing it.
        let capture_timer = if app.capture.is_some() {
            g_timeout_add(900, Some(capture_tick), (&mut *app as *mut App).cast())
        } else {
            0
        };
        if std::env::args().any(|a| a == "--smoke-test") {
            g_timeout_add(1400, Some(smoke_quit), ptr::null_mut());
        }
        if app.capture.is_none()
            && app.settings.update_policy != "manual"
            && !std::env::args().any(|a| a == "--smoke-test")
        {
            app.check_updates(false);
        }
        gtk_main();
        g_source_remove(timer);
        if capture_timer != 0
            && app
                .capture
                .as_ref()
                .is_some_and(|c| c.page < 3 && c.error.is_none())
        {
            g_source_remove(capture_timer);
        }
        if app.player_signal != 0 {
            g_signal_handler_block(app.server_combo, app.player_signal);
        }
        // Keep worker alive if the window is closed: the game and downloads continue.
        // GTK owns the widgets; destroy them before dropping callback storage.
        gtk_widget_destroy(window);
        g_object_unref(app.theme_provider);
        g_object_unref(app.utility_buttons);
        g_object_unref(tab_sizes);
        if let Some(capture) = &app.capture {
            if let Some(error) = &capture.error {
                return Err(error.clone());
            }
            if capture.page != 3 {
                return Err("Съёмка прервана до сохранения всех трёх экранов".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod window_tests {
    use super::*;
    #[test]
    fn initial_size_fits_smaller_workareas() {
        assert_eq!(bounded_size(860, 760, 1920, 1080), (860, 760));
        assert_eq!(bounded_size(860, 760, 1280, 720), (860, 624));
        assert_eq!(bounded_size(860, 760, 640, 480), (576, 384));
    }
}
