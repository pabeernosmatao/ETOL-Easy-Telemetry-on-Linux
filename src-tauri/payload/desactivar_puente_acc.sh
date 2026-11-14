#!/usr/bin/env bash
# Para el espejo de Assetto Corsa Competizione. No cierra SimHub/CrewChief ni
# toca el juego: solo termina el receptor, el emisor, los stubs y el vigilante.
set -euo pipefail

RUN_DIR="${ACC_MIRROR_RUN_DIR:-/tmp/etol/acc_mirror}"

is_our_mirror() {
  local pid="$1" cmd
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "$cmd" == *"acc_shm_mirror.exe"* ]]
}

normalize() {
  printf '%s' "${1//\\//}"
}

stop_watchdog() {
  local pidfile="$RUN_DIR/watchdog.pid" pid cmd
  [[ -f "$pidfile" ]] || { echo "vigilante no estaba en marcha."; return 0; }
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  rm -f "$pidfile"
  [[ -n "$pid" ]] || return 0
  kill -0 "$pid" 2>/dev/null || { echo "vigilante no estaba en marcha."; return 0; }
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  if [[ "$cmd" != *"acc_bridge_watchdog.sh"* ]]; then
    echo "AVISO: el PID $pid del vigilante ya no es el vigilante; no lo toco." >&2
    return 0
  fi
  kill -TERM "$pid" 2>/dev/null || true
  for _ in $(seq 1 30); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  kill -KILL "$pid" 2>/dev/null || true
  echo "vigilante detenido."
}

stop_stubs() {
  local names=("acc.exe" "AC2-Win64-Shipping.exe") name pattern pid cmd found=""
  for name in "${names[@]}"; do
    case "$name" in
      AC2*) pattern='AC2-Win64-Shipping[.]exe' ;;
      *) pattern='acc[.]exe' ;;
    esac
    for pid in $(pgrep -f -- "$pattern" 2>/dev/null || true); do
      cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
      [[ "$(normalize "$cmd")" == *"$RUN_DIR/$name"* ]] || continue
      kill "$pid" 2>/dev/null || true
      found="1"
    done
  done
  if [[ -n "$found" ]]; then
    for _ in $(seq 1 30); do
      local alive=0
      for name in "${names[@]}"; do
        case "$name" in
          AC2*) pattern='AC2-Win64-Shipping[.]exe' ;;
          *) pattern='acc[.]exe' ;;
        esac
        for pid in $(pgrep -f -- "$pattern" 2>/dev/null || true); do
          cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
          [[ "$(normalize "$cmd")" == *"$RUN_DIR/$name"* ]] && alive=1
        done
      done
      [[ "$alive" == 0 ]] && break
      sleep 0.1
    done
    for name in "${names[@]}"; do
      case "$name" in
        AC2*) pattern='AC2-Win64-Shipping[.]exe' ;;
        *) pattern='acc[.]exe' ;;
      esac
      for pid in $(pgrep -f -- "$pattern" 2>/dev/null || true); do
        cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
        [[ "$(normalize "$cmd")" == *"$RUN_DIR/$name"* ]] && kill -9 "$pid" 2>/dev/null || true
      done
    done
    echo "stubs de deteccion detenidos."
  else
    echo "stubs de deteccion ya estaban parados."
  fi
  rm -f "$RUN_DIR"/stub-*.pid
}

stop_pidfile() {
  local name="$1" pidfile="$RUN_DIR/$1.pid" pid
  [[ -f "$pidfile" ]] || return 0
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  rm -f "$pidfile"
  [[ -n "$pid" ]] || return 0
  kill -0 "$pid" 2>/dev/null || return 0
  if ! is_our_mirror "$pid"; then
    echo "AVISO: el PID $pid de $name ya no es el espejo; no lo toco." >&2
    return 0
  fi
  kill -TERM -- "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null || true
  for _ in $(seq 1 20); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  kill -KILL -- "-$pid" 2>/dev/null || kill -KILL "$pid" 2>/dev/null || true
  echo "$name detenido."
}

stop_watchdog
stop_stubs
stop_pidfile receiver
stop_pidfile sender

# Limpieza de procesos huerfanos si se perdio el PID file.
for mode in receiver sender; do
  for pid in $(pgrep -f -- "acc_shm_mirror\\.exe --${mode}" 2>/dev/null || true); do
    if is_our_mirror "$pid"; then
      kill -TERM -- "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null || true
    fi
  done
done

sleep 0.5
if pgrep -f -- 'acc_shm_mirror\.exe' >/dev/null 2>&1; then
  echo "AVISO: aun queda algun acc_shm_mirror.exe; revisa pgrep -af acc_shm_mirror." >&2
else
  echo "Espejo, vigilante y stubs de Assetto Corsa Competizione detenidos."
fi
