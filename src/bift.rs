use serde::{de, Deserialize, Deserializer};
use serde_json::Value;
use serde_repr::Deserialize_repr;
use std::{net::IpAddr, num::ParseIntError, str::FromStr};

#[derive(Deserialize, Debug)]
pub struct BierState {
    loopback: IpAddr,
    bifts: Vec<Bift>,
}

#[derive(Deserialize, Debug)]
pub struct Bift {
    bift_id: u32,
    bift_type: BiftType,
    bfr_id: u64,
    entries: Vec<BiftEntry>,
}

#[derive(Deserialize, Debug)]
pub struct BiftEntry {
    /// Bit representing the router of the entry.
    bit: u64,
    /// All (bitmask, next-hop) pairsfor this bit.
    paths: Vec<BierEntryPath>,
}

#[derive(Debug, Deserialize)]
struct BierEntryPath {
    bitmask: Bitmask,
    next_hop: IpAddr,
}

#[derive(Debug)]
pub struct Bitmask {
    bitmask: Vec<u64>,
}

impl<'de> Deserialize<'de> for Bitmask {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        println!("TENTE ICI");
        let s = String::deserialize(deserializer)?;
        println!("This is the string: {}", &s);
        let s = FromStr::from_str(&s).map_err(de::Error::custom);
        println!("This is the result: {:?}", s);
        s
    }
}

impl FromStr for Bitmask {
    type Err = String;

    fn from_str(str_bitmask: &str) -> Result<Self, Self::Err> {
        let len_of_64_bits = (str_bitmask.len() as f64 / 8.0).ceil() as usize;

        match (0..len_of_64_bits)
            .map(|i| {
                let lower_bound = match str_bitmask.len().checked_sub(64 * (i + 1)) {
                    Some(v) => v,
                    None => 0,
                };
                let upper_bound = usize::min(lower_bound + 64, str_bitmask.len());
                let substr = &str_bitmask[lower_bound..upper_bound];
                println!("This is the substr: {}", substr);
                u64::from_str_radix(substr, 2)
            })
            .collect()
        {
            Ok(v) => Ok(Bitmask { bitmask: v }),
            Err(e) => Err(format!("Impossible to parse: {:?}", e)),
        }
    }
}

#[derive(Deserialize_repr, PartialEq, Debug)]
#[repr(u32)]
pub enum BiftType {
    Bier = 1,
    BierTe = 2,
}

#[cfg(test)]
mod tests {
    use serde_json::{from_reader, from_value, Value};

    use super::{BierState, BiftType};
    use std::net::{IpAddr, Ipv6Addr};

    #[test]
    fn test_deserialize() {
        let txt = r#"{"loopback": "fc00::a","bifts": [
                {
                    "bift_id": 1,
                    "bift_type": 1,
                    "bfr_id": 1,
                    "entries": [
                        {
                            "bit": 1,
                            "paths": [
                                {
                                    "bitmask": "1",
                                    "next_hop": "fc00:a::1"
                                }
                            ]
                        },
                        {
                            "bit": 2,
                            "paths": [
                                {
                                    "bitmask": "11010",
                                    "next_hop": "fc00:b::1"
                                }
                            ]
                        },
                        {
                            "bit": 3,
                            "paths": [
                                {
                                    "bitmask": "11100",
                                    "next_hop": "fc00:c::1"
                                }
                            ]
                        },
                        {
                            "bit": 4,
                            "paths": [
                                {
                                    "bitmask": "11010",
                                    "next_hop": "fc00:b::1"
                                },
                                {
                                    "bitmask": "11100",
                                    "next_hop": "fc00:c::1"
                                }
                            ]
                        },
                        {
                            "bit": 5,
                            "paths": [
                                {
                                    "bitmask": "11010",
                                    "next_hop": "fc00:b::1"
                                },
                                {
                                    "bitmask": "11100",
                                    "next_hop": "fc00:c::1"
                                }
                            ]
                        }
                    ]
                }
            ]
        }
        "#;
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
        assert_eq!(bift.entries[0].paths[0].bitmask.bitmask.len(), 1);
        assert_eq!(bift.entries[0].paths[0].bitmask.bitmask[0], 1);
        assert_eq!(
            bift.entries[0].paths[0].next_hop,
            IpAddr::V6("fc00:a::1".parse().unwrap())
        );

        // Entry 2.
        assert_eq!(bift.entries[1].bit, 2);
        assert_eq!(bift.entries[1].paths.len(), 1);
        assert_eq!(bift.entries[1].paths[0].bitmask.bitmask.len(), 1);
        assert_eq!(bift.entries[1].paths[0].bitmask.bitmask[0], 26);
        assert_eq!(
            bift.entries[1].paths[0].next_hop,
            IpAddr::V6("fc00:b::1".parse().unwrap())
        );

        // Entry 3.
        assert_eq!(bift.entries[2].bit, 3);
        assert_eq!(bift.entries[2].paths.len(), 1);
        assert_eq!(bift.entries[2].paths[0].bitmask.bitmask.len(), 1);
        assert_eq!(bift.entries[2].paths[0].bitmask.bitmask[0], 28);
        assert_eq!(
            bift.entries[2].paths[0].next_hop,
            IpAddr::V6("fc00:c::1".parse().unwrap())
        );

        // Entry 4.
        assert_eq!(bift.entries[3].bit, 4);
        assert_eq!(bift.entries[3].paths.len(), 2);
        assert_eq!(bift.entries[3].paths[0].bitmask.bitmask.len(), 1);
        assert_eq!(bift.entries[3].paths[0].bitmask.bitmask[0], 26);
        assert_eq!(
            bift.entries[3].paths[0].next_hop,
            IpAddr::V6("fc00:b::1".parse().unwrap())
        );
        assert_eq!(bift.entries[3].paths[1].bitmask.bitmask.len(), 1);
        assert_eq!(bift.entries[3].paths[1].bitmask.bitmask[0], 28);
        assert_eq!(
            bift.entries[3].paths[1].next_hop,
            IpAddr::V6("fc00:c::1".parse().unwrap())
        );

        // Entry 5.
        assert_eq!(bift.entries[4].bit, 5);
        assert_eq!(bift.entries[4].paths.len(), 2);
        assert_eq!(bift.entries[4].paths[0].bitmask.bitmask.len(), 1);
        assert_eq!(bift.entries[4].paths[0].bitmask.bitmask[0], 26);
        assert_eq!(
            bift.entries[4].paths[0].next_hop,
            IpAddr::V6("fc00:b::1".parse().unwrap())
        );
        assert_eq!(bift.entries[4].paths[1].bitmask.bitmask.len(), 1);
        assert_eq!(bift.entries[4].paths[1].bitmask.bitmask[0], 28);
        assert_eq!(
            bift.entries[4].paths[1].next_hop,
            IpAddr::V6("fc00:c::1".parse().unwrap())
        );
    }
}
