pub const OK_PACKET: u8 = 0x00;
pub const ERR_PACKET: u8 = 0xFF;

pub struct Message {
    pub(crate) sequence_id: u8,
    pub(crate) payload: Vec<u8>,
}

impl Message {
    pub fn new(sequence_id: u8, payload: Vec<u8>) -> Self {
        Self {
            sequence_id,
            payload,
        }
    }

    pub fn message_type(&self) -> Option<u8> {
        self.payload.first().copied()
    }

    pub fn handshake_response(
        username: &str,
        auth_response: &[u8],
        db_name: &str,
        capability_flags: u32,
        character_set: u8,
        auth_plugin_name: &str,
    ) -> Vec<u8> {
        let mut payload = Vec::new();

        // Client capabilities
        payload.extend_from_slice(&capability_flags.to_le_bytes());

        // Maximum packet size
        payload.extend_from_slice(&0u32.to_le_bytes());

        // Character set
        payload.push(character_set);

        // Reserved
        payload.extend_from_slice(&[0u8; 23]);

        // Username
        payload.extend_from_slice(username.as_bytes());
        payload.push(0);

        // Auth response length
        payload.push(auth_response.len() as u8);

        // Auth response
        payload.extend_from_slice(auth_response);

        // Database
        payload.extend_from_slice(db_name.as_bytes());
        payload.push(0);

        // Authentication plugin name
        payload.extend_from_slice(auth_plugin_name.as_bytes());
        payload.push(0);

        payload
    }

    pub fn parse(self) -> ServerMessage {
        let message_type = self.message_type().unwrap_or(0xFF);

        match message_type {
            0x00 => ServerMessage::Ok(self.payload),

            0x0A => ServerMessage::Handshake(self.payload),

            0xFF => ServerMessage::Error(self.payload),

            0xFE => ServerMessage::Eof(self.payload),

            0x01 => ServerMessage::AuthMoreData(self.payload),

            _ => ServerMessage::Unknown(message_type, self.payload),
        }
    }

    pub fn encode(payload: &[u8], sequence_id: u8) -> Vec<u8> {
        let length = payload.len() as u32;

        let mut packet = vec![
            (length & 0xFF) as u8, // Payload length: 3-byte little endian
            ((length >> 8) & 0xFF) as u8,
            ((length >> 16) & 0xFF) as u8,
            sequence_id, // Sequence ID
        ];

        // Payload
        packet.extend_from_slice(payload);

        packet
    }

    pub fn query(sql: &str) -> Vec<u8> {
        let mut payload = Vec::new();

        // COM_QUERY
        payload.push(0x03);

        payload.extend_from_slice(sql.as_bytes());

        payload
    }
}

#[expect(dead_code)]
pub enum ServerMessage {
    Handshake(Vec<u8>),
    Ok(Vec<u8>),
    Error(Vec<u8>),
    Eof(Vec<u8>),
    ResultSetHeader(Vec<u8>),
    ColumnDefinition(Vec<u8>),
    Row(Vec<u8>),
    AuthMoreData(Vec<u8>),
    Unknown(u8, Vec<u8>),
}
