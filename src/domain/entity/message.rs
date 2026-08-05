use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::Direction;
use super::Channel;
use super::MessageStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(pub Uuid);

impl MessageId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for MessageId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for MessageId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<MessageId> for Uuid {
    fn from(id: MessageId) -> Self { id.0 }
}

impl AsRef<Uuid> for MessageId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for MessageId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Message {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub company_id: Uuid,
    pub direction: Direction,
    pub channel: Channel,
    pub external_id: Option<String>,
    pub address_from: Option<String>,
    pub address_to: Option<String>,
    pub body: String,
    pub status: MessageStatus,
    pub failure_reason: Option<String>,
    pub occurred_at: DateTime<Utc>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Message {
    /// Create a builder for Message
    pub fn builder() -> MessageBuilder {
        MessageBuilder::default()
    }

    /// Create a new Message with required fields
    pub fn new(thread_id: Uuid, company_id: Uuid, direction: Direction, channel: Channel, body: String, status: MessageStatus, occurred_at: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            thread_id,
            company_id,
            direction,
            channel,
            external_id: None,
            address_from: None,
            address_to: None,
            body,
            status,
            failure_reason: None,
            occurred_at,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> MessageId {
        MessageId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.created_at.as_ref()
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.updated_at.as_ref()
    }

    /// Check if this entity is soft deleted
    pub fn is_deleted(&self) -> bool {
        self.metadata.deleted_at.is_some()
    }

    /// Check if this entity is active (not deleted)
    pub fn is_active(&self) -> bool {
        self.metadata.deleted_at.is_none()
    }

    /// Get when this entity was deleted
    pub fn deleted_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.deleted_at.as_ref()
    }

    /// Get who created this entity
    pub fn created_by(&self) -> Option<&Uuid> {
        self.metadata.created_by.as_ref()
    }

    /// Get who last updated this entity
    pub fn updated_by(&self) -> Option<&Uuid> {
        self.metadata.updated_by.as_ref()
    }

    /// Get who deleted this entity
    pub fn deleted_by(&self) -> Option<&Uuid> {
        self.metadata.deleted_by.as_ref()
    }

    /// Get the current status
    pub fn status(&self) -> &MessageStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the external_id field (chainable)
    pub fn with_external_id(mut self, value: String) -> Self {
        self.external_id = Some(value);
        self
    }

    /// Set the address_from field (chainable)
    pub fn with_address_from(mut self, value: String) -> Self {
        self.address_from = Some(value);
        self
    }

    /// Set the address_to field (chainable)
    pub fn with_address_to(mut self, value: String) -> Self {
        self.address_to = Some(value);
        self
    }

    /// Set the failure_reason field (chainable)
    pub fn with_failure_reason(mut self, value: String) -> Self {
        self.failure_reason = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "thread_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.thread_id = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "direction" => {
                    if let Ok(v) = serde_json::from_value(value) { self.direction = v; }
                }
                "channel" => {
                    if let Ok(v) = serde_json::from_value(value) { self.channel = v; }
                }
                "external_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.external_id = v; }
                }
                "address_from" => {
                    if let Ok(v) = serde_json::from_value(value) { self.address_from = v; }
                }
                "address_to" => {
                    if let Ok(v) = serde_json::from_value(value) { self.address_to = v; }
                }
                "body" => {
                    if let Ok(v) = serde_json::from_value(value) { self.body = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "failure_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.failure_reason = v; }
                }
                "occurred_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.occurred_at = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Message {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Message"
    }
}

impl backbone_core::PersistentEntity for Message {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.created_at
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.created_at = Some(ts);
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.updated_at
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.updated_at = Some(ts);
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.deleted_at
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        self.metadata.deleted_at = ts;
    }
}

impl backbone_orm::EntityRepoMeta for Message {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("thread_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("direction".to_string(), "direction".to_string());
        m.insert("channel".to_string(), "channel".to_string());
        m.insert("status".to_string(), "message_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["body"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for Message entity
///
/// Provides a fluent API for constructing Message instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct MessageBuilder {
    thread_id: Option<Uuid>,
    company_id: Option<Uuid>,
    direction: Option<Direction>,
    channel: Option<Channel>,
    external_id: Option<String>,
    address_from: Option<String>,
    address_to: Option<String>,
    body: Option<String>,
    status: Option<MessageStatus>,
    failure_reason: Option<String>,
    occurred_at: Option<DateTime<Utc>>,
}

impl MessageBuilder {
    /// Set the thread_id field (required)
    pub fn thread_id(mut self, value: Uuid) -> Self {
        self.thread_id = Some(value);
        self
    }

    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the direction field (required)
    pub fn direction(mut self, value: Direction) -> Self {
        self.direction = Some(value);
        self
    }

    /// Set the channel field (required)
    pub fn channel(mut self, value: Channel) -> Self {
        self.channel = Some(value);
        self
    }

    /// Set the external_id field (optional)
    pub fn external_id(mut self, value: String) -> Self {
        self.external_id = Some(value);
        self
    }

    /// Set the address_from field (optional)
    pub fn address_from(mut self, value: String) -> Self {
        self.address_from = Some(value);
        self
    }

    /// Set the address_to field (optional)
    pub fn address_to(mut self, value: String) -> Self {
        self.address_to = Some(value);
        self
    }

    /// Set the body field (required)
    pub fn body(mut self, value: String) -> Self {
        self.body = Some(value);
        self
    }

    /// Set the status field (default: `MessageStatus::default()`)
    pub fn status(mut self, value: MessageStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the failure_reason field (optional)
    pub fn failure_reason(mut self, value: String) -> Self {
        self.failure_reason = Some(value);
        self
    }

    /// Set the occurred_at field (default: `Utc::now()`)
    pub fn occurred_at(mut self, value: DateTime<Utc>) -> Self {
        self.occurred_at = Some(value);
        self
    }

    /// Build the Message entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Message, String> {
        let thread_id = self.thread_id.ok_or_else(|| "thread_id is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let direction = self.direction.ok_or_else(|| "direction is required".to_string())?;
        let channel = self.channel.ok_or_else(|| "channel is required".to_string())?;
        let body = self.body.ok_or_else(|| "body is required".to_string())?;

        Ok(Message {
            id: Uuid::new_v4(),
            thread_id,
            company_id,
            direction,
            channel,
            external_id: self.external_id,
            address_from: self.address_from,
            address_to: self.address_to,
            body,
            status: self.status.unwrap_or(MessageStatus::default()),
            failure_reason: self.failure_reason,
            occurred_at: self.occurred_at.unwrap_or(Utc::now()),
            metadata: AuditMetadata::default(),
        })
    }
}
