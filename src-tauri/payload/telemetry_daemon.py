#!/usr/bin/env python3
"""Daemon de telemetria nativo para Linux.

Fase 1 (actual):
  - Lee la telemetria normalizada de SimHub por WebSocket (fuente temporal).
  - Lee sesion/standings/clima/estrategia de LMU por REST (6397).
  - Sirve un API local (HTTP + SSE) y las plantillas .djson de SimHub.

Fase 2 (pendiente):
  - Sustituir SimHub por un puente dentro del prefix de Proton que lea la
    memoria compartida de rFactor2/LMU y envie los datos al host.

Uso:
  python3 telemetry_daemon.py --port 8770
  python3 telemetry_daemon.py --no-simhub   # solo LMU REST
  python3 telemetry_daemon.py --no-lmu      # solo SimHub

No modifica el juego ni SimHub: solo lee.
"""
from __future__ import annotations

import argparse
import base64
import json
import math
import os
import queue
import socket
import struct
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

# ---------------------------------------------------------------------------
# Configuracion
# ---------------------------------------------------------------------------

ROOT = Path(__file__).resolve().parent
WEB = ROOT / "web"

_BOTTLE = os.environ.get("SIMHUB_BOTTLE", "").strip()
DEFAULT_SIMHUB_DIR = Path(
    os.environ.get(
        "SIMHUB_DIR",
        str(Path(_BOTTLE).expanduser() / "drive_c/Program Files (x86)/SimHub")
        if _BOTTLE
        else str(Path.home() / "SimHub"),
    )
)
DEFAULT_SIMHUB_WS = "ws://localhost:8888/api/GetGameDataUpdates"
DEFAULT_LMU_REST = "http://localhost:6397"
DEFAULT_SHM_UDP = "127.0.0.1:8771"

SIMHUB_DIR = DEFAULT_SIMHUB_DIR
DASH_DIR = SIMHUB_DIR / "DashTemplates"
DASH_FONTS = SIMHUB_DIR / "DashFonts"
IMAGE_LIBRARY = SIMHUB_DIR / "ImageLibrary"
WEB_IMG = SIMHUB_DIR / "Web" / "img"

# ---------------------------------------------------------------------------
# Estado compartido
# ---------------------------------------------------------------------------

LOCK = threading.Lock()
SUBSCRIBERS: set[queue.Queue] = set()
STOP = threading.Event()

STATE = {
    "meta": {
        "ts": None,
        "simhub_ts": 0.0,
        "lmu_ts": 0.0,
        "shm_ts": 0.0,
        "shm_seq": 0,
        "simhub_connected": False,
        "lmu_connected": False,
        "shm_connected": False,
        "source": "none",
    },
    "simhub": {},
    "simhub_raw": {},
    "simhub_map": {},
    "native": {},
    "native_simhub": {},
    "lmu": {
        "sessionInfo": {},
        "gameState": {},
        "standings": [],
        "weather": {},
        "strategy": {},
        "player": {},
    },
}


def log(msg: str) -> None:
    print(f"[{datetime.now().strftime('%H:%M:%S')}] {msg}", flush=True)


def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def _n(v, default: float = 0.0) -> float:
    try:
        f = float(v)
        return f if math.isfinite(f) else default
    except (TypeError, ValueError):
        return default


def _i(v, default: int = 0) -> int:
    try:
        return int(v)
    except (TypeError, ValueError):
        return default


def _s(v, default: str = "") -> str:
    return v if isinstance(v, str) else default


def _wheel_val(t: dict, idx: int, key: str, default: float = 0.0) -> float:
    wheels = t.get("wheels")
    if isinstance(wheels, list) and 0 <= idx < len(wheels) and isinstance(wheels[idx], dict):
        return _n(wheels[idx].get(key), default)
    return default


def _wheel_temp(t: dict, idx: int) -> float:
    wheels = t.get("wheels")
    if isinstance(wheels, list) and 0 <= idx < len(wheels) and isinstance(wheels[idx], dict):
        temp = wheels[idx].get("tempC")
        if isinstance(temp, list) and temp:
            vals = [_n(x, 0.0) for x in temp]
            # Centro si existe; si no, media.
            if len(vals) >= 2 and vals[1] > 0:
                return vals[1]
            vals = [x for x in vals if x > 0]
            return sum(vals) / len(vals) if vals else 0.0
    return 0.0


