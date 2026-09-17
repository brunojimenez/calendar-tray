//! Ventana de Agenda del día (SPEC.md §2.7). Filas dinámicas (una por evento), así que
//! está escrita a mano en vez de con la macro `#[derive(NwgUi)]` — la cantidad de filas no
//! se sabe en tiempo de compilación.
//!
//! Muestra **todas** las actividades del día (no solo la ventana de 24h del semáforo del
//! ícono), con un indicador de color por fila según el estado temporal de cada una — igual
//! al comportamiento de la versión Java anterior:
//! - Gris: ya pasó.
//! - Rojo: está ocurriendo ahora mismo.
//! - Amarillo: está por empezar (dentro del umbral "cercano").
//! - Verde: todavía falta.
//! - Azul: evento de día completo (no encaja en la escala temporal de arriba).

use crate::calendar::CalendarEvent;
use crate::meeting_clock::Thresholds;
use chrono::{DateTime, Local, Utc};
use native_windows_gui as nwg;
use nwg::{Event as E, NativeUi};
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

const ROW_HEIGHT: i32 = 28;
const WINDOW_WIDTH: i32 = 400;
// Deja lugar solo para el boton "cerrar" (X) de arriba a la derecha -- sin titulo, sin
// margen extra (el hueco que parecia "para un titulo" que no iba a existir).
const ROWS_TOP: i32 = 26;
const DOT_SIZE: i32 = 10;

#[derive(Clone, Copy)]
enum RowStatus {
    Past,
    Ongoing,
    Near,
    Future,
    AllDay,
}

impl RowStatus {
    fn color(self) -> [u8; 3] {
        match self {
            RowStatus::Past => [140, 140, 140],
            RowStatus::Ongoing => [201, 42, 42],
            RowStatus::Near => [212, 168, 24],
            RowStatus::Future => [46, 160, 67],
            RowStatus::AllDay => [90, 110, 200],
        }
    }
}

fn classify_row(event: &CalendarEvent, now: DateTime<Utc>, near_minutes: i64) -> RowStatus {
    if event.all_day {
        return RowStatus::AllDay;
    }
    if event.end <= now {
        RowStatus::Past
    } else if event.start <= now {
        RowStatus::Ongoing
    } else if (event.start - now).num_minutes() <= near_minutes {
        RowStatus::Near
    } else {
        RowStatus::Future
    }
}

/// Eventos cuyo inicio cae en el día calendario local de `now` (no la ventana rolling de
/// 24h que usa el semáforo del ícono — acá se quiere ver el día completo, pasado incluido).
fn events_for_today(events: &[CalendarEvent], now: DateTime<Utc>) -> Vec<CalendarEvent> {
    let today = now.with_timezone(&Local).date_naive();
    events
        .iter()
        .filter(|e| e.start.with_timezone(&Local).date_naive() == today)
        .cloned()
        .collect()
}

struct AgendaRow {
    dot: nwg::Label,
    label: nwg::Label,
    copy_button: nwg::Button,
    join_button: nwg::Button,
    meeting_url: Option<String>,
}

#[derive(Default)]
pub struct AgendaWindow {
    pub window: nwg::Window,
    empty_label: nwg::Label,
    close_button: nwg::Button,
    rows: RefCell<Vec<AgendaRow>>,
}

