/* Panel de etol: capa fina sobre los comandos de Tauri. */

const invoke = window.__TAURI__?.core?.invoke;

// La ventana siempre debe abrirse arriba, sin heredar scroll de sesiones previas.
if ("scrollRestoration" in history) history.scrollRestoration = "manual";

const state = {
  config: null,
  snapshot: null,
  report: null,
  paths: { config: "—", env: "—" },
  logsPath: "—",
  statusKey: "",
  lang: "es",
};

const $ = (selector, root = document) => root.querySelector(selector);

function normalizeText(value) {
  return String(value ?? "")
    .replace(/\s+/g, " ")
    .trim();
}

/** Traduce una cadena de la interfaz (clave: texto en español). */
function t(text) {
  const key = normalizeText(text);
  if (state.lang === "es") return text;
  return I18N[state.lang]?.[key] ?? text;
}

/** Traduce una cadena con marcadores {0}, {1}... */
function tf(text, ...args) {
  return t(text).replace(/\{(\d+)\}/g, (_, index) => String(args[index] ?? ""));
}

const originalText = new WeakMap();
const originalAttrs = new WeakMap();
const TRANSLATABLE_ATTRS = ["title", "placeholder", "aria-label"];

function translateValue(value) {
  const key = normalizeText(value);
  if (!key) return value;
  const dict = I18N[state.lang];
  if (!dict) return value;

  if (dict[key]) return dict[key];

  // Valores compuestos que llevan una parte traducible.
  if (key.startsWith("Configurar ")) {
    return `${dict["Configurar"]}${key.slice("Configurar".length)}`;
  }
  for (const prefix of ["Abriendo", "Lanzando", "Parando"]) {
    if (key.startsWith(`${prefix} `) && dict[prefix]) {
      return `${dict[prefix]}${key.slice(prefix.length)}`;
    }
  }
  if (key.endsWith(" arrancando (pid") || key.includes(" arrancando (pid ")) {
    return value.replace(" arrancando (pid ", ` ${dict["arrancando"] ?? "arrancando"} (pid `);
  }
  const port = key.match(/^(\d+)\s+(en uso|ocupado|libre)$/);
  if (port) return `${port[1]} ${dict[port[2]] ?? port[2]}`;
  const count = key.match(/^(\d+)\s+(fallo|fallos|aviso|avisos)$/);
  if (count) return `${count[1]} ${dict[count[2]] ?? count[2]}`;
  if (key.startsWith("botella:")) {
    return `${dict["botella:"]}${key.slice("botella:".length)}`;
  }
  let out = value;
  for (const fragment of ["(incorrecto)", "sin enlace", "no conectado", "no detectado", "no detectada"]) {
    if (dict[fragment]) out = out.split(fragment).join(dict[fragment]);
  }
  return out;
}

function translateTextNode(node) {
  let original = originalText.get(node);
  if (original === undefined) {
    original = node.nodeValue;
    originalText.set(node, original);
  }
  const trimmed = original.trim();
  if (!trimmed) return;
  const translated =
    state.lang === "es" ? trimmed : translateValue(trimmed);
  if (translated === trimmed) {
    node.nodeValue = original;
    return;
  }
  const start = original.indexOf(trimmed);
  node.nodeValue =
    original.slice(0, start) + translated + original.slice(start + trimmed.length);
}

function translateAttrs(element) {
  let values = originalAttrs.get(element);
  if (!values) {
    values = new Map();
    originalAttrs.set(element, values);
  }
  for (const attr of TRANSLATABLE_ATTRS) {
    if (!element.hasAttribute(attr)) continue;
    if (!values.has(attr)) values.set(attr, element.getAttribute(attr));
    const original = values.get(attr);
    const value = state.lang === "es" ? original : translateValue(original);
    if (element.getAttribute(attr) !== value) element.setAttribute(attr, value);
  }
}

function translateDom(root = document.body) {
  if (!root) return;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) {
    const node = walker.currentNode;
    if (
      node.parentElement?.closest(
        "code, pre, script, style, [data-no-i18n], #toast"
      )
    ) {
      continue;
    }
    translateTextNode(node);
  }
  if (root.nodeType === 1) translateAttrs(root);
  root.querySelectorAll?.("*").forEach(translateAttrs);
}

/** Aplica un idioma y vuelve a pintar las partes dinamicas. */
function setLanguage(lang) {
  state.lang = ["es", "ca", "en"].includes(lang) ? lang : "es";
  document.documentElement.lang = state.lang;
  if (state.snapshot) {
    renderGames(state.snapshot);
    renderApps(state.snapshot);
    renderHf8(state.snapshot);
    renderUsbAccess(state.snapshot);
    renderMessages(state.snapshot);
    renderDaemonGames();
  }
  if (state.report) renderDoctor(state.report);
  if (state.config) renderOptions({ config: state.config });
  renderDisplayConfig();
  translateDom(document);
}

const esc = (value) =>
  String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

const base = (path) => {
  if (!path) return "";
  const parts = String(path).split("/").filter(Boolean);
  return parts.length ? parts[parts.length - 1] : String(path);
};

/* ------------------------------------------------------------------ toast */

let toastTimer;
function toast(message, kind = "") {
  const node = $("#toast");
  node.textContent = translateValue(message);
  node.className = `toast show ${kind}`;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    node.className = "toast";
  }, kind === "error" ? 6000 : 3200);
}

function fail(error) {
  const text = typeof error === "string" ? error : error?.message ?? String(error);
  toast(text, "error");
}

async function call(command, args) {
  if (!invoke) throw new Error("Esta página necesita abrirse desde la aplicación.");
  return invoke(command, args);
}

/* ------------------------------------------------------------------ modal */

function resetModal() {
  const body = $("#modal-body");
  const rich = $("#modal-rich");
  const actions = $("#modal-actions");
  body.hidden = true;
  body.textContent = "";
  rich.hidden = true;
  rich.innerHTML = "";
  actions.hidden = true;
  actions.innerHTML = "";
}

/**
 * Abre el modal compartido.
 * - `text`: contenido literal (scripts, entorno) en <pre>.
 * - `html`: contenido con formato (avisos y explicaciones).
 * - `actions`: botones {label, primary, onClick}.
 */
function openModal({ title, text, html, actions = [] }) {
  resetModal();
  $("#modal-title").textContent = title;

  if (html) {
    $("#modal-rich").innerHTML = html;
    $("#modal-rich").hidden = false;
  } else {
    $("#modal-body").textContent = text ?? "";
    $("#modal-body").hidden = false;
  }

  if (actions.length) {
    const box = $("#modal-actions");
    actions.forEach((action) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = action.primary ? "btn" : "btn ghost";
      button.textContent = action.label;
      button.addEventListener("click", action.onClick);
      box.appendChild(button);
    });
    box.hidden = false;
  }

  translateDom($("#modal"));
  const modal = $("#modal");
  if (!modal.open) modal.showModal();
  return modal;
}

/* ------------------------------------------------------------------ games */

const GAME_SETUP = {
  lmu: {
    html: `Copia al espejo los plugins de Le Mans Ultimate
      (<code>ChatTransceiver.dll</code> y el plugin de shared memory), activa el
      plugin de LMU en SimHub y deja CrewChief en LMU, tanto en el perfil como
      en el <code>user.config</code>.`,
  },
  ams2: {
    html: `Deja CrewChief en Automobilista 2 (perfil y
      <code>user.config</code>) y desactiva el plugin LMU de SimHub para que no
      interfiera con la telemetría de AMS2.`,
  },
  acevo: {
    html: `Deja CrewChief en Assetto Corsa EVO, añade <code>ASSETTO_EVO</code>
      a la lista de juegos disponibles y desactiva el plugin LMU de SimHub.`,
  },
  assetto: {
    html: `Instala la app de SimHub para rivales (<code>acpmf_simhub_v2</code>)
      y el plugin <code>CrewChiefEx</code>, enlaza la biblioteca de Steam de la
      botella y prepara el <code>python.ini</code> del juego.`,
  },
  rf2: {
    html: `Copia el plugin <code>rF2 Shared Memory Map</code> a
      <code>Bin64/Plugins</code>, enlaza la biblioteca de Steam de la botella,
      deja CrewChief en RF2 y activa el lector de RF2 en SimHub.`,
  },
  r3e: {
    html: `Deja CrewChief en RaceRoom (<code>RACE_ROOM</code>), añade
      <code>RACE_ROOM</code> a la lista de juegos y activa la entrada
      <code>RRRE</code> de SimHub. El puente copia el mapping <code>$R3E</code>
      al prefijo de Bottles; no toca el juego.`,
  },
  ets2: {
    html: `Instala el plugin SCS del juego y deja SimHub listo para leer la telemetría por el puente de memoria compartida. Es la versión nativa de Linux y no usa CrewChief.`,
  },
  beamng: {
    html: `Deja BeamNG con el protocolo OutGauge en 127.0.0.1:63392 y fija en SimHub el modo «Outgauge + motionsim».`,
    alert: `SimHub está abierto: ciérralo antes de aplicar. El modo de telemetría se escribe en su configuración y, con SimHub abierto, no se puede cambiar.`,
  },
};

