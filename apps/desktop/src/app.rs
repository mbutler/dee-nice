use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dee_nice_core::{Key, OutputDisplay, Show};
use eframe::egui;

use crate::derivatives::Library;
use crate::displays::{self, AttachedDisplay};
use crate::media::{self, ScanError};
use crate::readiness::{self, readiness};
use crate::session::{MpvSession, SessionUpdate};
use crate::store::{self, Startup};
use crate::theme::{
    self, ASSIGNED, BACKGROUND, DROP, ERROR_FILL, ERROR_TEXT, PANEL, PRIMARY, PRIMARY_TEXT,
};
use crate::thumbs::Gallery;

const KEY_ROWS: &[(&[&str], f32)] = &[
    (&["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"], 0.0),
    (&["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"], 26.0),
    (&["a", "s", "d", "f", "g", "h", "j", "k", "l"], 52.0),
    (&["z", "x", "c", "v", "b", "n", "m"], 84.0),
];

pub fn run() -> eframe::Result<()> {
    if std::env::var_os("DEE_NICE_PRINT_TOOLS").is_some() {
        print_tools();
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("dee-nice")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([1180.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "dee-nice",
        options,
        Box::new(|context| Ok(Box::new(DeeNiceApp::new(context)))),
    )
}

fn print_tools() {
    report("mpv", crate::playback::mpv_binary());
    report("ffmpeg", crate::derivatives::ffmpeg_binary());
    report("ffprobe", crate::derivatives::ffprobe_binary());
}

fn report(name: &str, path: Option<PathBuf>) {
    match path {
        Some(path) => println!("{name}\t{}", path.display()),
        None => println!("{name}\tmissing"),
    }
}

struct DeeNiceApp {
    show: Show,
    show_path: PathBuf,
    support_dir: PathBuf,
    machine: store::Machine,
    save_enabled: bool,
    machine_writable: bool,
    load_block: Option<String>,
    notice: Option<String>,
    clips: Vec<String>,
    clips_truncated: bool,
    scan_note: Option<String>,
    selected: Option<String>,
    displays: Vec<AttachedDisplay>,
    display_error: Option<String>,
    derivatives: Library,
    thumbs: Gallery,
    textures: BTreeMap<String, (PathBuf, egui::TextureHandle)>,
    bad_thumbs: BTreeSet<PathBuf>,
    session: Option<MpvSession>,
    operator_placed: bool,
}

impl DeeNiceApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        install_style(&context.egui_ctx);
        let support_dir = store::default_support_dir();
        let Startup {
            show,
            show_path,
            save_enabled,
            machine_writable,
            notice,
            machine,
        } = store::load_startup(&support_dir);
        let blocked = !save_enabled && show_path.exists();
        let mut app = Self {
            show,
            show_path,
            support_dir: support_dir.clone(),
            machine,
            save_enabled,
            machine_writable,
            load_block: blocked.then_some(notice.clone()).flatten(),
            notice: (!blocked).then_some(notice).flatten(),
            clips: Vec::new(),
            clips_truncated: false,
            scan_note: None,
            selected: None,
            displays: Vec::new(),
            display_error: None,
            derivatives: Library::open(support_dir.join("Derivatives")),
            thumbs: Gallery::open(support_dir.join("Thumbnails")),
            textures: BTreeMap::new(),
            bad_thumbs: BTreeSet::new(),
            session: None,
            operator_placed: false,
        };
        app.refresh_clips();
        app.refresh_displays();
        app
    }

    fn refresh_clips(&mut self) {
        match media::list_clips(self.show.media_directory()) {
            Ok(list) => {
                self.clips = list.clips;
                self.clips_truncated = list.truncated;
                self.scan_note = None;
            }
            Err(ScanError::Missing) => {
                self.clips.clear();
                self.clips_truncated = false;
                self.scan_note = None;
            }
            Err(ScanError::Unreadable(detail)) => {
                self.clips.clear();
                self.clips_truncated = false;
                self.scan_note = Some(detail);
            }
        }
        if self
            .selected
            .as_ref()
            .is_some_and(|clip| !self.clips.iter().any(|listed| listed == clip))
        {
            self.selected = None;
        }
        self.sync_thumbs();
    }

    fn sync_thumbs(&mut self) {
        self.thumbs.sync(self.show.media_directory(), &self.clips);
        self.textures
            .retain(|clip, (path, _)| self.thumbs.path(clip) == Some(path.as_path()));
        self.bad_thumbs.retain(|path| {
            self.clips
                .iter()
                .any(|clip| self.thumbs.path(clip) == Some(path.as_path()))
        });
    }

    fn refresh_displays(&mut self) {
        match displays::attached_displays() {
            Ok(displays) => {
                self.display_error = None;
                let names: Vec<_> = displays
                    .iter()
                    .map(|display| display.name.clone())
                    .collect();
                let had_display = self.show.output_display().is_some();
                readiness::forget_missing_display(&mut self.show, &names);
                let forgot = had_display && self.show.output_display().is_none();
                let only = (displays.len() == 1).then(|| displays[0].clone());
                self.displays = displays;
                if self.show.output_display().is_none()
                    && let Some(display) = only
                {
                    self.choose_display(&display);
                } else if forgot {
                    self.remember_display();
                    self.persist_machine();
                }
                self.sync_derivatives();
            }
            Err(error) => {
                self.displays.clear();
                self.display_error = Some(error);
            }
        }
    }

    fn choose_display(&mut self, display: &AttachedDisplay) {
        match OutputDisplay::new(display.name.clone(), display.index) {
            Ok(output) => {
                self.show.set_output_display(Some(output));
                self.notice = None;
                self.remember_display();
                self.persist_machine();
                self.sync_derivatives();
            }
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    fn connected_names(&self) -> Vec<String> {
        if self.display_error.is_some() {
            return self
                .show
                .output_display()
                .map(|display| display.name().to_string())
                .into_iter()
                .collect();
        }
        self.displays
            .iter()
            .map(|display| display.name.clone())
            .collect()
    }

    fn remember_display(&mut self) {
        if !self.save_enabled {
            return;
        }
        let folder = self.show.media_directory().to_path_buf();
        self.machine.set_last_folder(&folder);
        self.machine
            .set_display(&folder, self.show.output_display());
    }

    fn persist_show(&mut self) {
        if !self.save_enabled {
            return;
        }
        if let Err(error) = store::save(&self.show_path, &self.show) {
            self.notice = Some(error);
        }
    }

    fn sync_derivatives(&mut self) {
        if !self.save_enabled {
            return;
        }
        let edge = self.chosen_pixel_edge();
        self.derivatives.sync(&self.show, edge);
        self.surface_derivative_notes();
    }

    fn chosen_pixel_edge(&self) -> Option<u32> {
        let name = self.show.output_display()?.name();
        self.displays
            .iter()
            .find(|display| display.name == name)
            .map(|display| display.pixel_long_edge)
    }

    fn derivative_line(&self) -> String {
        self.derivatives.status_line(self.chosen_pixel_edge())
    }

    fn surface_derivative_notes(&mut self) {
        for failure in self.derivatives.take_failures() {
            self.notice = Some(failure);
        }
    }

    fn persist_machine(&mut self) {
        if !self.machine_writable || !self.save_enabled {
            return;
        }
        if let Err(error) = store::save_machine(&self.support_dir, &self.machine) {
            self.notice = Some(error);
        }
    }

    fn set_folder(&mut self, folder: PathBuf) {
        match store::open_folder(&folder, &self.machine) {
            Ok(opened) => {
                self.show = opened.show;
                self.show_path = opened.show_path;
                self.machine = opened.machine;
                self.save_enabled = true;
                self.load_block = None;
                self.notice = None;
                self.selected = None;
                self.refresh_clips();
                self.refresh_displays();
                self.persist_show();
                self.persist_machine();
            }
            Err(error) => self.notice = Some(error),
        }
    }

    fn assign_selected(&mut self, key: Key) {
        let Some(clip) = self.selected.clone() else {
            self.notice = Some("Select a video first.".to_string());
            return;
        };
        if self.show.clip(key) == Some(clip.as_str()) {
            self.show.clear(key);
        } else if let Err(error) = self.show.assign(key, &clip) {
            self.notice = Some(error.to_string());
            return;
        }
        self.notice = None;
        self.persist_show();
        self.sync_derivatives();
        self.push_bindings();
    }

    fn clear_key(&mut self, key: Key) {
        self.show.clear(key);
        self.notice = None;
        self.persist_show();
        self.sync_derivatives();
        self.push_bindings();
    }

    fn try_enter(&mut self, ctx: &egui::Context) {
        if self.session.is_some() {
            return;
        }
        if let Err(reason) = readiness(&self.show, &self.connected_names()) {
            self.notice = Some(reason);
            return;
        }
        match MpvSession::start(&self.show, self.derivatives.ready()) {
            Ok(session) => {
                self.session = Some(session);
                self.notice = None;
                self.operator_placed = false;
                self.apply_show_window(ctx);
                ctx.request_repaint();
            }
            Err(error) => self.notice = Some(error),
        }
    }

    fn operator_display(&self) -> Option<&crate::displays::AttachedDisplay> {
        let output = self
            .show
            .output_display()
            .map(|display| display.name().to_string())
            .unwrap_or_default();
        displays::operator_display(self.machine.presentation(), &output, &self.displays)
    }

    fn apply_show_window(&mut self, ctx: &egui::Context) {
        let place = self
            .operator_display()
            .map(|display| (display.origin_x, display.origin_y));
        let Some((x, y)) = place else {
            self.operator_placed = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        if self.operator_placed {
            return;
        }
        let margin = 40.0;
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            x as f32 + margin,
            y as f32 + margin,
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.operator_placed = true;
    }

    fn push_bindings(&mut self) {
        if let Some(session) = self.session.as_mut() {
            session.replace_bindings(&self.show);
        }
    }

    fn presentation_line(&self) -> &'static str {
        if !self.machine.presentation() {
            "Kiosk hides this window while the show plays."
        } else if self.displays.len() > 1 {
            "Presentation keeps this window on the other display."
        } else {
            "Presentation needs a second display, so this window will hide."
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        ui.heading("dee-nice");
        ui.label("Assign a video to a key, then play it fullscreen. Clips loop. A key cuts to the next clip.");
        if let Some(load_block) = self.load_block.clone() {
            error_banner(ui, &load_block);
            if ui.button("Save over the show file").clicked() {
                self.save_enabled = true;
                self.load_block = None;
                self.persist_show();
            }
        }
        if let Some(notice) = self.notice.clone() {
            error_banner(ui, &notice);
        }
        self.toolbar(ui);
        ui.add_space(8.0);
        ui.separator();
        let available = ui.available_height() - 28.0;
        ui.horizontal_top(|ui| {
            let left_width = (ui.available_width() * 0.28).clamp(220.0, 320.0);
            ui.allocate_ui_with_layout(
                egui::vec2(left_width, available),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.video_column(ui),
            );
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), available),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.key_column(ui),
            );
        });
        ui.separator();
        self.footer(ui);
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.save_enabled || self.show.media_directory().is_dir() {
                ui.label(self.show.media_directory().display().to_string());
            } else {
                ui.label("Choose a folder of videos.");
            }
            if ui.button("Choose folder").clicked()
                && let Some(folder) = rfd::FileDialog::new()
                    .set_title("Choose a folder of videos")
                    .pick_folder()
            {
                self.set_folder(folder);
            }
            if ui.button("Rescan").clicked() {
                self.refresh_clips();
            }
        });
        ui.horizontal(|ui| {
            self.display_picker(ui);
            if ui.button("Refresh displays").clicked() {
                self.refresh_displays();
            }
        });
        ui.horizontal(|ui| {
            let mut muted = self.show.muted();
            if ui.checkbox(&mut muted, "Mute").changed() {
                self.show.set_muted(muted);
                self.persist_show();
            }
            let mut start_locked = self.show.start_locked();
            if ui.checkbox(&mut start_locked, "Start locked").changed() {
                self.show.set_start_locked(start_locked);
                self.persist_show();
            }
            let mut presentation = self.machine.presentation();
            if ui.checkbox(&mut presentation, "Presentation").changed() {
                self.machine.set_presentation(presentation);
                self.operator_placed = false;
                if self.machine_writable
                    && let Err(error) = store::save_machine(&self.support_dir, &self.machine)
                {
                    self.notice = Some(error);
                }
            }
        });
        ui.weak(self.presentation_line());
        ui.label(self.derivative_line());
        if let Some(name) = self.derivatives.preparing() {
            ui.weak(format!("Preparing a lighter copy of {name}."));
        }
        if self.show.start_locked() {
            match readiness::starting_clip(&self.show) {
                Some((key, clip)) => {
                    ui.label(format!("Display mode starts locked on {key}: {clip}"));
                }
                None => {
                    ui.label("Start locked needs an assigned clip.");
                }
            }
        }
        if self.session.is_some() {
            self.show_controls(ui);
        } else {
            ui.horizontal(|ui| {
                let blocked = readiness(&self.show, &self.connected_names()).err();
                let button = egui::Button::new(
                    egui::RichText::new("Enter display mode")
                        .size(16.0)
                        .color(PRIMARY_TEXT),
                )
                .fill(PRIMARY)
                .min_size(egui::vec2(210.0, 36.0));
                if ui
                    .add_enabled(blocked.is_none() && self.session.is_none(), button)
                    .clicked()
                {
                    self.try_enter(ui.ctx());
                }
                if let Some(reason) = blocked {
                    ui.label(reason);
                }
            });
        }
        ui.label("Drag a video onto a key. Click a key to assign the selected video. Right-click a key to clear it.");
        ui.label("Ctrl+Shift+L locks switching. Ctrl+Shift+K leaves display mode.");
    }

    fn show_controls(&mut self, ui: &mut egui::Ui) {
        let display = self
            .show
            .output_display()
            .map(|display| display.name().to_string())
            .unwrap_or_else(|| "the chosen display".to_string());
        let live = self
            .session
            .as_ref()
            .and_then(|session| session.live_key())
            .map(str::to_string);
        let locked = self
            .session
            .as_ref()
            .is_some_and(|session| session.is_locked());
        ui.label(format!("Playing on {display}."));
        if let Some(name) = live.as_deref() {
            let label = Key::from_name(name)
                .map(|key| key.label())
                .unwrap_or_else(|| name.to_string());
            ui.label(format!("Live: {label}"));
        } else {
            ui.label("Waiting for a key.");
        }
        ui.horizontal(|ui| {
            let lock_label = if locked { "Unlock keys" } else { "Lock keys" };
            if ui.button(lock_label).clicked()
                && let Some(session) = self.session.as_mut()
            {
                session.toggle_lock();
            }
            if ui.button("Leave display mode").clicked()
                && let Some(session) = self.session.as_mut()
            {
                session.quit();
            }
        });
    }

    fn display_picker(&mut self, ui: &mut egui::Ui) {
        let saved = self
            .show
            .output_display()
            .map(|display| display.name().to_string());
        let selected_text = saved
            .clone()
            .unwrap_or_else(|| "Choose a display".to_string());
        let mut chosen = None;
        egui::ComboBox::from_id_salt("output-display")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for display in &self.displays {
                    let selected = saved.as_deref() == Some(display.name.as_str());
                    if ui.selectable_label(selected, &display.label).clicked() {
                        chosen = Some(display.clone());
                    }
                }
            });
        if let Some(display) = chosen {
            self.choose_display(&display);
        }
        if let Some(error) = &self.display_error {
            ui.label(format!("Displays could not be listed: {error}"));
        }
    }

    fn video_column(&mut self, ui: &mut egui::Ui) {
        ui.heading("Videos");
        if let Some(note) = &self.scan_note {
            ui.colored_label(ERROR_TEXT, note);
        }
        if self.clips.is_empty() {
            ui.label("Choose a folder of videos.");
            return;
        }
        if self.clips_truncated {
            ui.label("The list stopped at 2000 files.");
        }
        let clips = self.clips.clone();
        let mut pick = None;
        egui::ScrollArea::vertical()
            .id_salt("videos")
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for clip in &clips {
                        if self.video_card(ui, clip) {
                            pick = Some(clip.clone());
                        }
                    }
                });
            });
        if let Some(clip) = pick {
            self.selected = Some(clip);
        }
    }

    fn video_card(&mut self, ui: &mut egui::Ui, clip: &str) -> bool {
        let id = ui.make_persistent_id(("video", clip));
        let texture = self.texture_for(ui.ctx(), clip);
        let selected = self.selected.as_deref() == Some(clip);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(148.0, 112.0), egui::Sense::click_and_drag());
        if response.dragged() {
            response.dnd_set_drag_payload(clip.to_string());
            self.paint_drag_preview(ui, id, rect, texture.as_ref(), clip);
        }
        paint_card(ui.painter(), rect, clip, texture.as_ref(), selected, false);
        response
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text(clip)
            .clicked()
    }

    fn paint_drag_preview(
        &self,
        ui: &egui::Ui,
        id: egui::Id,
        rect: egui::Rect,
        texture: Option<&egui::TextureHandle>,
        clip: &str,
    ) {
        let Some(pointer) = ui.ctx().pointer_interact_pos() else {
            return;
        };
        let layer = egui::LayerId::new(egui::Order::Tooltip, id);
        let preview = egui::Rect::from_center_size(pointer, rect.size());
        paint_card(
            &ui.ctx().layer_painter(layer),
            preview,
            clip,
            texture,
            true,
            true,
        );
    }

    fn key_column(&mut self, ui: &mut egui::Ui) {
        ui.heading("Keyboard");
        let selected = self.selected.as_deref().unwrap_or("none");
        ui.weak(format!("Selected: {selected}"));
        egui::ScrollArea::vertical().id_salt("keys").show(ui, |ui| {
            for (row, indent) in KEY_ROWS {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.add_space(*indent);
                    for name in *row {
                        self.key_button(ui, name);
                    }
                });
                ui.add_space(4.0);
            }
        });
    }

    fn key_button(&mut self, ui: &mut egui::Ui, name: &str) {
        let Some(key) = Key::from_name(name) else {
            return;
        };
        let assigned = self.show.clip(key).map(str::to_string);
        let texture = assigned
            .as_deref()
            .and_then(|clip| self.texture_for(ui.ctx(), clip));
        let (rect, response) = ui.allocate_exact_size(theme::KEY_SIZE, egui::Sense::click());
        let live = self.session.as_ref().and_then(|session| session.live_key()) == Some(name);
        let drag_hot = response.dnd_hover_payload::<String>().is_some();
        let fill = if drag_hot {
            DROP
        } else if assigned.is_some() {
            ASSIGNED
        } else {
            PANEL
        };
        let painter = ui.painter();
        painter.rect_filled(rect, egui::CornerRadius::same(6), fill);
        let stroke = if live {
            egui::Stroke::new(2.5, PRIMARY)
        } else {
            egui::Stroke::new(1.0, egui::Color32::from_gray(58))
        };
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(6),
            stroke,
            egui::StrokeKind::Inside,
        );
        if let Some(texture) = &texture {
            let image_rect = fitted_rect(rect.shrink(4.0), texture.size_vec2());
            painter.image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        let letter_pos = if assigned.is_some() {
            rect.left_top() + egui::vec2(14.0, 12.0)
        } else {
            rect.center()
        };
        if assigned.is_some() {
            painter.rect_filled(
                egui::Rect::from_center_size(letter_pos, egui::vec2(22.0, 16.0)),
                egui::CornerRadius::same(3),
                egui::Color32::from_black_alpha(180),
            );
        }
        painter.text(
            letter_pos,
            egui::Align2::CENTER_CENTER,
            key.label(),
            egui::FontId::proportional(if assigned.is_some() { 12.0 } else { 18.0 }),
            PRIMARY,
        );
        if let Some(clip) = &assigned {
            response.clone().on_hover_text(clip);
        }
        if let Some(clip) = response.dnd_release_payload::<String>() {
            self.assign_clip(key, clip.as_str());
        } else if response.clicked() {
            self.assign_selected(key);
        } else if response.secondary_clicked() {
            self.clear_key(key);
        }
    }

    fn assign_clip(&mut self, key: Key, clip: &str) {
        if let Err(error) = self.show.assign(key, clip) {
            self.notice = Some(error.to_string());
            return;
        }
        self.selected = Some(clip.to_string());
        self.notice = None;
        self.persist_show();
        self.sync_derivatives();
        self.push_bindings();
    }

    fn texture_for(&mut self, ctx: &egui::Context, clip: &str) -> Option<egui::TextureHandle> {
        let path = self.thumbs.path(clip)?.to_path_buf();
        if self.bad_thumbs.contains(&path) {
            return None;
        }
        if let Some((cached, texture)) = self.textures.get(clip)
            && cached == &path
        {
            return Some(texture.clone());
        }
        let Some(image) = load_png(&path) else {
            self.bad_thumbs.insert(path);
            return None;
        };
        let texture = ctx.load_texture(
            path.display().to_string(),
            image,
            egui::TextureOptions::LINEAR,
        );
        self.textures
            .insert(clip.to_string(), (path, texture.clone()));
        Some(texture)
    }

    fn footer(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.weak(format!("Show file: {}", self.show_path.display()));
            if crate::playback::mpv_binary().is_none() {
                ui.weak("mpv was not found.");
            }
        });
    }
}

