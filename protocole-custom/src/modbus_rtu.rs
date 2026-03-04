use heapless::Vec;
use crate::crc16::crc16;
use crate::register_map::RegisterMap;

const INTER_FRAME_TICKS: u32 = 4; // ~4ms à 9600 baud = 3.5T

#[derive(Debug, Clone, Copy)]
pub enum ModbusException {
    IllegalFunction   = 0x01,
    IllegalDataAddress = 0x02,
    IllegalDataValue   = 0x03,
    ServerDeviceFailure = 0x04,
}

pub struct ModbusRtuSlave {
    pub address:    u8,
    pub regs:       RegisterMap,
    rx_buf:         [u8; 256],
    rx_len:         usize,
    last_rx_tick:   u32,
}

impl ModbusRtuSlave {
    pub const fn new(address: u8) -> Self {
        Self {
            address,
            regs: RegisterMap::new(),
            rx_buf: [0u8; 256],
            rx_len: 0,
            last_rx_tick: 0,
        }
    }

    /// Appelé à chaque octet reçu sur l'UART. 
    /// Retourne Some(réponse) quand une trame complète est traitée.
    pub fn process_byte(&mut self, byte: u8, tick: u32) -> Option<Vec<u8, 256>> {
        // Détection du silence inter-trame (3.5T)
        if self.rx_len > 0 && tick.wrapping_sub(self.last_rx_tick) >= INTER_FRAME_TICKS {
            let len = self.rx_len;
            self.rx_len = 0; // reset silencieux
            // Copie locale pour libérer l'emprunt de self.rx_buf avant dispatch()
            let mut frame_copy = [0u8; 256];
            frame_copy[..len].copy_from_slice(&self.rx_buf[..len]);
            let response = self.dispatch(&frame_copy[..len]);
            // On accumule le nouvel octet APRÈS le dispatch
            self.rx_buf[0] = byte;
            self.rx_len = 1;
            self.last_rx_tick = tick;
            return response;
        }

        if self.rx_len < 256 {
            self.rx_buf[self.rx_len] = byte;
            self.rx_len += 1;
        }
        self.last_rx_tick = tick;
        None
    }

    fn dispatch(&mut self, frame: &[u8]) -> Option<Vec<u8, 256>> {
        // Trame minimum : addr(1) + FC(1) + data(min 2) + CRC(2) = 6 octets
        if frame.len() < 6 {
            return None;
        }
        // Vérification adresse
        if frame[0] != self.address {
            return None;
        }
        // Vérification CRC
        let data = &frame[..frame.len() - 2];
        let crc_received = u16::from_le_bytes([
            frame[frame.len() - 2],
            frame[frame.len() - 1],
        ]);
        if crc16(data) != crc_received {
            return None; // CRC invalide → silence
        }

        let fc = frame[1];
        let result = match fc {
            0x01 => self.fc01_read_coils(frame),
            0x03 => self.fc03_read_holding_regs(frame),
            0x06 => self.fc06_write_single_reg(frame),
            0x10 => self.fc16_write_multiple_regs(frame),
            _    => Err(ModbusException::IllegalFunction),
        };

        match result {
            Ok(response) => Some(response),
            Err(exc)     => Some(self.build_exception(fc, exc)),
        }
    }

    fn fc03_read_holding_regs(&self, frame: &[u8]) -> Result<Vec<u8, 256>, ModbusException> {
        let start = u16::from_be_bytes([frame[2], frame[3]]);
        let count = u16::from_be_bytes([frame[4], frame[5]]);

        if count == 0 || count > 125 {
            return Err(ModbusException::IllegalDataValue);
        }

        let mut resp: Vec<u8, 256> = Vec::new();
        let _ = resp.push(self.address);
        let _ = resp.push(0x03);
        let _ = resp.push((count * 2) as u8);

        for i in 0..count {
            let val = self.regs.read_reg(start + i)
                .ok_or(ModbusException::IllegalDataAddress)?;
            let _ = resp.push((val >> 8) as u8);
            let _ = resp.push(val as u8);
        }

        let crc = crc16(&resp);
        let _ = resp.extend_from_slice(&crc.to_le_bytes());
        Ok(resp)
    }

    fn fc06_write_single_reg(&mut self, frame: &[u8]) -> Result<Vec<u8, 256>, ModbusException> {
        let addr  = u16::from_be_bytes([frame[2], frame[3]]);
        let value = u16::from_be_bytes([frame[4], frame[5]]);

        if !self.regs.write_reg(addr, value) {
            return Err(ModbusException::IllegalDataAddress);
        }

        // Echo de la requête = réponse FC06
        let mut resp: Vec<u8, 256> = Vec::new();
        let _ = resp.extend_from_slice(&frame[..6]);
        let crc = crc16(&resp);
        let _ = resp.extend_from_slice(&crc.to_le_bytes());
        Ok(resp)
    }