function configGameDialog(gameId) {
  const game = (state.snapshot?.games ?? []).find((item) => item.id === gameId);
  const info = GAME_SETUP[gameId] ?? {
    html: "Aplica la configuración que necesita este simulador para SimHub y CrewChief.",
  };
  // Aviso en rojo solo si de verdad impide aplicar la configuracion.
  const simhubRunning = Boolean(state.snapshot?.apps?.simhub?.running);
  const alert =
    info.alert && simhubRunning
      ? `<p class="setup-alert">${esc(info.alert)}</p>`
      : "";
  openModal({
    title: `Configurar ${game?.name ?? gameId}`,
    html: `
      <p>${info.html}</p>${alert}
      <p class="setup-note">No arranca ni cierra el juego, el puente, SimHub ni
      CrewChief. Se puede repetir sin riesgo.</p>`,
    actions: [
      {
        label: "Aplicar configuración",
        primary: true,
        onClick: () => applyGameConfig(gameId),
      },
      { label: "Cancelar", onClick: () => $("#modal").close() },
    ],
  });
}

async function applyGameConfig(gameId) {
  const actions = $("#modal-actions");
  const rich = $("#modal-rich");
  actions.querySelectorAll("button").forEach((button) => {
    button.disabled = true;
  });
  rich.innerHTML = `<p class="setup-note">Aplicando configuración…</p>`;

  try {
    const result = await call("configure_game", { game: gameId });
    const badge = result.ok
      ? '<span class="chip ok">Configuración aplicada</span>'
      : '<span class="chip bad">Error</span>';
    const warnings = Array.isArray(result.warnings) ? result.warnings : [];
    const warningHtml = warnings
      .map((warning) => `<p class="setup-warning">${esc(warning)}</p>`)
      .join("");
    rich.innerHTML = `
      <p>${badge}</p>
      ${warningHtml}
      <pre class="config-output">${esc(result.output || "(sin salida)")}</pre>
      <p class="setup-note">${esc(result.script)}</p>`;

    actions.innerHTML = "";
    const close = document.createElement("button");
    close.type = "button";
    close.className = "btn";
    close.textContent = "Cerrar";
    close.addEventListener("click", () => $("#modal").close());
    actions.appendChild(close);

    if (result.ok) {
      toast("Configuración aplicada", "ok");
      await Promise.all([refreshStatus(true), refreshDoctor()]);
    } else {
      toast("No se pudo aplicar la configuración", "error");
    }
  } catch (error) {
    fail(error);
    actions.querySelectorAll("button").forEach((button) => {
      button.disabled = false;
    });
  }
}

function gameBadge(game) {
  if (game.bridge_running) return `<span class="state on"><span class="dot"></span>Puente activo</span>`;
  return `<span class="state"><span class="dot"></span>Parado</span>`;
}

function renderGames(snapshot) {
  const container = $("#games");
  const installed = snapshot.games.filter((game) => game.installed);

  if (!installed.length) {
    container.innerHTML = `
      <div class="card empty-games">
        <p>No hay simuladores instalados.</p>
        <button class="btn" data-detect>Detectar simuladores</button>
      </div>`;
    translateDom(container);
    return;
  }

  const anyBridge = (snapshot.games ?? []).some((game) => game.bridge_running);

  container.innerHTML = installed
    .map((game) => {
      const running = game.bridge_running;
      const apps = snapshot.apps ?? {};
      // Cada juego declara que apps usa: ETS2 solo SimHub.
      const supported =
        Array.isArray(game.apps) && game.apps.length
          ? game.apps
          : ["crewchief", "simhub"];
      const usesCrew = supported.includes("crewchief");
      const usesSim = supported.includes("simhub");
      const native = Boolean(game.native);
      // Deja claro en el titulo que la ficha es para la version Linux nativa.
      const label = native ? `${game.name} (Linux)` : game.name;
      // Con un puente activo, solo esa ficha muestra las apps en rojo. Si no
      // hay ningun puente, las apps abiertas se muestran en rojo en todas para
      // poder cerrarlas desde cualquier ficha.
      const appScope = running || !anyBridge;
      const crewRunning = appScope && usesCrew && Boolean(apps.crewchief?.running);
      const simRunning = appScope && usesSim && Boolean(apps.simhub?.running);
      const proton = native
        ? "Linux nativo"
        : game.proton
        ? base(game.proton)
        : "Proton no detectado";

      const portClass = game.port_in_use ? (running ? "ok" : "warn") : "quiet";
      const portText = game.port_in_use ? (running ? "en uso" : "ocupado") : "libre";

      const prefixClass = game.prefix ? "ok" : "bad";
      const prefixText = game.prefix ? "listo" : "falta";

      let lockText = "—";
      let lockClass = "quiet";
      if (game.lock) {
        lockText = "activo";
        lockClass = "ok";
      }

      const pids = game.bridge_pids.length ? ` · pid ${game.bridge_pids.join(", ")}` : "";
      // Engranaje en rojo si el juego nunca se ha configurado o ETOL ve algo mal.
      const needsConfig =
        game.config_state === "bad" || game.config_state === "pending";

      const metas = [];
      if (game.port) {
        metas.push(`
            <div>
              <dt>Puerto</dt>
              <dd class="${portClass}" title="${game.port} ${portText}">${game.port}<br><span class="quiet">${portText}</span></dd>
            </div>`);
      }
      if (native) {
        if (game.plugin_ok === true || game.plugin_ok === false) {
          const pluginOk = game.plugin_ok === true;
          metas.push(`
            <div>
              <dt>Plugin</dt>
              <dd class="${pluginOk ? "ok" : "bad"}" title="${esc(game.exe ?? "")}">${pluginOk ? "listo" : "falta"}</dd>
            </div>`);
        } else {
          // Juego nativo sin plugin: la telemetria viaja por UDP.
          metas.push(`
            <div>
              <dt>Puente</dt>
              <dd class="ok">UDP</dd>
            </div>`);
        }
      } else {
        metas.push(`
            <div>
              <dt>Prefijo</dt>
              <dd class="${prefixClass}" title="${esc(game.prefix ?? "no detectado")}">${prefixText}</dd>
            </div>`);
      }
      metas.push(`
            <div>
              <dt>Arranque</dt>
              <dd class="${lockClass}" title="${esc(game.lock ?? "sin lock")}">${lockText}</dd>
            </div>`);

      return `
        <article class="game ${running ? "running" : ""}">
          <div class="game-head">
            <div class="game-title">
              <h3>${esc(label)}<button type="button" class="game-config ${needsConfig ? "warn" : ""}" data-config="${game.id}"
                title="Configurar ${esc(game.name)}" aria-label="Configurar ${esc(game.name)}">⚙</button></h3>
              <span class="sub">${esc(proton)}</span>
            </div>
            ${gameBadge(game)}
          </div>

          <dl class="meta">${metas.join("")}
          </dl>

          <div class="game-actions${usesSim && !usesCrew ? " solo-simhub" : ""}">
            <button class="btn ${running ? "danger" : "blue"}" data-game="${game.id}"
              data-mode="${running ? "stop" : "todo"}">
              ${running ? "Parar" : "Lanzar"}
            </button>
            ${
              usesCrew
                ? `<button class="btn app-btn ${crewRunning ? "running" : "blue"}"
              data-app="crewchief" data-game="${game.id}"
              title="${crewRunning ? "Cerrar CrewChief" : "Arrancar CrewChief"}">
              <span class="app-dot"></span>CrewChief
            </button>`
                : ""
            }
            ${
              usesSim
                ? `<button class="btn app-btn ${simRunning ? "running" : "blue"}"
              data-app="simhub" data-game="${game.id}"
              title="${simRunning ? "Cerrar SimHub" : "Arrancar SimHub"}">
              <span class="app-dot"></span>SimHub
            </button>`
                : ""
            }
          </div>
          <span class="path" style="display:${pids ? "block" : "none"}">${esc(pids)}</span>
        </article>`;
    })
    .join("");
  translateDom(container);
}

/* ------------------------------------------------------------------- apps */

function renderApps(snapshot) {
  const { apps } = snapshot;
  const origin = apps.bottle_origin ? ` · ${apps.bottle_origin}` : "";
  $("#bottle-tag").textContent = `${apps.bottle ?? "sin botella"}${origin}`;

  const rows = [
    { key: "simhub", label: "SimHub", status: apps.simhub },
    { key: "crewchief", label: "CrewChief", status: apps.crewchief },
  ];

  $("#apps").innerHTML = rows
    .map(({ key, label, status }) => {
      const pids = status.pids.length ? `pid ${status.pids.join(", ")}` : "parado";
      return `
        <div class="row">
          <span class="dot ${status.running ? "on" : ""}"></span>
          <div class="row-main">
            <div class="label">${label}</div>
            <div class="value">${esc(pids)}</div>
          </div>
          <button class="btn ghost" data-app="${key}" ${status.running ? "disabled" : ""}>
            ${status.running ? "En marcha" : "Abrir"}
          </button>
        </div>`;
    })
    .join("");

  const wineSource = apps.wine_source ? ` (${apps.wine_source})` : "";
  $("#apps-paths").innerHTML = `
    <span class="path" title="${esc(apps.bottle_path ?? "")}">botella: ${esc(apps.bottle_path ?? "no detectada")}</span>
    <span class="path" title="${esc(apps.wine ?? "")}">wine: ${esc(apps.wine ?? "no detectado")}${esc(wineSource)}</span>`;
  translateDom(document);
}

