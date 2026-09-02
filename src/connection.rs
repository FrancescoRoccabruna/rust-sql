use std::{
    io::{Read, Write},
    net::TcpStream,
};

use crate::{
    backend::{postgres::PostgresBackend, Backend},
    config::DatabaseKind,
    query::{Query, QueryResult},
};



pub struct Connection {
    host: String,
    port: u16,
    kind: DatabaseKind,
    stream: Option<TcpStream>,
}

impl Connection {
    pub fn new(host: String, port: u16, kind: DatabaseKind) -> Self {
        Self {
            host,
            port,
            kind,
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


    pub fn open(
        &mut self,
        username: &str,
        password: &str,
        db_name: &str
    ) -> Result<(), DbError> {

        self.connect_tcp()?;

        let mut backend = PostgresBackend::new(self);
        backend.open(username, password, db_name)
    }

    pub fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    pub(crate) fn write(&mut self, data: &[u8]) -> Result<(), DbError> {
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

    pub(crate) fn read(&mut self, buffer: &mut [u8]) -> Result<(), DbError>{
        let stream = match &mut self.stream {
            Some(stream) => stream,
            None => {
                return Err(DbError::new(String::from("Connection is not open")));
            }
        };

        stream
            .read_exact(buffer)
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }


    pub fn exec(&mut self, query: &Query) -> Result<QueryResult, DbError>{
        let mut backend = PostgresBackend::new(self);

        backend.exec(query)

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

