
use std::{io::{Read, Write}, net::TcpStream};
use rand::{distr::Alphanumeric, Rng};

use crate::AuthKind::{ClearTextPassword, MD5Password, SASL, SASLContinue, SASLFinal};





#[cfg(test)]
mod tests {
    use crate::DatabaseConfig;

    #[test]
    fn connection_test() {
        let config = DatabaseConfig::new(
            String::from("localhost"),
            5432,
            String::from("postgres"),
            String::from("password"),
            String::from("testdb"),
        );

        let result = config.connect();

        assert!(result.is_ok());
    }
}





pub struct DatabaseConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
    db_name: String,
}


impl DatabaseConfig {
    pub fn new(
        host: String,
        port: u16, username: String,
        password: String,
        db_name: String
    ) -> Self {
        Self {
            host,
            port,
            username,
            password,
            db_name,
        }
    }

    pub fn connect(&self) -> Result<Connection, DbError> {
        if self.host.is_empty(){
            return Err(DbError { message: String::from("Host is empty") });
        }
        let mut connection = Connection::new(self.host.clone(), self.port);

        connection.open(
            &self.username,
            &self.password,
            &self.db_name
        )?;

        Ok(connection)
    }

}


pub struct Connection {
    host: String,
    port: u16,
    stream: Option<TcpStream>,
}

