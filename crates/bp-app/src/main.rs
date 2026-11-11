//! BankPerfect — application de bureau.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    // Moteur retenu après un échec ou un rendu logiciel. Avant tout autre fil.
    bankperfect::graphics::prepare();
    bankperfect::crash::install();
    let initial_file = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let translucent = bankperfect::translucent_window_setting();
    let options = eframe::NativeOptions {
        renderer: bankperfect::graphics::renderer(),
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
    bankperfect::crash::step(bankperfect::crash::GRAPHICS);
    let result = eframe::run_native(
        "BankPerfect",
        options,
        Box::new(move |cc| {
            if let Some(render_state) = &cc.wgpu_render_state
                && bankperfect::graphics::set_wgpu(&render_state.adapter.get_info())
            {
                // Rendu logiciel (WARP) : glow saura peut-être se servir de
                // la carte. `main` relance.
                return Err(Box::new(bankperfect::graphics::SwitchToGlow));
            }
            if let Some(gl) = &cc.gl {
                bankperfect::graphics::set_glow(gl);
            }
            if std::env::var_os("BP_PROFILE").is_some() {
                let gpu = bankperfect::graphics::summary().unwrap_or_default();
                eprintln!("BankPerfect : carte graphique {gpu}");
            }
            bankperfect::crash::step("interface et fichier");
            bankperfect::theme::set_surface_transparent(translucent);
            bankperfect::theme::install(&cc.egui_ctx);
            bankperfect::theme::set_translucent(&cc.egui_ctx, translucent);
            Ok(Box::new(bankperfect::BankPerfectApp::new(initial_file)))
        }),
    );
    if bankperfect::graphics::switch_requested() {
        bankperfect::crash::startup_done();
        bankperfect::graphics::switch_to_glow();
        std::process::exit(0);
    }
    let glow_failed = bankperfect::graphics::on_glow()
        && result
            .as_ref()
            .is_err_and(bankperfect::graphics::is_glow_error);
    if glow_failed || (matches!(result, Err(eframe::Error::Wgpu(_))) && gl_fallback()) {
        // Moteur de secours, retenu pour les lancements suivants : sans
        // Vulkan (ou DirectX 12), wgpu en OpenGL ; sans OpenGL 3.3, glow ;
        // glow sans OpenGL utilisable, retour à wgpu.
        bankperfect::crash::startup_done();
        let reason = result
            .as_ref()
            .err()
            .map(ToString::to_string)
            .unwrap_or_default();
        bankperfect::graphics::fall_back(&reason);
        std::process::exit(0);
    }
    if let Err(error) = &result {
        bankperfect::crash::startup_failed(error);
    }
    result
}

/// Relancer avec un moteur de secours si la carte graphique n'a pas pu
/// servir : sous Linux (Vulkan seul par défaut) et sous Windows (machines
/// virtuelles, bureau à distance, vieux pilotes). Un `WGPU_BACKEND` autre
/// qu'OpenGL garde la main.
fn gl_fallback() -> bool {
    cfg!(any(target_os = "linux", target_os = "windows"))
        && (std::env::var_os("WGPU_BACKEND").is_none() || bankperfect::graphics::on_gl())
        && bankperfect::graphics::can_fall_back()
}

/// Moteur par défaut, `WGPU_BACKEND` gardant la main. Sous Linux, Vulkan
/// seul : préparer aussi OpenGL (EGL) coûte une vingtaine de millisecondes
/// au démarrage, pour rien quand Vulkan est là. Sous Windows, DirectX 12
/// seul : interroger aussi Vulkan charge son pilote, et celui d'Intel
/// plante sur certaines machines (igvk64.dll) avant toute fenêtre. Sans
/// eux, l'application se relance avec OpenGL.
fn default_backends() -> Option<eframe::wgpu::Backends> {
    use eframe::wgpu::Backends;
    if std::env::var_os("WGPU_BACKEND").is_some() {
        None
    } else if cfg!(target_os = "linux") {
        Some(Backends::VULKAN)
    } else if cfg!(target_os = "windows") {
        Some(Backends::DX12)
    } else {
        None
    }
}

fn wgpu_setup() -> eframe::egui_wgpu::WgpuSetup {
    use eframe::wgpu;
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    if let Some(backends) = default_backends() {
        setup.instance_descriptor.backends = backends;
    }
    // Carte intégrée plutôt que la carte dédiée des portables qui en ont
    // deux : elle suffit largement, et réveiller l'autre retarde le
    // démarrage et vide la batterie. `WGPU_POWER_PREF` garde la main.
    setup.power_preference =
        wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::LowPower);
    eframe::egui_wgpu::WgpuSetup::CreateNew(setup)
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
