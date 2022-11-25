use crate::bier::{Error, Result};

#[allow(dead_code)]
#[derive(Debug)]
pub struct BierHeader {
    bift_id: u32,
    tc: u8,
    s: bool,
    ttl: u8,
    nibble: u8,
    ver: u8,
    bsl: u8,
    entropy: u32,
    oam: u8,
    dscp: u8,
    rsv: u8,
    proto: u8,
    bfr_id: u16,
    bitstring: Vec<u64>,
}

pub const BIER_MINIMUM_HEADER_LENGTH: usize = 20;
pub const BIER_HEADER_WITHOUT_BITSTRING_LENGTH: usize = 12;

impl BierHeader {
    pub fn from_slice(slice: &[u8]) -> Result<BierHeader> {
        if slice.len() < BIER_MINIMUM_HEADER_LENGTH {
            return Err(Error::Header);
        }

        let bsl = unsafe { (*slice.get_unchecked(5) & 0xf0) >> 4 };

        let bitstring_length = 1 << (bsl + 5);
        let bitstring_length = bitstring_length / 8;
        if slice.len() - bitstring_length != BIER_HEADER_WITHOUT_BITSTRING_LENGTH {
            return Err(Error::Header);
        }

        let header = BierHeader {
            bift_id: get_bift_id(slice),
            tc: get_tc(slice),
            s: get_s(slice),
            ttl: get_ttl(slice),
            nibble: get_nibble(slice),
            ver: get_version(slice),
            bsl: get_bsl(slice),
            entropy: get_entropy(slice),
            oam: get_oam(slice),
            dscp: get_dscp(slice),
            rsv: get_rsv(slice),
            proto: get_proto(slice),
            bfr_id: get_bifr_id(slice),
            bitstring: get_bitstring(slice),
        };

        Ok(header)
    }

    pub fn to_slice(&self, slice: &mut [u8]) -> Result<()> {
        if slice.len() != self.header_length() {
            return Err(crate::bier::Error::SliceWrongLength);
        }

        let val: u32 = (self.bift_id << 12)
            + ((self.tc as u32) << 9)
            + ((self.s as u32) << 8)
            + (self.ttl as u32);
        let bytes: [u8; 4] = val.to_be_bytes();
        slice[..4].copy_from_slice(&bytes);

        let val: u32 = ((self.nibble as u32) << 28)
            + ((self.ver as u32) << 24)
            + ((self.bsl as u32) << 20)
            + self.entropy;
        let bytes: [u8; 4] = val.to_be_bytes();
        slice[4..8].copy_from_slice(&bytes);

        let val: u32 = ((self.oam as u32) << 30)
            + ((self.rsv as u32) << 28)
            + ((self.dscp as u32) << 22)
            + ((self.proto as u32) << 16)
            + (self.bfr_id as u32);
        let bytes: [u8; 4] = val.to_be_bytes();
        slice[8..12].copy_from_slice(&bytes);

        unsafe {
            let bitstring: Vec<u64> = self.bitstring.iter().map(|item| item.to_be()).collect();
            let p = bitstring.as_ptr() as *const u8;
            let bitstring = std::slice::from_raw_parts(p, self.bitstring.len() * 8);
            slice[12..].copy_from_slice(bitstring);
        }

        Ok(())
    }

    pub fn get_bitstring(&self) -> crate::bier::Bitstring {
        self.bitstring.clone().into()
    }

    pub fn get_bift_id(&self) -> u32 {
        self.bift_id
    }

    pub fn header_length(&self) -> usize {
        BIER_HEADER_WITHOUT_BITSTRING_LENGTH + self.bitstring.len() * 8
    }
}

impl Default for BierHeader {
    fn default() -> Self {
        Self {
            bift_id: Default::default(),
            tc: Default::default(),
            s: Default::default(),
            ttl: Default::default(),
            nibble: Default::default(),
            ver: Default::default(),
            bsl: 0,
            entropy: Default::default(),
            oam: Default::default(),
            dscp: Default::default(),
            rsv: Default::default(),
            proto: Default::default(),
            bfr_id: Default::default(),
            bitstring: vec![0; 1],
        }
    }
}

fn get_bift_id(slice: &[u8]) -> u32 {
    unsafe { (get_unchecked_be_u32(slice.as_ptr()) & 0xfffff000) >> 12 }
}

fn get_tc(slice: &[u8]) -> u8 {
    unsafe { (slice.get_unchecked(2) & 0x0e) >> 1 }
}

fn get_s(slice: &[u8]) -> bool {
    unsafe { slice.get_unchecked(2) & 1 == 1 }
}

fn get_ttl(slice: &[u8]) -> u8 {
    unsafe { *slice.get_unchecked(3) }
}

fn get_nibble(slice: &[u8]) -> u8 {
    unsafe { (*slice.get_unchecked(4) & 0xf0) >> 4 }
}

