use crate::{Error, Result};

pub type SendInfo<'a> = CommunicationInfo<'a>;
pub type RecvInfo<'a> = CommunicationInfo<'a>;

#[derive(Debug)]
pub struct CommunicationInfo<'a> {
    pub bift_id: u32,
    pub proto: u16,
    pub bitstring: &'a [u8],
    pub payload: &'a [u8],
}

impl CommunicationInfo<'_> {
    pub fn from_slice(slice: &'_ [u8]) -> Result<CommunicationInfo> {
        let bift_id = unsafe { crate::get_unchecked_be_u32(slice.as_ptr()) };

        let proto = unsafe { crate::get_unchecked_be_u16(slice.as_ptr().add(4)) };

        let bitstring_length =
            unsafe { crate::get_unchecked_be_u16(slice.as_ptr().add(6)) as usize };

        if slice.len() < 4 + 2 + 2 + bitstring_length {
            return Err(crate::Error::SliceWrongLength);
        }

        Ok(CommunicationInfo {
            bift_id,
            proto,
            bitstring: &slice[8..8 + bitstring_length],
            payload: &slice[8 + bitstring_length..],
        })
    }

    pub fn to_slice(&self, slice: &mut [u8]) -> Result<usize> {
        let len = 8 + self.bitstring.len() + self.payload.len();
        if slice.len() < len {
            return Err(Error::SliceWrongLength);
        }

        let val = self.bift_id.to_be_bytes();
        slice[..4].copy_from_slice(&val);
        slice[4..6].copy_from_slice(&self.proto.to_be_bytes());
        slice[6..8].copy_from_slice(&(self.bitstring.len() as u16).to_be_bytes());
        slice[8..8 + self.bitstring.len()].copy_from_slice(self.bitstring);
        slice[8 + self.bitstring.len()..len].copy_from_slice(self.payload);

        Ok(len)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_recv_info_from_slice() {
        let buffer = [
            0, 0, 0, 1, // BIFT-ID
            0, 36, // Proto
            0, 8, // Bitstring length
            0, 0, 0, 0, 0, 0, 0, 0xff, // Bitstring
            0, 4, 1, 2, 5, // Payload
        ];

        let recv_info = RecvInfo::from_slice(&buffer);
        assert!(recv_info.is_ok());

        let recv_info = recv_info.unwrap();
        assert_eq!(recv_info.bift_id, 1);
        assert_eq!(recv_info.proto, 36);
        assert_eq!(recv_info.bitstring.len(), 8);
        assert_eq!(recv_info.bitstring, &[0, 0, 0, 0, 0, 0, 0, 0xff]);
        assert_eq!(recv_info.payload.len(), 5);
        assert_eq!(recv_info.payload, &[0, 4, 1, 2, 5]);
    }

    #[test]
    fn test_send_info_to_slice() {
        let send_info = SendInfo {
            bift_id: 0xffddee11,
            proto: 0x37,
            bitstring: &[0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x43, 0x78],
            payload: &[0x11, 0x44, 0xdf, 0x21, 0x44, 0x33, 0x3, 0x21],
        };

        let mut buffer = [0u8; 1000];

        let res = send_info.to_slice(&mut buffer[..]);
        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res, 4 + 2 + 2 + send_info.bitstring.len() + send_info.payload.len());
        assert_eq!(&buffer[..4], &[0xff, 0xdd, 0xee, 0x11]);
        assert_eq!(&buffer[4..6], &[0x00, 0x37]);
        assert_eq!(&buffer[6..8], &[0, 8]);
        assert_eq!(&buffer[8..16], &[0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x43, 0x78]);
        assert_eq!(&buffer[16..res], &[0x11, 0x44, 0xdf, 0x21, 0x44, 0x33, 0x3, 0x21]);
    }
}
