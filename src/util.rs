#![allow(dead_code)]
pub fn relative_offset_u32(base: u32, offset: i32) -> u32 {
    base.wrapping_add(offset as u32)
}

pub fn relative_offset_u16(base: u16, offset: i16) -> u16 {
    base.wrapping_add(offset as u16)
}

pub fn sign_extend_u8_to_u16(some_u8: u8) -> u16 {
    some_u8 as i8 as i16 as u16
}

//pub fn get_linear_address(segment: u16, offset: u16) -> u32 {
//    (((segment as u32) << 4) + offset as u32) & 0xFFFFFu32
//}

pub fn fmt_byte_array(bytes: &[u8]) -> String {
    let mut fmt_str = String::new();

    for byte in bytes {
        fmt_str.push_str(&format!("{:02X}", byte));
    }
    fmt_str
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    pub fn test_extend() {

        let extend1 = 0x7F;
        let extend2 = 0x80;

        let extended1 = sign_extend_u8_to_u16(extend1);
        assert_eq!(extended1, 0x007F);

        let extended2 = sign_extend_u8_to_u16(extend2);
        assert_eq!(extended2, 0xFF80);


    }
}