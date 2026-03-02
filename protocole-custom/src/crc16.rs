/// CRC16 Modbus — polynôme 0xA001 (Little-Endian bit processing)
/// Implémentation par table de lookup (256 entrées × u16)
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        let idx = (crc ^ byte as u16) & 0xFF;
        crc = (crc >> 8) ^ TABLE[idx as usize];
    }
    crc
}

static TABLE: [u16; 256] = generate_table();

const fn generate_table() -> [u16; 256] {
    let mut table = [0u16; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = i as u16;
        let mut j = 0;
        while j < 8 {
            if crc & 0x0001 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc_known_frame() {
        // Trame Modbus connue : esclave=1, FC=3, start=0, count=2
        // CRC u16 = 0x0BC4 → transmis low byte first : 0xC4 puis 0x0B
        // Trame complète sur le fil : 01 03 00 00 00 02 C4 0B
        let frame = [0x01u8, 0x03, 0x00, 0x00, 0x00, 0x02];
        assert_eq!(crc16(&frame), 0x0BC4);
    }
}
