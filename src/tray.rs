//! Construcción del ícono de bandeja — reloj de arena (SPEC.md §2.1, §2.6).
//!
//! Silueta del reloj dibujada a mano (marco + paredes diagonales) con la arena
//! acumulándose abajo a medida que se acerca la próxima reunión — nada de sprites/PNG,
//! todo generado en runtime como buffer RGBA.

use tray_icon::{BadIcon, Icon};

pub const ICON_SIZE: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    /// Sin URL de calendario configurada todavía (SPEC.md §2.6).
    Unconfigured,
    /// Feed inaccesible más allá del margen de tolerancia (SPEC.md §2.6).
    Error,
    /// Sin próxima reunión elegible en la ventana de 24h (SPEC.md §2.1).
    Neutral,
    Green,
    Yellow,
    Red,
}

impl TrayState {
    fn rgb(self) -> [u8; 3] {
        match self {
            TrayState::Unconfigured => [140, 140, 140],
            TrayState::Error => [100, 100, 100],
            TrayState::Neutral => [170, 170, 170],
            TrayState::Green => [46, 160, 67],
            TrayState::Yellow => [212, 168, 24],
            TrayState::Red => [201, 42, 42],
        }
    }
}

const SIZE: i32 = ICON_SIZE as i32;
const CENTER_X: f32 = SIZE as f32 / 2.0;
const TOP: i32 = 3;
const BOTTOM: i32 = SIZE - 3; // 29
const CAP: i32 = 2; // grosor de las barras horizontales arriba/abajo
const NECK: i32 = SIZE / 2; // 16
const OUTER_HALF_WIDTH: f32 = 12.0;
const NECK_HALF_WIDTH: f32 = 1.5;
const OUTLINE_THICKNESS: f32 = 1.6;

/// Medio-ancho del vidrio en la fila `y` (distancia al centro), interpolando linealmente
/// entre el borde exterior y el cuello.
fn half_width_at(y: i32) -> f32 {
    if y <= TOP + CAP {
        OUTER_HALF_WIDTH
    } else if y < NECK {
        let t = (y - (TOP + CAP)) as f32 / (NECK - (TOP + CAP)) as f32;
        OUTER_HALF_WIDTH + (NECK_HALF_WIDTH - OUTER_HALF_WIDTH) * t
    } else if y < BOTTOM - CAP {
        let t = (y - NECK) as f32 / ((BOTTOM - CAP) - NECK) as f32;
        NECK_HALF_WIDTH + (OUTER_HALF_WIDTH - NECK_HALF_WIDTH) * t
    } else {
        OUTER_HALF_WIDTH
    }
}

/// Dibuja el reloj de arena para el estado dado.
///
/// `fill_fraction` (0.0..=1.0) es cuánta arena ya "cayó" al fondo — 0.0 recién arrancó la
/// cuenta regresiva (arena toda arriba), 1.0 está por empezar la reunión (arena toda abajo).
/// Para estados sin cuenta regresiva (Neutral/Error/Unconfigured) se pasa 0.0: el reloj se
/// ve "recién dado vuelta", vacío abajo.
pub fn build_icon(state: TrayState, fill_fraction: f32) -> Result<Icon, BadIcon> {
    let [r, g, b] = state.rgb();
    let fill_fraction = fill_fraction.clamp(0.0, 1.0);
    let sand_top_y = BOTTOM as f32 - fill_fraction * (BOTTOM - CAP - NECK) as f32;

    let mut rgba = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    let mut put = |x: i32, y: i32, alpha: u8| {
        if x < 0 || y < 0 || x >= SIZE || y >= SIZE || alpha == 0 {
            return;
        }
        let idx = ((y * SIZE + x) * 4) as usize;
        // Combina por si el pixel ya tiene algo de alpha (evita bordes duros feos al superponer
        // el contorno diagonal con las barras horizontales).
        let existing = rgba[idx + 3];
        if alpha > existing {
            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = alpha;
        }
    };

    for y in TOP..BOTTOM {
        let hw = half_width_at(y);
        let is_cap_row = y < TOP + CAP || y >= BOTTOM - CAP;
        let is_sand_row = y as f32 >= sand_top_y && y >= NECK;

        for x in 0..SIZE {
            let dx = (x as f32 + 0.5 - CENTER_X).abs();
            if dx > hw {
                continue;
            }

            if is_cap_row || is_sand_row {
                // Barra superior/inferior solida, o arena acumulada: relleno completo.
                put(x, y, 255);
            } else if dx >= hw - OUTLINE_THICKNESS {
                // Pared diagonal del vidrio: solo el contorno, hueco por dentro.
                put(x, y, 255);
            }
        }
    }

    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE)
}
