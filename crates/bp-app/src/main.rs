//! BankPerfect — application de bureau.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    bankperfect::crash::install();
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
            wgpu_setup: wgpu_setup(),
            ..Default::default()
        },
        ..Default::default()
    };
    let result = eframe::run_native(
        "BankPerfect",
        options,
        Box::new(move |cc| {
            bankperfect::theme::set_surface_transparent(translucent);
            bankperfect::theme::install(&cc.egui_ctx);
            bankperfect::theme::set_translucent(&cc.egui_ctx, translucent);
            Ok(Box::new(bankperfect::BankPerfectApp::new(initial_file)))
        }),
    );
    if matches!(result, Err(eframe::Error::Wgpu(_))) && gl_fallback() {
        // Pas de Vulkan (ou de DirectX 12) utilisable : on relance avec OpenGL.
        if let Some(code) = relaunch_with_gl() {
            std::process::exit(code);
        }
    }
    if let Err(error) = &result {
        bankperfect::crash::startup_failed(error);
    }
    result
}

/// Relancer avec OpenGL si la carte graphique n'a pas pu servir : sous
/// Linux (Vulkan seul par défaut) et sous Windows (machines virtuelles,
/// bureau à distance, vieux pilotes). `WGPU_BACKEND` garde la main.
fn gl_fallback() -> bool {
    cfg!(any(target_os = "linux", target_os = "windows"))
        && std::env::var_os("WGPU_BACKEND").is_none()
}

/// Sous Linux, Vulkan seul par défaut : préparer aussi OpenGL (EGL) coûte
/// une vingtaine de millisecondes au démarrage, pour rien quand Vulkan est
/// là. Sans Vulkan, l'application se relance avec OpenGL. `WGPU_BACKEND`
/// garde la main.
fn vulkan_only() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("WGPU_BACKEND").is_none()
}

fn wgpu_setup() -> eframe::egui_wgpu::WgpuSetup {
    use eframe::wgpu;
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    if vulkan_only() {
        setup.instance_descriptor.backends = wgpu::Backends::VULKAN;
    }
    // Carte intégrée plutôt que la carte dédiée des portables qui en ont
    // deux : elle suffit largement, et réveiller l'autre retarde le
    // démarrage et vide la batterie. `WGPU_POWER_PREF` garde la main.
    setup.power_preference =
        wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::LowPower);
    eframe::egui_wgpu::WgpuSetup::CreateNew(setup)
}

/// Relance l'application avec OpenGL ; renvoie son code de sortie.
fn relaunch_with_gl() -> Option<i32> {
    let exe = std::env::current_exe().ok()?;
    let status = std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .env("WGPU_BACKEND", "gl")
        .status()
        .ok()?;
    Some(status.code().unwrap_or(1))
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
