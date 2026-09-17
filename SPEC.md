# Calendar Tray — Especificación funcional y técnica

> Proyecto personal (no es código WOM/Genesis). Reescritura desde cero: la versión anterior
> (Java 21 + Maven + Swing/AWT) se perdió al borrarse accidentalmente la carpeta de fuentes.
> **Requisito explícito de esta reescritura: tecnología nativa, NO Java/NO JVM.** El motivo es
> el consumo de memoria — la versión Java rondaba los ~200 MB solo por tener la JVM cargada
> para un simple ícono de bandeja.

## 1. Objetivo

App de escritorio para Windows que vive en la bandeja del sistema (system tray) y muestra,
de un vistazo, cuánto falta para la próxima reunión de Google Calendar del usuario. Debe ser
liviana (target: consumo de memoria muy por debajo de los ~200 MB de la versión Java —
idealmente de un solo dígito o bajos dos dígitos de MB) y arrancar sola con el inicio de sesión
de Windows.

## 2. Alcance funcional

### 2.1 Ícono de bandeja — "reloj de arena"

- El ícono de la bandeja es un reloj de arena que representa visualmente cuánto falta para el
  próximo evento (p. ej. arena que se va vaciando a medida que se acerca la hora).
- **Ventana de búsqueda de "próxima reunión": rolling 24 h desde el instante actual** (no un
  corte de "día calendario" a medianoche). Se busca el próximo evento elegible (ver filtros de
  §2.4 y exclusión de eventos de día completo más abajo) cuyo inicio caiga dentro de las
  próximas 24 h; si no hay ninguno, estado neutro. Se acepta esta ventana simple (en vez de
  "próximo evento sin límite de horizonte") asumiendo que las reuniones de trabajo se agendan
  en horario de oficina/tarde — no hace falta anticipar reuniones a más de un día vista.
- Colores tipo semáforo según el tiempo restante a la próxima reunión:
  - **Verde**: faltan más de 15 minutos.
  - **Amarillo**: faltan entre 5 y 15 minutos.
  - **Rojo**: faltan menos de 5 minutos.
  - **Parpadeo**: durante el último minuto antes de que empiece la reunión, el ícono parpadea
    para llamar la atención.
  - *(Estos valores son un punto de partida razonable, no vinieron confirmados con precisión
    de la versión anterior — dejarlos configurables, ver §3, para poder ajustarlos sin
    recompilar.)*
- **El parpadeo se apaga solo por una acción explícita y detectable del usuario dentro de la
  app**, nunca solo — es un distractor si queda pegado sin que el usuario pueda pararlo:
  - Click izquierdo sobre el ícono de bandeja, o
  - Click en el botón "ir a la reunión" dentro de la ventana de Agenda del día (§2.7).
  - *Nota de diseño*: la app no puede detectar de forma confiable si el usuario "ya entró" a
    la reunión por otro medio (móvil, cliente de Meet aparte, etc.) — por eso el apagado de
    parpadeo se ata solo a estas dos acciones que sí controla, no a un intento de detectar la
    asistencia real.
  - El silencio aplica a esa instancia puntual del evento (identificada por UID del evento +
    fecha/hora de la ocurrencia, para distinguir instancias de una serie recurrente) y **no
    afecta la siguiente reunión**: si se acerca otra reunión después, el ícono vuelve a
    parpadear con normalidad para ella.
- **Snooze corto** (mejora de UX, v1.1): en vez de silenciar del todo, una acción de snooze
  pausa el parpadeo por un intervalo corto configurable (default: 2 min, ver §2.8) y luego lo
  retoma automáticamente si la reunión todavía no empezó — para el caso "estoy terminando algo,
  avisame de nuevo en un toque" sin perder la alerta por completo.
- **Tooltip del ícono**: al pasar el mouse por el ícono de bandeja, mostrar un tooltip nativo
  con el detalle de la próxima reunión (título + minutos/hora restante), o el mensaje de estado
  correspondiente si está en error/sin-configurar (§2.6) — evita tener que abrir la Agenda solo
  para confirmar de qué reunión se trata.
