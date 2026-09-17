//! Autoarranque con el inicio de sesión de Windows (SPEC.md §4).
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` — no requiere admin, por-usuario.

use winreg::HKCU;

const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "CalendarTray";

/// La fuente de verdad es el registro, no `config.autostart` — así si el usuario lo cambió
/// por fuera (ej. Administrador de tareas > Inicio) la casilla de Configuración lo refleja.
pub fn is_enabled() -> bool {
    let Ok(key) = HKCU.open_subkey(RUN_KEY_PATH) else {
        return false;
    };
    key.get_value::<String, _>(VALUE_NAME).is_ok()
}

pub fn set_enabled(enabled: bool) -> std::io::Result<()> {
    let (key, _) = HKCU.create_subkey(RUN_KEY_PATH)?;

    if enabled {
        let exe_path = std::env::current_exe()?;
        // Comillas por si la ruta tiene espacios (ej. "Program Files").
        let value = format!("\"{}\"", exe_path.display());
        key.set_value(VALUE_NAME, &value)?;
    } else {
        match key.delete_value(VALUE_NAME) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }

    Ok(())
}
