use rand::{distr::Alphanumeric, Rng};
use base64::{engine::general_purpose, Engine};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use pbkdf2::pbkdf2_hmac;

use crate::DbError;

type HmacSha256 = Hmac<Sha256>;


pub struct ScramClient {
    username: String,
    password: String,
    client_nonce: String,

    server_nonce: Option<String>,
    salt: Option<String>,
    iterations: Option<u32>,

    server_first_message: Option<String>,
}

impl ScramClient {
    pub fn new(username: &str, password: &str) -> Self {
        Self {
            username: username.to_string(),
            password: password.to_string(),
            client_nonce: Self::generate_client_nonce(),

            server_nonce: None,
            salt: None,
            iterations: None,

            server_first_message: None,
        }
    }

    pub fn client_nonce(&self) -> &str {
        &self.client_nonce
    }

    pub fn first_message(&self) -> String {
        format!(
            "n,,n={},r={}",
            self.username,
            self.client_nonce
        )
    }

    pub fn final_message(&mut self) -> Result<String, DbError> {
        let client_proof = self.calculate_client_proof()?;

        let server_nonce = self.server_nonce.as_ref()
            .ok_or_else(|| DbError::new(
                String::from("Missing server nonce")
            ))?;

        Ok(format!(
            "c=biws,r={},p={}",
            server_nonce,
            client_proof
        ))
    }

    fn calculate_client_proof(&self) -> Result<String, DbError> {
        let salt = self.salt.as_ref()
            .ok_or_else(|| DbError::new(
                String::from("Missing SCRAM salt")
            ))?;

        let iterations = self.iterations
            .ok_or_else(|| DbError::new(
                String::from("Missing SCRAM iterations")
            ))?;

        let salt = general_purpose::STANDARD
            .decode(salt)
            .map_err(|_| DbError::new(
                String::from("Invalid SCRAM salt")
            ))?;

        // SaltedPassword
        let mut salted_password = [0u8; 32];

        pbkdf2_hmac::<Sha256>(
            self.password.as_bytes(),
            &salt,
            iterations,
            &mut salted_password,
        );

        // ClientKey
        let mut mac = HmacSha256::new_from_slice(&salted_password)
            .map_err(|_| DbError::new(
                String::from("Failed to create HMAC")
            ))?;

        mac.update(b"Client Key");

        let client_key = mac.finalize().into_bytes();

        // StoredKey
        let mut hasher = Sha256::new();

        hasher.update(&client_key);

        let stored_key = hasher.finalize();


        // AuthMessage
        let auth_message = self.auth_message()?;

        // ClientSignature
        let mut mac = HmacSha256::new_from_slice(&stored_key)
            .map_err(|_| DbError::new(
                String::from("Failed to create HMAC")
            ))?;

        mac.update(auth_message.as_bytes());

        let client_signature = mac.finalize().into_bytes();

        // ClientProof = ClientKey XOR ClientSignature
        let mut client_proof = [0u8; 32];

        for i in 0..32 {
            client_proof[i] = client_key[i] ^ client_signature[i];
        }

        Ok(
            general_purpose::STANDARD.encode(client_proof)
        )
    }
    fn generate_client_nonce() -> String {
        let mut rng = rand::rng();

        (&mut rng)
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    }


