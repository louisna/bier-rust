use serde::{de, Deserialize, Deserializer};
use serde_repr::Deserialize_repr;
use std::{net::IpAddr, str::FromStr};

pub type BierSendInfo = (Bitstring, Option<IpAddr>);

#[derive(Deserialize, Debug)]
pub struct BierState {
    loopback: IpAddr,
    bifts: Vec<Bift>,
}

impl BierState {
    pub fn process_bier(
        &self,
        original_bitstring: &Bitstring,
        bift_id: u32,
    ) -> Result<Vec<BierSendInfo>> {
        let bift_id = bift_id as usize;

        // Make a copy that will be edited during the processing.
        let mut bitstring = original_bitstring.clone();

        let mut out = Vec::new();
        let bift = self.bifts.get(bift_id - 1).ok_or(Error::BiftId)?;
        // TODO: is the vector correctly indexed?
        assert_eq!(bift.bift_id, bift_id);

        // TODO: currently only supports BIER (RFC8279).
        assert_eq!(bift.bift_type, BiftType::Bier);

        let bitstring_number_u64 = bitstring.bitstring.len();
        let mut bfr_idx = 0;

        // Iterate over all u64 words.
        for idx_u64_word in 0..bitstring_number_u64 {
            let mut bitstring_word = bitstring.bitstring[bitstring_number_u64 - 1 - idx_u64_word];

            // Iterate over all bits of the word.
            while bitstring_word > 0 {
                // The `bfr_idx` BFR has its bit set to 1. Process.
                if ((bitstring_word >> (bfr_idx % 64)) & 1) == 1 {
                    // Bitstring for this packet duplication.
                    let mut dst_bitstring = bitstring.clone();
                    let bift_entry = bift.entries.get(bfr_idx).ok_or(Error::NoEntry)?;
                    // TODO: is the vector correctly indexed?
                    assert_eq!(bift_entry.bit - 1, bfr_idx as u64);

                    // Get the first path always.
                    let bier_entry_path = bift_entry.paths.get(0).ok_or(Error::NoEntry)?;

                    // Update the bitstring with the bitmask of the corresponding entry.
                    dst_bitstring.update(&bier_entry_path.bitstring, BitstringOp::And);

                    // Add new destination.
                    // `None` if the packet must be sent to the local BFER.
                    let nxt_hop_ip = if bfr_idx as u64 == bift.bfr_id - 1 {
                        None
                    } else {
                        Some(bier_entry_path.next_hop)
                    };
                    out.push((dst_bitstring, nxt_hop_ip));

                    // Update global bitstring.
                    bitstring.update(&bier_entry_path.bitstring, BitstringOp::AndNot);

                    // Update the iterated bitstring word in case we cleaned some bits.
                    bitstring_word = bitstring.bitstring[bitstring_number_u64 - 1 - idx_u64_word];
                }
                // Next BFR.
                bfr_idx += 1;
            }
        }

        Ok(out)
    }

    // pub fn

    pub fn get_loopback(&self) -> IpAddr {
        self.loopback
    }
}

#[derive(Deserialize, Debug)]
pub struct Bift {
    bift_id: usize,
    bift_type: BiftType,
    bfr_id: u64,
    entries: Vec<BiftEntry>,
}

#[derive(Deserialize, Debug)]
pub struct BiftEntry {
    /// Bit representing the router of the entry.
    bit: u64,
    /// All (Bitstring, next-hop) pairsfor this bit.
    paths: Vec<BierEntryPath>,
}

#[derive(Debug, Deserialize)]
struct BierEntryPath {
    bitstring: Bitstring,
    next_hop: IpAddr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitstring {
    bitstring: Vec<u64>,
}

impl Bitstring {
    pub fn update(&mut self, other: &Bitstring, bitop: BitstringOp) {
        self.bitstring = self
            .bitstring
            .iter()
            .zip(other.bitstring.iter())
            .map(|(bw_self, bw_other)| match bitop {
                BitstringOp::And => bw_self & bw_other,
                BitstringOp::AndNot => bw_self & !bw_other,
            })
            .collect();
    }

    pub fn get_ref(&self) -> &[u64] {
        &self.bitstring
    }

