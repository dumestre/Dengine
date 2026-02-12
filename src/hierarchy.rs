use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, Order, Pos2, Rect, Stroke, TextureHandle, Vec2,
};
use epaint::ColorImage;

pub struct HierarchyWindow {
    pub open: bool,
    selector_icon_texture: Option<TextureHandle>,
    dock_side: Option<HierarchyDockSide>,
    window_pos: Option<Pos2>,
    window_width: f32,
    dragging_from_header: bool,
    resizing_width: bool,
    selected_object: &'static str,
}

#[derive(Clone, Copy)]
enum HierarchyDockSide {
    Left,
    Right,
}

fn load_png_as_texture(ctx: &egui::Context, png_path: &str) -> Option<TextureHandle> {
    let bytes = std::fs::read(png_path).ok()?;
    let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(
        png_path.to_owned(),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}

impl HierarchyWindow {
    pub fn new() -> Self {
        Self {
            open: true,
            selector_icon_texture: None,
            dock_side: Some(HierarchyDockSide::Right),
            window_pos: None,
            window_width: 220.0,
            dragging_from_header: false,
            resizing_width: false,
            selected_object: "Main Camera",
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, left_reserved: f32, right_reserved: f32) {
        if !self.open {
            return;
        }

        if self.selector_icon_texture.is_none() {
            self.selector_icon_texture =
                load_png_as_texture(ctx, "src/assets/icons/seletorcor.png");
        }

        let dock_rect = ctx.available_rect();
        let pointer_down = ctx.input(|i| i.pointer.primary_down());

        let height = if self.dock_side.is_some() {
            dock_rect.height().max(120.0)
        } else {
            (dock_rect.height() * 0.85).max(520.0)
        };
        let max_width = (dock_rect.width() - left_reserved - right_reserved - 40.0).max(180.0);
        self.window_width = self.window_width.clamp(180.0, max_width.min(520.0));
        let window_size = egui::vec2(self.window_width, height);
        let left_snap_x = dock_rect.left() + left_reserved;
        let right_snap_right = dock_rect.right() - right_reserved;

        if self.window_pos.is_none() {
            self.window_pos = Some(egui::pos2(
                right_snap_right - self.window_width,
                dock_rect.top(),
            ));
        }

        if let Some(side) = self.dock_side {
            if !self.dragging_from_header && !self.resizing_width && !pointer_down {
                let x = match side {
                    HierarchyDockSide::Left => left_snap_x,
                    HierarchyDockSide::Right => right_snap_right - self.window_width,
                };
                self.window_pos = Some(egui::pos2(x, dock_rect.top()));
            }
        }

        let pos = self
            .window_pos
            .unwrap_or(egui::pos2(right_snap_right - self.window_width, dock_rect.top()));

        let mut header_drag_started = false;
        let mut header_drag_stopped = false;
        let mut resize_started = false;
        let mut resize_stopped = false;
        let mut panel_rect = Rect::from_min_size(pos, window_size);

        egui::Area::new(Id::new("hierarquia_window_id"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(window_size, egui::Sense::hover());
                panel_rect = rect;

                ui.painter()
                    .rect_filled(rect, 6.0, Color32::from_rgb(28, 28, 28));
                ui.painter().rect_stroke(
                    rect,
                    6.0,
                    Stroke::new(1.0, Color32::from_gray(60)),
                    egui::StrokeKind::Outside,
                );

                let inner = rect.shrink2(egui::vec2(8.0, 6.0));
                let header_h = 18.0;
                let header_rect =
                    Rect::from_min_max(inner.min, egui::pos2(inner.max.x, inner.min.y + header_h));

                let icon_side = 16.0;
                let icon_rect = Rect::from_min_size(
                    egui::pos2(header_rect.max.x - icon_side, header_rect.min.y + 1.0),
                    egui::vec2(icon_side, icon_side),
                );
                let drag_rect =
                    Rect::from_min_max(header_rect.min, egui::pos2(icon_rect.min.x - 4.0, header_rect.max.y));

                let drag_resp = ui.interact(
                    drag_rect,
                    ui.id().with("hierarchy_header_drag"),
                    egui::Sense::click_and_drag(),
                );
                if drag_resp.drag_started() {
                    header_drag_started = true;
                }
                if drag_resp.drag_stopped() {
                    header_drag_stopped = true;
                }

                ui.painter().text(
                    drag_rect.center(),
                    Align2::CENTER_CENTER,
                    "Hierarquia",
                    FontId::new(13.0, FontFamily::Proportional),
                    Color32::WHITE,
                );

                if let Some(icon) = &self.selector_icon_texture {
                    let icon_resp = ui.put(
                        icon_rect,
                        egui::Image::new(icon)
                            .fit_to_exact_size(egui::vec2(icon_side, icon_side))
                            .sense(egui::Sense::click()),
                    );
                    if icon_resp.hovered() {
                        ui.painter().rect_filled(
                            icon_rect.expand2(egui::vec2(2.0, 2.0)),
                            4.0,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 28),
                        );
                    }
                }

                let sep_y = header_rect.max.y + 5.0;
                ui.painter().line_segment(
                    [
                        egui::pos2(inner.min.x, sep_y),
                        egui::pos2(inner.max.x, sep_y),
                    ],
                    Stroke::new(1.0, Color32::from_gray(60)),
                );

                let content_rect = Rect::from_min_max(
                    egui::pos2(inner.min.x, sep_y + 8.0),
                    egui::pos2(inner.max.x, rect.bottom() - 6.0),
                );
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(content_rect).layout(egui::Layout::top_down(
                        egui::Align::Min,
                    )),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("hierarchy_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.style_mut().visuals.selection.bg_fill =
                                    Color32::from_rgb(47, 47, 47);
                                ui.style_mut().visuals.selection.stroke =
                                    Stroke::new(1.0, Color32::from_rgb(15, 232, 121));

                                if ui
                                    .selectable_label(
                                        self.selected_object == "Directional Light",
                                        "Directional Light",
                                    )
                                    .clicked()
                                {
                                    self.selected_object = "Directional Light";
                                }

                                if ui
                                    .selectable_label(
                                        self.selected_object == "Main Camera",
                                        "Main Camera",
                                    )
                                    .clicked()
                                {
                                    self.selected_object = "Main Camera";
                                }

                                egui::CollapsingHeader::new("Player")
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        if ui
                                            .selectable_label(
                                                self.selected_object == "Mesh",
                                                "Mesh",
                                            )
                                            .clicked()
                                        {
                                            self.selected_object = "Mesh";
                                        }
                                        if ui
                                            .selectable_label(
                                                self.selected_object == "Weapon Socket",
                                                "Weapon Socket",
                                            )
                                            .clicked()
                                        {
                                            self.selected_object = "Weapon Socket";
                                        }
                                        egui::CollapsingHeader::new("Armature")
                                            .default_open(true)
                                            .show(ui, |ui| {
                                                if ui
                                                    .selectable_label(
                                                        self.selected_object == "Spine",
                                                        "Spine",
                                                    )
                                                    .clicked()
                                                {
                                                    self.selected_object = "Spine";
                                                }
                                                if ui
                                                    .selectable_label(
                                                        self.selected_object == "Head",
                                                        "Head",
                                                    )
                                                    .clicked()
                                                {
                                                    self.selected_object = "Head";
                                                }
                                            });
                                    });

                                egui::CollapsingHeader::new("Environment")
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        if ui
                                            .selectable_label(self.selected_object == "Terrain", "Terrain")
                                            .clicked()
                                        {
                                            self.selected_object = "Terrain";
                                        }
                                        if ui
                                            .selectable_label(self.selected_object == "Trees", "Trees")
                                            .clicked()
                                        {
                                            self.selected_object = "Trees";
                                        }
                                        if ui
                                            .selectable_label(self.selected_object == "Fog Volume", "Fog Volume")
                                            .clicked()
                                        {
                                            self.selected_object = "Fog Volume";
                                        }
                                    });
                            });
                    },
                );

                let handle_w = 10.0;
                let handle_rect = match self.dock_side {
                    Some(HierarchyDockSide::Right) => Rect::from_min_max(
                        egui::pos2(rect.left(), rect.top()),
                        egui::pos2(rect.left() + handle_w, rect.bottom()),
                    ),
                    _ => Rect::from_min_max(
                        egui::pos2(rect.right() - handle_w, rect.top()),
                        egui::pos2(rect.right(), rect.bottom()),
                    ),
                };
                let resize_resp = ui.interact(
                    handle_rect,
                    ui.id().with("hierarchy_width_resize_handle"),
                    egui::Sense::click_and_drag(),
                );
                if resize_resp.hovered() || resize_resp.dragged() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                }
                if resize_resp.drag_started() {
                    resize_started = true;
                }
                if resize_resp.drag_stopped() {
                    resize_stopped = true;
                }
            });

        if header_drag_started {
            self.dragging_from_header = true;
            self.resizing_width = false;
            self.dock_side = None;
        }
        if resize_started {
            self.resizing_width = true;
            self.dragging_from_header = false;
        }

        let delta = ctx.input(|i| i.pointer.delta());
        if pointer_down {
            if self.dragging_from_header && delta != Vec2::ZERO {
                if let Some(p) = self.window_pos {
                    self.window_pos = Some(p + delta);
                }
            } else if self.resizing_width && delta.x != 0.0 {
                match self.dock_side {
                    Some(HierarchyDockSide::Right) => {
                        let old_w = self.window_width;
                        let new_w = (old_w - delta.x).clamp(180.0, 520.0);
                        let applied = old_w - new_w;
                        self.window_width = new_w;
                        if let Some(p) = self.window_pos {
                            self.window_pos = Some(egui::pos2(p.x + applied, p.y));
                        }
                    }
                    _ => {
                        self.window_width = (self.window_width + delta.x).clamp(180.0, 520.0);
                    }
                }
            }
        }

        let near_left = (panel_rect.left() - left_snap_x).abs() <= 28.0;
        let near_right = (right_snap_right - panel_rect.right()).abs() <= 28.0;
        if self.dragging_from_header && pointer_down && (near_left || near_right) {
            let hint_w = 14.0;
            let hint_rect = if near_left {
                Rect::from_min_max(
                    egui::pos2(left_snap_x, dock_rect.top()),
                    egui::pos2(left_snap_x + hint_w, dock_rect.bottom()),
                )
            } else {
                Rect::from_min_max(
                    egui::pos2(right_snap_right - hint_w, dock_rect.top()),
                    egui::pos2(right_snap_right, dock_rect.bottom()),
                )
            };
            ctx.layer_painter(egui::LayerId::new(
                Order::Foreground,
                Id::new("hierarchy_dock_hint"),
            ))
            .rect_filled(
                hint_rect,
                6.0,
                Color32::from_rgba_unmultiplied(15, 232, 121, 110),
            );
        }

        if header_drag_stopped || (self.dragging_from_header && !pointer_down) {
            self.dragging_from_header = false;
            if near_left {
                self.dock_side = Some(HierarchyDockSide::Left);
            } else if near_right {
                self.dock_side = Some(HierarchyDockSide::Right);
            } else {
                self.dock_side = None;
            }
        }

        if resize_stopped || (self.resizing_width && !pointer_down) {
            self.resizing_width = false;
        }
    }

}
