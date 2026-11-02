#!/usr/bin/env python3
"""Reenvia la telemetria UDP de DiRT Rally 2.0 a SimHub y CrewChief.

El juego envia los paquetes a 127.0.0.1:20777. Este proceso los escucha y
manda una copia a cada aplicacion, en puertos distintos, para que ambas puedan
leer la misma telemetria sin pelearse por el socket del juego.
"""

from __future__ import annotations

import os
import signal
import socket
import sys
import time

LISTEN_HOST = os.environ.get("DIRT2_LISTEN_HOST", "127.0.0.1")
LISTEN_PORT = int(os.environ.get("DIRT2_LISTEN_PORT", "20777"))
SIMHUB_HOST = os.environ.get("DIRT2_SIMHUB_HOST", "127.0.0.1")
SIMHUB_PORT = int(os.environ.get("DIRT2_SIMHUB_PORT", "20778"))
CREWCHIEF_HOST = os.environ.get("DIRT2_CREWCHIEF_HOST", "127.0.0.1")
CREWCHIEF_PORT = int(os.environ.get("DIRT2_CREWCHIEF_PORT", "20779"))

TARGETS = [
    ("SimHub", (SIMHUB_HOST, SIMHUB_PORT)),
    ("CrewChief", (CREWCHIEF_HOST, CREWCHIEF_PORT)),
]

running = True


def log(message: str) -> None:
    print(f"{time.strftime('%H:%M:%S')} {message}", flush=True)


def stop(_signum, _frame) -> None:
    global running
    running = False


def main() -> int:
    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)

    listener = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        listener.bind((LISTEN_HOST, LISTEN_PORT))
    except OSError as exc:
        log(f"ERROR: no se pudo escuchar en {LISTEN_HOST}:{LISTEN_PORT}: {exc}")
        return 1
    listener.settimeout(0.5)

    sender = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    log(
        "relay listo: "
        f"{LISTEN_HOST}:{LISTEN_PORT} -> "
        + ", ".join(f"{name} {host}:{port}" for name, (host, port) in TARGETS)
    )

    packets = 0
    last_report = time.monotonic()
    while running:
        try:
            payload, _source = listener.recvfrom(65535)
        except socket.timeout:
            continue
        except OSError as exc:
            if running:
                log(f"AVISO: error leyendo UDP: {exc}")
            continue

        packets += 1
        for name, target in TARGETS:
            try:
                sender.sendto(payload, target)
            except OSError as exc:
                # El destino puede no estar escuchando todavia. No es fatal.
                if packets == 1:
                    log(f"AVISO: no se pudo enviar a {name} {target}: {exc}")

        now = time.monotonic()
        if packets == 1 or now - last_report >= 30:
            log(f"paquetes reenviados: {packets} ({len(payload)} bytes el ultimo)")
            last_report = now

    listener.close()
    sender.close()
    log(f"relay detenido ({packets} paquetes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