/* -------------------------------------------------------------------- hf8 */

function renderHf8(snapshot) {
  const hf8 = snapshot.hf8;

  if (hf8.usb_scan === false) {
    $("#hf8").innerHTML = `
      <div class="row">
        <span class="dot warn"></span>
        <div class="row-main">
          <div class="label">Escaneo USB desactivado</div>
          <div class="value">Actívalo en Opciones para buscar el HF8 y otros dispositivos.</div>
        </div>
      </div>`;
    translateDom($("#hf8"));
    return;
  }

  const toggle = $("#opt-hf8-enabled");
  toggle.checked = hf8.enabled;

  if (!hf8.enabled) {
    $("#hf8").innerHTML = `
      <div class="row">
        <span class="dot"></span>
        <div class="row-main">
          <div class="label">Desactivado</div>
          <div class="value">Actívalo para configurar el pad al arrancar</div>
        </div>
      </div>`;
    translateDom($("#hf8"));
    return;
  }

  const device = hf8.device;
  const deviceValue = device
    ? `${device}${hf8.device_name ? ` · ${hf8.device_name}` : ""}`
    : "no conectado";

  const comValue = hf8.com_target
    ? `${hf8.com_port} → ${hf8.com_target}${hf8.com_ok ? "" : " (incorrecto)"}`
    : `${hf8.com_port} → sin enlace`;

  const holders = hf8.holders.length ? hf8.holders.join(", ") : "nadie";
  const usbHint = snapshot.usb?.needs_fix
    ? `<div class="row">
        <span class="dot warn"></span>
        <div class="row-main">
          <div class="label">${esc(t("Acceso USB"))}</div>
          <div class="value">${esc(t("Revisa la tarjeta «Acceso a dispositivos USB»."))}</div>
        </div>
      </div>`
    : "";

  $("#hf8").innerHTML = `
    <div class="row">
      <span class="dot ${device ? "on" : "bad"}"></span>
      <div class="row-main">
        <div class="label">Dispositivo</div>
        <div class="value" title="${esc(deviceValue)}">${esc(deviceValue)}</div>
      </div>
    </div>
    <div class="row">
      <span class="dot ${hf8.com_ok ? "on" : "warn"}"></span>
      <div class="row-main">
        <div class="label">Puerto</div>
        <div class="value" title="${esc(comValue)}">${esc(comValue)}</div>
      </div>
    </div>
    <div class="row">
      <span class="dot"></span>
      <div class="row-main">
        <div class="label">Abierto por</div>
        <div class="value" title="${esc(holders)}">${esc(holders)}</div>
      </div>
    </div>
    ${usbHint}`;
  translateDom($("#hf8"));
}

/* ------------------------------------------------------ acceso usb */

function renderUsbAccess(snapshot) {
  const usb = snapshot?.usb;
  const box = $("#usb-access");
  if (!usb || !box) return;

  const tag = $("#usb-tag");
  if (tag) {
    tag.textContent = usb.needs_fix ? t("Revisar") : t("Correcto");
    tag.className = `tag ${usb.needs_fix ? "warn" : "ok"}`;
  }

  const groupValue = usb.serial_group
    ? usb.in_group
      ? tf("{0} · usuario incluido", usb.serial_group)
      : tf("{0} · falta añadir el usuario", usb.serial_group)
    : t("No se detectó el grupo serie (uucp/dialout)");

  const brlttyValue = !usb.brltty_present
    ? t("No instalado")
    : usb.brltty_active
      ? t("Activo (puede bloquear el puerto)")
      : usb.brltty_masked
        ? t("Enmascarado")
        : t("Instalado y sin enmascarar");

  const ruleValue = usb.rule_ok
    ? t("Instalada")
    : usb.rule_present
      ? t("Instalada pero incompleta")
      : t("No instalada");

  const devValue = usb.devices.length
    ? usb.devices
        .map((device) =>
          esc(
            `${device.dev}${device.name ? ` · ${device.name}` : ""}${
              device.accessible ? "" : ` (${t("sin acceso")})`
            }`
          )
        )
        .join("<br>")
    : esc(t("No hay puertos serie USB conectados"));

  const devicesOk = usb.devices.length
    ? usb.devices.every((device) => device.accessible)
    : usb.in_group || usb.rule_ok;

  const relogin = usb.needs_relogin
    ? `<div class="row">
        <span class="dot warn"></span>
        <div class="row-main">
          <div class="label">${esc(t("Sesión"))}</div>
          <div class="value">${esc(t("Cierra sesión o reinicia para aplicar el grupo."))}</div>
        </div>
      </div>`
    : "";

  box.innerHTML = `
    <div class="row">
      <span class="dot ${usb.in_group ? "on" : "warn"}"></span>
      <div class="row-main">
        <div class="label">${esc(t("Grupo serie"))}</div>
        <div class="value">${esc(groupValue)}</div>
      </div>
    </div>
    <div class="row">
      <span class="dot ${usb.brltty_present && (!usb.brltty_masked || usb.brltty_active) ? "warn" : "on"}"></span>
      <div class="row-main">
        <div class="label">${esc(t("brltty"))}</div>
        <div class="value">${esc(brlttyValue)}</div>
      </div>
    </div>
    <div class="row">
      <span class="dot ${usb.rule_ok ? "on" : ""}"></span>
      <div class="row-main">
        <div class="label">${esc(t("Regla udev"))}</div>
        <div class="value">${esc(ruleValue)}</div>
      </div>
    </div>
    <div class="row">
      <span class="dot ${devicesOk ? "on" : "warn"}"></span>
      <div class="row-main">
        <div class="label">${esc(t("Dispositivos serie"))}</div>
        <div class="value">${devValue}</div>
      </div>
    </div>
    ${relogin}`;
}

/* --------------------------------------------------------------- mensajes */

const BANNERS = {
  install: {
    kicker: "requisito",
    title: "Falta Bottles Flatpak",
  },
  user: {
    kicker: "requisito",
    title: "Bottles está instalado en modo sistema",
  },
  permissions: {
    kicker: "permisos",
    title: "Bottles no tiene los permisos necesarios",
  },
  library: {
    kicker: "permisos",
    title: "Nueva biblioteca de Steam: Bottles necesita permisos",
  },
  prefix: {
    kicker: "prefijo",
    title: "Falta la botella de SimHub y CrewChief",
  },
  device: {
    kicker: "dispositivo",
    title: "El HF8 necesita reconfigurarse",
  },
};

const SETUP_SCRIPTS = {
  install: "bottles-install",
  user: "bottles-user",
  permissions: "bottles-user",
  library: "bottles-user",
  prefix: "prefix",
};

/** Devuelve el aviso que debe sustituir al mensaje por defecto. */
function setupBanner(snapshot) {
  const setup = snapshot.setup ?? {};
  const flatpak = setup.flatpak ?? {};
  if (!flatpak.available || (!flatpak.user && !flatpak.system)) return "install";
  if (flatpak.needs_user_install) return "user";
  if (!flatpak.permissions_ok) {
    const paths = flatpak.missing_paths ?? [];
    const onlyPaths =
      paths.length &&
      !flatpak.missing_home &&
      !flatpak.missing_host_etc &&
      !flatpak.missing_devices;
    return onlyPaths ? "library" : "permissions";
  }
  if (!setup.bottle_ready) return "prefix";
  if (setup.device_needs_fix) return "device";
  return null;
}

function renderMessages(snapshot) {
  const box = $("#messages");
  if (!box) return;
  const kind = setupBanner(snapshot);
  const automatic =
    snapshot.daemon_enabled ?? (state.config?.daemon?.enabled === true);
  if (!kind) {
    box.innerHTML = automatic
      ? `
      <div class="message default">
        <div class="message-kicker">modo automático</div>
        <strong>Abre el simulador. ETOL hace el resto.</strong>
        <p>La vigilancia automática levanta el puente y las apps elegidas al
        abrir el juego, y al cerrarlo para el puente y cierra CrewChief y
        SimHub. Si pulsas «Parar», se respeta hasta que cierres y vuelvas a
        abrir ese juego. Se cambia en Opciones.</p>
      </div>`
      : `
      <div class="message default">
        <div class="message-kicker">modo manual</div>
        <strong>Primero el simulador; después CrewChief y SimHub.</strong>
        <p>Abre el simulador y pulsa «Lanzar» en su ficha: ETOL arranca el
        puente, CrewChief y SimHub. Si vas a cambiar de simulador, pulsa «Parar»
        en la ficha del anterior antes de lanzar el siguiente. La vigilancia
        automática se activa en Opciones.</p>
      </div>`;
    translateDom(box);
    return;
  }
  const info = BANNERS[kind];
  box.innerHTML = `
    <button type="button" class="message important" data-setup="${kind}"
      title="Pulsa para ver qué se va a hacer y por qué">
      <span class="message-kicker">${info.kicker}</span>
      <strong>${info.title}</strong>
      <span class="message-more">Ver detalles y arreglarlo →</span>
    </button>`;
  translateDom(box);
}

