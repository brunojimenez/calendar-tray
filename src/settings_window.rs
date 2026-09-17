//! Ventana de Configuración (SPEC.md §2.8). Se abre desde el menú contextual, edita el
//! feed ICS y los umbrales del semáforo, y guarda en `%APPDATA%\CalendarTray\config.toml`.
//!
//! Campos no editables todavía acá (tema, autoarranque, snooze, no-molestar) porque su
//! funcionalidad de fondo no está implementada aún — se agregan a este formulario cuando
//! esos pasos del plan lleguen, para no mostrar controles que no hacen nada.

use crate::config::AppConfig;
use native_windows_derive as nwd;
use native_windows_gui as nwg;
use nwd::NwgUi;
use std::cell::RefCell;
use std::sync::mpsc::Sender;

#[derive(Default, NwgUi)]
pub struct SettingsWindow {
    #[nwg_control(size: (380, 280), position: (300, 300), title: "Configuración - Calendar Tray", flags: "WINDOW")]
    #[nwg_events( OnWindowClose: [SettingsWindow::on_close] )]
    pub window: nwg::Window,

    #[nwg_control(text: "URL del feed ICS (secreta):", position: (12, 12), size: (350, 20))]
    url_label: nwg::Label,
    #[nwg_control(text: "", position: (12, 34), size: (278, 24))]
    pub url_input: nwg::TextInput,
    #[nwg_control(text: "Probar", position: (296, 33), size: (66, 24))]
    #[nwg_events( OnButtonClick: [SettingsWindow::on_test] )]
    test_button: nwg::Button,

    #[nwg_control(text: "Umbral verde (min):", position: (12, 70), size: (220, 20))]
    green_label: nwg::Label,
    #[nwg_control(text: "15", position: (240, 68), size: (120, 24))]
    pub green_input: nwg::TextInput,

    #[nwg_control(text: "Umbral amarillo (min):", position: (12, 100), size: (220, 20))]
    yellow_label: nwg::Label,
    #[nwg_control(text: "5", position: (240, 98), size: (120, 24))]
    pub yellow_input: nwg::TextInput,

    #[nwg_control(text: "Parpadeo (min):", position: (12, 130), size: (220, 20))]
    blink_label: nwg::Label,
    #[nwg_control(text: "1", position: (240, 128), size: (120, 24))]
    pub blink_input: nwg::TextInput,

    #[nwg_control(text: "Refresco del feed (min):", position: (12, 160), size: (220, 20))]
    refresh_label: nwg::Label,
    #[nwg_control(text: "5", position: (240, 158), size: (120, 24))]
    pub refresh_input: nwg::TextInput,

    #[nwg_control(text: "Guardar", position: (12, 220), size: (170, 32))]
    #[nwg_events( OnButtonClick: [SettingsWindow::on_save] )]
    save_button: nwg::Button,

    #[nwg_control(text: "Cancelar", position: (198, 220), size: (170, 32))]
    #[nwg_events( OnButtonClick: [SettingsWindow::on_close] )]
    cancel_button: nwg::Button,

    /// Config tal como estaba al abrir la ventana — para preservar los campos que este
    /// formulario todavía no edita (tema, autoarranque, snooze, no-molestar) al guardar.
    base_config: RefCell<AppConfig>,

    /// Config nueva lista para aplicar, consumida por el loop principal (SPEC.md §9: main
    /// hace polling de esto en vez de que la ventana llame directo a la lógica del tray).
    pub pending_config: RefCell<Option<AppConfig>>,

    /// Canal hacia el thread de "probar conexión" (SPEC.md §2.8) -- lo setea `main` una vez
    /// construida la ventana. El fetch real corre en ese thread, nunca acá: bloquear el
    /// thread de UI con una llamada de red colgaba toda la app si el feed no respondía.
    pub test_request_tx: RefCell<Option<Sender<String>>>,
}

impl SettingsWindow {
    pub fn load_from(&self, config: &AppConfig) {
        *self.base_config.borrow_mut() = config.clone();
        self.url_input.set_text(&config.ics_feed_url);
        self.green_input
            .set_text(&config.green_threshold_minutes.to_string());
        self.yellow_input
            .set_text(&config.yellow_threshold_minutes.to_string());
        self.blink_input
            .set_text(&config.blink_threshold_minutes.to_string());
        self.refresh_input
            .set_text(&config.refresh_interval_minutes.to_string());
    }

    fn on_save(&self) {
        let url = self.url_input.text().trim().to_string();
        let green = self.green_input.text().trim().parse::<u32>();
        let yellow = self.yellow_input.text().trim().parse::<u32>();
        let blink = self.blink_input.text().trim().parse::<u32>();
        let refresh = self.refresh_input.text().trim().parse::<u32>();

        let (green, yellow, blink, refresh) = match (green, yellow, blink, refresh) {
            (Ok(g), Ok(y), Ok(b), Ok(r)) if r >= 1 => (g, y, b, r),
            _ => {
                nwg::modal_error_message(
                    &self.window,
                    "Configuración inválida",
                    "Los umbrales y el intervalo de refresco deben ser números enteros \
                     (el refresco mínimo es 1 minuto).",
                );
                return;
            }
        };

        let mut new_config = self.base_config.borrow().clone();
        new_config.ics_feed_url = url;
        new_config.green_threshold_minutes = green;
        new_config.yellow_threshold_minutes = yellow;
        new_config.blink_threshold_minutes = blink;
        new_config.refresh_interval_minutes = refresh;

        if let Err(err) = new_config.save() {
            nwg::modal_error_message(
                &self.window,
                "No se pudo guardar",
                &format!("No se pudo escribir la configuración: {err}"),
            );
            return;
        }

        *self.pending_config.borrow_mut() = Some(new_config);
        self.window.set_visible(false);
    }

    fn on_close(&self) {
        self.window.set_visible(false);
    }

    /// Pide probar la URL tal como está escrita ahora mismo (sin guardar) — el fetch real lo
    /// hace el thread dedicado (`main::test_request_rx`), esto solo dispara el pedido y
    /// deja el botón en "Probando..." hasta que `main` traiga el resultado
    /// (`show_test_result`, ver abajo) y lo restaure.
    fn on_test(&self) {
        let url = self.url_input.text().trim().to_string();
        if url.is_empty() {
            nwg::modal_error_message(&self.window, "Probar conexión", "Ingresá una URL primero.");
            return;
        }

        if let Some(tx) = self.test_request_tx.borrow().as_ref() {
            if tx.send(url).is_ok() {
                self.test_button.set_text("Probando...");
                self.test_button.set_enabled(false);
            }
        }
    }

    /// Llamado desde el loop principal cuando llega el resultado del thread de prueba.
    pub fn show_test_result(&self, result: Result<usize, String>) {
        self.test_button.set_enabled(true);
        self.test_button.set_text("Probar");

        match result {
            Ok(count) => nwg::modal_info_message(
                &self.window,
                "Conexión OK",
                &format!(
                    "Se pudo leer el feed correctamente.\n{count} reunion(es) en las próximas 24h."
                ),
            ),
            Err(msg) => nwg::modal_error_message(&self.window, "No se pudo conectar", &msg),
        };
    }
}
