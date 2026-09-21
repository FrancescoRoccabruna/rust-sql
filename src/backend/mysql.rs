use crate::{
    Connection, DbError,
    backend::Backend,
    mysql_protocol::{
        authentication::Handshake,
        message::{ERR_PACKET, Message, OK_PACKET, ServerMessage},
        result::ResultParser,
    },
    query::{Query, QueryResult},
};

use rsa::{Oaep, RsaPublicKey, pkcs8::DecodePublicKey};

use sha1::Sha1;

use rsa::rand_core::OsRng;

const CLIENT_CONNECT_WITH_DB: u32 = 1 << 3;
const CLIENT_PROTOCOL_41: u32 = 1 << 9;
const CLIENT_SECURE_CONNECTION: u32 = 1 << 15;
const CLIENT_PLUGIN_AUTH: u32 = 1 << 19;

pub struct MysqlBackend<'a> {
    connection: &'a mut Connection,
}
impl<'a> MysqlBackend<'a> {
    pub fn new(connection: &'a mut Connection) -> Self {
        Self { connection }
    }

    fn read_message(&mut self) -> Result<Message, DbError> {
        let mut length_buffer = [0u8; 3];

        self.connection.read(&mut length_buffer)?;

        let payload_length =
            u32::from_le_bytes([length_buffer[0], length_buffer[1], length_buffer[2], 0]);

        let mut sequence_buffer = [0u8; 1];

        self.connection.read(&mut sequence_buffer)?;

        let sequence_id = sequence_buffer[0];

        let mut payload = vec![0u8; payload_length as usize];

        self.connection.read(&mut payload)?;

        if payload.is_empty() {
            return Err(DbError::new(String::from("Empty MySQL packet")));
        }

        Ok(Message::new(sequence_id, payload))
    }

    fn parse_error_packet(payload: &[u8]) -> DbError {
        if payload.len() < 2 {
            return DbError::new(String::from("Invalid MySQL error packet"));
        }

        let error_code = u16::from_le_bytes([payload[0], payload[1]]);

        let message_start = if payload.len() >= 8 && payload[2] == b'#' {
            8
        } else {
            2
        };

        let message = String::from_utf8_lossy(&payload[message_start..]);

        DbError::new(format!("MySQL error {}: {}", error_code, message))
    }
}

