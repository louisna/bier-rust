pub struct RecvInfo<'a> {
    pub bift_id: u32,
    pub bitstring: &'a [u8],
    pub payload: &'a [u8],
}

impl RecvInfo<'_> {
    pub fn from_slice(slice: &'_ [u8]) -> crate::Result<RecvInfo> {
        let bift_id = unsafe { crate::get_unchecked_be_u32(slice.as_ptr()) };

        let bitstring_length =
            unsafe { crate::get_unchecked_be_u16(slice.as_ptr().add(4)) as usize };

        if slice.len() < 4 + 2 + bitstring_length {
            return Err(crate::Error::SliceWrongLength);
        }

        Ok(RecvInfo {
            bift_id,
            bitstring: &slice[6..6 + bitstring_length],
            payload: &slice[6 + bitstring_length..],
        })
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_recv_info_from_slice() {
        let buffer = [
            0, 0, 0, 1, // BIFT-ID
            0, 8, // Bitstring length
            0, 0, 0, 0, 0, 0, 0, 0xff, // Bitstring
            0, 4, 1, 2, 5, // Payload
        ];

        let recv_info = RecvInfo::from_slice(&buffer);
        assert!(recv_info.is_ok());

        let recv_info = recv_info.unwrap();
        assert_eq!(recv_info.bift_id, 1);
        assert_eq!(recv_info.bitstring.len(), 8);
        assert_eq!(recv_info.bitstring, &[0, 0, 0, 0, 0, 0, 0, 0xff]);
        assert_eq!(recv_info.payload.len(), 5);
        assert_eq!(recv_info.payload, &[0, 4, 1, 2, 5]);
    }
}
