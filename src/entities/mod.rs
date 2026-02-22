use entity_tag::EntityTagData;
use serde::{Deserialize, Serialize};

pub mod entity_tag;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EntitySkinData {
    Generic,
    Fixed(String),
    None,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EntityNetworkComponent {
    Tag(Option<EntityTagData>),
    Skin(EntitySkinData),
}
