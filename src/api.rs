pub struct RecvInfo<'a> {
    bift_id: u32,
    bitstring: &'a [u8],
    payload: &'a [u8],
}

impl RecvInfo<'_> {
    pub fn from_slice<'a>(slice: &'a [u8]) -> crate::Result<RecvInfo> {
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
