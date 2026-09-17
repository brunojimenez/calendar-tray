// Punto de entrada. Arranca el ícono de bandeja y el loop de eventos de Windows.
// Ver SPEC.md §9 para el mapa de responsabilidades de cada módulo.
#![windows_subsystem = "windows"]

mod config;
mod tray;

use config::AppConfig;
use native_windows_gui as nwg;
use tray::TrayState;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIconBuilder, TrayIconEvent};

fn main() {
    let config = AppConfig::load_or_default();

    let initial_state = if config.is_unconfigured() {
        TrayState::Unconfigured
    } else {
        TrayState::Neutral
    };

    nwg::init().expect("no se pudo inicializar native-windows-gui");

    let menu = Menu::new();
    let quit_item = MenuItem::new("Salir", true, None);
    menu.append(&quit_item)
        .expect("no se pudo agregar el item 'Salir' al menu");
    let quit_id = quit_item.id().clone();

    let tooltip = if config.is_unconfigured() {
        "Calendar Tray - sin configurar (click derecho > Configuracion)".to_string()
    } else {
        "Calendar Tray".to_string()
    };

    let icon = tray::build_icon(initial_state).expect("no se pudo construir el icono de bandeja");

    let _tray_icon = TrayIconBuilder::new()
        .with_tooltip(tooltip)
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .expect("no se pudo crear el icono de bandeja");

    nwg::dispatch_thread_events_with_callback(move || {
        if let Ok(_event) = TrayIconEvent::receiver().try_recv() {
            // TODO: click izquierdo -> silenciar parpadeo + abrir Agenda del dia (SPEC.md §2.2).
        }

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == quit_id {
                nwg::stop_thread_dispatch();
            }
        }
    });
}
