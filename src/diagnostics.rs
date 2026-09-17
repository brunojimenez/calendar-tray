//! Log mínimo a `%APPDATA%\CalendarTray\debug.log` para poder diagnosticar problemas de
//! parseo/RRULE sin depender de que el usuario comparta el contenido de su calendario acá.
//! Solo se registran títulos de eventos y metadatos estructurales (uid, fechas, si una regla
//! de recurrencia se pudo validar) — nunca emails de asistentes ni links de reunión.

use std::io::Write;

pub fn log(line: impl AsRef<str>) {
    let Some(dir) = crate::config::AppConfig::config_path()
        .parent()
        .map(|p| p.to_path_buf())
    else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("debug.log"))
    {
        let _ = writeln!(
            file,
            "[{}] {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
            line.as_ref()
        );
    }
}
