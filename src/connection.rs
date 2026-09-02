use std::{
    io::{Read, Write},
    net::TcpStream,
};

use crate::{postgres_protocol::{
    authentication::AuthKind, message::{Message, ServerMessage}, scram::ScramClient
}, query::{Query, QueryResult}};


pub struct Connection {
    host: String,
    port: u16,
    stream: Option<TcpStream>,
}

impl Connection {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            stream: None,
        }
    }

    fn connect_tcp(&mut self) -> Result<(), DbError> {
        let address = format!("{}:{}", self.host, self.port);

        let stream = TcpStream::connect(address)
            .map_err( |error| {
                DbError::new(error.to_string())
            })?;

        self.stream = Some(stream);

        Ok(())
    }

    fn send_startup(&mut self, username: &str, db_name: &str) -> Result<(), DbError> {
        let message = Message::startup(username, db_name);

        self.send(&message)?;

        Ok(())
    }

    pub fn open(
        &mut self,
        username: &str,
        password: &str,
        db_name: &str
    ) -> Result<(), DbError> {

        self.connect_tcp()?;

        self.send_startup(username, db_name)?;

        let message = self.read_message()?;
        let message = message.parse();

        match message {
            ServerMessage::Authentication(payload) => {


                let auth = AuthKind::parse(&payload)?;


                match auth {
                    AuthKind::SASL(mechanisms) => {
                        self.authenticate_scram(mechanisms, username, password)?;


                        loop {
                            let message = self.read_message()?;
                            let message = message.parse();

                            match message {
                                ServerMessage::ReadyForQuery(_) => {
                                    return Ok(());
                                }

                                ServerMessage::Unknown(message_type, _ ) => {
                                    return Err(DbError::new(format!("Unknown message type: {}", message_type)));
                                }

                                _ => {}
                            }
                        }
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

        if length < 4 {
            return Err(DbError::new(
                String::from("Invalid PostgreSQL message length")
            ));
        }

        let payload_length = length - 4; //la lunghezza del length stesso

        let mut payload = vec![0u8; payload_length as usize];

        stream
            .read_exact(&mut payload)
            .map_err(|e| DbError::new(e.to_string()))?;

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
                DbError::new(
                    String::from("SCRAM-SHA-256 is not supported by server")
                )
            })?;

        let mut scram = ScramClient::new(username, password);

        let first_message = scram.first_message();

        let message = Message::sasl_initial_response(
            mechanism,
            &first_message,
        );

        self.send(&message)?;

        let message = self.read_message()?;

        let message = message.parse();

        match message {
            ServerMessage::Authentication(payload) => {
                let auth = AuthKind::parse(&payload)?;

                match auth {
                    AuthKind::SASLContinue(message) => {
                        scram.handle_server_first(&message)?;

                        let final_message = scram.final_message()?;
                        let message = Message::sasl_response(&final_message);

                        self.send(&message)?;

                        let message = self.read_message()?;
                        let message = message.parse();

                        match message {
                            ServerMessage::Authentication(payload) => {
                                let auth = AuthKind::parse(&payload)?;

                                match auth {
                                    AuthKind::SASLFinal(message) => {
                                        scram.handle_server_final(&message)?;
                                    }
                                    _ => {
                                        return Err(DbError::new(
                                            String::from("Expected SASL final")
                                        ));
                                    }
                                }
                            }
                            _ => {
                                return Err(DbError::new(String::from("Expected Authentication message")));
                            }
                        }
                    }
                    _ => {
                        return Err(DbError::new(String::from("Expected Authentication message")));
                    }
                }
            }
            _ => {
                return Err(DbError::new(String::from("Expected Authentication message")));
            }
        }

        Ok(())
    }

    pub fn exec(&mut self, query: &Query) -> Result<QueryResult, DbError>{
        self.send(&query.encode())?;

        let mut result = QueryResult::new();

        loop {
            let message = self.read_message()?;

            match message.parse() {
                ServerMessage::ErrorResponse(payload) => {
                    return Err(DbError::new(format!("error: {:?}", payload)));
                }

                ServerMessage::ReadyForQuery(_) => {
                    break;
                }

                other => {
                    result.add(other);
                }
            }
        }


        Ok(result)

    }
}


#[derive(Debug)]
pub struct DbError {
    pub message: String
}

impl DbError {
    pub fn new(message: String) -> Self {
        Self {
            message,
        }
    }
}