- **Modo "no molestar"**: toggle disponible en el menú contextual (§2.2) que suprime el
  parpadeo y la escalada a rojo mientras está activo, para no ser interrumpido durante una
  reunión importante. Mientras está activo, el ícono muestra un estado visual propio y
  distinguible (no confundir con "sin reuniones" ni con el semáforo normal). Tiene una duración
  máxima configurable (p. ej. 30/60 min) tras la cual se desactiva solo, para no dejarlo
  prendido sin querer el resto del día; también se puede apagar manualmente antes desde el
  mismo menú.
- **Eventos de día completo (`DATE`, sin hora) no participan del semáforo ni de la búsqueda de
  "próxima reunión"** — se excluyen por completo de esta lógica (más allá del tratamiento
  aparte que ya requiere §2.5 para no romper el cálculo de recurrencia). No se cuentan como
  motivo para dejar de mostrar el estado neutro tampoco: si el único evento de las próximas
  24 h es uno de día completo, el ícono se comporta como si no hubiera ninguno.
- Si no hay próxima reunión elegible en la ventana de 24 h, el ícono debe mostrar un estado
  neutro (p. ej. reloj de arena "vacío" o gris), sin semáforo.
- **Mejoras futuras fuera de alcance de esta v1** (anotadas para no perderlas, ver también
  §10): rojo permanente mientras el usuario está dentro del rango horario de una reunión en
  curso (con el semáforo volviendo a activarse/parpadeando si se acerca la siguiente); e
  indicador visual dedicado para señalar que hay eventos de día completo activos.

### 2.2 Interacción con el ícono

- **Click** (botón izquierdo) sobre el ícono:
  1. Silencia el parpadeo/alerta de esa reunión puntual (no cierra la app, solo detiene la
     llamada de atención visual para ese evento específico — ver regla de silencio en §2.1).
  2. Abre la ventana propia de Agenda del día (§2.7). **Decisión cerrada**: es una ventana
     propia de la app, no se abre Google Calendar en el navegador — porque la agenda necesita
     mostrar, por evento, acciones propias (copiar link de Meet, ir directo a la reunión; ver
     §2.7) que la vista web de Calendar no ofrece igual de simple.
- **Click derecho**: menú contextual propio (no el menú nativo mínimo de un `TrayIcon`), con
  al menos:
  - Ir a la próxima reunión → atajo directo (sin pasar por la lista) que abre el link de Meet
    de la próxima reunión en el navegador; equivale a un "ir a la reunión" (§2.7) sobre el
    evento actual y también apaga su parpadeo. Deshabilitado/oculto si no hay próxima reunión
    o si no tiene link detectado.
  - Ver agenda del día → abre la misma ventana de Agenda del día que el click izquierdo.
  - No molestar → toggle on/off (ver §2.1), con marca visual de estado activo/inactivo en el
    propio ítem del menú.
  - Configuración → abre la ventana propia de Configuración (§2.8).
  - Elegir tema (claro/oscuro) si no se sigue el tema del SO automáticamente.
  - Salir.

### 2.3 Fuente de datos del calendario

**Decisión heredada de la versión anterior (mantener):** usar el **feed ICS** (URL secreta de
iCal que expone Google Calendar) como fuente **por defecto**, en vez de la API oficial de
Google Calendar.

- **Por qué**: la cuenta del usuario es una cuenta Google Workspace corporativa (dominio
  `wom.cl`) administrada por IT. Para usar la API oficial hay que crear un proyecto en Google
  Cloud Console, y la consola pide elegir un "recurso superior" (organización/carpeta) dentro
  del dominio administrado — el admin del dominio no da permiso para crear proyectos ahí.
  La "URL secreta" de iCal es una función nativa de Google Calendar (Configuración → Integrar
  calendario → "Dirección secreta en formato iCal"), no requiere Cloud Console, OAuth ni
  aprobación de IT.
- **Trade-off aceptado**: el feed ICS no refleja cambios al instante — puede demorar minutos
  u horas en reflejar un evento nuevo o modificado. Se acepta ese delay a cambio de cero
  fricción con IT.
