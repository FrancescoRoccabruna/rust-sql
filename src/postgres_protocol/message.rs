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

    pub fn startup(username: &str, db_name: &str) -> Vec<u8> {
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

        let length = message.len() as u32;

        message[0..4].copy_from_slice(&length.to_be_bytes());

        message
    }

    pub fn sasl_initial_response(mechanism: &str, client_message: &str) -> Vec<u8> {
        let mut message = Vec::new();

        message.push(b'p');

        message.extend_from_slice(&[0, 0, 0, 0]);

        message.extend_from_slice(mechanism.as_bytes());
        message.push(0);

        let response_length = client_message.len() as u32;

        message.extend_from_slice(&response_length.to_be_bytes());

        message.extend_from_slice(client_message.as_bytes());

        let length = (message.len() - 1) as u32;

        message[1..5].copy_from_slice(&length.to_be_bytes());

        message
    }

    pub fn sasl_response(client_message: &str) -> Vec<u8> {
        let mut message = Vec::new();

        message.push(b'p');

        message.extend_from_slice(&[0, 0, 0, 0]);

        message.extend_from_slice(client_message.as_bytes());

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

            b'T' => ServerMessage::RowDescription(self.payload),

            b'D' => ServerMessage::DataRow(self.payload),

            b'C' => ServerMessage::CommandComplete(self.payload),

            b'1' => ServerMessage::ParseComplete(self.payload),

            b't' => ServerMessage::ParameterDescription(self.payload),

            b'2' => ServerMessage::BindComplete(self.payload),

            b'n' => ServerMessage::NoData(self.payload),

            other => ServerMessage::Unknown(other, self.payload),
        }
    }

    pub fn query(sql: &str) -> Vec<u8> {
        let mut query = Vec::new();

        query.push(b'Q');

        let length = (4 + sql.len() + 1) as u32;

        query.extend_from_slice(&length.to_be_bytes());

        query.extend_from_slice(sql.as_bytes());
        query.push(0);

        query
    }

    pub fn parse_statement(statement_name: &str, sql: &str, parameter_oids: &[u32]) -> Vec<u8> {
        let mut payload = Vec::new();

        // PreparedStatement message
        payload.push(b'P');

        // Statement name
        payload.extend_from_slice(statement_name.as_bytes());
        payload.push(0);

        // Query
        payload.extend_from_slice(sql.as_bytes());
        payload.push(0);

        // Number of parameter type OIDs
        payload.extend_from_slice(&(parameter_oids.len() as u16).to_be_bytes());

        // Parameter type OIDs
        for oid in parameter_oids {
            payload.extend_from_slice(&oid.to_be_bytes());
        }

        Self::frame_message(payload)
    }

    pub fn bind(portal_name: &str, statement_name: &str, params: &[Option<Vec<u8>>]) -> Vec<u8> {
        let mut payload = Vec::new();

        // Bind
        payload.push(b'B');

        // Portal
        payload.extend_from_slice(portal_name.as_bytes());
        payload.push(0);

        // Prepared statement
        payload.extend_from_slice(statement_name.as_bytes());
        payload.push(0);

        // Parameter format codes.
        //
        // 0 = text format.
        payload.extend_from_slice(&0u16.to_be_bytes());

        // Number of parameters
        payload.extend_from_slice(&(params.len() as u16).to_be_bytes());

        for param in params {
            match param {
                Some(param) => {
                    payload.extend_from_slice(&(param.len() as i32).to_be_bytes());

                    payload.extend_from_slice(param);
                }

                None => {
                    // PostgreSQL NULL
                    payload.extend_from_slice(&(-1i32).to_be_bytes());
                }
            }
        }

        // Result format codes.
        //
        // 0 = text format.
        payload.extend_from_slice(&0u16.to_be_bytes());

        Self::frame_message(payload)
    }

    pub fn describe_portal(portal_name: &str) -> Vec<u8> {
        let mut payload = Vec::new();

        payload.push(b'D');

        // 'P' = portal
        payload.push(b'P');

        payload.extend_from_slice(portal_name.as_bytes());
        payload.push(0);

        Self::frame_message(payload)
    }

    pub fn execute(portal_name: &str) -> Vec<u8> {
        let mut payload = Vec::new();

        payload.push(b'E');

        payload.extend_from_slice(portal_name.as_bytes());
        payload.push(0);

        // Max rows = 0 => nessun limite
        payload.extend_from_slice(&0u32.to_be_bytes());

        Self::frame_message(payload)
    }

    pub fn sync() -> Vec<u8> {
        Self::frame_message(vec![b'S'])
    }

    fn frame_message(mut payload: Vec<u8>) -> Vec<u8> {
        let message_type = payload.remove(0);

        let length = (payload.len() + 4) as u32;

        let mut message = Vec::new();

        message.push(message_type);

        message.extend_from_slice(&length.to_be_bytes());

        message.extend_from_slice(&payload);

        message
    }
}

#[expect(dead_code)]
pub enum ServerMessage {
    Authentication(Vec<u8>),
    ParameterStatus(Vec<u8>),
    BackendKeyData(Vec<u8>),
    ReadyForQuery(Vec<u8>),
    ErrorResponse(Vec<u8>),

    RowDescription(Vec<u8>),
    DataRow(Vec<u8>),
    CommandComplete(Vec<u8>),

    // Extended Query Protocol
    ParseComplete(Vec<u8>),
    ParameterDescription(Vec<u8>),
    BindComplete(Vec<u8>),
    NoData(Vec<u8>),

    Unknown(u8, Vec<u8>),
}
