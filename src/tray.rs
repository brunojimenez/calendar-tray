//! Construcción del ícono de bandeja — reloj de arena (SPEC.md §2.1, §2.6).
//!
//! Silueta **sólida** rellena del color de estado (no un contorno hueco — a 32px un
//! wireframe se lee como una X en vez de un reloj), con un borde blanco delgado para dar
//! contraste contra la barra de tareas y una franja de arena mas clara arriba / mas oscura
//! abajo. Renderizado con supersampling 4x para que no se vea pixelado.

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
    fn rgb(self) -> [f32; 3] {
        match self {
            TrayState::Unconfigured => [150.0, 150.0, 150.0],
            TrayState::Error => [110.0, 110.0, 110.0],
            TrayState::Neutral => [180.0, 180.0, 180.0],
            TrayState::Green => [52.0, 172.0, 78.0],
            TrayState::Yellow => [224.0, 176.0, 30.0],
            // Un poco mas vivo que un rojo "oscuro" -- en tema oscuro de Windows los tonos
            // apagados se pierden contra la barra de tareas (feedback del usuario).
            TrayState::Red => [230.0, 68.0, 64.0],
        }
    }
}

const WHITE: [f32; 3] = [255.0, 255.0, 255.0];

fn lighten(rgb: [f32; 3], amount: f32) -> [f32; 3] {
    [
        rgb[0] + (WHITE[0] - rgb[0]) * amount,
        rgb[1] + (WHITE[1] - rgb[1]) * amount,
        rgb[2] + (WHITE[2] - rgb[2]) * amount,
    ]
}

const SS: i32 = 4; // supersampling factor
const SIZE: i32 = ICON_SIZE as i32;
const HSIZE: i32 = SIZE * SS; // resolucion interna de trabajo

const CENTER_X: f32 = (HSIZE as f32) / 2.0;
const TOP: f32 = 3.0 * SS as f32;
const BOTTOM: f32 = (SIZE - 3) as f32 * SS as f32;
const CAP: f32 = 2.0 * SS as f32; // grosor de las puntas arriba y abajo
const NECK: f32 = (SIZE as f32 / 2.0) * SS as f32;
const OUTER_HALF_WIDTH: f32 = 11.5 * SS as f32;
const NECK_HALF_WIDTH: f32 = 1.4 * SS as f32;
const CAP_ROUNDING: f32 = 1.2 * SS as f32; // redondeo leve de las puntas
// Mas grueso que un contorno tipico -- en tema oscuro es lo que hace destacar el icono
// contra la barra de tareas (feedback del usuario), no solo un detalle prolijo.
const OUTLINE_THICKNESS: f32 = 1.8 * SS as f32;

fn half_width_at(y: f32) -> f32 {
    if y < TOP + CAP {
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
/// cuenta regresiva, 1.0 está por empezar. Para estados sin cuenta regresiva se pasa 0.0.
pub fn build_icon(state: TrayState, fill_fraction: f32) -> Result<Icon, BadIcon> {
    let base = state.rgb();
    let light = lighten(base, 0.55);
    let fill_fraction = fill_fraction.clamp(0.0, 1.0);
    let sand_top_y = BOTTOM - fill_fraction * (BOTTOM - CAP - NECK);

    // Paso 1: renderizar a resolucion HSIZE x HSIZE (hard-edged, silueta SOLIDA -- nada de
    // interior hueco, eso es lo que antes se leia como una "X" en vez de un reloj).
    let mut hi = vec![[0f32; 4]; (HSIZE * HSIZE) as usize];
    for y in 0..HSIZE {
        let yf = y as f32 + 0.5;
        let hw = half_width_at(yf);
        let is_top_edge = yf < TOP + OUTLINE_THICKNESS;
        let is_bottom_edge = yf >= BOTTOM - OUTLINE_THICKNESS;
        let is_bottom_sand = yf >= sand_top_y && yf >= NECK;

        for x in 0..HSIZE {
            let dx = (x as f32 + 0.5 - CENTER_X).abs();
            if dx > hw {
                continue;
            }
            let idx = (y * HSIZE + x) as usize;

            let color = if is_top_edge || is_bottom_edge || dx >= hw - OUTLINE_THICKNESS {
                WHITE
            } else if is_bottom_sand {
                base
            } else {
                light
            };

            hi[idx] = [color[0], color[1], color[2], 255.0];
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