function missingPermissions(flatpak) {
  const items = [];
  if (flatpak.missing_home) items.push("home");
  if (flatpak.missing_host_etc) items.push("host-etc");
  if (flatpak.missing_devices) items.push("devices=all (pad HF8)");
  (flatpak.missing_paths ?? []).forEach((path) => items.push(path));
  return items;
}

function setupDialog(kind) {
  const setup = state.snapshot?.setup ?? {};
  const flatpak = setup.flatpak ?? {};
  const scriptKey = SETUP_SCRIPTS[kind];
  let title = "";
  let html = "";
  let accept = "Aceptar";

  if (kind === "install") {
    title = "Instalar Bottles Flatpak";
    accept = "Instalar Bottles";
    html = `
      <p><strong>ETOL necesita Bottles como Flatpak de usuario</strong> para
      ejecutar SimHub y CrewChief en un prefijo aislado, sin mezclarlo con el
      resto del sistema.</p>
      <p>Se instalará <code>com.usebottles.bottles</code> desde flathub y se le
      darán solo estos permisos:</p>
      <ul>
        <li><code>home</code> y <code>host-etc</code>: lo normal para que Wine
        encuentre las fuentes y la configuración del sistema.</li>
        <li>Las bibliotecas de Steam que estén fuera del home (otros discos).</li>
        <li><code>devices=all</code>: para que el pad HF8 aparezca como
        <code>COM33</code>.</li>
      </ul>
      <p><strong>No se da acceso total al sistema</strong>: se quitan
      <code>host</code> y <code>host-os</code>.</p>
      <p class="setup-note">No se usa <code>sudo</code>. La instalación es para
      tu usuario y no toca Steam, los juegos ni sus prefijos.</p>`;
  } else if (kind === "user") {
    title = "Pasar Bottles a modo usuario";
    accept = "Instalar copia de usuario";
    html = `
      <p>Bottles está instalado <strong>para todo el sistema</strong> (hace falta
      ser administrador para actualizarlo). ETOL necesita la copia
      <strong>de usuario</strong> porque:</p>
      <ul>
        <li>se actualiza sin permisos de administrador,</li>
        <li>sus permisos de Steam y del HF8 se aplican solo a tu usuario,</li>
        <li>las botellas y los datos viven en
        <code>~/.var/app/com.usebottles.bottles</code>, así que la copia de
        usuario reutiliza todo lo que ya tienes.</li>
      </ul>
      <p>Se instalará la copia de usuario y se aplicarán los permisos. La copia
      de sistema <strong>no se borra</strong>: puedes quitarla cuando quieras
      con <code>sudo flatpak uninstall --system com.usebottles.bottles</code>.
      Tus botellas no se mueven.</p>`;
  } else if (kind === "library") {
    title = "Permisos para la nueva biblioteca de Steam";
    accept = "Aplicar permisos";
    const paths = (flatpak.missing_paths ?? [])
      .map((path) => `<li><code>${esc(path)}</code></li>`)
      .join("");
    html = `
      <p>Se ha detectado una nueva biblioteca de Steam:</p>
      <ul>${paths}</ul>
      <p>Bottles es Flatpak y necesita permiso para acceder a ella. Al aplicar
      los permisos también se revisan <code>home</code>, <code>host-etc</code>
      y <code>devices=all</code>.</p>
      <p class="setup-note">Después reinicia Bottles/SimHub para que vean la
      biblioteca. No se toca el prefijo ni la configuración de los juegos.</p>`;
  } else if (kind === "permissions") {
    title = "Aplicar permisos a Bottles";
    accept = "Aplicar permisos";
    const missing = missingPermissions(flatpak);
    html = `
      <p>Bottles es Flatpak, pero le faltan permisos para que ETOL pueda
      trabajar:</p>
      <ul>${missing.map((item) => `<li><code>${esc(item)}</code></li>`).join("")}</ul>
      <p>Se aplicarán con <code>flatpak override --user</code>: <code>home</code>,
      <code>host-etc</code>, las bibliotecas de Steam de otros discos y
      <code>devices=all</code> (pad HF8). Se quitan <code>host</code> y
      <code>host-os</code>.</p>
      <p class="setup-note">No se toca el prefijo ni la configuración de
      SimHub/CrewChief.</p>`;
  } else if (kind === "prefix") {
    title = "Crear la botella de SimHub y CrewChief";
    accept = "Crear prefijo e instalar";
    html = `
      <p>No existe la botella que ETOL usa para SimHub y CrewChief. El script
      hará, en este orden:</p>
      <ol>
        <li>Descargar el último <strong>GE-Proton</strong> de GitHub y
        registrarlo como runner de Bottles.</li>
        <li>Crear la botella <code>simhub</code> (64 bits, Windows 11).</li>
        <li>Instalar las dependencias: <code>dotnet48</code>,
        <code>vcrun2022</code>, <code>d3dcompiler_43</code>,
        <code>d3dx9</code> y fuentes.</li>
        <li>Descargar la última versión de <strong>CrewChief</strong> (MSI) y de
        <strong>SimHub</strong> (ZIP de GitHub) y abrir sus instaladores.</li>
      </ol>
      <div class="setup-callout">
        <strong>En los instaladores:</strong>
        <ul>
          <li><strong>CrewChief:</strong> al terminar, no marques
          «Run CrewChiefV4».</li>
          <li><strong>SimHub:</strong> desmarca <em>Install .NET Runtime</em>,
          <em>Install USB Display Drivers</em> e
          <em>Install Visual C++ Redistributables</em>. Al terminar, no lo
          ejecutes.</li>
        </ul>
        <p>Las dependencias ya las instala el script dentro del prefijo.</p>
      </div>
      <p class="setup-note">CrewChief se descarga del espejo oficial y puede ser
      una versión anterior. Ábrelo una vez al terminar y acepta la
      actualización antes de usarlo con los juegos.</p>
      <p class="setup-note">Puede tardar bastante: GE-Proton y SimHub son
      descargas grandes. No se toca ningún juego ni prefijo de Steam.</p>`;
  } else if (kind === "device") {
    title = "Reconfigurar el HF8";
    accept = "Reconfigurar HF8";
    html = `
      <p>${esc(setup.device_hint ?? "El pad está conectado, pero su puerto COM no apunta al dispositivo.")}</p>
      <p>Se volverá a crear el enlace <code>dosdevices/${esc(state.config?.hf8?.com_port ?? "COM33")}</code>
      hacia el dispositivo USB y se comprobará el registro del prefijo. No se
      escribe nada en el juego.</p>`;
  }

  const actions = [];
  if (scriptKey) {
    actions.push({
      label: "Ver script",
      onClick: () => previewScript(scriptKey, kind),
    });
  }
  if (kind === "device") {
    actions.push({
      label: "Ver Dispositivos",
      onClick: () => {
        $("#modal").close();
        showTab("dispositivos");
      },
    });
  }
  actions.push({
    label: accept,
    primary: true,
    onClick: () => (kind === "device" ? runDeviceFix() : runSetup(kind)),
  });

  openModal({ title, html, actions });
}

async function previewScript(key, backKind) {
  try {
    const data = await call("script_preview", { key });
    openModal({
      title: `script · ${data.path}`,
      text: data.content,
      actions: [
        {
          label: "Volver",
          onClick: () => setupDialog(backKind),
        },
      ],
    });
  } catch (error) {
    fail(error);
  }
}

const SETUP_TEXT = {
  install: {
    running: "Instalando Bottles Flatpak y aplicando permisos…",
    done: "Bottles Flatpak instalado.",
    toast: "Bottles Flatpak instalado",
  },
  user: {
    running: "Instalando la copia de usuario y aplicando permisos…",
    done: "Bottles queda en modo usuario.",
    toast: "Bottles pasado a modo usuario",
  },
  permissions: {
    running: "Aplicando permisos al Flatpak de Bottles…",
    done: "Permisos aplicados.",
    toast: "Permisos de Bottles aplicados",
  },
  library: {
    running: "Aplicando permisos para la nueva biblioteca…",
    done: "Permisos aplicados.",
    toast: "Permisos de Bottles aplicados",
  },
  prefix: {
    running: "Creando runner, botella, dependencias e instaladores…",
    done: "Botella preparada. Revisa los instaladores que se hayan abierto.",
    toast: "Prefijo preparado",
  },
};

