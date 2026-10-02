use crate::{
    Connection, DbError, Value, ValueType, backend::Backend, orm::query::{Condition, InsertQueryRequest, OrderBy, SelectQueryRequest, UpdateQueryRequest}, postgres_protocol::{
        authentication::AuthKind,
        message::{Message, ServerMessage},
        result::ResultParser,
        scram::ScramClient,
    }, query::{Query, QueryResult},
};

pub struct PostgresBackend<'a> {
    connection: &'a mut Connection,
}
impl<'a> PostgresBackend<'a> {
    pub fn new(connection: &'a mut Connection) -> Self {
        Self { connection }
    }

    fn send_startup(&mut self, username: &str, db_name: &str) -> Result<(), DbError> {
        let message = Message::startup(username, db_name);

        self.connection.write(&message)?;

        Ok(())
    }

    fn read_message(&mut self) -> Result<Message, DbError> {
        let mut type_buffer = [0u8; 1]; //il tipo non è incluso nella lunghezza

        self.connection.read(&mut type_buffer)?;

        let message_type = type_buffer[0];

        let mut length_buffer = [0u8; 4];

        self.connection.read(&mut length_buffer)?;

        let length = u32::from_be_bytes(length_buffer);

        if length < 4 {
            return Err(DbError::new(String::from(
                "Invalid PostgreSQL message length",
            )));
        }

        let payload_length = length - 4; //la lunghezza del length stesso

        let mut payload = vec![0u8; payload_length as usize];

        self.connection.read(&mut payload)?;

        Ok(Message::new(message_type, payload))
    }

    fn authenticate_scram(
        &mut self,
        mechanisms: Vec<String>,
        username: &str,
        password: &str,
    ) -> Result<(), DbError> {
        let mechanism = mechanisms
            .iter()
            .find(|m| *m == "SCRAM-SHA-256")
            .ok_or_else(|| {
                DbError::new(String::from("SCRAM-SHA-256 is not supported by server"))
            })?;

        let mut scram = ScramClient::new(username, password);

        let first_message = scram.first_message();

        let message = Message::sasl_initial_response(mechanism, &first_message);

        self.connection.write(&message)?;

        let message = self.read_message()?;

        let message = message.parse();

        match message {
            ServerMessage::Authentication(payload) => {
                let auth = AuthKind::parse(&payload)?;

                match auth {
                    AuthKind::SaslContinue(message) => {
                        scram.handle_server_first(&message)?;

                        let final_message = scram.final_message()?;
                        let message = Message::sasl_response(&final_message);

                        self.connection.write(&message)?;

                        let message = self.read_message()?;
                        let message = message.parse();

                        match message {
                            ServerMessage::Authentication(payload) => {
                                let auth = AuthKind::parse(&payload)?;

                                match auth {
                                    AuthKind::SaslFinal(message) => {
                                        scram.handle_server_final(&message)?;
                                    }
                                    _ => {
                                        return Err(DbError::new(String::from(
                                            "Expected Sasl final message",
                                        )));
                                    }
                                }
                            }
                            _ => {
                                return Err(DbError::new(String::from(
                                    "Expected Authentication message",
                                )));
                            }
                        }
                    }
                    _ => {
                        return Err(DbError::new(String::from("Expected Sasl Continue message")));
                    }
                }
            }
            _ => {
                return Err(DbError::new(String::from(
                    "Expected Authentication message",
                )));
            }
        }

        Ok(())
    }

    fn encode_parameter(value: &Value) -> Option<Vec<u8>> {
        match value {
            Value::Null => None,

            Value::Int(value) => Some(value.to_string().into_bytes()),

            Value::UInt(value) => Some(value.to_string().into_bytes()),

            Value::Float(value) => Some(value.to_string().into_bytes()),

            Value::String(value) => Some(value.as_bytes().to_vec()),

            Value::Bytes(value) => Some(value.clone()),

            Value::Bool(value) => {
                if *value {
                    Some(b"true".to_vec())
                } else {
                    Some(b"false".to_vec())
                }
            }
        }
    }

    fn build_condition(
        index: &mut usize,
        condition: &Condition,
        params: &mut Vec<Value>,
    ) -> String {
        match condition {
            Condition::Eq(column, value) => {
                params.push(value.clone());

                let placeholder = *index;
                *index += 1;

                format!("{} = ${}", column.name, placeholder)
            }

            Condition::NotEq(column, value) => {
                params.push(value.clone());

                let placeholder = *index;
                *index += 1;

                format!("{} != ${}", column.name, placeholder)
            }

            Condition::Gt(column, value) => {
                params.push(value.clone());

                let placeholder = *index;
                *index += 1;

                format!("{} > ${}", column.name, placeholder)
            }

            Condition::Gte(column, value) => {
                params.push(value.clone());

                let placeholder = *index;
                *index += 1;

                format!("{} >= ${}", column.name, placeholder)
            }

            Condition::Lt(column, value) => {
                params.push(value.clone());

                let placeholder = *index;
                *index += 1;

                format!("{} < ${}", column.name, placeholder)
            }

            Condition::Lte(column, value) => {
                params.push(value.clone());

                let placeholder = *index;
                *index += 1;

                format!("{} <= ${}", column.name, placeholder)
            }

            Condition::IsNull(column) => {
                format!("{} IS NULL", column.name)
            }

            Condition::IsNotNull(column) => {
                format!("{} IS NOT NULL", column.name)
            }

            Condition::And(left, right) => {
                let left = Self::build_condition(index, left, params);
                let right = Self::build_condition(index, right, params);

                format!("({} AND {})", left, right)
            }

            Condition::Or(left, right) => {
                let left = Self::build_condition(index, left, params);
                let right = Self::build_condition(index, right, params);

                format!("({} OR {})", left, right)
            }
        }
    }
}

