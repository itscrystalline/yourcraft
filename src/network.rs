use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[repr(u8)]
pub enum PacketTypes {
    Invalid,
    HelloPacket,
    PlayerCoordinates
}

impl Into<u8> for PacketTypes {
    fn into(self) -> u8 {
        self as u8
    }
}
impl Into<PacketTypes> for u8 {
    fn into(self) -> PacketTypes {
        match self {
            0 => PacketTypes::HelloPacket,
            1 => PacketTypes::PlayerCoordinates,
            _ => PacketTypes::Invalid
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    pub t: u8,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>
}
#[derive(Serialize, Deserialize, Debug)]
pub struct HelloPacket {
    pub timestamp: u64
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PlayerCoordinates {
    pub x: i32,
    pub y: i32
}