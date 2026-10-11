// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

pub mod communication_events;
pub mod communication_ports;
pub mod communication_write_service;
pub use communication_events::{
    CommunicationEvent, CommunicationEventSink, LoggingSink, MessageReceived,
};
pub use communication_ports::{ChannelAck, ChannelPort, ChannelRejected, OutboundSend};
pub use communication_write_service::{
    CommError, CommunicationWriteService, InboundMessage, InboundOutcome,
};
