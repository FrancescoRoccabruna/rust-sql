use sha1::Digest;
use sha2::Sha256;

use crate::DbError;

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

    pub fn prepare_password(
        password: &str,
        auth_plugin_data: &[u8],
    ) -> Vec<u8> {
        let mut password_data = password.as_bytes().to_vec();
        password_data.push(0);

        password_data
            .iter()
            .enumerate()
            .map(|(index, byte)| {
                byte ^ auth_plugin_data[index % auth_plugin_data.len()]
            })
            .collect()
    }

    pub fn parse_public_key(payload: &[u8]) -> Result<String, DbError> {
        String::from_utf8(payload.to_vec())
            .map_err(|_| DbError::new(
                String::from("Invalid UTF-8 in MySQL RSA public key")
            ))
    }
}