async function runSetup(kind) {
  const key = SETUP_SCRIPTS[kind];
  const info = SETUP_TEXT[kind];
  openModal({
    title: BANNERS[kind].title,
    html: `<div class="setup-state" id="setup-state">${info.running}</div>
      <div class="setup-progress" aria-hidden="true"><span id="setup-bar"></span></div>
      <p class="setup-stage" id="setup-stage">Preparando…</p>
      <pre class="setup-log" id="setup-log" hidden></pre>`,
    actions: [{ label: "Cerrar", onClick: () => $("#modal").close() }],
  });
  const closeButton = $("#modal-actions").querySelector("button");
  if (closeButton) closeButton.disabled = true;
  const bar = $("#setup-bar");
  const stage = $("#setup-stage");

  // El script deja su avance en un fichero; se consulta mientras corre.
  const timer = setInterval(async () => {
    try {
      const progress = await call("setup_progress", { key });
      if (progress?.stage) stage.textContent = progress.stage;
      if (typeof progress?.percent === "number") {
        bar.style.width = `${progress.percent}%`;
      }
    } catch {
      // La ventana puede haberse cerrado o el comando haber terminado.
    }
  }, 900);

  try {
    const result = await call("run_setup_script", { key });
    clearInterval(timer);
    const log = $("#setup-log");
    log.textContent = result.output || "(sin salida)";
    log.hidden = false;
    const stateBox = $("#setup-state");
    stateBox.className = `setup-state ${result.ok ? "ok" : "bad"}`;
    stateBox.textContent = result.ok
      ? translateValue(info.done)
      : `El script terminó con errores${result.code != null ? ` (código ${result.code})` : ""}. Revisa la salida.`;
    if (result.ok) {
      bar.style.width = "100%";
      stage.textContent = translateValue(info.done);
    } else {
      stage.textContent = "El script se detuvo; revisa la salida de abajo.";
    }
    if (closeButton) closeButton.disabled = false;
    if (result.ok) {
      toast(info.toast, "ok");
      const payload = await call("detect");
      await applyBootstrap(payload);
    }
  } catch (error) {
    clearInterval(timer);
    const stateBox = $("#setup-state");
    if (stateBox) {
      stateBox.className = "setup-state bad";
      stateBox.textContent = String(error?.message ?? error);
    }
    if (stage) stage.textContent = "No se pudo ejecutar el script.";
    if (closeButton) closeButton.disabled = false;
    fail(error);
  }
}

async function runDeviceFix() {
  openModal({
    title: "Reconfigurar el HF8",
    html: `<div class="setup-state" id="setup-state">Recreando el enlace del pad…</div>
      <pre class="setup-log" id="setup-log" hidden></pre>`,
    actions: [{ label: "Cerrar", onClick: () => $("#modal").close() }],
  });
  const closeButton = $("#modal-actions").querySelector("button");
  if (closeButton) closeButton.disabled = true;
  try {
    const output = await call("configure_hf8");
    const log = $("#setup-log");
    log.textContent = output || "(sin salida)";
    log.hidden = false;
    const stateBox = $("#setup-state");
    stateBox.className = "setup-state ok";
    stateBox.textContent = "Pad reconfigurado.";
    if (closeButton) closeButton.disabled = false;
    toast(output || "Pad HF8 configurado", "ok");
    await Promise.all([refreshStatus(true), refreshDoctor()]);
  } catch (error) {
    const stateBox = $("#setup-state");
    if (stateBox) {
      stateBox.className = "setup-state bad";
      stateBox.textContent = String(error?.message ?? error);
    }
    if (closeButton) closeButton.disabled = false;
    fail(error);
  }
}

/* ----------------------------------------------------------------- doctor */

function renderDoctor(report) {
  const summary = $("#doctor-summary");
  const chips = [];
  if (report.failures) chips.push(`<span class="chip bad">${report.failures} fallo${report.failures === 1 ? "" : "s"}</span>`);
  if (report.warnings) chips.push(`<span class="chip warn">${report.warnings} aviso${report.warnings === 1 ? "" : "s"}</span>`);
  if (!report.failures && !report.warnings) chips.push(`<span class="chip ok">todo correcto</span>`);
  summary.innerHTML = chips.join("");
  translateDom(summary);

  $("#checks").innerHTML = report.checks
    .map(
      (check) => `
      <li class="check ${check.level}">
        <span class="mark"></span>
        <div class="check-main">
          <div class="check-title">${esc(check.title)}</div>
          <div class="check-detail">${esc(check.detail)}</div>
          ${check.fix ? `<code class="check-fix">${esc(check.fix)}</code>` : ""}
        </div>
      </li>`
    )
    .join("");
}

/* --------------------------------------------------------- puerto 8888 */

const PORT_8888 = 8888;

async function portCheckDialog() {
  try {
    const plan = await call("port_check_plan", { port: PORT_8888 });
    openModal({
      title: "Comprobar el puerto 8888",
      html: `
        <p>SimHub usa TCP <code>${PORT_8888}</code> para los dashboards por
        wifi. El doctor no puede leer las reglas del firewall sin permisos, así
        que esta comprobación se hace en una terminal con <code>sudo</code>.</p>
        <p>Comando que se ejecutará:</p>
        <pre class="config-output">${esc(plan.command)}</pre>
        <p>Comprobaciones internas:</p>
        <pre class="config-output">${esc(plan.checks.join("\n"))}</pre>
        <p class="setup-note">Cuando cierres la terminal, pulsa «Leer resultado».
        Si el puerto no está abierto, se mostrará el comando para abrirlo.</p>`,
      actions: [
        {
          label: "Abrir terminal",
          primary: true,
          onClick: () => openPortCheck(),
        },
        { label: "Leer resultado", onClick: () => readPortResult() },
        { label: "Cerrar", onClick: () => $("#modal").close() },
      ],
    });
  } catch (error) {
    fail(error);
  }
}

async function openPortCheck() {
  try {
    const plan = await call("open_port_check", { port: PORT_8888 });
    $("#modal-rich").innerHTML = `
      <p class="setup-note">Se ha abierto <code>${esc(plan.terminal)}</code> con
      <code>${esc(plan.command)}</code>. Escribe tu contraseña si te la pide,
      espera el resultado y cierra la terminal. Después pulsa «Leer resultado».</p>`;
    toast("Terminal abierta: escribe tu contraseña y espera el resultado", "ok");
  } catch (error) {
    fail(error);
  }
}

async function readPortResult() {
  try {
    const result = await call("port_check_result", { port: PORT_8888 });
    if (!result) {
      toast("Todavía no hay resultado; termina la comprobación en la terminal", "error");
      return;
    }
    const chip = result.ok
      ? '<span class="chip ok">● puerto abierto</span>'
      : '<span class="chip bad">● puerto cerrado o bloqueado</span>';
    const fix = result.fix
      ? `<p>Para abrirlo:</p><pre class="config-output">${esc(result.fix)}</pre>`
      : "";
    $("#modal-rich").innerHTML = `
      <p>${chip}</p>
      <p>${esc(result.detail)}</p>${fix}`;
    toast(result.detail, result.ok ? "ok" : "error");
    await Promise.all([refreshStatus(true), refreshDoctor()]);
  } catch (error) {
    fail(error);
  }
}

async function refreshPortBadge() {
  const button = $("#btn-port");
  if (!button) return;
  try {
    const result = await call("port_check_result", { port: PORT_8888 });
    const wsCheck = (state.report?.checks ?? []).find(
      (check) => check.title === "SimHub WebSocket"
    );
    const listening = wsCheck ? wsCheck.level === "ok" : null;
    const warn = !result || !result.ok || listening === false;
    button.classList.toggle("warn", warn);
    button.title = !result
      ? "Aún no se ha comprobado el puerto 8888"
      : listening === false
        ? "SimHub no está escuchando en el puerto 8888"
        : result.ok
          ? "Puerto 8888 comprobado: abierto"
          : `Puerto 8888: ${result.detail}`;
  } catch {
    button.classList.add("warn");
  }
}

/* --------------------------------------------------------------- opciones */

function daemonGameList() {
  const snapshot = state.snapshot;
  if (snapshot?.games?.length) {
    return snapshot.games.filter((game) => game.installed);
  }
  const config = state.config ?? {};
  return Object.values(config.games ?? {}).filter((game) => game.installed);
}

function renderDaemonGames() {
  const container = $("#daemon-games");
  if (!container) return;

  const daemon = state.config?.daemon ?? {};
  const enabled = daemon.enabled === true;
  const row = $("#daemon-games-row");
  if (row) row.hidden = !enabled;
  container.classList.toggle("idle", !enabled);

  const games = daemonGameList();
  state.daemonGamesKey = games.map((game) => game.id).join("|");
  if (!enabled) return;

  if (!games.length) {
    container.innerHTML = `<p class="option-help">No hay simuladores instalados.</p>`;
    return;
  }

  container.innerHTML = games
    .map((game) => {
      const choice = (daemon.games ?? {})[game.id] ?? {};
      const checked = (value) => (value === false ? "" : " checked");
      const supported =
        Array.isArray(game.apps) && game.apps.length
          ? game.apps
          : ["crewchief", "simhub"];
      const crew = supported.includes("crewchief")
        ? `
          <label class="switch small" title="Abrir CrewChief con ${esc(game.name)}">
            <input type="checkbox" data-daemon-game="${esc(game.id)}" data-daemon-app="crewchief"${checked(choice.crewchief)}>
            <span class="switch-track"><span class="switch-thumb"></span></span>
            <span class="daemon-app-name">CrewChief</span>
          </label>`
        : "";
      const simhub = supported.includes("simhub")
        ? `
          <label class="switch small" title="Abrir SimHub con ${esc(game.name)}">
            <input type="checkbox" data-daemon-game="${esc(game.id)}" data-daemon-app="simhub"${checked(choice.simhub)}>
            <span class="switch-track"><span class="switch-thumb"></span></span>
            <span class="daemon-app-name">SimHub</span>
          </label>`
        : "";
      return `
        <div class="daemon-game">
          <span class="daemon-game-name">${esc(game.name)}</span>${crew}${simhub}
        </div>`;
    })
    .join("");
}

