//! Idioma de la aplicacion.
//!
//! El idioma se guarda en `[ui] language` como `auto`, `es`, `ca` o `en`.
//! `auto` detecta el idioma del sistema (español/catalan/resto -> ingles).
//! Los textos del backend usan el macro `tr!` para no perder ninguna cadena.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Es,
    Ca,
    En,
}

static CURRENT: AtomicU8 = AtomicU8::new(3); // 3 = sin fijar (auto)

pub fn code(lang: Lang) -> &'static str {
    match lang {
        Lang::Es => "es",
        Lang::Ca => "ca",
        Lang::En => "en",
    }
}

fn from_code(value: &str) -> Option<Lang> {
    match value.trim().to_lowercase().as_str() {
        "es" | "spa" | "spanish" | "espanol" | "español" => Some(Lang::Es),
        "ca" | "cat" | "catalan" | "català" | "catala" => Some(Lang::Ca),
        "en" | "eng" | "english" | "ingles" | "inglés" => Some(Lang::En),
        _ => None,
    }
}

/// Idioma del sistema a partir de LC_ALL, LC_MESSAGES o LANG.
pub fn detect() -> Lang {
    let raw = std::env::var("LC_ALL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("LC_MESSAGES")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| {
            std::env::var("LANGUAGE")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| std::env::var("LANG").ok().filter(|value| !value.trim().is_empty()))
        .unwrap_or_default()
        .to_lowercase();

    let base = raw
        .split([':', '.', '@'])
        .next()
        .unwrap_or("")
        .replace('-', "_");

    if base.starts_with("es") {
        Lang::Es
    } else if base.starts_with("ca") {
        Lang::Ca
    } else {
        Lang::En
    }
}

/// Resuelve el valor guardado en la configuracion.
pub fn resolve(setting: &str) -> Lang {
    if setting.trim().eq_ignore_ascii_case("auto") || setting.trim().is_empty() {
        return detect();
    }
    from_code(setting).unwrap_or_else(detect)
}

pub fn set(lang: Lang) {
    CURRENT.store(
        match lang {
            Lang::Es => 0,
            Lang::Ca => 1,
            Lang::En => 2,
        },
        Ordering::Relaxed,
    );
}

pub fn set_from_setting(setting: &str) -> Lang {
    let lang = resolve(setting);
    set(lang);
    lang
}

pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        0 => Lang::Es,
        1 => Lang::Ca,
        2 => Lang::En,
        _ => detect(),
    }
}

