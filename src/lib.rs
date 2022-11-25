pub mod api;
pub mod bier;
pub mod header;

unsafe fn get_unchecked_be_u16(ptr: *const u8) -> u16 {
    u16::from_be_bytes([*ptr, *ptr.add(1)])
}

unsafe fn get_unchecked_be_u32(ptr: *const u8) -> u32 {
    u32::from_be_bytes([*ptr, *ptr.add(1), *ptr.add(2), *ptr.add(3)])
}

/// Custom result used for Bier processing.
pub type Result<T> = std::result::Result<T, Error>;

/// A BIER error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Impossible to parse the Bier header.
    Header,

    /// Invalid BIFT-ID.
    BiftId,

    /// Impossible to parse the BIFTs.
    BiftParsing,

    /// No entry in the BIFT.
    NoEntry,

    /// Wrong Bitstring length.
    BitstringLength,

    /// The buffer does not have the correct length for the BIER header.
    SliceWrongLength,
}
