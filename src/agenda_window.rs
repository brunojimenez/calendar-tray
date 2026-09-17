//! Ventana de Agenda del día (SPEC.md §2.7). Filas dinámicas (una por evento), así que
//! está escrita a mano en vez de con la macro `#[derive(NwgUi)]` — la cantidad de filas no
//! se sabe en tiempo de compilación.

use crate::calendar::CalendarEvent;
use chrono::{DateTime, Local};
use native_windows_gui as nwg;
use nwg::{Event as E, NativeUi};
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

const ROW_HEIGHT: i32 = 30;
const WINDOW_WIDTH: i32 = 460;
const ROWS_TOP: i32 = 40;

struct AgendaRow {
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
    /// Reconstruye la lista con los eventos dados. Se llama cada vez que se abre la ventana
    /// (SPEC.md §2.7), así siempre refleja la última data conocida.
    pub fn rebuild(&self, events: &[CalendarEvent]) {
        self.rows.borrow_mut().clear(); // dropea los controles viejos (destruye el HWND)

        if events.is_empty() {
            self.empty_label.set_visible(true);
            self.window.set_size(WINDOW_WIDTH as u32, (ROWS_TOP + 80) as u32);
            return;
        }
        self.empty_label.set_visible(false);

        let mut rows = self.rows.borrow_mut();
        for (i, event) in events.iter().enumerate() {
            let y = ROWS_TOP + (i as i32) * ROW_HEIGHT;

            let mut label = nwg::Label::default();
            nwg::Label::builder()
                .text(&row_text(event))
                .position((12, y + 4))
                .size((280, 20))
                .parent(&self.window)
                .build(&mut label)
                .expect("no se pudo crear la fila de agenda");

            let mut copy_button = nwg::Button::default();
            let has_link = event.meeting_url.is_some();
            nwg::Button::builder()
                .text(if has_link { "Copiar" } else { "" })
                .position((296, y))
                .size((70, 26))
                .parent(&self.window)
                .build(&mut copy_button)
                .expect("no se pudo crear el boton copiar");
            copy_button.set_visible(has_link);

            let mut join_button = nwg::Button::default();
            nwg::Button::builder()
                .text(if has_link { "Ir" } else { "" })
                .position((372, y))
                .size((70, 26))
                .parent(&self.window)
                .build(&mut join_button)
                .expect("no se pudo crear el boton ir");
            join_button.set_visible(has_link);

            rows.push(AgendaRow {
                label,
                copy_button,
                join_button,
                meeting_url: event.meeting_url.clone(),
            });
        }
        drop(rows);

        let content_height = ROWS_TOP + (events.len() as i32) * ROW_HEIGHT + 50;
        self.window
            .set_size(WINDOW_WIDTH as u32, content_height.clamp(160, 700) as u32);
        self.reposition_close_button();
    }

    fn reposition_close_button(&self) {
        let rows_count = self.rows.borrow().len() as i32;
        let y = ROWS_TOP + rows_count * ROW_HEIGHT + 12;
        self.close_button.set_position(12, y);
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
    }
}

impl NativeUi<AgendaWindowUi> for AgendaWindow {
    fn build_ui(mut data: AgendaWindow) -> Result<AgendaWindowUi, nwg::NwgError> {
        nwg::Window::builder()
            .flags(nwg::WindowFlags::WINDOW)
            .size((WINDOW_WIDTH, 200))
            .position((320, 260))
            .title("Agenda del dia - Calendar Tray")
            .build(&mut data.window)?;

        nwg::Label::builder()
            .text("No hay reuniones en la ventana de las proximas 24h.")
            .position((12, 12))
            .size((420, 40))
            .parent(&data.window)
            .build(&mut data.empty_label)?;

        nwg::Button::builder()
            .text("Cerrar")
            .position((12, 140))
            .size((100, 30))
            .parent(&data.window)
            .build(&mut data.close_button)?;

        let ui = AgendaWindowUi {
            inner: Rc::new(data),
            default_handler: Default::default(),
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

        Ok(ui)
    }
}
