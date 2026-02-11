// src/main.rs
mod inspector;

use eframe::{App, Frame, NativeOptions, egui};
use inspector::InspectorWindow;

struct EditorApp {
    inspector: InspectorWindow,
}

impl App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut Frame) {
        // Dark theme
        ctx.set_visuals(egui::Visuals::dark());

        // Top menu
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Arquivo", |ui| {
                    if ui.button("Novo").clicked() {
                        ui.close();
                    }
                    if ui.button("Salvar").clicked() {
                        ui.close();
                    }
                    if ui.button("Sair").clicked() {
                        ui.close();
                    }
                });

                ui.menu_button("Editar", |ui| {
                    if ui.button("Undo").clicked() {}
                    if ui.button("Redo").clicked() {}
                });

                ui.menu_button("Ajuda", |ui| if ui.button("Sobre").clicked() {});
            });
        });

        // Janela Inspetor
        self.inspector.show(ctx);
    }
}

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Dengine Editor")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Dengine Editor",
        options,
        Box::new(|_cc| {
            Ok(Box::new(EditorApp {
                inspector: InspectorWindow::new(),
            }))
        }),
    )
}
