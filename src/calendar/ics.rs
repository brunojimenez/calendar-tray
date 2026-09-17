//! `IcsCalendarSource`: obtiene eventos desde la URL secreta de iCal de Google Calendar
//! (SPEC.md §2.3).

use super::{CalendarError, CalendarEvent, CalendarSource, RsvpStatus};
use chrono::{DateTime, Utc};
use icalendar::{
    Calendar, CalendarComponent, Component, DatePerhapsTime, EventLike, EventStatus, PartStat,
};
use std::time::Duration;

pub struct IcsCalendarSource {
    feed_url: String,
    /// Email propio, para matchear el ATTENDEE correspondiente (SPEC.md §2.4).
    /// Se extrae de la propia URL del feed cuando es posible.
    user_email: Option<String>,
}

impl IcsCalendarSource {
    pub fn new(feed_url: String) -> Self {
        let user_email = extract_email_from_ics_url(&feed_url);
        IcsCalendarSource {
            feed_url,
            user_email,
        }
    }
}

/// Las URLs privadas de iCal de Google tienen la forma
/// `.../ical/<email percent-encoded>/private-xxxx/basic.ics` — de ahí se puede sacar el
/// email propio sin pedirlo aparte en Configuración (SPEC.md §2.4).
fn extract_email_from_ics_url(url: &str) -> Option<String> {
    let marker = "/ical/";
    let start = url.find(marker)? + marker.len();
    let rest = &url[start..];
    let end = rest.find('/')?;
    percent_decode(&rest[..end])
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

impl CalendarSource for IcsCalendarSource {
    fn fetch_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<CalendarEvent>, CalendarError> {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build();
        let agent: ureq::Agent = config.into();

        let body = agent
            .get(&self.feed_url)
            .call()
            .map_err(|e| CalendarError::Fetch(e.to_string()))?
            .body_mut()
            .read_to_string()
            .map_err(|e| CalendarError::Fetch(e.to_string()))?;

        parse_and_filter_events(&body, self.user_email.as_deref(), from, to)
    }
}

/// Parsea el texto ICS y aplica los filtros de SPEC.md §2.4 (RSVP/cancelados). Separado de
/// `fetch_events` para poder testear la lógica de parseo/filtrado sin red (ver tests abajo).
fn parse_and_filter_events(
    ics_text: &str,
    user_email: Option<&str>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<CalendarEvent>, CalendarError> {
    let parsed: Calendar = ics_text
        .parse()
        .map_err(|e| CalendarError::Parse(format!("{e:?}")))?;

    let mut events = Vec::new();

    for component in &parsed.components {
        let CalendarComponent::Event(event) = component else {
            continue;
        };

        // Cancelados se ocultan igual que DECLINED (SPEC.md §2.4).
        if event.get_status() == Some(EventStatus::Cancelled) {
            continue;
        }

        // Recurrencia (RRULE): todavia no se expande (ver doc del modulo y SPEC.md §2.5) —
        // se omite en vez de mostrar solo la fecha de la primera ocurrencia, que seria
        // enganoso.
        if event.property_value("RRULE").is_some() {
            continue;
        }

        let Some((start_utc, start_all_day)) =
            event.get_start().and_then(|d| to_utc_and_all_day(&d))
        else {
            continue;
        };

        let end_utc = event
            .get_end()
            .and_then(|d| to_utc_and_all_day(&d))
            .map(|(dt, _)| dt)
            .unwrap_or(start_utc);

        if start_utc < from || start_utc >= to {
            continue;
        }

        let rsvp = resolve_rsvp(event, user_email);
        if rsvp == RsvpStatus::Declined {
            continue;
        }

        let mut link_text = String::new();
        if let Some(desc) = event.get_description() {
            link_text.push_str(desc);
            link_text.push(' ');
        }
        if let Some(loc) = event.get_location() {
            link_text.push_str(loc);
        }
        let meeting_url = extract_meeting_url(&link_text);

        events.push(CalendarEvent {
            uid: event.get_uid().unwrap_or_default().to_string(),
            summary: event.get_summary().unwrap_or("(sin titulo)").to_string(),
            start: start_utc,
            end: end_utc,
            all_day: start_all_day,
            rsvp,
            meeting_url,
        });
    }

    events.sort_by_key(|e| e.start);
    Ok(events)
}

/// Reconoce links de Meet, Zoom y Teams en texto libre (SPEC.md §2.7: no limitarse a
/// `meet.google.com`, entorno corporativo mixto).
const MEETING_LINK_MARKERS: [&str; 3] = ["meet.google.com/", "zoom.us/", "teams.microsoft.com/"];

fn extract_meeting_url(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '<' || c == '>' || c == '"' || c == '\'')
        .map(|word| {
            word.trim_matches(|c: char| {
                !(c.is_ascii_alphanumeric() || matches!(c, ':' | '/' | '.' | '-' | '_' | '?' | '=' | '&'))
            })
        })
        .find(|word| {
            (word.starts_with("http://") || word.starts_with("https://"))
                && MEETING_LINK_MARKERS.iter().any(|marker| word.contains(marker))
        })
        .map(|word| word.to_string())
}

fn resolve_rsvp(event: &icalendar::Event, user_email: Option<&str>) -> RsvpStatus {
    let Some(my_email) = user_email else {
        // Sin email propio detectado no se puede matchear el ATTENDEE -> se trata como si
        // hubiera respondido, para no ocultar reuniones por error (SPEC.md §2.4: "no asumir
        // que no aceptado explicitamente equivale a menos prioritario").
        return RsvpStatus::Accepted;
    };

    let my_email_lower = my_email.to_lowercase();
    let attendee = event.get_attendees().into_iter().find(|a| {
        a.cal_address.to_lowercase().trim_start_matches("mailto:") == my_email_lower
    });

    match attendee.and_then(|a| a.part_stat) {
        Some(PartStat::Declined) => RsvpStatus::Declined,
        Some(PartStat::Accepted) => RsvpStatus::Accepted,
        Some(_) | None => RsvpStatus::NeedsAction,
    }
}

/// `Date` (dia completo) se ancla a medianoche UTC; `DateTime` se normaliza vía
/// `try_into_utc` (SPEC.md §2.5 punto 1: tratar dia-completo aparte de fecha-hora).
fn to_utc_and_all_day(date: &DatePerhapsTime) -> Option<(DateTime<Utc>, bool)> {
    match date {
        DatePerhapsTime::Date(naive_date) => {
            let dt = naive_date.and_hms_opt(0, 0, 0)?.and_utc();
            Some((dt, true))
        }
        DatePerhapsTime::DateTime(cdt) => cdt.try_into_utc().map(|dt| (dt, false)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const FIXTURE: &str = "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Test//Test//EN\r\n\
BEGIN:VEVENT\r\n\
UID:accepted-1@test\r\n\
DTSTAMP:20260101T000000Z\r\n\
DTSTART:20260101T100000Z\r\n\
DTEND:20260101T110000Z\r\n\
SUMMARY:Reunion aceptada\r\n\
ATTENDEE;PARTSTAT=ACCEPTED:mailto:me@wom.cl\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:needs-action-1@test\r\n\
DTSTAMP:20260101T000000Z\r\n\
DTSTART:20260101T120000Z\r\n\
DTEND:20260101T130000Z\r\n\
SUMMARY:Reunion sin responder\r\n\
ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:me@wom.cl\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:declined-1@test\r\n\
DTSTAMP:20260101T000000Z\r\n\
DTSTART:20260101T140000Z\r\n\
DTEND:20260101T150000Z\r\n\
SUMMARY:Reunion rechazada\r\n\
ATTENDEE;PARTSTAT=DECLINED:mailto:me@wom.cl\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:cancelled-1@test\r\n\
DTSTAMP:20260101T000000Z\r\n\
DTSTART:20260101T160000Z\r\n\
DTEND:20260101T170000Z\r\n\
SUMMARY:Reunion cancelada\r\n\
STATUS:CANCELLED\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:allday-1@test\r\n\
DTSTAMP:20260101T000000Z\r\n\
DTSTART;VALUE=DATE:20260101\r\n\
DTEND;VALUE=DATE:20260102\r\n\
SUMMARY:Evento todo el dia\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:outside-range-1@test\r\n\
DTSTAMP:20260101T000000Z\r\n\
DTSTART:20260103T100000Z\r\n\
DTEND:20260103T110000Z\r\n\
SUMMARY:Fuera de rango\r\n\
ATTENDEE;PARTSTAT=ACCEPTED:mailto:me@wom.cl\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:recurring-1@test\r\n\
DTSTAMP:20260101T000000Z\r\n\
DTSTART:20260101T090000Z\r\n\
DTEND:20260101T093000Z\r\n\
RRULE:FREQ=WEEKLY;COUNT=5\r\n\
SUMMARY:Reunion semanal\r\n\
ATTENDEE;PARTSTAT=ACCEPTED:mailto:me@wom.cl\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

    #[test]
    fn filtra_declined_cancelados_y_recurrentes_sin_expandir() {
        let from = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();

        let events = parse_and_filter_events(FIXTURE, Some("me@wom.cl"), from, to).unwrap();
        let uids: Vec<&str> = events.iter().map(|e| e.uid.as_str()).collect();

        assert_eq!(
            uids,
            vec!["allday-1@test", "accepted-1@test", "needs-action-1@test"],
            "declined, cancelado, fuera-de-rango y recurrente (sin expandir) deben quedar afuera"
        );
    }

    #[test]
    fn needs_action_se_trata_igual_que_accepted() {
        let from = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();

        let events = parse_and_filter_events(FIXTURE, Some("me@wom.cl"), from, to).unwrap();
        let needs_action = events
            .iter()
            .find(|e| e.uid == "needs-action-1@test")
            .expect("needs-action-1 deberia estar presente");

        assert_eq!(needs_action.rsvp, RsvpStatus::NeedsAction);
    }

    #[test]
    fn evento_todo_el_dia_se_marca_como_tal() {
        let from = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();

        let events = parse_and_filter_events(FIXTURE, Some("me@wom.cl"), from, to).unwrap();
        let all_day = events
            .iter()
            .find(|e| e.uid == "allday-1@test")
            .expect("allday-1 deberia estar presente");

        assert!(all_day.all_day);
    }

    #[test]
    fn extrae_link_de_meet() {
        let text = "Unite a la videollamada: https://meet.google.com/abc-defg-hij por Meet";
        assert_eq!(
            extract_meeting_url(text).as_deref(),
            Some("https://meet.google.com/abc-defg-hij")
        );
    }

    #[test]
    fn extrae_link_de_zoom() {
        let text = "Join Zoom Meeting\nhttps://wom.zoom.us/j/123456789?pwd=abc\nMeeting ID: 123";
        assert_eq!(
            extract_meeting_url(text).as_deref(),
            Some("https://wom.zoom.us/j/123456789?pwd=abc")
        );
    }

    #[test]
    fn extrae_link_de_teams() {
        let text = "<a href=\"https://teams.microsoft.com/l/meetup-join/19%3ameeting_abc\">Unirse</a>";
        assert_eq!(
            extract_meeting_url(text).as_deref(),
            Some("https://teams.microsoft.com/l/meetup-join/19%3ameeting_abc")
        );
    }

    #[test]
    fn sin_link_de_reunion_devuelve_none() {
        assert_eq!(extract_meeting_url("Reunion presencial en sala 3"), None);
    }

    #[test]
    fn extrae_email_de_url_privada_de_google() {
        let url = "https://calendar.google.com/calendar/ical/me%40wom.cl/private-abc123/basic.ics";
        assert_eq!(
            extract_email_from_ics_url(url).as_deref(),
            Some("me@wom.cl")
        );
    }

    #[test]
    fn sin_email_propio_no_oculta_needs_action() {
        // Sin poder matchear ATTENDEE, no se debe asumir DECLINED (SPEC.md §2.4).
        let from = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();

        let events = parse_and_filter_events(FIXTURE, None, from, to).unwrap();
        let uids: Vec<&str> = events.iter().map(|e| e.uid.as_str()).collect();

        // El explicitamente DECLINED del organizador para "me" ya no se puede detectar sin
        // email propio, asi que tambien pasa -- documentado como limite conocido.
        assert!(uids.contains(&"accepted-1@test"));
        assert!(uids.contains(&"needs-action-1@test"));
    }
}
