pub mod deserialize;
pub mod serialize;
pub mod types;

pub use types::{Message, ResourceInfo};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Frame;
    use crate::protocol::message::types::ResourceId;

    #[test]
    fn test_hello_serialization() {
        let pk = [1u8; 32];
        let nonce = [2u8; 32];
        let msg = Message::Hello {
            public_key: pk,
            nonce,
        };
        let frame: Frame = (&msg).into();
        assert_eq!(frame.message_type, 1);
        let decoded = Message::try_from(frame).unwrap();
        match decoded {
            Message::Hello {
                public_key,
                nonce: d_nonce,
            } => {
                assert_eq!(public_key, pk);
                assert_eq!(d_nonce, nonce);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_resource_list_serialization() {
        let r1 = ResourceInfo {
            id: ResourceId([3u8; 32]),
            name: "test.txt".to_string(),
            size: 1024,
        };
        let msg = Message::ResourceList {
            request_id: 42,
            resources: vec![r1.clone()],
        };
        let frame: Frame = (&msg).into();
        assert_eq!(frame.message_type, 4);

        let decoded = Message::try_from(frame).unwrap();
        match decoded {
            Message::ResourceList {
                request_id,
                resources,
            } => {
                assert_eq!(request_id, 42);
                assert_eq!(resources.len(), 1);
                assert_eq!(resources[0].name, r1.name);
                assert_eq!(resources[0].size, r1.size);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