function renderOptions(payload = {}) {
  const cfg = payload.config ?? state.config ?? {};
  const logs = cfg.logs ?? {};
  const devices = cfg.devices ?? {};
  const daemon = cfg.daemon ?? {};
  const overlays = cfg.overlays ?? {};
  const ui = cfg.ui ?? {};

  $("#opt-logs-enabled").checked = logs.enabled !== false;
  document.querySelectorAll("[data-retention]").forEach((button) => {
    button.classList.toggle(
      "active",
      button.dataset.retention === String(logs.retention_days ?? 7)
    );
  });
  document.querySelectorAll("#opt-language [data-language]").forEach((button) => {
    button.classList.toggle(
      "active",
      button.dataset.language === String(ui.language ?? "auto")
    );
  });
  $("#opt-scan-usb").checked = devices.scan_usb !== false;
  $("#opt-daemon-enabled").checked = daemon.enabled === true;
  $("#opt-kwin-overlay").checked = overlays.kwin !== false;
  renderDaemonGames();

  const path = payload.logs_path ?? state.logsPath;
  state.logsPath = path;
  $("#opt-logs-path").textContent = path || "—";
  $("#opt-logs-path").title = path || "";
  translateDom(document);
}

function showTab(name) {
  const tabs = ["juegos", "aplicaciones", "dispositivos", "diagnostico", "opciones"];
  const tab = tabs.includes(name) ? name : "juegos";
  document.querySelectorAll("[data-tab]").forEach((button) => {
    button.classList.toggle("active", button.dataset.tab === tab);
  });
  document.querySelectorAll("[data-panel]").forEach((panel) => {
    panel.hidden = panel.dataset.panel !== tab;
  });
  try {
    localStorage.setItem("etol.tab", tab);
  } catch (_) {
    // sin almacenamiento local: seguimos igual
  }
}

/* --------------------------------------------------------------- refresco */

function renderPaths() {
  $("#path-config").textContent = `config: ${state.paths.config}`;
  $("#path-env").textContent = `entorno: ${state.paths.env}`;
  $("#path-config").title = state.paths.config;
  $("#path-env").title = state.paths.env;
}

async function refreshStatus(force = false) {
  try {
    const snapshot = await call("status");
    state.snapshot = snapshot;
    $("#clock").textContent = `actualizado ${new Date().toLocaleTimeString()}`;

    const key = JSON.stringify(snapshot);
    if (force || key !== state.statusKey) {
      state.statusKey = key;
      renderGames(snapshot);
      renderApps(snapshot);
      renderHf8(snapshot);
      renderUsbAccess(snapshot);
      renderMessages(snapshot);
      const ids = daemonGameList().map((game) => game.id).join("|");
      if (ids !== state.daemonGamesKey) renderDaemonGames();
    }
  } catch (error) {
    fail(error);
  }
}

async function refreshDoctor() {
  try {
    state.report = await call("doctor_report");
    renderDoctor(state.report);
    await refreshPortBadge();
  } catch (error) {
    fail(error);
  }
}

function showPayloadBanner(info) {
  const banner = $("#banner");
  if (!banner) return;
  if (info && !info.ok) {
    const fix = info.fix ? `<code>${esc(info.fix)}</code>` : "";
    banner.innerHTML = `<strong>No se pudo preparar el payload de scripts.</strong>
      <span>${esc(info.detail)}</span>${fix}`;
    banner.hidden = false;
  } else {
    banner.hidden = true;
    banner.innerHTML = "";
  }
  translateDom(banner);
}

async function applyBootstrap(payload) {
  state.config = payload.config;
  const version = payload.version ? `v${payload.version}` : "v…";
  const versionNode = document.getElementById("app-version");
  if (versionNode) versionNode.textContent = version;
  state.lang = payload.language || "es";
  state.paths = { config: payload.config_path, env: payload.env_path };
  state.logsPath = payload.logs_path ?? state.logsPath;
  showPayloadBanner(payload.payload);
  renderPaths();
  renderOptions(payload);
  setLanguage(state.lang);
  await Promise.all([refreshStatus(true), refreshDoctor()]);
}

/* --------------------------------------------------------------- acciones */

function busy(button, value) {
  if (!button) return;
  button.classList.toggle("busy", value);
}

async function launchGame(button, game, mode = "todo") {
  if (mode === "stop") {
    await stopGame(button, game);
    return;
  }

  busy(button, true);
  try {
    const noHf8 =
      !$("#opt-hf8").checked ||
      state.config?.hf8?.enabled === false ||
      state.config?.devices?.scan_usb === false;
    const result = await call("launch", { game, mode, noHf8 });
    toast(`Lanzando ${game.toUpperCase()} · ${mode} (pid ${result.pid})`, "ok");
    setTimeout(() => refreshStatus(true), 1200);
    setTimeout(() => refreshStatus(true), 6000);
    setTimeout(refreshDoctor, 6000);
  } catch (error) {
    fail(error);
  } finally {
    busy(button, false);
  }
}

/* Para el puente y cierra SimHub y CrewChief. */
async function stopGame(button, game) {
  busy(button, true);
  toast(`Parando ${game.toUpperCase()}, SimHub y CrewChief…`);
  try {
    const report = await call("stop", { game });
    const parts = [report.bridge.ok ? "puente parado" : "el puente no se pudo parar"];

    const closed = report.apps.filter((app) => app.closed).map((app) => app.app);
    const open = report.apps.filter((app) => !app.closed).map((app) => app.app);
    if (closed.length) parts.push(`${closed.join(" y ")} cerrado${closed.length > 1 ? "s" : ""}`);
    if (open.length) parts.push(`${open.join(" y ")} sigue${open.length > 1 ? "n" : ""} abierto${open.length > 1 ? "s" : ""}`);

    const ok = report.bridge.ok && !open.length;
    toast(`${game.toUpperCase()} · ${parts.join(" · ")}`, ok ? "ok" : "error");
    setTimeout(() => refreshStatus(true), 800);
    setTimeout(() => refreshStatus(true), 4000);
    setTimeout(refreshDoctor, 4000);
  } catch (error) {
    fail(error);
  } finally {
    busy(button, false);
  }
}

async function openApp(button, app) {
  busy(button, true);
  try {
    const result = await call("start_app", { app });
    toast(`${app === "simhub" ? "SimHub" : "CrewChief"} arrancando (pid ${result.pid})`, "ok");
    setTimeout(() => refreshStatus(true), 2500);
    setTimeout(() => refreshStatus(true), 9000);
  } catch (error) {
    fail(error);
  } finally {
    busy(button, false);
  }
}

function appRunningGlobal(app) {
  const apps = state.snapshot?.apps ?? {};
  return app === "simhub"
    ? Boolean(apps.simhub?.running)
    : Boolean(apps.crewchief?.running);
}

function gameBridgeRunning(gameId) {
  return Boolean(
    (state.snapshot?.games ?? []).find((game) => game.id === gameId)
      ?.bridge_running
  );
}

function activeGameName() {
  return (
    (state.snapshot?.games ?? []).find((game) => game.bridge_running)?.name ?? ""
  );
}

async function toggleApp(button, app) {
  const name = app === "simhub" ? "SimHub" : "CrewChief";
  const gameId = button.dataset.game;
  busy(button, true);
  try {
    // Lee el estado real antes de decidir: si la app acaba de arrancar o de
    // cerrarse, el snapshot de 4 s puede estar desfasado y daria "ya esta en
    // marcha" al intentar abrirla.
    await refreshStatus(true);
    const running = appRunningGlobal(app);
    const anyBridge = (state.snapshot?.games ?? []).some(
      (game) => game.bridge_running
    );
    const activeHere = running && (gameBridgeRunning(gameId) || !anyBridge);
    if (activeHere) {
      const report = await call("stop_app", { app });
      const hint = !anyBridge
        ? " · vuelve a pulsarlo para arrancarlo con el puente del juego"
        : "";
      toast(
        report.closed
          ? `${name}: cerrado${hint}`
          : `${name}: no se pudo cerrar · ${report.detail}`,
        report.closed ? "ok" : "error"
      );
    } else if (running) {
      const active = activeGameName();
      toast(
        `${name} ya está en marcha${active ? ` (${active})` : ""}; ciérralo desde su ficha roja o Aplicaciones`,
        ""
      );
      return;
    } else if (anyBridge && !gameBridgeRunning(gameId)) {
      const active = activeGameName();
      toast(
        `Hay un puente activo${active ? ` (${active})` : ""}; pulsa Parar en su ficha antes de abrir ${name} aquí`,
        "error"
      );
      return;
    } else {
      const result = await call("start_app_for_game", { game: gameId, app });
      toast(
        `${name} arrancando${
          result.bridge_started
            ? result.bridge_ok
              ? " y puente del juego"
              : " (el puente no arrancó)"
            : ""
        }`,
        result.bridge_ok ? "ok" : "error"
      );
    }
    setTimeout(() => refreshStatus(true), 1200);
    setTimeout(() => refreshStatus(true), 6000);
  } catch (error) {
    fail(error);
  } finally {
    busy(button, false);
  }
}

