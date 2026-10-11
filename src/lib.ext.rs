//! The module's extension (hand-written; ADR-0031): what the module adds beside its
//! generated services. The generated module and builder carry `ModuleExt` and
//! `ModuleBuilderExt` and dereference to them, so their fields read as `module.field`.

#[allow(unused_imports)]
use super::*;

/// State the module adds; read through the generated module's `Deref`.
pub struct ModuleExt {
    /// The validated write path for the communication domain (idempotent inbound
    /// ingest, thread routing, outbound send/deliver). This is the module's
    /// deliberate outward write contract — the front door, in contrast to the
    /// unguarded generic CRUD mutators on the sibling services.
    pub(crate) write_service: Arc<crate::application::service::CommunicationWriteService>,
}

/// State the builder adds.
pub struct ModuleBuilderExt {
}

impl Default for ModuleBuilderExt {
    fn default() -> Self {
        Self {
        }
    }
}

impl ModuleBuilderExt {
    /// Build the extension's state from what the generated build made.
    #[allow(unused_variables, clippy::redundant_clone)]
    pub(crate) fn build(self, parts: &ModuleParts<'_>) -> anyhow::Result<ModuleExt> {
        let db_pool = parts.db_pool.clone();
        let write_service = Arc::new(crate::application::service::CommunicationWriteService::new(
            db_pool.clone(),
        ));
        Ok(ModuleExt {
            write_service,
        })
    }
}

impl crate::ThisModule {
    /// The validated write service for the communication domain.
    ///
    /// Consumers should call this (not the generic CRUD mutators) for any production
    /// write: it enforces idempotent inbound ingest (`unique(channel, external_id)`),
    /// thread routing onto `party_id`, and the outbound queue/sent/failed/delivered
    /// transitions.
    pub fn write_service(&self) -> Arc<crate::application::service::CommunicationWriteService> {
        self.write_service.clone()
    }
    /// The outward READ contract for sibling modules (`CommunicationQueryService`),
    /// ready-constructed over the same CRUD services. Cheap to build per use.
    pub fn query_service(&self) -> crate::exports::CommunicationQueryServiceImpl {
        crate::exports::CommunicationQueryServiceImpl::new(
            self.message_service.clone(),
            self.thread_service.clone(),
        )
    }
}