    pub fn update_header_from_self(&self, header: &mut [u8]) -> Result<()> {
        if header.len() < crate::header::BIER_HEADER_WITHOUT_BITSTRING_LENGTH + self.bitstring.len() {
            return Err(crate::bier::Error::BitstringLength);
        }

        // Get the bitstring.
        let bitstring_hdr = &mut header[crate::header::BIER_HEADER_WITHOUT_BITSTRING_LENGTH
        ..crate::header::BIER_HEADER_WITHOUT_BITSTRING_LENGTH + self.bitstring.len()];

        unsafe {
            let p = self.bitstring.as_ptr() as *const u8;
            let slice = std::slice::from_raw_parts(p, self.bitstring.len() * 8);
            bitstring_hdr.copy_from_slice(slice);
        }
        
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Bitstring {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl From<Vec<u64>> for Bitstring {
    fn from(slice: Vec<u64>) -> Self {
        Bitstring { bitstring: slice }
    }
}

impl FromStr for Bitstring {
    type Err = String;

    fn from_str(str_bitstring: &str) -> std::result::Result<Self, Self::Err> {
        let len_of_64_bits = (str_bitstring.len() as f64 / 8.0).ceil() as usize;

        match (0..len_of_64_bits)
            .map(|i| {
                let lower_bound = str_bitstring.len().saturating_sub(64 * (i + 1));
                let upper_bound = usize::min(lower_bound + 64, str_bitstring.len());
                let substr = &str_bitstring[lower_bound..upper_bound];
                u64::from_str_radix(substr, 2)
            })
            .collect()
        {
            Ok(v) => Ok(Bitstring { bitstring: v }),
            Err(e) => Err(format!("Impossible to parse: {:?}", e)),
        }
    }
}

#[derive(Deserialize_repr, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum BiftType {
    Bier = 1,
    BierTe = 2,
}

pub enum BitstringOp {
    And = 1,
    AndNot = 2,
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
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::net::IpAddr;

    fn get_dummy_config_json() -> &'static str {
        r#"{"loopback": "fc00::a","bifts": [
                {
                    "bift_id": 1,
                    "bift_type": 1,
                    "bfr_id": 1,
                    "entries": [
                        {
                            "bit": 1,
                            "paths": [
                                {
                                    "bitstring": "1",
                                    "next_hop": "fc00:a::1"
                                }
                            ]
                        },
                        {
                            "bit": 2,
                            "paths": [
                                {
                                    "bitstring": "11010",
                                    "next_hop": "fc00:b::1"
                                }
                            ]
                        },
                        {
                            "bit": 3,
                            "paths": [
                                {
                                    "bitstring": "11100",
                                    "next_hop": "fc00:c::1"
                                }
                            ]
                        },
                        {
                            "bit": 4,
                            "paths": [
                                {
                                    "bitstring": "11010",
                                    "next_hop": "fc00:b::1"
                                },
                                {
                                    "bitstring": "11100",
                                    "next_hop": "fc00:c::1"
                                }
                            ]
                        },
                        {
                            "bit": 5,
                            "paths": [
                                {
                                    "bitstring": "11010",
                                    "next_hop": "fc00:b::1"
                                },
                                {
                                    "bitstring": "11100",
                                    "next_hop": "fc00:c::1"
                                }
                            ]
                        }
                    ]
                }
            ]
        }
        "#
    }

    #[test]
    /// Tests the JSON deserialize of a BIFT.
    fn test_deserialize() {
        let txt = get_dummy_config_json();
        let bier_state: BierState = serde_json::from_str(txt).unwrap();

        assert_eq!(bier_state.loopback, IpAddr::V6("fc00::a".parse().unwrap()));
        assert_eq!(bier_state.bifts.len(), 1);

        let bift = bier_state.bifts.get(0).unwrap();
        assert_eq!(bift.bfr_id, 1);
        assert_eq!(bift.bift_type, BiftType::Bier);
        assert_eq!(bift.bfr_id, 1);
        assert_eq!(bift.entries.len(), 5);

        // Entry 1.
        assert_eq!(bift.entries[0].bit, 1);
        assert_eq!(bift.entries[0].paths.len(), 1);
        assert_eq!(bift.entries[0].paths[0].bitstring.bitstring.len(), 1);
        assert_eq!(bift.entries[0].paths[0].bitstring.bitstring[0], 1);
        assert_eq!(
            bift.entries[0].paths[0].next_hop,
            IpAddr::V6("fc00:a::1".parse().unwrap())
        );

        // Entry 2.
        assert_eq!(bift.entries[1].bit, 2);
        assert_eq!(bift.entries[1].paths.len(), 1);
        assert_eq!(bift.entries[1].paths[0].bitstring.bitstring.len(), 1);
        assert_eq!(bift.entries[1].paths[0].bitstring.bitstring[0], 26);
        assert_eq!(
            bift.entries[1].paths[0].next_hop,
            IpAddr::V6("fc00:b::1".parse().unwrap())
        );

        // Entry 3.
        assert_eq!(bift.entries[2].bit, 3);
        assert_eq!(bift.entries[2].paths.len(), 1);
        assert_eq!(bift.entries[2].paths[0].bitstring.bitstring.len(), 1);
        assert_eq!(bift.entries[2].paths[0].bitstring.bitstring[0], 28);
        assert_eq!(
            bift.entries[2].paths[0].next_hop,
            IpAddr::V6("fc00:c::1".parse().unwrap())
        );

        // Entry 4.
        assert_eq!(bift.entries[3].bit, 4);
        assert_eq!(bift.entries[3].paths.len(), 2);
        assert_eq!(bift.entries[3].paths[0].bitstring.bitstring.len(), 1);
        assert_eq!(bift.entries[3].paths[0].bitstring.bitstring[0], 26);
        assert_eq!(
            bift.entries[3].paths[0].next_hop,
            IpAddr::V6("fc00:b::1".parse().unwrap())
        );
        assert_eq!(bift.entries[3].paths[1].bitstring.bitstring.len(), 1);
        assert_eq!(bift.entries[3].paths[1].bitstring.bitstring[0], 28);
        assert_eq!(
            bift.entries[3].paths[1].next_hop,
            IpAddr::V6("fc00:c::1".parse().unwrap())
        );

        // Entry 5.
        assert_eq!(bift.entries[4].bit, 5);
        assert_eq!(bift.entries[4].paths.len(), 2);
        assert_eq!(bift.entries[4].paths[0].bitstring.bitstring.len(), 1);
        assert_eq!(bift.entries[4].paths[0].bitstring.bitstring[0], 26);
        assert_eq!(
            bift.entries[4].paths[0].next_hop,
            IpAddr::V6("fc00:b::1".parse().unwrap())
        );
        assert_eq!(bift.entries[4].paths[1].bitstring.bitstring.len(), 1);
        assert_eq!(bift.entries[4].paths[1].bitstring.bitstring[0], 28);
        assert_eq!(
            bift.entries[4].paths[1].next_hop,
            IpAddr::V6("fc00:c::1".parse().unwrap())
        );
    }

