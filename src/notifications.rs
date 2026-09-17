//! Detecta cambios de fase (verde→amarillo→rojo→"ahora") para disparar un toast breve.
//! Lógica pura y testeable, separada del dibujo de la ventana (`toast_window.rs`) y del
//! mecanismo de notificación en sí.

use crate::calendar::CalendarEvent;
use crate::meeting_clock::{self, SemaphoreColor, Thresholds};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Green,
    Yellow,
    Red,
    Started,
}

#[derive(Debug, Clone)]
pub struct ToastEvent {
    pub summaries: Vec<String>,
    pub phase: Phase,
    pub minutes_remaining: i64,
}

/// No dejar crecer el tracker para siempre en una sesión larga.
const FORGET_STARTED_AFTER: Duration = Duration::hours(1);

#[derive(Default)]
pub struct NotificationTracker {
    /// (uid, start) -> última fase notificada para esa instancia puntual.
    phases: HashMap<(String, DateTime<Utc>), Phase>,
}

impl NotificationTracker {
    /// Se llama en cada tick con la data ya fetcheada. Devuelve los toasts a disparar en
    /// este tick (normalmente 0, a veces 1, agrupando reuniones que coinciden en horario).
    pub fn tick(
        &mut self,
        events: &[CalendarEvent],
        now: DateTime<Utc>,
        thresholds: &Thresholds,
    ) -> Vec<ToastEvent> {
        let mut toasts = Vec::new();

        let next = meeting_clock::find_next_meetings(events, now, thresholds);
        if let Some(first) = next.first() {
            let phase = match first.color {
                SemaphoreColor::Green => Phase::Green,
                SemaphoreColor::Yellow => Phase::Yellow,
                SemaphoreColor::Red => Phase::Red,
            };

            let mut changed_summaries = Vec::new();
            for m in &next {
                let key = (m.event.uid.clone(), m.event.start);
                if self.phases.get(&key) != Some(&phase) {
                    self.phases.insert(key, phase);
                    changed_summaries.push(m.event.summary.clone());
                }
            }

            if !changed_summaries.is_empty() {
                toasts.push(ToastEvent {
                    summaries: changed_summaries,
                    phase,
                    minutes_remaining: first.minutes_remaining,
                });
            }
        }

        // Barrido de "ahora": lo que ya vino avisando y cuyo horario ya llegó, sin importar
        // si sigue siendo "la próxima" (una vez que arranca deja de aparecer en
        // find_next_meetings, así que hay que revisar el tracker completo).
        let just_started: Vec<(String, DateTime<Utc>)> = self
            .phases
            .iter()
            .filter(|(key, phase)| **phase != Phase::Started && key.1 <= now)
            .map(|(key, _)| key.clone())
            .collect();

        if !just_started.is_empty() {
            for key in &just_started {
                self.phases.insert(key.clone(), Phase::Started);
            }
            let summaries: Vec<String> = events
                .iter()
                .filter(|e| just_started.iter().any(|(uid, start)| &e.uid == uid && &e.start == start))
                .map(|e| e.summary.clone())
                .collect();
            if !summaries.is_empty() {
                toasts.push(ToastEvent {
                    summaries,
                    phase: Phase::Started,
                    minutes_remaining: 0,
                });
            }
        }

        self.phases.retain(|&(_, start), &mut phase| {
            !(phase == Phase::Started && now - start > FORGET_STARTED_AFTER)
        });

        toasts
    }
}

