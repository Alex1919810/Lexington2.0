mod card;
pub mod character_memory;
mod context;
mod creation;
mod profile;
mod registry;
mod store;
mod vector_memory;

pub use card::{RoleCard, Rule, WorldEntry};
pub use context::{ContextPlan, ContextSettings, RoleplaySession, SceneState, static_plan};
pub use creation::create_character;
pub use profile::{Memory, UserProfile, session_card};
pub use registry::{agent_instructions, default_character};
pub use store::save_role_card;
pub use vector_memory::{EmbeddingSource, VectorMemorySettings, clear_vector_memory};