    pub fn handle_server_first(
    &mut self,
    message: &str,
    ) -> Result<(), DbError> {

        let mut server_nonce = None;
        let mut salt = None;
        let mut iterations = None;

        for part in message.split(',') {
            let (key, value) = part.split_once('=')
                .ok_or_else(|| DbError::new(
                    String::from("Invalid SCRAM server-first-message")
                ))?;

            match key {
                "r" => server_nonce = Some(value.to_string()),
                "s" => salt = Some(value.to_string()),
                "i" => {
                    iterations = Some(
                        value.parse::<u32>()
                            .map_err(|_| DbError::new(
                                String::from("Invalid SCRAM iteration count")
                            ))?
                    );
                }
                _ => {}
            }
        }

        let server_nonce = server_nonce
            .ok_or_else(|| DbError::new(String::from("Missing server nonce")))?;

        let salt = salt
            .ok_or_else(|| DbError::new(String::from("Missing SCRAM salt")))?;

        let iterations = iterations
            .ok_or_else(|| DbError::new(String::from("Missing SCRAM iterations")))?;

        if !server_nonce.starts_with(&self.client_nonce) {
            return Err(DbError::new(
                String::from("Server nonce does not contain client nonce")
            ));
        }

        self.server_nonce = Some(server_nonce);
        self.salt = Some(salt);
        self.iterations = Some(iterations);

        self.server_first_message = Some(message.to_string());

        Ok(())
    }

    fn auth_message(&self) -> Result<String, DbError> {

        let client_first_bare = format!(
            "n={},r={}",
            self.username,
            self.client_nonce
        );

        let server_first_message = self.server_first_message.as_ref()
            .ok_or_else(|| DbError::new(
                String::from("Missing server first message")
            ))?;

        let server_nonce = self.server_nonce.as_ref()
            .ok_or_else(|| DbError::new(
                String::from("Missing server nonce")
            ))?;

        let client_final_without_proof = format!(
            "c=biws,r={}",
            server_nonce
        );


        let auth_message = format!(
            "{},{},{}",
            client_first_bare,
            server_first_message,
            client_final_without_proof
        );

        Ok(auth_message)
    }

    pub fn handle_server_final(
    &mut self,
    message: &str,
    ) -> Result<(), DbError> {


        let (key, value) = message.split_once('=')
            .ok_or_else(|| DbError::new(
                String::from("Invalid SCRAM server-final-message")
            ))?;

        match key {
            "v" => {
                let received_signature = general_purpose::STANDARD
                    .decode(value)
                    .map_err(|_| DbError::new(
                        String::from("Invalid SCRAM server signature")
                    ))?;
                let expected_signature = self.server_signature()?;

                if received_signature != expected_signature {
                    return Err(DbError::new(
                        String::from("SCRAM server signature mismatch")
                    ));
                }
            }

            "e" => {
                // Il server sta comunicando un errore SCRAM
                return Err(DbError::new(
                    format!("SCRAM authentication failed: {}", value)
                ));
            }

            _ => {
                return Err(DbError::new(
                    String::from("Invalid SCRAM server-final-message")
                ));
            }
        }

        Ok(())
    }


    fn server_signature(&self) -> Result<Vec<u8>, DbError> {
        let salt = self.salt.as_ref()
            .ok_or_else(|| DbError::new(
                String::from("Missing SCRAM salt")
            ))?;

        let iterations = self.iterations
            .ok_or_else(|| DbError::new(
                String::from("Missing SCRAM iterations")
            ))?;

        let salt = general_purpose::STANDARD
            .decode(salt)
            .map_err(|_| DbError::new(
                String::from("Invalid SCRAM salt")
            ))?;

        // SaltedPassword
        let mut salted_password = [0u8; 32];

        pbkdf2_hmac::<Sha256>(
            self.password.as_bytes(),
            &salt,
            iterations,
            &mut salted_password,
        );

        // ServerKey
        let mut mac = HmacSha256::new_from_slice(&salted_password)
            .map_err(|_| DbError::new(
                String::from("Failed to create HMAC")
            ))?;

        mac.update(b"Server Key");

        let server_key = mac.finalize().into_bytes();

        // AuthMessage
        let auth_message = self.auth_message()?;

        // ServerSignature
        let mut mac = HmacSha256::new_from_slice(&server_key)
            .map_err(|_| DbError::new(
                String::from("Failed to create HMAC")
            ))?;

        mac.update(auth_message.as_bytes());

        Ok(mac.finalize().into_bytes().to_vec())
    }
}