impl AgendaWindow {
    /// Reconstruye la lista con las actividades del día (SPEC.md §2.7). Se llama cada vez
    /// que se abre la ventana, así siempre refleja la última data conocida.
    pub fn rebuild(&self, all_cached_events: &[CalendarEvent], now: DateTime<Utc>, thresholds: &Thresholds) {
        self.rows.borrow_mut().clear(); // dropea los controles viejos (destruye el HWND)

        let events = events_for_today(all_cached_events, now);

        if events.is_empty() {
            self.empty_label.set_visible(true);
            self.window.set_size(WINDOW_WIDTH as u32, (ROWS_TOP + 60) as u32);
            return;
        }
        self.empty_label.set_visible(false);

        let mut rows = self.rows.borrow_mut();
        for (i, event) in events.iter().enumerate() {
            let y = ROWS_TOP + (i as i32) * ROW_HEIGHT;
            let status = classify_row(event, now, thresholds.yellow_minutes as i64);

            let mut dot = nwg::Label::default();
            nwg::Label::builder()
                .text("")
                .position((12, y + 8))
                .size((DOT_SIZE, DOT_SIZE))
                .background_color(Some(status.color()))
                .parent(&self.window)
                .build(&mut dot)
                .expect("no se pudo crear el indicador de color");

            let mut label = nwg::Label::default();
            nwg::Label::builder()
                .text(&row_text(event))
                .position((28, y + 4))
                .size((244, 20))
                .parent(&self.window)
                .build(&mut label)
                .expect("no se pudo crear la fila de agenda");

            let has_link = event.meeting_url.is_some();

            let mut copy_button = nwg::Button::default();
            nwg::Button::builder()
                .text(if has_link { "📋" } else { "" })
                .position((278, y))
                .size((52, 24))
                .parent(&self.window)
                .build(&mut copy_button)
                .expect("no se pudo crear el boton copiar");
            copy_button.set_visible(has_link);

            let mut join_button = nwg::Button::default();
            nwg::Button::builder()
                .text(if has_link { "🔗" } else { "" })
                .position((334, y))
                .size((52, 24))
                .parent(&self.window)
                .build(&mut join_button)
                .expect("no se pudo crear el boton ir");
            join_button.set_visible(has_link);

            rows.push(AgendaRow {
                dot,
                label,
                copy_button,
                join_button,
                meeting_url: event.meeting_url.clone(),
            });
        }
        drop(rows);

        let content_height = ROWS_TOP + (events.len() as i32) * ROW_HEIGHT + 10;
        self.window
            .set_size(WINDOW_WIDTH as u32, content_height.clamp(120, 700) as u32);
    }

    /// Posiciona el flyout junto al punto del click en el ícono (normalmente la bandeja
    /// está abajo a la derecha, así que se ancla arriba-izquierda del punto). Llamar
    /// **después** de `rebuild`, que es quien fija el tamaño final de la ventana.
    pub fn position_near(&self, anchor_x: i32, anchor_y: i32) {
        let (w, h) = self.window.size();
        let x = (anchor_x - w as i32 + 20).max(4);
        let y = (anchor_y - h as i32 - 8).max(4);
        self.window.set_position(x, y);
    }

    fn copy_link(&self, url: &str) {
        nwg::Clipboard::set_data_text(&self.window, url);
    }

    fn open_link(url: &str) {
        // Evita el flash de una consola de cmd.exe al abrir el navegador por defecto
        // (truco estandar en apps Win32 GUI para lanzar el protocol handler de shell).
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
    }

    fn on_close(&self) {
        self.window.set_visible(false);
    }
}

fn row_text(event: &CalendarEvent) -> String {
    if event.all_day {
        format!("(todo el dia) {}", event.summary)
    } else {
        let local: DateTime<Local> = event.start.with_timezone(&Local);
        format!("{} - {}", local.format("%H:%M"), event.summary)
    }
}

pub struct AgendaWindowUi {
    inner: Rc<AgendaWindow>,
    default_handler: RefCell<Option<nwg::EventHandler>>,
    /// Handler de bajo nivel para WM_ACTIVATE — así el flyout se cierra solo al perder
    /// el foco (click afuera), como pidió el usuario en vez de una ventana normal.
    deactivate_handler: RefCell<Option<nwg::RawEventHandler>>,
}

impl Deref for AgendaWindowUi {
    type Target = AgendaWindow;
    fn deref(&self) -> &AgendaWindow {
        &self.inner
    }
}

impl Drop for AgendaWindowUi {
    fn drop(&mut self) {
        if let Some(handler) = self.default_handler.borrow().as_ref() {
            nwg::unbind_event_handler(handler);
        }
        if let Some(handler) = self.deactivate_handler.borrow().as_ref() {
            let _ = nwg::unbind_raw_event_handler(handler);
        }
    }
}

const WM_ACTIVATE: u32 = 0x0006;
const WA_INACTIVE: usize = 0;
const DEACTIVATE_HANDLER_ID: usize = 0x1_0001; // > 0xFFFF, reservado por NWG debajo de eso

