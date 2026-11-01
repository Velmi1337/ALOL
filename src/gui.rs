//! Small GTK 3 ABI surface. Widgets and signal data live on GTK's main thread.
//! No webview, background polling loop, Cargo dependencies, or bundled toolkit.
use crate::core::{self, Result, Settings};
use crate::game::{self, PlayerProfile, Server};
use crate::game_options::{self, GameOptions};
use crate::mods::{self, CatalogMod, InstalledMod};
use crate::updates::{self, Release};
use std::{
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
    fn gtk_box_new(orientation: c_int, spacing: c_int) -> W;
    fn gtk_box_set_homogeneous(container: W, homogeneous: c_int);
    fn gtk_box_pack_start(container: W, child: W, expand: c_int, fill: c_int, padding: c_uint);
    fn gtk_container_add(container: W, child: W);
    fn gtk_container_child_set(container: W, child: W, first_property: *const c_char, ...);
    fn gtk_container_set_border_width(container: W, width: c_uint);
    fn gtk_widget_show_all(widget: W);
    fn gtk_widget_destroy(widget: W);
    fn gtk_widget_set_name(widget: W, name: *const c_char);
    fn gtk_widget_set_sensitive(widget: W, sensitive: c_int);
    fn gtk_widget_set_tooltip_text(widget: W, text: *const c_char);
    fn gtk_widget_set_size_request(widget: W, width: c_int, height: c_int);
    fn gtk_widget_set_hexpand(widget: W, expand: c_int);
    fn gtk_widget_set_valign(widget: W, align: c_int);
    fn gtk_widget_get_allocated_width(widget: W) -> c_int;
    fn gtk_widget_get_allocated_height(widget: W) -> c_int;
    fn gtk_widget_draw(widget: W, cairo: W);
    fn gtk_widget_get_style_context(widget: W) -> W;
    fn gtk_style_context_add_class(context: W, class: *const c_char);
    fn gtk_style_context_has_class(context: W, class: *const c_char) -> c_int;
    fn gtk_size_group_new(mode: c_int) -> W;
    fn gtk_size_group_add_widget(group: W, widget: W);
    fn gtk_label_new(text: *const c_char) -> W;
    fn gtk_label_set_text(label: W, text: *const c_char);
    fn gtk_label_set_xalign(label: W, align: f32);
    fn gtk_label_set_line_wrap(label: W, wrap: c_int);
    fn gtk_button_new_with_label(label: *const c_char) -> W;
    fn gtk_button_set_label(button: W, label: *const c_char);
    fn gtk_entry_new() -> W;
    fn gtk_entry_set_text(entry: W, text: *const c_char);
    fn gtk_entry_get_text(entry: W) -> *const c_char;
    fn gtk_entry_set_placeholder_text(entry: W, text: *const c_char);
    fn gtk_check_button_new_with_label(text: *const c_char) -> W;
    fn gtk_toggle_button_set_active(button: W, active: c_int);
    fn gtk_toggle_button_get_active(button: W) -> c_int;
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
    fn gtk_scrolled_window_new(hadjustment: W, vadjustment: W) -> W;
    fn gtk_scrolled_window_set_policy(window: W, horizontal: c_int, vertical: c_int);
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
    fn gtk_separator_new(orientation: c_int) -> W;
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
}
#[link(name = "gdk-3")]
unsafe extern "C" {
    fn gdk_screen_get_default() -> W;
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
        let w = gtk_check_button_new_with_label(c(text).as_ptr());
        gtk_toggle_button_set_active(w, active.into());
        pack(parent, w, false);
        w
    }
}
fn checked(w: W) -> bool {
    unsafe { gtk_toggle_button_get_active(w) != 0 }
}

