use crate::{
    Connection,
    DbError,
    backend::Backend,
    mysql_protocol::message::Message,
    query::{Query, QueryResult},
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
        Ok(())
    }




    fn exec(&mut self, query: &Query) -> Result<QueryResult, DbError> {
        Ok(QueryResult::new())
    }
}