impl<'a> Backend for MysqlBackend<'a> {
    fn open(&mut self, username: &str, password: &str, db_name: &str) -> Result<(), DbError> {
        let message = self.read_message()?;
        let message = message.parse();

        match message {
            ServerMessage::Handshake(payload) => {
                let handshake = Handshake::parse(&payload)?;

                let auth_response =
                    Handshake::scramble_password(password, &handshake.auth_plugin_data);

                let client_capabilities = CLIENT_CONNECT_WITH_DB
                    | CLIENT_PROTOCOL_41
                    | CLIENT_SECURE_CONNECTION
                    | CLIENT_PLUGIN_AUTH;

                let response = Message::handshake_response(
                    username,
                    &auth_response,
                    db_name,
                    client_capabilities,
                    handshake.character_set,
                    &handshake.auth_plugin_name,
                );

                let packet = Message::encode(&response, 1);

                self.connection.write(&packet)?;

                let response = self.read_message()?;

                let sequence_id = response.sequence_id;

                match response.parse() {
                    ServerMessage::Ok(_) => {}

                    ServerMessage::AuthMoreData(payload) => {
                        if payload.len() < 2 {
                            return Err(DbError::new(String::from("Invalid MySQL AuthMoreData")));
                        }

                        let auth_data = &payload[1..];

                        if auth_data == [0x03] {
                            // Fast authentication succeeded.
                            let response = self.read_message()?;

                            match response.parse() {
                                ServerMessage::Ok(_) => {}

                                ServerMessage::Error(payload) => {
                                    return Err(DbError::new(format!(
                                        "MySQL authentication error: {:02X?}",
                                        payload
                                    )));
                                }

                                _ => {
                                    return Err(DbError::new(String::from(
                                        "Expected MySQL OK after authentication",
                                    )));
                                }
                            }
                        } else if auth_data == [0x04] {
                            // Full authentication required.
                            let request_public_key = vec![0x02];

                            let packet = Message::encode(&request_public_key, sequence_id + 1);

                            self.connection.write(&packet)?;

                            let response = self.read_message()?;

                            let public_key = Handshake::parse_public_key(&response.payload)?;

                            let prepared_password =
                                Handshake::prepare_password(password, &handshake.auth_plugin_data);

                            let public_key = RsaPublicKey::from_public_key_pem(&public_key)
                                .map_err(|e| DbError::new(e.to_string()))?;

                            let encrypted = public_key
                                .encrypt(&mut OsRng, Oaep::new::<Sha1>(), &prepared_password)
                                .map_err(|e| DbError::new(e.to_string()))?;

                            let packet = Message::encode(&encrypted, response.sequence_id + 1);

                            self.connection.write(&packet)?;

                            let response = self.read_message()?;

                            match response.parse() {
                                ServerMessage::Ok(_) => {}

                                ServerMessage::Error(payload) => {
                                    return Err(DbError::new(format!(
                                        "MySQL authentication error: {:02X?}",
                                        payload
                                    )));
                                }

                                _ => {
                                    return Err(DbError::new(String::from(
                                        "Unexpected MySQL authentication response",
                                    )));
                                }
                            }
                        } else {
                            return Err(DbError::new(format!(
                                "Unexpected MySQL AuthMoreData: {:02X?}",
                                payload
                            )));
                        }
                    }

                    ServerMessage::Error(payload) => {
                        return Err(DbError::new(format!(
                            "MySQL authentication error: {:02X?}",
                            payload
                        )));
                    }

                    _ => {
                        return Err(DbError::new(String::from(
                            "Unexpected MySQL authentication response",
                        )));
                    }
                }
            }

            ServerMessage::Error(payload) => {
                return Err(DbError::new(format!(
                    "MySQL handshake error: {:?}",
                    payload
                )));
            }

            _ => {
                return Err(DbError::new(String::from("Expected MySQL handshake")));
            }
        }

        Ok(())
    }

    fn exec(&mut self, query: &Query) -> Result<QueryResult, DbError> {
        let payload = Message::query(query.sql());

        let packet = Message::encode(&payload, 0);

        self.connection.write(&packet)?;

        // First server packet.
        let message = self.read_message()?;

        match message.message_type() {
            Some(OK_PACKET) => {
                return Ok(QueryResult::new());
            }

            Some(ERR_PACKET) => {
                return Err(Self::parse_error_packet(&message.payload));
            }

            _ => {}
        }

        // The first byte is the length-encoded
        // column count, so the complete payload
        // must be passed to the parser.
        let column_count = ResultParser::parse_column_count(&message.payload)?;

        let mut columns = Vec::with_capacity(column_count);

        let mut column_types = Vec::with_capacity(column_count);

        let mut flags = Vec::with_capacity(column_count);

        // Column definitions.
        for _ in 0..column_count {
            let message = self.read_message()?;

            let (column, column_type, column_flags) =
                ResultParser::parse_column_definition(&message.payload)?;

            columns.push(column);
            column_types.push(column_type);
            flags.push(column_flags);
        }

        let mut result = QueryResult::new();

        result.set_columns(columns);

        // EOF after column definitions.
        let message = self.read_message()?;

        if message.message_type() != Some(0xFE) {
            return Err(DbError::new(String::from(
                "Expected EOF after column definitions",
            )));
        }

        // Rows.
        loop {
            let message = self.read_message()?;

            if message.message_type() == Some(0xFE) {
                break;
            }

            if message.message_type() == Some(ERR_PACKET) {
                return Err(Self::parse_error_packet(&message.payload));
            }

            let row =
                ResultParser::parse_row(&message.payload, result.columns(), &column_types, &flags)?;

            result.add_row(row);
        }

        Ok(result)
    }
}
