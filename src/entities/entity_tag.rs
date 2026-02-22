use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityTagData {
    content: String,
    offset: Option<f32>,
    font_size: Option<i32>,
    outline_size: Option<i32>,
}

impl EntityTagData {
    pub fn create(content: String, offset: Option<f32>, font_size: Option<i32>, outline_size: Option<i32>) -> Self {
        Self {
            content,
            offset,
            font_size,
            outline_size,
        }
    }

    pub fn get_offset(&self) -> Option<&f32> {
        self.offset.as_ref()
    }

    pub fn get_outline_size(&self) -> Option<&i32> {
        self.outline_size.as_ref()
    }

    pub fn get_font_size(&self) -> Option<&i32> {
        self.font_size.as_ref()
    }

    pub fn get_content(&self) -> &String {
        &self.content
    }
}