/// Devuelve el texto en el idioma activo.
pub fn t(es: &'static str, ca: &'static str, en: &'static str) -> &'static str {
    match current() {
        Lang::Es => es,
        Lang::Ca => ca,
        Lang::En => en,
    }
}

/// Traduce una cadena con formato.
#[macro_export]
macro_rules! tr {
    ($es:expr, $ca:expr, $en:expr) => {
        $crate::i18n::t($es, $ca, $en)
    };
    ($es:expr, $ca:expr, $en:expr, $($arg:tt)*) => {
        match $crate::i18n::current() {
            $crate::i18n::Lang::Es => format!($es, $($arg)*),
            $crate::i18n::Lang::Ca => format!($ca, $($arg)*),
            $crate::i18n::Lang::En => format!($en, $($arg)*),
        }
    };
}

/// Traduce una cadena del backend que no usa `tr!` (por ejemplo, el doctor).
/// Las cadenas que ya vienen traducidas se dejan intactas.
pub fn text(es: &str) -> String {
    let lang = current();
    if lang == Lang::Es {
        return es.to_string();
    }
    let key = es.trim();
    let direct = match lang {
        Lang::Ca => ca_text(key),
        Lang::En => en_text(key),
        Lang::Es => None,
    };
    if let Some(value) = direct {
        return value.to_string();
    }

    // Patrones con valores variables (rutas, numeros...).
    if let Some(rest) = key.strip_prefix("faltan permisos: ") {
        return format!("{} {rest}", t("falten permisos:", "falten permisos:", "missing permissions:"));
    }
    if let Some(rest) = key.strip_prefix("tambien hay una instalacion del sistema en ") {
        return format!(
            "{} {rest}",
            t(
                "també hi ha una instal·lació del sistema a",
                "també hi ha una instal·lació del sistema a",
                "there is also a system installation in"
            )
        );
    }
    if let Some(rest) = key.strip_prefix("el wine configurado no existe: ") {
        return format!(
            "{} {rest}",
            t(
                "el wine configurat no existeix:",
                "el wine configurat no existeix:",
                "the configured wine does not exist:"
            )
        );
    }
    if let Some(rest) = key.strip_prefix("instalado en ") {
        return format!(
            "{} {rest}",
            t("instal·lat a", "instal·lat a", "installed at")
        );
    }
    if let Some(rest) = key.strip_prefix("Espejo ") {
        return format!("{} {rest}", t("Mirror", "Mirror", "Mirror"));
    }
    if let Some(rest) = key.strip_prefix("falta ") {
        return format!("{} {rest}", t("falta", "falta", "missing"));
    }
    if let Some(rest) = key.strip_prefix("abierto por: ") {
        return format!("{} {rest}", t("abierto por:", "obert per:", "opened by:"));
    }
    if let Some(rest) = key.strip_prefix("Puerto ") {
        return format!("{} {rest}", t("Puerto", "Port", "Port"));
    }
    if key.contains(" juego(s) enlazados, ") && key.ends_with(" sin enlace en la botella") {
        if let Some((left, right)) = key.split_once(" juego(s) enlazados, ") {
            let right = right.trim_end_matches(" sin enlace en la botella");
            return match lang {
                Lang::Ca => format!("{left} joc(s) enllaçats, {right} sense enllaç a l'ampolla"),
                Lang::En => format!("{left} game(s) linked, {right} without a link in the bottle"),
                Lang::Es => key.to_string(),
            };
        }
    }
    if key.ends_with(" juego(s) visibles en la botella") {
        let left = key.trim_end_matches(" juego(s) visibles en la botella");
        return match lang {
            Lang::Ca => format!("{left} joc(s) visibles a l'ampolla"),
            Lang::En => format!("{left} game(s) visible in the bottle"),
            Lang::Es => key.to_string(),
        };
    }
    if key.starts_with("apunta a ") && key.contains(" en vez de ") {
        if let Some((left, right)) = key.split_once(" en vez de ") {
            let left = left.trim_start_matches("apunta a ");
            return match lang {
                Lang::Ca => format!("apunta a {left} en lloc de {right}"),
                Lang::En => format!("points to {left} instead of {right}"),
                Lang::Es => key.to_string(),
            };
        }
    }
    if key.starts_with("arranque en curso (") && key.ends_with(" procesos)") {
        let inner = key
            .trim_start_matches("arranque en curso (")
            .trim_end_matches(" procesos)");
        return match lang {
            Lang::Ca => format!("arrencada en curs ({inner} processos)"),
            Lang::En => format!("startup in progress ({inner} processes)"),
            Lang::Es => key.to_string(),
        };
    }
    if key.ends_with(" · prefijo listo") {
        let left = key.trim_end_matches(" · prefijo listo");
        return match lang {
            Lang::Ca => format!("{left} · prefix llest"),
            Lang::En => format!("{left} · prefix ready"),
            Lang::Es => key.to_string(),
        };
    }
    if key.starts_with("hay un enlace manual ") && key.ends_with("; redetecta para migrarlo a enlaces por juego") {
        let path = key
            .trim_start_matches("hay un enlace manual ")
            .trim_end_matches("; redetecta para migrarlo a enlaces por juego");
        return match lang {
            Lang::Ca => format!("hi ha un enllaç manual {path}; torna a detectar per migrar-lo a enllaços per joc"),
            Lang::En => format!("there is a manual link {path}; detect again to migrate it to per-game links"),
            Lang::Es => key.to_string(),
        };
    }

    es.to_string()
}

fn ca_text(key: &str) -> Option<&'static str> {
    Some(match key {
        "no se usa: la configuracion apunta a una carpeta de scripts propia" => "no s'usa: la configuració apunta a una carpeta de scripts pròpia",
        "captura desactivada en Opciones" => "captura desactivada a Opcions",
        "Carpeta de scripts" => "Carpeta de scripts",
        "Botella de SimHub" => "Ampolla de SimHub",
        "Bottles nativa" => "Bottles nativa",
        "ejecutable" => "executable",
        "no es ejecutable" => "no és executable",
        "no esta en la configuracion" => "no està a la configuració",
        "ruta del juego" => "ruta del joc",
        "prefijo de Proton" => "prefix de Proton",
        "wine de Proton" => "wine de Proton",
        "solo esta la instalacion de sistema (admin); falta la de usuario" => "només hi ha la instal·lació de sistema (admin); falta la d'usuari",
        "instalacion Flatpak de usuario" => "instal·lació Flatpak d'usuari",
        "instalacion Flatpak de sistema" => "instal·lació Flatpak de sistema",
        "no se encontro Bottles (ni nativo ni Flatpak)" => "no s'ha trobat Bottles (ni natiu ni Flatpak)",
        "Permisos Bottles Flatpak" => "Permisos de Bottles Flatpak",
        "home, host-etc, Steam y devices=all" => "home, host-etc, Steam i devices=all",
        "Enlaces de Steam" => "Enllaços de Steam",
        "no se encontro SimHubWPF.exe" => "no s'ha trobat SimHubWPF.exe",
        "no se encontro CrewChiefV4.exe" => "no s'ha trobat CrewChiefV4.exe",
        "Dispositivos USB" => "Dispositius USB",
        "escaneo desactivado en Opciones; no se busca el pad" => "escaneig desactivat a Opcions; no es cerca el pad",
        "HF8 conectado" => "HF8 connectat",
        "SimHub y el pad" => "SimHub i el pad",
        "SimHub tiene abierto el dispositivo" => "SimHub té obert el dispositiu",
        "nadie tiene abierto el dispositivo" => "ningú té obert el dispositiu",
        "abre SimHub o lanza el juego" => "obre SimHub o llança el joc",
        "desactivado en la configuracion" => "desactivat a la configuració",
        "escuchando en TCP 8888" => "escoltant a TCP 8888",
        "SimHub no escucha en TCP 8888 (abre SimHub o revisa su configuracion)" => "SimHub no escolta a TCP 8888 (obre SimHub o revisa'n la configuració)",
        "permite TCP 8888" => "permet TCP 8888",
        "no hay firewall activo" => "no hi ha firewall actiu",
        "el firewall bloquea TCP 8888 para los dispositivos de la wifi" => "el firewall bloqueja TCP 8888 per als dispositius del wifi",
        "no se pudo comprobar el firewall (prueba ufw o firewalld)" => "no s'ha pogut comprovar el firewall (prova ufw o firewalld)",
        "el espejo corre pero el puerto no responde" => "l'espill corre però el port no respon",
        "en uso por el espejo" => "en ús per l'espill",
        "libre" => "lliure",
        "Puerto 8888 (comprobado)" => "Port 8888 (comprovat)",
        "extradata=3 en 127.0.0.1:20777" => "extradata=3 a 127.0.0.1:20777",
        "no se encontro hardware_settings_config.xml (abre el juego una vez)" => "no s'ha trobat hardware_settings_config.xml (obre el joc una vegada)",
        "hace falta enabled=true, extradata=3 y puerto 20777" => "cal enabled=true, extradata=3 i port 20777",
        "Pulsa Lanzar con el juego cerrado" => "Prem Llança amb el joc tancat",
        "detectado" => "detectat",
        "Wine y botella" => "Wine i ampolla",
        "no hay una botella valida de la que resolver el wine" => "no hi ha cap ampolla vàlida per resoldre el wine",
        "no hay botella configurada" => "no hi ha cap ampolla configurada",
        _ => return None,
    })
}

