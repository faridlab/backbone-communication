use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::Channel;
use super::ThreadStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Thread
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThreadId(pub Uuid);

impl ThreadId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ThreadId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ThreadId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ThreadId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ThreadId> for Uuid {
    fn from(id: ThreadId) -> Self { id.0 }
}

impl AsRef<Uuid> for ThreadId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ThreadId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Thread {
    pub id: Uuid,
    pub company_id: Uuid,
    pub channel: Channel,
    pub party_id: Option<Uuid>,
    pub subject_type: Option<String>,
    pub subject_id: Option<Uuid>,
    pub external_ref: Option<String>,
    pub status: ThreadStatus,
    pub last_message_at: Option<DateTime<Utc>>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Thread {
    /// Create a builder for Thread
    pub fn builder() -> ThreadBuilder {
        ThreadBuilder::default()
    }

    /// Create a new Thread with required fields
    pub fn new(company_id: Uuid, channel: Channel, status: ThreadStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            channel,
            party_id: None,
            subject_type: None,
            subject_id: None,
            external_ref: None,
            status,
            last_message_at: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ThreadId {
        ThreadId(self.id)
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
    pub fn status(&self) -> &ThreadStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the party_id field (chainable)
    pub fn with_party_id(mut self, value: Uuid) -> Self {
        self.party_id = Some(value);
        self
    }

    /// Set the subject_type field (chainable)
    pub fn with_subject_type(mut self, value: String) -> Self {
        self.subject_type = Some(value);
        self
    }

    /// Set the subject_id field (chainable)
    pub fn with_subject_id(mut self, value: Uuid) -> Self {
        self.subject_id = Some(value);
        self
    }

    /// Set the external_ref field (chainable)
    pub fn with_external_ref(mut self, value: String) -> Self {
        self.external_ref = Some(value);
        self
    }

    /// Set the last_message_at field (chainable)
    pub fn with_last_message_at(mut self, value: DateTime<Utc>) -> Self {
        self.last_message_at = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "channel" => {
                    if let Ok(v) = serde_json::from_value(value) { self.channel = v; }
                }
                "party_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.party_id = v; }
                }
                "subject_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.subject_type = v; }
                }
                "subject_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.subject_id = v; }
                }
                "external_ref" => {
                    if let Ok(v) = serde_json::from_value(value) { self.external_ref = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "last_message_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.last_message_at = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Thread {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Thread"
    }
}

impl backbone_core::PersistentEntity for Thread {
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

impl backbone_orm::EntityRepoMeta for Thread {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("party_id".to_string(), "uuid".to_string());
        m.insert("subject_id".to_string(), "uuid".to_string());
        m.insert("channel".to_string(), "channel".to_string());
        m.insert("status".to_string(), "thread_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
}

/// Builder for Thread entity
///
/// Provides a fluent API for constructing Thread instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ThreadBuilder {
    company_id: Option<Uuid>,
    channel: Option<Channel>,
    party_id: Option<Uuid>,
    subject_type: Option<String>,
    subject_id: Option<Uuid>,
    external_ref: Option<String>,
    status: Option<ThreadStatus>,
    last_message_at: Option<DateTime<Utc>>,
}

impl ThreadBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the channel field (default: `Channel::default()`)
    pub fn channel(mut self, value: Channel) -> Self {
        self.channel = Some(value);
        self
    }

    /// Set the party_id field (optional)
    pub fn party_id(mut self, value: Uuid) -> Self {
        self.party_id = Some(value);
        self
    }

    /// Set the subject_type field (optional)
    pub fn subject_type(mut self, value: String) -> Self {
        self.subject_type = Some(value);
        self
    }

    /// Set the subject_id field (optional)
    pub fn subject_id(mut self, value: Uuid) -> Self {
        self.subject_id = Some(value);
        self
    }

    /// Set the external_ref field (optional)
    pub fn external_ref(mut self, value: String) -> Self {
        self.external_ref = Some(value);
        self
    }

    /// Set the status field (default: `ThreadStatus::default()`)
    pub fn status(mut self, value: ThreadStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the last_message_at field (optional)
    pub fn last_message_at(mut self, value: DateTime<Utc>) -> Self {
        self.last_message_at = Some(value);
        self
    }

    /// Build the Thread entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Thread, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;

        Ok(Thread {
            id: Uuid::new_v4(),
            company_id,
            channel: self.channel.unwrap_or(Channel::default()),
            party_id: self.party_id,
            subject_type: self.subject_type,
            subject_id: self.subject_id,
            external_ref: self.external_ref,
            status: self.status.unwrap_or(ThreadStatus::default()),
            last_message_at: self.last_message_at,
            metadata: AuditMetadata::default(),
        })
    }
}
