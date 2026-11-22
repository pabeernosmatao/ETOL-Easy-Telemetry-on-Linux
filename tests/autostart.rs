//! Prueba de ida y vuelta del fichero XDG de autostart.
//!
//! Usa `ETOL_AUTOSTART_DIR` para no tocar el autostart real de la sesion.

use std::fs;

#[test]
fn enable_upgrade_and_disable() {
    let dir = std::env::temp_dir().join(format!("etol-autostart-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    std::env::set_var("ETOL_AUTOSTART_DIR", &dir);

    let status = etol::autostart::enable().expect("activar autostart");
    assert!(status.enabled);
    assert_eq!(status.path, dir.join("etol.desktop").display().to_string());

    let text = fs::read_to_string(&dir.join("etol.desktop")).expect("leer entrada");
    assert!(text.contains("Exec="));
    assert!(text.contains("--autostart"));
    assert!(text.contains("StartupWMClass=etol-gui"));

    // Una entrada antigua sin --autostart se actualiza sin cambiar su ruta.
    let existing = dir.join("ETOL-viejo.AppImage");
    fs::write(&existing, b"x").expect("crear lanzador de prueba");
    let old = format!(
        "[Desktop Entry]\nType=Application\nName=ETOL\nExec={}\n",
        existing.display()
    );
    fs::write(dir.join("etol.desktop"), old).expect("escribir entrada antigua");
    etol::autostart::upgrade_if_needed();
    let upgraded = fs::read_to_string(dir.join("etol.desktop")).expect("leer entrada nueva");
    assert!(upgraded.contains(&format!("Exec={} --autostart", existing.display())));

    // Si la ruta ya no existe, se reescribe con el lanzador actual.
    let stale = "[Desktop Entry]\nType=Application\nName=ETOL\nExec=/no/existe/ETOL.AppImage\n";
    fs::write(dir.join("etol.desktop"), stale).expect("escribir entrada rota");
    etol::autostart::upgrade_if_needed();
    let repaired = fs::read_to_string(dir.join("etol.desktop")).expect("leer entrada reparada");
    assert!(!repaired.contains("/no/existe"));
    assert!(repaired.contains("--autostart"));

    let status = etol::autostart::disable().expect("desactivar autostart");
    assert!(!status.enabled);
    assert!(!dir.join("etol.desktop").exists());

    let _ = fs::remove_dir_all(&dir);
}
