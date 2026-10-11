// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// The hand-written communication SQL's parameter/projection types (see `message_repository` and
// `thread_repository`, both declared `user_owned` in metaphor.codegen.yaml).
pub use message_repository::{MessageRefRow, NewInboundMessageRow, NewOutboundMessageRow};
pub use thread_repository::{NewRoutedThreadRow, NewThreadRow, RoutedThreadRow, ThreadSendRow};
