use crate::{
    Connection, DbError, backend::Backend, mysql_protocol::message::{ServerMessage, Handshake, Message}, query::{Query, QueryResult},
};
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

        let payload_length = u32::from_le_bytes([
            length_buffer[0],
            length_buffer[1],
            length_buffer[2],
            0,
        ]);

        let mut sequence_buffer = [0u8; 1];

        self.connection.read(&mut sequence_buffer)?;

        let sequence_id = sequence_buffer[0];


        let mut payload = vec![0u8; payload_length as usize];

        self.connection.read(&mut payload)?;

        if payload.is_empty() {
            return Err(DbError::new(
                String::from("Empty MySQL packet")
            ));
        }

        let message_type = payload[0];

        payload = payload[1..].to_vec();

        Ok(Message::new(message_type, sequence_id, payload))
    }


}

impl<'a> Backend for MysqlBackend<'a> {
    fn open(
        &mut self,
        username: &str,
        password: &str,
        db_name: &str,
    ) -> Result<(), DbError> {

        let message = self.read_message()?;
        let message = message.parse();

        match message {
            ServerMessage::Handshake(payload) => {
                let handshake = Handshake::parse(&payload)?;

                let auth_response = Handshake::scramble_password(
                    password,
                    &handshake.auth_plugin_data,
                );

                let response = Message::handshake_response(
                    username,
                    &auth_response,
                    db_name,
                    handshake.capability_flags,
                    handshake.character_set,
                    &handshake.auth_plugin_name,
                );

                let packet = Message::encode(&response, 1);

                self.connection.write(&packet)?;

                /*let response = self.read_message()?;

                match response.parse() {
                    ServerMessage::Ok(payload) => {
                        println!("MySQL authentication OK: {:02X?}", payload);
                    }

                    ServerMessage::Error(payload) => {
                        return Err(DbError::new(
                            format!("MySQL authentication error: {:02X?}", payload)
                        ));
                    }

                    _ => {
                        return Err(DbError::new(
                            String::from("Unexpected MySQL authentication response")
                        ));
                    }
                }*/

                
            }

            ServerMessage::Error(payload) => {
                return Err(DbError::new(
                    format!("MySQL handshake error: {:?}", payload)
                ));
            }

            _ => {
                return Err(DbError::new(
                    String::from("Expected MySQL handshake")
                ));
            }
        }


        Ok(())
    }




    fn exec(&mut self, query: &Query) -> Result<QueryResult, DbError> {
        Ok(QueryResult::new())
    }
}