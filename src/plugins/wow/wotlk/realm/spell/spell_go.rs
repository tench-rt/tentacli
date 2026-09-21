use async_trait::async_trait;
use binrw::BinRead;
use fields_metadata::FieldsMetadata;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::client::prelude::*;
use crate::enum_field;
use crate::plugins::wow::wotlk::realm::object::types::packed_guid::PackedGuid;
use crate::plugins::wow::wotlk::realm::spell::types::{
    AdjustMissile, AmmoInfo, CastFlags, PredictedRunes, SpellTargets, VisualChain,
    VisualChainTargets,
};

#[derive(Packet, BinRead, Serialize, FieldsMetadata)]
#[br(little)]
struct Incoming {
    // who initiated the spell
    source_guid: PackedGuid,
    // who animated when cast
    caster_guid: PackedGuid,
    // pending spell cast
    cast_count: u8,
    spell_id: u32,
    cast_flags: CastFlags,
    // delay?
    timestamp: u32,
    spell_go_targets: SpellGOTargets,
    targets: SpellTargets,
    #[br(if(cast_flags.contains(CastFlags::PREDICTED_POWER)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    predicted_power: Option<u32>,
    #[br(if(cast_flags.contains(CastFlags::PREDICTED_RUNES)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    runes: Option<PredictedRunes>,
    #[br(if(cast_flags.contains(CastFlags::ADJUST_MISSILE)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    missile: Option<AdjustMissile>,
    #[br(if(cast_flags.contains(CastFlags::AMMO)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    ammo: Option<AmmoInfo>,
    #[br(if(cast_flags.contains(CastFlags::VISUAL_CHAIN)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    visual_chain: Option<VisualChain>,
    #[br(if(targets.has_dest_location))]
    #[serde(skip_serializing_if = "Option::is_none")]
    dest_loc_counter: Option<u8>,
    #[br(if(targets.has_visual_chain_targets))]
    #[serde(skip_serializing_if = "Option::is_none")]
    visual_chain_targets: Option<VisualChainTargets>,
}

pub struct Handler;
#[async_trait]
impl PacketHandler for Handler {
    async fn handle(
        &mut self,
        packet: &mut Packet,
        _: Arc<RwLock<CtxMap>>,
    ) -> anyhow::Result<Vec<HandlerOutput>> {
        let _ = Incoming::unpack(packet)?;
        Ok(vec![])
    }
}

enum_field! {
    pub enum SpellMissInfo: u8 {
        None     = 0,
        Miss     = 1,
        Resist  = 2,
        Dodge   = 3,
        Parry   = 4,
        Block   = 5,
        Evade   = 6,
        Immune  = 7,
        Immune2 = 8,
        Deflect = 9,
        Absorb  = 10,
        Reflect = 11,
    }
}

#[derive(BinRead, Debug, Clone, FieldsMetadata, Serialize)]
pub struct SpellGOTargets {
    pub hit_count: u8,

    #[br(count = hit_count)]
    pub hit_guids: Vec<u64>,

    pub miss_count: u8,

    #[br(count = miss_count)]
    pub misses: Vec<MissEntry>,
}

#[derive(BinRead, Debug, Clone, FieldsMetadata, Serialize)]
pub struct MissEntry {
    pub target_guid: u64,
    pub miss_condition: SpellMissInfo,

    #[br(if(matches!(miss_condition, SpellMissInfo::Reflect)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflect_result: Option<u8>,
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_f32(bytes: &mut Vec<u8>, value: f32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn base_spell_go(cast_flags: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0); // packed source guid = 0
        bytes.push(0); // packed caster guid = 0
        bytes.push(0); // cast count
        push_u32(&mut bytes, 1234); // spell id
        push_u32(&mut bytes, cast_flags);
        push_u32(&mut bytes, 5678); // timestamp
        bytes.push(0); // hit count
        bytes.push(0); // miss count
        bytes
    }

    fn push_destination(bytes: &mut Vec<u8>) {
        push_u32(bytes, 0x0000_0040); // TARGET_FLAG_DEST_LOCATION
        bytes.push(0); // packed destination transport guid = 0
        push_f32(bytes, 1.0);
        push_f32(bytes, 2.0);
        push_f32(bytes, 3.0);
    }

    #[test]
    fn cmangos_destination_spell_go_keeps_dest_counter_aligned() {
        // Real CMaNGOS layout: target mask, packed transport guid, xyz, counter.
        let mut bytes = base_spell_go(0);
        push_destination(&mut bytes);
        bytes.push(7); // dest loc counter

        let mut cursor = Cursor::new(bytes.as_slice());
        let incoming = Incoming::read_le(&mut cursor).unwrap();

        assert!(incoming.targets.has_dest_location);
        assert_eq!(incoming.targets.dest_transport, Some(PackedGuid(0)));
        assert_eq!(incoming.dest_loc_counter, Some(7));
        assert_eq!(cursor.position() as usize, bytes.len());
    }

    #[test]
    fn ammo_is_fixed_width_and_does_not_consume_dest_counter() {
        let mut bytes = base_spell_go(CastFlags::AMMO.bits());
        push_destination(&mut bytes);
        push_u32(&mut bytes, 5996); // ammo display id
        push_u32(&mut bytes, 24); // ammo inventory type
        bytes.push(9); // dest loc counter

        let mut cursor = Cursor::new(bytes.as_slice());
        let incoming = Incoming::read_le(&mut cursor).unwrap();

        let ammo = incoming.ammo.unwrap();
        assert_eq!(ammo.display_id, 5996);
        assert_eq!(ammo.inventory_type, 24);
        assert_eq!(incoming.dest_loc_counter, Some(9));
        assert_eq!(cursor.position() as usize, bytes.len());
    }

    #[test]
    fn predicted_runes_reads_only_spent_rune_cooldowns() {
        let mut bytes = base_spell_go(CastFlags::PREDICTED_RUNES.bits());
        push_destination(&mut bytes);
        bytes.push(0b0000_0011); // rune 0 + 1 ready before
        bytes.push(0b0000_0010); // rune 0 spent
        bytes.push(0x80); // exactly one cooldown byte
        bytes.push(11); // dest loc counter must remain unread until now

        let mut cursor = Cursor::new(bytes.as_slice());
        let incoming = Incoming::read_le(&mut cursor).unwrap();

        assert_eq!(incoming.runes.unwrap().cooldowns, vec![0x80]);
        assert_eq!(incoming.dest_loc_counter, Some(11));
        assert_eq!(cursor.position() as usize, bytes.len());
    }
}