- **Diseño**: la fuente de datos debe quedar detrás de una interfaz/abstracción
  (`CalendarSource` en la versión anterior) para poder enchufar más adelante una fuente vía
  API oficial de Google Calendar (`GoogleApiCalendarSource`) sin tocar el resto de la app.
  Config: un campo tipo `source = "ics" | "googleapi"`.
- La URL del feed ICS es secreta — debe guardarse en el archivo de configuración local del
  usuario, nunca en el código ni en ningún repo.

### 2.4 Filtro de eventos por respuesta RSVP

**Regla de negocio explícita (probada con el usuario, no adivinar):**

- Los eventos donde el usuario respondió **"No" (DECLINED)** se **ocultan** — no aparecen ni
  cuentan como "próxima reunión".
- Los eventos **sin respuesta ("NEEDS-ACTION" / sin RSVP)** se **muestran igual que los
  aceptados**, con el mismo tratamiento visual, sin tachado ni marca especial de "pendiente".
- **Por qué esto importa y no es obvio**: el usuario tiene reuniones a las que debe asistir
  pero nunca respondió explícitamente la invitación, y la interfaz web de Google Calendar a
  veces no las muestra bien (o quedan tapadas detrás de otro evento a la misma hora). La app
  es justamente el respaldo para no perdérselas. Se probó primero tachar visualmente los
  eventos "sin responder" y el usuario pidió revertirlo explícitamente: *"las que no me
  aparecen en Calendar, con click no tengo ni cómo saber si tiene link de Meet"* — necesitaba
  verlas con la misma prioridad visual que las aceptadas.
- Conclusión a preservar en la reescritura: **no asumir que "no aceptado explícitamente"
  equivale a "menos prioritario"** — en este caso de uso es al revés.
- Implementación: matchear el email propio del usuario (se puede extraer del propio contenido
  del feed ICS, o de la URL si el feed lo incluye) contra el organizador/asistente del evento
  para determinar el estado de RSVP propio.
- **Eventos cancelados** (`STATUS:CANCELLED` en el VEVENT): se ocultan igual que los
  `DECLINED` — no aparecen ni cuentan como "próxima reunión".

### 2.5 Recurrencia de eventos (RRULE) — cuidados técnicos a preservar

Estos problemas aparecieron con la librería iCal usada en Java (`ical4j`) al probar contra el
feed **real** del usuario (no con datos sintéticos). Son gotchas del *dominio* del formato
iCalendar/RRULE, así que es muy probable que reaparezcan con cualquier librería equivalente en
la tecnología nueva (en Rust, por ejemplo, crates como `icalendar` o `rrule`). Documentarlos
para que quien implemente los tenga en cuenta desde el diseño:

1. **Eventos de día completo rompen el cálculo de recurrencia si se mezclan con aritmética de
   fecha+hora con zona horaria.** Un evento "todo el día" tiene fecha sin hora
   (`DATE`, no `DATE-TIME`). Si el motor de recurrencia intenta compararlo contra una ventana
   de tipo fecha-hora-con-zona, revienta. **Regla**: filtrar/descartar (o tratar aparte) los
   eventos de día completo *antes* de calcular su set de recurrencia, no intentar que compartan
   el mismo camino de cálculo que los eventos con hora.
2. **El cálculo de recurrencia necesita aritmética de calendario, no de instantes lineales.**
   Para expandir una regla `FREQ=WEEKLY` (o mensual/anual) hay que poder sumar/restar
   "semanas", "meses", etc. de forma calendárica (respetando zona horaria, cambios de horario
   de verano, etc.) — un tipo de instante lineal puro (equivalente a `Instant` en Java) no
   soporta esa aritmética y falla. **Regla**: la ventana de consulta para expandir RRULEs debe
   construirse con un tipo fecha-hora-con-zona (zoned/local datetime), no con un instante
   absoluto en UTC puro.
