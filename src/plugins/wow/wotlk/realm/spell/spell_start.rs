use async_trait::async_trait;
use binrw::BinRead;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::client::prelude::*;
use crate::plugins::wow::wotlk::realm::object::types::packed_guid::PackedGuid;
use crate::plugins::wow::wotlk::realm::spell::types::{
    AmmoInfo, CastFlags, SpellImmunity, SpellTargets,
};

#[derive(Packet, BinRead, Serialize, FieldsMetadata)]
#[br(little)]
struct Incoming {
    cast_item_guid: PackedGuid,
    caster_guid: PackedGuid,
    // pending spell cast
    cast_count: u8,
    spell_id: u32,
    cast_flags: CastFlags,
    // delay?
    timestamp: u32,
    targets: SpellTargets,

    #[br(if(cast_flags.contains(CastFlags::AMMO)))]
    ammo: AmmoInfo,

    #[br(if(cast_flags.contains(CastFlags::IMMUNITY)))]
    immunity: SpellImmunity,
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


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn ammo_does_not_consume_following_immunity_fields() {
        let flags = CastFlags::AMMO | CastFlags::IMMUNITY;
        let mut bytes = Vec::new();
        bytes.push(0); // packed cast item guid
        bytes.push(0); // packed caster guid
        bytes.push(0); // cast count
        push_u32(&mut bytes, 1234); // spell id
        push_u32(&mut bytes, flags.bits());
        push_u32(&mut bytes, 2500); // cast timer
        push_u32(&mut bytes, 0); // TARGET_FLAG_SELF
        push_u32(&mut bytes, 5996); // ammo display id
        push_u32(&mut bytes, 24); // ammo inventory type
        push_u32(&mut bytes, 0x11); // school immunity
        push_u32(&mut bytes, 0x22); // mechanic immunity

        let mut cursor = Cursor::new(bytes.as_slice());
        let incoming = Incoming::read_le(&mut cursor).unwrap();

        assert_eq!(incoming.ammo.display_id, 5996);
        assert_eq!(incoming.ammo.inventory_type, 24);
        assert_eq!(incoming.immunity.school_immunity_mask, 0x11);
        assert_eq!(incoming.immunity.mechanic_immunity_mask, 0x22);
        assert_eq!(cursor.position() as usize, bytes.len());
    }
}