def normalize_shm(msg: dict) -> dict:
    """Convierte el snapshot del puente LMU SHM a propiedades tipo SimHub.

    El renderer .djson y el dashboard simple ya consumen estos nombres, así que
    el puente nativo puede sustituir a SimHub sin tocar los dashboards.
    """
    g = msg.get("game") if isinstance(msg.get("game"), dict) else {}
    p = msg.get("player") if isinstance(msg.get("player"), dict) else {}
    t = p.get("telemetry") if isinstance(p.get("telemetry"), dict) else {}

    max_rpm = _n(t.get("maxRpm"), 9000.0) or 9000.0
    redline = max_rpm * 0.95
    track_len = _n(g.get("lapDist"), 0.0)
    lap_dist = _n(p.get("lapDist"), 0.0)
    track_pct = (lap_dist / track_len) if track_len > 0 else 0.0
    current_lap = _n(t.get("currentLapTime"), 0.0)
    last_lap = _n(p.get("lastLap"), 0.0)
    best_lap = _n(p.get("bestLap"), 0.0)
    session_best = _n(g.get("sessionBestLap"), 0.0) or best_lap
    fuel_pct = _n(t.get("fuelPct"), 0.0)
    if fuel_pct <= 0 and _i(p.get("fuelFraction")) > 0:
        fuel_pct = (_i(p.get("fuelFraction")) / 255.0) * 100.0

    total_laps = _i(g.get("maxLaps"))
    if total_laps <= 0 or total_laps >= 10000:
        total_laps = 0

    gear_num = _i(t.get("gear"))
    gear_str = "R" if gear_num < 0 else ("N" if gear_num == 0 else str(gear_num))

    s = {
        "SpeedKmh": _n(t.get("speedKmh")),
        "Rpms": _n(t.get("rpm")),
        "Gear": gear_str,
        "MaxRpm": max_rpm,
        "CarSettings_MaxRPM": max_rpm,
        "CarSettings_RedLineRPM": redline,
        "Redline": redline,
        "Throttle": _n(t.get("throttle")) * 100.0,
        "Brake": _n(t.get("brake")) * 100.0,
        "Clutch": _n(t.get("clutch")) * 100.0,
        "Steering": _n(t.get("steering")) * 100.0,
        "Fuel": _n(t.get("fuel")),
        "FuelCapacity": _n(t.get("fuelCapacity")),
        "FuelPercent": fuel_pct,
        "FuelFraction": _i(p.get("fuelFraction")),
        "EngineWaterTemp": _n(t.get("waterTempC")),
        "EngineOilTemp": _n(t.get("oilTempC")),
        "BrakeBias": _n(t.get("frontBrakeBiasPct")),
        "TurboBoost": _n(t.get("turboBoostPressure")),
        "DeltaBest": _n(t.get("deltaBest")),
        "Battery": _n(t.get("batteryChargeFraction")) * 100.0,
        "SoC": _n(t.get("soc")),
        "VirtualEnergy": _n(t.get("virtualEnergy")),
        "Regen": _n(t.get("regen")),
        "TC": _i(t.get("tc")),
        "TCMax": _i(t.get("tcMax")),
        "ABS": _i(t.get("abs")),
        "ABSMax": _i(t.get("absMax")),
        "MotorMap": _i(t.get("motorMap")),
        "Migration": _i(t.get("migration")),
        "FrontAntiSway": _i(t.get("frontAntiSway")),
        "RearAntiSway": _i(t.get("rearAntiSway")),
        "TyreTemperatureFrontLeft": _wheel_temp(t, 0),
        "TyreTemperatureFrontRight": _wheel_temp(t, 1),
        "TyreTemperatureRearLeft": _wheel_temp(t, 2),
        "TyreTemperatureRearRight": _wheel_temp(t, 3),
        "TyreWearFrontLeft": _wheel_val(t, 0, "wear") * 100.0,
        "TyreWearFrontRight": _wheel_val(t, 1, "wear") * 100.0,
        "TyreWearRearLeft": _wheel_val(t, 2, "wear") * 100.0,
        "TyreWearRearRight": _wheel_val(t, 3, "wear") * 100.0,
        "TyreFlatFrontLeft": bool(_wheel_val(t, 0, "flat")),
        "TyreFlatFrontRight": bool(_wheel_val(t, 1, "flat")),
        "TyreFlatRearLeft": bool(_wheel_val(t, 2, "flat")),
        "TyreFlatRearRight": bool(_wheel_val(t, 3, "flat")),
        "TyrePressureFrontLeft": _wheel_val(t, 0, "pressureKpa"),
        "TyrePressureFrontRight": _wheel_val(t, 1, "pressureKpa"),
        "TyrePressureRearLeft": _wheel_val(t, 2, "pressureKpa"),
        "TyrePressureRearRight": _wheel_val(t, 3, "pressureKpa"),
        "BrakeTemperatureFrontLeft": _wheel_val(t, 0, "brakeTempC"),
        "BrakeTemperatureFrontRight": _wheel_val(t, 1, "brakeTempC"),
        "BrakeTemperatureRearLeft": _wheel_val(t, 2, "brakeTempC"),
        "BrakeTemperatureRearRight": _wheel_val(t, 3, "brakeTempC"),
        "TyreCompoundFront": _s(t.get("frontTireCompoundName")),
        "TyreCompoundRear": _s(t.get("rearTireCompoundName")),
        "Position": _i(p.get("place")),
        "CompletedLaps": _i(p.get("laps")),
        "CurrentLap": _i(p.get("laps")) + 1,
        "LapNumber": _i(t.get("lapNumber")),
        "TotalLaps": total_laps,
        "CurrentLapTime": current_lap,
        "LastLapTime": last_lap if last_lap > 0 else 0.0,
        "BestLapTime": best_lap if best_lap > 0 else 0.0,
        "SessionBestTime": session_best,
        "AllTimeBest": session_best,
        "LapTime": current_lap,
        "LapDistance": lap_dist,
        "TrackLength": track_len,
        "TrackPositionPercent": track_pct,
        "TrackName": _s(g.get("track")),
        "SessionTypeName": _s(g.get("sessionType")),
        "Session": _s(g.get("sessionType")),
        "CarName": _s(p.get("car")),
        "CarModel": _s(p.get("car")),
        "CarClass": _s(p.get("class")),
        "PlayerName": _s(p.get("name")) or _s(g.get("playerName")),
        "AirTemp": _n(g.get("ambientTemp")),
        "TrackTemp": _n(g.get("trackTemp")),
        "Rain": _n(g.get("raining")),
        "Wetness": _n(g.get("avgPathWetness")),
        "MaxGears": _i(t.get("maxGears")),
        "DRS": bool(p.get("drsState")),
        "Pit": bool(p.get("inPits")),
        "InPits": bool(p.get("inPits")),
        "Flag": _i(p.get("flag")),
        "Gap": _n(p.get("timeBehindLeader")),
        "Interval": _n(p.get("timeBehindNext")),
        "IsPlayer": True,
    }
    return s