impl NativeUi<AgendaWindowUi> for AgendaWindow {
    fn build_ui(mut data: AgendaWindow) -> Result<AgendaWindowUi, nwg::NwgError> {
        // POPUP: sin barra de título ni bordes de ventana — un "cuadro" flotante junto al
        // ícono en vez de una ventana normal (pedido explícito del usuario).
        nwg::Window::builder()
            .flags(nwg::WindowFlags::POPUP)
            .size((WINDOW_WIDTH, 200))
            .position((320, 260))
            .build(&mut data.window)?;

        nwg::Label::builder()
            .text("No hay actividades hoy.")
            .position((12, ROWS_TOP))
            .size((340, 40))
            .parent(&data.window)
            .build(&mut data.empty_label)?;

        // X chica arriba a la derecha en vez de un boton "Cerrar" abajo -- con el flyout
        // pegado al borde de la pantalla, un boton abajo podia quedar fuera de la vista.
        nwg::Button::builder()
            .text("✕")
            .position((WINDOW_WIDTH - 24, 2))
            .size((22, 22))
            .parent(&data.window)
            .build(&mut data.close_button)?;

        let ui = AgendaWindowUi {
            inner: Rc::new(data),
            default_handler: Default::default(),
            deactivate_handler: Default::default(),
        };

        let evt_ui = Rc::downgrade(&ui.inner);
        let handle_events = move |evt, _evt_data, handle| {
            let Some(ui) = evt_ui.upgrade() else {
                return;
            };

            match evt {
                E::OnWindowClose => {
                    if &handle == &ui.window {
                        AgendaWindow::on_close(&ui);
                    }
                }
                E::OnButtonClick => {
                    if &handle == &ui.close_button {
                        AgendaWindow::on_close(&ui);
                        return;
                    }
                    let rows = ui.rows.borrow();
                    for row in rows.iter() {
                        if &handle == &row.copy_button {
                            if let Some(url) = &row.meeting_url {
                                AgendaWindow::copy_link(&ui, url);
                            }
                            break;
                        }
                        if &handle == &row.join_button {
                            if let Some(url) = &row.meeting_url {
                                AgendaWindow::open_link(url);
                            }
                            break;
                        }
                    }
                }
                _ => {}
            }
        };

        *ui.default_handler.borrow_mut() =
            Some(nwg::full_bind_event_handler(&ui.window.handle, handle_events));

        let deactivate_ui = Rc::downgrade(&ui.inner);
        let deactivate_handler = nwg::bind_raw_event_handler(
            &ui.window.handle,
            DEACTIVATE_HANDLER_ID,
            move |_hwnd, msg, wparam, _lparam| {
                if msg == WM_ACTIVATE && (wparam & 0xFFFF) == WA_INACTIVE {
                    if let Some(ui) = deactivate_ui.upgrade() {
                        ui.window.set_visible(false);
                    }
                }
                None
            },
        )
        .ok();
        *ui.deactivate_handler.borrow_mut() = deactivate_handler;

        Ok(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::RsvpStatus;
    use chrono::{Duration, TimeZone};

    fn now() -> DateTime<Utc> {
        // Mediodia local para no rozar el borde de dia calendario en el test.
        Utc.with_ymd_and_hms(2026, 1, 1, 15, 0, 0).unwrap()
    }

    fn event(uid: &str, start_offset_min: i64, dur_min: i64, all_day: bool) -> CalendarEvent {
        let start = now() + Duration::minutes(start_offset_min);
        CalendarEvent {
            uid: uid.into(),
            summary: uid.into(),
            start,
            end: start + Duration::minutes(dur_min),
            all_day,
            rsvp: RsvpStatus::Accepted,
            meeting_url: None,
        }
    }

    #[test]
    fn clasifica_pasada_actual_cercana_y_futura() {
        let t = now();
        assert!(matches!(classify_row(&event("a", -120, 30, false), t, 15), RowStatus::Past));
        assert!(matches!(classify_row(&event("b", -10, 30, false), t, 15), RowStatus::Ongoing));
        assert!(matches!(classify_row(&event("c", 10, 30, false), t, 15), RowStatus::Near));
        assert!(matches!(classify_row(&event("d", 120, 30, false), t, 15), RowStatus::Future));
        assert!(matches!(classify_row(&event("e", 0, 30, true), t, 15), RowStatus::AllDay));
    }

    #[test]
    fn events_for_today_incluye_pasadas_del_mismo_dia_y_excluye_otros_dias() {
        let t = now();
        let events = vec![
            event("pasada-hoy", -300, 30, false),
            event("futura-hoy", 300, 30, false),
            event("manana", 60 * 24, 30, false), // mas de 24h -> otro dia
        ];

        let today = events_for_today(&events, t);
        let uids: Vec<&str> = today.iter().map(|e| e.uid.as_str()).collect();

        assert!(uids.contains(&"pasada-hoy"));
        assert!(uids.contains(&"futura-hoy"));
        assert!(!uids.contains(&"manana"));
    }
}
