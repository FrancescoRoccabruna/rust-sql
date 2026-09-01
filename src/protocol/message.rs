use crate::protocol::message;

pub struct Message {
    message_type: u8,
    payload: Vec<u8>,
}

impl Message {

    pub fn new(message_type: u8, payload: Vec<u8>) -> Self {
        Self {
            message_type,
            payload,
        }
    }

    pub fn startup(
        username: &str,
        db_name: &str,
    ) -> Vec<u8> {
        let mut message = Vec::new();

        // Placeholder per la lunghezza
        message.extend_from_slice(&[0, 0, 0, 0]);

        // Protocol version 3.0
        message.extend_from_slice(&196608u32.to_be_bytes());

        message.extend_from_slice(b"user");
        message.push(0);

        message.extend_from_slice(username.as_bytes());
        message.push(0);

        message.extend_from_slice(b"database");
        message.push(0);

        message.extend_from_slice(db_name.as_bytes());
        message.push(0);

        // Terminatore dei parametri
        message.push(0);

        // Ora conosciamo la lunghezza
        let length = message.len() as u32;

        message[0..4].copy_from_slice(&length.to_be_bytes());

        message
    }

    pub fn sasl_initial_response(
        mechanism: &str,
        client_message: &str,
    ) -> Vec<u8> {

        let mut message = Vec::new();

        // Message type
        message.push(b'p');

        // Placeholder per length
        message.extend_from_slice(&[0, 0, 0, 0]);

        // SASL mechanism
        message.extend_from_slice(mechanism.as_bytes());
        message.push(0);

        // Length del client-first-message
        let response_length = client_message.len() as u32;
        message.extend_from_slice(&response_length.to_be_bytes());

        // Client-first-message
        message.extend_from_slice(client_message.as_bytes());

        // Length totale del messaggio, escluso il byte 'p'
        let length = (message.len() - 1) as u32;

        message[1..5].copy_from_slice(&length.to_be_bytes());

        message
    }

    pub fn sasl_response(
        client_message: &str,
    ) -> Vec<u8> {
        let mut message = Vec::new();

        // Message type
        message.push(b'p');

        // Placeholder per length
        message.extend_from_slice(&[0, 0, 0, 0]);

        // Client-final-message
        message.extend_from_slice(client_message.as_bytes());

        // Length totale del messaggio, escluso il byte 'p'
        let length = (message.len() - 1) as u32;

        message[1..5].copy_from_slice(&length.to_be_bytes());

        message
    }

    pub fn parse(self) -> ServerMessage {
        match self.message_type {
            b'R' => ServerMessage::Authentication(self.payload),
            b'S' => ServerMessage::ParameterStatus(self.payload),
            b'K' => ServerMessage::BackendKeyData(self.payload),
            b'Z' => ServerMessage::ReadyForQuery(self.payload),
            b'E' => ServerMessage::ErrorResponse(self.payload),
            other => ServerMessage::Unknown(other, self.payload),
        }
    }
}

pub enum ServerMessage {
    Authentication(Vec<u8>),
    ParameterStatus(Vec<u8>),
    BackendKeyData(Vec<u8>),
    ReadyForQuery(Vec<u8>),
    ErrorResponse(Vec<u8>),
    Unknown(u8, Vec<u8>),
}