use crate::postgres_protocol::message::ServerMessage;


pub struct Query {
    sql: String,
}


impl Query {
    pub fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut query = Vec::new();

        query.push(b'Q');

        // Lunghezza: 4 byte length + query + terminatore NUL
        let length = (4 + self.sql.len() + 1) as u32;
        query.extend_from_slice(&length.to_be_bytes());

        query.extend_from_slice(self.sql.as_bytes());
        query.push(0);

        query
    }
}

pub struct QueryResult {
    messages: Vec<ServerMessage>
}


impl  QueryResult {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn add(&mut self, message: ServerMessage){
        self.messages.push(message);
    }
}