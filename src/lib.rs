//! Nucleo de etol: deteccion, configuracion, estado, doctor y lanzador.
//!
//! Lo comparten el binario de linea de comandos (`src/main.rs`) y la interfaz
//! grafica (`src-tauri`).

pub mod cli;
pub mod config;
pub mod daemon;
pub mod detect;
pub mod doctor;
pub mod envgen;
pub mod i18n;
pub mod kwin;
pub mod launch;
pub mod logs;
pub mod messages;
pub mod payload;
pub mod state;
pub mod status;
pub mod util;
pub mod vdf;

/// Limpia variables del runtime del AppImage que rompen a los scripts.
///
/// El AppImage exporta `PYTHONHOME` y `PYTHONPATH` apuntando a su montaje de
/// solo lectura. Los scripts del payload llaman al `python3` del sistema y
/// mueren con `No module named 'encodings'`, dejando la configuracion de
/// SimHub/CrewChief a medias. Se quitan al arrancar para que todos los hijos
/// hereden un entorno limpio.
pub fn sanitize_env() {
    for key in ["PYTHONHOME", "PYTHONPATH"] {
        std::env::remove_var(key);
    }
}
