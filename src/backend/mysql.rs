use crate::{
    Connection, DbError, Value, ValueType, backend::Backend, mysql_protocol::{
        authentication::Handshake,
        message::{ERR_PACKET, Message, OK_PACKET, ServerMessage},
        result::ResultParser,
    }, orm::query::{Condition, InsertQueryRequest, OrderBy, SelectQueryRequest, UpdateQueryRequest}, query::{Query, QueryResult},
};

use rsa::{Oaep, RsaPublicKey, pkcs8::DecodePublicKey};

use sha1::Sha1;

use rsa::rand_core::OsRng;

const CLIENT_CONNECT_WITH_DB: u32 = 1 << 3;
const CLIENT_PROTOCOL_41: u32 = 1 << 9;
const CLIENT_SECURE_CONNECTION: u32 = 1 << 15;
const CLIENT_PLUGIN_AUTH: u32 = 1 << 19;
const MYSQL_TYPE_TINY: u8 = 0x01;
const MYSQL_TYPE_DOUBLE: u8 = 0x05;
const MYSQL_TYPE_LONGLONG: u8 = 0x08;
const MYSQL_TYPE_STRING: u8 = 0x0F;
const MYSQL_TYPE_BLOB: u8 = 0xFC;
const MYSQL_TYPE_NULL: u8 = 0x06;

//const COM_STMT_PREPARE: u8 = 0x16;
const COM_STMT_EXECUTE: u8 = 0x17;
const COM_STMT_CLOSE: u8 = 0x19;