impl eframe::App for DeeNiceApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.derivatives.profile_changed() {
            self.sync_derivatives();
        }
        if self.derivatives.poll() {
            if let Some(session) = self.session.as_mut() {
                session.replace_ready(self.derivatives.ready());
            }
            self.surface_derivative_notes();
        }
        if self.thumbs.poll() {
            ctx.request_repaint();
        }
        if self.thumbs.take_missing_tool() && self.notice.is_none() {
            self.notice = Some("ffmpeg was not found, so videos have no thumbnails.".to_string());
        }
        if self.derivatives.busy() || self.thumbs.busy() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        if self.session.is_none() {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        self.apply_show_window(ctx);
        let update = self.session.as_mut().map(MpvSession::poll);
        if let Some(SessionUpdate::Stopped(message)) = update {
            self.session = None;
            self.operator_placed = false;
            if message != "Left display mode." {
                self.notice = Some(message);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let style = ui.style().clone();
        egui::Frame::central_panel(&style).show(ui, |ui| self.draw(ui));
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        let color = egui::Rgba::from(visuals.window_fill());
        color.to_array()
    }
}

fn install_style(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = BACKGROUND;
    visuals.panel_fill = PANEL;
    visuals.extreme_bg_color = extreme_background();
    ctx.set_visuals(visuals);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
    });
}