    fn fc01_read_coils(&self, frame: &[u8]) -> Result<Vec<u8, 256>, ModbusException> {
    let start = u16::from_be_bytes([frame[2], frame[3]]);
    let count = u16::from_be_bytes([frame[4], frame[5]]);

    if count == 0 || count > 2000 {
        return Err(ModbusException::IllegalDataValue);
    }

    let byte_count = (count as usize + 7) / 8; // arrondi supérieur

    let mut resp: Vec<u8, 256> = Vec::new();
    let _ = resp.push(self.address);
    let _ = resp.push(0x01);
    let _ = resp.push(byte_count as u8);

    for byte_idx in 0..byte_count {
        let mut byte_val: u8 = 0;
        for bit_idx in 0..8 {
            let coil_idx = byte_idx * 8 + bit_idx;
            if coil_idx < count as usize {
                let val = self.regs.read_coil(start + coil_idx as u16)
                    .ok_or(ModbusException::IllegalDataAddress)?;
                if val {
                    byte_val |= 1 << bit_idx; // LSB first
                }
            }
        }
        let _ = resp.push(byte_val);
    }

    let crc = crc16(&resp);
    let _ = resp.extend_from_slice(&crc.to_le_bytes());
    Ok(resp)
}

fn fc16_write_multiple_regs(&mut self, frame: &[u8]) -> Result<Vec<u8, 256>, ModbusException> {
    let start      = u16::from_be_bytes([frame[2], frame[3]]);
    let count      = u16::from_be_bytes([frame[4], frame[5]]);
    let byte_count = frame[6] as usize;

    if count == 0 || count > 123 || byte_count != count as usize * 2 {
        return Err(ModbusException::IllegalDataValue);
    }

    for i in 0..count {
        let offset = 7 + i as usize * 2;
        let value = u16::from_be_bytes([frame[offset], frame[offset + 1]]);
        if !self.regs.write_reg(start + i, value) {
            return Err(ModbusException::IllegalDataAddress);
        }
    }

    // Réponse FC16 : addr + FC + start(2) + count(2) + CRC
    let mut resp: Vec<u8, 256> = Vec::new();
    let _ = resp.extend_from_slice(&frame[..6]);
    let crc = crc16(&resp);
    let _ = resp.extend_from_slice(&crc.to_le_bytes());
    Ok(resp)
}

