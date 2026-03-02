# Projet 04 — Stack de communication custom : Modbus RTU bare-metal

## Contexte

Tu implémentes une stack **Modbus RTU** complète en Rust no_std, sans bibliothèque Modbus existante.
Modbus RTU est le protocole industriel le plus répandu en automatisation (variateurs, capteurs, automates).
Maîtriser son implémentation bare-metal est une compétence directement valorisable en industrie (énergie, automation, building).

**Cible matérielle** : STM32F411 — UART2 (RS-485 via MAX485)  
**Environnement** : `#![no_std]` `#![no_main]`, pas de `alloc`  
**Rôle** : Esclave Modbus RTU (slave) répondant à un maître PC (ModbusPoll ou script Python)

---

## Objectifs du projet

1. Implémenter le **framing Modbus RTU** : détection de trame par timeout inter-caractère (3,5 caractères)
2. Implémenter le **calcul CRC16 Modbus** (polynôme 0xA001)
3. Implémenter les **function codes** : FC01 (Read Coils), FC03 (Read Holding Registers), FC06 (Write Single Register), FC16 (Write Multiple Registers)
4. Implémenter une **table de registres** : 64 holding registers, 16 coils
5. Implémenter la **gestion d'erreurs Modbus** : exception codes 01, 02, 03, 04
6. Valider avec un **maître Modbus** (script Python `pymodbus` ou outil ModbusPoll)

---

## Spécifications techniques

### Trame Modbus RTU

```
[Addr 1B][FC 1B][Data N×B][CRC 2B]
```

Timing inter-trame : silence de 3,5 × temps d'un caractère (à 9600 baud = ~4 ms)

### Structure principale

```rust
pub struct ModbusRtuSlave {
    address: u8,                        // adresse esclave (1-247)
    holding_regs: [u16; 64],            // registres 40001-40064
    coils: [bool; 16],                  // bobines 00001-00016
    rx_buf: [u8; 256],                  // buffer réception
    rx_len: usize,
    last_rx_tick: u32,                  // pour le timeout 3.5T
}

impl ModbusRtuSlave {
    pub fn process_byte(&mut self, byte: u8, tick: u32) -> Option<&[u8]>;
    fn dispatch(&mut self, frame: &[u8]) -> Option<heapless::Vec<u8, 256>>;
    fn fc03_read_holding_regs(&self, start: u16, count: u16) -> Result<heapless::Vec<u8, 256>, ModbusException>;
    fn fc06_write_single_reg(&mut self, addr: u16, value: u16) -> Result<heapless::Vec<u8, 8>, ModbusException>;
    fn crc16(data: &[u8]) -> u16;
}
```

### Table des registres applicatifs

```rust
// Mapping des registres sur des données physiques simulées
const REG_TEMPERATURE:  u16 = 0;    // °C × 10 (ex: 235 = 23.5°C)
const REG_HUMIDITY:     u16 = 1;    // % × 10
const REG_PRESSURE:     u16 = 2;    // hPa
const REG_SETPOINT:     u16 = 10;   // consigne configurable
const REG_STATUS:       u16 = 20;   // bitfield état
```

### Script de validation Python

```python
# test_modbus.py — à inclure dans le projet
from pymodbus.client import ModbusSerialClient

client = ModbusSerialClient(port="COM3", baudrate=9600)
result = client.read_holding_registers(0, 3, slave=1)
print(f"T={result.registers[0]/10}°C")
```

---

## Livrables attendus

- [ ] `modbus_rtu.rs` : framing + dispatch complet
- [ ] `crc16.rs` : implémentation CRC16 Modbus avec table de lookup
- [ ] `register_map.rs` : table de registres avec accès typé
- [ ] FC01, FC03, FC06, FC16 implémentés et testés
- [ ] Gestion exceptions Modbus (codes 01–04)
- [ ] `tests/modbus_unit_tests.rs` : tests sur des trames statiques connues
- [ ] `test_modbus.py` : script de validation depuis le PC
- [ ] README avec tableau des registres et exemple de trame

---

## Critères de qualité

- CRC16 validé sur des trames de référence connues (cf. documentation Modbus)
- Pas d'allocation dynamique, buffers statiques uniquement
- Temps de réponse esclave < 1 ms après fin de trame maître (mesurable avec logic analyzer)
- Tests unitaires sur les function codes avec trames hex documentées
- Gestion du timeout 3,5T par compteur de ticks ou DWT