fn extreme_background() -> egui::Color32 {
    egui::Color32::from_rgb(16, 17, 20)
}

fn error_banner(ui: &mut egui::Ui, message: &str) {
    egui::Frame::NONE
        .fill(ERROR_FILL)
        .inner_margin(8)
        .corner_radius(6)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(message).color(ERROR_TEXT));
        });
}

fn paint_card(
    painter: &egui::Painter,
    rect: egui::Rect,
    clip: &str,
    texture: Option<&egui::TextureHandle>,
    selected: bool,
    dragging: bool,
) {
    let fill = if dragging { DROP } else { PANEL };
    let stroke = if selected {
        egui::Stroke::new(2.0, PRIMARY)
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_gray(55))
    };
    painter.rect_filled(rect, egui::CornerRadius::same(6), fill);
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(6),
        stroke,
        egui::StrokeKind::Inside,
    );
    let image_bounds =
        egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 4.0), theme::CARD_THUMB);
    painter.rect_filled(
        image_bounds,
        egui::CornerRadius::same(4),
        extreme_background(),
    );
    if let Some(texture) = texture {
        painter.image(
            texture.id(),
            fitted_rect(image_bounds, texture.size_vec2()),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    painter.text(
        egui::pos2(rect.left() + 8.0, image_bounds.bottom() + 14.0),
        egui::Align2::LEFT_CENTER,
        file_label(clip),
        egui::FontId::proportional(12.0),
        PRIMARY,
    );
}

fn file_label(clip: &str) -> String {
    let name = clip.rsplit('/').next().unwrap_or(clip);
    let count = name.chars().count();
    if count > 16 {
        let truncated: String = name.chars().take(15).collect();
        format!("{truncated}…")
    } else {
        name.to_string()
    }
}

fn fitted_rect(bounds: egui::Rect, image: egui::Vec2) -> egui::Rect {
    if image.x <= 0.0 || image.y <= 0.0 {
        return bounds;
    }
    let scale = (bounds.width() / image.x).min(bounds.height() / image.y);
    egui::Rect::from_center_size(bounds.center(), image * scale)
}

fn load_png(path: &Path) -> Option<egui::ColorImage> {
    let bytes = fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.into_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        image.as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::KEY_ROWS;

    #[test]
    fn the_setup_keyboard_is_letters_and_digits() {
        let mut names = Vec::new();
        for (row, _) in KEY_ROWS {
            names.extend(row.iter().copied());
        }
        assert_eq!(names.len(), 36);
        for name in names {
            let ch = name.chars().next().unwrap();
            assert_eq!(name.len(), 1);
            assert!(ch.is_ascii_alphanumeric());
        }
        assert_eq!(KEY_ROWS[1].0[0], "q");
        assert_eq!(KEY_ROWS[2].0[0], "a");
        assert_eq!(KEY_ROWS[3].0[0], "z");
    }
}