def _light_state_locked() -> dict:
    lmu = STATE["lmu"]
    return {
        "meta": STATE["meta"],
        "simhub": STATE["simhub"],
        "lmu": {
            "sessionInfo": lmu.get("sessionInfo") or {},
            "gameState": lmu.get("gameState") or {},
            "player": lmu.get("player") or {},
        },
    }


def snapshot_light_json() -> bytes:
    with LOCK:
        return json.dumps(_light_state_locked(), separators=(",", ":")).encode()


def snapshot_full_json() -> bytes:
    with LOCK:
        return json.dumps(STATE, separators=(",", ":")).encode()


def broadcast_loop() -> None:
    """Envia el estado ligero a los clientes SSE a ~10 Hz."""
    while not STOP.is_set():
        time.sleep(0.1)
        with LOCK:
            if not SUBSCRIBERS:
                continue
            subs = list(SUBSCRIBERS)
            data = json.dumps(_light_state_locked(), separators=(",", ":")).encode()
        for q in subs:
            try:
                q.put_nowait(data)
            except queue.Full:
                try:
                    q.get_nowait()
                except queue.Empty:
                    pass
                try:
                    q.put_nowait(data)
                except queue.Full:
                    pass


# ---------------------------------------------------------------------------
# WebSocket cliente (sin dependencias)
# ---------------------------------------------------------------------------

