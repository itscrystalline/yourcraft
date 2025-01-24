use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    pub t: u8,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

// https://chatgpt.com/share/67910f66-8c24-8006-bf28-7bc00ff905ed
macro_rules! define_packets {
    (
        $(
            $name:ident = $value:expr => $struct:ident {
                $($field_name:ident: $field_type:ty),* $(,)?
            }
        ),* $(,)?
    ) => {
        #[derive(Serialize, Deserialize, Debug)]
        #[repr(u8)]
        pub enum PacketTypes {
            Invalid = 0,
            $($name = $value),*
        }

        impl Into<u8> for PacketTypes {
            fn into(self) -> u8 {
                self as u8
            }
        }

        impl Into<PacketTypes> for u8 {
            fn into(self) -> PacketTypes {
                match self {
                    $($value => PacketTypes::$name),*,
                    _ => PacketTypes::Invalid,
                }
            }
        }

        $(
            #[derive(Serialize, Deserialize, Debug)]
            pub struct $struct {
                $($field_name: $field_type),*
            }
        )*
    };
}

// Use the macro to define packets
define_packets!(
    ClientHello = 1 => ClientHello {
        timestamp: u64
    },
    ServerSync = 2 => ServerSync {
        x: i32,
        y: i32
    }
);
