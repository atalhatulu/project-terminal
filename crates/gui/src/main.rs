mod config;
mod hints;
mod layout;
mod theme;

use config::Config;
use layout::{PaneGeometry, SplitDirection, TabTree, TreeDivider};

use std::sync::Arc;
use term_core::{CoreParser, Selection};
use term_pty::Pty;
use term_render::{
    get_top_bar_hit, CommandPaletteInfo, CommandPaletteItem, HeaderAction, PaneDrawInfo, TabInfo,
    TerminalRenderer, TopBarHit,
};

const ALL_COMMANDS: &[CommandPaletteItem] = &[
    CommandPaletteItem {
        id: "command_palette",
        title: "Komut Paletini Aç / Kapat",
        shortcut: "Ctrl+Shift+P",
        category: "Genel",
    },
    CommandPaletteItem {
        id: "new_tab",
        title: "Yeni Sekme Aç",
        shortcut: "Ctrl+Shift+T",
        category: "Sekmeler",
    },
    CommandPaletteItem {
        id: "split_horizontal",
        title: "Paneli Yatay Böl (Yan Yana)",
        shortcut: "Ctrl+Shift+D",
        category: "Paneller",
    },
    CommandPaletteItem {
        id: "split_vertical",
        title: "Paneli Dikey Böl (Alt Alta)",
        shortcut: "Ctrl+Shift+E",
        category: "Paneller",
    },
    CommandPaletteItem {
        id: "close_pane",
        title: "Aktif Paneli / Sekmeyi Kapat",
        shortcut: "Ctrl+Shift+W",
        category: "Paneller",
    },
    CommandPaletteItem {
        id: "toggle_zoom",
        title: "Aktif Paneli Büyüt (Zoom) / Geri Al",
        shortcut: "Alt+Z",
        category: "Paneller",
    },
    CommandPaletteItem {
        id: "toggle_search",
        title: "Arama Çubuğunu Aç / Kapat",
        shortcut: "Ctrl+Shift+F",
        category: "Arama",
    },
    CommandPaletteItem {
        id: "copy_selection",
        title: "Seçili Metni Kopyala",
        shortcut: "Ctrl+Shift+C",
        category: "Düzen",
    },
    CommandPaletteItem {
        id: "paste_clipboard",
        title: "Panodaki Metni Yapıştır",
        shortcut: "Ctrl+Shift+V",
        category: "Düzen",
    },
    CommandPaletteItem {
        id: "copy_last_output",
        title: "Son Komut Çıktısını Kopyala (OSC 133)",
        shortcut: "Ctrl+Shift+O",
        category: "Düzen",
    },
    CommandPaletteItem {
        id: "prev_command",
        title: "Önceki Komut Başlangıcına Git",
        shortcut: "Ctrl+Shift+Up",
        category: "Gezinme",
    },
    CommandPaletteItem {
        id: "next_command",
        title: "Sonraki Komut Başlangıcına Git",
        shortcut: "Ctrl+Shift+Down",
        category: "Gezinme",
    },
    CommandPaletteItem {
        id: "zoom_in",
        title: "Yazı Boyutunu Büyüt",
        shortcut: "Ctrl++",
        category: "Görünüm",
    },
    CommandPaletteItem {
        id: "zoom_out",
        title: "Yazı Boyutunu Küçült",
        shortcut: "Ctrl+-",
        category: "Görünüm",
    },
    CommandPaletteItem {
        id: "zoom_reset",
        title: "Varsayılan Yazı Boyutuna Dön",
        shortcut: "Ctrl+0",
        category: "Görünüm",
    },
    CommandPaletteItem {
        id: "toggle_fullscreen",
        title: "Tam Ekran Modunu Aç / Kapat",
        shortcut: "F11",
        category: "Görünüm",
    },
    CommandPaletteItem {
        id: "clear_scrollback",
        title: "Geçmişi ve Ekranı Temizle",
        shortcut: "Ctrl+Shift+K",
        category: "Terminal",
    },
    CommandPaletteItem {
        id: "open_config",
        title: "Ayarlar Dosyasını Aç (config.toml)",
        shortcut: "config.toml",
        category: "Ayarlar",
    },
    CommandPaletteItem {
        id: "toggle_hints",
        title: "Komut İpuçlarını Aç / Kapat",
        shortcut: "İpucu",
        category: "Terminal",
    },
    CommandPaletteItem {
        id: "toggle_typewriter",
        title: "Daktilo Animasyonunu Aç / Kapat",
        shortcut: "Daktilo",
        category: "Efektler",
    },
];

pub fn filter_commands_by_query(query: &str) -> Vec<CommandPaletteItem> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        ALL_COMMANDS.to_vec()
    } else {
        ALL_COMMANDS
            .iter()
            .filter(|cmd| {
                cmd.title.to_lowercase().contains(&q)
                    || cmd.shortcut.to_lowercase().contains(&q)
                    || cmd.category.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }
}

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

const APP_ICON_RGBA: &[u8] = include_bytes!("../../../assets/icon-64.rgba");

fn app_window_icon() -> Option<winit::window::Icon> {
    winit::window::Icon::from_rgba(APP_ICON_RGBA.to_vec(), 64, 64).ok()
}

#[derive(Debug)]
pub enum AppEvent {
    PtyData { pane_id: usize, bytes: Vec<u8> },
    PtyExit { pane_id: usize },
    BlinkTick,
    TypewriterTick,
    ConfigReloaded(Config),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DragState {
    None,
    SplitResize {
        split_index: usize,
        direction: SplitDirection,
        start_pixel: f32,
        initial_ratio: f32,
        total_span_pixels: f32,
    },
}

struct Pane {
    id: usize,
    parser: CoreParser,
    pty: Pty,
    title: String,
    is_held: bool,
    pending_typewriter_bytes: std::collections::VecDeque<u8>,
}

struct Tab {
    #[allow(dead_code)]
    id: usize,
    panes: Vec<Pane>,
    tree: TabTree,
    active_pane_id: usize,
    is_zoomed: bool,
}

impl Tab {
    fn active_pane(&self) -> Option<&Pane> {
        self.panes.iter().find(|p| p.id == self.active_pane_id)
    }

    fn active_pane_mut(&mut self) -> Option<&mut Pane> {
        let active_id = self.active_pane_id;
        self.panes.iter_mut().find(|p| p.id == active_id)
    }

    fn pane_mut(&mut self, pane_id: usize) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|p| p.id == pane_id)
    }

    fn geometries(&self, total_cols: usize, total_rows: usize) -> Vec<PaneGeometry> {
        if self.is_zoomed {
            vec![PaneGeometry {
                pane_id: self.active_pane_id,
                col_offset: 0,
                row_offset: 0,
                cols: total_cols,
                rows: total_rows,
            }]
        } else {
            self.tree.calculate_layout(0, 0, total_cols, total_rows)
        }
    }

    fn title(&self) -> &str {
        self.active_pane()
            .map(|p| p.title.as_str())
            .unwrap_or("Project Terminal")
    }
}

struct TerminalApp {
    window: Option<Arc<Window>>,
    renderer: Option<TerminalRenderer>,
    tabs: Vec<Tab>,
    active_tab_index: usize,
    next_tab_id: usize,
    next_pane_id: usize,
    event_loop_proxy: EventLoopProxy<AppEvent>,
    modifiers: ModifiersState,
    cols: usize,
    rows: usize,
    mouse_pressed: bool,
    selection_start: Option<(usize, usize)>,
    last_cursor_pos: (f32, f32),
    last_cell: Option<(usize, usize)>,
    last_click_time: std::time::Instant,
    click_count: usize,
    custom_cmd: Option<Vec<String>>,
    initial_cwd: Option<std::path::PathBuf>,
    custom_title: Option<String>,
    hold: bool,
    config: Config,
    hovered_top_bar: Option<TopBarHit>,
    drag_state: DragState,
    command_palette_open: bool,
    command_palette_query: String,
    command_palette_selected: usize,
    last_key_time: std::time::Instant,
}

impl TerminalApp {
    #[allow(clippy::too_many_arguments)]
    fn new(
        event_loop_proxy: EventLoopProxy<AppEvent>,
        cols: usize,
        rows: usize,
        custom_cmd: Option<Vec<String>>,
        initial_cwd: Option<std::path::PathBuf>,
        custom_title: Option<String>,
        hold: bool,
        config: Config,
    ) -> Self {
        Self {
            window: None,
            renderer: None,
            tabs: Vec::new(),
            active_tab_index: 0,
            next_tab_id: 1,
            next_pane_id: 1,
            event_loop_proxy,
            modifiers: ModifiersState::empty(),
            cols,
            rows,
            mouse_pressed: false,
            selection_start: None,
            last_cursor_pos: (0.0, 0.0),
            last_cell: None,
            last_click_time: std::time::Instant::now(),
            click_count: 0,
            custom_cmd,
            initial_cwd,
            custom_title,
            hold,
            config,
            hovered_top_bar: None,
            drag_state: DragState::None,
            command_palette_open: false,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            last_key_time: std::time::Instant::now(),
        }
    }

    fn has_header_bar(&self) -> bool {
        true
    }

    fn has_tabs(&self) -> bool {
        self.has_header_bar()
    }

    fn get_active_cwd(&self) -> Option<std::path::PathBuf> {
        self.tabs.get(self.active_tab_index).and_then(|t| {
            t.active_pane()
                .and_then(|p| p.pty.current_working_directory())
        })
    }

