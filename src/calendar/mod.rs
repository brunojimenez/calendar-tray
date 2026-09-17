//! Abstracción de fuente de calendario (SPEC.md §2.3: `CalendarSource`).

pub mod ics;

use chrono::{DateTime, Utc};
use std::fmt;

/// Estado de RSVP propio para un evento (SPEC.md §2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RsvpStatus {
    Accepted,
    NeedsAction,
    Declined,
}

#[derive(Debug, Clone)]
pub struct CalendarEvent {
    pub uid: String,
    pub summary: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub all_day: bool,
    pub rsvp: RsvpStatus,
    /// Link de Meet/Zoom/Teams detectado en la descripción o ubicación (SPEC.md §2.7).
    pub meeting_url: Option<String>,
}

#[derive(Debug)]
pub enum CalendarError {
    Fetch(String),
    Parse(String),
}

impl fmt::Display for CalendarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalendarError::Fetch(msg) => write!(f, "no se pudo obtener el feed: {msg}"),
            CalendarError::Parse(msg) => write!(f, "no se pudo interpretar el feed ICS: {msg}"),
        }
    }
}

impl std::error::Error for CalendarError {}

/// Contrato para obtener eventos del calendario (SPEC.md §2.3, §9).
///
/// Implementaciones deben devolver solo eventos elegibles: sin cancelar y sin RSVP
/// `Declined` (SPEC.md §2.4) — ese filtro vive en el `CalendarSource`, no en el llamador,
/// para que `MeetingClock` y la ventana de Agenda compartan el mismo criterio.
///
/// Nota: las series recurrentes (RRULE) se expanden dentro del rango pedido, con soporte de
/// excepciones puntuales (`RECURRENCE-ID`) — ver `ics::expand_recurring_event` y SPEC.md
/// §2.5. Las series de eventos de día completo recurrentes quedan fuera de esta v1.
pub trait CalendarSource {
    fn fetch_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<CalendarEvent>, CalendarError>;
}
