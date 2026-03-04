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
            0x03 => self.fc03_read_holding_regs(frame),
            0x06 => self.fc06_write_single_reg(frame),
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