    fn toggle_search(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            if let Some(pane) = tab.active_pane_mut() {
                let is_open = pane.parser.grid().search_query.is_some();
                if is_open {
                    pane.parser.grid_mut().set_search_query(None);
                } else {
                    pane.parser.grid_mut().set_search_query(Some(String::new()));
                }
            }
        }
    }

    fn open_settings(&self) {
        if let Some(path) = Config::config_path() {
            if !path.exists() {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let default_toml = r##"# Project Terminal Yapılandırma Dosyası
# Hazır Temalar: "pure-black", "tokyo-night", "catppuccin-mocha", "gruvbox-dark", "one-dark", "dracula", "nord"
theme = "pure-black"

[font]
size = 14.5
family = "monospace"

[colors]
# Tema renklerini özel olarak ezmek isterseniz:
# background = "#000000"
# foreground = "#e2e4ea"

[cursor]
# color = "#7aa2f7"
style = "block" # "block", "beam", "underline"
blink = true

[window]
opacity = 0.95
"##;
                let _ = std::fs::write(&path, default_toml);
            }
            let _ = std::process::Command::new("xdg-open")
                .arg(&path)
                .spawn();
        }
    }

    fn filtered_commands(&self) -> Vec<CommandPaletteItem> {
        filter_commands_by_query(&self.command_palette_query)
    }

    fn toggle_command_palette(&mut self) {
        self.command_palette_open = !self.command_palette_open;
        if self.command_palette_open {
            self.command_palette_query.clear();
            self.command_palette_selected = 0;
        }
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }

    fn execute_command(&mut self, cmd_id: &str, event_loop: &ActiveEventLoop) {
        match cmd_id {
            "new_tab" => {
                let cwd = self.get_active_cwd();
                let _ = self.spawn_tab(None, cwd);
            }
            "split_horizontal" => {
                let _ = self.split_active_pane(SplitDirection::Horizontal);
            }
            "split_vertical" => {
                let _ = self.split_active_pane(SplitDirection::Vertical);
            }
            "close_pane" => {
                self.close_active_pane(event_loop);
            }
            "toggle_zoom" => {
                self.toggle_zoom_active_pane();
            }
            "toggle_search" => {
                self.toggle_search();
            }
            "copy_selection" => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(pane) = tab.active_pane_mut() {
                        if let Some(selected_text) = pane.parser.grid().get_selected_text() {
                            if !selected_text.is_empty() {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    let _ = cb.set_text(selected_text);
                                }
                                pane.parser.grid_mut().set_selection(None);
                            }
                        }
                    }
                }
            }
            "paste_clipboard" => {
                self.paste_from_clipboard();
            }
            "copy_last_output" => {
                if let Some(tab) = self.active_tab() {
                    if let Some(pane) = tab.active_pane() {
                        if let Some(output) = pane.parser.grid().get_last_command_output() {
                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                let _ = cb.set_text(output);
                            }
                        }
                    }
                }
            }
            "prev_command" => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(pane) = tab.active_pane_mut() {
                        pane.parser.grid_mut().scroll_to_prev_command();
                    }
                }
            }
            "next_command" => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(pane) = tab.active_pane_mut() {
                        pane.parser.grid_mut().scroll_to_next_command();
                    }
                }
            }
            "zoom_in" => {
                if let Some(renderer) = &mut self.renderer {
                    let cur = renderer.font_size;
                    renderer.set_font_size((cur + 1.0).min(48.0));
                    self.update_grid_size();
                }
            }
            "zoom_out" => {
                if let Some(renderer) = &mut self.renderer {
                    let cur = renderer.font_size;
                    renderer.set_font_size((cur - 1.0).max(8.0));
                    self.update_grid_size();
                }
            }
            "zoom_reset" => {
                if let Some(renderer) = &mut self.renderer {
                    let def = self.config.font_size();
                    renderer.set_font_size(def);
                    self.update_grid_size();
                }
            }
            "toggle_fullscreen" => {
                if let Some(win) = &self.window {
                    let is_fs = win.fullscreen().is_some();
                    if is_fs {
                        win.set_fullscreen(None);
                    } else {
                        win.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                    }
                }
            }
            "clear_scrollback" => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(pane) = tab.active_pane_mut() {
                        pane.parser.grid_mut().clear();
                        pane.parser.grid_mut().scrollback.clear();
                        pane.parser.grid_mut().scroll_offset = 0;
                    }
                }
            }
            "open_config" => {
                self.open_settings();
            }
            "toggle_hints" => {
                let current = self.config.show_hints.unwrap_or(true);
                self.config.show_hints = Some(!current);
            }
            "toggle_typewriter" => {
                let current = self.config.effects.as_ref().and_then(|e| e.typewriter).unwrap_or(false);
                let delay = self.config.effects.as_ref().and_then(|e| e.typewriter_delay_ms).unwrap_or(15);
                self.config.effects = Some(config::EffectsConfig {
                    typewriter: Some(!current),
                    typewriter_delay_ms: Some(delay),
                });
            }
            _ => {}
        }
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }

    fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab_index)
    }

    fn tab_info_list(&self) -> Vec<TabInfo> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| TabInfo {
                title: if tab.title().is_empty() {
                    format!("Tab {}", i + 1)
                } else if tab.is_zoomed {
                    format!("{} [ZOOM]", tab.title())
                } else if tab.panes.len() > 1 {
                    format!("{} [{}]", tab.title(), tab.panes.len())
                } else {
                    tab.title().to_string()
                },
                is_active: i == self.active_tab_index,
            })
            .collect()
    }

    fn update_window_title(&self) {
        if let Some(win) = &self.window {
            if let Some(hit) = self.hovered_top_bar {
                let title = match hit {
                    TopBarHit::Tab(idx) => format!("Project Terminal — Sekme {} / {}", idx + 1, self.tabs.len()),
                    TopBarHit::TabClose(idx) => format!("Project Terminal — Sekmeyi Kapat (Sekme {})", idx + 1),
                    TopBarHit::NewTab => "Project Terminal — Yeni Sekme (Ctrl+Shift+T)".to_string(),
                    TopBarHit::Action(action) => format!("Project Terminal — {}", action.tooltip()),
                };
                win.set_title(&title);
                return;
            }
            if let Some(tab) = self.active_tab() {
                let base_title = if let Some(ref ct) = self.custom_title {
                    ct.as_str()
                } else {
                    tab.title()
                };
                let zoom_tag = if tab.is_zoomed { " [ZOOM]" } else { "" };
                if self.tabs.len() > 1 {
                    win.set_title(&format!(
                        "[{}/{}] {}{} - Project Terminal",
                        self.active_tab_index + 1,
                        self.tabs.len(),
                        base_title,
                        zoom_tag
                    ));
                } else {
                    win.set_title(&format!("{}{zoom_tag} - Project Terminal", base_title));
                }
            }
        }
    }

    fn update_grid_size(&mut self) {
        let has_tabs = self.has_tabs();
        let inner_size = self.window.as_ref().map(|w| w.inner_size());
        if let (Some(renderer), Some(inner)) = (&mut self.renderer, inner_size) {
            let (new_cols, new_rows) =
                renderer.calculate_grid_size(inner.width, inner.height, has_tabs);
            self.cols = new_cols;
            self.rows = new_rows;

            for tab in &mut self.tabs {
                let geoms = tab.geometries(new_cols, new_rows);
                for geom in geoms {
                    if let Some(pane) = tab.pane_mut(geom.pane_id) {
                        pane.parser.resize(geom.cols, geom.rows);
                        let _ = pane.pty.resize(geom.cols as u16, geom.rows as u16);
                    }
                }
            }
        }
    }

    fn create_pane(
        &mut self,
        cols: usize,
        rows: usize,
        cmd: Option<Vec<String>>,
        cwd: Option<std::path::PathBuf>,
    ) -> anyhow::Result<Pane> {
        let pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        let mut parser = CoreParser::new(cols, rows);
        parser.set_default_colors(self.config.foreground_color(), self.config.background_color());
        parser.set_palette(self.config.palette());
        if let Some(shape) = self.config.cursor_shape() {
            parser.grid_mut().cursor_shape = shape;
        }

        let cmd = cmd.or_else(|| self.config.shell.clone().map(|s| vec![s]));
        let (pty, pty_rx) = Pty::spawn(cols as u16, rows as u16, cmd, cwd)?;

        let proxy = self.event_loop_proxy.clone();
        std::thread::spawn(move || {
            while let Ok(first_chunk) = pty_rx.recv() {
                let mut bytes = first_chunk;
                while let Ok(more) = pty_rx.try_recv() {
                    bytes.extend(more);
                }
                if proxy.send_event(AppEvent::PtyData { pane_id, bytes }).is_err() {
                    break;
                }
            }
            let _ = proxy.send_event(AppEvent::PtyExit { pane_id });
        });

        Ok(Pane {
            id: pane_id,
            parser,
            pty,
            title: "Project Terminal".to_string(),
            is_held: false,
            pending_typewriter_bytes: std::collections::VecDeque::new(),
        })
    }

    fn spawn_tab(
        &mut self,
        cmd: Option<Vec<String>>,
        cwd: Option<std::path::PathBuf>,
    ) -> anyhow::Result<usize> {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;

        let pane = self.create_pane(self.cols, self.rows, cmd, cwd)?;
        let pane_id = pane.id;

        let tab = Tab {
            id: tab_id,
            panes: vec![pane],
            tree: TabTree::Single(pane_id),
            active_pane_id: pane_id,
            is_zoomed: false,
        };

        self.tabs.push(tab);
        let new_index = self.tabs.len() - 1;
        self.active_tab_index = new_index;
        self.update_grid_size();
        self.update_window_title();

        Ok(new_index)
    }

    fn split_active_pane(&mut self, direction: SplitDirection) -> anyhow::Result<()> {
        if self.tabs.is_empty() {
            return Ok(());
        }
        let (active_pane_id, cwd) = {
            let active_tab = &self.tabs[self.active_tab_index];
            (
                active_tab.active_pane_id,
                active_tab
                    .active_pane()
                    .and_then(|p| p.pty.current_working_directory()),
            )
        };

        let new_pane = self.create_pane(self.cols / 2, self.rows / 2, None, cwd)?;
        let new_pane_id = new_pane.id;

        let active_tab = &mut self.tabs[self.active_tab_index];
        if active_tab.is_zoomed {
            active_tab.is_zoomed = false;
        }
        active_tab.panes.push(new_pane);
        active_tab.tree.split_pane(active_pane_id, new_pane_id, direction);
        active_tab.active_pane_id = new_pane_id;

        self.update_grid_size();
        if let Some(win) = &self.window {
            win.request_redraw();
        }

        Ok(())
    }

    fn close_active_pane(&mut self, event_loop: &ActiveEventLoop) {
        if self.tabs.is_empty() {
            return;
        }
        let tab = &mut self.tabs[self.active_tab_index];
        if tab.panes.len() > 1 {
            let active_id = tab.active_pane_id;
            if let Some(pos) = tab.panes.iter().position(|p| p.id == active_id) {
                tab.panes.remove(pos);
            }
            tab.tree.remove_pane(active_id);
            if tab.panes.len() <= 1 {
                tab.is_zoomed = false;
            }
            if let Some(first_pane) = tab.panes.first() {
                tab.active_pane_id = first_pane.id;
            }
            self.update_grid_size();
            self.update_window_title();
            if let Some(win) = &self.window {
                win.request_redraw();
            }
        } else {
            self.close_tab(self.active_tab_index, event_loop);
        }
    }

    fn navigate_pane_focus(&mut self, dir: SplitDirection, forward: bool) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            if tab.is_zoomed {
                tab.is_zoomed = false;
            }
            let geoms = tab.geometries(self.cols, self.rows);
            let active_id = tab.active_pane_id;
            if let Some(active_geom) = geoms.iter().find(|g| g.pane_id == active_id) {
                let neighbor = geoms.iter().find(|g| {
                    if g.pane_id == active_id {
                        return false;
                    }
                    match (dir, forward) {
                        (SplitDirection::Horizontal, false) => {
                            g.col_offset + g.cols <= active_geom.col_offset
                        }
                        (SplitDirection::Horizontal, true) => {
                            g.col_offset >= active_geom.col_offset + active_geom.cols
                        }
                        (SplitDirection::Vertical, false) => {
                            g.row_offset + g.rows <= active_geom.row_offset
                        }
                        (SplitDirection::Vertical, true) => {
                            g.row_offset >= active_geom.row_offset + active_geom.rows
                        }
                    }
                });

                if let Some(target) = neighbor {
                    tab.active_pane_id = target.pane_id;
                    if let Some(win) = &self.window {
                        win.request_redraw();
                    }
                }
            }
        }
    }

    fn toggle_zoom_active_pane(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            if tab.panes.len() > 1 {
                tab.is_zoomed = !tab.is_zoomed;
                self.update_grid_size();
                self.update_window_title();
                if let Some(win) = &self.window {
                    win.request_redraw();
                }
            } else {
                tab.is_zoomed = false;
            }
        }
    }

    fn resize_active_pane(&mut self, dir: SplitDirection, delta: f32) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            if !tab.is_zoomed && tab.panes.len() > 1 {
                if tab.tree.adjust_pane_ratio(tab.active_pane_id, dir, delta) {
                    self.update_grid_size();
                    if let Some(win) = &self.window {
                        win.request_redraw();
                    }
                }
            }
        }
    }

    fn find_divider_at(&self, mouse_x: f32, mouse_y: f32) -> Option<TreeDivider> {
        let tab = self.tabs.get(self.active_tab_index)?;
        if tab.is_zoomed || tab.panes.len() <= 1 {
            return None;
        }
        let renderer = self.renderer.as_ref()?;
        let has_header = self.has_header_bar();
        let padding_left = 8.0;
        let padding_top = if has_header { 30.0 + 6.0 } else { 8.0 };

        let dividers = tab.tree.collect_dividers(0, 0, self.cols, self.rows);
        for div in dividers {
            match div.direction {
                SplitDirection::Horizontal => {
                    let div_px = padding_left + div.col_or_row as f32 * renderer.cell_width;
                    let y_min = padding_top + div.span_start as f32 * renderer.cell_height;
                    let y_max = padding_top + div.span_end as f32 * renderer.cell_height;
                    if (mouse_x - div_px).abs() <= 5.0 && mouse_y >= y_min && mouse_y <= y_max {
                        return Some(div);
                    }
                }
                SplitDirection::Vertical => {
                    let div_py = padding_top + div.col_or_row as f32 * renderer.cell_height;
                    let x_min = padding_left + div.span_start as f32 * renderer.cell_width;
                    let x_max = padding_left + div.span_end as f32 * renderer.cell_width;
                    if (mouse_y - div_py).abs() <= 5.0 && mouse_x >= x_min && mouse_x <= x_max {
                        return Some(div);
                    }
                }
            }
        }
        None
    }

    fn close_tab(&mut self, index: usize, event_loop: &ActiveEventLoop) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            event_loop.exit();
            return;
        }
        if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
        self.update_grid_size();
        self.update_window_title();
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }

    fn paste_from_clipboard(&mut self) {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if let Ok(clip_text) = cb.get_text() {
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(pane) = tab.active_pane_mut() {
                        let normalized = clip_text.replace("\r\n", "\r").replace('\n', "\r");
                        let grid = pane.parser.grid_mut();
                        grid.set_selection(None);
                        grid.scroll_offset = 0;
                        if pane.parser.bracketed_paste {
                            let mut data = Vec::with_capacity(normalized.len() + 12);
                            data.extend_from_slice(b"\x1b[200~");
                            data.extend_from_slice(normalized.as_bytes());
                            data.extend_from_slice(b"\x1b[201~");
                            let _ = pane.pty.write_bytes(&data);
                        } else {
                            let _ = pane.pty.write_bytes(normalized.as_bytes());
                        }
                    }
                }
            }
        }
    }

    fn send_mouse_event(&self, button: u8, col: usize, row: usize, press: bool) {
        if let Some(tab) = self.active_tab() {
            if let Some(pane) = tab.active_pane() {
                let x = col + 1;
                let y = row + 1;

                let mut code = button;
                if self.modifiers.shift_key() {
                    code += 4;
                }
                if self.modifiers.alt_key() {
                    code += 8;
                }
                if self.modifiers.control_key() {
                    code += 16;
                }

                if pane.parser.mouse_sgr {
                    let suffix = if press { 'M' } else { 'm' };
                    let seq = format!("\x1b[<{};{};{}{}", code, x, y, suffix);
                    let _ = pane.pty.write_bytes(seq.as_bytes());
                } else if x <= 223 && y <= 223 {
                    let event_code = if press { code } else { 3 };
                    let bytes = [
                        0x1b,
                        b'[',
                        b'M',
                        event_code.saturating_add(32),
                        (x as u8).saturating_add(32),
                        (y as u8).saturating_add(32),
                    ];
                    let _ = pane.pty.write_bytes(&bytes);
                }
            }
        }
    }
}

