//! Toast propio (no notificación nativa de Windows -- esas requieren registrar un
//! AppUserModelID en el registro para verse bien, lo que complica justo lo que el spec pide
//! evitar: una app portable sin instalador, SPEC.md §6/§7). Reusa el mismo patrón de ventana
//! flotante que la Agenda, pero se autodestruye sola por tiempo en vez de por foco.

use native_windows_derive as nwd;
use native_windows_gui as nwg;
use nwd::NwgUi;
use std::cell::RefCell;

const WINDOW_WIDTH: i32 = 300;
const WINDOW_HEIGHT: i32 = 64;
const ACCENT_WIDTH: i32 = 5;

#[derive(Default, NwgUi)]
pub struct ToastWindow {
    // ex_flags 0x80 = WS_EX_TOOLWINDOW (nunca aparece en la barra de tareas ni Alt+Tab);
    // topmost para que no quede tapado detrás de otras ventanas ya abiertas.
    #[nwg_control(size: (WINDOW_WIDTH, WINDOW_HEIGHT), position: (0, 0), flags: "POPUP", ex_flags: 0x80, topmost: true)]
    pub window: nwg::Window,

    #[nwg_control(text: "", position: (18, 16), size: (WINDOW_WIDTH - 30, 34))]
    message: nwg::Label,

    // El color de la franja cambia por toast (verde/amarillo/rojo) y nwg no permite mutar
    // background_color en runtime -- se reconstruye este control cada vez (igual que las
    // filas dinamicas de la Agenda).
    accent: RefCell<Option<nwg::Label>>,
}

impl ToastWindow {
    pub fn show(&self, text: &str, accent_rgb: [u8; 3]) {
        self.message.set_text(text);

        let mut accent = nwg::Label::default();
        nwg::Label::builder()
            .text("")
            .position((0, 0))
            .size((ACCENT_WIDTH, WINDOW_HEIGHT))
            .background_color(Some(accent_rgb))
            .parent(&self.window)
            .build(&mut accent)
            .expect("no se pudo crear la franja de color del toast");
        *self.accent.borrow_mut() = Some(accent); // dropea la franja vieja, destruye su HWND

        self.position_bottom_right();
        self.window.set_visible(true);
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    fn position_bottom_right(&self) {
        let monitor_w = nwg::Monitor::width_from_window(&self.window);
        let monitor_h = nwg::Monitor::height_from_window(&self.window);
        const TASKBAR_MARGIN: i32 = 56;
        let x = (monitor_w - WINDOW_WIDTH - 12).max(4);
        let y = (monitor_h - WINDOW_HEIGHT - TASKBAR_MARGIN).max(4);
        self.window.set_position(x, y);
    }
}
