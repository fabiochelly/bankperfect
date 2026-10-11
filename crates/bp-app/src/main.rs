//! BankPerfect — application de bureau.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    let initial_file = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("BankPerfect")
            .with_app_id("bankperfect")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        // Une seule image en attente de présentation : latence minimale.
        wgpu_options: eframe::WgpuConfiguration {
            surface: eframe::SurfaceConfig::LOW_LATENCY,
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "BankPerfect",
        options,
        Box::new(move |cc| {
            bankperfect::theme::install(&cc.egui_ctx);
            Ok(Box::new(bankperfect::BankPerfectApp::new(initial_file)))
        }),
    )
}