impl<'a> Backend for PostgresBackend<'a> {
    fn open(&mut self, username: &str, password: &str, db_name: &str) -> Result<(), DbError> {
        self.send_startup(username, db_name)?;

        let message = self.read_message()?;
        let message = message.parse();

        match message {
            ServerMessage::Authentication(payload) => {
                let auth = AuthKind::parse(&payload)?;

                if let AuthKind::Sasl(mechanisms) = auth {
                    self.authenticate_scram(mechanisms, username, password)?;

                    loop {
                        let message = self.read_message()?;
                        let message = message.parse();

                        match message {
                            ServerMessage::ReadyForQuery(_) => {
                                return Ok(());
                            }

                            ServerMessage::Unknown(message_type, _) => {
                                return Err(DbError::new(format!(
                                    "Unknown message type: {}",
                                    message_type
                                )));
                            }

                            _ => {}
                        }
                    }
                }
            }

            ServerMessage::ErrorResponse(payload) => {
                return Err(DbError::new(format!("PostgreSQL error: {:?}", payload)));
            }

            _ => {}
        }

        Ok(())
    }

    fn exec_simple(&mut self, query: &Query) -> Result<QueryResult, DbError> {
        let message = Message::query(query.sql());
        self.connection.write(&message)?;

        let mut result = QueryResult::new();
        let mut error = None;

        let mut columns = Vec::new();

        let mut type_oids = Vec::new();
        let mut format_codes = Vec::new();

        loop {
            let message = self.read_message()?;

            match message.parse() {
                ServerMessage::ErrorResponse(payload) => {
                    error = Some(DbError::new(format!("error: {:?}", payload)));
                }

                ServerMessage::ReadyForQuery(_) => {
                    break;
                }

                ServerMessage::RowDescription(payload) => {
                    let (parsed_columns, parsed_type_oids, parsed_format_codes) =
                        ResultParser::parse_row_description(&payload)?;

                    columns = parsed_columns;
                    type_oids = parsed_type_oids;
                    format_codes = parsed_format_codes;
                }

                ServerMessage::DataRow(payload) => {
                    let row = ResultParser::parse_data_row(
                        &payload,
                        &columns,
                        &type_oids,
                        &format_codes,
                    )?;

                    result.add_row(row);
                }

                _ => {}
            }
        }

        if let Some(error) = error {
            return Err(error);
        }

        result.set_columns(columns);

        Ok(result)
    }

    fn exec_prepared(&mut self, query: &Query) -> Result<QueryResult, DbError> {
        let params = query
            .params()
            .iter()
            .map(Self::encode_parameter)
            .collect::<Vec<_>>();

        // Unnamed statement: viene sostituito ad ogni Parse.
        let message = Message::parse_statement("", query.sql(), &[]);

        self.connection.write(&message)?;

        // Unnamed portal.
        let message = Message::bind("", "", &params);

        self.connection.write(&message)?;

        // Chiediamo al server la descrizione
        // del risultato del portal.
        let message = Message::describe_portal("");

        self.connection.write(&message)?;

        // Eseguiamo il portal.
        let message = Message::execute("");

        self.connection.write(&message)?;

        // Chiude il ciclo extended query.
        let message = Message::sync();

        self.connection.write(&message)?;

        let mut result = QueryResult::new();

        let mut error = None;

        let mut columns = Vec::new();

        let mut type_oids = Vec::new();

        let mut format_codes = Vec::new();

        loop {
            let message = self.read_message()?;

            match message.parse() {
                ServerMessage::ErrorResponse(payload) => {
                    error = Some(DbError::new(format!("error: {:?}", payload)));
                }

                ServerMessage::ReadyForQuery(_) => {
                    break;
                }

                ServerMessage::RowDescription(payload) => {
                    let (parsed_columns, parsed_type_oids, parsed_format_codes) =
                        ResultParser::parse_row_description(&payload)?;

                    columns = parsed_columns;

                    type_oids = parsed_type_oids;

                    format_codes = parsed_format_codes;
                }

                ServerMessage::DataRow(payload) => {
                    let row = ResultParser::parse_data_row(
                        &payload,
                        &columns,
                        &type_oids,
                        &format_codes,
                    )?;

                    result.add_row(row);
                }

                ServerMessage::ParseComplete(_) => {}

                ServerMessage::ParameterDescription(_) => {}

                ServerMessage::BindComplete(_) => {}

                ServerMessage::NoData(_) => {}

                ServerMessage::CommandComplete(_) => {}

                _ => {}
            }
        }

        if let Some(error) = error {
            return Err(error);
        }

        result.set_columns(columns);

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

        let mut index = 1;

        if !query.where_clause().is_empty() {
            let conditions = query
                .where_clause()
                .iter()
                .map(|condition| Self::build_condition(&mut index, condition, &mut params))
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
        let query = Query::new("BEGIN;");

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
                    ValueType::UInt => "BIGINT",
                    ValueType::Float => "DOUBLE PRECISION",
                    ValueType::String => "TEXT",
                    ValueType::Bytes => "BYTEA",
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

        let placeholders = (1..=values.len())
            .map(|index| format!("${index}"))
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
            .enumerate()
            .map(|(index, (name, _))| {
                format!("{} = ${}", name, index + 1)
            })
            .collect::<Vec<_>>()
            .join(", ");

        let pk_index = values.len() + 1;

        let mut params = values
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();

        params.push(primary_key_value);

        let sql = format!(
            "UPDATE {} SET {} WHERE {} = ${}",
            query.table_name(),
            columns,
            primary_key_name,
            pk_index,
        );

        let query = Query::with_params(&sql, params);

        self.exec(&query)?;

        Ok(())
    }
}