#[derive(Clone, Copy)]
enum Action {
    Work(&'static str),
    Play,
    Save,
    Folder,
    Stop,
    Browse(W, bool),
    ChooseInstaller,
    Website,
    ModFilter,
    ModInstall,
    ModFile,
    ModImport,
    ModManage(&'static str),
    PlayerChanged,
    Update,
}
struct Signal {
    app: *mut App,
    action: Action,
}
struct Fields {
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
}
struct App {
    settings: Settings,
    window: W,
    notebook: W,
    status: W,
    runtime: W,
    detail: W,
    play: W,
    spinner: W,
    stop: W,
    settings_page: W,
    log_buffer: W,
    fields: Fields,
    buttons: Vec<W>,
    utility_buttons: W,
    signals: Vec<SignalHolder>,
    child: Option<Child>,
    action: String,
    log_text: String,
    stop_at: Option<i64>,
    mods_page: W,
    mod_list: W,
    mod_search: W,
    mod_file: W,
    mod_target: W,
    installed_combo: W,
    mod_count: W,
    catalog_items: Vec<CatalogMod>,
    installed_items: Vec<InstalledMod>,
    player: PlayerProfile,
    server_rows: Vec<Server>,
    server_combo: W,
    nickname_entry: W,
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
}
struct Capture {
    output: std::path::PathBuf,
    page: usize,
    error: Option<String>,
}
// Heap allocations keep signal pointers stable as the vector grows.
struct SignalHolder(Box<Signal>);

impl App {
    fn button(&mut self, parent: W, title: &str, action: Action, primary: bool) -> W {
        unsafe {
            let button = gtk_button_new_with_label(c(title).as_ptr());
            if primary {
                style(button, "primary");
            }
            gtk_widget_set_valign(button, 3); // GTK_ALIGN_CENTER
            gtk_widget_set_size_request(button, -1, 44);
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
        };
        s.validate()?;
        Ok(s)
    }

    fn save(&mut self) -> Result<()> {
        let s = self.collect()?;
        s.save()?;
        self.settings = s;
        let selected = unsafe { gtk_combo_box_get_active(self.game_memory) };
        let memory = *[1024, 2048, 3072, 4096]
            .get(selected as usize)
            .ok_or("Выберите память игры")?;
        GameOptions {
            memory,
            values: self
                .game_checks
                .iter()
                .map(|(id, w)| ((*id).into(), checked(*w)))
                .collect(),
        }
        .save()?;
        self.persist_player(true)?;
        self.refresh();
        Ok(())
    }

    fn refresh(&self) {
        let s = &self.settings;
        let installed = s.find_executable();
        let game_ready = game::game_executable(s).is_ok();
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
                "Выбери сервер и ник. ALOL запускает игру напрямую и сохраняет твой выбор."
            } else if installed.is_some() {
                "Сначала скачайте файлы игры через «Обновить игру». Затем подключайтесь прямо из ALOL."
            } else if prepared {
                "Окружение готово. Установите Arizona из выбранного .exe."
            } else {
                "Начните с установки: ALOL подготовит Proton и запустит установщик Arizona."
            },
        );
        unsafe {
            gtk_button_set_label(
                self.play,
                c(if game_ready {
                    "Играть  →"
                } else if installed.is_some() {
                    "Скачать игру  →"
                } else {
                    "Установить Arizona  →"
                })
                .as_ptr(),
            );
        }
        if self.child.is_none() {
            set_label(
                self.status,
                if installed.is_some() {
                    "Готов к запуску"
                } else if prepared {
                    "Можно установить Arizona"
                } else {
                    "Требуется установка"
                },
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
        let file = core::open_log()?;
        let stderr = file.try_clone().map_err(|e| e.to_string())?;
        let mut cmd = Command::new(&self.executable);
        cmd.arg(action)
            .args(args)
            .stdout(Stdio::from(file))
            .stderr(Stdio::from(stderr))
            .stdin(Stdio::null())
            .process_group(0);
        self.child = Some(cmd.spawn().map_err(|e| e.to_string())?);
        self.action = action.into();
        self.stop_at = None;
        set_label(
            self.status,
            match action {
                "prepare" => "Подготовка окружения, шрифтов и Visual C++…",
                "install" => "Установка Arizona и необходимых компонентов…",
                "launch" => "Arizona работает · закройте её для завершения",
                "play" => "Запуск игры на выбранном сервере…",
                "servers-refresh" => "Обновление списка серверов Arizona…",
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
                } else if let Some(release) = &self.update_release {
                    let version = release.version.clone();
                    self.start_args("self-update", &[version])?;
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
            Action::ModImport => {
                let file = entry_text(self.mod_file);
                let selected = unsafe { gtk_combo_box_get_active(self.mod_target) };
                let mode = ["auto", "root", "modloader", "cleo", "moonloader"]
                    .get(selected as usize)
                    .copied()
                    .unwrap_or("auto");
                self.start_args("mods-add", &[file, mode.into()])?;
            }
            Action::ModManage(operation) => {
                let index = unsafe { gtk_combo_box_get_active(self.installed_combo) };
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
                        "Установщик выбран. Нажмите «Установить Arizona».",
                    );
                }
            }
            Action::Website => {
                Command::new("xdg-open")
                    .arg("https://arizona-rp.com/")
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
        self.server_rows = game::servers()?;
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
        let selected = unsafe { gtk_combo_box_get_active(self.server_combo) };
        let text = self
            .server_rows
            .get(selected as usize)
            .map(|s| {
                format!(
                    "{}:{} · ник сохраняется отдельно для этого сервера",
                    s.host, s.port
                )
            })
            .unwrap_or_else(|| "Выберите сервер или обновите список.".into());
        set_label(self.player_hint, &text);
    }

    fn persist_player(&mut self, force: bool) -> Result<()> {
        if self.nickname_entry.is_null() || self.updating_player {
            return Ok(());
        }
        let typed = entry_text(self.nickname_entry);
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
        } else if typed != self.player.nickname {
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
        let controls = box_new(true, 8);
        self.player_controls = controls;
        pack(controls, label("СЕРВЕР И ИГРОК", "field-label"), false);
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
        let row = box_new(false, 10);
        pack(row, label("Никнейм", "field-label"), false);
        self.nickname_entry = entry(&self.player.nickname);
        unsafe {
            gtk_entry_set_placeholder_text(self.nickname_entry, c"Nick_Name".as_ptr());
        }
        pack(row, self.nickname_entry, true);
        pack(controls, row, false);
        self.player_hint = label("", "caption");
        pack(controls, self.player_hint, false);
        self.reload_server_choices()?;
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
            gtk_container_set_border_width(page, 12);
            self.mods_page = page;
            pack(page, label("Моды Arizona", "alol-title"), false);
            pack(
                page,
                label(
                    "Официальный каталог Arizona и ваши скачанные моды. Перед установкой закройте игру и официальный лаунчер.",
                    "caption",
                ),
                false,
            );
            let search_row = box_new(false, 10);
            self.mod_search = entry("");
            gtk_entry_set_placeholder_text(
                self.mod_search,
                c"Поиск по названию и описанию".as_ptr(),
            );
            pack(search_row, self.mod_search, true);
            self.button(search_row, "Найти", Action::ModFilter, false);
            self.button(
                search_row,
                "Обновить каталог",
                Action::Work("mods-refresh"),
                false,
            );
            pack(page, search_row, false);
            self.mod_count = label(
                "Нажмите «Обновить каталог», чтобы получить моды Arizona.",
                "caption",
            );
            pack(page, self.mod_count, false);
            self.mod_list = gtk_list_box_new();
            let scroller = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
            gtk_scrolled_window_set_policy(scroller, 2, 1);
            gtk_widget_set_size_request(scroller, -1, 160);
            gtk_container_add(scroller, self.mod_list);
            pack(page, scroller, true);
            self.button(
                page,
                "Установить выбранный официальный мод",
                Action::ModInstall,
                true,
            );
            pack(page, gtk_separator_new(0), false);
            pack(page, label("Добавить свой мод", "field-label"), false);
            let row = box_new(false, 10);
            self.mod_file = entry("");
            gtk_entry_set_placeholder_text(self.mod_file, c"ZIP, RAR, 7z, ASI, CS, Lua…".as_ptr());
            pack(row, self.mod_file, true);
            self.button(row, "Файл…", Action::ModFile, false);
            pack(page, row, false);
            let row = box_new(false, 10);
            self.mod_target = gtk_combo_box_text_new();
            for text in [
                "Куда: автоматически",
                "Корень игры",
                "Mod Loader",
                "CLEO",
                "MoonLoader",
            ] {
                gtk_combo_box_text_append_text(self.mod_target, c(text).as_ptr());
            }
            gtk_combo_box_set_active(self.mod_target, 0);
            pack(row, self.mod_target, true);
            self.button(row, "Добавить мод", Action::ModImport, false);
            pack(page, row, false);
            pack(
                page,
                label(
                    "Для Lua нужен MoonLoader; для CS — CLEO. Структура готовых архивов сохраняется.",
                    "caption",
                ),
                false,
            );
            pack(
                page,
                label("Установленные через ALOL", "field-label"),
                false,
            );
            self.installed_combo = gtk_combo_box_text_new();
            pack(page, self.installed_combo, false);
            let row = action_row();
            for (name, op) in [
                ("Включить", "mods-enable"),
                ("Отключить", "mods-disable"),
                ("Удалить", "mods-remove"),
            ] {
                self.button(row, name, Action::ModManage(op), false);
            }
            pack(page, row, false);
            pack(
                page,
                label(
                    "Заменённые файлы сохраняются в резервных копиях. При отключении мода ALOL восстанавливает их.",
                    "caption",
                ),
                false,
            );
            page
        }
    }

    fn reload_mods(&mut self) {
        if self.mod_list.is_null() {
            return;
        }
        let query = entry_text(self.mod_search).to_lowercase();
        let all = match mods::catalog() {
            Ok(v) => v,
            Err(e) => {
                set_label(self.mod_count, &e);
                Vec::new()
            }
        };
        self.catalog_items = all
            .into_iter()
            .filter(|m| {
                format!("{} {} {}", m.name, m.description, m.category)
                    .to_lowercase()
                    .contains(&query)
            })
            .collect();
        self.installed_items = mods::installed(&self.settings).unwrap_or_default();
        unsafe {
            loop {
                let row = gtk_list_box_get_row_at_index(self.mod_list, 0);
                if row.is_null() {
                    break;
                }
                gtk_widget_destroy(row);
            }
            for m in &self.catalog_items {
                let row = box_new(true, 3);
                gtk_container_set_border_width(row, 10);
                let active = self
                    .installed_items
                    .iter()
                    .find(|i| i.id == format!("official-{}", m.id));
                let status = active
                    .map(|i| {
                        if i.active {
                            " · включён"
                        } else {
                            " · отключён"
                        }
                    })
                    .unwrap_or("");
                pack(
                    row,
                    label(
                        &format!("{}  {}{}", m.name, m.version, status),
                        "field-label",
                    ),
                    false,
                );
                let description = m.description.chars().take(700).collect::<String>();
                if !description.is_empty() {
                    pack(row, label(&description, "caption"), false);
                }
                gtk_list_box_insert(self.mod_list, row, -1);
            }
            let row = gtk_list_box_get_row_at_index(self.mod_list, 0);
            if !row.is_null() {
                gtk_list_box_select_row(self.mod_list, row);
            }
            gtk_widget_show_all(self.mod_list);
            gtk_combo_box_text_remove_all(self.installed_combo);
            for m in &self.installed_items {
                let source = if m.source == "official" {
                    "Arizona"
                } else {
                    "свой"
                };
                gtk_combo_box_text_append_text(
                    self.installed_combo,
                    c(&format!(
                        "{} {} · {} · {}",
                        m.name,
                        m.version,
                        source,
                        if m.active {
                            "включён"
                        } else {
                            "отключён"
                        }
                    ))
                    .as_ptr(),
                );
            }
            if !self.installed_items.is_empty() {
                gtk_combo_box_set_active(self.installed_combo, 0);
            }
        }
        set_label(
            self.mod_count,
            if self.catalog_items.is_empty() {
                "Каталог пуст или моды не найдены. Нажмите «Обновить каталог»."
            } else {
                "Выберите мод из официального каталога Arizona."
            },
        );
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
                    }
                    if let Ok(s) = Settings::load() {
                        unsafe {
                            gtk_entry_set_text(self.fields.proton, c(&s.proton).as_ptr());
                        }
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
                        gtk_spinner_stop(self.spinner);
                        if !status.success() && !stopped {
                            gtk_notebook_set_current_page(self.notebook, 3);
                        }
                    }
                }
                Err(e) => set_label(self.status, &format!("Ошибка ожидания: {e}")),
                Ok(None) => (),
            }
        }
        let log = core::tail_log();
        if log != self.log_text {
            unsafe {
                gtk_text_buffer_set_text(self.log_buffer, c(&log).as_ptr(), -1);
            }
            self.log_text = log;
        }
    }

    fn check_updates(&mut self, force: bool) {
        if self.update_receiver.is_some() || self.update_installed {
            return;
        }
        self.update_release = None;
        set_label(self.update_label, "Проверка обновлений ALOL…");
        unsafe {
            gtk_widget_set_sensitive(self.update_button, 0);
        }
        let (sender, receiver) = mpsc::channel();
        self.update_receiver = Some(receiver);
        // One bounded check per launch or button click; GTK is only touched by the main thread.
        std::thread::spawn(move || {
            let _ = sender.send(updates::check(force));
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
}

unsafe extern "C" fn clicked(_: W, data: W) {
    // SAFETY: SignalHolder and App outlive GTK's main loop; callbacks run there only.
    let signal = unsafe { &*(data as *const Signal) };
    let app = unsafe { &mut *signal.app };
    if let Err(error) = app.dispatch(signal.action) {
        set_label(app.status, &error);
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
@define-color theme_bg_color #12131a;
@define-color theme_fg_color #ebedf5;
@define-color theme_base_color #1c1e2a;
@define-color theme_text_color #ebedf5;
@define-color theme_selected_bg_color #544475;
@define-color theme_selected_fg_color #ffffff;
@define-color insensitive_fg_color #858a9f;
window { background: #12131a; color: #ebedf5; }
* { font-family: Sans; font-size: 14px; }
.brand { font-size: 34px; font-weight: 900; letter-spacing: 3px; color: #bcacff; }
.caption { color: #9a9eaf; font-size: 12px; }
.alol-title { font-size: 30px; font-weight: 800; }
.body { color: #b2b6c9; }
.runtime { color: #c8bbff; font-size: 13px; }
.field-label { font-size: 12px; color: #b8bcce; margin-top: 6px; }
.card { background: #1c1e2a; border-radius: 16px; padding: 18px; }
button { background: #292c3c; color: #e4e6f1; border: 1px solid #3a3e54; border-radius: 10px; min-height: 20px; padding: 11px 16px; box-shadow: none; text-shadow: none; transition: background-color 120ms ease, border-color 120ms ease; }
button:hover { background: #35394d; border-color: #646281; }
button:active { background: #232638; }
button:focus { border-color: #bcacff; box-shadow: 0 0 0 2px alpha(#bcacff, 0.18); }
button.primary { background: #aa91ff; color: #171223; border-color: #aa91ff; font-weight: 700; }
button.primary:hover { background: #c0adff; border-color: #c0adff; }
button.primary:active { background: #9b80ef; }
button:disabled { opacity: 0.45; }
entry, combobox entry { background: #1c1e2a; color: #eceef6; border: 1px solid #383c50; border-radius: 10px; min-height: 20px; padding: 11px 12px; }
entry:focus { border-color: #aa91ff; }
notebook, notebook > stack { background: transparent; border: none; }
notebook > header { background: transparent; border: none; }
notebook > header tabs tab { background: transparent; color: #a0a5b9; border: none; border-bottom: 2px solid transparent; padding: 12px 18px; }
notebook > header tabs tab:checked { color: #d1c4ff; border-bottom: 2px solid #aa91ff; }
textview, textview text { background: #171923; color: #bfc8dc; font-family: monospace; font-size: 12px; }
checkbutton { color: #c7cbd9; padding: 4px 0; }
check { background: #303448; border: 1px solid #555b77; }
check:checked { background: #aa91ff; color: #171223; }
separator { background: #2d3041; min-height: 1px; }
scrolledwindow { border: none; }
list { background: #1c1e2a; color: #ebedf5; }
list row { background: transparent; border-bottom: 1px solid #2d3041; }
list row:selected { background: #35304c; }
list row:hover { background: #292c3c; }
/* Popup windows have their own CSS root: window#alol selectors cannot reach them. */
menu, .menu, .context-menu, popover, popover.background, window.popup {
    background: #1c1e2a; color: #ebedf5; border: 1px solid #555b77;
}
menuitem { background: transparent; color: #ebedf5; padding: 8px 12px; }
menuitem label, menuitem cellview, combobox cellview { color: inherit; }
menuitem:hover, menuitem:selected, menuitem:focus { background: #544475; color: #ffffff; }
menuitem:disabled { color: #858a9f; }
menu arrow { color: #bcacff; background: #292c3c; min-height: 16px; }
combobox arrow, expander arrow { color: #bcacff; }
expander > title { color: #c7cbd9; padding: 8px 0; }
expander > title:hover { color: #ffffff; }
headerbar, .titlebar { background: #1c1e2a; color: #ebedf5; border-color: #383c50; }
headerbar .title, .titlebar .title { font-size: 14px; font-weight: 600; color: #ebedf5; }
headerbar .subtitle, .titlebar .subtitle { color: #b8bcce; }
button.titlebutton { padding: 4px; min-height: 24px; min-width: 24px; }
dialog, messagedialog, filechooser, filechooser .view, treeview.view {
    background: #1c1e2a; color: #ebedf5;
}
treeview.view:selected, treeview.view:selected:focus { background: #544475; color: #ffffff; }
treeview header button { background: #292c3c; color: #ebedf5; }
tooltip, tooltip.background { background: #292c3c; color: #ebedf5; border: 1px solid #555b77; }
scrollbar { background: #171923; }
scrollbar slider { background: #555b77; border-radius: 8px; min-width: 8px; min-height: 8px; }
scrollbar slider:hover { background: #8580ab; }
"#;

pub fn run() -> Result<()> {
    run_mode(None)
}
pub(crate) fn run_mode(capture_output: Option<std::path::PathBuf>) -> Result<()> {
    let settings = Settings::load()?;
    let player = PlayerProfile::load(&settings)?;
    let game_options = GameOptions::load(&settings)?;
    unsafe {
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
        gtk_css_provider_load_from_data(provider, c(CSS).as_ptr(), -1, ptr::null_mut());
        gtk_style_context_add_provider_for_screen(gdk_screen_get_default(), provider, 600);
        g_object_unref(provider);
        let window = gtk_window_new(0);
        gtk_window_set_title(window, c"ALOL — Arizona Launcher On Linux".as_ptr());
        gtk_window_set_default_size(window, 800, 760);
        gtk_window_set_position(window, 1);
        gtk_window_set_icon_name(window, c"alol".as_ptr());
        let outer = box_new(true, 16);
        gtk_container_set_border_width(outer, 26);
        gtk_container_add(window, outer);
        let header = box_new(false, 18);
        let brand = box_new(true, 1);
        pack(brand, label("ALOL", "brand"), false);
        pack(brand, label("ARIZONA LAUNCHER ON LINUX", "caption"), false);
        pack(header, brand, true);
        pack(outer, header, false);
        let notebook = gtk_notebook_new();
        let tab_sizes = gtk_size_group_new(1);
        pack(outer, notebook, true);
        let home = box_new(true, 12);
        gtk_container_set_border_width(home, 12);
        let card = box_new(true, 10);
        style(card, "card");
        pack(card, label("ARIZONA ROLE PLAY", "caption"), false);
        pack(
            card,
            label("Твой вход в игру. На Linux.", "alol-title"),
            false,
        );
        let detail = label("", "body");
        pack(card, detail, false);
        let runtime = label("", "runtime");
        pack(card, runtime, false);
        pack(home, card, false);
        let settings_page = box_new(true, 8);
        gtk_container_set_border_width(settings_page, 12);
        let log_page = box_new(true, 10);
        gtk_container_set_border_width(log_page, 12);
        pack(log_page, label("Журнал операций", "field-label"), false);
        pack(
            log_page,
            label(
                &format!("Последние 48 КБ · {}", core::log_file().display()),
                "caption",
            ),
            false,
        );
        let view = gtk_text_view_new();
        gtk_text_view_set_editable(view, 0);
        gtk_text_view_set_cursor_visible(view, 0);
        gtk_text_view_set_monospace(view, 1);
        gtk_text_view_set_wrap_mode(view, 3);
        let scroller = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
        gtk_scrolled_window_set_policy(scroller, 1, 1);
        gtk_container_add(scroller, view);
        pack(log_page, scroller, true);
        let status = label("", "body");
        let spinner = gtk_spinner_new();
        let footer = box_new(false, 10);
        gtk_widget_set_size_request(status, 160, -1);
        pack(footer, spinner, false);
        pack(footer, status, true);
        let empty = Fields {
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
        };
        let mut app = Box::new(App {
            settings: settings.clone(),
            window,
            notebook,
            status,
            runtime,
            detail,
            play: ptr::null_mut(),
            spinner,
            stop: ptr::null_mut(),
            settings_page,
            log_buffer: gtk_text_view_get_buffer(view),
            fields: empty,
            buttons: Vec::new(),
            utility_buttons: gtk_size_group_new(1),
            signals: Vec::new(),
            child: None,
            action: String::new(),
            log_text: String::new(),
            stop_at: None,
            mods_page: ptr::null_mut(),
            mod_list: ptr::null_mut(),
            mod_search: ptr::null_mut(),
            mod_file: ptr::null_mut(),
            mod_target: ptr::null_mut(),
            installed_combo: ptr::null_mut(),
            mod_count: ptr::null_mut(),
            catalog_items: Vec::new(),
            installed_items: Vec::new(),
            player,
            server_rows: Vec::new(),
            server_combo: ptr::null_mut(),
            nickname_entry: ptr::null_mut(),
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
        });
        let update_column = box_new(true, 4);
        pack(update_column, app.update_label, false);
        app.update_button =
            app.button(update_column, "Проверить обновления", Action::Update, false);
        pack(header, update_column, false);
        let player_controls = app.build_player_controls()?;
        pack(home, player_controls, false);
        let play_row = action_row();
        app.play = app.button(play_row, "Установить Arizona  →", Action::Play, true);
        app.button(play_row, "Обновить игру", Action::Work("launch"), false);
        app.button(play_row, "Папка игры", Action::Folder, false);
        pack(home, play_row, false);
        let maintenance = box_new(true, 10);
        let expander = gtk_expander_new(c"Установка и обслуживание".as_ptr());
        gtk_expander_set_expanded(expander, game::game_executable(&settings).is_err().into());
        gtk_container_add(expander, maintenance);
        pack(home, expander, false);
        let installer_row = action_row();
        app.button(
            installer_row,
            "Выбрать установщик .exe…",
            Action::ChooseInstaller,
            false,
        );
        app.button(installer_row, "Сайт Arizona", Action::Website, false);
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
            label("УСТАНОВКА И ВОССТАНОВЛЕНИЕ", "field-label"),
            false,
        );
        let row = action_row();
        app.button(
            row,
            "Подготовить Proton / Wine",
            Action::Work("prepare"),
            false,
        );
        app.button(row, "Установить лаунчер", Action::Work("install"), false);
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
                "Починка сохраняет копии кэша. Сброс графики сохраняет gta_sa.set.\nПроверка и восстановление файлов игры доступны в официальном лаунчере.",
                "caption",
            ),
            false,
        );
        let home_scroller = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
        gtk_scrolled_window_set_policy(home_scroller, 2, 1);
        gtk_container_add(home_scroller, home);
        notebook_page(notebook, home_scroller, "Играть", tab_sizes);
        let settings_scroller = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
        gtk_scrolled_window_set_policy(settings_scroller, 2, 1);
        gtk_container_add(settings_scroller, settings_page);
        notebook_page(notebook, settings_scroller, "Настройки", tab_sizes);
        let mods_page = app.build_mods();
        let mods_scroller = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
        gtk_scrolled_window_set_policy(mods_scroller, 2, 1);
        gtk_container_add(mods_scroller, mods_page);
        notebook_page(notebook, mods_scroller, "Моды", tab_sizes);
        notebook_page(notebook, log_page, "Журнал", tab_sizes);
        pack(
            settings_page,
            label("Настройки Arizona", "alol-title"),
            false,
        );
        pack(settings_page, label("Память игры", "field-label"), false);
        app.game_memory = gtk_combo_box_text_new();
        for memory in [1024, 2048, 3072, 4096] {
            gtk_combo_box_text_append_text(app.game_memory, c(&format!("{memory} МБ")).as_ptr());
        }
        gtk_combo_box_set_active(
            app.game_memory,
            ([1024, 2048, 3072, 4096]
                .iter()
                .position(|m| *m == game_options.memory)
                .unwrap_or(1)) as i32,
        );
        pack(settings_page, app.game_memory, false);
        for option in game_options::OPTIONS {
            let w = check(settings_page, option.label, game_options.enabled(option.id));
            gtk_widget_set_tooltip_text(w, c(option.tooltip).as_ptr());
            app.game_checks.push((option.id, w));
        }
        pack(
            settings_page,
            label(
                "Настройки применяются при запуске игры. Нажмите «Сохранить», чтобы сохранить изменения без запуска.",
                "caption",
            ),
            false,
        );
        pack(settings_page, gtk_separator_new(0), false);
        pack(
            settings_page,
            label("Окружение запуска", "field-label"),
            false,
        );
        let runner = gtk_combo_box_text_new();
        gtk_combo_box_text_append_text(runner, c"GE-Proton / UMU (основной вариант)".as_ptr());
        gtk_combo_box_text_append_text(runner, c"Wine (отдельное окружение)".as_ptr());
        gtk_combo_box_set_active(runner, if settings.runner == "wine" { 1 } else { 0 });
        pack(settings_page, runner, false);
        pack(
            settings_page,
            label(
                "Сборка Proton · можно вписать путь к другой сборке",
                "field-label",
            ),
            false,
        );
        let combo = gtk_combo_box_text_new_with_entry();
        for item in ["GE-Proton", "UMU-Proton"] {
            gtk_combo_box_text_append_text(combo, c(item).as_ptr());
        }
        for path in core::proton_versions() {
            gtk_combo_box_text_append_text(combo, c(&path.to_string_lossy()).as_ptr());
        }
        let proton = gtk_bin_get_child(combo);
        gtk_entry_set_text(proton, c(&settings.proton).as_ptr());
        pack(settings_page, combo, false);
        pack(
            settings_page,
            label(
                "Первая подготовка скачивает сборку и закрепляет её. Для обновления снова выберите GE-Proton. Обе семьи используют разные префиксы.",
                "caption",
            ),
            false,
        );
        pack(
            settings_page,
            label("Команда или полный путь к Wine", "field-label"),
            false,
        );
        let wine = entry(&settings.wine);
        pack(settings_page, wine, false);
        let installer = app.file_field(
            settings_page,
            "Установщик Arizona",
            &settings.installer,
            false,
        );
        let executable = app.file_field(
            settings_page,
            "Официальный лаунчер .exe · пусто = найти автоматически",
            &settings.executable,
            false,
        );
        gtk_entry_set_placeholder_text(executable, c"Автоматически после установки".as_ptr());
        let prefix = app.file_field(
            settings_page,
            "Папка окружений · подпапки proton и wine",
            &settings.prefix_root,
            true,
        );
        let wined3d = check(
            settings_page,
            "WineD3D / OpenGL вместо DXVK (если игра не отображается)",
            settings.wined3d,
        );
        let disable_gpu = check(
            settings_page,
            "Отключить GPU в официальном лаунчере (при пустом окне)",
            settings.disable_gpu,
        );
        let discrete_gpu = check(
            settings_page,
            "Второй GPU · DRI_PRIME=1 (Mesa)",
            settings.discrete_gpu,
        );
        let no_sync = check(
            settings_page,
            "Совместимость: отключить Esync / Fsync",
            settings.no_sync,
        );
        let debug = check(
            settings_page,
            "Подробный журнал Wine / Proton",
            settings.debug,
        );
        app.fields = Fields {
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
        };
        let row = action_row();
        app.button(row, "Сохранить", Action::Save, true);
        app.button(row, "Настройки Wine", Action::Work("winecfg"), false);
        pack(settings_page, row, false);
        pack(outer, gtk_separator_new(0), false);
        app.stop = app.button(footer, "Остановить", Action::Stop, false);
        gtk_widget_set_sensitive(app.stop, 0);
        pack(outer, footer, false);
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
        if app.capture.is_none() && !std::env::args().any(|a| a == "--smoke-test") {
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
