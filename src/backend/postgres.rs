use crate::{Connection, DbError, backend::Backend, postgres_protocol::{authentication::AuthKind, message::{Message, ServerMessage}, scram::ScramClient}, query::{Query, QueryResult}};

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
            return Err(DbError::new(
                String::from("Invalid PostgreSQL message length")
            ));
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

        self.connection.write(&message)?;

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

                        self.connection.write(&message)?;

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
                                            String::from("Expected SASL final message")
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
                        return Err(DbError::new(String::from("Expected SASL Continue message")));
                    }
                }
            }
            _ => {
                return Err(DbError::new(String::from("Expected Authentication message")));
            }
        }

        Ok(())
    }
}

impl<'a> Backend for PostgresBackend<'a> {
    fn open(
        &mut self,
        username: &str,
        password: &str,
        db_name: &str,
    ) -> Result<(), DbError> {

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




    fn exec(&mut self, query: &Query) -> Result<QueryResult, DbError> {

        let message = Message::query(query.sql());
        self.connection.write(&message)?;

        let mut result = QueryResult::new();
        let mut error = None;

        loop {
            let message = self.read_message()?;

            match message.parse() {
                ServerMessage::ErrorResponse(payload) => {
                    error = Some(DbError::new(format!("error: {:?}", payload)));
                }

                ServerMessage::ReadyForQuery(_) => {
                    break;
                }

                other => {
                    result.add(other);
                }
            }
        }

        if let Some(error) = error {
            return Err(error);
        }

        Ok(result)
    }
}