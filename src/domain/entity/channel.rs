use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "channel", rename_all = "snake_case")]
pub enum Channel {
    Whatsapp,
    Email,
    Sms,
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Whatsapp => write!(f, "whatsapp"),
            Self::Email => write!(f, "email"),
            Self::Sms => write!(f, "sms"),
        }
    }
}

impl FromStr for Channel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "whatsapp" => Ok(Self::Whatsapp),
            "email" => Ok(Self::Email),
            "sms" => Ok(Self::Sms),
            _ => Err(format!("Unknown Channel variant: {}", s)),
        }
    }
}

impl Default for Channel {
    fn default() -> Self {
        Self::Whatsapp
    }
}
