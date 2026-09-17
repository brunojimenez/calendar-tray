//! "Cuánto falta para la próxima reunión" y umbrales de semáforo (SPEC.md §2.1).

use crate::calendar::CalendarEvent;
use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemaphoreColor {
    Green,
    Yellow,
    Red,
}

#[derive(Debug, Clone)]
pub struct NextMeeting {
    pub event: CalendarEvent,
    pub minutes_remaining: i64,
    pub color: SemaphoreColor,
    /// Último minuto antes de empezar (SPEC.md §2.1) — orthogonal al color, no un color aparte.
    pub blinking: bool,
}

pub struct Thresholds {
    pub green_minutes: u32,
    pub yellow_minutes: u32,
    pub blink_minutes: u32,
}

/// Ventana de búsqueda de "próxima reunión" (SPEC.md §2.1): rolling 24h desde `now`.
const LOOKAHEAD: Duration = Duration::hours(24);

/// Busca la próxima reunión elegible.
///
/// `events` ya debe venir filtrado por `CalendarSource` (RSVP/cancelados, SPEC.md §2.4).
/// Acá se aplican los dos filtros que son responsabilidad de `MeetingClock`:
/// - Eventos de día completo se excluyen (SPEC.md §2.1).
/// - Solo se consideran eventos que **todavía no empezaron** (`start > now`). Una vez que
///   arranca una reunión se deja de mostrar como "próxima" — el v1 no tiene un estado
///   especial de "reunión en curso" (eso quedó como mejora futura, SPEC.md §10), así que
///   simplemente se pasa a buscar la siguiente.
pub fn find_next_meeting(
    events: &[CalendarEvent],
    now: DateTime<Utc>,
    thresholds: &Thresholds,
) -> Option<NextMeeting> {
    let horizon = now + LOOKAHEAD;

    let event = events
        .iter()
        .filter(|e| !e.all_day)
        .filter(|e| e.start > now && e.start < horizon)
        .min_by_key(|e| e.start)?;

    let minutes_remaining = (event.start - now).num_minutes();
    let color = classify_color(minutes_remaining, thresholds);
    let blinking = minutes_remaining <= thresholds.blink_minutes as i64;

    Some(NextMeeting {
        event: event.clone(),
        minutes_remaining,
        color,
        blinking,
    })
}

fn classify_color(minutes_remaining: i64, thresholds: &Thresholds) -> SemaphoreColor {
    if minutes_remaining > thresholds.green_minutes as i64 {
        SemaphoreColor::Green
    } else if minutes_remaining > thresholds.yellow_minutes as i64 {
        SemaphoreColor::Yellow
    } else {
        SemaphoreColor::Red
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::RsvpStatus;
    use chrono::TimeZone;

    fn thresholds() -> Thresholds {
        Thresholds {
            green_minutes: 15,
            yellow_minutes: 5,
            blink_minutes: 1,
        }
    }

    fn event_at(uid: &str, minutes_from_now: i64, now: DateTime<Utc>, all_day: bool) -> CalendarEvent {
        let start = now + Duration::minutes(minutes_from_now);
        CalendarEvent {
            uid: uid.to_string(),
            summary: uid.to_string(),
            start,
            end: start + Duration::minutes(30),
            all_day,
            rsvp: RsvpStatus::Accepted,
            meeting_url: None,
        }
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()
    }

    #[test]
    fn elige_la_reunion_mas_cercana_en_el_futuro() {
        let now = now();
        let events = vec![
            event_at("lejos", 120, now, false),
            event_at("cerca", 10, now, false),
        ];

        let next = find_next_meeting(&events, now, &thresholds()).unwrap();
        assert_eq!(next.event.uid, "cerca");
        assert_eq!(next.minutes_remaining, 10);
    }

    #[test]
    fn ignora_reuniones_que_ya_empezaron() {
        let now = now();
        let events = vec![event_at("ya-empezo", -5, now, false)];

        assert!(find_next_meeting(&events, now, &thresholds()).is_none());
    }

    #[test]
    fn ignora_eventos_de_dia_completo() {
        let now = now();
        let events = vec![event_at("todo-el-dia", 60, now, true)];

        assert!(find_next_meeting(&events, now, &thresholds()).is_none());
    }

    #[test]
    fn ignora_reuniones_fuera_de_la_ventana_de_24h() {
        let now = now();
        let events = vec![event_at("manana-tarde", 25 * 60, now, false)];

        assert!(find_next_meeting(&events, now, &thresholds()).is_none());
    }

    #[test]
    fn clasifica_colores_segun_umbrales() {
        let now = now();
        let t = thresholds();

        let verde = find_next_meeting(&[event_at("v", 20, now, false)], now, &t).unwrap();
        assert_eq!(verde.color, SemaphoreColor::Green);
        assert!(!verde.blinking);

        let amarillo = find_next_meeting(&[event_at("a", 10, now, false)], now, &t).unwrap();
        assert_eq!(amarillo.color, SemaphoreColor::Yellow);

        let rojo = find_next_meeting(&[event_at("r", 3, now, false)], now, &t).unwrap();
        assert_eq!(rojo.color, SemaphoreColor::Red);
        assert!(!rojo.blinking);

        let parpadeo = find_next_meeting(&[event_at("p", 1, now, false)], now, &t).unwrap();
        assert_eq!(parpadeo.color, SemaphoreColor::Red);
        assert!(parpadeo.blinking);
    }
}