impl ApplicationHandler<AppEvent> for TerminalApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let initial_title = self
                .custom_title
                .clone()
                .unwrap_or_else(|| "Project Terminal".to_string());

            let mut window_attributes = Window::default_attributes()
                .with_title(initial_title)
                .with_inner_size(PhysicalSize::new(960, 600))
                .with_transparent(true);

            if let Some(icon) = app_window_icon() {
                window_attributes = window_attributes.with_window_icon(Some(icon));
            }

            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            window.set_ime_allowed(true);
            self.window = Some(window.clone());

            let mut renderer = pollster::block_on(TerminalRenderer::new(
                window.clone(),
                self.config.font_family(),
                Some(self.config.font_size()),
            ))
            .expect("Renderer başlatılamadı");

            renderer.set_colors(
                self.config.background_color(),
                self.config.foreground_color(),
                self.config.cursor_color(),
            );
            renderer.set_opacity(self.config.opacity());

            let inner = window.inner_size();
            let (initial_cols, initial_rows) =
                renderer.calculate_grid_size(inner.width, inner.height, self.has_header_bar());
            self.cols = initial_cols;
            self.rows = initial_rows;
            self.renderer = Some(renderer);

            // İlk sekme başlatılır
            let cmd = self.custom_cmd.take();
            let cwd = self.initial_cwd.clone();
            if let Err(e) = self.spawn_tab(cmd, cwd) {
                eprintln!("İlk sekme ve kabuk başlatılamadı: {:?}", e);
            }

            // İmleç Yanıp Sönme Zamanlayıcısı
            let timer_proxy = self.event_loop_proxy.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    if timer_proxy.send_event(AppEvent::BlinkTick).is_err() {
                        break;
                    }
                }
            });

            // Daktilo Animasyon Zamanlayıcısı (Typewriter Tick)
            let typewriter_proxy = self.event_loop_proxy.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(15));
                    if typewriter_proxy.send_event(AppEvent::TypewriterTick).is_err() {
                        break;
                    }
                }
            });

            // Canlı Yapılandırma Dosyası İzleyici (Config Live Reload)
            if let Some(cfg_path) = Config::config_path() {
                let config_proxy = self.event_loop_proxy.clone();
                std::thread::spawn(move || {
                    let mut last_modified = std::fs::metadata(&cfg_path)
                        .and_then(|m| m.modified())
                        .ok();
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(750));
                        if let Ok(meta) = std::fs::metadata(&cfg_path) {
                            if let Ok(mod_time) = meta.modified() {
                                if Some(mod_time) != last_modified {
                                    last_modified = Some(mod_time);
                                    let new_cfg = Config::load();
                                    if config_proxy
                                        .send_event(AppEvent::ConfigReloaded(new_cfg))
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                });
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::PtyData { pane_id, bytes } => {
                let is_typewriter = self.config.effects.as_ref().and_then(|e| e.typewriter).unwrap_or(false);

                for (tab_idx, tab) in self.tabs.iter_mut().enumerate() {
                    if let Some(pane) = tab.pane_mut(pane_id) {
                        // Eğer daktilo modu açıksa kuyruğa ekle, devasa çıktılarda (16KB+) doğrudan işlet
                        if is_typewriter && pane.pending_typewriter_bytes.len() < 16384 && pane.parser.grid().cursor_visible {
                            pane.pending_typewriter_bytes.extend(bytes);
                        } else {
                            // Tampon çok şişmişse veya daktilo kapalıysa doğrudan işlet
                            if !pane.pending_typewriter_bytes.is_empty() {
                                let drained: Vec<u8> = pane.pending_typewriter_bytes.drain(..).collect();
                                pane.parser.process_bytes(&drained);
                            }
                            pane.parser.process_bytes(&bytes);
                        }

                        if let Some(text) = pane.parser.clipboard_store.take() {
                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                let _ = cb.set_text(text);
                            }
                        }

                        for resp in pane.parser.drain_responses() {
                            let _ = pane.pty.write_bytes(&resp);
                        }

                        if pane.parser.bell {
                            pane.parser.bell = false;
                            if let Some(win) = &self.window {
                                win.request_user_attention(Some(winit::window::UserAttentionType::Informational));
                            }
                        }

                        if let Some(t) = pane.parser.grid().title.as_ref() {
                            if t != &pane.title {
                                pane.title = t.clone();
                                if tab_idx == self.active_tab_index && pane.id == tab.active_pane_id {
                                    self.update_window_title();
                                }
                            }
                        }

                        if tab_idx == self.active_tab_index {
                            if let Some(win) = &self.window {
                                win.request_redraw();
                            }
                        }
                        break;
                    }
                }
            }
            AppEvent::TypewriterTick => {
                let mut needs_redraw = false;
                for tab in &mut self.tabs {
                    for pane in &mut tab.panes {
                        let pending_len = pane.pending_typewriter_bytes.len();
                        if pending_len > 0 {
                            // Dinamik adaptif hız hesabı:
                            // Toplam animasyonun maksimum ~150-200ms civarında tamamlanması hedeflenir.
                            // - Kısa çıktılarda (örneğin < 60 bayt): 3-5 bayt ile daktilo ritmi.
                            // - Orta çıktılarda (100 - 500 bayt): 20 - 40 baytlık akıcı kayma.
                            // - Uzun çıktılarda (ls / tree, 1000+ bayt): 100 - 250 baytlık yüksek hızlı akış.
                            let count = if pending_len > 1000 {
                                (pending_len / 10).clamp(120, 512)
                            } else if pending_len > 250 {
                                (pending_len / 12).clamp(35, 120)
                            } else if pending_len > 60 {
                                12.min(pending_len)
                            } else {
                                4.min(pending_len)
                            };

                            let mut chunk = Vec::with_capacity(count);
                            for _ in 0..count {
                                if let Some(b) = pane.pending_typewriter_bytes.pop_front() {
                                    chunk.push(b);
                                }
                            }
                            pane.parser.process_bytes(&chunk);
                            needs_redraw = true;
                        }
                    }
                }
                if needs_redraw {
                    if let Some(win) = &self.window {
                        win.request_redraw();
                    }
                }
            }
            AppEvent::PtyExit { pane_id } => {
                let mut found = None;
                for (tab_idx, tab) in self.tabs.iter_mut().enumerate() {
                    if let Some(pos) = tab.panes.iter().position(|p| p.id == pane_id) {
                        found = Some((tab_idx, pos, tab.panes.len()));
                        break;
                    }
                }

                if let Some((tab_idx, pane_pos, pane_count)) = found {
                    if self.hold {
                        let tab = &mut self.tabs[tab_idx];
                        let pane = &mut tab.panes[pane_pos];
                        pane.is_held = true;
                        pane.parser.process_bytes(
                            b"\r\n\x1b[33m[Surec sonlandi - Kapatmak icin herhangi bir tusa basin]\x1b[0m\r\n",
                        );
                        if tab_idx == self.active_tab_index {
                            if let Some(win) = &self.window {
                                win.request_redraw();
                            }
                        }
                    } else if pane_count > 1 {
                        let tab = &mut self.tabs[tab_idx];
                        let closed_id = tab.panes[pane_pos].id;
                        tab.panes.remove(pane_pos);
                        tab.tree.remove_pane(closed_id);
                        if tab.active_pane_id == closed_id {
                            if let Some(first) = tab.panes.first() {
                                tab.active_pane_id = first.id;
                            }
                        }
                        self.update_grid_size();
                        self.update_window_title();
                        if let Some(win) = &self.window {
                            win.request_redraw();
                        }
                    } else {
                        self.close_tab(tab_idx, event_loop);
                    }
                }
            }
            AppEvent::BlinkTick => {
                if let Some(win) = &self.window {
                    win.request_redraw();
                }
            }
            AppEvent::ConfigReloaded(new_cfg) => {
                tracing::info!("Ayar dosyası güncellendi, uygulanıyor...");
                self.config = new_cfg;
                if let Some(renderer) = &mut self.renderer {
                    renderer.set_colors(
                        self.config.background_color(),
                        self.config.foreground_color(),
                        self.config.cursor_color(),
                    );
                    renderer.set_opacity(self.config.opacity());
                    renderer.set_font_size(self.config.font_size());
                }
                for tab in &mut self.tabs {
                    for pane in &mut tab.panes {
                        pane.parser.set_default_colors(
                            self.config.foreground_color(),
                            self.config.background_color(),
                        );
                        pane.parser.set_palette(self.config.palette());
                        if let Some(shape) = self.config.cursor_shape() {
                            pane.parser.grid_mut().cursor_shape = shape;
                        }
                    }
                }
                self.update_grid_size();
                if let Some(win) = &self.window {
                    win.request_redraw();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(new_size.width, new_size.height);
                }
                self.update_grid_size();
                if let Some(win) = &self.window {
                    win.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let tab_infos = self.tab_info_list();
                let active_idx = self.active_tab_index;
                let cols = self.cols;
                let rows = self.rows;

                let filtered = if self.command_palette_open {
                    Some(self.filtered_commands())
                } else {
                    None
                };

                let pal_info = filtered.as_ref().map(|items| CommandPaletteInfo {
                    query: &self.command_palette_query,
                    items,
                    selected_index: self.command_palette_selected,
                });

                if let Some(tab) = self.tabs.get(active_idx) {
                    let is_in_sync = tab.panes.iter().any(|p| {
                        p.parser.synchronized_output
                            && p.parser
                                .sync_output_start
                                .map(|t| t.elapsed().as_millis() < 60)
                                .unwrap_or(false)
                    });
                    if is_in_sync && !self.command_palette_open {
                        // Senkronize güncelleme bitene kadar kare çizimini ertele (yırtılmasız atomik render)
                        return;
                    }
                }

                if let (Some(renderer), Some(tab)) = (&mut self.renderer, self.tabs.get(active_idx)) {
                    let geoms = tab.geometries(cols, rows);
                    let show_hints = self.config.show_hints.unwrap_or(true);
                    let is_typing_paused = self.last_key_time.elapsed().as_millis() >= 250;

                    let draw_panes: Vec<PaneDrawInfo> = geoms
                        .iter()
                        .filter_map(|g| {
                            tab.panes.iter().find(|p| p.id == g.pane_id).map(|p| {
                                let is_active = p.id == tab.active_pane_id;
                                let ghost_hint = if is_active && show_hints && is_typing_paused {
                                    let grid = p.parser.grid();
                                    let row_cells = grid.get_display_line(grid.cursor_row);
                                    let line_str: String = row_cells.iter().map(|c| if c.c == '\0' { ' ' } else { c.c }).collect();
                                    // Bash/zsh komut isteminde ($ veya # veya >) sonrasını al
                                    let prompt_stripped = if let Some((pos, ch)) = line_str.char_indices().rfind(|&(_, c)| matches!(c, '$' | '#' | '>' | '❯' | '➜' | ':')) {
                                        &line_str[pos + ch.len_utf8()..]
                                    } else {
                                        &line_str
                                    };
                                    let first_word = prompt_stripped.split_whitespace().next().unwrap_or("");
                                    hints::get_command_description(first_word)
                                } else {
                                    None
                                };

                                PaneDrawInfo {
                                    id: p.id,
                                    grid: p.parser.grid(),
                                    col_offset: g.col_offset,
                                    row_offset: g.row_offset,
                                    cols: g.cols,
                                    rows: g.rows,
                                    is_active,
                                    ghost_hint,
                                }
                            })
                        })
                        .collect();

                    if let Err(e) = renderer.render_panes(&draw_panes, Some(&tab_infos), pal_info.as_ref()) {
                        tracing::warn!("Render hatası: {:?}", e);
                    }

                    if renderer.is_animating {
                        if let Some(win) = &self.window {
                            win.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }
            WindowEvent::Ime(ime) => match ime {
                winit::event::Ime::Commit(text) => {
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                        if let Some(pane) = tab.active_pane_mut() {
                            let _ = pane.pty.write_bytes(text.as_bytes());
                        }
                    }
                }
                _ => {}
            },
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        text,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                // Eğer süreç sonlanmış ve hold modundaysa, herhangi bir tuşa basılınca sekmeyi kapat
                if let Some(tab) = self.tabs.get(self.active_tab_index) {
                    if let Some(pane) = tab.active_pane() {
                        if pane.is_held {
                            self.close_active_pane(event_loop);
                            return;
                        }
                    }
                }

                let is_ctrl = self.modifiers.control_key();
                let is_alt = self.modifiers.alt_key();
                let is_shift = self.modifiers.shift_key();

                self.last_key_time = std::time::Instant::now();

                // 0. Ctrl + Shift + P ile Komut Paletini Aç / Kapat
                if is_ctrl && is_shift && matches!(&logical_key, Key::Character(c) if c.eq_ignore_ascii_case("p")) {
                    self.toggle_command_palette();
                    return;
                }

                // Komut Paleti aktifken tüm tuş girdilerini modal pencereye yönlendir
                if self.command_palette_open {
                    match &logical_key {
                        Key::Named(NamedKey::Escape) => {
                            self.command_palette_open = false;
                            if let Some(win) = &self.window {
                                win.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::ArrowUp) => {
                            let total = self.filtered_commands().len();
                            if total > 0 {
                                self.command_palette_selected = if self.command_palette_selected == 0 {
                                    total - 1
                                } else {
                                    self.command_palette_selected - 1
                                };
                                if let Some(win) = &self.window {
                                    win.request_redraw();
                                }
                            }
                            return;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            let total = self.filtered_commands().len();
                            if total > 0 {
                                self.command_palette_selected = (self.command_palette_selected + 1) % total;
                                if let Some(win) = &self.window {
                                    win.request_redraw();
                                }
                            }
                            return;
                        }
                        Key::Named(NamedKey::Enter) => {
                            let filtered = self.filtered_commands();
                            if !filtered.is_empty() {
                                let sel = self.command_palette_selected.min(filtered.len() - 1);
                                let cmd_id = filtered[sel].id;
                                self.command_palette_open = false;
                                self.execute_command(cmd_id, event_loop);
                            } else {
                                self.command_palette_open = false;
                                if let Some(win) = &self.window {
                                    win.request_redraw();
                                }
                            }
                            return;
                        }
                        Key::Named(NamedKey::Backspace) => {
                            self.command_palette_query.pop();
                            self.command_palette_selected = 0;
                            if let Some(win) = &self.window {
                                win.request_redraw();
                            }
                            return;
                        }
                        _ => {
                            if let Some(txt) = text {
                                for ch in txt.chars() {
                                    if !ch.is_control() {
                                        self.command_palette_query.push(ch);
                                        self.command_palette_selected = 0;
                                    }
                                }
                                if let Some(win) = &self.window {
                                    win.request_redraw();
                                }
                            }
                            return;
                        }
                    }
                }

                // 1. Sekmeler Arası Geçiş (Ctrl + Tab / Ctrl + Shift + Tab)
                if is_ctrl && matches!(&logical_key, Key::Named(NamedKey::Tab)) {
                    if self.tabs.len() > 1 {
                        if is_shift {
                            self.active_tab_index =
                                (self.active_tab_index + self.tabs.len() - 1) % self.tabs.len();
                        } else {
                            self.active_tab_index = (self.active_tab_index + 1) % self.tabs.len();
                        }
                        self.update_window_title();
                        if let Some(win) = &self.window {
                            win.request_redraw();
                        }
                    }
                    return;
                }

                // 2. Alt + 1..9 ile Doğrudan Sekmeye Atlama
                if is_alt && !is_ctrl && !is_shift {
                    if let Key::Character(ref c) = logical_key {
                        if let Some(ch) = c.chars().next() {
                            if ('1'..='9').contains(&ch) {
                                let target_idx = (ch as usize) - ('1' as usize);
                                if self.tabs.len() > 1 && target_idx < self.tabs.len() {
                                    self.active_tab_index = target_idx;
                                    self.update_window_title();
                                    if let Some(win) = &self.window {
                                        win.request_redraw();
                                    }
                                    return;
                                }
                            }
                        }
                    }
                }

                // 3. Alt + Ok Tuşları ile Paneller Arası Odak Geçişi (Split Pane Focus Navigation)
                if is_alt && !is_ctrl && !is_shift {
                    match &logical_key {
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.navigate_pane_focus(SplitDirection::Horizontal, false);
                            return;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            self.navigate_pane_focus(SplitDirection::Horizontal, true);
                            return;
                        }
                        Key::Named(NamedKey::ArrowUp) => {
                            self.navigate_pane_focus(SplitDirection::Vertical, false);
                            return;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            self.navigate_pane_focus(SplitDirection::Vertical, true);
                            return;
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("z") => {
                            self.toggle_zoom_active_pane();
                            return;
                        }
                        _ => {}
                    }
                }

                // 3b. Ctrl + Alt + Ok Tuşları ile Panel Yeniden Boyutlandırma (Split Resize)
                if is_ctrl && is_alt && !is_shift {
                    match &logical_key {
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.resize_active_pane(SplitDirection::Horizontal, -0.05);
                            return;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            self.resize_active_pane(SplitDirection::Horizontal, 0.05);
                            return;
                        }
                        Key::Named(NamedKey::ArrowUp) => {
                            self.resize_active_pane(SplitDirection::Vertical, -0.05);
                            return;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            self.resize_active_pane(SplitDirection::Vertical, 0.05);
                            return;
                        }
                        _ => {}
                    }
                }

                // 4. Ctrl + Shift Kısayolları (Sekme, Panel Bölme, Kapatma, Arama, Panel Zoom)
                if is_ctrl && is_shift {
                    match &logical_key {
                        Key::Named(NamedKey::Enter) => {
                            self.toggle_zoom_active_pane();
                            return;
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("t") => {
                            let cwd = self.tabs.get(self.active_tab_index).and_then(|t| {
                                t.active_pane()
                                    .and_then(|p| p.pty.current_working_directory())
                            });
                            let _ = self.spawn_tab(None, cwd);
                            if let Some(win) = &self.window {
                                win.request_redraw();
                            }
                            return;
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("d") => {
                            // Yatay bölme (Horizontal Split - Yan yana iki panel)
                            let _ = self.split_active_pane(SplitDirection::Horizontal);
                            return;
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("e") => {
                            // Dikey bölme (Vertical Split - Alt alta iki panel)
                            let _ = self.split_active_pane(SplitDirection::Vertical);
                            return;
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("w") => {
                            // Aktif paneli kapat (Son panel ise sekmeyi kapat)
                            self.close_active_pane(event_loop);
                            return;
                        }
                        Key::Named(NamedKey::ArrowUp) => {
                            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                                if let Some(pane) = tab.active_pane_mut() {
                                    if pane.parser.grid_mut().scroll_to_prev_command() {
                                        if let Some(win) = &self.window {
                                            win.request_redraw();
                                        }
                                    }
                                }
                            }
                            return;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                                if let Some(pane) = tab.active_pane_mut() {
                                    if pane.parser.grid_mut().scroll_to_next_command() {
                                        if let Some(win) = &self.window {
                                            win.request_redraw();
                                        }
                                    }
                                }
                            }
                            return;
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                                if let Some(pane) = tab.active_pane_mut() {
                                    if let Some(selected_text) = pane.parser.grid().get_selected_text() {
                                        if !selected_text.is_empty() {
                                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                                let _ = cb.set_text(selected_text);
                                            }
                                            pane.parser.grid_mut().set_selection(None);
                                            if let Some(win) = &self.window {
                                                win.request_redraw();
                                            }
                                        }
                                    }
                                }
                            }
                            return;
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("v") => {
                            self.paste_from_clipboard();
                            return;
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("f") => {
                            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                                if let Some(pane) = tab.active_pane_mut() {
                                    let is_open = pane.parser.grid().search_query.is_some();
                                    if is_open {
                                        pane.parser.grid_mut().set_search_query(None);
                                    } else {
                                        pane.parser.grid_mut().set_search_query(Some(String::new()));
                                    }
                                    if let Some(win) = &self.window {
                                        win.request_redraw();
                                    }
                                }
                            }
                            return;
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("o") => {
                            self.execute_command("copy_last_output", event_loop);
                            return;
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("k") => {
                            self.execute_command("clear_scrollback", event_loop);
                            return;
                        }
                        _ => {}
                    }
                }

                // Tam Ekran Geçişi (F11 veya Alt + Enter)
                if logical_key == Key::Named(NamedKey::F11)
                    || (is_alt && matches!(&logical_key, Key::Named(NamedKey::Enter)))
                {
                    if let Some(win) = &self.window {
                        let is_fs = win.fullscreen().is_some();
                        if is_fs {
                            win.set_fullscreen(None);
                        } else {
                            win.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                        }
                    }
                    return;
                }

                // 5. Standart Ctrl Zoom Kısayolları (Ctrl +, Ctrl -, Ctrl 0)
                if is_ctrl && !is_alt {
                    let mut zoom_delta = None;
                    match &logical_key {
                        Key::Character(c) if c == "=" || c == "+" => zoom_delta = Some(1.0),
                        Key::Character(c) if c == "-" => zoom_delta = Some(-1.0),
                        Key::Character(c) if c == "0" => zoom_delta = Some(0.0),
                        _ => {}
                    }

                    if let Some(delta) = zoom_delta {
                        let new_size = if delta == 0.0 {
                            14.0
                        } else {
                            self.renderer.as_ref().map(|r| r.font_size + delta).unwrap_or(14.0)
                        };
                        if let Some(renderer) = &mut self.renderer {
                            renderer.set_font_size(new_size);
                        }
                        self.update_grid_size();
                        if let Some(win) = &self.window {
                            win.request_redraw();
                        }
                        return;
                    }
                }

                // 6. Arama Modu Girişleri (Search Mode Input Interception)
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(pane) = tab.active_pane_mut() {
                        if pane.parser.grid().search_query.is_some() {
                            match &logical_key {
                                Key::Named(NamedKey::Escape) => {
                                    pane.parser.grid_mut().set_search_query(None);
                                    if let Some(win) = &self.window {
                                        win.request_redraw();
                                    }
                                    return;
                                }
                                Key::Named(NamedKey::Enter) => {
                                    if is_shift {
                                        pane.parser.grid_mut().prev_search_match();
                                    } else {
                                        pane.parser.grid_mut().next_search_match();
                                    }
                                    if let Some(win) = &self.window {
                                        win.request_redraw();
                                    }
                                    return;
                                }
                                Key::Named(NamedKey::Backspace) => {
                                    let mut q = pane.parser.grid().search_query.clone().unwrap_or_default();
                                    q.pop();
                                    pane.parser.grid_mut().set_search_query(Some(q));
                                    if let Some(win) = &self.window {
                                        win.request_redraw();
                                    }
                                    return;
                                }
                                _ => {
                                    if !is_ctrl && !is_alt {
                                        if let Some(ref t) = text {
                                            if !t.chars().any(|c| c.is_control()) {
                                                let mut q =
                                                    pane.parser.grid().search_query.clone().unwrap_or_default();
                                                q.push_str(t.as_str());
                                                pane.parser.grid_mut().set_search_query(Some(q));
                                                if let Some(win) = &self.window {
                                                    win.request_redraw();
                                                }
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // 7. Ctrl Tuş Kombinasyonları (Ctrl+C, Ctrl+V, Ctrl+A..Z, Ctrl+Space)
                if is_ctrl && !is_alt && !is_shift {
                    match &logical_key {
                        Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                                if let Some(pane) = tab.active_pane_mut() {
                                    if let Some(selected_text) = pane.parser.grid().get_selected_text() {
                                        if !selected_text.is_empty() {
                                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                                let _ = cb.set_text(selected_text);
                                            }
                                            pane.parser.grid_mut().set_selection(None);
                                            if let Some(win) = &self.window {
                                                win.request_redraw();
                                            }
                                            return;
                                        }
                                    }
                                    let _ = pane.pty.write_bytes(&[0x03]); // SIGINT
                                }
                            }
                            return;
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("v") => {
                            self.paste_from_clipboard();
                            return;
                        }
                        Key::Character(c) => {
                            if let Some(ch) = c.chars().next() {
                                let ch_lower = ch.to_ascii_lowercase();
                                if ch_lower.is_ascii_lowercase() {
                                    let ctrl_byte = (ch_lower as u8) - b'a' + 1;
                                    if let Some(tab) = self.active_tab() {
                                        if let Some(pane) = tab.active_pane() {
                                            let _ = pane.pty.write_bytes(&[ctrl_byte]);
                                        }
                                    }
                                    return;
                                }
                            }
                        }
                        Key::Named(NamedKey::Space) => {
                            if let Some(tab) = self.active_tab() {
                                if let Some(pane) = tab.active_pane() {
                                    let _ = pane.pty.write_bytes(&[0x00]); // NUL
                                }
                            }
                            return;
                        }
                        _ => {}
                    }
                }

                // 8. Shift ile Geçmişte Gezinme (Shift+PageUp, Shift+PageDown, Shift+Home, Shift+End)
                if is_shift && !is_ctrl && !is_alt {
                    let rows = self.rows;
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                        if let Some(pane) = tab.active_pane_mut() {
                            if !pane.parser.use_alt {
                                match &logical_key {
                                    Key::Named(NamedKey::PageUp) => {
                                        let step = (rows / 2).max(1) as i32;
                                        pane.parser.grid_mut().scroll(step);
                                        if let Some(win) = &self.window {
                                            win.request_redraw();
                                        }
                                        return;
                                    }
                                    Key::Named(NamedKey::PageDown) => {
                                        let step = (rows / 2).max(1) as i32;
                                        pane.parser.grid_mut().scroll(-step);
                                        if let Some(win) = &self.window {
                                            win.request_redraw();
                                        }
                                        return;
                                    }
                                    Key::Named(NamedKey::Home) => {
                                        let max_s = pane.parser.grid().scrollback.len();
                                        pane.parser.grid_mut().scroll_offset = max_s;
                                        pane.parser.grid_mut().mark_all_dirty();
                                        if let Some(win) = &self.window {
                                            win.request_redraw();
                                        }
                                        return;
                                    }
                                    Key::Named(NamedKey::End) => {
                                        pane.parser.grid_mut().scroll_offset = 0;
                                        pane.parser.grid_mut().mark_all_dirty();
                                        if let Some(win) = &self.window {
                                            win.request_redraw();
                                        }
                                        return;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                // 8.5. Kitty Klavye Protokolü (CSI u Modu)
                if let Some(tab) = self.active_tab() {
                    if let Some(pane) = tab.active_pane() {
                        if pane.parser.keyboard_flags > 0 {
                            let mut mod_mask = 0u32;
                            if is_shift { mod_mask |= 1; }
                            if is_alt { mod_mask |= 2; }
                            if is_ctrl { mod_mask |= 4; }

                            let modifier_param = mod_mask + 1;

                            let codepoint_opt = match &logical_key {
                                Key::Named(NamedKey::Enter) => Some(13u32),
                                Key::Named(NamedKey::Tab) => Some(9u32),
                                Key::Named(NamedKey::Backspace) => Some(127u32),
                                Key::Named(NamedKey::Escape) => Some(27u32),
                                Key::Named(NamedKey::Space) if is_ctrl || is_alt => Some(32u32),
                                Key::Character(c) if is_ctrl || is_alt => {
                                    c.chars().next().map(|ch| ch as u32)
                                }
                                _ => None,
                            };

                            if let Some(cp) = codepoint_opt {
                                if mod_mask > 0 || cp == 13 || cp == 9 || cp == 127 || cp == 27 {
                                    let seq = format!("\x1b[{};{}u", cp, modifier_param);
                                    let _ = pane.pty.write_bytes(seq.as_bytes());
                                    if let Some(win) = &self.window {
                                        win.request_redraw();
                                    }
                                    return;
                                }
                            }
                        }
                    }
                }

                let app_cursor = self
                    .active_tab()
                    .and_then(|t| t.active_pane())
                    .map(|p| p.parser.app_cursor_keys)
                    .unwrap_or(false);

                // 9. Özel Tuşlar (Oklar, Fonksiyon Tuşları, Home/End, Backspace, Enter, Boşluk vb.)
                let bytes: Option<Vec<u8>> = match &logical_key {
                    Key::Named(NamedKey::Space) => Some(vec![b' ']),
                    Key::Named(NamedKey::Enter) => Some(vec![b'\r']),
                    Key::Named(NamedKey::Backspace) => Some(vec![0x7f]),
                    Key::Named(NamedKey::Tab) => {
                        if is_shift {
                            Some(vec![0x1b, b'[', b'Z'])
                        } else {
                            Some(vec![b'\t'])
                        }
                    }
                    Key::Named(NamedKey::Escape) => Some(vec![0x1b]),
                    Key::Named(NamedKey::Delete) => Some(vec![0x1b, b'[', b'3', b'~']),
                    Key::Named(NamedKey::Insert) => Some(vec![0x1b, b'[', b'2', b'~']),
                    Key::Named(NamedKey::Home) => {
                        if app_cursor {
                            Some(b"\x1bOH".to_vec())
                        } else {
                            Some(b"\x1b[H".to_vec())
                        }
                    }
                    Key::Named(NamedKey::End) => {
                        if app_cursor {
                            Some(b"\x1bOF".to_vec())
                        } else {
                            Some(b"\x1b[F".to_vec())
                        }
                    }
                    Key::Named(NamedKey::PageUp) => Some(vec![0x1b, b'[', b'5', b'~']),
                    Key::Named(NamedKey::PageDown) => Some(vec![0x1b, b'[', b'6', b'~']),

                    Key::Named(NamedKey::ArrowUp) => {
                        if is_ctrl {
                            Some(b"\x1b[1;5A".to_vec())
                        } else if is_alt {
                            Some(b"\x1b[1;3A".to_vec())
                        } else if is_shift {
                            Some(b"\x1b[1;2A".to_vec())
                        } else if app_cursor {
                            Some(b"\x1bOA".to_vec())
                        } else {
                            Some(b"\x1b[A".to_vec())
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if is_ctrl {
                            Some(b"\x1b[1;5B".to_vec())
                        } else if is_alt {
                            Some(b"\x1b[1;3B".to_vec())
                        } else if is_shift {
                            Some(b"\x1b[1;2B".to_vec())
                        } else if app_cursor {
                            Some(b"\x1bOB".to_vec())
                        } else {
                            Some(b"\x1b[B".to_vec())
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if is_ctrl {
                            Some(b"\x1b[1;5C".to_vec())
                        } else if is_alt {
                            Some(b"\x1b[1;3C".to_vec())
                        } else if is_shift {
                            Some(b"\x1b[1;2C".to_vec())
                        } else if app_cursor {
                            Some(b"\x1bOC".to_vec())
                        } else {
                            Some(b"\x1b[C".to_vec())
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if is_ctrl {
                            Some(b"\x1b[1;5D".to_vec())
                        } else if is_alt {
                            Some(b"\x1b[1;3D".to_vec())
                        } else if is_shift {
                            Some(b"\x1b[1;2D".to_vec())
                        } else if app_cursor {
                            Some(b"\x1bOD".to_vec())
                        } else {
                            Some(b"\x1b[D".to_vec())
                        }
                    }

                    Key::Named(NamedKey::F1) => Some(b"\x1bOP".to_vec()),
                    Key::Named(NamedKey::F2) => Some(b"\x1bOQ".to_vec()),
                    Key::Named(NamedKey::F3) => Some(b"\x1bOR".to_vec()),
                    Key::Named(NamedKey::F4) => Some(b"\x1bOS".to_vec()),
                    Key::Named(NamedKey::F5) => Some(b"\x1b[15~".to_vec()),
                    Key::Named(NamedKey::F6) => Some(b"\x1b[17~".to_vec()),
                    Key::Named(NamedKey::F7) => Some(b"\x1b[18~".to_vec()),
                    Key::Named(NamedKey::F8) => Some(b"\x1b[19~".to_vec()),
                    Key::Named(NamedKey::F9) => Some(b"\x1b[20~".to_vec()),
                    Key::Named(NamedKey::F10) => Some(b"\x1b[21~".to_vec()),
                    Key::Named(NamedKey::F11) => Some(b"\x1b[23~".to_vec()),
                    Key::Named(NamedKey::F12) => Some(b"\x1b[24~".to_vec()),

                    _ => {
                        if let Some(t) = text {
                            let mut b = t.as_bytes().to_vec();
                            if is_alt {
                                let mut prefixed = vec![0x1b];
                                prefixed.extend(b);
                                b = prefixed;
                            }
                            Some(b)
                        } else if let Key::Character(s) = logical_key {
                            let mut b = s.as_bytes().to_vec();
                            if is_alt {
                                let mut prefixed = vec![0x1b];
                                prefixed.extend(b);
                                b = prefixed;
                            }
                            Some(b)
                        } else {
                            None
                        }
                    }
                };

                if let Some(b) = bytes {
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                        if let Some(pane) = tab.active_pane_mut() {
                            let grid = pane.parser.grid_mut();
                            grid.set_selection(None);
                            grid.scroll_offset = 0;
                            let _ = pane.pty.write_bytes(&b);
                        }
                    }
                    if let Some(win) = &self.window {
                        win.request_redraw();
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => (y * 3.0) as i32,
                    MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0) as i32,
                };

                if lines != 0 {
                    if self.tabs.len() > 1 && self.last_cursor_pos.1 < 28.0 {
                        if lines > 0 {
                            self.active_tab_index =
                                (self.active_tab_index + self.tabs.len() - 1) % self.tabs.len();
                        } else {
                            self.active_tab_index = (self.active_tab_index + 1) % self.tabs.len();
                        }
                        self.update_window_title();
                        if let Some(win) = &self.window {
                            win.request_redraw();
                        }
                        return;
                    }

                    let has_tabs = self.has_tabs();
                    let shift_key = self.modifiers.shift_key();
                    let (col, row) = if let Some(renderer) = &self.renderer {
                        renderer.pixel_to_cell(self.last_cursor_pos.0, self.last_cursor_pos.1, has_tabs)
                    } else {
                        (0, 0)
                    };

                    if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                        let geoms = tab.geometries(self.cols, self.rows);
                        let target_pane_id = geoms
                            .iter()
                            .find(|g| g.contains(col, row))
                            .map(|g| (g.pane_id, col - g.col_offset, row - g.row_offset))
                            .unwrap_or((tab.active_pane_id, col, row));

                        if let Some(pane) = tab.pane_mut(target_pane_id.0) {
                            if pane.parser.mouse_tracking && !shift_key {
                                let btn = if lines > 0 { 64 } else { 65 };
                                for _ in 0..lines.abs() {
                                    self.send_mouse_event(btn, target_pane_id.1, target_pane_id.2, true);
                                }
                            } else if pane.parser.use_alt {
                                let code = if lines > 0 { b"\x1b[A" } else { b"\x1b[B" };
                                for _ in 0..lines.abs() {
                                    let _ = pane.pty.write_bytes(code);
                                }
                            } else {
                                pane.parser.grid_mut().scroll(lines);
                                if let Some(win) = &self.window {
                                    win.request_redraw();
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor_pos = (position.x as f32, position.y as f32);

                // Panel sınır çizgisi sürükleme (Split Divider Drag Resizing)
                if let DragState::SplitResize {
                    split_index,
                    direction,
                    start_pixel,
                    initial_ratio,
                    total_span_pixels,
                } = self.drag_state
                {
                    let delta_ratio = match direction {
                        SplitDirection::Horizontal => {
                            (self.last_cursor_pos.0 - start_pixel) / total_span_pixels
                        }
                        SplitDirection::Vertical => {
                            (self.last_cursor_pos.1 - start_pixel) / total_span_pixels
                        }
                    };
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                        if tab.tree.set_split_ratio_by_index(split_index, initial_ratio + delta_ratio) {
                            self.update_grid_size();
                            if let Some(win) = &self.window {
                                let cursor_icon = match direction {
                                    SplitDirection::Horizontal => winit::window::CursorIcon::ColResize,
                                    SplitDirection::Vertical => winit::window::CursorIcon::RowResize,
                                };
                                win.set_cursor(cursor_icon);
                                win.request_redraw();
                            }
                        }
                    }
                    return;
                }

                let has_header = self.has_header_bar();
                if has_header && self.last_cursor_pos.1 < 30.0 {
                    let win_w = self.window.as_ref().map(|w| w.inner_size().width as f32).unwrap_or(800.0);
                    let new_hover = get_top_bar_hit(self.last_cursor_pos.0, self.last_cursor_pos.1, win_w, self.tabs.len());

                    if new_hover != self.hovered_top_bar {
                        self.hovered_top_bar = new_hover;
                        self.update_window_title();
                        if let Some(renderer) = &mut self.renderer {
                            renderer.hovered_top_bar = new_hover;
                        }
                        if let Some(win) = &self.window {
                            win.request_redraw();
                        }
                    }

                    if let Some(win) = &self.window {
                        let cursor_icon = match new_hover {
                            Some(TopBarHit::TabClose(_))
                            | Some(TopBarHit::NewTab)
                            | Some(TopBarHit::Action(_)) => winit::window::CursorIcon::Pointer,
                            Some(TopBarHit::Tab(_)) => winit::window::CursorIcon::Default,
                            None => winit::window::CursorIcon::Default,
                        };
                        win.set_cursor(cursor_icon);
                    }
                    return;
                } else if self.hovered_top_bar.is_some() {
                    self.hovered_top_bar = None;
                    self.update_window_title();
                    if let Some(renderer) = &mut self.renderer {
                        renderer.hovered_top_bar = None;
                        renderer.pressed_top_bar = None;
                    }
                    if let Some(win) = &self.window {
                        win.request_redraw();
                    }
                }

                // Panel sınır çizgisi üzerine gelme kontrolü (Divider Hover)
                if let Some(div) = self.find_divider_at(self.last_cursor_pos.0, self.last_cursor_pos.1) {
                    if let Some(win) = &self.window {
                        let cursor_icon = match div.direction {
                            SplitDirection::Horizontal => winit::window::CursorIcon::ColResize,
                            SplitDirection::Vertical => winit::window::CursorIcon::RowResize,
                        };
                        win.set_cursor(cursor_icon);
                    }
                    return;
                }

                let (col, row) = if let Some(renderer) = &self.renderer {
                    renderer.pixel_to_cell(self.last_cursor_pos.0, self.last_cursor_pos.1, has_header)
                } else {
                    (0, 0)
                };

                let is_ctrl = self.modifiers.control_key();
                let is_shift = self.modifiers.shift_key();
                let mouse_pressed = self.mouse_pressed;
                let selection_start = self.selection_start;

                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    let geoms = tab.geometries(self.cols, self.rows);
                    let target_info = geoms
                        .iter()
                        .find(|g| g.contains(col, row))
                        .map(|g| (g.pane_id, col - g.col_offset, row - g.row_offset))
                        .unwrap_or((tab.active_pane_id, col, row));

                    if let Some(pane) = tab.pane_mut(target_info.0) {
                        let (curr_col, curr_row) = (target_info.1, target_info.2);
                        if is_ctrl {
                            let is_link = pane.parser.grid().get_url_at(curr_col, curr_row).is_some()
                                || pane.parser.grid().get_file_target_at(curr_col, curr_row).is_some();
                            if let Some(win) = &self.window {
                                win.set_cursor(if is_link {
                                    winit::window::CursorIcon::Pointer
                                } else {
                                    winit::window::CursorIcon::Text
                                });
                            }
                        } else if let Some(win) = &self.window {
                            win.set_cursor(winit::window::CursorIcon::Text);
                        }

                        if pane.parser.mouse_tracking && !is_shift {
                            if mouse_pressed {
                                if self.last_cell != Some((curr_col, curr_row)) {
                                    self.last_cell = Some((curr_col, curr_row));
                                    self.send_mouse_event(32, curr_col, curr_row, true);
                                }
                            }
                        } else if mouse_pressed {
                            if let Some((start_col, start_row)) = selection_start {
                                pane.parser.grid_mut().set_selection(
                                    Some(Selection::new(start_col, start_row, curr_col, curr_row)),
                                );
                                if let Some(win) = &self.window {
                                    win.request_redraw();
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                use winit::event::MouseButton;

                // Sürükleme bitişi
                if button == MouseButton::Left && state == ElementState::Released {
                    if self.drag_state != DragState::None {
                        self.drag_state = DragState::None;
                        return;
                    }
                }

                let has_header = self.has_header_bar();

                // Üst tek parça araç çubuğu ve sekme kontrolü
                if has_header && self.last_cursor_pos.1 < 30.0 {
                    let win_w = self.window.as_ref().map(|w| w.inner_size().width as f32).unwrap_or(800.0);
                    let hit = get_top_bar_hit(self.last_cursor_pos.0, self.last_cursor_pos.1, win_w, self.tabs.len());

                    if state == ElementState::Pressed {
                        if button == MouseButton::Left {
                            if let Some(h) = hit {
                                if let Some(renderer) = &mut self.renderer {
                                    renderer.pressed_top_bar = Some(h);
                                }
                                if let Some(win) = &self.window {
                                    win.request_redraw();
                                }
                            }
                        } else if button == MouseButton::Middle {
                            if let Some(TopBarHit::Tab(idx) | TopBarHit::TabClose(idx)) = hit {
                                if idx < self.tabs.len() {
                                    self.close_tab(idx, event_loop);
                                }
                            }
                        }
                        return;
                    } else if state == ElementState::Released && button == MouseButton::Left {
                        let prev_pressed = self.renderer.as_mut().and_then(|r| r.pressed_top_bar.take());
                        if let Some(win) = &self.window {
                            win.request_redraw();
                        }

                        if let (Some(pressed), Some(current_hit)) = (prev_pressed, hit) {
                            if pressed == current_hit {
                                match current_hit {
                                    TopBarHit::Action(action) => {
                                        match action {
                                            HeaderAction::SplitHorizontal => {
                                                let _ = self.split_active_pane(SplitDirection::Horizontal);
                                            }
                                            HeaderAction::SplitVertical => {
                                                let _ = self.split_active_pane(SplitDirection::Vertical);
                                            }
                                            HeaderAction::Find => {
                                                self.toggle_search();
                                            }
                                            HeaderAction::Menu => {
                                                self.toggle_command_palette();
                                            }
                                        }
                                        if let Some(win) = &self.window {
                                            win.request_redraw();
                                        }
                                    }
                                    TopBarHit::NewTab => {
                                        let cwd = self.get_active_cwd();
                                        let _ = self.spawn_tab(None, cwd);
                                        if let Some(win) = &self.window {
                                            win.request_redraw();
                                        }
                                    }
                                    TopBarHit::TabClose(idx) => {
                                        self.close_tab(idx, event_loop);
                                    }
                                    TopBarHit::Tab(idx) => {
                                        if idx < self.tabs.len() {
                                            self.active_tab_index = idx;
                                            self.update_window_title();
                                            if let Some(win) = &self.window {
                                                win.request_redraw();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        return;
                    }
                }

                // Eğer mouse başka bir yerde bırakıldıysa basılı buton durumunu sıfırla
                if state == ElementState::Released {
                    if let Some(renderer) = &mut self.renderer {
                        if renderer.pressed_top_bar.is_some() {
                            renderer.pressed_top_bar = None;
                            if let Some(win) = &self.window {
                                win.request_redraw();
                            }
                        }
                    }
                }

                // Panel sınır çizgisine tıklama ve sürükleme başlatma
                if button == MouseButton::Left && state == ElementState::Pressed {
                    if let Some(div) = self.find_divider_at(self.last_cursor_pos.0, self.last_cursor_pos.1) {
                        let (start_pixel, total_span_pixels) = if let Some(renderer) = &self.renderer {
                            match div.direction {
                                SplitDirection::Horizontal => (
                                    self.last_cursor_pos.0,
                                    div.total_span as f32 * renderer.cell_width,
                                ),
                                SplitDirection::Vertical => (
                                    self.last_cursor_pos.1,
                                    div.total_span as f32 * renderer.cell_height,
                                ),
                            }
                        } else {
                            (0.0, 100.0)
                        };
                        self.drag_state = DragState::SplitResize {
                            split_index: div.split_index,
                            direction: div.direction,
                            start_pixel,
                            initial_ratio: div.current_ratio,
                            total_span_pixels: total_span_pixels.max(1.0),
                        };
                        return;
                    }
                }

                let (col, row) = if let Some(renderer) = &self.renderer {
                    renderer.pixel_to_cell(self.last_cursor_pos.0, self.last_cursor_pos.1, has_header)
                } else {
                    (0, 0)
                };

                // Tıklanan paneli aktif yap ve göreceli koordinatları bul
                let (target_pane_id, rel_col, rel_row) = if let Some(tab) = self.tabs.get(self.active_tab_index) {
                    let geoms = tab.geometries(self.cols, self.rows);
                    if let Some(geom) = geoms.iter().find(|g| g.contains(col, row)) {
                        (geom.pane_id, col - geom.col_offset, row - geom.row_offset)
                    } else {
                        (tab.active_pane_id, col, row)
                    }
                } else {
                    return;
                };

                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if tab.active_pane_id != target_pane_id {
                        tab.active_pane_id = target_pane_id;
                        if let Some(win) = &self.window {
                            win.request_redraw();
                        }
                    }
                }

                let col = rel_col;
                let row = rel_row;

                // Ctrl + Sol Tık: Tıklanabilir URL veya Dosya Açma
                if button == MouseButton::Left
                    && state == ElementState::Pressed
                    && self.modifiers.control_key()
                {
                    if let Some(tab) = self.active_tab() {
                        if let Some(pane) = tab.active_pane() {
                            if let Some(url) = pane.parser.grid().get_url_at(col, row) {
                                let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
                                return;
                            }

                            if let Some(target) = pane.parser.grid().get_file_target_at(col, row) {
                                let loc = if let Some(l) = target.line {
                                    if let Some(c) = target.col {
                                        format!("{}:{}:{}", target.path, l, c)
                                    } else {
                                        format!("{}:{}", target.path, l)
                                    }
                                } else {
                                    target.path.clone()
                                };

                                let editor = std::env::var("EDITOR").unwrap_or_default();
                                if !editor.is_empty() {
                                    let mut cmd = std::process::Command::new(&editor);
                                    if let Some(l) = target.line {
                                        cmd.arg(format!("+{}", l));
                                    }
                                    let _ = cmd.arg(&target.path).spawn();
                                } else {
                                    let spawned = std::process::Command::new("code")
                                        .arg("-g")
                                        .arg(&loc)
                                        .spawn();
                                    if spawned.is_err() {
                                        let _ = std::process::Command::new("xdg-open")
                                            .arg(&target.path)
                                            .spawn();
                                    }
                                }
                                return;
                            }
                        }
                    }
                }

                // TUI Uygulaması Fare Takibi
                let mouse_tracking = self
                    .active_tab()
                    .and_then(|t| t.active_pane())
                    .map(|p| p.parser.mouse_tracking)
                    .unwrap_or(false);

                if mouse_tracking && !self.modifiers.shift_key() {
                    let btn_code = match button {
                        MouseButton::Left => 0,
                        MouseButton::Middle => 1,
                        MouseButton::Right => 2,
                        _ => return,
                    };
                    let is_press = state == ElementState::Pressed;
                    self.mouse_pressed = is_press;
                    self.last_cell = Some((col, row));
                    self.send_mouse_event(btn_code, col, row, is_press);
                    return;
                }

                if button == MouseButton::Left {
                    match state {
                        ElementState::Pressed => {
                            self.mouse_pressed = true;
                            let now = std::time::Instant::now();
                            if now.duration_since(self.last_click_time).as_millis() < 400 {
                                self.click_count += 1;
                            } else {
                                self.click_count = 1;
                            }
                            self.last_click_time = now;
                            self.selection_start = Some((col, row));

                            let click_count = self.click_count;
                            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                                if let Some(pane) = tab.active_pane_mut() {
                                    let grid = pane.parser.grid_mut();
                                    if click_count == 2 {
                                        grid.select_word(col, row);
                                    } else if click_count >= 3 {
                                        grid.select_line(row);
                                    } else {
                                        grid.set_selection(None);
                                    }
                                }
                            }
                            if let Some(win) = &self.window {
                                win.request_redraw();
                            }
                        }
                        ElementState::Released => {
                            self.mouse_pressed = false;
                        }
                    }
                } else if button == MouseButton::Right && state == ElementState::Pressed {
                    self.paste_from_clipboard();
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.drag_state = DragState::None;
            }
            _ => {}
        }
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut custom_cmd = None;
    let mut working_dir = None;
    let mut custom_title = None;
    let mut hold = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!("project-terminal - Hizli GPU hizlandirmali terminal emulatoru");
                println!();
                println!("Kullanim:");
                println!("  project-terminal [SECENEKLER] [-e <komut> [argumanlar...]]");
                println!();
                println!("Secenekler:");
                println!("  -w, --working-directory <DIZIN>   Baslangic calisma dizini");
                println!("  -t, --title <BASLIK>              Pencere basligi belirle");
                println!("      --hold                        Komut bitince pencereyi kapatma, bekle");
                println!("  -e <KOMUT> [ARGUMANLAR...]        Belirtilen komutu calistir");
                println!("  -v, --version                     Surum bilgisini goster");
                println!("  -h, --help                        Yardim metnini goster");
                return Ok(());
            }
            "-v" | "--version" => {
                println!("project-terminal {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "-w" | "--working-directory" => {
                if i + 1 < args.len() {
                    working_dir = Some(std::path::PathBuf::from(&args[i + 1]));
                    i += 2;
                    continue;
                }
            }
            "-t" | "--title" => {
                if i + 1 < args.len() {
                    custom_title = Some(args[i + 1].clone());
                    i += 2;
                    continue;
                }
            }
            "--hold" => {
                hold = true;
                i += 1;
                continue;
            }
            "-e" => {
                let remaining: Vec<String> = args[i + 1..].to_vec();
                if !remaining.is_empty() {
                    custom_cmd = Some(remaining);
                }
                break;
            }
            _ => {
                if !args[i].starts_with('-') {
                    custom_cmd = Some(args[i..].to_vec());
                    break;
                }
            }
        }
        i += 1;
    }

    let config = Config::load();

    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let proxy = event_loop.create_proxy();
    let mut app = TerminalApp::new(
        proxy,
        100,
        30,
        custom_cmd,
        working_dir,
        custom_title,
        hold,
        config,
    );

    let _ = event_loop.run_app(&mut app);
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_commands_empty_query() {
        let results = filter_commands_by_query("");
        assert_eq!(results.len(), ALL_COMMANDS.len());
    }

    #[test]
    fn test_filter_commands_by_keyword() {
        let results = filter_commands_by_query("böl");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|c| c.id == "split_horizontal"));
        assert!(results.iter().any(|c| c.id == "split_vertical"));

        let results_copy = filter_commands_by_query("kopyala");
        assert!(results_copy.iter().any(|c| c.id == "copy_last_output"));
    }

    #[test]
    fn test_filter_commands_by_shortcut() {
        let results = filter_commands_by_query("Ctrl+Shift+P");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "command_palette");

        let results_osc = filter_commands_by_query("Ctrl+Shift+O");
        assert_eq!(results_osc.len(), 1);
        assert_eq!(results_osc[0].id, "copy_last_output");
    }

    #[test]
    fn test_filter_commands_no_match() {
        let results = filter_commands_by_query("nonexistent_command_xyz");
        assert!(results.is_empty());
    }
}
