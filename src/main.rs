// Punto de entrada. Arranca el ícono de bandeja y el loop de eventos de Windows.
// Ver SPEC.md §9 para el mapa de responsabilidades de cada módulo.
#![windows_subsystem = "windows"]

mod agenda_window;
mod app_state;
mod calendar;
mod config;
mod diagnostics;
mod meeting_clock;
mod notifications;
mod settings_window;
mod toast_window;
mod tray;

use agenda_window::AgendaWindow;
use app_state::{AppState, DisplayState};
use calendar::ics::IcsCalendarSource;
use calendar::{CalendarError, CalendarEvent, CalendarSource};
use chrono::TimeZone;
use config::AppConfig;
use meeting_clock::SemaphoreColor;
use native_windows_gui as nwg;
use notifications::{NotificationTracker, Phase};
use nwg::NativeUi;
use settings_window::SettingsWindow;
use std::sync::mpsc;
use std::time::{Duration as StdDuration, Instant};
use toast_window::ToastWindow;
use tray::TrayState;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

/// Cuánto queda visible el toast antes de desaparecer solo (pedido explícito: "algunos
/// segundos y desaparezca", SPEC.md — mejora de UX conversada, no en el documento original).
const TOAST_VISIBLE_DURATION: StdDuration = StdDuration::from_secs(5);

fn toast_accent_rgb(phase: Phase) -> [u8; 3] {
    match phase {
        Phase::Green => [46, 160, 67],
        Phase::Yellow => [212, 168, 24],
        Phase::Red | Phase::Started => [201, 42, 42],
    }
}

const TICK_INTERVAL: StdDuration = StdDuration::from_millis(500);
/// Si entre dos ticks pasa mucho más que `TICK_INTERVAL`, asumimos que el equipo salió de
/// suspensión y forzamos un refetch inmediato en vez de esperar al próximo ciclo (SPEC.md §2.6).
const RESUME_GAP_THRESHOLD: StdDuration = StdDuration::from_secs(120);

