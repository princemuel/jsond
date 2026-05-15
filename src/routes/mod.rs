//! Axum router construction and route handlers for collections and singletons.
//!
//! Routes are registered dynamically based on what is in the database.
//! Collections get full CRUD; singletons get GET/PUT/PATCH.
//! A catch-all layer dispatches unknown resources through the same handlers —
//! because json-server supports creating resources that don't exist yet (POST).
pub mod collection;
pub mod root;
pub mod singleton;
