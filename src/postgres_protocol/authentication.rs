use crate::DbError;

pub enum AuthKind {
    AuthenticationOk,
    KerberosV5,
    ClearTextPassword,
    MD5Password([u8; 4]),
    SASL(Vec<String>),
    SASLContinue(String),
    SASLFinal(String),
    Unknown(u32),
}

impl AuthKind {
    pub fn parse(payload: &[u8]) -> Result<Self, DbError> {
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
            0 => Ok(Self::AuthenticationOk),

            2 => Ok(Self::KerberosV5),

            3 => Ok(Self::ClearTextPassword),

            5 => {
                if payload.len() < 8 {
                    return Err(DbError::new(
                        String::from("Invalid MD5 authentication payload")
                    ));
                }

                let salt = payload[4..8]
                    .try_into()
                    .map_err(|_| {
                        DbError::new(
                            String::from("Invalid MD5 authentication payload")
                        )
                    })?;

                Ok(Self::MD5Password(salt))
            }

            10 => {
                let mechanisms = Self::parse_sasl_mechanisms(&payload[4..])?;

                Ok(Self::SASL(mechanisms))
            }

            11 => {
                let message = Self::parse_sasl_continue(&payload[4..])?;
                Ok(Self::SASLContinue(message))
            }

            12 => {
                let message = Self::parse_sasl_final(&payload[4..])?;
                Ok(Self::SASLFinal(message))
            }

            other => Ok(Self::Unknown(other)),
        }
    }

    fn parse_sasl_mechanisms(
        payload: &[u8],
    ) -> Result<Vec<String>, DbError> {
        let mut mechanisms = Vec::new();
        let mut start = 0;

        for i in 0..payload.len() {
            if payload[i] == 0 {
                if i == start {
                    break;
                }

                let mechanism = std::str::from_utf8(
                    &payload[start..i]
                )
                .map_err(|_| {
                    DbError::new(
                        String::from("Invalid UTF-8 in SASL mechanism")
                    )
                })?;

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


    fn parse_sasl_continue(payload: &[u8]) -> Result<String, DbError> {
        let message = std::str::from_utf8(&payload)
            .map_err(|_| DbError::new(
                String::from("Invalid UTF-8 in SASL continue message")
            ))?;

        Ok(message.to_string())
    }


    fn parse_sasl_final(payload: &[u8]) -> Result<String, DbError> {
        let message = std::str::from_utf8(&payload)
            .map_err(|_| DbError::new(
                String::from("Invalid UTF-8 in SASL final message")
            ))?;

        Ok(message.to_string())
    }
}