3. **Verificar contra la librería real antes de asumir su API**, sobre todo si tiene tipos
   genéricos "pesados" para fechas/horas — en la versión Java se verificó la firma real con
   `javap` contra el jar de `ical4j` en vez de adivinar. Aplicar el mismo criterio con la
   librería elegida en la tecnología nueva: revisar su documentación/firmas reales antes de
   codear contra ella a ciegas.
4. **Validar temprano contra el feed .ics real del usuario**, no solo con eventos sintéticos de
   prueba — los dos bugs de arriba solo salieron a la luz con datos reales.
5. **Excepciones de recurrencia (`RECURRENCE-ID`)**: una ocurrencia individual de una serie
   recurrente puede venir modificada (movida de horario, cancelada, etc.) como un VEVENT aparte
   que referencia la serie original vía `RECURRENCE-ID`. El cálculo debe tomar esa instancia
   modificada en vez de la generada por expansión pura de la `RRULE`. Agregar un caso de test
   específico para esto (con fixture del feed real, no solo sintético) antes de dar por
   validada la reescritura — es la clase de gotcha que solo aparece con datos reales, igual
   que los puntos 1 y 2 de arriba.

### 2.6 Resiliencia y manejo de errores

- **Intervalo de refresco**: la app debe volver a consultar el feed ICS **cada 5 minutos como
  máximo** — es el límite de "frescura" que se definió como aceptable para que una edición en
  Google Calendar se refleje en la app (coherente con el trade-off de delay ya aceptado en
  §2.3).
- **Reanudar de suspensión**: al detectar que el equipo salió de suspensión/hibernación, forzar
  un refresh inmediato del feed en vez de esperar al próximo ciclo de polling — evita mostrar
  una cuenta regresiva desactualizada tras horas de equipo dormido.
- **Feed inaccesible** (sin red, timeout, URL secreta revocada/regenerada, respuesta
  inválida): el ícono pasa a un estado neutro/error distinguible (p. ej. gris con un
  indicador visual distinto al "sin reuniones" normal) y el tooltip del ícono debe indicar el
  problema (ej. "Sin conexión al calendario desde hace 12 min"). No se bloquea la última data
  conocida de forma indefinida: si el fetch anterior fue exitoso hace poco, se puede seguir
  mostrando esa última cuenta regresiva conocida por un margen corto (p. ej. hasta 10-15 min de
  antigüedad) antes de degradar a estado de error explícito.
- **Primer arranque sin configurar** (no hay URL/ID de calendario guardado todavía): el ícono
  arranca en un estado "sin configurar" distinguible (no confundir con "sin reuniones" ni con
  "error de red") — el menú contextual (§2.2) sigue funcionando igual para poder abrir
  Configuración (§2.8) y completar el ID del calendario.

### 2.7 Ventana de Agenda del día

Se abre con click izquierdo sobre el ícono o con "Ver agenda del día" del menú contextual
(§2.2). Al abrirse, apaga el parpadeo de la reunión que estuviera alertando (§2.1).

- Lista los eventos del día (aplicando los mismos filtros que el ícono: RSVP `DECLINED` y
  `CANCELLED` ocultos — §2.4 —, eventos de día completo se listan aparte o se excluyen de la
  cuenta regresiva pero pueden mostrarse igual en la lista como contexto del día).
- Cada fila de evento tiene, alineados a la derecha, botones con ícono:
  - **Copiar link de la reunión** al portapapeles.
  - **Ir a la reunión**: abre el link en el navegador predeterminado. Este click también
    cuenta como la acción que apaga el parpadeo de esa reunión puntual (§2.1).
- **Detección de link de reunión** (v1.1): no limitarse a `meet.google.com` — reconocer
  también links de Zoom y Microsoft Teams dentro de la descripción/ubicación del evento, ya
  que en un entorno corporativo mixto llegan invitaciones con esas plataformas igual que con
  Meet.
- **Evento sin link de reunión detectado**: no se muestran los botones de esa fila (no hay nada
  que copiar ni a dónde ir) — se deja solo la información del evento (hora, título).
