# Calendar Tray

App de bandeja del sistema para Windows que muestra, de un vistazo, cuánto falta para tu
próxima reunión de Google Calendar — sin usar la API oficial, sin OAuth, y con un consumo de
memoria mínimo (nativa, sin runtime pesado de por medio).

El ícono es un reloj de arena que se va llenando de abajo hacia arriba a medida que se acerca
la hora, con colores tipo semáforo (verde / amarillo / rojo) y parpadeo en el último minuto.

## Funcionalidad

- **Ícono de bandeja — reloj de arena**: color según cuánto falta para la próxima reunión
  (verde > 15 min, amarillo 5–15 min, rojo < 5 min), parpadeo en el último minuto, y estado
  rojo fijo mientras hay una reunión **en curso**.
- **Agenda del día**: click izquierdo abre un flyout junto al ícono con todas las actividades
  del día (pasadas incluidas), cada una con un indicador de color según su estado temporal y
  botones para copiar el link de la reunión o abrirlo en el navegador. Se cierra solo al hacer
  click afuera.
- **Reuniones recurrentes**: expansión real de reglas `RRULE`, con soporte de excepciones
  puntuales (una instancia movida de horario, por ejemplo).
- **Filtro por respuesta RSVP**: oculta las reuniones que rechazaste o que fueron canceladas;
  las que no respondiste explícitamente se muestran igual que las aceptadas.
- **Toasts de cambio de estado**: un aviso breve (se cierra solo a los pocos segundos) cada vez
  que una reunión cambia de fase — verde → amarillo → rojo → "ahora" — agrupando las que
  coinciden en horario.
- **Detección de links de reunión**: Google Meet, Zoom y Microsoft Teams.
- **Autoarranque** con el inicio de sesión de Windows (opcional, vía
  `HKCU\...\CurrentVersion\Run`, sin necesitar permisos de administrador).
- **Resiliencia**: si el feed no responde, no se pierde la última cuenta regresiva conocida de
  golpe — hay un margen de tolerancia antes de mostrar el ícono en estado de error.

## Requisitos

- Windows 10 o 11 (64-bit).
- Una cuenta de Google Calendar con la **URL secreta del feed iCal** (ver más abajo cómo
  conseguirla). No requiere crear nada en Google Cloud Console ni pasar por OAuth.

No hace falta instalar ningún runtime aparte (ni JVM, ni .NET) — es un único ejecutable nativo.

## Instalación

1. Descargá el último `calendar_tray.exe` desde [Releases](../../releases) (o compilalo vos
   mismo, ver más abajo).
2. Ejecutalo. No hay instalador — es un `.exe` portable, corré donde quieras.
3. Click derecho en el ícono de la bandeja → **Configuración**.
4. Pegá tu URL secreta del feed ICS (ver siguiente sección) y guardá.

### Cómo conseguir la URL secreta del feed ICS

En [Google Calendar](https://calendar.google.com):

1. Configuración ⚙️ → **Configuración**.
2. En la lista de la izquierda, elegí tu calendario.
3. Buscá la sección **Integrar calendario**.
4. Copiá la **Dirección secreta en formato iCal**.

Esa URL es secreta — no la compartas ni la subas a ningún repositorio. La app la guarda
localmente en `%APPDATA%\CalendarTray\config.toml`, nunca en el código.

## Configuración

Desde el menú de Configuración se pueden ajustar:

| Campo | Descripción |
|---|---|
| URL del feed ICS | La dirección secreta obtenida arriba. Tiene un botón "Probar" para validar la conexión sin guardar. |
| Umbral verde / amarillo (min) | Minutos de anticipación para cada color del semáforo. |
| Parpadeo (min) | Minutos antes del inicio en que el ícono empieza a parpadear. |
| Refresco del feed (min) | Cada cuánto se vuelve a consultar el calendario. |
| Iniciar con Windows | Autoarranque al iniciar sesión. |

## Compilar desde el código fuente

Requiere:

- [Rust](https://rustup.rs/) (toolchain estable, `x86_64-pc-windows-msvc`).
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)
  con el componente "Desarrollo para escritorio con C++" (para el linker de MSVC).

```bash
git clone https://github.com/brunojimenez/calendar-tray.git
cd calendar-tray
cargo build --release
```

El ejecutable queda en `target\release\calendar_tray.exe`.

```bash
cargo test
```

corre la suite de tests (lógica de dominio: parseo ICS, expansión de RRULE, umbrales del
semáforo, resiliencia de red — sin dependencias externas ni datos reales).

## Estructura del proyecto

```
src/
├── main.rs              # entry point, loop de eventos de Windows
├── calendar/             # CalendarSource + IcsCalendarSource (fetch, parseo, RRULE)
├── meeting_clock.rs       # "cuánto falta para la próxima reunión" y umbrales
├── app_state.rs           # estado de la app, resolución de qué mostrar (lógica pura)
├── tray.rs                # dibujo del ícono (reloj de arena, RGBA generado en runtime)
├── agenda_window.rs        # flyout de la Agenda del día
├── settings_window.rs      # ventana de Configuración
├── toast_window.rs         # toasts de cambio de fase
├── notifications.rs        # detección de transiciones de fase para los toasts
├── autostart.rs            # autoarranque vía registro de Windows
├── config.rs               # carga/guarda %APPDATA%\CalendarTray\config.toml
└── diagnostics.rs          # log mínimo a debug.log para diagnóstico
```

## Estado y roadmap

El diseño completo (decisiones de producto, gotchas técnicos del formato iCalendar, y qué
quedó fuera de esta versión) está documentado en [`SPEC.md`](SPEC.md).

Pendiente del plan original:

- Seguir el tema claro/oscuro del sistema operativo.
- Snooze corto y modo "no molestar" (los campos de configuración ya existen, falta la lógica).