fn en_text(key: &str) -> Option<&'static str> {
    Some(match key {
        "no se usa: la configuracion apunta a una carpeta de scripts propia" => "not used: the configuration points to a custom scripts folder",
        "captura desactivada en Opciones" => "capture disabled in Options",
        "Carpeta de scripts" => "Scripts folder",
        "Botella de SimHub" => "SimHub bottle",
        "Bottles nativa" => "Bottles native",
        "ejecutable" => "executable",
        "no es ejecutable" => "not executable",
        "no esta en la configuracion" => "not in the configuration",
        "ruta del juego" => "game path",
        "prefijo de Proton" => "Proton prefix",
        "wine de Proton" => "Proton wine",
        "solo esta la instalacion de sistema (admin); falta la de usuario" => "only the system installation (admin) is present; the user one is missing",
        "instalacion Flatpak de usuario" => "user Flatpak installation",
        "instalacion Flatpak de sistema" => "system Flatpak installation",
        "no se encontro Bottles (ni nativo ni Flatpak)" => "Bottles not found (native or Flatpak)",
        "Permisos Bottles Flatpak" => "Bottles Flatpak permissions",
        "home, host-etc, Steam y devices=all" => "home, host-etc, Steam and devices=all",
        "Enlaces de Steam" => "Steam links",
        "no se encontro SimHubWPF.exe" => "SimHubWPF.exe not found",
        "no se encontro CrewChiefV4.exe" => "CrewChiefV4.exe not found",
        "Dispositivos USB" => "USB devices",
        "escaneo desactivado en Opciones; no se busca el pad" => "scanning disabled in Options; the pad is not scanned",
        "HF8 conectado" => "HF8 connected",
        "SimHub y el pad" => "SimHub and the pad",
        "SimHub tiene abierto el dispositivo" => "SimHub has the device open",
        "nadie tiene abierto el dispositivo" => "nobody has the device open",
        "abre SimHub o lanza el juego" => "open SimHub or launch the game",
        "desactivado en la configuracion" => "disabled in the configuration",
        "escuchando en TCP 8888" => "listening on TCP 8888",
        "SimHub no escucha en TCP 8888 (abre SimHub o revisa su configuracion)" => "SimHub is not listening on TCP 8888 (open SimHub or check its configuration)",
        "permite TCP 8888" => "allows TCP 8888",
        "no hay firewall activo" => "no active firewall",
        "el firewall bloquea TCP 8888 para los dispositivos de la wifi" => "the firewall blocks TCP 8888 for Wi-Fi devices",
        "no se pudo comprobar el firewall (prueba ufw o firewalld)" => "could not check the firewall (try ufw or firewalld)",
        "el espejo corre pero el puerto no responde" => "the mirror is running but the port does not respond",
        "en uso por el espejo" => "in use by the mirror",
        "libre" => "free",
        "Puerto 8888 (comprobado)" => "Port 8888 (checked)",
        "extradata=3 en 127.0.0.1:20777" => "extradata=3 on 127.0.0.1:20777",
        "no se encontro hardware_settings_config.xml (abre el juego una vez)" => "hardware_settings_config.xml not found (open the game once)",
        "hace falta enabled=true, extradata=3 y puerto 20777" => "enabled=true, extradata=3 and port 20777 are required",
        "Pulsa Lanzar con el juego cerrado" => "Press Launch with the game closed",
        "detectado" => "detected",
        "Wine y botella" => "Wine and bottle",
        "no hay una botella valida de la que resolver el wine" => "there is no valid bottle to resolve the wine from",
        "no hay botella configurada" => "no bottle configured",
        _ => return None,
    })
}
