# Modbus RTU Slave — Bare-Metal Rust `no_std`

Stack de communication **Modbus RTU** complète, implémentée en Rust `no_std` sans allocation dynamique.  
Cible : **STM32F411** — UART2 + MAX485 (RS-485).

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Maître Modbus (PC)                   │
│              ModbusPoll / pymodbus / SCADA              │
└─────────────────────────┬───────────────────────────────┘
                          │ RS-485 / UART 9600 8N1
                          ▼
┌─────────────────────────────────────────────────────────┐
│                   ModbusRtuSlave                        │
│                                                         │
│  ┌─────────────┐   ┌──────────────┐   ┌─────────────┐  │
│  │  Framing    │   │   Dispatch   │   │  Register   │  │
│  │ process_byte│──▶│  CRC check   │──▶│    Map      │  │
│  │  3.5T timer │   │  FC routing  │   │  64 × u16   │  │
│  └─────────────┘   └──────────────┘   │  16 × bool  │  │
│                                       └─────────────┘  │
│  ┌──────────────────────────────────────────────────┐   │
│  │                   crc16.rs                       │   │
│  │         Lookup table 256 × u16 — 0xA001          │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

---

## Structure du projet

```
src/
├── lib.rs          — point d'entrée no_std, déclaration des modules
├── crc16.rs        — CRC16 Modbus par table de lookup (const fn)
├── register_map.rs — table de registres 64×u16 + 16×bool
└── modbus_rtu.rs   — framing, dispatch, FC01/FC03/FC06/FC16, exceptions
test_modbus.py      — validation end-to-end via pymodbus
```

---

## Protocole — Format de trame RTU

```
┌────────┬────────┬──────────────┬───────────┐
│ Addr   │  FC    │    Data      │  CRC16    │
│  1 B   │  1 B   │   N bytes    │   2 B     │
└────────┴────────┴──────────────┴───────────┘
         ←──── CRC calculé sur ────────►

Silence inter-trame : 3.5 × tchar ≈ 4 ms @ 9600 baud
CRC : polynôme 0xA001, init 0xFFFF, little-endian (low byte first)
```

### Exemple de trame — FC03 Read Holding Registers

```
Requête  : 01 03 00 00 00 03 05 0B
           ↑  ↑  ↑──────┘ ↑──────┘ ↑─────────┘
           │  │  start=0  count=3  CRC

Réponse  : 01 03 06 00 EB 02 8A 03 F5 xx xx
           ↑  ↑  ↑   ↑────┘ ↑────┘ ↑────┘ ↑─┘
           │  │  │   235    650    1013   CRC
           │  │  byte_count=6
           │  FC=03
           addr=1
```

---

## Function Codes implémentés

| FC   | Nom                      | Requête                          | Réponse                     |
|------|--------------------------|----------------------------------|-----------------------------|
| 0x01 | Read Coils               | addr, start(2), count(2)         | addr, FC, bytecount, data…  |
| 0x03 | Read Holding Registers   | addr, start(2), count(2)         | addr, FC, bytecount, data…  |
| 0x06 | Write Single Register    | addr, reg(2), value(2)           | echo de la requête          |
| 0x10 | Write Multiple Registers | addr, start(2), count(2), data…  | addr, FC, start(2), count(2)|

### Codes d'exception

| Code | Nom                   | Déclenchement                        |
|------|-----------------------|--------------------------------------|
| 0x01 | Illegal Function      | FC non supporté                      |
| 0x02 | Illegal Data Address  | Adresse registre hors range (≥ 64)   |
| 0x03 | Illegal Data Value    | Count = 0 ou > max autorisé          |
| 0x04 | Server Device Failure | Réservé                              |

Réponse exception : `addr | FC+0x80 | exception_code | CRC`

---

## Table des registres

### Holding Registers (FC03 / FC06 / FC16)

| Adresse | Constante        | Unité    | Exemple      | Description             |
|---------|------------------|----------|--------------|-------------------------|
| 0       | `REG_TEMPERATURE`| °C × 10  | 235 = 23.5°C | Température mesurée     |
| 1       | `REG_HUMIDITY`   | % × 10   | 650 = 65.0%  | Humidité relative       |
| 2       | `REG_PRESSURE`   | hPa      | 1013         | Pression atmosphérique  |
| 3–9     | —                | —        | —            | Réservé                 |
| 10      | `REG_SETPOINT`   | °C × 10  | 200 = 20.0°C | Consigne configurable   |
| 11–19   | —                | —        | —            | Réservé                 |
| 20      | `REG_STATUS`     | bitfield | 0b00000011   | État système            |
| 21–63   | —                | —        | —            | Disponible              |

### Coils (FC01)

| Adresse | Description              |
|---------|--------------------------|
| 0–15    | Sorties logiques ON/OFF  |

---

## Tests unitaires

```
cargo test
```

```
running 8 tests
test crc16::tests::test_crc_known_frame         ... ok
test modbus_rtu::tests::test_fc01_read_coils    ... ok
test modbus_rtu::tests::test_fc03_read_3_regs   ... ok
test modbus_rtu::tests::test_fc06_write_reg     ... ok
test modbus_rtu::tests::test_fc16_write_multiple_regs ... ok
test modbus_rtu::tests::test_fc_inconnu_exception     ... ok
test modbus_rtu::tests::test_crc_invalide_silence     ... ok
test modbus_rtu::tests::test_mauvaise_adresse_silence ... ok

test result: ok. 8 passed; 0 failed
```

---

## Validation end-to-end (hardware requis)

```bash
pip install pymodbus
python test_modbus.py --port COM3 --baud 9600
```

Prérequis : firmware flashé sur STM32F411, MAX485 branché sur UART2.

---

## Contraintes techniques

| Critère                        | Valeur              |
|-------------------------------|---------------------|
| Environnement                 | `no_std`, `no_main` |
| Allocation dynamique          | aucune              |
| Buffer RX                     | 256 B statique      |
| Temps de réponse cible        | < 1 ms après fin de trame |
| Timeout inter-trame (3.5T)    | ~4 ms @ 9600 baud   |
| Holding registers             | 64 × u16            |
| Coils                         | 16 × bool           |
