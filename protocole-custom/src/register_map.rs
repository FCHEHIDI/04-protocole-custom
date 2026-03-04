/// Adresses des registres applicatifs
pub const REG_TEMPERATURE: u16 = 0;   // °C × 10
pub const REG_HUMIDITY:    u16 = 1;   // % × 10
pub const REG_PRESSURE:    u16 = 2;   // hPa
pub const REG_SETPOINT:    u16 = 10;  // consigne configurable
pub const REG_STATUS:      u16 = 20;  // bitfield état

pub const HOLDING_REG_COUNT: usize = 64;
pub const COIL_COUNT:        usize = 16;

pub struct RegisterMap {
    pub holding_regs: [u16; HOLDING_REG_COUNT],
    pub coils:        [bool; COIL_COUNT],
}

impl RegisterMap {
    pub const fn new() -> Self {
        Self {
            holding_regs: [0u16; HOLDING_REG_COUNT],
            coils:        [false; COIL_COUNT],
        }
    }

    /// Lit un holding register — retourne None si hors range
    pub fn read_reg(&self, addr: u16) -> Option<u16> {
        self.holding_regs.get(addr as usize).copied()
    }

    /// Écrit un holding register — retourne false si hors range
    pub fn write_reg(&mut self, addr: u16, value: u16) -> bool {
        if let Some(slot) = self.holding_regs.get_mut(addr as usize) {
            *slot = value;
            true
        } else {
            false
        }
    }

    /// Lit un coil — retourne None si hors range
    pub fn read_coil(&self, addr: u16) -> Option<bool> {
        self.coils.get(addr as usize).copied()
    }

    /// Écrit un coil — retourne false si hors range
    pub fn write_coil(&mut self, addr: u16, value: bool) -> bool {
        if let Some(slot) = self.coils.get_mut(addr as usize) {
            *slot = value;
            true
        } else {
            false
        }
    }
}