- **Indicador de "ya silenciada"**: la fila de un evento cuyo parpadeo ya fue apagado por el
  usuario (§2.1) se muestra atenuada/marcada distinto del resto — para no tener que recordar de
  memoria cuál alerta ya se vio al reabrir la ventana.
- **Marca de conflicto de horario**: si dos o más eventos listados se solapan en el tiempo, se
  resalta visualmente esa fila (ambos cuentan igual como "próximos", ver §2.1 — la marca es
  solo para ayudar a decidir a cuál ir, no cambia el filtrado).

### 2.9 Historial de reuniones perdidas (v1.1)

- Se considera "perdida" una reunión cuyo horario de inicio ya pasó **sin** que el usuario haya
  realizado la acción de silencio (click en ícono o "ir a la reunión", §2.1) para esa instancia
  puntual.
- Se muestran en una sección aparte, colapsable, dentro de la misma ventana de Agenda del día
  (§2.7) — p. ej. "Reuniones perdidas hoy" debajo de la lista de eventos futuros/en curso.
- Alcance acotado al día en curso: la lista se reinicia al pasar de día, no se mantiene un
  historial multi-día persistente (eso sería una ampliación mayor, fuera de esta v1.1).

### 2.8 Ventana de Configuración

Se abre con "Configuración" del menú contextual (§2.2). Formulario editable con, al menos, en
este orden:

1. ID/URL del feed ICS del calendario (el campo más importante — sin esto la app no funciona,
   ver primer-arranque en §2.6).
2. Fuente de datos (`source`: `ics` / `googleapi`, ver §2.3).
3. Umbrales de minutos para verde/amarillo/rojo y parpadeo (§2.1).
4. Duración del snooze corto (default 2 min, §2.1).
5. Duración máxima del modo "no molestar" (default 30 min, §2.1).
6. Tema: claro/oscuro/seguir sistema (§5).
7. Autoarranque on/off (§4).

Los cambios se guardan en el archivo de configuración local (§3) al confirmar el formulario.

## 3. Configuración

- Archivo de configuración local editable por el usuario (equivalente a
  `%APPDATA%\CalendarTray\config.properties` de la versión anterior). Mantener esa misma
  carpeta/ubicación por continuidad, salvo que la tecnología nueva tenga una convención propia
  más idiomática (p. ej. `%APPDATA%\CalendarTray\config.toml` si se usa Rust con `serde` +
  TOML).
- Debe incluir al menos (mismo orden que el formulario de Configuración, §2.8):
  - URL del feed ICS (secreta — nunca en el código ni versionada).
  - `source`: `"ics"` (default) o `"googleapi"`.
  - Umbrales de minutos para verde/amarillo/rojo y parpadeo (configurables, ver §2.1).
  - Duración del snooze corto en minutos (default: 2 — ver §2.1).
  - Duración máxima del modo "no molestar" en minutos (default: 30 — ver §2.1).
  - Tema: claro/oscuro/seguir sistema.
  - Autoarranque: on/off (ver §4).
  - Intervalo de refresco del feed en minutos (default y máximo: 5 — ver §2.6).

## 4. Autoarranque