const MYSQL_UNSIGNED_FLAG: u8 = 0x80;

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

    fn build_statement_execute(statement_id: u32, params: &[Value]) -> Result<Vec<u8>, DbError> {
        let mut payload = Vec::new();

        payload.push(COM_STMT_EXECUTE);

        payload.extend_from_slice(&statement_id.to_le_bytes());

        // flags
        payload.push(0);

        // iteration count
        payload.extend_from_slice(&1u32.to_le_bytes());

        let bitmap_length = (params.len() + 7) / 8;

        let mut null_bitmap = vec![0u8; bitmap_length];

        for (index, value) in params.iter().enumerate() {
            if matches!(value, Value::Null) {
                null_bitmap[index / 8] |= 1 << (index % 8);
            }
        }

        payload.extend_from_slice(&null_bitmap);

        // new parameters bound
        payload.push(1);

        for value in params {
            let (mysql_type, flags) = Self::mysql_parameter_type(value);

            payload.push(mysql_type);
            payload.push(flags);
        }

        for value in params {
            if !matches!(value, Value::Null) {
                Self::encode_parameter_value(value, &mut payload)?;
            }
        }

        Ok(payload)
    }

    fn mysql_parameter_type(value: &Value) -> (u8, u8) {
        match value {
            Value::Null => (MYSQL_TYPE_NULL, 0),

            Value::Int(_) => (MYSQL_TYPE_LONGLONG, 0),

            Value::UInt(_) => (MYSQL_TYPE_LONGLONG, MYSQL_UNSIGNED_FLAG),

            Value::Float(_) => (MYSQL_TYPE_DOUBLE, 0),

            Value::String(_) => (MYSQL_TYPE_STRING, 0),

            Value::Bytes(_) => (MYSQL_TYPE_BLOB, 0),

            Value::Bool(_) => (MYSQL_TYPE_TINY, 0),
        }
    }

    fn encode_parameter_value(value: &Value, payload: &mut Vec<u8>) -> Result<(), DbError> {
        match value {
            Value::Null => {
                // NULL è rappresentato nel null bitmap,
                // quindi non ha bytes nel value section.
                Ok(())
            }

            Value::Int(value) => {
                payload.extend_from_slice(&value.to_le_bytes());

                Ok(())
            }

            Value::UInt(value) => {
                payload.extend_from_slice(&value.to_le_bytes());

                Ok(())
            }

            Value::Float(value) => {
                payload.extend_from_slice(&value.to_le_bytes());

                Ok(())
            }

            Value::Bool(value) => {
                payload.push(if *value { 1 } else { 0 });

                Ok(())
            }

            Value::String(value) => {
                Self::encode_lenenc_bytes(value.as_bytes(), payload);

                Ok(())
            }

            Value::Bytes(value) => {
                Self::encode_lenenc_bytes(value, payload);

                Ok(())
            }
        }
    }

    fn encode_lenenc_bytes(value: &[u8], payload: &mut Vec<u8>) {
        let length = value.len();

        if length < 251 {
            payload.push(length as u8);
        } else if length <= 0xFFFF {
            payload.push(0xFC);
            payload.extend_from_slice(&(length as u16).to_le_bytes());
        } else if length <= 0xFFFFFF {
            payload.push(0xFD);
            payload.push((length & 0xFF) as u8);
            payload.push(((length >> 8) & 0xFF) as u8);
            payload.push(((length >> 16) & 0xFF) as u8);
        } else {
            payload.push(0xFE);
            payload.extend_from_slice(&(length as u64).to_le_bytes());
        }

        payload.extend_from_slice(value);
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

    fn build_condition(condition: &Condition, params: &mut Vec<Value>) -> String {
        match condition {
            Condition::Eq(column, value) => {
                params.push(value.clone());

                format!("{} = ?", column.name)
            }

            Condition::NotEq(column, value) => {
                params.push(value.clone());

                format!("{} != ?", column.name)
            }

            Condition::Gt(column, value) => {
                params.push(value.clone());

                format!("{} > ?", column.name)
            }

            Condition::Gte(column, value) => {
                params.push(value.clone());

                format!("{} >= ?", column.name)
            }

            Condition::Lt(column, value) => {
                params.push(value.clone());

                format!("{} < ?", column.name)
            }

            Condition::Lte(column, value) => {
                params.push(value.clone());

                format!("{} <= ?", column.name)
            }

            Condition::IsNull(column) => {
                format!("{} IS NULL", column.name)
            }

            Condition::IsNotNull(column) => {
                format!("{} IS NOT NULL", column.name)
            }

            Condition::And(left, right) => {
                let left = Self::build_condition(left, params);
                let right = Self::build_condition(right, params);

                format!("({} AND {})", left, right)
            }

            Condition::Or(left, right) => {
                let left = Self::build_condition(left, params);
                let right = Self::build_condition(right, params);

                format!("({} OR {})", left, right)
            }
        }
    }

    fn parse_statement_prepare_response(payload: &[u8]) -> Result<(u32, u16, u16), DbError> {
        if payload.len() < 12 {
            return Err(DbError::new(String::from(
                "Invalid MySQL statement prepare response",
            )));
        }

        let statement_id = u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]]);

        let num_columns = u16::from_le_bytes([payload[5], payload[6]]);

        let num_params = u16::from_le_bytes([payload[7], payload[8]]);

        Ok((statement_id, num_columns, num_params))
    }

    fn read_statement_metadata(&mut self, count: u16, kind: &str) -> Result<(), DbError> {
        for _ in 0..count {
            let message = self.read_message()?;

            if message.message_type() == Some(ERR_PACKET) {
                return Err(Self::parse_error_packet(&message.payload));
            }

            ResultParser::parse_column_definition(&message.payload)?;
        }

        if count > 0 {
            let message = self.read_message()?;

            if message.message_type() != Some(0xFE) {
                return Err(DbError::new(format!(
                    "Expected EOF after MySQL {} metadata",
                    kind
                )));
            }
        }

        Ok(())
    }

    fn exec_prepared_result(&mut self, first_message: Message) -> Result<QueryResult, DbError> {
        if first_message.message_type() == Some(ERR_PACKET) {
            return Err(Self::parse_error_packet(&first_message.payload));
        }

        // Il primo packet contiene il column count.
        let column_count = ResultParser::parse_column_count(&first_message.payload)?;

        let mut columns = Vec::with_capacity(column_count);

        let mut column_types = Vec::with_capacity(column_count);

        let mut column_flags = Vec::with_capacity(column_count);

        // Column definitions
        for _ in 0..column_count {
            let message = self.read_message()?;

            if message.message_type() == Some(ERR_PACKET) {
                return Err(Self::parse_error_packet(&message.payload));
            }

            let (column, column_type, flags) =
                ResultParser::parse_column_definition(&message.payload)?;

            columns.push(column);
            column_types.push(column_type);
            column_flags.push(flags);
        }

        // EOF dopo le column definitions.
        let message = self.read_message()?;

        if message.message_type() != Some(0xFE) {
            return Err(DbError::new(String::from(
                "Expected EOF after MySQL prepared column metadata",
            )));
        }

        let mut result = QueryResult::new();

        result.set_columns(columns);

        // Binary result rows
        loop {
            let message = self.read_message()?;

            match message.message_type() {
                Some(0xFE) => {
                    break;
                }

                Some(ERR_PACKET) => {
                    return Err(Self::parse_error_packet(&message.payload));
                }

                _ => {
                    let row = ResultParser::parse_binary_row(
                        &message.payload,
                        result.columns(),
                        &column_types,
                        &column_flags,
                    )?;

                    result.add_row(row);
                }
            }
        }

        Ok(result)
    }

    fn close_statement(&mut self, statement_id: u32) -> Result<(), DbError> {
        let mut payload = Vec::new();

        payload.push(COM_STMT_CLOSE);

        payload.extend_from_slice(&statement_id.to_le_bytes());

        let packet = Message::encode(&payload, 0);

        self.connection.write(&packet)?;

        // COM_STMT_CLOSE non produce una risposta.
        Ok(())
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

    fn exec_simple(&mut self, query: &Query) -> Result<QueryResult, DbError> {
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

    fn exec_prepared(&mut self, query: &Query) -> Result<QueryResult, DbError> {
        let payload = Message::statement_prepare(query.sql());

        let packet = Message::encode(&payload, 0);

        self.connection.write(&packet)?;

        let response = self.read_message()?;

        if response.message_type() == Some(ERR_PACKET) {
            return Err(Self::parse_error_packet(&response.payload));
        }

        if response.message_type() != Some(OK_PACKET) {
            return Err(DbError::new(String::from(
                "Expected MySQL statement prepare response",
            )));
        }

        let (statement_id, num_columns, num_params) =
            Self::parse_statement_prepare_response(&response.payload)?;

        if num_params > 0 {
            self.read_statement_metadata(num_params, "parameter")?;
        }

        if num_columns > 0 {
            self.read_statement_metadata(num_columns, "column")?;
        }

        let payload = Self::build_statement_execute(statement_id, query.params())?;

        let packet = Message::encode(&payload, 0);

        self.connection.write(&packet)?;

        let response = self.read_message()?;

        let result = match response.message_type() {
            Some(OK_PACKET) => QueryResult::new(),

            Some(ERR_PACKET) => {
                return Err(Self::parse_error_packet(&response.payload));
            }

            _ => self.exec_prepared_result(response)?,
        };

        self.close_statement(statement_id)?;

        Ok(result)
    }

    fn exec_orm(&mut self, query: &dyn SelectQueryRequest) -> Result<QueryResult, DbError> {
        let fields = query.fields();

        let mut sql = format!(
            "SELECT {} FROM {}",
            fields
                .iter()
                .map(|field| field.name)
                .collect::<Vec<_>>()
                .join(", "),
            query.table_name(),
        );

        let mut params = Vec::new();

        if !query.where_clause().is_empty() {
            let conditions = query
                .where_clause()
                .iter()
                .map(|condition| Self::build_condition(condition, &mut params))
                .collect::<Vec<_>>();

            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        if !query.order_by().is_empty() {
            sql.push_str(" ORDER BY ");

            let order_by = query
                .order_by()
                .iter()
                .map(|order| match order {
                    OrderBy::Asc(column) => {
                        format!("{} ASC", column.name)
                    }

                    OrderBy::Desc(column) => {
                        format!("{} DESC", column.name)
                    }
                })
                .collect::<Vec<_>>();

            sql.push_str(&order_by.join(", "));
        }

        if let Some(limit) = query.limit() {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let query = Query::with_params(&sql, params);

        self.exec(&query)
    }

    fn start_transaction(&mut self) -> Result<(), DbError> {
        let query = Query::new("START TRANSACTION;");

        self.exec(&query)?;

        Ok(())
    }

    fn commit_transaction(&mut self) -> Result<(), DbError> {
        let query = Query::new("COMMIT;");

        self.exec(&query)?;

        Ok(())
    }

    fn rollback_transaction(&mut self) -> Result<(), DbError> {
        let query = Query::new("ROLLBACK;");

        self.exec(&query)?;

        Ok(())
    }

    fn create_table(
        &mut self,
        table: &crate::orm::TableDefinition,
    ) -> Result<(), DbError> {
        let columns = table
            .fields
            .iter()
            .map(|field| {
                let sql_type = match field.value_type {
                    ValueType::Int => "BIGINT",
                    ValueType::UInt => "BIGINT UNSIGNED",
                    ValueType::Float => "DOUBLE",
                    ValueType::String => "TEXT",
                    ValueType::Bytes => "BLOB",
                    ValueType::Bool => "BOOLEAN",
                };

                let mut definition =
                    format!("{} {}", field.name, sql_type);

                if field.primary_key {
                    definition.push_str(" PRIMARY KEY");
                }

                if !field.nullable {
                    definition.push_str(" NOT NULL");
                }

                definition
            })
            .collect::<Vec<_>>()
            .join(", ");

        let query = Query::new(&format!(
            "CREATE TABLE IF NOT EXISTS {} ({})",
            table.name,
            columns,
        ));

        self.exec(&query)?;

        Ok(())
    }

    fn exec_insert(
        &mut self,
        query: &dyn InsertQueryRequest,
    ) -> Result<(), DbError> {
        let values = query.values();

        let columns = values
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(", ");

        let placeholders = std::iter::repeat("?")
            .take(values.len())
            .collect::<Vec<_>>()
            .join(", ");

        let params = values
            .into_iter()
            .map(|(_, value)| value)
            .collect();

        let query = Query::with_params(
            &format!(
                "INSERT INTO {} ({}) VALUES ({})",
                query.table_name(),
                columns,
                placeholders,
            ),
            params,
        );

        self.exec(&query)?;

        Ok(())
    }


    fn exec_update(
        &mut self,
        query: &dyn UpdateQueryRequest,
    ) -> Result<(), DbError> {
        let values = query.values();
        let (primary_key_name, primary_key_value) = query.primary_key();

        let columns = values
            .iter()
            .map(|(name, _)| format!("{} = ?", name))
            .collect::<Vec<_>>()
            .join(", ");

        let mut params = values
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();

        params.push(primary_key_value);

        let sql = format!(
            "UPDATE {} SET {} WHERE {} = ?",
            query.table_name(),
            columns,
            primary_key_name,
        );

        let query = Query::with_params(&sql, params);

        self.exec(&query)?;

        Ok(())
    }
}