fn main() {
    let mut config = AppConfig::load_or_default();
    let mut thresholds = app_state::thresholds_from(
        config.green_threshold_minutes,
        config.yellow_threshold_minutes,
        config.blink_threshold_minutes,
    );
    let mut refresh_interval =
        StdDuration::from_secs(config.refresh_interval_minutes as u64 * 60);

    // El thread de fetch recibe la URL en cada pedido (en vez de fijarla una sola vez al
    // arrancar) para poder atender cambios guardados desde la ventana de Configuración sin
    // tener que reiniciar el proceso.
    let (fetch_request_tx, fetch_request_rx) = mpsc::channel::<String>();
    let (fetch_result_tx, fetch_result_rx) = mpsc::channel::<Result<Vec<CalendarEvent>, String>>();

    std::thread::spawn(move || {
        while let Ok(url) = fetch_request_rx.recv() {
            let source = IcsCalendarSource::new(url);
            let now = chrono::Utc::now();
            // Rango lo bastante ancho para cubrir tanto la ventana rolling de 24h que usa
            // el semaforo del icono como el dia calendario local completo (pasado incluido)
            // que muestra la Agenda (SPEC.md §2.1 vs §2.7 — son consultas distintas pero se
            // sirven con un solo fetch para no duplicar llamadas al feed).
            let local_today = now.with_timezone(&chrono::Local).date_naive();
            let local_midnight = local_today.and_hms_opt(0, 0, 0).expect("medianoche valida");
            let day_start = chrono::Local
                .from_local_datetime(&local_midnight)
                .earliest()
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or(now);
            let day_end = day_start + chrono::Duration::days(1);
            let from = day_start.min(now);
            let to = day_end.max(now + chrono::Duration::hours(24));

            let result = source
                .fetch_events(from, to)
                .map_err(|e: CalendarError| e.to_string());
            if fetch_result_tx.send(result).is_err() {
                break;
            }
        }
    });

    nwg::init().expect("no se pudo inicializar native-windows-gui");
    // Tamaño chico a propósito -- el default de nwg sin esto se ve grande, casi el doble
    // que la letra de la barra de tareas.
    let mut default_font = nwg::Font::default();
    if nwg::Font::builder()
        .family("Segoe UI")
        .size(15)
        .build(&mut default_font)
        .is_ok()
    {
        nwg::Font::set_global_default(Some(default_font));
    }

    let menu = Menu::new();
    let settings_item = MenuItem::new("Configuración", true, None);
    let quit_item = MenuItem::new("Salir", true, None);
    menu.append(&settings_item)
        .expect("no se pudo agregar el item 'Configuracion' al menu");
    menu.append(&quit_item)
        .expect("no se pudo agregar el item 'Salir' al menu");
    let settings_id = settings_item.id().clone();
    let quit_id = quit_item.id().clone();

    let icon = tray::build_icon(TrayState::Unconfigured, 0.0)
        .expect("no se pudo construir el icono de bandeja");

    let tray_icon = TrayIconBuilder::new()
        .with_tooltip("Calendar Tray")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .expect("no se pudo crear el icono de bandeja");

    // Solo el click derecho abre el menú nativo — el izquierdo queda para silenciar +
    // abrir la Agenda (SPEC.md §2.2).
    tray_icon.set_show_menu_on_left_click(false);

    let mut state = AppState::default();
    let mut unconfigured = config.is_unconfigured();
    let mut settings_ui: Option<_> = None;
    let mut agenda_ui: Option<_> = None;
    let mut toast_ui: Option<_> = None;
    let mut notification_tracker = NotificationTracker::default();
    let mut toast_hide_at: Option<Instant> = None;

    let mut last_tick = Instant::now();
    let mut last_fetch_request = Instant::now() - refresh_interval; // fuerza el primer fetch ya
    let mut blink_phase = false;
    let mut last_applied: Option<(TrayState, u8, String)> = None;

    if !unconfigured {
        let _ = fetch_request_tx.send(config.ics_feed_url.clone());
        last_fetch_request = Instant::now();
    }

    nwg::dispatch_thread_events_with_callback(move || {
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                // Click izquierdo: silencia el parpadeo de la reunión que esté alertando y
                // abre la Agenda del día (SPEC.md §2.1, §2.2, §2.7).
                let now = chrono::Utc::now();
                if let DisplayState::Meeting { uid, start, .. } =
                    state.compute_display(now, &thresholds)
                {
                    state.silence(uid, start);
                }

                if agenda_ui.is_none() {
                    agenda_ui = AgendaWindow::build_ui(Default::default())
                        .map_err(|e| eprintln!("no se pudo crear la ventana de Agenda: {e}"))
                        .ok();
                }
                if let Some(ui) = &agenda_ui {
                    ui.rebuild(&state.cached_events, now, &thresholds);
                    ui.position_near(position.x as i32, position.y as i32);
                    ui.window.set_visible(true);
                    ui.window.set_focus();
                }
            }
        }

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == quit_id {
                nwg::stop_thread_dispatch();
                return;
            }
            if event.id == settings_id {
                if settings_ui.is_none() {
                    settings_ui = SettingsWindow::build_ui(Default::default())
                        .map_err(|e| eprintln!("no se pudo crear la ventana de Configuracion: {e}"))
                        .ok();
                }
                if let Some(ui) = &settings_ui {
                    ui.load_from(&config);
                    ui.window.set_visible(true);
                }
            }
        }

        // Config guardada desde la ventana de Configuración (SPEC.md §2.8): se aplica acá,
        // en el thread único que también toca el ícono y el thread de fetch.
        if let Some(ui) = &settings_ui {
            if let Some(new_config) = ui.pending_config.borrow_mut().take() {
                let url_changed = new_config.ics_feed_url != config.ics_feed_url;
                config = new_config;
                thresholds = app_state::thresholds_from(
                    config.green_threshold_minutes,
                    config.yellow_threshold_minutes,
                    config.blink_threshold_minutes,
                );
                refresh_interval = StdDuration::from_secs(config.refresh_interval_minutes as u64 * 60);
                unconfigured = config.is_unconfigured();

                if !unconfigured && url_changed {
                    let _ = fetch_request_tx.send(config.ics_feed_url.clone());
                    last_fetch_request = Instant::now();
                }
            }
        }

        if let Ok(result) = fetch_result_rx.try_recv() {
            let now = chrono::Utc::now();
            match result {
                Ok(events) => state.on_fetch_success(events, now),
                Err(msg) => state.on_fetch_error(msg),
            }
        }

        let now_instant = Instant::now();
        let elapsed_since_tick = now_instant.duration_since(last_tick);

        if elapsed_since_tick < TICK_INTERVAL {
            return;
        }

        if elapsed_since_tick > RESUME_GAP_THRESHOLD && !unconfigured {
            // Probable resume de suspensión: forzar refetch inmediato (SPEC.md §2.6).
            let _ = fetch_request_tx.send(config.ics_feed_url.clone());
            last_fetch_request = now_instant;
        } else if !unconfigured && now_instant.duration_since(last_fetch_request) >= refresh_interval
        {
            let _ = fetch_request_tx.send(config.ics_feed_url.clone());
            last_fetch_request = now_instant;
        }

        last_tick = now_instant;
        blink_phase = !blink_phase;

        let now = chrono::Utc::now();

        if !unconfigured {
            for toast in notification_tracker.tick(&state.cached_events, now, &thresholds) {
                if toast_ui.is_none() {
                    toast_ui = ToastWindow::build_ui(Default::default())
                        .map_err(|e| eprintln!("no se pudo crear el toast: {e}"))
                        .ok();
                }
                if let Some(ui) = &toast_ui {
                    let text = notifications::format_toast(&toast);
                    ui.show(&text, toast_accent_rgb(toast.phase));
                    toast_hide_at = Some(now_instant + TOAST_VISIBLE_DURATION);
                }
            }
        }

        if let Some(hide_at) = toast_hide_at {
            if now_instant >= hide_at {
                if let Some(ui) = &toast_ui {
                    ui.hide();
                }
                toast_hide_at = None;
            }
        }

        let display = if unconfigured {
            DisplayState::Unconfigured
        } else {
            state.compute_display(now, &thresholds)
        };

        let (desired_state, fill_fraction, tooltip) = render(&display, blink_phase);
        // Redondeado a pasos de 5% -- evita reconstruir el icono en cada tick por jitter de
        // punto flotante, pero igual se ve la arena avanzar con el paso del tiempo.
        let fraction_bucket = (fill_fraction * 20.0).round() as u8;
        let applied_key = (desired_state, fraction_bucket, tooltip.clone());

        if last_applied.as_ref() != Some(&applied_key) {
            if let Ok(icon) = tray::build_icon(desired_state, fill_fraction) {
                let _ = tray_icon.set_icon(Some(icon));
            }
            let _ = tray_icon.set_tooltip(Some(&tooltip));
            last_applied = Some(applied_key);
        }
    });
}

