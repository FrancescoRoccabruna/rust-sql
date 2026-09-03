use crate::DbError;


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

    pub fn handshake_response() -> Vec<u8> {
        todo!()
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

pub struct Handshake {
    server_version: String,
    connection_id: u32,
    capability_flags: u32,
    character_set: u8,
    status_flags: u16,
    auth_plugin_data: Vec<u8>,
    auth_plugin_name: String,
}

impl Handshake {
    pub fn parse(payload: &[u8]) -> Result<Self, DbError> {
        let mut offset = 0;

        let server_version =
            Self::read_null_string(payload, &mut offset)?;

        let connection_id =
            Self::read_u32_le(payload, &mut offset)?;

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

        if offset + 23 > payload.len() {
            return Err(DbError::new(
                String::from("Unexpected end of MySQL handshake")
            ));
        }

        offset += 23;

        let auth_plugin_data_len = payload
            .get(offset)
            .copied()
            .ok_or_else(|| DbError::new(
                String::from("Unexpected end of MySQL handshake")
            ))?;

        offset += 1;

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

        offset += 1;

        let remaining_auth_data_len =
            auth_plugin_data_len.saturating_sub(8);

        let remaining_auth_data_len =
            remaining_auth_data_len.saturating_sub(1);

        /*let available = payload.len() - offset;
        let length = remaining_auth_data_len.min(available);

        auth_plugin_data.extend_from_slice(
            &payload[offset..offset + length]
        );

        offset += length;*/

        todo!()
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
}