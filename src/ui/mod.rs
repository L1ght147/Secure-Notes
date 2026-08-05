//! Native application interface.

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Secure Notes",
        options,
        Box::new(|_cc| Ok(Box::<SecureNotesApp>::default())),
    )
}

#[derive(Default)]
struct SecureNotesApp;

impl eframe::App for SecureNotesApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        eframe::egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            ui.heading("Secure Notes");
        });
    }
}
