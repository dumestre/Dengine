use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, Order, Pos2, Rect, Stroke, TextureHandle, Vec2,
};
use epaint::ColorImage;
use std::collections::{HashMap, HashSet};

pub struct HierarchyWindow {
    pub open: bool,
    selector_icon_texture: Option<TextureHandle>,
    arrow_icon_texture: Option<TextureHandle>,
    view_icon_texture: Option<TextureHandle>,
    no_view_icon_texture: Option<TextureHandle>,
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
    object_visibility: HashMap<&'static str, bool>,
    deleted_objects: HashSet<&'static str>,
    top_level_order: Vec<&'static str>,
    player_order: Vec<&'static str>,
    armature_order: Vec<&'static str>,
    environment_order: Vec<&'static str>,
    dragging_object: Option<&'static str>,
    drop_target: Option<HierarchyDropTarget>,
    drag_hover_parent: Option<(&'static str, f64)>,
    color_picker_open: bool,
    picker_color: Color32,
}

#[derive(Clone, Copy)]
enum HierarchyDockSide {
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HierarchyContainer {
    Top,
    Player,
    Armature,
    Environment,
}

#[derive(Clone, Copy)]
enum HierarchyDropTarget {
    Row {
        target: &'static str,
        after: bool,
    },
    Container(HierarchyContainer),
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
            view_icon_texture: None,
            no_view_icon_texture: None,
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
            object_visibility: HashMap::new(),
            deleted_objects: HashSet::new(),
            top_level_order: vec!["Directional Light", "Main Camera", "Player", "Environment"],
            player_order: vec!["Mesh", "Weapon Socket", "Armature"],
            armature_order: vec!["Spine", "Head"],
            environment_order: vec!["Terrain", "Trees", "Fog Volume"],
            dragging_object: None,
            drop_target: None,
            drag_hover_parent: None,
            color_picker_open: false,
            picker_color: Color32::from_rgb(15, 232, 121),
        }
    }

    fn is_parent_open(&self, object_id: &'static str) -> Option<bool> {
        match object_id {
            "Player" => Some(self.player_open),
            "Armature" => Some(self.armature_open),
            "Environment" => Some(self.environment_open),
            _ => None,
        }
    }

    fn set_parent_open(&mut self, object_id: &'static str, open: bool) {
        match object_id {
            "Player" => self.player_open = open,
            "Armature" => self.armature_open = open,
            "Environment" => self.environment_open = open,
            _ => {}
        }
    }

    fn container_of(&self, object_id: &'static str) -> Option<HierarchyContainer> {
        if self.top_level_order.contains(&object_id) {
            Some(HierarchyContainer::Top)
        } else if self.player_order.contains(&object_id) {
            Some(HierarchyContainer::Player)
        } else if self.armature_order.contains(&object_id) {
            Some(HierarchyContainer::Armature)
        } else if self.environment_order.contains(&object_id) {
            Some(HierarchyContainer::Environment)
        } else {
            None
        }
    }