    fn build_exception(&self, fc: u8, exc: ModbusException) -> Vec<u8, 256> {
        let mut resp: Vec<u8, 256> = Vec::new();
        let _ = resp.push(self.address);
        let _ = resp.push(fc | 0x80); // bit 7 = erreur
        let _ = resp.push(exc as u8);
        let crc = crc16(&resp);
        let _ = resp.extend_from_slice(&crc.to_le_bytes());
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_slave() -> ModbusRtuSlave {
        let mut slave = ModbusRtuSlave::new(0x01);
        // Valeurs initiales simulées
        slave.regs.write_reg(0, 235);  // température 23.5°C
        slave.regs.write_reg(1, 650);  // humidité 65.0%
        slave.regs.write_reg(2, 1013); // pression 1013 hPa
        slave
    }

    /// Construit une trame complète en ajoutant le CRC calculé dynamiquement
    fn build_frame(data: &[u8]) -> Vec<u8, 256> {
        let mut frame: Vec<u8, 256> = Vec::new();
        let _ = frame.extend_from_slice(data);
        let crc = crc16(data);
        let _ = frame.extend_from_slice(&crc.to_le_bytes());
        frame
    }

    fn feed_frame(slave: &mut ModbusRtuSlave, frame: &[u8]) -> Option<Vec<u8, 256>> {
        for (i, &byte) in frame.iter().enumerate() {
            slave.process_byte(byte, i as u32);
        }
        // Simuler le silence 3.5T avec un tick lointain
        let silence_tick = frame.len() as u32 + 10;
        slave.process_byte(0x00, silence_tick)
    }

    #[test]
    fn test_fc03_read_3_regs() {
        let mut slave = make_slave();
        // Requête : addr=1, FC=03, start=0, count=3
        let request = build_frame(&[0x01u8, 0x03, 0x00, 0x00, 0x00, 0x03]);
        let resp = feed_frame(&mut slave, &request).expect("pas de réponse");

        // Réponse : addr FC bytecount val0H val0L val1H val1L val2H val2L CRC(2)
        assert_eq!(resp[0], 0x01); // adresse
        assert_eq!(resp[1], 0x03); // FC
        assert_eq!(resp[2], 0x06); // 3 registres × 2 octets
        assert_eq!(u16::from_be_bytes([resp[3], resp[4]]), 235);
        assert_eq!(u16::from_be_bytes([resp[5], resp[6]]), 650);
        assert_eq!(u16::from_be_bytes([resp[7], resp[8]]), 1013);
    }

    #[test]
    fn test_fc06_write_reg() {
        let mut slave = make_slave();
        // Requête : addr=1, FC=06, addr_reg=10, value=180
        let request = build_frame(&[0x01u8, 0x06, 0x00, 0x0A, 0x00, 0xB4]);
        let resp = feed_frame(&mut slave, &request).expect("pas de réponse");

        // FC06 répond en echo de la requête
        assert_eq!(resp[0], 0x01);
        assert_eq!(resp[1], 0x06);
        assert_eq!(u16::from_be_bytes([resp[2], resp[3]]), 10);   // adresse reg
        assert_eq!(u16::from_be_bytes([resp[4], resp[5]]), 180);  // valeur écrite
        // Vérifier que la valeur est bien en mémoire
        assert_eq!(slave.regs.read_reg(10), Some(180));
    }

    #[test]
    fn test_crc_invalide_silence() {
        let mut slave = make_slave();
        // Trame FC03 avec CRC volontairement faux
        let request = [0x01u8, 0x03, 0x00, 0x00, 0x00, 0x03, 0xFF, 0xFF];
        let resp = feed_frame(&mut slave, &request);
        assert!(resp.is_none(), "CRC invalide doit produire silence");
    }

    #[test]
    fn test_mauvaise_adresse_silence() {
        let mut slave = make_slave();
        // Requête adressée à l'esclave 2, pas 1
        let request = build_frame(&[0x02u8, 0x03, 0x00, 0x00, 0x00, 0x03]);
        let resp = feed_frame(&mut slave, &request);
        assert!(resp.is_none(), "mauvaise adresse doit produire silence");
    }

        #[test]
    fn test_fc01_read_coils() {
        let mut slave = make_slave();
        // Activer coils 0 et 2
        slave.regs.write_coil(0, true);
        slave.regs.write_coil(1, false);
        slave.regs.write_coil(2, true);

        let request = build_frame(&[0x01u8, 0x01, 0x00, 0x00, 0x00, 0x03]);
        let resp = feed_frame(&mut slave, &request).expect("pas de réponse");

        assert_eq!(resp[0], 0x01); // adresse
        assert_eq!(resp[1], 0x01); // FC
        assert_eq!(resp[2], 0x01); // 1 octet de données (3 coils → 1 byte)
        // coil0=1, coil1=0, coil2=1 → bits LSB first → 0b00000101 = 0x05
        assert_eq!(resp[3], 0x05);
    }

    #[test]
    fn test_fc16_write_multiple_regs() {
        let mut slave = make_slave();
        // Écrire 2 registres à partir de l'adresse 10 : valeurs 100 et 200
        // Trame : addr FC start_H start_L count_H count_L byte_count val0H val0L val1H val1L
        let request = build_frame(&[
            0x01u8, 0x10,
            0x00, 0x0A,       // start = 10
            0x00, 0x02,       // count = 2
            0x04,             // byte_count = 4
            0x00, 0x64,       // reg[10] = 100
            0x00, 0xC8,       // reg[11] = 200
        ]);
        let resp = feed_frame(&mut slave, &request).expect("pas de réponse");

        assert_eq!(resp[0], 0x01); // adresse
        assert_eq!(resp[1], 0x10); // FC
        assert_eq!(u16::from_be_bytes([resp[2], resp[3]]), 10);  // start
        assert_eq!(u16::from_be_bytes([resp[4], resp[5]]), 2);   // count
        // Vérifier en mémoire
        assert_eq!(slave.regs.read_reg(10), Some(100));
        assert_eq!(slave.regs.read_reg(11), Some(200));
    }

        #[test]
    fn test_fc_inconnu_exception() {
        let mut slave = make_slave();
        // FC 0x99 n'existe pas → exception 0x01 IllegalFunction
        let request = build_frame(&[0x01u8, 0x99, 0x00, 0x00, 0x00, 0x01]);
        let resp = feed_frame(&mut slave, &request).expect("pas de réponse");

        assert_eq!(resp[0], 0x01);        // adresse
        assert_eq!(resp[1], 0x99 | 0x80); // FC + bit erreur = 0xD9 (0x99 | 0x80)
        assert_eq!(resp[2], 0x01);        // exception code : IllegalFunction
    }
}