La app debe registrarse para iniciar automáticamente con el inicio de sesión de Windows (es
una app de bandeja pensada para estar siempre corriendo en segundo plano). Mecanismo típico en
Windows: entrada en `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, o un acceso directo
en la carpeta de inicio (`shell:startup`) apuntando al ejecutable. Debe poder
activarse/desactivarse desde el menú de configuración (no forzado sin opción de apagarlo).

## 5. Tema visual (claro/oscuro)

La versión anterior en Swing/AWT no seguía el tema de Windows automáticamente, así que se
implementó un tema propio (claro/oscuro) manual. Si la tecnología nueva permite seguir el tema
del sistema operativo de forma nativa (Windows expone el modo claro/oscuro), preferir eso por
sobre reimplementar un tema propio — solo mantener un tema custom si la librería de UI elegida
no lo soporta de forma nativa.

## 6. Empaquetado y distribución

- Salida final: **un único ejecutable** para Windows (equivalente al `CalendarTray.exe` que se
  generaba con `jpackage` en la versión Java), sin necesidad de instalar un runtime aparte
  (nada de "instalá el JRE primero" — justamente ese era el problema de la versión anterior).
  Con Rust esto es natural (binario nativo estático o casi estático).
- El plan de uso es: compilar en el equipo de casa (donde sí hay entorno de compilación),
  y traer a este equipo **solo el ejecutable final**, no los fuentes ni el toolchain.

## 7. Requisitos no funcionales

- **Memoria**: objetivo explícito de esta reescritura — bajar muy por debajo de los ~200 MB
  que consumía la JVM en la versión Java. Con un stack nativo (Rust recomendado, ver §8) el
  target razonable es de un dígito a bajos dos dígitos de MB en reposo.
- **Arranque**: debe ser prácticamente instantáneo (sin el costo de arranque de una VM/runtime
  administrado).
- **Sin instalador pesado**: idealmente un solo `.exe` portable; un instalador es opcional, no
  requisito.
- **Sin dependencia de Cloud/OAuth por defecto**: la fuente `ics` no debe requerir que el
  usuario pase por ningún flujo de autenticación OAuth ni configuración en Google Cloud
  Console — ese es justamente el punto de usar el feed ICS por defecto (§2.3).
- **Resiliencia**: un feed temporalmente inaccesible (sin red, URL revocada, timeout) no debe
  crashear la app ni dejarla en un estado ambiguo — debe degradar a un estado de ícono
  reconocible como error, según §2.6.
- **Íconos en múltiples resoluciones**: el ícono de bandeja debe proveerse en al menos 16x16 y
  32x32 (idealmente como `.ico` multi-resolución) para verse correctamente con distintos
  niveles de escalado/DPI de Windows.
- **Idioma de la interfaz**: español (menú contextual, ventana de Configuración, ventana de
  Agenda del día) — no se requiere soporte multi-idioma.

## 8. Tecnología recomendada para la reescritura

**Recomendación: Rust**, con:

- Ícono de bandeja: crate tipo [`tray-icon`](https://crates.io/crates/tray-icon) (o
  equivalente vigente al momento de implementar — verificar el ecosistema actual, no asumir
  que sigue siendo el mismo crate/versión).
- Ventana de configuración y ventana de agenda del día (**decisión cerrada: ambas son ventanas
  propias de la app, no se delega a abrir el navegador** — ver §2.7 y §2.8, la ventana de
  agenda necesita botones por evento que una vista web no da igual de simple): un toolkit
  liviano tipo `egui`/`iced`. Queda descartada la opción de Win32/`windows-rs` puro sin
  toolkit, porque ambas ventanas necesitan listas con filas de controles (botones con ícono,
  formularios), no solo diálogos mínimos.
- Parseo de ICS y expansión de RRULE: buscar el crate vigente equivalente a `ical4j`
  (candidatos a evaluar al momento de implementar: `icalendar`, `rrule`, `ical`) — prestar
  atención a los gotchas del §2.5, que son del formato iCalendar, no de una librería en
  particular.
- Por qué Rust y no las alternativas evaluadas:
  - **Go** (`systray` + `Fyne`/`Walk`): API más simple, memoria baja (~10-20 MB), pero
    ecosistema de UI nativa en Windows más limitado.
  - **C#/.NET con Native AOT**: estilo más parecido a Java (más cercano a lo ya conocido),
    buen soporte de tray en Windows (WinForms/WPF), Native AOT baja bastante el consumo vs. la
    JVM tradicional, pero no llega al nivel de Rust/Go.
  - Se prioriza Rust por dar la menor huella de memoria posible (objetivo explícito de esta
    reescritura) a costa de una curva de aprendizaje algo mayor si no se conoce el lenguaje —
    aceptable dado que el desarrollo se hará con asistencia de Claude en el equipo de casa.

> Esta sección es una recomendación de partida, no una decisión cerrada — al retomar el
> desarrollo en el equipo de casa, validar disponibilidad y madurez actual de los crates antes
> de comprometerse.

## 9. Estructura de referencia (versión anterior, solo como mapa conceptual)

No migrar literalmente estos nombres de clase a Rust (no aplica un mapeo 1:1 OO), pero sirve
como mapa de las piezas que existían y sus responsabilidades:

- `Main` — entry point, arranque de la app y del ícono de bandeja.
- `TrayController` — maneja el ícono, sus estados/colores y los eventos de click.
- `SettingsDialog` — ventana/diálogo de configuración (ver §2.8).
- `AgendaWindow` — ventana con la lista de eventos del día y sus acciones (copiar link, ir a
  la reunión) (ver §2.7) — pieza nueva respecto a la versión anterior, no existía como tal.
- `HourglassRenderer` — dibuja el ícono del reloj de arena según el estado (incluye los
  estados nuevos de error/sin-configurar de §2.6, además del semáforo normal).
- `CalendarSource` (interfaz) — contrato para obtener eventos del día.
  - `IcsCalendarSource` — implementación vía feed ICS + expansión de RRULE.
  - `GoogleApiCalendarSource` — implementación alternativa vía API oficial (no usada por
    defecto, ver §2.3).
- `MeetingClock` — lógica de "cuánto falta para la próxima reunión" (ventana rolling de 24 h,
  §2.1), umbrales de semáforo, estado de silencio/snooze por instancia de evento, y estado del
  modo "no molestar".
- `AppConfig` — carga/guarda la configuración local (`source`, URL del feed, umbrales,
  duraciones de snooze/no-molestar, tema, autoarranque, intervalo de refresco — ver §3).

## 10. Fuera de alcance (por ahora)

- Soporte multi-calendario / múltiples cuentas Google.
- Sincronización en tiempo real (websocket/push) — se acepta el delay del feed ICS (§2.3).
- Notificaciones nativas de Windows (toast) más allá del parpadeo del ícono — evaluar como
  mejora futura, no requisito de esta reescritura.
- Instalador MSI/gráfico — un `.exe` portable es suficiente.
- Rojo permanente en el ícono mientras el usuario está dentro del rango horario de una reunión
  en curso (§2.1) — mejora futura, no requisito v1.
- Indicador visual dedicado para eventos de día completo activos (§2.1) — mejora futura, no
  requisito v1.
- Detección real de "el usuario entró a la reunión" más allá de las dos acciones controladas
  por la app (click en ícono / click en "ir a la reunión", §2.1) — no es técnicamente viable
  sin integraciones adicionales, queda fuera de alcance.

## 11. Historial / origen de este documento

Esta especificación reconstruye el conocimiento acumulado en una sesión de desarrollo previa
(2026-09-09) de la versión Java de Calendar Tray, cuyos fuentes se perdieron al borrarse
accidentalmente la carpeta del proyecto. Se preserva aquí todo el conocimiento de dominio
(reglas de negocio, decisiones y gotchas técnicos del formato iCalendar) para no tener que
redescubrirlo en la reescritura nativa.

**Actualización (2026-09-17)**: se cerraron las ambigüedades que quedaban abiertas para poder
empezar a implementar — ventana de "próxima reunión" (rolling 24 h), semántica exacta del
apagado de parpadeo, exclusión de eventos de día completo del semáforo, eventos cancelados,
intervalo de refresco (5 min), manejo de feed inaccesible/primer arranque, y el diseño de las
ventanas propias de Configuración y Agenda del día (con acciones de copiar/ir a la reunión por
evento). Con esto la especificación se considera cerrada para empezar la reescritura en Rust.

Misma fecha, segunda pasada: se sumaron mejoras de experiencia de usuario — tooltip del ícono,
atajo de menú "ir a la próxima reunión", modo "no molestar", snooze corto, indicador de alerta
ya vista y marca de conflicto de horario en la Agenda, detección de links de Zoom/Teams además
de Meet, e historial de reuniones perdidas del día (§2.9). Las marcadas como v1.1 en el texto
son candidatas a una segunda iteración, no bloquean el arranque de la v1 si se prioriza
entregar antes.