impl Connection {
    const AUTHENTICATION_SASL: u32 = 10;

    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            stream: None,
        }
    }

    fn handle_authentication(&self, payload: Vec<u8>) ->Result<AuthKind, DbError> {
        if payload.len() < 4 {
            return Err(DbError::new(
                String::from("Invalid authentication message")
            ));
        }

        let auth_type = u32::from_be_bytes([
            payload[0],
            payload[1],
            payload[2],
            payload[3],
        ]);

        match auth_type {
            0 => {
                return Ok(AuthKind::AuthenticationOk);
            }

            2 => {
                return Ok(AuthKind::KerberosV5);
            }

            3 => {
                return Ok(AuthKind::ClearTextPassword);
            }

            5 => {
                if payload.len() < 8 {
                    return Err(DbError::new(
                        String::from("Invalid MD5 authentication payload")
                    ));
                }

                let salt = payload[4..8]
                    .try_into()
                    .map_err(|_| DbError::new(
                        String::from("Invalid MD5 authentication payload")
                    ))?;

                Ok(AuthKind::MD5Password(salt))
            }

            10 => {
                let mechanisms = Self::parse_sasl_mechanisms(&payload)?;

                Ok(AuthKind::SASL(mechanisms))
            }

            11 => {
                return Ok(AuthKind::SASLContinue(payload));
            }

            12 => {
                return Ok(AuthKind::SASLFinal(payload));
            }

            other => {
                return Ok(AuthKind::Unknown(other));
            }
        }
    }

    fn open(
        &mut self,
        username: &str,
        password: &str,
        db_name: &str
    ) -> Result<(), DbError> {

        let address = format!("{}:{}", self.host, self.port);

        let stream = TcpStream::connect(address)
            .map_err( |error| {
                DbError::new(error.to_string())
            })?;

        self.stream = Some(stream);

        let message = Self::build_startup_message(username, db_name);

        self.send(&message)?;

        let message = self.read_message()?;
        let message = message.parse();

        match message {
            ServerMessage::Authentication(payload) => {
                match self.handle_authentication(payload)? {
                    AuthKind::SASL(mechanisms) => {
                        let client_nonce = Self::generate_client_nonce();

                        let message = Self::build_sasl_initial_response(
                            &mechanisms[0],
                            username,
                            &client_nonce
                        );

                        self.send(&message)?;

                        let message = self.read_message()?;
                        let message = message.parse();
                    }
                    _ => {}
                }
            }

            ServerMessage::ErrorResponse(payload) => {
                println!("Error: {:?}", payload);
            }

            _ => {}
        }

        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    fn send(&mut self, data: &[u8]) -> Result<(), DbError> {
        match &mut self.stream {
            Some(stream) => {
                stream
                    .write_all(data)
                    .map_err( |e| DbError::new(e.to_string()))?;

                Ok(())
            }
            None => Err(DbError::new(String::from("Connection is not open")))
        }
    }

    fn read_message(&mut self) -> Result<Message, DbError> {
        let stream = match &mut self.stream {
            Some(stream) => stream,
            None => {
                return Err(DbError::new(String::from("Connection is not open")));
            }
        };

        let mut type_buffer = [0u8; 1]; //il tipo non è incluso nella lunghezza

        stream
            .read_exact(&mut type_buffer)
            .map_err(|e| DbError::new(e.to_string()))?;

        let message_type = type_buffer[0];

        let mut length_buffer = [0u8; 4];

        stream
            .read_exact(&mut length_buffer)
            .map_err(|e| DbError::new(e.to_string()))?;

        let length = u32::from_be_bytes(length_buffer);

        let payload_length = length - 4; //la lunghezza del length stesso

        let mut payload = vec![0u8; payload_length as usize];

        stream
            .read_exact(&mut payload)
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(Message {
            message_type,
            payload,
        })
    }

    fn build_startup_message(
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

    fn build_sasl_initial_response(
        mechanism: &str,
        username: &str,
        client_nonce: &str,
    ) -> Vec<u8> {
        let client_first_message = format!(
            "n,,n={},r={}",
            username,
            client_nonce
        );

        let mut message = Vec::new();

        // Message type
        message.push(b'p');

        // Placeholder per length
        message.extend_from_slice(&[0, 0, 0, 0]);

        // SASL mechanism
        message.extend_from_slice(mechanism.as_bytes());
        message.push(0);

        // Length del client-first-message
        let response_length = client_first_message.len() as u32;
        message.extend_from_slice(&response_length.to_be_bytes());

        // Client-first-message
        message.extend_from_slice(client_first_message.as_bytes());

        // Length totale del messaggio, escluso il byte 'p'
        let length = (message.len() - 1) as u32;

        message[1..5].copy_from_slice(&length.to_be_bytes());

        message
    }

    fn parse_sasl_mechanisms(payload: &[u8]) -> Result<Vec<String>, DbError> {
        if payload.len() < 5 {
            return Err(DbError::new(
                String::from("Invalid authentication payload")
            ));
        }

        let auth_type = u32::from_be_bytes([
            payload[0],
            payload[1],
            payload[2],
            payload[3],
        ]);

        if auth_type != 10 {
            return Err(DbError::new(
                String::from("Authentication is not SASL")
            ));
        }

        let mechanism_data = &payload[4..];

        let mut mechanisms = Vec::new();
        let mut start = 0;

        for i in 0..mechanism_data.len() {
            if mechanism_data[i] == 0 {
                if i == start {
                    break;
                }

                let mechanism = std::str::from_utf8(
                    &mechanism_data[start..i]
                )
                .map_err(|_| DbError::new(
                    String::from("Invalid UTF-8 in SASL mechanism")
                ))?;

                mechanisms.push(mechanism.to_string());

                start = i + 1;
            }
        }

        if mechanisms.is_empty() {
            return Err(DbError::new(
                String::from("No SASL mechanisms found")
            ));
        }

        Ok(mechanisms)
    }


    fn generate_client_nonce() -> String {
        let mut rng = rand::rng();

        (&mut rng)
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    }
}


pub struct DbError {
    message: String
}

impl DbError {
    pub fn new(message: String) -> Self {
        Self {
            message,
        }
    }
}


pub struct Message {
    message_type: u8,
    payload: Vec<u8>,
}

impl Message {
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


pub enum AuthKind {
    AuthenticationOk,
    KerberosV5,
    ClearTextPassword,
    MD5Password([u8; 4]),
    SASL(Vec<String>),
    SASLContinue(Vec<u8>),
    SASLFinal(Vec<u8>),
    Unknown(u32),
}