/* Renderer nativo de un subconjunto de Dash Studio (.djson).
 *
 * Soporta:
 *   - Layer / GroupItem (contenedores)
 *   - TextItem / OutlinedTextItem
 *   - RectangleItem / EllipseItem
 *   - GradientItem
 *   - LinearGaugeItem / VerticalLinearGaugeItem
 *   - ImageItem / ImageFromFileItem (best effort)
 *   - Behaviors: LastLapTime, SessionBestTime, LapTime, CurrentGear,
 *     Speed, RPM, LapPercentage
 *   - Bindings sencillos: Text, Visible, TextColor, Value/CurrentValue
 *
 * El estado llega por SSE (/api/stream) con:
 *   { meta, simhub: {...250 props...}, lmu: { sessionInfo, gameState, player } }
 */
(() => {
  'use strict';

  const stage = document.getElementById('stage');
  const sel = document.getElementById('dashSelect');
  const statusEl = document.getElementById('status');
  const hud = document.getElementById('hud');

  let dash = null;
  let state = { simhub: {}, lmu: {}, meta: {} };
  let baseW = 1280;
  let baseH = 720;
  let rendered = [];

  const fontMap = {
    'RussellSquare': "'RussellSquare', sans-serif",
    'Cousine': "'Cousine', monospace",
    'Arial': 'Arial, sans-serif',
    'Arial Narrow': "'Arial Narrow', Arial, sans-serif",
    'Droid Sans': "'Droid Sans', sans-serif",
    '0Arame Mono': "'Cousine', monospace",
    'Segoe UI': "'Segoe UI', system-ui, sans-serif",
  };

  const namedColors = {
    transparent: 'transparent',
    black: '#000000',
    white: '#ffffff',
    red: '#ff0000',
    green: '#00ff00',
    blue: '#0000ff',
    yellow: '#ffff00',
    orange: '#ffa500',
    gray: '#808080',
    grey: '#808080',
  };

  // ------------------------------------------------------------------
  // Utilidades
  // ------------------------------------------------------------------

  function shortType(item) {
    const t = item && item.$type ? String(item.$type) : '';
    return t.split(',')[0].split('.').pop();
  }

  function isContainer(item) {
    const t = shortType(item);
    return t === 'Layer' || t === 'GroupItem';
  }

  function clamp(v, a, b) {
    return Math.max(a, Math.min(b, v));
  }

  function px(v) {
    return (Number(v) || 0) + 'px';
  }

  function colorToCss(c) {
    if (c == null) return null;
    if (typeof c === 'object') return null;
    let s = String(c).trim();
    if (!s) return null;
    if (namedColors[s.toLowerCase()]) return namedColors[s.toLowerCase()];

    const m8 = s.match(/^#([0-9a-fA-F]{8})$/);
    if (m8) {
      const a = parseInt(m8[1].slice(0, 2), 16) / 255;
      const r = parseInt(m8[1].slice(2, 4), 16);
      const g = parseInt(m8[1].slice(4, 6), 16);
      const b = parseInt(m8[1].slice(6, 8), 16);
      if (a === 0) return 'transparent';
      return `rgba(${r},${g},${b},${a.toFixed(3)})`;
    }
    if (/^#[0-9a-fA-F]{6}$/.test(s)) return s;
    if (/^#[0-9a-fA-F]{3}$/.test(s)) return s;
    return s;
  }

  function getProp(name) {
    if (!name) return null;
    const s = state.simhub || {};
    const p = (state.lmu && state.lmu.player) || {};
    const si = (state.lmu && state.lmu.sessionInfo) || {};
    const gs = (state.lmu && state.lmu.gameState) || {};

    const aliases = {
      Speed: 'SpeedKmh',
      RPM: 'Rpms',
      MaxRPM: 'MaxRpm',
      SessionBestLapTime: 'BestLapTime',
      PlayerPosition: 'Position',
    };
    const k = aliases[name] || name;

    if (k === 'LapPercentage') {
      const v = s.TrackPositionPercent != null ? s.TrackPositionPercent : p.TrackPositionPercent;
      return v == null ? null : v * 100;
    }

    if (k in s) return s[k];
    if (k in p) return p[k];
    if (k in si) return si[k];
    if (k in gs) return gs[k];

    const cap = k.charAt(0).toUpperCase() + k.slice(1);
    if (cap in s) return s[cap];
    if (cap in p) return p[cap];

    const short = k.split('.').pop();
    if (short in s) return s[short];
    if (short in p) return p[short];
    return null;
  }

  function evalExpr(expr) {
    if (expr == null) return null;
    if (typeof expr !== 'string') return expr;
    let s = expr.trim();
    if (!s) return null;

    // Referencia directa: [DataCorePlugin.GameData.NewData.SpeedKmh]
    let m = s.match(/^\[(?:DataCorePlugin\.GameData\.(?:NewData\.)?)?([A-Za-z0-9_]+)\]$/);
    if (m) return getProp(m[1]);

    // Referencia directa: $prop('SpeedKmh')
    m = s.match(/^\$prop\(\s*['"]([^'"]+)['"]\s*\)$/);
    if (m) return getProp(m[1]);

    // Sustituye referencias por literales JSON
    s = s.replace(
      /\[(?:DataCorePlugin\.GameData\.(?:NewData\.)?)?([A-Za-z0-9_]+)\]/g,
      (_, k) => JSON.stringify(getProp(k))
    );
    s = s.replace(
      /\$prop\(\s*['"]([^'"]+)['"]\s*\)/g,
      (_, k) => JSON.stringify(getProp(k))
    );

    // Funciones tipo NCalc/SimHub
    s = s.replace(/\bif\s*\(/g, '__if(');
    s = s.replace(/\bisnull\s*\(/g, '__isnull(');
    s = s.replace(/\blcase\s*\(/g, '__lcase(');
    s = s.replace(/\bucase\s*\(/g, '__ucase(');
    s = s.replace(/\bblink\s*\(/g, '__blink(');

    try {
      const fn = new Function(
        '__if', '__isnull', '__lcase', '__ucase', '__blink',
        'return (' + s + ');'
      );
      return fn(
        (c, a, b) => (c ? a : b),
        (x) => x == null,
        (x) => String(x).toLowerCase(),
        (x) => String(x).toUpperCase(),
        () => 1
      );
    } catch (_) {
      return null;
    }
  }

  function parseTime(v) {
    if (v == null) return null;
    if (typeof v === 'number') return v;
    const s = String(v).trim();
    if (!s) return null;
    let m = s.match(/^(-)?(\d+):(\d+):(\d+(?:\.\d+)?)$/);
    if (m) {
      const sign = m[1] ? -1 : 1;
      return sign * (parseInt(m[2], 10) * 3600 + parseInt(m[3], 10) * 60 + parseFloat(m[4]));
    }
    m = s.match(/^(-)?(\d+):(\d+(?:\.\d+)?)$/);
    if (m) {
      const sign = m[1] ? -1 : 1;
      return sign * (parseInt(m[2], 10) * 60 + parseFloat(m[3]));
    }
    const f = parseFloat(s);
    return isNaN(f) ? null : f;
  }

  function formatTime(v, decimals, placeholder) {
    const t = parseTime(v);
    if (t == null || t <= 0) return placeholder || '-:--.---';
    const m = Math.floor(t / 60);
    const s = t - m * 60;
    const ss = s.toFixed(decimals == null ? 3 : decimals);
    const pad = s < 10 ? '0' : '';
    return `${m}:${pad}${ss}`;
  }

  function gearText(g) {
    if (g == null || g === '') return 'N';
    const s = String(g).trim();
    if (s === '0') return 'N';
    if (s === '-1') return 'R';
    return s;
  }

  function behaviorValue(item) {
    const b = item.Behavior;
    if (!b) return undefined;
    const type = String(b.$type || '').split(',')[0];
    const st = b.Settings || {};

    if (type.includes('TimespanText.Imp.LastLapTime')) {
      return formatTime(getProp('LastLapTime'), st.SecondsDecimals != null ? st.SecondsDecimals : 3, '-:--.---');
    }
    if (type.includes('TimespanText.Imp.SessionBestTime')) {
      const sb = getProp('SessionBestTime');
      const v = sb != null && Number(sb) > 0 ? sb : getProp('BestLapTime');
      return formatTime(v, st.SecondsDecimals != null ? st.SecondsDecimals : 3, '-:--.---');
    }
    if (type.includes('TimespanText.Imp.LapTime')) {
      return formatTime(getProp('CurrentLapTime'), st.SecondsDecimals != null ? st.SecondsDecimals : 1, '-:--.-');
    }
    if (type.includes('CurrentGearBehavior')) {
      return gearText(getProp('Gear'));
    }
    if (type.includes('DoubleText.Imp.Speed')) {
      const v = Number(getProp('SpeedKmh') != null ? getProp('SpeedKmh') : 0);
      let txt = v.toFixed(st.Decimals != null ? st.Decimals : 0);
      if (st.AppendUnit) txt += ' km/h';
      if (st.AlwaysAddSign && v > 0) txt = '+' + txt;
      return txt;
    }
    if (type.includes('Gauges.RPM')) {
      return Number(getProp('Rpms') != null ? getProp('Rpms') : 0);
    }
    if (type.includes('Gauges.LapPercentage')) {
      const v = getProp('TrackPositionPercent');
      return v == null ? 0 : v * 100;
    }
    return undefined;
  }

  function applyFormat(v, fmt, fallback) {
    if (v == null) return fallback == null ? '' : String(fallback);
    if (typeof v === 'number') {
      if (fmt && /^[0#.,]+$/.test(fmt)) {
        const dec = (fmt.split('.')[1] || '').replace(/[^0#]/g, '').length;
        return v.toFixed(dec);
      }
      return String(v);
    }
    return String(v);
  }

  function textFor(item) {
    const bv = behaviorValue(item);
    if (bv !== undefined) return String(bv);

    const bind = item.Bindings && item.Bindings.Text;
    if (bind) {
      const expr = bind.Formula && bind.Formula.Expression;
      if (expr) {
        let v = evalExpr(expr);
        if (v == null) v = item.Text;
        return applyFormat(v, bind.FormatString, item.Text);
      }
    }
    return item.Text == null ? '' : String(item.Text);
  }

  function visibleFromBindings(item) {
    const b = item.Bindings && item.Bindings.Visible;
    if (b && b.Formula && b.Formula.Expression) {
      const v = evalExpr(b.Formula.Expression);
      if (v === 0 || v === false || v === '0' || v == null) return false;
    }
    return true;
  }

  function hasPaint(item) {
    const bg = colorToCss(item.BackgroundColor);
    const b = item.BorderStyle || {};
    const border = (b.BorderTop || b.BorderRight || b.BorderBottom || b.BorderLeft);
    return (bg && bg !== 'transparent') || border;
  }

  function applyGeometry(el, item) {
    el.style.left = px(item.Left);
    el.style.top = px(item.Top);
    el.style.width = px(item.Width);
    el.style.height = px(item.Height);
  }

  function applyBorder(el, item) {
    const b = item.BorderStyle || {};
    const color = colorToCss(b.BorderColor || item.BorderColor) || '#ffffff';
    const t = b.BorderTop || 0;
    const r = b.BorderRight || 0;
    const bot = b.BorderBottom || 0;
    const l = b.BorderLeft || 0;
    if (t || r || bot || l) el.style.borderStyle = 'solid';
    if (t) el.style.borderTop = `${t}px solid ${color}`;
    if (r) el.style.borderRight = `${r}px solid ${color}`;
    if (bot) el.style.borderBottom = `${bot}px solid ${color}`;
    if (l) el.style.borderLeft = `${l}px solid ${color}`;

    const rad = b.CornerRadius;
    if (rad && typeof rad === 'string' && rad !== '0,0,0,0') {
      el.style.borderRadius = rad.split(',').map((x) => x.trim() + 'px').join(' ');
    } else {
      const rt = b.RadiusTopLeft || 0;
      const rr = b.RadiusTopRight || 0;
      const rb = b.RadiusBottomRight || 0;
      const rl = b.RadiusBottomLeft || 0;
      if (rt || rr || rb || rl) el.style.borderRadius = `${rt}px ${rr}px ${rb}px ${rl}px`;
    }
  }

  // ------------------------------------------------------------------
  // Construccion del DOM
  // ------------------------------------------------------------------

  function flattenAll(items, out, parentVisible, parentGeom) {
    const visibleParent = parentVisible !== false;
    for (const item of items || []) {
      if (!item || typeof item !== 'object') continue;

      const vis = visibleParent && item.Visible !== false && visibleFromBindings(item);
      const geom = {
        Left: item.Left != null ? item.Left : (parentGeom ? parentGeom.Left : 0),
        Top: item.Top != null ? item.Top : (parentGeom ? parentGeom.Top : 0),
        Width: item.Width != null ? item.Width : (parentGeom ? parentGeom.Width : 0),
        Height: item.Height != null ? item.Height : (parentGeom ? parentGeom.Height : 0),
      };
      const merged = Object.assign({}, item, geom);

      if (isContainer(item)) {
        if (vis && hasPaint(merged)) {
          out.push(Object.assign({}, merged, { __container: true }));
        }
        flattenAll(item.Childrens || item.Children || [], out, vis, geom);
      } else if (vis) {
        out.push(merged);
      }
    }
  }

  function zRank(item) {
    const t = shortType(item);
    if (t === 'GradientItem') return 0;
    if (t === 'RectangleItem' && Number(item.Width) >= baseW * 0.9 && Number(item.Height) >= baseH * 0.9) return 1;
    return 2;
  }

  function build() {
    stage.innerHTML = '';
    rendered = [];

    const screens = (dash && dash.Screens) || [];
    const hasItems = (s) => s && (s.Items || []).length > 0;

    // SimHub compone capas: fondo(s) + pantalla activa + foreground.
    const backgrounds = screens.filter((s) => s.IsBackgroundLayer && hasItems(s));
    const active =
      screens.find((s) => s.InGameScreen && hasItems(s)) ||
      screens.find((s) => s.IdleScreen && hasItems(s)) ||
      screens.find((s) => !s.IsBackgroundLayer && !s.IsForegroundLayer && !s.IsOverlayLayer && hasItems(s)) ||
      screens.find(hasItems);
    const foregrounds = screens.filter((s) => s.IsForegroundLayer && s !== active && hasItems(s));

    const chosen = [];
    for (const s of [...backgrounds, active, ...foregrounds]) {
      if (s && !chosen.includes(s)) chosen.push(s);
    }
    if (!chosen.length) return;

    const items = [];
    for (const s of chosen) flattenAll(s.Items || [], items, true, null);
    items.sort((a, b) => zRank(a) - zRank(b));

    for (const item of items) {
      const type = shortType(item);
      const el = document.createElement('div');
      el.className = 'item';
      el.dataset.type = type;
      stage.appendChild(el);

      const rec = { item, el, type, lastText: null, lastVis: null, fill: null, vertical: false };

      if (type === 'TextItem' || type === 'OutlinedTextItem') {
        renderText(rec);
      } else if (type === 'RectangleItem' || type === 'EllipseItem' || type === 'Layer' || type === 'GroupItem') {
        renderBox(rec);
      } else if (type === 'GradientItem') {
        renderGradient(rec);
      } else if (type === 'LinearGaugeItem' || type === 'VerticalLinearGaugeItem') {
        renderGauge(rec);
      } else if (type === 'ImageItem' || type === 'ImageFromFileItem') {
        renderImage(rec);
      } else {
        renderBox(rec);
      }

      rendered.push(rec);
    }

    updateAll();
  }

  function renderText(rec) {
    const { item, el } = rec;
    applyGeometry(el, item);
    el.style.display = 'flex';
    el.style.overflow = 'hidden';
    el.style.whiteSpace = 'pre';
    el.style.lineHeight = '1';

    const ha = Number(item.HorizontalAlignment != null ? item.HorizontalAlignment : 0);
    const va = Number(item.VerticalAlignment != null ? item.VerticalAlignment : 0);
    el.style.justifyContent = ha === 1 ? 'center' : ha === 2 ? 'flex-end' : 'flex-start';
    el.style.alignItems = va === 1 ? 'center' : va === 2 ? 'flex-end' : 'flex-start';
    el.style.textAlign = ha === 1 ? 'center' : ha === 2 ? 'right' : 'left';

    const p = item.TextPadding || {};
    el.style.padding = `${p.PaddingTop || 0}px ${p.PaddingRight || 0}px ${p.PaddingBottom || 0}px ${p.PaddingLeft || 0}px`;

    const font = fontMap[item.Font] || (item.Font ? `'${item.Font}', sans-serif` : 'sans-serif');
    el.style.fontFamily = font;
    el.style.fontSize = px(item.FontSize || 20);
    el.style.fontWeight = item.FontWeight || 'normal';
    el.style.fontStyle = item.FontStyle || 'normal';
    if (item.UseMonospacedText) el.style.fontVariantNumeric = 'tabular-nums';
    el.style.color = colorToCss(item.TextColor) || '#ffffff';

    const bg = colorToCss(item.BackgroundColor);
    if (bg && bg !== 'transparent') el.style.background = bg;
    applyBorder(el, item);
    if (item.Opacity != null) el.style.opacity = String(Number(item.Opacity) / 100);
  }

  function renderBox(rec) {
    const { item, el } = rec;
    applyGeometry(el, item);
    const bg = colorToCss(item.BackgroundColor);
    if (bg && bg !== 'transparent') el.style.background = bg;
    applyBorder(el, item);
    if (shortType(item) === 'EllipseItem') el.style.borderRadius = '50%';
    if (item.Opacity != null) el.style.opacity = String(Number(item.Opacity) / 100);
  }

  function renderGradient(rec) {
    const { item, el } = rec;
    applyGeometry(el, item);

    const brush = item.Color && item.Color.LinearGradientBrush;
    if (!brush) {
      el.style.background = colorToCss(item.BackgroundColor) || '#111';
      return;
    }

    const stopsRaw = brush['LinearGradientBrush.GradientStops'] && brush['LinearGradientBrush.GradientStops'].GradientStop;
    const stops = Array.isArray(stopsRaw) ? stopsRaw : (stopsRaw ? [stopsRaw] : []);
    const cssStops = stops.map((s) => {
      const c = colorToCss(s['@Color']) || '#000';
      const o = (parseFloat(s['@Offset'] || '0') * 100).toFixed(1);
      return `${c} ${o}%`;
    }).join(', ');

    const sp = String(brush['@StartPoint'] || '0.5,0').split(',').map(Number);
    const ep = String(brush['@EndPoint'] || '0.5,1').split(',').map(Number);
    const dx = (ep[0] || 0) - (sp[0] || 0);
    const dy = (ep[1] || 0) - (sp[1] || 0);
    const angle = Math.atan2(dy, dx) * 180 / Math.PI + 90;

    el.style.background = `linear-gradient(${angle.toFixed(1)}deg, ${cssStops || '#000, #333'})`;
    if (item.Opacity != null) el.style.opacity = String(Number(item.Opacity) / 100);
  }

  function renderGauge(rec) {
    const { item, el } = rec;
    applyGeometry(el, item);

    const bg = colorToCss(item.BackgroundColor);
    el.style.background = bg && bg !== 'transparent' ? bg : '#222';
    applyBorder(el, item);
    if (item.Opacity != null) el.style.opacity = String(Number(item.Opacity) / 100);

    const vertical = rec.type === 'VerticalLinearGaugeItem';
    const fill = document.createElement('div');
    fill.style.position = 'absolute';
    fill.style.background = colorToCss(item.GaugeColor) || '#ffffff';
    if (vertical) {
      fill.style.left = '0';
      fill.style.bottom = '0';
      fill.style.width = '100%';
      fill.style.height = '0%';
    } else {
      fill.style.left = '0';
      fill.style.top = '0';
      fill.style.height = '100%';
      fill.style.width = '0%';
    }
    el.appendChild(fill);
    rec.fill = fill;
    rec.vertical = vertical;
  }

  function renderImage(rec) {
    const { item, el } = rec;
    applyGeometry(el, item);
    let name = item.Image || item.ImagePath || '';
    if (!name) return;
    const img = document.createElement('img');
    img.style.width = '100%';
    img.style.height = '100%';
    img.style.objectFit = 'contain';
    if (String(name).startsWith('data:')) {
      img.src = name;
    } else {
      const base = String(name).replace(/\\/g, '/').split('/').pop();
      img.src = '/api/simhub-image?name=' + encodeURIComponent(base);
    }
    img.onerror = () => { img.style.display = 'none'; };
    el.appendChild(img);
  }

  // ------------------------------------------------------------------
  // Actualizacion por frame
  // ------------------------------------------------------------------

  function gaugePercent(rec) {
    const { item } = rec;
    const btype = String((item.Behavior && item.Behavior.$type) || '');

    if (btype.includes('Gauges.RPM')) {
      const rpm = Number(getProp('Rpms') != null ? getProp('Rpms') : 0);
      const max = Number(getProp('MaxRpm') != null ? getProp('MaxRpm') : 9000) || 9000;
      return clamp((rpm / max) * 100, 0, 100);
    }
    if (btype.includes('Gauges.LapPercentage')) {
      const v = getProp('TrackPositionPercent');
      return clamp((v == null ? 0 : v * 100), 0, 100);
    }

    let v = null;
    const bind = item.Bindings && (item.Bindings.Value || item.Bindings.CurrentValue);
    if (bind && bind.Formula && bind.Formula.Expression) v = evalExpr(bind.Formula.Expression);
    if (v == null) v = item.Value;

    const min = Number(item.Minimum != null ? item.Minimum : 0);
    const max = Number(item.Maximum != null ? item.Maximum : 100);
    const n = Number(v);
    if (!isFinite(n) || max === min) return 0;
    return clamp(((n - min) / (max - min)) * 100, 0, 100);
  }

  function updateAll() {
    for (const rec of rendered) {
      const { item, el, type } = rec;

      const vis = visibleFromBindings(item) && item.Visible !== false;
      if (rec.lastVis !== vis) {
        el.classList.toggle('hidden', !vis);
        rec.lastVis = vis;
      }
      if (!vis) continue;

      if (type === 'TextItem' || type === 'OutlinedTextItem') {
        const txt = textFor(item);
        if (txt !== rec.lastText) {
          el.textContent = txt;
          rec.lastText = txt;
        }

        const cbind = item.Bindings && item.Bindings.TextColor;
        if (cbind && cbind.Formula && cbind.Formula.Expression) {
          const c = colorToCss(evalExpr(cbind.Formula.Expression));
          if (c) el.style.color = c;
        }

        if (item.Behavior && String(item.Behavior.$type).includes('CurrentGearBehavior')) {
          const st = item.Behavior.Settings || {};
          const rpm = Number(getProp('Rpms') != null ? getProp('Rpms') : 0);
          const red = Number(getProp('CarSettings_RedLineRPM') || getProp('Redline') || getProp('MaxRpm') || 0);
          if (st.UseTextColorFlash && red > 0 && rpm >= red) {
            const on = Math.floor(Date.now() / (st.FlashInterval || 100)) % 2 === 0;
            el.style.color = colorToCss(on ? st.FlashTextColor : st.DefaultTextColor) || '#ffffff';
          } else if (st.DefaultTextColor) {
            el.style.color = colorToCss(st.DefaultTextColor) || '#ffffff';
          }
        }
      } else if (type === 'LinearGaugeItem' || type === 'VerticalLinearGaugeItem') {
        const pct = gaugePercent(rec);
        if (rec.vertical) rec.fill.style.height = pct + '%';
        else rec.fill.style.width = pct + '%';

        const btype = String((item.Behavior && item.Behavior.$type) || '');
        if (btype.includes('Gauges.RPM')) {
          const max = Number(getProp('MaxRpm') || 9000) || 9000;
          const red = Number(getProp('CarSettings_RedLineRPM') || getProp('Redline') || max) || max;
          const redPct = clamp((red / max) * 100, 0, 100);
          rec.fill.style.background =
            pct >= redPct ? '#ff2b2b' : pct >= redPct * 0.82 ? '#ffb300' : '#3ddc84';
        }
      }
    }
  }

  // ------------------------------------------------------------------
  // Escalado / carga
  // ------------------------------------------------------------------

  function fit() {
    const scale = Math.min(window.innerWidth / baseW, window.innerHeight / baseH);
    stage.style.transform = `scale(${scale})`;
    stage.style.left = ((window.innerWidth - baseW * scale) / 2) + 'px';
    stage.style.top = ((window.innerHeight - baseH * scale) / 2) + 'px';
  }

  function setStatus(msg) {
    if (msg) {
      statusEl.textContent = msg;
      return;
    }
    const m = state.meta || {};
    statusEl.textContent = `SimHub ${m.simhub_connected ? 'ON' : 'OFF'} · LMU ${m.lmu_connected ? 'ON' : 'OFF'}`;
  }

  async function loadDashboard(path) {
    setStatus('cargando…');
    try {
      const res = await fetch('/api/dashboard?path=' + encodeURIComponent(path));
      if (!res.ok) {
        setStatus('error cargando dashboard');
        return;
      }
      dash = await res.json();
      baseW = Number(dash.BaseWidth) || 1280;
      baseH = Number(dash.BaseHeight) || 720;
      stage.style.width = baseW + 'px';
      stage.style.height = baseH + 'px';
      const bg = colorToCss(dash.BackgroundColor);
      stage.style.background = bg && bg !== 'transparent' ? bg : '#000';
      build();
      fit();
      setStatus('');
      const u = new URL(location.href);
      u.searchParams.set('path', path);
      history.replaceState(null, '', u);
    } catch (e) {
      setStatus('error: ' + e.message);
    }
  }

  async function loadList() {
    try {
      const res = await fetch('/api/dashboards');
      const list = await res.json();
      sel.innerHTML = '';
      for (const d of list) {
        const o = document.createElement('option');
        o.value = d.path;
        o.textContent = `${d.dir} / ${d.name}`;
        sel.appendChild(o);
      }
      const q = new URLSearchParams(location.search).get('path');
      const def = 'SimHub - SimpleDash/SimHub - SimpleDash.djson';
      const pick = q || (list.find((x) => x.path === def) || {}).path || (list[0] || {}).path;
      if (pick) {
        sel.value = pick;
        loadDashboard(pick);
      }
      sel.onchange = () => loadDashboard(sel.value);
    } catch (e) {
      setStatus('error listando dashboards: ' + e.message);
    }
  }

  function connect() {
    const es = new EventSource('/api/stream');
    es.onopen = () => setStatus('');
    es.onmessage = (ev) => {
      try {
        state = JSON.parse(ev.data);
        updateAll();
        setStatus('');
      } catch (_) {
        // ignora frames corruptos
      }
    };
    es.onerror = () => setStatus('reconectando…');
  }

  window.addEventListener('resize', fit);
  window.addEventListener('keydown', (e) => {
    if (e.key === 'h' || e.key === 'H') hud.classList.toggle('hidden');
  });

  loadList();
  connect();
})();