    fn order_mut(&mut self, container: HierarchyContainer) -> &mut Vec<&'static str> {
        match container {
            HierarchyContainer::Top => &mut self.top_level_order,
            HierarchyContainer::Player => &mut self.player_order,
            HierarchyContainer::Armature => &mut self.armature_order,
            HierarchyContainer::Environment => &mut self.environment_order,
        }
    }

    fn container_parent_object(container: HierarchyContainer) -> Option<&'static str> {
        match container {
            HierarchyContainer::Top => None,
            HierarchyContainer::Player => Some("Player"),
            HierarchyContainer::Armature => Some("Armature"),
            HierarchyContainer::Environment => Some("Environment"),
        }
    }

    fn parent_of_current(&self, name: &'static str) -> Option<&'static str> {
        match self.container_of(name) {
            Some(container) => Self::container_parent_object(container),
            None => None,
        }
    }

    fn is_descendant_of(&self, node: &'static str, maybe_ancestor: &'static str) -> bool {
        let mut cursor = self.parent_of_current(node);
        while let Some(current) = cursor {
            if current == maybe_ancestor {
                return true;
            }
            cursor = self.parent_of_current(current);
        }
        false
    }

    fn can_move_to_container(&self, dragged: &'static str, to_container: HierarchyContainer) -> bool {
        if let Some(parent_obj) = Self::container_parent_object(to_container) {
            if parent_obj == dragged {
                return false;
            }
            if self.is_descendant_of(parent_obj, dragged) {
                return false;
            }
        }
        true
    }

    fn remove_from_container(&mut self, obj: &'static str, container: HierarchyContainer) {
        let order = self.order_mut(container);
        if let Some(idx) = order.iter().position(|x| *x == obj) {
            order.remove(idx);
        }
    }

    fn move_to_target(&mut self, dragged: &'static str, target: HierarchyDropTarget) {
        let Some(from_container) = self.container_of(dragged) else { return };

        match target {
            HierarchyDropTarget::Container(to_container) => {
                if !self.can_move_to_container(dragged, to_container) {
                    return;
                }
                if from_container == to_container {
                    return;
                }
                self.remove_from_container(dragged, from_container);
                let to_order = self.order_mut(to_container);
                if !to_order.contains(&dragged) {
                    to_order.push(dragged);
                }
            }
            HierarchyDropTarget::Row { target, after } => {
                let Some(to_container) = self.container_of(target) else { return };
                if !self.can_move_to_container(dragged, to_container) {
                    return;
                }

                self.remove_from_container(dragged, from_container);
                let to_order = self.order_mut(to_container);
                let Some(target_idx) = to_order.iter().position(|x| *x == target) else { return };
                let insert_idx = if after { target_idx + 1 } else { target_idx };
                let idx = insert_idx.min(to_order.len());
                if !to_order.contains(&dragged) {
                    to_order.insert(idx, dragged);
                }
            }
        }
    }

    fn effective_color(&self, name: &'static str) -> Option<Color32> {
        let mut cursor = Some(name);
        while let Some(current) = cursor {
            if let Some(color) = self.object_colors.get(current) {
                return Some(*color);
            }
            cursor = self.parent_of_current(current);
        }
        None
    }

    fn is_deleted(&self, name: &'static str) -> bool {
        let mut cursor = Some(name);
        while let Some(current) = cursor {
            if self.deleted_objects.contains(current) {
                return true;
            }
            cursor = self.parent_of_current(current);
        }
        false
    }

    fn children_of(name: &'static str) -> &'static [&'static str] {
        match name {
            "Player" => &["Mesh", "Weapon Socket", "Armature"],
            "Armature" => &["Spine", "Head"],
            "Environment" => &["Terrain", "Trees", "Fog Volume"],
            _ => &[],
        }
    }

    fn delete_object_recursive(&mut self, name: &'static str) {
        self.deleted_objects.insert(name);
        self.object_colors.remove(name);
        self.object_visibility.remove(name);
        for &child in Self::children_of(name) {
            self.delete_object_recursive(child);
        }
        if self.selected_object == name {
            self.selected_object = "Main Camera";
        }
    }

    fn draw_object_row(
        &mut self,
        ui: &mut egui::Ui,
        indent: f32,
        object_id: &'static str,
        label: &str,
        selected: bool,
    ) -> egui::Response {
        if self.is_deleted(object_id) {
            return ui.allocate_response(egui::vec2(0.0, 0.0), egui::Sense::hover());
        }

        let color_dot = self.effective_color(object_id);
        let is_visible = *self.object_visibility.get(object_id).unwrap_or(&true);
        let (row_rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 18.0), egui::Sense::hover());

        let controls_w = 36.0;
        let left_rect = Rect::from_min_max(
            row_rect.min,
            egui::pos2((row_rect.max.x - controls_w).max(row_rect.min.x), row_rect.max.y),
        );
        let mut row_resp = ui.allocate_response(egui::vec2(0.0, 0.0), egui::Sense::hover());
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(left_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.add_space(indent);
                row_resp = ui.selectable_label(selected, label);
            },
        );

        let vis_rect = Rect::from_center_size(
            egui::pos2(row_rect.max.x - 20.0, row_rect.center().y),
            egui::vec2(16.0, 16.0),
        );
        let dot_center = egui::pos2(row_rect.max.x - 4.0, row_rect.center().y);

        let vis_resp = ui.interact(
            vis_rect,
            ui.id().with(("vis_toggle", object_id)),
            egui::Sense::click(),
        );
        if vis_resp.hovered() {
            ui.painter().rect_filled(
                vis_rect.expand2(egui::vec2(1.0, 1.0)),
                3.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 20),
            );
        }

        let vis_tex = if is_visible {
            self.view_icon_texture.as_ref()
        } else {
            self.no_view_icon_texture.as_ref()
        };
        if let Some(vis_tex) = vis_tex {
            ui.painter().image(
                vis_tex.id(),
                vis_rect.shrink(0.0),
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        if vis_resp.clicked() {
            self.object_visibility.insert(object_id, !is_visible);
        }

        if let Some(color) = color_dot {
            ui.painter().circle_filled(dot_center, 4.0, color);
        }

        row_resp
    }

    fn draw_object_row_with_context(
        &mut self,
        ui: &mut egui::Ui,
        indent: f32,
        object_id: &'static str,
        label: &str,
    ) {
        let resp = self.draw_object_row(ui, indent, object_id, label, self.selected_object == object_id);
        self.apply_row_interactions(ui, &resp, object_id, label);
    }

    fn apply_row_interactions(
        &mut self,
        ui: &mut egui::Ui,
        resp: &egui::Response,
        object_id: &'static str,
        label: &str,
    ) {
        if !self.is_deleted(object_id) {
            let mut copy_clicked = false;
            let mut delete_clicked = false;
            resp.context_menu(|ui| {
                if ui.button("Copiar").clicked() {
                    copy_clicked = true;
                    ui.close();
                }
                if ui.button("Deletar").clicked() {
                    delete_clicked = true;
                    ui.close();
                }
            });
            if copy_clicked {
                ui.ctx().copy_text(label.to_owned());
            }
            if delete_clicked {
                self.delete_object_recursive(object_id);
            }
        }

        let drag_resp = ui.interact(
            resp.rect,
            ui.id().with(("hierarchy_drag_row", object_id)),
            egui::Sense::click_and_drag(),
        );
        if resp.clicked() || drag_resp.clicked() {
            self.selected_object = object_id;
        }
        if drag_resp.drag_started() {
            self.dragging_object = Some(object_id);
            self.drop_target = None;
        }
        if let Some(dragging) = self.dragging_object {
            let hover_pos = ui.ctx().input(|i| i.pointer.hover_pos());
            let hovering_row = hover_pos.map(|p| resp.rect.contains(p)).unwrap_or(false);
            if dragging != object_id && hovering_row {
                let now = ui.ctx().input(|i| i.time);
                let hover_y = ui
                    .ctx()
                    .input(|i| i.pointer.hover_pos().map(|p| p.y))
                    .unwrap_or(resp.rect.center().y);
                let top_band = resp.rect.top() + resp.rect.height() * 0.28;
                let bottom_band = resp.rect.bottom() - resp.rect.height() * 0.28;
                let as_container = match object_id {
                    "Player" => Some(HierarchyContainer::Player),
                    "Armature" => Some(HierarchyContainer::Armature),
                    "Environment" => Some(HierarchyContainer::Environment),
                    _ => None,
                };

                if let Some(container) = as_container {
                    if hover_y > top_band && hover_y < bottom_band {
                        self.drop_target = Some(HierarchyDropTarget::Container(container));
                        ui.painter().rect_stroke(
                            resp.rect.shrink(1.0),
                            3.0,
                            Stroke::new(1.5, Color32::from_rgb(15, 232, 121)),
                            egui::StrokeKind::Outside,
                        );
                        if self.is_parent_open(object_id) == Some(false) {
                            match self.drag_hover_parent {
                                Some((id, start)) if id == object_id => {
                                    if now - start >= 0.45 {
                                        self.set_parent_open(object_id, true);
                                        self.drag_hover_parent = None;
                                    }
                                }
                                _ => {
                                    self.drag_hover_parent = Some((object_id, now));
                                }
                            }
                        } else {
                            self.drag_hover_parent = None;
                        }
                    } else {
                        let after = hover_y > resp.rect.center().y;
                        self.drop_target = Some(HierarchyDropTarget::Row {
                            target: object_id,
                            after,
                        });
                        let y = if after { resp.rect.bottom() } else { resp.rect.top() };
                        ui.painter().line_segment(
                            [egui::pos2(resp.rect.left(), y), egui::pos2(resp.rect.right(), y)],
                            Stroke::new(2.0, Color32::from_rgb(15, 232, 121)),
                        );
                        self.drag_hover_parent = None;
                    }
                } else {
                    let after = hover_y > resp.rect.center().y;
                    self.drop_target = Some(HierarchyDropTarget::Row {
                        target: object_id,
                        after,
                    });
                    let y = if after { resp.rect.bottom() } else { resp.rect.top() };
                    ui.painter().line_segment(
                        [egui::pos2(resp.rect.left(), y), egui::pos2(resp.rect.right(), y)],
                        Stroke::new(2.0, Color32::from_rgb(15, 232, 121)),
                    );
                    self.drag_hover_parent = None;
                }
            }
        }
    }

    fn draw_parent_row_with_context(
        &mut self,
        ui: &mut egui::Ui,
        indent: f32,
        object_id: &'static str,
        label: &str,
        is_open: &mut bool,
    ) {
        let resp = self.draw_object_row(
            ui,
            indent + 16.0,
            object_id,
            label,
            self.selected_object == object_id,
        );
        self.apply_row_interactions(ui, &resp, object_id, label);

        let arrow_rect = Rect::from_center_size(
            egui::pos2(resp.rect.left() - 8.0, resp.rect.center().y),
            egui::vec2(14.0, 14.0),
        );
        let arrow_resp = ui.interact(
            arrow_rect,
            ui.id().with(("hierarchy_arrow", object_id)),
            egui::Sense::click(),
        );
        if arrow_resp.clicked() {
            *is_open = !*is_open;
        }

        if let Some(arrow_tex) = &self.arrow_icon_texture {
            let angle = if *is_open {
                std::f32::consts::FRAC_PI_2
            } else {
                0.0
            };
            let _ = ui.put(
                arrow_rect,
                egui::Image::new(arrow_tex)
                    .fit_to_exact_size(egui::vec2(10.0, 10.0))
                    .rotate(angle, Vec2::splat(0.5)),
            );
        } else {
            ui.painter().text(
                arrow_rect.center(),
                Align2::CENTER_CENTER,
                if *is_open { "▾" } else { "▸" },
                FontId::new(11.0, FontFamily::Proportional),
                Color32::from_gray(140),
            );
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
        if self.arrow_icon_texture.is_none() {
            self.arrow_icon_texture = load_png_as_texture(ctx, "src/assets/icons/seta.png");
        }
        if self.view_icon_texture.is_none() {
            self.view_icon_texture = load_png_as_texture(ctx, "src/assets/icons/view.png");
        }
        if self.no_view_icon_texture.is_none() {
            self.no_view_icon_texture = load_png_as_texture(ctx, "src/assets/icons/noview.png");
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

                                let top_order = self.top_level_order.clone();
                                for object in top_order {
                                    match object {
                                        "Directional Light" => {
                                            self.draw_object_row_with_context(
                                                ui,
                                                0.0,
                                                "Directional Light",
                                                "Directional Light",
                                            );
                                        }
                                        "Main Camera" => {
                                            self.draw_object_row_with_context(
                                                ui,
                                                0.0,
                                                "Main Camera",
                                                "Main Camera",
                                            );
                                        }
                                        "Player" => {
                                            let mut player_open = self.player_open;
                                            self.draw_parent_row_with_context(
                                                ui,
                                                0.0,
                                                "Player",
                                                "Player",
                                                &mut player_open,
                                            );
                                            self.player_open = player_open;

                                            if self.player_open {
                                                let player_children = self.player_order.clone();
                                                for child in player_children {
                                                    match child {
                                                        "Mesh" => {
                                                            self.draw_object_row_with_context(
                                                                ui, 18.0, "Mesh", "Mesh",
                                                            );
                                                        }
                                                        "Weapon Socket" => {
                                                            self.draw_object_row_with_context(
                                                                ui,
                                                                18.0,
                                                                "Weapon Socket",
                                                                "Weapon Socket",
                                                            );
                                                        }
                                                        "Armature" => {
                                                            let mut armature_open = self.armature_open;
                                                            self.draw_parent_row_with_context(
                                                                ui,
                                                                18.0,
                                                                "Armature",
                                                                "Armature",
                                                                &mut armature_open,
                                                            );
                                                            self.armature_open = armature_open;
                                                            if self.armature_open {
                                                                let arm_children =
                                                                    self.armature_order.clone();
                                                                for arm_child in arm_children {
                                                                    match arm_child {
                                                                        "Spine" => self
                                                                            .draw_object_row_with_context(
                                                                                ui,
                                                                                36.0,
                                                                                "Spine",
                                                                                "Spine",
                                                                            ),
                                                                        "Head" => self
                                                                            .draw_object_row_with_context(
                                                                                ui,
                                                                                36.0,
                                                                                "Head",
                                                                                "Head",
                                                                            ),
                                                                        _ => {}
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                        "Environment" => {
                                            let mut environment_open = self.environment_open;
                                            self.draw_parent_row_with_context(
                                                ui,
                                                0.0,
                                                "Environment",
                                                "Environment",
                                                &mut environment_open,
                                            );
                                            self.environment_open = environment_open;
                                            if self.environment_open {
                                                let env_children = self.environment_order.clone();
                                                for env_child in env_children {
                                                    match env_child {
                                                        "Terrain" => self.draw_object_row_with_context(
                                                            ui, 18.0, "Terrain", "Terrain",
                                                        ),
                                                        "Trees" => self.draw_object_row_with_context(
                                                            ui, 18.0, "Trees", "Trees",
                                                        ),
                                                        "Fog Volume" => self
                                                            .draw_object_row_with_context(
                                                                ui,
                                                                18.0,
                                                                "Fog Volume",
                                                                "Fog Volume",
                                                            ),
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }

                                let empty_h = ui.available_height().max(120.0);
                                let (empty_rect, empty_resp) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), empty_h),
                                    egui::Sense::click(),
                                );
                                ui.painter().rect_filled(
                                    empty_rect,
                                    0.0,
                                    Color32::from_rgba_unmultiplied(0, 0, 0, 0),
                                );
                                if self.dragging_object.is_some() && empty_resp.hovered() {
                                    self.drop_target = Some(HierarchyDropTarget::Container(
                                        HierarchyContainer::Top,
                                    ));
                                    ui.painter().rect_stroke(
                                        empty_rect.shrink(2.0),
                                        3.0,
                                        Stroke::new(1.5, Color32::from_rgb(15, 232, 121)),
                                        egui::StrokeKind::Outside,
                                    );
                                }
                                empty_resp.context_menu(|ui| {
                                    if ui.button("Criar objeto vazio").clicked() {
                                        ui.close();
                                    }
                                    ui.menu_button("Luzes", |ui| {
                                        if ui.button("Directional Light").clicked() {
                                            self.deleted_objects.remove("Directional Light");
                                            ui.close();
                                        }
                                        if ui.button("Point Light").clicked() {
                                            ui.close();
                                        }
                                        if ui.button("Spot Light").clicked() {
                                            ui.close();
                                        }
                                    });
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
                            self.color_picker_open = false;
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

        if self.dragging_object.is_some() && !pointer_down {
            if let (Some(dragged), Some(target)) = (self.dragging_object, self.drop_target) {
                self.move_to_target(dragged, target);
            }
            self.dragging_object = None;
            self.drop_target = None;
            self.drag_hover_parent = None;
        }

        if let Some(dragging) = self.dragging_object {
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let preview_rect =
                    Rect::from_min_size(pos + egui::vec2(12.0, 12.0), egui::vec2(124.0, 22.0));
                let painter =
                    ctx.layer_painter(egui::LayerId::new(Order::Tooltip, Id::new("hier_drag_preview")));
                painter.rect_filled(
                    preview_rect,
                    4.0,
                    Color32::from_rgba_unmultiplied(28, 28, 28, 220),
                );
                painter.rect_stroke(
                    preview_rect,
                    4.0,
                    Stroke::new(1.0, Color32::from_gray(90)),
                    egui::StrokeKind::Outside,
                );
                painter.text(
                    preview_rect.left_center() + egui::vec2(8.0, 0.0),
                    Align2::LEFT_CENTER,
                    dragging,
                    FontId::new(12.0, FontFamily::Proportional),
                    Color32::from_gray(220),
                );
            }
        }
    }

}