async function detectAll(button) {
  busy(button, true);
  toast("Escaneando Steam, Bottles y el pad…");
  try {
    const payload = await call("detect");
    await applyBootstrap(payload);
    toast("Detección actualizada", "ok");
  } catch (error) {
    fail(error);
  } finally {
    busy(button, false);
  }
}

/* --------------------------------------------------------------- eventos */

document.addEventListener("click", (event) => {
  const tabButton = event.target.closest("[data-tab]");
  if (tabButton) {
    showTab(tabButton.dataset.tab);
    return;
  }

  const detectButton = event.target.closest("[data-detect]");
  if (detectButton) {
    detectAll(detectButton);
    return;
  }

  const configButton = event.target.closest("[data-config]");
  if (configButton) {
    configGameDialog(configButton.dataset.config);
    return;
  }

  const appButton = event.target.closest("[data-app]");
  if (appButton) {
    if (appButton.dataset.game) {
      toggleApp(appButton, appButton.dataset.app);
    } else {
      openApp(appButton, appButton.dataset.app);
    }
    return;
  }

  const gameButton = event.target.closest("[data-game]");
  if (gameButton) {
    launchGame(gameButton, gameButton.dataset.game, gameButton.dataset.mode);
    return;
  }

  const setupButton = event.target.closest("[data-setup]");
  if (setupButton) {
    setupDialog(setupButton.dataset.setup);
    return;
  }

  const retentionButton = event.target.closest("[data-retention]");
  if (retentionButton) {
    const days = retentionButton.dataset.retention;
    saveOption("logs.retention_days", days, `Retención: ${days} día(s)`);
    return;
  }

  const openButton = event.target.closest("[data-open]");
  if (openButton) {
    call("open_path", { which: openButton.dataset.open })
      .then((path) => toast(`Abriendo ${path}`, "ok"))
      .catch(fail);
  }
});

$("#btn-refresh").addEventListener("click", async (event) => {
  busy(event.currentTarget, true);
  await Promise.all([refreshStatus(true), refreshDoctor()]);
  busy(event.currentTarget, false);
});

$("#btn-detect").addEventListener("click", (event) => detectAll(event.currentTarget));

$("#btn-doctor").addEventListener("click", async (event) => {
  busy(event.currentTarget, true);
  await refreshDoctor();
  busy(event.currentTarget, false);
});

$("#btn-port").addEventListener("click", () => portCheckDialog());

$("#btn-fix").addEventListener("click", async (event) => {
  busy(event.currentTarget, true);
  try {
    const removed = await call("fix_locks");
    toast(
      removed.length ? `Locks borrados: ${removed.length}` : "No había locks huérfanos",
      "ok"
    );
    await Promise.all([refreshStatus(true), refreshDoctor()]);
  } catch (error) {
    fail(error);
  } finally {
    busy(event.currentTarget, false);
  }
});

$("#btn-hf8").addEventListener("click", async (event) => {
  busy(event.currentTarget, true);
  try {
    const output = await call("configure_hf8");
    toast(output || "Pad HF8 configurado", "ok");
    await Promise.all([refreshStatus(true), refreshDoctor()]);
  } catch (error) {
    fail(error);
  } finally {
    busy(event.currentTarget, false);
  }
});

$("#btn-usb-fix").addEventListener("click", async (event) => {
  const button = event.currentTarget;
  busy(button, true);
  openModal({
    title: t("Preparar acceso USB"),
    html: `<div class="setup-state" id="setup-state">${esc(t("Pidiendo autorización…"))}</div>
      <pre class="setup-log" id="setup-log" hidden></pre>`,
    actions: [{ label: t("Cerrar"), onClick: () => $("#modal").close() }],
  });
  const closeButton = $("#modal-actions").querySelector("button");
  if (closeButton) closeButton.disabled = true;
  try {
    const output = await call("fix_usb_access");
    const log = $("#setup-log");
    log.textContent = output || "(sin salida)";
    log.hidden = false;
    const stateBox = $("#setup-state");
    stateBox.className = "setup-state ok";
    stateBox.textContent = t("Acceso USB preparado.");
    if (closeButton) closeButton.disabled = false;
    toast(t("Acceso USB preparado"), "ok");
    await Promise.all([refreshStatus(true), refreshDoctor()]);
  } catch (error) {
    const stateBox = $("#setup-state");
    if (stateBox) {
      stateBox.className = "setup-state bad";
      stateBox.textContent = String(error?.message ?? error);
    }
    if (closeButton) closeButton.disabled = false;
    fail(error);
  } finally {
    busy(button, false);
  }
});

$("#opt-hf8-enabled").addEventListener("change", async (event) => {
  const enabled = event.currentTarget.checked;
  try {
    const payload = await call("set_config", {
      key: "hf8.enabled",
      value: enabled ? "true" : "false",
    });
    state.config = payload.config;
    toast(enabled ? "HF8 activado" : "HF8 desactivado", "ok");
    await refreshStatus(true);
  } catch (error) {
    event.currentTarget.checked = !enabled;
    fail(error);
  }
});

async function saveOption(key, value, message) {
  try {
    const payload = await call("set_config", { key, value });
    state.config = payload.config;
    renderOptions(payload);
    toast(message, "ok");
    await refreshStatus(true);
    setTimeout(refreshDoctor, 600);
  } catch (error) {
    fail(error);
    renderOptions();
  }
}

$("#opt-logs-enabled").addEventListener("change", (event) => {
  const enabled = event.currentTarget.checked;
  saveOption("logs.enabled", enabled ? "true" : "false", enabled ? "Logs activados" : "Logs desactivados");
});

$("#opt-scan-usb").addEventListener("change", (event) => {
  const enabled = event.currentTarget.checked;
  saveOption(
    "devices.scan_usb",
    enabled ? "true" : "false",
    enabled ? "Búsqueda USB activada" : "Búsqueda USB desactivada"
  );
  setTimeout(() => refreshStatus(true), 300);
});

$("#opt-daemon-enabled").addEventListener("change", (event) => {
  const enabled = event.currentTarget.checked;
  saveOption(
    "daemon.enabled",
    enabled ? "true" : "false",
    enabled ? "Vigilancia automática activada" : "Vigilancia automática desactivada"
  );
});

$("#opt-kwin-overlay").addEventListener("change", (event) => {
  const enabled = event.currentTarget.checked;
  saveOption(
    "overlays.kwin",
    enabled ? "true" : "false",
    enabled ? "Dashboards sobre el juego activados" : "Dashboards sobre el juego desactivados"
  );
});

$("#opt-language").addEventListener("click", async (event) => {
  const button = event.target.closest("[data-language]");
  if (!button) return;
  const language = button.dataset.language;
  try {
    const payload = await call("set_config", {
      key: "ui.language",
      value: language,
    });
    state.config = payload.config;
    setLanguage(payload.language || "es");
    toast(t("Idioma actualizado"), "ok");
    await Promise.all([refreshStatus(true), refreshDoctor()]);
  } catch (error) {
    fail(error);
  }
});

$("#daemon-games").addEventListener("change", async (event) => {
  const input = event.target.closest("input[data-daemon-game]");
  if (!input) return;
  const game = input.dataset.daemonGame;
  const app = input.dataset.daemonApp;
  const value = input.checked;
  try {
    const payload = await call("set_config", {
      key: `daemon.games.${game}.${app}`,
      value: value ? "true" : "false",
    });
    state.config = payload.config;
    renderDaemonGames();
    toast(
      `${app === "crewchief" ? "CrewChief" : "SimHub"} ${value ? "activado" : "desactivado"} para ${game}`,
      "ok"
    );
  } catch (error) {
    input.checked = !value;
    fail(error);
  }
});

$("#btn-env").addEventListener("click", async () => {
  try {
    const text = await call("env_preview");
    openModal({ title: "generated/entorno.sh", text });
  } catch (error) {
    fail(error);
  }
});

$("#modal-close").addEventListener("click", () => $("#modal").close());

$("#modal").addEventListener("click", (event) => {
  if (event.target === $("#modal")) $("#modal").close();
});

/* ------------------------------------------------ configuración de pantallas */

const DISPLAY_RESOLUTIONS = {
  "1080p": { width: 1920, height: 1080 },
  "1440p": { width: 2560, height: 1440 },
  "4K": { width: 3840, height: 2160 },
};

