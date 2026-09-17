//! Construcción del ícono de bandeja (SPEC.md §2.1, §2.6).
//!
//! Esto es un placeholder geométrico (círculo de color) hasta que se implemente el
//! `HourglassRenderer` real (reloj de arena vaciándose). Los estados y colores ya siguen
//! la spec para poder cablear el resto de la app contra esta misma interfaz.

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
            TrayState::Unconfigured => [120, 120, 120],
            TrayState::Error => [90, 90, 90],
            TrayState::Neutral => [160, 160, 160],
            TrayState::Green => [46, 160, 67],
            TrayState::Yellow => [212, 168, 24],
            TrayState::Red => [201, 42, 42],
        }
    }
}

/// Dibuja un círculo relleno del color del estado sobre fondo transparente.
pub fn build_icon(state: TrayState) -> Result<Icon, BadIcon> {
    let [r, g, b] = state.rgb();
    let size = ICON_SIZE as i32;
    let center = size as f32 / 2.0;
    let radius = center - 2.0;

    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let inside = (dx * dx + dy * dy).sqrt() <= radius;
            if inside {
                rgba.extend_from_slice(&[r, g, b, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE)
}