/// Ventana visual de referencia para la arena del reloj (SPEC.md §2.1) -- no es un umbral
/// funcional, solo define a partir de cuántos minutos de anticipación empieza a "caer" la
/// arena en el ícono. Independiente de los umbrales de color configurables.
const HOURGLASS_PROGRESS_WINDOW_MINUTES: f32 = 20.0;

/// Traduce el estado de dominio a (ícono, fracción de arena caída, tooltip). El parpadeo
/// alterna entre el color real y el estado neutro cada tick (SPEC.md §2.1).
fn render(display: &DisplayState, blink_phase: bool) -> (TrayState, f32, String) {
    match display {
        DisplayState::Unconfigured => (
            TrayState::Unconfigured,
            0.0,
            "Calendar Tray - sin configurar (click derecho > Configuración)".to_string(),
        ),
        DisplayState::Error { message } => (
            TrayState::Error,
            0.0,
            format!("Calendar Tray - {message}"),
        ),
        DisplayState::Neutral => (
            TrayState::Neutral,
            0.0,
            "Calendar Tray - sin reuniones próximas".to_string(),
        ),
        DisplayState::Meeting {
            summary,
            minutes_remaining,
            color,
            blinking,
            ..
        } => {
            let base = match color {
                SemaphoreColor::Green => TrayState::Green,
                SemaphoreColor::Yellow => TrayState::Yellow,
                SemaphoreColor::Red => TrayState::Red,
            };
            let fraction = (1.0
                - (*minutes_remaining as f32 / HOURGLASS_PROGRESS_WINDOW_MINUTES))
                .clamp(0.0, 1.0);

            let (shown, shown_fraction) = if *blinking && blink_phase {
                (TrayState::Neutral, 0.0)
            } else {
                (base, fraction)
            };
            let tooltip = format!("{summary} en {minutes_remaining} min");
            (shown, shown_fraction, tooltip)
        }
    }
}