fn get_version(slice: &[u8]) -> u8 {
    unsafe { *slice.get_unchecked(4) & 0xf }
}

fn get_bsl(slice: &[u8]) -> u8 {
    unsafe { (*slice.get_unchecked(5) & 0xf0) >> 4 }
}

fn get_entropy(slice: &[u8]) -> u32 {
    unsafe { get_unchecked_be_u32(slice.as_ptr().add(4)) & 0xfffff }
}

fn get_oam(slice: &[u8]) -> u8 {
    unsafe { (*slice.get_unchecked(8) & 0xc0) >> 6 }
}

fn get_rsv(slice: &[u8]) -> u8 {
    unsafe { (*slice.get_unchecked(8) & 0x30) >> 4 }
}

fn get_dscp(slice: &[u8]) -> u8 {
    unsafe { ((get_unchecked_be_u16(slice.as_ptr().add(8)) & 0xfc0) >> 6) as u8 }
}

fn get_proto(slice: &[u8]) -> u8 {
    unsafe { *slice.get_unchecked(9) & 0x3f }
}

fn get_bifr_id(slice: &[u8]) -> u16 {
    unsafe { get_unchecked_be_u16(slice.as_ptr().add(10)) }
}

fn get_bitstring(slice: &[u8]) -> Vec<u64> {
    slice[12..]
        .chunks(8)
        .map(|chunk| u64::from_be_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<u64>>()
}

unsafe fn get_unchecked_be_u16(ptr: *const u8) -> u16 {
    u16::from_be_bytes([*ptr, *ptr.add(1)])
}

unsafe fn get_unchecked_be_u32(ptr: *const u8) -> u32 {
    u32::from_be_bytes([*ptr, *ptr.add(1), *ptr.add(2), *ptr.add(3)])
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn get_dummy_bier_header_slice() -> [u8; 20] {
        [
            0u8, 0, 0x43, // BIFT-ID + TC + S
            7,    // TTL
            0x51, // Nibble + Version
            0x10, // BSL + Entropy
            0x0, 0x3,  // Entropy
            0xf1, // Oam + Rsv + DSCP
            0x4,  // DSCP + Proto
            0x0, 0x11, // BFR-ID
            0, 0, 0, 0, 0, 0, 0xff, 0xff, // Bitstring
        ]
    }

    #[test]
    fn test_bier_header_from_bytes() {
        let buf = get_dummy_bier_header_slice();

        let bier_header_opt = BierHeader::from_slice(&buf);
        assert!(bier_header_opt.is_ok());
        let bier_header = bier_header_opt.unwrap();

        assert_eq!(bier_header.bift_id, 4);
        assert_eq!(bier_header.tc, 1);
        assert_eq!(bier_header.s, true);
        assert_eq!(bier_header.ttl, 7);
        assert_eq!(bier_header.nibble, 5);
        assert_eq!(bier_header.ver, 1);
        assert_eq!(bier_header.bsl, 1);
        assert_eq!(bier_header.entropy, 3);
        assert_eq!(bier_header.oam, 3);
        assert_eq!(bier_header.rsv, 3);
        assert_eq!(bier_header.dscp, 4);
        assert_eq!(bier_header.proto, 4);
        assert_eq!(bier_header.bfr_id, 0x11);
        assert_eq!(bier_header.bitstring.len(), 1);
        assert_eq!(bier_header.bitstring[0], 0xffff);
    }

    #[test]
    fn test_bier_header_from_bytes_wrong_bitstring_length() {
        let buf = [
            0u8, 0, 0x43, 7, 0x51, 0x20, // BSL of 2
            0x0, 0x3, 0xf1, 0x4, 0x0, 0x11, 0, 0, 0, 0, 0, 0, 0xff, 0xff,
        ];

        let bier_header_opt = BierHeader::from_slice(&buf);
        assert!(bier_header_opt.is_err());
    }

    #[test]
    fn test_bier_header_to_slice_dummy() {
        // Get a dummy BIER header and slice it.
        let bier_header = BierHeader::default();
        let mut buff = [42u8; BIER_MINIMUM_HEADER_LENGTH];

        assert!(bier_header.to_slice(&mut buff).is_ok());

        let expected = [0u8; BIER_MINIMUM_HEADER_LENGTH];
        assert_eq!(buff, expected);
    }

    #[test]
    fn test_bier_header_to_slice() {
        let buf = get_dummy_bier_header_slice();

        // Convert to a BIER header.
        let bier_header = BierHeader::from_slice(&buf).unwrap();

        // Convert back to a slice in a different buffer.
        let mut res = [0u8; 20];
        assert!(bier_header.to_slice(&mut res).is_ok());
        println!("BIER header: {:?}", bier_header);

        // Expect the result to be the same.
        assert_eq!(buf, res);
    }
}