const GAMESCOPE_EXTRAS = [
  { id: "grab", flag: "--force-grab-cursor" },
  { id: "mangoapp", flag: "--mangoapp", nativeEnv: "MANGOHUD=1" },
  { id: "forcefs", flag: "--force-windows-fullscreen" },
  { id: "rt", flag: "--rt" },
  { id: "itm", flag: "--hdr-itm-enabled" },
  { id: "fsr4", env: "PROTON_FSR4_UPGRADE=1" },
  { id: "antilag", env: "ENABLE_LAYER_MESA_ANTI_LAG=1" },
  { id: "fsr", flag: "-F fsr --sharpness 5" },
  { id: "radv", env: "AMD_VULKAN_ICD=RADV" },
  { id: "nvapi", env: "PROTON_ENABLE_NVAPI=1" },
  { id: "dlss", env: "PROTON_DLSS_UPGRADE=1" },
  { id: "shadercache", env: "__GL_SHADER_DISK_CACHE=1" },
  { id: "shadercachekeep", env: "__GL_SHADER_DISK_CACHE_SKIP_CLEANUP=1" },
  { id: "nis", flag: "-F nis --sharpness 5" },
];
const GAMESCOPE_EXTRA_IDS = new Set(GAMESCOPE_EXTRAS.map((extra) => extra.id));

let displayMode = "gamescope";
let displayScreens = "1";
let displayResolution = "1440p";
let displayExtras = new Set();
try {
  const savedMode = localStorage.getItem("etol.display.mode");
  if (savedMode === "gamescope" || savedMode === "native") displayMode = savedMode;
  const savedScreens = localStorage.getItem("etol.display.screens");
  if (savedScreens === "1" || savedScreens === "3") displayScreens = savedScreens;
  const savedRes = localStorage.getItem("etol.display.resolution");
  if (DISPLAY_RESOLUTIONS[savedRes]) displayResolution = savedRes;
  if (!savedScreens) {
    const oldPreset = localStorage.getItem("etol.gamescope");
    if (oldPreset === "7680x1440") displayScreens = "3";
  }
  const savedExtras = JSON.parse(
    localStorage.getItem("etol.display.extras") ||
      localStorage.getItem("etol.gamescope.extras") ||
      "[]"
  );
  if (Array.isArray(savedExtras)) {
    displayExtras = new Set(savedExtras.filter((id) => GAMESCOPE_EXTRA_IDS.has(id)));
  }
} catch (_) {
  // sin almacenamiento local: valores por defecto
}

function displayResolutionData() {
  return DISPLAY_RESOLUTIONS[displayResolution] ?? DISPLAY_RESOLUTIONS["1440p"];
}

function displayWidth() {
  const res = displayResolutionData();
  return displayScreens === "3" ? res.width * 3 : res.width;
}

function displayHeight() {
  return displayResolutionData().height;
}

function displayExtrasChosen() {
  return GAMESCOPE_EXTRAS.filter((extra) => displayExtras.has(extra.id));
}

function displayCommand() {
  const envs = displayExtrasChosen().map((extra) => extra.env).filter(Boolean);
  // Extras que tienen equivalente fuera de gamescope (MangoHud: MANGOHUD=1).
  const nativeEnvs = displayExtrasChosen()
    .map((extra) => extra.nativeEnv)
    .filter(Boolean);
  if (displayMode === "native") {
    return [...envs, ...nativeEnvs, "%command%"].join(" ");
  }
  const flags = displayExtrasChosen().map((extra) => extra.flag).filter(Boolean);
  const width = displayWidth();
  const height = displayHeight();
  const base = [
    "gamescope",
    `-W ${width}`,
    `-H ${height}`,
    `-w ${width}`,
    `-h ${height}`,
    "-r 144",
    "-b",
    "--adaptive-sync",
    "--hdr-enabled",
  ];
  return [...envs, ...base, ...flags, "--", "%command%"].join(" ");
}

function displayNote() {
  const width = displayWidth();
  const height = displayHeight();
  if (displayMode === "native") {
    const target =
      displayScreens === "3"
        ? tf("el triple screen a {0}×{1}", width, height)
        : tf("la resolución {0}×{1}", width, height);
    return tf(
      "Sin gamescope: configura en el juego {0}. ETOL mantiene los dashboards encima con la regla de KWin.",
      target
    );
  }
  return t(
    "Gamescope con -b (nunca -f). Ajusta -r si tu monitor no es de 144 Hz. Para los dashboards, activa «Dashboards sobre el juego» en Opciones o usa esta línea como alternativa."
  );
}

function renderDisplayConfig() {
  document.querySelectorAll("#display-mode [data-mode]").forEach((button) => {
    button.classList.toggle("active", button.dataset.mode === displayMode);
  });
  document.querySelectorAll("#display-screens [data-screens]").forEach((button) => {
    button.classList.toggle("active", button.dataset.screens === displayScreens);
  });
  document.querySelectorAll("#display-resolution [data-resolution]").forEach((button) => {
    button.classList.toggle("active", button.dataset.resolution === displayResolution);
  });
  document.querySelectorAll("#display-config input[data-extra]").forEach((input) => {
    input.checked = displayExtras.has(input.dataset.extra);
  });
  const note = $("#display-note");
  if (note) note.textContent = displayNote();
  const command = $("#display-command");
  if (command) command.textContent = displayCommand();
  translateDom($("#display-config"));
}

function saveDisplayConfig() {
  try {
    localStorage.setItem("etol.display.mode", displayMode);
    localStorage.setItem("etol.display.screens", displayScreens);
    localStorage.setItem("etol.display.resolution", displayResolution);
    localStorage.setItem("etol.display.extras", JSON.stringify([...displayExtras]));
  } catch (_) {
    // sin almacenamiento local: vale para esta sesión
  }
}

function fallbackCopy(text) {
  const area = document.createElement("textarea");
  area.value = text;
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.appendChild(area);
  area.select();
  const ok = document.execCommand("copy");
  area.remove();
  if (!ok) throw new Error("No se pudo copiar el comando");
}

function copyText(text) {
  if (navigator.clipboard?.writeText) {
    return navigator.clipboard
      .writeText(text)
      .catch(() => Promise.resolve(fallbackCopy(text)));
  }
  return Promise.resolve(fallbackCopy(text));
}

renderDisplayConfig();

$("#display-mode").addEventListener("click", (event) => {
  const button = event.target.closest("[data-mode]");
  if (!button) return;
  displayMode = button.dataset.mode;
  saveDisplayConfig();
  renderDisplayConfig();
});

$("#display-screens").addEventListener("click", (event) => {
  const button = event.target.closest("[data-screens]");
  if (!button) return;
  displayScreens = button.dataset.screens;
  saveDisplayConfig();
  renderDisplayConfig();
});

$("#display-resolution").addEventListener("click", (event) => {
  const button = event.target.closest("[data-resolution]");
  if (!button) return;
  displayResolution = button.dataset.resolution;
  saveDisplayConfig();
  renderDisplayConfig();
});

$("#btn-display-config").addEventListener("click", () => {
  renderDisplayConfig();
  const dialog = $("#display-config");
  if (dialog && !dialog.open) dialog.showModal();
});

$("#btn-guide").addEventListener("click", async () => {
  try {
    await call("open_guide");
  } catch (error) {
    fail(error);
  }
});

$("#display-config-close").addEventListener("click", () => {
  $("#display-config").close();
});

$("#display-config").addEventListener("click", (event) => {
  if (event.target === $("#display-config")) $("#display-config").close();
});

$("#display-config").addEventListener("change", (event) => {
  const input = event.target.closest("input[data-extra]");
  if (!input) return;
  if (input.checked) displayExtras.add(input.dataset.extra);
  else displayExtras.delete(input.dataset.extra);
  saveDisplayConfig();
  renderDisplayConfig();
});

$("#display-reset").addEventListener("click", () => {
  displayMode = "gamescope";
  displayScreens = "1";
  displayResolution = "1440p";
  displayExtras.clear();
  saveDisplayConfig();
  renderDisplayConfig();
});

$("#btn-copy-display").addEventListener("click", async () => {
  try {
    await copyText(displayCommand());
    toast("Comando copiado", "ok");
  } catch (error) {
    fail(error);
  }
});

/* -------------------------------------------------------------- arranque */

try {
  showTab(localStorage.getItem("etol.tab") || "juegos");
} catch (_) {
  showTab("juegos");
}

(async () => {
  if (!invoke) {
    $("#games").innerHTML = `<div class="card">Abre esta ventana desde la aplicación (Tauri) para ver el estado.</div>`;
    return;
  }

  try {
    const payload = await call("bootstrap");
    await applyBootstrap(payload);
    if (payload.first_run) toast("Primera ejecución: configuración creada", "ok");
  } catch (error) {
    const text = typeof error === "string" ? error : error?.message ?? String(error);
    showPayloadBanner({ ok: false, detail: text, fix: null });
    fail(error);
  }
  // No sondear con la ventana oculta o minimizada en la bandeja.
  setInterval(() => {
    if (!document.hidden) refreshStatus();
  }, 4000);
  setInterval(() => {
    if (!document.hidden) refreshDoctor();
  }, 15000);
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) return;
    refreshStatus(true);
    refreshDoctor();
  });
})();