def ws_connect(url: str, timeout: float = 6.0):
    u = urllib.parse.urlparse(url)
    host = u.hostname
    port = u.port or (443 if u.scheme == "wss" else 80)
    path = u.path or "/"
    if u.query:
        path += "?" + u.query
    s = socket.create_connection((host, port), timeout=timeout)
    if u.scheme == "wss":
        import ssl

        s = ssl.create_default_context().wrap_socket(s, server_hostname=host)
    key = base64.b64encode(os.urandom(16)).decode()
    req = (
        f"GET {path} HTTP/1.1\r\n"
        f"Host: {host}:{port}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\n"
        "Sec-WebSocket-Version: 13\r\n\r\n"
    )
    s.sendall(req.encode())
    buf = b""
    while b"\r\n\r\n" not in buf:
        chunk = s.recv(4096)
        if not chunk:
            raise RuntimeError("cerrado durante el handshake")
        buf += chunk
    head, rest = buf.split(b"\r\n\r\n", 1)
    status = head.split(b"\r\n")[0].decode(errors="replace")
    if "101" not in status:
        raise RuntimeError(f"handshake fallido: {status}")
    return s, rest


def _recv_exact(s, n: int, buf: bytes):
    while len(buf) < n:
        chunk = s.recv(65536)
        if not chunk:
            raise ConnectionError("conexion cerrada")
        buf += chunk
    return buf[:n], buf[n:]


def ws_read_frame(s, buf: bytes):
    hdr, buf = _recv_exact(s, 2, buf)
    b1, b2 = hdr[0], hdr[1]
    fin = b1 >> 7
    opcode = b1 & 0x0F
    masked = b2 >> 7
    ln = b2 & 0x7F
    if ln == 126:
        ext, buf = _recv_exact(s, 2, buf)
        ln = struct.unpack("!H", ext)[0]
    elif ln == 127:
        ext, buf = _recv_exact(s, 8, buf)
        ln = struct.unpack("!Q", ext)[0]
    mask = None
    if masked:
        mask, buf = _recv_exact(s, 4, buf)
    payload, buf = _recv_exact(s, ln, buf)
    if mask:
        payload = bytes(payload[i] ^ mask[i % 4] for i in range(len(payload)))
    return fin, opcode, payload, buf


def ws_send_frame(s, opcode: int, payload: bytes) -> None:
    mask = os.urandom(4)
    masked = bytes(payload[i] ^ mask[i % 4] for i in range(len(payload)))
    ln = len(payload)
    if ln < 126:
        hdr = bytes([0x80 | opcode, 0x80 | ln])
    elif ln < 65536:
        hdr = bytes([0x80 | opcode, 0x80 | 126]) + struct.pack("!H", ln)
    else:
        hdr = bytes([0x80 | opcode, 0x80 | 127]) + struct.pack("!Q", ln)
    s.sendall(hdr + mask + masked)