    #[test]
    /// Tests the update of a bitstring.
    fn test_update_bitstring() {
        let bitstring = Bitstring::from_str("1101");
        assert!(bitstring.is_ok());
        let mut bitstring = bitstring.unwrap();

        bitstring.update(&Bitstring::from_str("1011").unwrap(), BitstringOp::And);
        assert_eq!(bitstring.bitstring[0], 0b1001);

        bitstring.update(&Bitstring::from_str("0011").unwrap(), BitstringOp::AndNot);
        assert_eq!(bitstring.bitstring[0], 0b1000);
    }

    #[test]
    /// Tests the BIER processing of a bitstring using the dummy BIFT.
    fn test_bier_processing() {
        let txt = get_dummy_config_json();
        let bier_state: BierState = serde_json::from_str(txt).unwrap();

        let bitstring = Bitstring::from_str("11111");
        assert!(bitstring.is_ok());
        let bitstring = bitstring.unwrap();
        // TODO: test also with invalid bitstring length (e.g., longer).

        let outputs = bier_state.process_bier(&bitstring, 1);
        assert!(outputs.is_ok());
        let outputs = outputs.unwrap();

        // Considering the example BIFT and the full-set bitstring, we should have three different paths.
        assert_eq!(outputs.len(), 3);

        let expected = [
            (Bitstring::from_str("1").unwrap(), None), // Local bitstring.
            (
                Bitstring::from_str("11010").unwrap(),
                Some(IpAddr::V6("fc00:b::1".parse().unwrap())),
            ), // Going to node B.
            (
                Bitstring::from_str("100").unwrap(),
                Some(IpAddr::V6("fc00:c::1".parse().unwrap())),
            ), // going to node C.
        ];

        let res = expected.iter().map(|out| outputs.contains(out)).all(|v| v);
        assert!(res);
    }

    #[test]
    /// Tests the BIER processing of a bitstring using the dummy BIFT.
    fn test_bier_processing_2() {
        let txt = get_dummy_config_json();
        let bier_state: BierState = serde_json::from_str(txt).unwrap();

        let bitstring = Bitstring::from_str("11000");
        assert!(bitstring.is_ok());
        let bitstring = bitstring.unwrap();
        // TODO: test also with invalid bitstring length (e.g., longer).

        let outputs = bier_state.process_bier(&bitstring, 1);
        assert!(outputs.is_ok());
        let outputs = outputs.unwrap();

        // Considering the example BIFT and the full-set bitstring, we should have three different paths.
        assert_eq!(outputs.len(), 1);

        let expected = [
            (
                Bitstring::from_str("11000").unwrap(),
                Some(IpAddr::V6("fc00:b::1".parse().unwrap())),
            ), // Going to node B.
        ];

        let res = expected.iter().map(|out| outputs.contains(out)).all(|v| v);
        assert!(res);
    }
}
