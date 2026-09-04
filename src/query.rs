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

    pub fn sql(&self) -> &str {
        &self.sql
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