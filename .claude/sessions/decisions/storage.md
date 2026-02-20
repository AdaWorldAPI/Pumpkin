# Storage — Decisions

## STOR-001: Player data helpers are pure NBT converters
**Date:** 2026-02-07
**Session:** .claude/sessions/2026-02-07/002_storage_player-data-and-hardening.md
**Decision:** `pumpkin_nbt::player_data` provides only NBT ↔ Rust type conversions (UUID encoding, position/rotation serialization, entity/ability structs). No file I/O, no GZip compression. Consumers (pumpkin-world/) handle persistence.
**Rationale:** Clean separation between serialization format (our scope) and persistence logic (WorldGen/world scope).
**Affects:** Storage, WorldGen
**Status:** active

## STOR-002: Struct-based API for entity/ability NBT helpers
**Date:** 2026-02-07
**Session:** .claude/sessions/2026-02-07/002_storage_player-data-and-hardening.md
**Decision:** `EntityBase` and `PlayerAbilities` are structs with `write_to()` and `read_from()` methods, not free functions with tuple returns. Avoids clippy complaints about complex return types and too many arguments.
**Rationale:** Clippy pedantic/nursery lints are enforced project-wide. Structs are more ergonomic and self-documenting than 6-element tuples.
**Affects:** Storage
**Status:** active

## STOR-003: WorldGen must finish a61238a LevelData serde defaults (handover)
**Date:** 2026-02-20
**Session:** .claude/sessions/2026-02-20/ (upstream sync session)
**Decision:** Storage ported the NBT-layer half of upstream commit `a61238a` (unknown list type IDs now tolerated in `tag.rs`). The `pumpkin-world/` half is WorldGen's responsibility. WorldGen must apply the following changes to `pumpkin-world/src/world_info/mod.rs`:

1. Add `#[serde(default)]` to all optional `LevelData` fields:
   - `allow_commands`, `border_center_x`, `border_center_z`, `clear_weather_time`, `day_time`
   - `difficulty_locked`, `game_rules`, `last_played`, `spawn_x`, `spawn_y`, `spawn_z`
   - `spawn_yaw`, `world_version`, `level_version`, `level_name`
   - Border fields: `border_damage_per_block`, `border_size`, `border_safe_zone`,
     `border_size_lerp_target`, `border_size_lerp_time`, `border_warning_blocks`, `border_warning_time`

2. Add default-value functions matching vanilla defaults (already present in `LevelData::default()`):
   ```rust
   fn default_border_damage_per_block() -> f64 { 0.2 }
   fn default_border_size() -> f64 { 60_000_000.0 }
   fn default_border_safe_zone() -> f64 { 5.0 }
   fn default_border_warning_blocks() -> f64 { 5.0 }
   fn default_border_warning_time() -> f64 { 15.0 }
   fn default_level_name() -> String { "world".to_string() }
   fn default_spawn_y() -> i32 { 200 }
   ```

3. Change `Generator.settings: String` to a `GeneratorSettings` enum that handles both
   string references and inline compound objects (older worlds use compound):
   ```rust
   #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
   #[serde(untagged)]
   pub enum GeneratorSettings {
       Named(String),
       Compound(serde_json::Value),  // or NbtCompound
   }
   ```

4. Make `Generator.biome_source: BiomeSource` → `Option<BiomeSource>` with `#[serde(default)]`.

**Rationale:** These are all in `pumpkin-world/` which is outside Storage's write_paths. Storage's NBT fix (STOR-003 part 1, now done) removes the hard parse failure on unknown list type IDs. Without the serde defaults (part 2), imported worlds still fail with missing-field errors at the struct level.
**Affects:** WorldGen (must implement), Storage (done)
**Status:** PARTIALLY DONE — Storage half complete, WorldGen half pending
