//! Construcción del ícono de bandeja — reloj de arena (SPEC.md §2.1, §2.6).
//!
//! Silueta dibujada a mano (vidrio + marco en tono neutro, arena del color de estado
//! acumulándose abajo) y renderizada con supersampling 4x para que los bordes diagonales y
//! las puntas redondeadas no se vean pixeladas/toscas a 32px.

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
    fn sand_rgb(self) -> [f32; 3] {
        match self {
            TrayState::Unconfigured => [150.0, 150.0, 150.0],
            TrayState::Error => [110.0, 110.0, 110.0],
            TrayState::Neutral => [180.0, 180.0, 180.0],
            TrayState::Green => [52.0, 172.0, 78.0],
            TrayState::Yellow => [224.0, 176.0, 30.0],
            TrayState::Red => [214.0, 51.0, 51.0],
        }
    }
}

/// Marco/vidrio en un tono neutro claro — separado del color de estado (que solo tiñe la
/// arena) para que se lea como "reloj de vidrio con arena de color" en vez de una silueta
/// monocromática plana.
const FRAME_RGB: [f32; 3] = [225.0, 225.0, 230.0];

const SS: i32 = 4; // supersampling factor
const SIZE: i32 = ICON_SIZE as i32;
const HSIZE: i32 = SIZE * SS; // resolucion interna de trabajo

const CENTER_X: f32 = (HSIZE as f32) / 2.0;
const TOP: f32 = 3.0 * SS as f32;
const BOTTOM: f32 = (SIZE - 3) as f32 * SS as f32;
const CAP: f32 = 2.2 * SS as f32; // grosor de las barras/puntas arriba y abajo
const NECK: f32 = (SIZE as f32 / 2.0) * SS as f32;
const OUTER_HALF_WIDTH: f32 = 11.5 * SS as f32;
const NECK_HALF_WIDTH: f32 = 1.3 * SS as f32;
const FRAME_THICKNESS: f32 = 1.7 * SS as f32;
/// Redondeo de las puntas (arriba/abajo), para que no sean barras cuadradas "toscas".
const CAP_ROUNDING: f32 = 1.6 * SS as f32;

fn half_width_at(y: f32) -> f32 {
    if y < TOP + CAP {
        // Punta superior: se angosta levemente hacia el borde (redondeada) en vez de una
        // barra recta de esquinas duras.
        let t = ((y - TOP) / CAP).clamp(0.0, 1.0);
        OUTER_HALF_WIDTH - CAP_ROUNDING * (1.0 - t)
    } else if y < NECK {
        let t = (y - (TOP + CAP)) / (NECK - (TOP + CAP));
        OUTER_HALF_WIDTH + (NECK_HALF_WIDTH - OUTER_HALF_WIDTH) * t
    } else if y < BOTTOM - CAP {
        let t = (y - NECK) / ((BOTTOM - CAP) - NECK);
        NECK_HALF_WIDTH + (OUTER_HALF_WIDTH - NECK_HALF_WIDTH) * t
    } else {
        let t = ((y - (BOTTOM - CAP)) / CAP).clamp(0.0, 1.0);
        OUTER_HALF_WIDTH - CAP_ROUNDING * t
    }
}

/// Dibuja el reloj de arena para el estado dado.
///
/// `fill_fraction` (0.0..=1.0) es cuánta arena ya "cayó" al fondo — 0.0 recién arrancó la
/// cuenta regresiva (arena toda arriba), 1.0 está por empezar la reunión (arena toda abajo).
/// Para estados sin cuenta regresiva (Neutral/Error/Unconfigured) se pasa 0.0.
pub fn build_icon(state: TrayState, fill_fraction: f32) -> Result<Icon, BadIcon> {
    let sand = state.sand_rgb();
    let fill_fraction = fill_fraction.clamp(0.0, 1.0);
    let sand_top_y = BOTTOM - fill_fraction * (BOTTOM - CAP - NECK);

    // Paso 1: renderizar a resolucion HSIZE x HSIZE (hard-edged, sin AA).
    let mut hi = vec![[0f32; 4]; (HSIZE * HSIZE) as usize];
    for y in 0..HSIZE {
        let yf = y as f32 + 0.5;
        let hw = half_width_at(yf);
        let is_cap_row = yf < TOP + CAP || yf >= BOTTOM - CAP;
        let is_sand_row = yf >= sand_top_y && yf >= NECK;

        for x in 0..HSIZE {
            let dx = (x as f32 + 0.5 - CENTER_X).abs();
            if dx > hw {
                continue;
            }
            let idx = (y * HSIZE + x) as usize;
            if is_cap_row || is_sand_row {
                let [r, g, b] = sand;
                hi[idx] = [r, g, b, 255.0];
            } else if dx >= hw - FRAME_THICKNESS {
                let [r, g, b] = FRAME_RGB;
                hi[idx] = [r, g, b, 235.0];
            }
        }
    }

    // Paso 2: downsample SSxSS -> antialiasing gratis promediando bloques.
    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    let norm = (SS * SS) as f32;
    for by in 0..SIZE {
        for bx in 0..SIZE {
            let mut acc = [0f32; 4];
            for oy in 0..SS {
                for ox in 0..SS {
                    let idx = ((by * SS + oy) * HSIZE + (bx * SS + ox)) as usize;
                    let px = hi[idx];
                    acc[0] += px[0] * px[3];
                    acc[1] += px[1] * px[3];
                    acc[2] += px[2] * px[3];
                    acc[3] += px[3];
                }
            }
            let alpha = acc[3] / norm;
            let (r, g, b) = if acc[3] > 0.0 {
                (acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3])
            } else {
                (0.0, 0.0, 0.0)
            };
            rgba.push(r.round() as u8);
            rgba.push(g.round() as u8);
            rgba.push(b.round() as u8);
            rgba.push(alpha.round().clamp(0.0, 255.0) as u8);
        }
    }

    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE)
}
