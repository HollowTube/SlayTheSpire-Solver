//! Published JSON Schema for [`GameState`].

use crate::state::GameState;

pub fn game_state_schema() -> schemars::Schema {
    schemars::schema_for!(GameState)
}

/// The schema as checked in at `schema/game_state.schema.json`.
pub fn game_state_schema_json() -> String {
    let mut json =
        serde_json::to_string_pretty(&game_state_schema()).expect("a schema always serializes");
    json.push('\n');
    json
}
