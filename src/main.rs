// Punto de entrada. Arranca el ícono de bandeja y el loop de eventos de Windows.
// Ver SPEC.md §9 para el mapa de responsabilidades de cada módulo.
#![windows_subsystem = "windows"]

mod app_state;
mod calendar;
mod config;
mod meeting_clock;
mod tray;

use app_state::{AppState, DisplayState};
use calendar::ics::IcsCalendarSource;
use calendar::{CalendarError, CalendarEvent, CalendarSource};
use config::AppConfig;
use meeting_clock::SemaphoreColor;
use native_windows_gui as nwg;
use std::sync::mpsc;
use std::time::{Duration as StdDuration, Instant};
use tray::TrayState;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

const TICK_INTERVAL: StdDuration = StdDuration::from_millis(500);
/// Si entre dos ticks pasa mucho más que `TICK_INTERVAL`, asumimos que el equipo salió de
/// suspensión y forzamos un refetch inmediato en vez de esperar al próximo ciclo (SPEC.md §2.6).
const RESUME_GAP_THRESHOLD: StdDuration = StdDuration::from_secs(120);

fn main() {
    let config = AppConfig::load_or_default();
    let thresholds = app_state::thresholds_from(
        config.green_threshold_minutes,
        config.yellow_threshold_minutes,
        config.blink_threshold_minutes,
    );
    let refresh_interval = StdDuration::from_secs(config.refresh_interval_minutes as u64 * 60);

    let (fetch_request_tx, fetch_request_rx) = mpsc::channel::<()>();
    let (fetch_result_tx, fetch_result_rx) = mpsc::channel::<Result<Vec<CalendarEvent>, String>>();

    if !config.is_unconfigured() {
        let source = IcsCalendarSource::new(config.ics_feed_url.clone());
        std::thread::spawn(move || {
            while fetch_request_rx.recv().is_ok() {
                let now = chrono::Utc::now();
                let result = source
                    .fetch_events(now, now + chrono::Duration::hours(24))
                    .map_err(|e: CalendarError| e.to_string());
                if fetch_result_tx.send(result).is_err() {
                    break;
                }
            }
        });
    }

    nwg::init().expect("no se pudo inicializar native-windows-gui");

    let menu = Menu::new();
    let quit_item = MenuItem::new("Salir", true, None);
    menu.append(&quit_item)
        .expect("no se pudo agregar el item 'Salir' al menu");
    let quit_id = quit_item.id().clone();

    let icon = tray::build_icon(TrayState::Unconfigured)
        .expect("no se pudo construir el icono de bandeja");

    let tray_icon = TrayIconBuilder::new()
        .with_tooltip("Calendar Tray")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .expect("no se pudo crear el icono de bandeja");

    let mut state = AppState::default();
    let unconfigured = config.is_unconfigured();

    let mut last_tick = Instant::now();
    let mut last_fetch_request = Instant::now() - refresh_interval; // fuerza el primer fetch ya
    let mut blink_phase = false;
    let mut last_applied: Option<(TrayState, String)> = None;

    if !unconfigured {
        let _ = fetch_request_tx.send(());
        last_fetch_request = Instant::now();
    }

    nwg::dispatch_thread_events_with_callback(move || {
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // Click izquierdo silencia el parpadeo de la reunión que esté alertando
                // ahora mismo (SPEC.md §2.1, §2.2). La ventana de Agenda (§2.7) llega en un
                // paso posterior; por ahora el click solo silencia.
                let now = chrono::Utc::now();
                if let DisplayState::Meeting { uid, start, .. } =
                    state.compute_display(now, &thresholds)
                {
                    state.silence(uid, start);
                }
            }
        }

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == quit_id {
                nwg::stop_thread_dispatch();
                return;
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
            let _ = fetch_request_tx.send(());
            last_fetch_request = now_instant;
        } else if !unconfigured && now_instant.duration_since(last_fetch_request) >= refresh_interval
        {
            let _ = fetch_request_tx.send(());
            last_fetch_request = now_instant;
        }

        last_tick = now_instant;
        blink_phase = !blink_phase;

        let display = if unconfigured {
            DisplayState::Unconfigured
        } else {
            state.compute_display(chrono::Utc::now(), &thresholds)
        };

        let (desired_state, tooltip) = render(&display, blink_phase);
        let applied_key = (desired_state, tooltip.clone());

        if last_applied.as_ref() != Some(&applied_key) {
            if let Ok(icon) = tray::build_icon(desired_state) {
                let _ = tray_icon.set_icon(Some(icon));
            }
            let _ = tray_icon.set_tooltip(Some(&tooltip));
            last_applied = Some(applied_key);
        }
    });
}

/// Traduce el estado de dominio a (ícono, tooltip). El parpadeo alterna entre el color real
/// y el estado neutro cada tick (SPEC.md §2.1).
fn render(display: &DisplayState, blink_phase: bool) -> (TrayState, String) {
    match display {
        DisplayState::Unconfigured => (
            TrayState::Unconfigured,
            "Calendar Tray - sin configurar (edita %APPDATA%\\CalendarTray\\config.toml)"
                .to_string(),
        ),
        DisplayState::Error { message } => (TrayState::Error, format!("Calendar Tray - {message}")),
        DisplayState::Neutral => (TrayState::Neutral, "Calendar Tray - sin reuniones próximas".to_string()),
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
            let shown = if *blinking && blink_phase {
                TrayState::Neutral
            } else {
                base
            };
            let tooltip = format!("{summary} en {minutes_remaining} min");
            (shown, tooltip)
        }
    }
}