class SimHubClient(threading.Thread):
    """Lee /api/GetGameDataUpdates y guarda el ultimo NewData."""

    def __init__(self, url: str):
        super().__init__(daemon=True, name="simhub-ws")
        self.url = url
        self._last_error = None

    def run(self) -> None:
        while not STOP.is_set():
            try:
                s, buf = ws_connect(self.url)
                with LOCK:
                    STATE["meta"]["simhub_connected"] = True
                    STATE["meta"]["ts"] = _now_iso()
                log("SimHub WS conectado")
                self._last_error = None
                frag = b""
                while not STOP.is_set():
                    s.settimeout(5.0)
                    try:
                        fin, opcode, payload, buf = ws_read_frame(s, buf)
                    except socket.timeout:
                        continue
                    if opcode == 0x9:  # ping
                        ws_send_frame(s, 0xA, payload)
                        continue
                    if opcode == 0x8:  # close
                        break
                    if opcode == 0x1:
                        frag = payload
                        if not fin:
                            continue
                    elif opcode == 0x0:
                        frag += payload
                        if not fin:
                            continue
                    else:
                        continue
                    self._handle(frag)
                s.close()
            except Exception as exc:  # noqa: BLE001
                err = f"{type(exc).__name__}: {exc}"
                if err != self._last_error:
                    log(f"SimHub WS: {err}")
                    self._last_error = err
            finally:
                with LOCK:
                    STATE["meta"]["simhub_connected"] = False
            if not STOP.is_set():
                time.sleep(2.0)

    def _handle(self, raw: bytes) -> None:
        try:
            msg = json.loads(raw.decode("utf-8", "replace"))
        except json.JSONDecodeError:
            return
        if not isinstance(msg, dict):
            return
        now = time.time()
        with LOCK:
            if isinstance(msg.get("NewData"), dict):
                STATE["simhub_raw"] = msg["NewData"]
                if STATE["meta"].get("source") != "shm+lmu":
                    STATE["simhub"] = msg["NewData"]
                    STATE["meta"]["source"] = "simhub+lmu"
                STATE["meta"]["simhub_ts"] = now
                STATE["meta"]["ts"] = _now_iso()
            elif msg.get("type") == "mapLayout" or "MapLayout" in msg:
                STATE["simhub_map"] = msg


# ---------------------------------------------------------------------------
# LMU REST
# ---------------------------------------------------------------------------

