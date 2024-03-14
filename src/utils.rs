const CRC16_POLY: u16 = 0xA001;

#[allow(unused)]
pub fn compute_crc(data: &[u8]) -> [u8; 2] {
    let mut crc: u16 = 0xFFFF;

    for &byte in data.iter() {
        crc ^= u16::from(byte);
        for _ in 0..8 {
            crc = if crc & 0x0001 != 0 {
                (crc >> 1) ^ CRC16_POLY
            } else {
                crc >> 1
            };
        }
    }

    [(crc & 0xff) as u8, (crc >> 8) as u8]
}