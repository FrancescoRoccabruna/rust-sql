use crate::DbError;
use sha2::{Digest, Sha256};


pub struct Message {
    message_type: u8,
    sequence_id: u8,
    payload: Vec<u8>,
}


impl Message {

    pub fn new(message_type: u8, sequence_id: u8, payload: Vec<u8>) -> Self {
        Self {
            message_type,
            sequence_id,
            payload,
        }
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
        match self.message_type {
            0x00 => ServerMessage::Ok(self.payload),
            0x0A => ServerMessage::Handshake(self.payload),
            0xFF => ServerMessage::Error(self.payload),
            0xFE => ServerMessage::Eof(self.payload),

            _ => ServerMessage::Unknown(
                self.message_type,
                self.payload,
            ),
        }
    }

    pub fn encode(payload: &[u8], sequence_id: u8) -> Vec<u8> {
        let length = payload.len() as u32;

        let mut packet = Vec::new();

        packet.push((length & 0xFF) as u8);
        packet.push(((length >> 8) & 0xFF) as u8);
        packet.push(((length >> 16) & 0xFF) as u8);

        packet.push(sequence_id);

        packet.extend_from_slice(payload);

        packet
    }
}

pub enum ServerMessage {
    Handshake(Vec<u8>),
    Ok(Vec<u8>),
    Error(Vec<u8>),
    Eof(Vec<u8>),
    ResultSetHeader(Vec<u8>),
    ColumnDefinition(Vec<u8>),
    Row(Vec<u8>),
    Unknown(u8, Vec<u8>),
}

pub(crate) struct Handshake {
    pub(crate) server_version: String,
    pub(crate) connection_id: u32,
    pub(crate) capability_flags: u32,
    pub(crate) character_set: u8,
    pub(crate) status_flags: u16,
    pub(crate) auth_plugin_data: Vec<u8>,
    pub(crate) auth_plugin_name: String,
}

impl Handshake {
    pub fn parse(payload: &[u8]) -> Result<Self, DbError> {
        let mut offset = 0;

        let server_version =
            Self::read_null_string(payload, &mut offset)?;

        let connection_id =
            Self::read_u32_le(payload, &mut offset)?;

        if offset + 8 > payload.len() {
            return Err(DbError::new(
                String::from("Unexpected end of MySQL handshake")
            ));
        }

        let mut auth_plugin_data =
            payload[offset..offset + 8].to_vec();

        offset += 8;

        if offset >= payload.len() {
            return Err(DbError::new(
                String::from("Unexpected end of MySQL handshake")
            ));
        }

        // Filler
        offset += 1;

        let capability_flags_lower =
            Self::read_u16_le(payload, &mut offset)?;

        let character_set = payload
            .get(offset)
            .copied()
            .ok_or_else(|| DbError::new(
                String::from("Unexpected end of MySQL handshake")
            ))?;

        offset += 1;

        let status_flags =
            Self::read_u16_le(payload, &mut offset)?;

        let capability_flags_upper =
            Self::read_u16_le(payload, &mut offset)?;

        let capability_flags =
            (capability_flags_upper as u32) << 16
            | capability_flags_lower as u32;

        let auth_plugin_data_len = payload
            .get(offset)
            .copied()
            .ok_or_else(|| DbError::new(
                String::from("Unexpected end of MySQL handshake")
            ))?;

        offset += 1;

        // Reserved
        if offset + 10 > payload.len() {
            return Err(DbError::new(
                String::from("Unexpected end of MySQL handshake")
            ));
        }

        offset += 10;

        let auth_plugin_data_len =
            auth_plugin_data_len as usize;

        let remaining_auth_data_len =
            auth_plugin_data_len.saturating_sub(8);

        let available =
            payload.len() - offset;

        let length =
            remaining_auth_data_len.min(available);

        auth_plugin_data.extend_from_slice(
            &payload[offset..offset + length]
        );

        offset += length;

        while auth_plugin_data.last() == Some(&0) {
            auth_plugin_data.pop();
        }

        let auth_plugin_name =
            if offset < payload.len() {
                Self::read_null_string(payload, &mut offset)?
            } else {
                String::new()
            };

        Ok(Self {
            server_version,
            connection_id,
            capability_flags,
            character_set,
            status_flags,
            auth_plugin_data,
            auth_plugin_name,
        })
    }

    fn read_null_string(
        payload: &[u8],
        offset: &mut usize,
    ) -> Result<String, DbError> {
        let start = *offset;

        while *offset < payload.len() {
            if payload[*offset] == 0 {
                let value = std::str::from_utf8(&payload[start..*offset])
                    .map_err(|_| DbError::new(
                        String::from("Invalid UTF-8 in MySQL handshake")
                    ))?
                    .to_string();

                *offset += 1;

                return Ok(value);
            }

            *offset += 1;
        }

        Err(DbError::new(
            String::from("Missing NUL terminator in MySQL handshake")
        ))
    }

    fn read_u32_le(
        payload: &[u8],
        offset: &mut usize,
    ) -> Result<u32, DbError> {
        if *offset + 4 > payload.len() {
            return Err(DbError::new(
                String::from("Unexpected end of MySQL handshake")
            ));
        }

        let value = u32::from_le_bytes([
            payload[*offset],
            payload[*offset + 1],
            payload[*offset + 2],
            payload[*offset + 3],
        ]);

        *offset += 4;

        Ok(value)
    }

    fn read_u16_le(
        payload: &[u8],
        offset: &mut usize,
    ) -> Result<u16, DbError> {
        if *offset + 2 > payload.len() {
            return Err(DbError::new(
                String::from("Unexpected end of MySQL handshake")
            ));
        }

        let value = u16::from_le_bytes([
            payload[*offset],
            payload[*offset + 1],
        ]);

        *offset += 2;

        Ok(value)
    }

    pub fn scramble_password(
        password: &str,
        auth_plugin_data: &[u8],
    ) -> Vec<u8> {
        let password_hash = Sha256::digest(password.as_bytes());

        let password_hash_hash = Sha256::digest(password_hash);

        let mut input = Vec::new();
        input.extend_from_slice(&password_hash_hash);
        input.extend_from_slice(auth_plugin_data);

        let scramble_hash = Sha256::digest(&input);

        password_hash
            .iter()
            .zip(scramble_hash.iter())
            .map(|(a, b)| a ^ b)
            .collect()
    }
}