use std::collections::HashMap;

use common::inventory::item::BodyPart;
use entity_tag::EntityTagData;
use serde::{Deserialize, Serialize};

pub mod entity_tag;
mod animation_clip;
pub use animation_clip::AnimationClip;
mod animation_state;
pub use animation_state::AnimationState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EntitySkinData {
    Generic { body_parts: HashMap<BodyPart, String> },
    Fixed(String),
    None,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EntityNetworkComponent {
    Tag(Option<EntityTagData>),
    Skin(EntitySkinData),
}
