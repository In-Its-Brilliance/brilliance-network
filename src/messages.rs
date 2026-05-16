use common::blocks::block_type::BlockType;
use common::chunks::block_position::BlockPosition;
use common::chunks::chunk_data::BlockDataInfo;
use common::chunks::chunk_data::{BlockIndexType, ChunkData};
use common::chunks::chunk_position::ChunkPosition;
use common::chunks::position::Vector3;
use common::chunks::rotation::Rotation;
use common::inventory::inventory::{ClientInventory, InventoryType};
use common::inventory::item::ClientItem;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use strum_macros::AsRefStr;
use strum_macros::Display;

use crate::entities::{AnimationState, EntityNetworkComponent};

#[derive(Debug, Serialize, Deserialize, Clone, Display)]
pub enum ClientMessages {
    ConnectionInfo {
        login: String,
        version: String,
        architecture: String,
        rendering_device: String,
    },
    ConsoleInput {
        command: String,
    },
    PlayerMove {
        position: Vector3,
        rotation: Rotation,
        animation_state: AnimationState,
    },
    ChunkRecieved {
        chunk_positions: Vec<ChunkPosition>,
    },
    ClientScriptEvent {
        script_slug: String,
        slug: String,
        json: String,
    },
    ResourcesHasCache {
        exists: bool,
    },
    ResourcesLoaded {
        last_index: u32,
    },
    SettingsLoaded,

    InventoryAction(InventoryAction),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum InventoryAction {
    Move {
        from_inventory: InventoryType,
        from_slot: u16,

        to_inventory: InventoryType,
        to_slot: u16,

        amount: u16,
    },
    Swap {
        a_inventory: InventoryType,
        a_slot: u16,

        b_inventory: InventoryType,
        b_slot: u16,
    },
    Split {
        from_inventory: InventoryType,
        from_slot: u16,

        to_inventory: InventoryType,
        to_slot: u16,

        amount: u16,
    },
    Drop {
        inventory: InventoryType,
        slot: u16,
        amount: u16,
    },
    Close {
        inventory: InventoryType,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResurceScheme {
    pub slug: String,

    // Hash: name
    pub scripts: HashMap<String, String>,

    // Hash: name
    pub media: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Display, AsRefStr)]
#[strum(serialize_all = "kebab-case")]
pub enum ServerMessages {
    AllowConnection,
    ConsoleOutput {
        message: String,
    },
    Disconnect {
        message: Option<String>,
    },

    // Information about server resources (media, scripts)
    ResourcesScheme {
        list: Vec<ResurceScheme>,
        archive_hash: u64,
    },
    ResourcesPart {
        index: u32,
        total: u32,
        data: Vec<u8>,
    },
    Settings {
        block_types: Vec<BlockType>,
        block_id_map: BTreeMap<BlockIndexType, String>,
        chunks_distance: u16,
    },

    SpawnWorld {
        world_slug: String,
    },
    UpdatePlayerComponent {
        component: EntityNetworkComponent,
    },
    PlayerSpawn {
        world_slug: String,
        position: Vector3,
        rotation: Rotation,
        components: Vec<EntityNetworkComponent>,
    },
    ChunkSectionInfo {
        world_slug: String,
        chunk_position: ChunkPosition,
        sections: ChunkData,
    },
    ChunkSectionInfoEncoded {
        world_slug: String,
        chunk_position: ChunkPosition,
        encoded: Vec<u8>,
    },
    UnloadChunks {
        world_slug: String,
        chunks: Vec<ChunkPosition>,
    },

    // In case the entity gets in the player's line of sight
    StartStreamingEntity {
        world_slug: String,
        id: u32,
        position: Vector3,
        rotation: Rotation,
        components: Vec<EntityNetworkComponent>,
    },
    UpdateEntityComponent {
        world_slug: String,
        id: u32,
        component: EntityNetworkComponent,
    },
    // In case the entity escapes from the visible chunk or is deleted
    StopStreamingEntities {
        world_slug: String,
        ids: Vec<u32>,
    },
    EntityMove {
        world_slug: String,
        id: u32,
        position: Vector3,
        rotation: Rotation,
        animation_state: AnimationState,
        /// Server time in seconds since startup
        timestamp: f64,
    },

    EditBlock {
        world_slug: String,
        position: BlockPosition,
        new_block_info: Option<BlockDataInfo>,
    },

    ServerStatus {
        tps: f32,
    },

    InventoryStream(InventoryStream),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventorySlotChange {
    pub slot: usize,
    pub item: Option<ClientItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum InventoryStream {
    // Not sended in case of PlayerPersonal, its always streaming to the player
    StartStream {
        inventory_type: InventoryType,
        inventory: ClientInventory,
    },
    // Not sended in case of PlayerPersonal, its always streaming to the player
    StopStream {
        inventory_type: InventoryType,
    },
    UpdateSlots {
        inventory_type: InventoryType,
        changes: Vec<InventorySlotChange>,
    },
}

pub enum NetworkMessageType {
    ReliableOrdered,
    ReliableUnordered,
    Unreliable,
    WorldInfo,
}
