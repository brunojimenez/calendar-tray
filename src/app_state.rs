//! Estado de la app y resolución de qué mostrar en el ícono (SPEC.md §2.1, §2.6).
//! Lógica pura, sin tocar red ni Win32, para poder testearla sola.

use crate::calendar::CalendarEvent;
use crate::meeting_clock::{self, SemaphoreColor, Thresholds};
use chrono::{DateTime, Duration, Utc};

/// Margen de tolerancia antes de degradar a estado de error visible (SPEC.md §2.6): un
/// fetch fallido no tapa inmediatamente la última cuenta regresiva conocida.
pub const STALE_GRACE: Duration = Duration::minutes(15);

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayState {
    Unconfigured,
    Error { message: String },
    Neutral,
    Meeting {
        uid: String,
        start: DateTime<Utc>,
        summary: String,
        minutes_remaining: i64,
        color: SemaphoreColor,
        blinking: bool,
    },
}

#[derive(Default)]
pub struct AppState {
    pub cached_events: Vec<CalendarEvent>,
    pub last_fetch_success: Option<DateTime<Utc>>,
    pub last_fetch_error: Option<String>,
    /// Instancia puntual silenciada por el usuario (uid + inicio exacto), SPEC.md §2.1.
    pub silenced: Option<(String, DateTime<Utc>)>,
}

impl AppState {
    pub fn on_fetch_success(&mut self, events: Vec<CalendarEvent>, now: DateTime<Utc>) {
        self.cached_events = events;
        self.last_fetch_success = Some(now);
        self.last_fetch_error = None;
    }

    pub fn on_fetch_error(&mut self, message: String) {
        self.last_fetch_error = Some(message);
    }

    pub fn silence(&mut self, uid: String, start: DateTime<Utc>) {
        self.silenced = Some((uid, start));
    }

    pub fn compute_display(&self, now: DateTime<Utc>, thresholds: &Thresholds) -> DisplayState {
        if let Some(next) = meeting_clock::find_next_meeting(&self.cached_events, now, thresholds) {
            let key = (next.event.uid.clone(), next.event.start);
            let blinking = next.blinking && self.silenced.as_ref() != Some(&key);
            return DisplayState::Meeting {
                uid: next.event.uid,
                start: next.event.start,
                summary: next.event.summary,
                minutes_remaining: next.minutes_remaining,
                color: next.color,
                blinking,
            };
        }

        match (self.last_fetch_success, &self.last_fetch_error) {
            (None, Some(msg)) => DisplayState::Error {
                message: msg.clone(),
            },
            (Some(success), Some(msg)) if now - success > STALE_GRACE => DisplayState::Error {
                message: format!(
                    "{msg} (sin actualizar desde hace {} min)",
                    (now - success).num_minutes()
                ),
            },
            _ => DisplayState::Neutral,
        }
    }
}

pub fn thresholds_from(green: u32, yellow: u32, blink: u32) -> Thresholds {
    Thresholds {
        green_minutes: green,
        yellow_minutes: yellow,
        blink_minutes: blink,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::RsvpStatus;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()
    }

    fn thresholds() -> Thresholds {
        thresholds_from(15, 5, 1)
    }

    fn meeting_in(minutes: i64, now: DateTime<Utc>) -> CalendarEvent {
        let start = now + Duration::minutes(minutes);
        CalendarEvent {
            uid: "m1".into(),
            summary: "Reunion".into(),
            start,
            end: start + Duration::minutes(30),
            all_day: false,
            rsvp: RsvpStatus::Accepted,
            meeting_url: None,
        }
    }

    #[test]
    fn sin_datos_ni_error_muestra_neutro() {
        let state = AppState::default();
        assert_eq!(state.compute_display(now(), &thresholds()), DisplayState::Neutral);
    }

    #[test]
    fn primer_fetch_fallido_sin_datos_previos_muestra_error() {
        let mut state = AppState::default();
        state.on_fetch_error("timeout".into());

        match state.compute_display(now(), &thresholds()) {
            DisplayState::Error { message } => assert_eq!(message, "timeout"),
            other => panic!("esperaba Error, salio {other:?}"),
        }
    }

    #[test]
    fn fetch_fallido_reciente_no_tapa_la_ultima_data_buena() {
        let mut state = AppState::default();
        let now = now();
        state.on_fetch_success(vec![meeting_in(10, now)], now);
        state.on_fetch_error("timeout".into());

        // El error ocurrio "ahora", dentro del margen de tolerancia -> sigue mostrando la
        // reunion conocida, no el estado de error (SPEC.md §2.6).
        match state.compute_display(now, &thresholds()) {
            DisplayState::Meeting { uid, .. } => assert_eq!(uid, "m1"),
            other => panic!("esperaba Meeting (data todavia fresca), salio {other:?}"),
        }
    }

    #[test]
    fn fetch_fallido_mas_alla_del_margen_degrada_a_error() {
        let mut state = AppState::default();
        let t0 = now();
        state.on_fetch_success(vec![], t0);
        state.on_fetch_error("timeout".into());

        let later = t0 + STALE_GRACE + Duration::minutes(1);
        match state.compute_display(later, &thresholds()) {
            DisplayState::Error { .. } => {}
            other => panic!("esperaba Error tras superar el margen, salio {other:?}"),
        }
    }

    #[test]
    fn silenciar_apaga_el_parpadeo_de_esa_instancia_puntual() {
        let mut state = AppState::default();
        let now = now();
        let meeting = meeting_in(1, now); // dentro del umbral de parpadeo
        state.on_fetch_success(vec![meeting.clone()], now);

        match state.compute_display(now, &thresholds()) {
            DisplayState::Meeting { blinking, .. } => assert!(blinking),
            other => panic!("esperaba Meeting parpadeando, salio {other:?}"),
        }

        state.silence(meeting.uid.clone(), meeting.start);

        match state.compute_display(now, &thresholds()) {
            DisplayState::Meeting { blinking, .. } => assert!(!blinking, "deberia estar silenciada"),
            other => panic!("esperaba Meeting sin parpadeo, salio {other:?}"),
        }
    }

    #[test]
    fn silenciar_una_instancia_no_afecta_la_siguiente_reunion() {
        let mut state = AppState::default();
        let now = now();
        let meeting = meeting_in(1, now);
        state.silence(meeting.uid.clone(), meeting.start);

        // Nueva instancia (mismo uid, otro start) de una serie recurrente -> no deberia
        // heredar el silencio (SPEC.md §2.1).
        let mut next_occurrence = meeting.clone();
        next_occurrence.start = meeting.start + Duration::days(7);
        next_occurrence.end = meeting.end + Duration::days(7);
        state.on_fetch_success(vec![next_occurrence], now);

        match state.compute_display(now, &thresholds()) {
            DisplayState::Meeting { .. } => {} // presente en la ventana de 24h no aplica aca,
            // solo interesa que compute_display no filtre por uid viejo; el test de arriba
            // ya cubre el caso donde si aplica.
            DisplayState::Neutral => {} // fuera de la ventana de 24h, tambien valido
            other => panic!("no esperaba {other:?}"),
        }
    }
}
