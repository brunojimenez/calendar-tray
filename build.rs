use embed_manifest::manifest::DpiAwareness;
use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        // Common Controls v6 (tema visual moderno para botones/labels, viene por defecto en
        // new_manifest) + DPI-awareness explicito -- sin esto Windows estira la app como
        // bitmap en monitores con escalado, dando texto grande/borroso y controles con la
        // chrome plana de Windows 95 en vez del tema actual.
        embed_manifest(
            new_manifest("CalendarTray.App").dpi_awareness(DpiAwareness::PerMonitorV2),
        )
        .expect("no se pudo embeber el manifest de Windows");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
