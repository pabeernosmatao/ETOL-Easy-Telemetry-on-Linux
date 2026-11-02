#!/usr/bin/env bash
# Comprueba si SimHub (bajo Wine) ve el pad HF8/ForceFeel.
#
# No abre el puerto ni toca SimHub: solo llama a
# ForceFeelAccess.GetDeviceSerialPort(), que recorre los dispositivos USB.
# Devuelve 0 si encuentra un COM y 1 si no.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BOTTLE="${SIMHUB_BOTTLE:-}"
if [[ -z "$BOTTLE" ]]; then
  echo "ERROR: falta SIMHUB_BOTTLE. Ejecuta: etol detect --save o etol config set apps.bottle <ruta>" >&2
  exit 1
fi
WINE="${BOTTLE_WINE:-}"
if [[ -z "$WINE" ]]; then
  echo "ERROR: falta BOTTLE_WINE. Ejecuta: etol detect --save o etol config set apps.wine <ruta>" >&2
  exit 1
fi
RUN_DIR="${LMU_MIRROR_RUN_DIR:-/tmp/etol/lmu_mirror}"

CSC_WIN='C:\windows\Microsoft.NET\Framework\v4.0.30319\csc.exe'
SRC="$RUN_DIR/check_hf8.cs"
EXE="$RUN_DIR/check_hf8.exe"

[[ -d "$BOTTLE" ]] || { echo "ERROR: no existe la botella $BOTTLE" >&2; exit 1; }
[[ -x "$WINE" ]] || { echo "ERROR: no existe wine de Bottles: $WINE" >&2; exit 1; }
[[ -f "$BOTTLE/drive_c/Program Files (x86)/SimHub/SimHub.Plugins.dll" ]] || {
  echo "ERROR: no encuentro SimHub.Plugins.dll en la botella." >&2
  exit 1
}

mkdir -p "$RUN_DIR"

if [[ ! -f "$SRC" || "$HERE/check_hf8.sh" -nt "$SRC" ]]; then
  cat > "$SRC" <<'CS'
using System;
using System.Reflection;

class CheckHf8
{
    static void P(string s) { Console.WriteLine(s); Console.Out.Flush(); }

    static void Main()
    {
        AppDomain.CurrentDomain.AssemblyResolve += (s, e) =>
        {
            string name = new AssemblyName(e.Name).Name;
            string p = System.IO.Path.Combine(@"C:\Program Files (x86)\SimHub", name + ".dll");
            return System.IO.File.Exists(p) ? Assembly.LoadFrom(p) : null;
        };
        try
        {
            Assembly a = Assembly.LoadFrom(@"C:\Program Files (x86)\SimHub\SimHub.Plugins.dll");
            Type t = null;
            Type[] all;
            try { all = a.GetTypes(); }
            catch (ReflectionTypeLoadException ex) { all = ex.Types; }
            foreach (Type tt in all)
            {
                if (tt != null && tt.Name.IndexOf("ForceFeelAccess", StringComparison.OrdinalIgnoreCase) >= 0)
                {
                    t = tt;
                    break;
                }
            }
            if (t == null) { P("ERROR: no encuentro ForceFeelAccess"); return; }

            MethodInfo gm = t.GetMethod("GetDeviceSerialPort",
                BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static | BindingFlags.Instance);
            if (gm == null) { P("ERROR: no encuentro GetDeviceSerialPort"); return; }

            object target = gm.IsStatic ? null : Activator.CreateInstance(t);
            object result = gm.Invoke(target, null);
            if (result == null) { P("HF8: NO detectado"); return; }
            P("HF8: " + result);
        }
        catch (Exception e)
        {
            P("ERROR: " + e.GetType().Name + ": " + e.Message);
            if (e.InnerException != null) P(" inner: " + e.InnerException.Message);
        }
    }
}
CS
fi

export WINEPREFIX="$BOTTLE"
export WINEDEBUG=-all

if [[ ! -x "$EXE" || "$SRC" -nt "$EXE" ]]; then
  "$WINE" "$CSC_WIN" /nologo /out:"Z:${EXE//\//\\}" "Z:${SRC//\//\\}" >/dev/null 2>&1 || {
    echo "ERROR: no se pudo compilar $EXE" >&2
    exit 1
  }
fi

OUTPUT="$("$WINE" "Z:${EXE//\//\\}" 2>&1 | tr -d '\r' || true)"
echo "$OUTPUT" | grep -v '^ntsync:' || true

if echo "$OUTPUT" | grep -q '^HF8: '; then
  exit 0
fi
exit 1