/// Texto del toast, ej. "Reunión Daily en 10 min" o "Reuniones A, B ahora".
pub fn format_toast(toast: &ToastEvent) -> String {
    let label = if toast.summaries.len() == 1 { "Reunión" } else { "Reuniones" };
    let joined = toast.summaries.join(", ");
    match toast.phase {
        Phase::Started => format!("{label} {joined} ahora"),
        _ => format!("{label} {joined} en {} min", toast.minutes_remaining),
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

    fn event_at(uid: &str, summary: &str, minutes_from_now: i64, now: DateTime<Utc>) -> CalendarEvent {
        let start = now + Duration::minutes(minutes_from_now);
        CalendarEvent {
            uid: uid.to_string(),
            summary: summary.to_string(),
            start,
            end: start + Duration::minutes(30),
            all_day: false,
            rsvp: RsvpStatus::Accepted,
            meeting_url: None,
        }
    }

    fn base_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()
    }

    #[test]
    fn avisa_una_sola_vez_por_fase_no_todos_los_ticks() {
        let mut tracker = NotificationTracker::default();
        let now = base_now();
        let events = vec![event_at("m1", "Daily", 20, now)];
        let t = thresholds();

        let first = tracker.tick(&events, now, &t);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].phase, Phase::Green);

        // Mismo tick/estado otra vez -> no debe repetir el aviso.
        let second = tracker.tick(&events, now, &t);
        assert!(second.is_empty());
    }

    #[test]
    fn avisa_en_cada_transicion_de_color() {
        let mut tracker = NotificationTracker::default();
        let t = thresholds();
        let start_now = base_now();
        let event_start = start_now + Duration::minutes(20);
        let make_events = |_now: DateTime<Utc>| {
            vec![CalendarEvent {
                uid: "m1".into(),
                summary: "Daily".into(),
                start: event_start,
                end: event_start + Duration::minutes(30),
                all_day: false,
                rsvp: RsvpStatus::Accepted,
                meeting_url: None,
            }]
        };

        let verde = tracker.tick(&make_events(start_now), start_now, &t);
        assert_eq!(verde[0].phase, Phase::Green);

        let now_amarillo = event_start - Duration::minutes(10);
        let amarillo = tracker.tick(&make_events(now_amarillo), now_amarillo, &t);
        assert_eq!(amarillo[0].phase, Phase::Yellow);

        let now_rojo = event_start - Duration::minutes(3);
        let rojo = tracker.tick(&make_events(now_rojo), now_rojo, &t);
        assert_eq!(rojo[0].phase, Phase::Red);
    }

    #[test]
    fn avisa_ahora_cuando_arranca_aunque_ya_no_sea_la_proxima() {
        let mut tracker = NotificationTracker::default();
        let t = thresholds();
        let now = base_now();
        let events = vec![event_at("m1", "Daily", 3, now)];

        let rojo = tracker.tick(&events, now, &t);
        assert_eq!(rojo[0].phase, Phase::Red);

        // El tiempo pasa y la reunion ya arranco -- ya no aparece en find_next_meetings
        // porque start <= now, pero igual debe avisar "ahora".
        let now_started = now + Duration::minutes(4);
        let ahora = tracker.tick(&events, now_started, &t);
        assert_eq!(ahora.len(), 1);
        assert_eq!(ahora[0].phase, Phase::Started);
        assert_eq!(ahora[0].summaries, vec!["Daily".to_string()]);
    }

    #[test]
    fn agrupa_reuniones_que_coinciden_en_horario_en_un_solo_toast() {
        let mut tracker = NotificationTracker::default();
        let t = thresholds();
        let now = base_now();
        let events = vec![
            event_at("a", "Reunion A", 10, now),
            event_at("b", "Reunion B", 10, now),
        ];

        let toasts = tracker.tick(&events, now, &t);
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].summaries.len(), 2);
    }

    #[test]
    fn format_toast_usa_singular_y_plural_correctamente() {
        let uno = ToastEvent {
            summaries: vec!["Daily".into()],
            phase: Phase::Yellow,
            minutes_remaining: 8,
        };
        assert_eq!(format_toast(&uno), "Reunión Daily en 8 min");

        let dos = ToastEvent {
            summaries: vec!["A".into(), "B".into()],
            phase: Phase::Started,
            minutes_remaining: 0,
        };
        assert_eq!(format_toast(&dos), "Reuniones A, B ahora");
    }
}
