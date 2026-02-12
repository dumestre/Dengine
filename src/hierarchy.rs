use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, Order, Pos2, Rect, Stroke, TextureHandle, Vec2,
};
use epaint::ColorImage;
use std::collections::HashMap;

pub struct HierarchyWindow {
    pub open: bool,
    selector_icon_texture: Option<TextureHandle>,
    arrow_icon_texture: Option<TextureHandle>,
    dock_side: Option<HierarchyDockSide>,
    window_pos: Option<Pos2>,
    window_width: f32,
    dragging_from_header: bool,
    resizing_width: bool,
    selected_object: &'static str,
    player_open: bool,
    armature_open: bool,
    environment_open: bool,
    object_colors: HashMap<&'static str, Color32>,
    color_picker_open: bool,
    picker_color: Color32,
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
            arrow_icon_texture: None,
            dock_side: Some(HierarchyDockSide::Right),
            window_pos: None,
            window_width: 220.0,
            dragging_from_header: false,
            resizing_width: false,
            selected_object: "Main Camera",
            player_open: true,
            armature_open: true,
            environment_open: true,
            object_colors: HashMap::new(),
            color_picker_open: false,
            picker_color: Color32::from_rgb(15, 232, 121),
        }
    }

    fn parent_of(name: &'static str) -> Option<&'static str> {
        match name {
            "Mesh" | "Weapon Socket" | "Armature" => Some("Player"),
            "Spine" | "Head" => Some("Armature"),
            "Terrain" | "Trees" | "Fog Volume" => Some("Environment"),
            _ => None,
        }
    }

    fn effective_color(&self, name: &'static str) -> Option<Color32> {
        let mut cursor = Some(name);
        while let Some(current) = cursor {
            if let Some(color) = self.object_colors.get(current) {
                return Some(*color);
            }
            cursor = Self::parent_of(current);
        }
        None
    }

    fn draw_object_row(
        &self,
        ui: &mut egui::Ui,
        indent: f32,
        object_id: &'static str,
        label: &str,
        selected: bool,
    ) -> egui::Response {
        let color_dot = self.effective_color(object_id);
        ui.horizontal(|ui| {
            ui.add_space(indent);
            let resp = ui.selectable_label(selected, label);
            if let Some(color) = color_dot {
                let center = egui::pos2(ui.max_rect().right() - 8.0, resp.rect.center().y);
                ui.painter().circle_filled(center, 4.0, color);
            }
            resp
        })
        .inner
    }

    pub fn show(&mut self, ctx: &egui::Context, left_reserved: f32, right_reserved: f32) {
        if !self.open {
            return;
        }

        if self.selector_icon_texture.is_none() {
            self.selector_icon_texture =
                load_png_as_texture(ctx, "src/assets/icons/seletorcor.png");
        }
        if self.arrow_icon_texture.is_none() {
            self.arrow_icon_texture = load_png_as_texture(ctx, "src/assets/icons/seta.png");
        }
        let arrow_icon_texture = self.arrow_icon_texture.clone();

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
        let mut selector_icon_rect: Option<Rect> = None;

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
                    selector_icon_rect = Some(icon_rect);
                    if icon_resp.hovered() {
                        ui.painter().rect_filled(
                            icon_rect.expand2(egui::vec2(2.0, 2.0)),
                            4.0,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 28),
                        );
                    }
                    if icon_resp.clicked() {
                        self.color_picker_open = !self.color_picker_open;
                        self.picker_color = self
                            .object_colors
                            .get(self.selected_object)
                            .copied()
                            .or_else(|| self.effective_color(self.selected_object))
                            .unwrap_or(Color32::from_rgb(15, 232, 121));
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

                                if self
                                    .draw_object_row(
                                        ui,
                                        0.0,
                                        "Directional Light",
                                        "Directional Light",
                                        self.selected_object == "Directional Light",
                                    )
                                    .clicked()
                                {
                                    self.selected_object = "Directional Light";
                                }

                                if self
                                    .draw_object_row(
                                        ui,
                                        0.0,
                                        "Main Camera",
                                        "Main Camera",
                                        self.selected_object == "Main Camera",
                                    )
                                    .clicked()
                                {
                                    self.selected_object = "Main Camera";
                                }

                                ui.horizontal(|ui| {
                                    let arrow_clicked = if let Some(arrow_tex) = &arrow_icon_texture {
                                        let arrow_img = egui::Image::new(arrow_tex)
                                            .fit_to_exact_size(egui::vec2(10.0, 10.0))
                                            .rotate(
                                                if self.player_open {
                                                    std::f32::consts::FRAC_PI_2
                                                } else {
                                                    0.0
                                                },
                                                Vec2::splat(0.5),
                                            );
                                        ui.add_sized(
                                            [16.0, 16.0],
                                            egui::Button::image(arrow_img).frame(false),
                                        )
                                        .clicked()
                                    } else {
                                        ui.add_sized(
                                            [16.0, 16.0],
                                            egui::Button::new(if self.player_open { "▾" } else { "▸" })
                                                .frame(false),
                                        )
                                        .clicked()
                                    };
                                    if arrow_clicked
                                    {
                                        self.player_open = !self.player_open;
                                    }
                                    if self
                                        .draw_object_row(
                                            ui,
                                            0.0,
                                            "Player",
                                            "Player",
                                            self.selected_object == "Player",
                                        )
                                        .clicked()
                                    {
                                        self.selected_object = "Player";
                                    }
                                });

                                if self.player_open {
                                    ui.horizontal(|ui| {
                                        ui.add_space(18.0);
                                        if self
                                            .draw_object_row(
                                                ui,
                                                0.0,
                                                "Mesh",
                                                "Mesh",
                                                self.selected_object == "Mesh",
                                            )
                                            .clicked()
                                        {
                                            self.selected_object = "Mesh";
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        ui.add_space(18.0);
                                        if self
                                            .draw_object_row(
                                                ui,
                                                0.0,
                                                "Weapon Socket",
                                                "Weapon Socket",
                                                self.selected_object == "Weapon Socket",
                                            )
                                            .clicked()
                                        {
                                            self.selected_object = "Weapon Socket";
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        ui.add_space(18.0);
                                        let arrow_clicked = if let Some(arrow_tex) = &arrow_icon_texture {
                                            let arrow_img = egui::Image::new(arrow_tex)
                                                .fit_to_exact_size(egui::vec2(10.0, 10.0))
                                                .rotate(
                                                    if self.armature_open {
                                                        std::f32::consts::FRAC_PI_2
                                                    } else {
                                                        0.0
                                                    },
                                                    Vec2::splat(0.5),
                                                );
                                            ui.add_sized(
                                                [16.0, 16.0],
                                                egui::Button::image(arrow_img).frame(false),
                                            )
                                            .clicked()
                                        } else {
                                            ui.add_sized(
                                                [16.0, 16.0],
                                                egui::Button::new(if self.armature_open {
                                                    "▾"
                                                } else {
                                                    "▸"
                                                })
                                                .frame(false),
                                            )
                                            .clicked()
                                        };
                                        if arrow_clicked
                                        {
                                            self.armature_open = !self.armature_open;
                                        }
                                        if self
                                            .draw_object_row(
                                                ui,
                                                0.0,
                                                "Armature",
                                                "Armature",
                                                self.selected_object == "Armature",
                                            )
                                            .clicked()
                                        {
                                            self.selected_object = "Armature";
                                        }
                                    });

                                    if self.armature_open {
                                        ui.horizontal(|ui| {
                                            ui.add_space(36.0);
                                            if self
                                                .draw_object_row(
                                                    ui,
                                                    0.0,
                                                    "Spine",
                                                    "Spine",
                                                    self.selected_object == "Spine",
                                                )
                                                .clicked()
                                            {
                                                self.selected_object = "Spine";
                                            }
                                        });
                                        ui.horizontal(|ui| {
                                            ui.add_space(36.0);
                                            if self
                                                .draw_object_row(
                                                    ui,
                                                    0.0,
                                                    "Head",
                                                    "Head",
                                                    self.selected_object == "Head",
                                                )
                                                .clicked()
                                            {
                                                self.selected_object = "Head";
                                            }
                                        });
                                    }
                                }

                                ui.horizontal(|ui| {
                                    let arrow_clicked = if let Some(arrow_tex) = &arrow_icon_texture {
                                        let arrow_img = egui::Image::new(arrow_tex)
                                            .fit_to_exact_size(egui::vec2(10.0, 10.0))
                                            .rotate(
                                                if self.environment_open {
                                                    std::f32::consts::FRAC_PI_2
                                                } else {
                                                    0.0
                                                },
                                                Vec2::splat(0.5),
                                            );
                                        ui.add_sized(
                                            [16.0, 16.0],
                                            egui::Button::image(arrow_img).frame(false),
                                        )
                                        .clicked()
                                    } else {
                                        ui.add_sized(
                                            [16.0, 16.0],
                                            egui::Button::new(if self.environment_open {
                                                "▾"
                                            } else {
                                                "▸"
                                            })
                                            .frame(false),
                                        )
                                        .clicked()
                                    };
                                    if arrow_clicked
                                    {
                                        self.environment_open = !self.environment_open;
                                    }
                                    if self
                                        .draw_object_row(
                                            ui,
                                            0.0,
                                            "Environment",
                                            "Environment",
                                            self.selected_object == "Environment",
                                        )
                                        .clicked()
                                    {
                                        self.selected_object = "Environment";
                                    }
                                });

                                if self.environment_open {
                                    ui.horizontal(|ui| {
                                        ui.add_space(18.0);
                                        if self
                                            .draw_object_row(
                                                ui,
                                                0.0,
                                                "Terrain",
                                                "Terrain",
                                                self.selected_object == "Terrain",
                                            )
                                            .clicked()
                                        {
                                            self.selected_object = "Terrain";
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        ui.add_space(18.0);
                                        if self
                                            .draw_object_row(
                                                ui,
                                                0.0,
                                                "Trees",
                                                "Trees",
                                                self.selected_object == "Trees",
                                            )
                                            .clicked()
                                        {
                                            self.selected_object = "Trees";
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        ui.add_space(18.0);
                                        if self
                                            .draw_object_row(
                                                ui,
                                                0.0,
                                                "Fog Volume",
                                                "Fog Volume",
                                                self.selected_object == "Fog Volume",
                                            )
                                            .clicked()
                                        {
                                            self.selected_object = "Fog Volume";
                                        }
                                    });
                                }
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

        if self.color_picker_open {
            let default_pos = egui::pos2(panel_rect.right() - 190.0, panel_rect.top() + 30.0);
            let picker_pos = selector_icon_rect
                .map(|r| egui::pos2((r.right() - 176.0).max(panel_rect.left()), r.bottom() + 6.0))
                .unwrap_or(default_pos);

            egui::Area::new(Id::new("hierarchy_color_picker_popup"))
                .order(Order::Foreground)
                .fixed_pos(picker_pos)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width(176.0);
                        ui.label("Selecionar cor");
                        let mut color = self.picker_color;
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            self.picker_color = color;
                            self.object_colors.insert(self.selected_object, color);
                        }
                    });
                });
        }

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
