pub mod deserialize;
pub mod serialize;
pub mod types;

pub use types::Message;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Frame;

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
    fn test_send_resource_request_serialization() {
        let msg = Message::SendResourceRequest {
            request_id: 42,
            name: "test.txt".to_string(),
            size: 1024,
        };
        let frame: Frame = (&msg).into();
        assert_eq!(frame.message_type, 3); // 3 is SendResourceRequest

        let decoded = Message::try_from(frame).unwrap();
        match decoded {
            Message::SendResourceRequest {
                request_id,
                name,
                size,
            } => {
                assert_eq!(request_id, 42);
                assert_eq!(name, "test.txt");
                assert_eq!(size, 1024);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
