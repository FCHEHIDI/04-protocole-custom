#!/usr/bin/env python3
"""
Script de validation Modbus RTU — esclave STM32F411 adresse 1
Prérequis : pip install pymodbus
Usage    : python test_modbus.py --port COM3
"""

import argparse
from pymodbus.client import ModbusSerialClient

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", default="COM3")
    parser.add_argument("--baud", type=int, default=9600)
    args = parser.parse_args()

    client = ModbusSerialClient(
        port=args.port,
        baudrate=args.baud,
        bytesize=8,
        parity="N",
        stopbits=1,
        timeout=1,
    )

    if not client.connect():
        print(f"[ERREUR] Impossible de se connecter sur {args.port}")
        return

    print(f"[OK] Connecté sur {args.port} @ {args.baud} baud\n")

    # FC03 — lire 3 registres (température, humidité, pression)
    result = client.read_holding_registers(address=0, count=3, slave=1)
    if result.isError():
        print(f"[ERREUR] FC03 : {result}")
    else:
        r = result.registers
        print(f"FC03 Température : {r[0] / 10:.1f} °C")
        print(f"FC03 Humidité    : {r[1] / 10:.1f} %")
        print(f"FC03 Pression    : {r[2]} hPa")

    # FC06 — écrire la consigne (registre 10) à 22.0°C
    result = client.write_register(address=10, value=220, slave=1)
    if result.isError():
        print(f"\n[ERREUR] FC06 : {result}")
    else:
        print(f"\nFC06 Consigne écrite : 22.0 °C")

    # FC03 — relire le registre 10 pour confirmer
    result = client.read_holding_registers(address=10, count=1, slave=1)
    if not result.isError():
        print(f"FC03 Consigne relue : {result.registers[0] / 10:.1f} °C")

    # FC01 — lire 4 coils
    result = client.read_coils(address=0, count=4, slave=1)
    if result.isError():
        print(f"\n[ERREUR] FC01 : {result}")
    else:
        print(f"\nFC01 Coils [0-3] : {result.bits[:4]}")

    client.close()
    print("\n[OK] Test terminé")

if __name__ == "__main__":
    main()