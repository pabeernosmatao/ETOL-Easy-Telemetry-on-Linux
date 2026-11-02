#!/usr/bin/env python3
"""Genera el .reg que hace visible el pad HF8 (ForceFeel) a SimHub bajo Wine.

Wine devuelve DEVPKEY_NAME (Properties\\{b725f130...}\\000A) como REG_SZ, pero
SimHub espera DEVPROP_TYPE_STRING (0x12) y descarta el dispositivo si el tipo
no coincide. El formato .reg permite forzar el tipo con hex(12):.

Solo la interfaz MI_00 (CDC) lleva el nombre con "(COMn)". El dispositivo padre
y MI_01 se dejan sin el sufijo para que SerialPortDescriptionHelper no los
confunda con un segundo puerto serie.
"""

from __future__ import annotations

import argparse
import sys

CLASS_GUID = "{4D36E978-E325-11CE-BFC1-08002BE10318}"
NULL_GUID = "{00000000-0000-0000-0000-000000000000}"
PROP_NAME = "{b725f130-47ef-101a-a5f1-02608c9eebac}"
VID = "04D8"
PID = "EE96"


def esc(value: str) -> str:
    """Escapa barras invertidas en valores de cadena del formato .reg."""
    return value.replace("\\", "\\\\")


def hex12(value: str) -> str:
    data = (value + "\0").encode("utf-16-le")
    return "hex(12):" + ",".join(f"{b:02x}" for b in data)


def hex7(values: list[str]) -> str:
    data = b""
    for value in values:
        data += value.encode("utf-16-le") + b"\0\0"
    data += b"\0\0"
    return "hex(7):" + ",".join(f"{b:02x}" for b in data)


def symbolic_link(instance: str) -> str:
    return "\\\\?\\" + instance.replace("\\", "#") + "#" + CLASS_GUID


def device_class_key(instance: str) -> str:
    return "##?#" + instance.replace("\\", "#") + "#" + CLASS_GUID


def device_sections(suffix: str, com: str) -> list[str]:
    name = f"ForceFeel Pad ({com})"
    parent = f"USB\\VID_{VID}&PID_{PID}\\{suffix}"
    mi00 = f"USB\\VID_{VID}&PID_{PID}&MI_00\\{suffix}"
    mi01 = f"USB\\VID_{VID}&PID_{PID}&MI_01\\{suffix}"

    devices = [
        (
            parent,
            {
                "ClassGUID": NULL_GUID,
                "CompatibleIDs": [
                    "USB\\DevClass_02&SubClass_00&Prot_00",
                    "USB\\DevClass_02&SubClass_00",
                    "USB\\DevClass_02",
                ],
                "HardwareId": [
                    f"USB\\VID_{VID}&PID_{PID}&REV_0100",
                    f"USB\\VID_{VID}&PID_{PID}",
                ],
                "FriendlyName": "ForceFeel Pad",
                "Name": "ForceFeel Pad",
                "PortName": None,
            },
        ),
        (
            mi00,
            {
                "Class": "Ports",
                "ClassGUID": CLASS_GUID,
                "Driver": f"{CLASS_GUID}\\0001",
                "Service": "usbser",
                "CompatibleIDs": [
                    "USB\\Class_02&SubClass_02&Prot_01",
                    "USB\\Class_02&SubClass_02",
                    "USB\\Class_02",
                ],
                "HardwareId": [
                    f"USB\\VID_{VID}&PID_{PID}&REV_0100&MI_00",
                    f"USB\\VID_{VID}&PID_{PID}&MI_00",
                ],
                "FriendlyName": name,
                "Name": name,
                "PortName": com,
            },
        ),
        (
            mi01,
            {
                "Class": "Ports",
                "ClassGUID": CLASS_GUID,
                "Driver": f"{CLASS_GUID}\\0001",
                "Service": "usbser",
                "CompatibleIDs": [
                    "USB\\Class_0a&SubClass_00&Prot_00",
                    "USB\\Class_0a&SubClass_00",
                    "USB\\Class_0a",
                ],
                "HardwareId": [
                    f"USB\\VID_{VID}&PID_{PID}&REV_0100&MI_01",
                    f"USB\\VID_{VID}&PID_{PID}&MI_01",
                ],
                "FriendlyName": "ForceFeel Pad",
                "Name": "ForceFeel Pad",
                "PortName": com,
            },
        ),
    ]

    lines: list[str] = []
    for instance, values in devices:
        lines.append(f"[HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Enum\\{instance}]")
        for key in ("Class", "ClassGUID", "Driver", "Service"):
            if key in values:
                lines.append(f'"{key}"="{esc(str(values[key]))}"')
        lines.append(f'"CompatibleIDs"={hex7(values["CompatibleIDs"])}')
        lines.append('"ConfigFlags"=dword:00000000')
        lines.append('"DeviceDesc"="ForceFeel Pad"')
        lines.append(f'"FriendlyName"="{esc(str(values["FriendlyName"]))}"')
        lines.append(f'"HardwareId"={hex7(values["HardwareId"])}')
        lines.append("")

        if values["PortName"]:
            lines.append(
                f"[HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Enum\\{instance}\\Device Parameters]"
            )
            lines.append(f'"PortName"="{esc(str(values["PortName"]))}"')
            lines.append("")

        lines.append(
            f"[HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Enum\\{instance}\\Properties\\{PROP_NAME}\\000A]"
        )
        lines.append("@=" + hex12(str(values["Name"])))
        lines.append("")

        base = (
            "HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\DeviceClasses\\"
            f"{CLASS_GUID}\\{device_class_key(instance)}"
        )
        lines.append(f"[{base}]")
        lines.append(f'"DeviceInstance"="{esc(instance)}"')
        lines.append("")
        lines.append(f"[{base}\\#]")
        lines.append(f'"SymbolicLink"="{esc(symbolic_link(instance))}"')
        lines.append("")
        lines.append(f"[{base}\\#\\Control]")
        lines.append('"Linked"=dword:00000001')
        lines.append("")

    return lines


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--com", default="COM33", help="puerto COM a publicar (defecto: COM33)")
    parser.add_argument(
        "--suffix",
        action="append",
        dest="suffixes",
        help="sufijo de instancia USB; se puede repetir (defecto: 513&256&1&10)",
    )
    parser.add_argument("--out", help="fichero .reg de salida (defecto: stdout)")
    args = parser.parse_args()

    suffixes = args.suffixes or ["513&256&1&10"]
    lines = ["Windows Registry Editor Version 5.00", ""]
    for suffix in suffixes:
        lines.extend(device_sections(suffix, args.com))

    text = "\n".join(lines)
    if args.out:
        with open(args.out, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(text)
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
