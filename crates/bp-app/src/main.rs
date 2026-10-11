//! BankPerfect — application de bureau.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    let initial_file = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let translucent = bankperfect::translucent_window_setting();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("BankPerfect")
            .with_app_id("bankperfect")
            .with_icon(std::sync::Arc::new(bankperfect::logo::window_icon()))
            // Surface transparente : le compositeur floute le bureau derrière.
            .with_transparent(translucent)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        // Une seule image en attente de présentation : latence minimale.
        wgpu_options: eframe::WgpuConfiguration {
            surface: eframe::SurfaceConfig {
                present_mode: present_mode(),
                ..eframe::SurfaceConfig::LOW_LATENCY
            },
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "BankPerfect",
        options,
        Box::new(move |cc| {
            bankperfect::theme::set_surface_transparent(translucent);
            bankperfect::theme::install(&cc.egui_ctx);
            bankperfect::theme::set_translucent(&cc.egui_ctx, translucent);
            Ok(Box::new(bankperfect::BankPerfectApp::new(initial_file)))
        }),
    )
}

/// Mode de présentation, réglable par `BP_PRESENT` (fifo, mailbox,
/// immediate, auto) pour comparer la latence. Par défaut : synchronisé
/// sur l'écran, sans déchirure.
fn present_mode() -> eframe::wgpu::PresentMode {
    use eframe::wgpu::PresentMode;
    match std::env::var("BP_PRESENT")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "fifo" => PresentMode::Fifo,
        "mailbox" => PresentMode::Mailbox,
        "immediate" => PresentMode::Immediate,
        "novsync" => PresentMode::AutoNoVsync,
        _ => PresentMode::AutoVsync,
    }
}
