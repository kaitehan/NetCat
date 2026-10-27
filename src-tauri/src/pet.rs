use serde::{Deserialize, Serialize};

use crate::{error::AppError, storage::Store};

const PET_PREFERENCES_KEY: &str = "desktop_pet_preferences";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PetEdge {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PetPlacement {
    pub monitor_name: Option<String>,
    pub edge: PetEdge,
    pub offset: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PetPreferences {
    pub version: u8,
    pub enabled: bool,
    pub placement: Option<PetPlacement>,
}

impl Default for PetPreferences {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: true,
            placement: None,
        }
    }
}

fn valid_placement(placement: &PetPlacement) -> bool {
    placement.offset.is_finite() && (0.0..=1.0).contains(&placement.offset)
}

fn valid_preferences(preferences: &PetPreferences) -> bool {
    preferences.version == 1 && preferences.placement.as_ref().is_none_or(valid_placement)
}

fn persist_preferences(store: &Store, preferences: &PetPreferences) -> Result<(), AppError> {
    let serialized = serde_json::to_string(preferences).map_err(|error| {
        AppError::new("SERIALIZE_ERROR", "无法保存桌面小橘设置").with_detail(error.to_string())
    })?;
    store.set_setting(PET_PREFERENCES_KEY, &serialized)
}

pub fn load_pet_preferences(store: &Store) -> Result<PetPreferences, AppError> {
    let Some(serialized) = store.get_setting(PET_PREFERENCES_KEY)? else {
        return Ok(PetPreferences::default());
    };
    Ok(serde_json::from_str::<PetPreferences>(&serialized)
        .ok()
        .filter(valid_preferences)
        .unwrap_or_default())
}

pub fn set_pet_enabled(store: &Store, enabled: bool) -> Result<PetPreferences, AppError> {
    let mut preferences = load_pet_preferences(store)?;
    preferences.enabled = enabled;
    persist_preferences(store, &preferences)?;
    Ok(preferences)
}

pub fn save_pet_placement(
    store: &Store,
    placement: PetPlacement,
) -> Result<PetPreferences, AppError> {
    if !valid_placement(&placement) {
        return Err(AppError::new("PET_PLACEMENT_INVALID", "桌面小橘位置无效"));
    }
    let mut preferences = load_pet_preferences(store)?;
    preferences.placement = Some(placement);
    persist_preferences(store, &preferences)?;
    Ok(preferences)
}
