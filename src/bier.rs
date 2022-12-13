use crate::{Error, Result};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{net::IpAddr, str::FromStr};

pub type BierSendInfo = (Bitstring, Option<IpAddr>);

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
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

    pub fn get_loopback(&self) -> IpAddr {
        self.loopback
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct Bift {
    pub bift_id: usize,
    pub bift_type: BiftType,
    pub bfr_id: u64,
    pub entries: Vec<BiftEntry>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct BiftEntry {
    /// Bit representing the router of the entry.
    pub bit: u64,
    /// All (Bitstring, next-hop) pairsfor this bit.
    pub paths: Vec<BierEntryPath>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BierEntryPath {
    pub bitstring: Bitstring,
    pub next_hop: IpAddr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitstring {
    pub bitstring: Vec<u64>,
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

    pub fn update_header_from_self(&self, header: &mut [u8]) -> Result<()> {
        if header.len() < crate::header::BIER_HEADER_WITHOUT_BITSTRING_LENGTH + self.bitstring.len()
        {
            return Err(Error::BitstringLength);
        }

        // Get the bitstring.
        let bitstring_hdr = &mut header[crate::header::BIER_HEADER_WITHOUT_BITSTRING_LENGTH
            ..crate::header::BIER_HEADER_WITHOUT_BITSTRING_LENGTH + self.bitstring.len() * 8];

        unsafe {
            let bitstring: Vec<u64> = self.bitstring.iter().map(|item| item.to_be()).collect();
            let p = bitstring.as_ptr() as *const u8;
            let slice = std::slice::from_raw_parts(p, self.bitstring.len() * 8);
            bitstring_hdr.copy_from_slice(slice);
        }

        Ok(())
    }

    pub fn is_valid(slice: &[u8]) -> bool {
        matches!(slice.len(), 8 | 16 | 32 | 64 | 128 | 256)
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

impl Serialize for Bitstring {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let a: String = self
            .bitstring
            .iter()
            .rev()
            .fold(String::new(), |s, v| s + &format!("{:064b}", v));
        serializer.serialize_str(&a)
    }
}

impl From<Vec<u64>> for Bitstring {
    fn from(slice: Vec<u64>) -> Self {
        Bitstring { bitstring: slice }
    }
}

impl TryFrom<&[u8]> for Bitstring {
    type Error = crate::Error;

    fn try_from(value: &[u8]) -> crate::Result<Self> {
        if !Bitstring::is_valid(value) {
            return Err(crate::Error::BitstringLength);
        }

        Ok(Bitstring {
            bitstring: {
                unsafe {
                    let p = value.as_ptr() as *mut u64;
                    let slice = std::slice::from_raw_parts(p, value.len() / 8);
                    slice.iter().map(|item| item.to_be()).collect()
                }
            },
        })
    }
}

impl FromStr for Bitstring {
    type Err = String;

    fn from_str(str_bitstring: &str) -> std::result::Result<Self, Self::Err> {
        let len_of_64_bits = (str_bitstring.len() as f64 / 64.0).ceil() as usize;

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

impl From<&Bitstring> for Vec<u8> {
    fn from(bitstring: &Bitstring) -> Self {
        bitstring
            .bitstring
            .iter()
            .flat_map(|elem| elem.to_be_bytes())
            .collect()
    }
}

#[derive(Deserialize_repr, Serialize_repr, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum BiftType {
    Bier = 1,
    BierTe = 2,
}

pub enum BitstringOp {
    And = 1,
    AndNot = 2,
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

    #[test]
    /// Tests that the update_header_from_self() method of the Bitstring struct
    /// correctly encodes a new bitstring in a packet slice.
    fn test_bitstring_update_header_from_self() {
        let bitstring = Bitstring::from_str("1101");
        assert!(bitstring.is_ok());
        let bitstring = bitstring.unwrap();

        // Get dummy header.
        let mut header = crate::header::tests::get_dummy_bier_header_slice();

        // Modify the bitstring of the header.
        assert!(bitstring.update_header_from_self(&mut header).is_ok());

        // The bitstring is correctly updated.
        let expected = [0u8, 0, 0, 0, 0, 0, 0, 0b1101];
        assert_eq!(expected, header[12..]);

        // The remaining of the header is the same.
        let expected = crate::header::tests::get_dummy_bier_header_slice();
        assert_eq!(expected[..12], header[..12]);
    }

    #[test]
    /// Tests the function returning if a bitstring given as input is valid
    /// following RFC 8279.
    fn test_bitstring_is_valid() {
        for i in 0..6 {
            let bitstring = vec![0u8; 8 << i];
            assert!(Bitstring::is_valid(&bitstring[..]));
            assert!(!Bitstring::is_valid(&bitstring[1..]));
        }
    }

    #[test]
    /// Tests the parsing of bitstring from &[u8].
    fn test_bitstring_from_slice_u8() {
        let raw = [0u8, 0, 0, 0, 0, 0, 0, 1];
        let res: Result<Bitstring> = raw.as_ref().try_into();
        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res.bitstring.len(), 1);
        assert_eq!(res.bitstring[0], 1);

        let raw = [0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff];
        let res: Result<Bitstring> = raw.as_ref().try_into();
        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res.bitstring.len(), 2);
        assert_eq!(res.bitstring[0], 0);
        assert_eq!(res.bitstring[1], 0xffff);

        // Wrong bitstring.
        let raw = [0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff];
        let res: Result<Bitstring> = raw.as_ref().try_into();
        assert!(res.is_err());

        let raw = [0u8, 0, 0, 0, 0xff, 0xff];
        let res: Result<Bitstring> = raw.as_ref().try_into();
        assert!(res.is_err());

        let raw: [u8; 0] = [];
        let res: Result<Bitstring> = raw.as_ref().try_into();
        assert!(res.is_err());
    }

    #[test]
    /// Tests the conversion of a Bitstring to a Vec<u8> method.
    fn test_vec_u8_from_bitstring() {
        let raw = [0u8, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let res: Result<Bitstring> = raw.as_ref().try_into();
        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res.bitstring.len(), 2);

        // Convert to an array of u8.
        let res_u8: Vec<u8> = (&res).into();
        assert_eq!(res_u8.len(), 16);
        assert_eq!(res_u8, raw);
    }

    #[test]
    /// Tests the serialization of a BIFT.
    /// This test assumes that the deserialization of a BIFT works.
    /// The result of this test is only relevant if the `test_deserialize test works as expected.
    fn test_bift_serialize() {
        let txt = get_dummy_config_json();
        let bier_state: BierState = serde_json::from_str(txt).unwrap();

        // Serialize using the tested procedure.
        let res = serde_json::to_string(&bier_state);
        assert!(res.is_ok());
        let res = res.unwrap();

        // Now deserialize back to get another copy of the structure.
        let bier_state_after: BierState = serde_json::from_str(&res).unwrap();

        // Assuming that the deserialization works, if we get the same content
        // between bier_state and bier_state_after, it means that the serialization works.
        assert_eq!(bier_state, bier_state_after);
    }
}