def lmu_get(base: str, path: str, timeout: float = 3.0):
    url = base.rstrip("/") + "/" + path.lstrip("/")
    req = urllib.request.Request(url, headers={"Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read())


class LMUPoller(threading.Thread):
    """Sondea los endpoints REST de LMU (sesion, standings, clima, estrategia)."""

    def __init__(self, base: str):
        super().__init__(daemon=True, name="lmu-rest")
        self.base = base
        self._last_error = None

    def run(self) -> None:
        fast = [
            ("sessionInfo", "rest/watch/sessionInfo"),
            ("gameState", "rest/sessions/GetGameState"),
            ("standings", "rest/watch/standings"),
        ]
        slow = [
            ("weather", "rest/sessions/weather"),
            ("strategy", "rest/strategy/usage"),
        ]
        last_slow = 0.0
        while not STOP.is_set():
            ok = True
            try:
                for key, path in fast:
                    data = lmu_get(self.base, path)
                    with LOCK:
                        STATE["lmu"][key] = data
                if time.time() - last_slow > 5.0:
                    for key, path in slow:
                        try:
                            data = lmu_get(self.base, path)
                            with LOCK:
                                STATE["lmu"][key] = data
                        except Exception:  # noqa: BLE001
                            pass
                    last_slow = time.time()
                with LOCK:
                    standings = STATE["lmu"].get("standings") or []
                    player = next(
                        (
                            x
                            for x in standings
                            if isinstance(x, dict) and (x.get("player") or x.get("hasFocus"))
                        ),
                        None,
                    )
                    STATE["lmu"]["player"] = player or {}
                    STATE["meta"]["lmu_connected"] = True
                    STATE["meta"]["lmu_ts"] = time.time()
                    STATE["meta"]["ts"] = _now_iso()
                self._last_error = None
            except Exception as exc:  # noqa: BLE001
                ok = False
                err = f"{type(exc).__name__}: {exc}"
                if err != self._last_error:
                    log(f"LMU REST: {err}")
                    self._last_error = err
                with LOCK:
                    STATE["meta"]["lmu_connected"] = False
            time.sleep(1.0 if ok else 2.0)


# ---------------------------------------------------------------------------
# Puente LMU Shared Memory (UDP)
# ---------------------------------------------------------------------------

class ShmUdpListener(threading.Thread):
    """Escucha los snapshots JSON del puente LMU SHM en UDP."""

    def __init__(self, host: str, port: int):
        super().__init__(daemon=True, name="lmu-shm-udp")
        self.host = host
        self.port = port
        self._last_error = None
        self._bad = 0

    def run(self) -> None:
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 4 * 1024 * 1024)
        except OSError:
            pass
        try:
            sock.bind((self.host, self.port))
        except OSError as exc:
            log(f"SHM UDP: no se pudo abrir {self.host}:{self.port}: {exc}")
            sock.close()
            return
        sock.settimeout(1.0)
        log(f"SHM UDP escuchando en udp://{self.host}:{self.port}")
        while not STOP.is_set():
            try:
                data, _addr = sock.recvfrom(65535)
            except socket.timeout:
                continue
            except OSError as exc:
                if str(exc) != self._last_error:
                    log(f"SHM UDP: {exc}")
                    self._last_error = str(exc)
                continue
            try:
                msg = json.loads(data)
            except (json.JSONDecodeError, UnicodeDecodeError):
                self._bad += 1
                if self._bad == 1 or self._bad % 300 == 0:
                    log(f"SHM UDP: datagrama JSON inválido ({self._bad})")
                continue
            if not isinstance(msg, dict) or msg.get("type") != "lmu_shm":
                continue
            norm = normalize_shm(msg)
            now = time.time()
            with LOCK:
                STATE["native"] = msg
                STATE["native_simhub"] = norm
                STATE["simhub"] = norm
                STATE["meta"]["shm_ts"] = now
                STATE["meta"]["shm_seq"] = _i(msg.get("seq"))
                STATE["meta"]["shm_connected"] = True
                STATE["meta"]["source"] = "shm+lmu"
                STATE["meta"]["ts"] = _now_iso()
            self._last_error = None
        sock.close()


class ShmWatchdog(threading.Thread):
    """Si el puente deja de enviar, vuelve a SimHub como fuente temporal."""

    def __init__(self, timeout: float = 2.0):
        super().__init__(daemon=True, name="shm-watchdog")
        self.timeout = timeout

    def run(self) -> None:
        while not STOP.is_set():
            time.sleep(0.5)
            with LOCK:
                if STATE["meta"].get("source") != "shm+lmu":
                    continue
                if time.time() - STATE["meta"].get("shm_ts", 0.0) <= self.timeout:
                    continue
                STATE["meta"]["shm_connected"] = False
                raw = STATE.get("simhub_raw") or {}
                if raw:
                    STATE["simhub"] = raw
                    STATE["meta"]["source"] = "simhub+lmu"
                    log("SHM: sin datos; fallback a SimHub WS")
                else:
                    STATE["meta"]["source"] = "none"
                    log("SHM: sin datos y sin SimHub; estado congelado")


# ---------------------------------------------------------------------------
# HTTP + SSE
# ---------------------------------------------------------------------------

MIME = {
    ".html": "text/html; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".css": "text/css; charset=utf-8",
    ".json": "application/json; charset=utf-8",
    ".ttf": "font/ttf",
    ".otf": "font/otf",
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".svg": "image/svg+xml",
    ".webp": "image/webp",
}


class Handler(BaseHTTPRequestHandler):
    server_version = "SimHubLinux/0.1"

    def log_message(self, fmt, *args):  # silencio; usamos log() propio
        pass

    # -- helpers ---------------------------------------------------------
    def _send(self, status: int, body: bytes, ctype: str = "application/json; charset=utf-8"):
        self.send_response(status)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        try:
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def _json(self, obj, status: int = 200):
        self._send(status, json.dumps(obj, separators=(",", ":")).encode())

    def _error(self, status: int, msg: str):
        self._json({"error": msg}, status)

    def _file(self, path: Path):
        try:
            body = path.read_bytes()
        except OSError:
            self._error(404, "no encontrado")
            return
        self._send(200, body, MIME.get(path.suffix.lower(), "application/octet-stream"))

    # -- rutas -----------------------------------------------------------
    def do_GET(self):  # noqa: N802
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        qs = urllib.parse.parse_qs(parsed.query)

        if path == "/":
            self._file(WEB / "dash.html")
            return
        if path in ("/relative", "/relative.html"):
            self._file(WEB / "relative.html")
            return
        if path == "/djson":
            self._file(WEB / "djson.html")
            return
        if path in ("/djson.js", "/dash.js"):
            self._file(WEB / path.lstrip("/"))
            return
        if path == "/api/health":
            with LOCK:
                self._json(
                    {
                        "ok": True,
                        "simhub": STATE["meta"]["simhub_connected"],
                        "lmu": STATE["meta"]["lmu_connected"],
                        "shm": STATE["meta"]["shm_connected"],
                        "source": STATE["meta"]["source"],
                        "simhub_ts": STATE["meta"]["simhub_ts"],
                        "lmu_ts": STATE["meta"]["lmu_ts"],
                        "shm_ts": STATE["meta"]["shm_ts"],
                        "shm_seq": STATE["meta"]["shm_seq"],
                        "clients": len(SUBSCRIBERS),
                    }
                )
            return
        if path == "/api/light":
            self._send(200, snapshot_light_json())
            return
        if path == "/api/state":
            self._send(200, snapshot_full_json())
            return
        if path == "/api/native":
            with LOCK:
                self._json(STATE.get("native") or {})
            return
        if path == "/api/standings":
            with LOCK:
                native = STATE.get("native") or {}
                if STATE["meta"].get("shm_connected") and isinstance(native.get("vehicles"), list):
                    vehicles = sorted(
                        native["vehicles"],
                        key=lambda v: (
                            _i(v.get("place")) if _i(v.get("place")) > 0 else 9999,
                            _s(v.get("name")),
                        ),
                    )
                    self._json(vehicles)
                else:
                    self._json(STATE["lmu"].get("standings") or [])
            return
        if path == "/api/stream":
            self._stream()
            return
        if path == "/api/dashboards":
            self._dashboards()
            return
        if path == "/api/dashboard":
            self._dashboard(qs)
            return
        if path.startswith("/api/fonts/"):
            self._font(path)
            return
        if path == "/api/simhub-image":
            self._simhub_image(qs)
            return
        if path.startswith("/api/lmu/"):
            self._proxy_lmu(path, parsed.query)
            return

        self._error(404, "ruta desconocida")

    # -- SSE -------------------------------------------------------------
    def _stream(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        q: queue.Queue = queue.Queue(maxsize=4)
        with LOCK:
            SUBSCRIBERS.add(q)
        try:
            self.wfile.write(b"retry: 1000\n\n")
            self.wfile.write(b"data: " + snapshot_light_json() + b"\n\n")
            self.wfile.flush()
            while not STOP.is_set():
                try:
                    data = q.get(timeout=15)
                except queue.Empty:
                    self.wfile.write(b": ping\n\n")
                    self.wfile.flush()
                    continue
                self.wfile.write(b"data: " + data + b"\n\n")
                self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError, OSError):
            pass
        finally:
            with LOCK:
                SUBSCRIBERS.discard(q)

    # -- dashboards ------------------------------------------------------
    def _dashboards(self):
        out = []
        if DASH_DIR.is_dir():
            for p in DASH_DIR.rglob("*.djson"):
                try:
                    rel = p.relative_to(DASH_DIR).as_posix()
                    out.append(
                        {
                            "name": p.stem,
                            "path": rel,
                            "dir": p.parent.name,
                            "size": p.stat().st_size,
                        }
                    )
                except OSError:
                    continue
        out.sort(key=lambda x: (x["dir"].lower(), x["name"].lower()))
        self._json(out)

    def _dashboard(self, qs):
        rel = (qs.get("path") or [""])[0]
        if not rel:
            self._error(400, "falta path")
            return
        try:
            target = (DASH_DIR / rel).resolve()
            if not target.is_relative_to(DASH_DIR.resolve()) or not target.is_file():
                self._error(404, "dashboard no encontrado")
                return
            self._file(target)
        except OSError:
            self._error(404, "dashboard no encontrado")

    def _font(self, path: str):
        name = path.rsplit("/", 1)[-1]
        if not name.lower().endswith((".ttf", ".otf")):
            self._error(400, "fuente no valida")
            return
        try:
            target = (DASH_FONTS / name).resolve()
            if not target.is_relative_to(DASH_FONTS.resolve()) or not target.is_file():
                self._error(404, "fuente no encontrada")
                return
            self._file(target)
        except OSError:
            self._error(404, "fuente no encontrada")

    def _simhub_image(self, qs):
        name = (qs.get("name") or [""])[0]
        if not name:
            self._error(400, "falta name")
            return
        # Evita traversal.
        if "/" in name or "\\" in name or ".." in name:
            self._error(400, "nombre no valido")
            return
        candidates = []
        for base in (IMAGE_LIBRARY, WEB_IMG):
            if not base.is_dir():
                continue
            for ext in ("", ".png", ".jpg", ".jpeg", ".svg", ".webp"):
                candidates.append(base / (name + ext))
        for cand in candidates:
            try:
                if cand.is_file():
                    self._file(cand)
                    return
            except OSError:
                continue
        self._error(404, "imagen no encontrada")

    def _proxy_lmu(self, path: str, query: str):
        sub = path[len("/api/lmu/"):]
        if not sub.startswith(("rest/", "navigation/")):
            self._error(400, "ruta LMU no permitida")
            return
        url = DEFAULT_LMU_REST.rstrip("/") + "/" + sub
        if query:
            url += "?" + query
        try:
            req = urllib.request.Request(url, headers={"Accept": "application/json"})
            with urllib.request.urlopen(req, timeout=4) as resp:
                body = resp.read()
                ctype = resp.headers.get("Content-Type", "application/json")
                self._send(resp.status, body, ctype)
        except urllib.error.HTTPError as exc:
            self._error(exc.code, str(exc))
        except Exception as exc:  # noqa: BLE001
            self._error(502, f"LMU no disponible: {exc}")


class Server(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    global SIMHUB_DIR, DASH_DIR, DASH_FONTS, IMAGE_LIBRARY, WEB_IMG

    ap = argparse.ArgumentParser(description="Telemetria nativa LMU/SimHub para Linux")
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=8770)
    ap.add_argument("--simhub-dir", default=str(DEFAULT_SIMHUB_DIR))
    ap.add_argument("--simhub-ws", default=DEFAULT_SIMHUB_WS)
    ap.add_argument("--lmu-rest", default=DEFAULT_LMU_REST)
    ap.add_argument("--shm-udp", default=DEFAULT_SHM_UDP)
    ap.add_argument("--no-simhub", action="store_true")
    ap.add_argument("--no-lmu", action="store_true")
    ap.add_argument("--no-shm", action="store_true")
    args = ap.parse_args()

    shm_host, _, shm_port_s = args.shm_udp.rpartition(":")
    shm_port = int(shm_port_s) if shm_port_s.isdigit() else 8771
    shm_host = shm_host or "127.0.0.1"

    SIMHUB_DIR = Path(args.simhub_dir)
    DASH_DIR = SIMHUB_DIR / "DashTemplates"
    DASH_FONTS = SIMHUB_DIR / "DashFonts"
    IMAGE_LIBRARY = SIMHUB_DIR / "ImageLibrary"
    WEB_IMG = SIMHUB_DIR / "Web" / "img"

    threads = []
    if not args.no_simhub:
        threads.append(SimHubClient(args.simhub_ws))
    if not args.no_lmu:
        threads.append(LMUPoller(args.lmu_rest))
    if not args.no_shm:
        threads.append(ShmUdpListener(shm_host, shm_port))
        threads.append(ShmWatchdog())
    threads.append(threading.Thread(target=broadcast_loop, daemon=True, name="broadcast"))
    for t in threads:
        t.start()

    server = Server((args.host, args.port), Handler)
    log(f"API en http://{args.host}:{args.port}/  (djson: /djson)")
    log(f"SimHub dir: {SIMHUB_DIR}")
    log(f"SimHub WS: {args.simhub_ws}  LMU REST: {args.lmu_rest}  SHM UDP: {args.shm_udp}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        log("cerrando...")
    finally:
        STOP.set()
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    main()
