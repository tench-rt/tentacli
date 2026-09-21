use binrw::BinRead;
use bitflags::bitflags;
use serde::Serialize;

use crate::client::prelude::*;
use crate::plugins::wow::wotlk::realm::object::types::movement::Point3D;
use crate::plugins::wow::wotlk::realm::object::types::packed_guid::PackedGuid;

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Default, BitflagExtras)]
    #[bitflags_repr(u32)]
    pub struct CastFlags: u32 {
        const NONE = 0x00000000;

        // hide in combat log?
        const HIDDEN_COMBATLOG = 0x00000001;
        const UNKNOWN2        = 0x00000002;
        const UNKNOWN3        = 0x00000004;
        const UNKNOWN4        = 0x00000008;

        // Spell has Persistent AA effect
        const PERSISTENT_AA  = 0x00000010;

        // Projectiles visual
        const AMMO          = 0x00000020;

        // !0x41 mask used to call CGTradeSkillInfo::DoRecast
        const UNKNOWN7     = 0x00000040;
        const UNKNOWN8     = 0x00000080;
        const UNKNOWN9     = 0x00000100;
        const UNKNOWN10    = 0x00000200;
        const UNKNOWN11    = 0x00000400;

        // wotlk, trigger rune cooldown
        const PREDICTED_POWER = 0x00000800;
        const UNKNOWN13       = 0x00001000;
        const UNKNOWN14       = 0x00002000;
        const UNKNOWN15       = 0x00004000;
        const UNKNOWN16       = 0x00008000;
        const UNKNOWN17       = 0x00010000;

        // wotlk
        const ADJUST_MISSILE = 0x00020000;

        // spell cooldown related (may be category cooldown)
        const NO_GCD        = 0x00040000;

        // wotlk
        const VISUAL_CHAIN = 0x00080000;
        const UNKNOWN21   = 0x00100000;

        // wotlk, rune cooldown list
        const PREDICTED_RUNES = 0x00200000;

        // spell cast school imminity info
        const IMMUNITY = 0x04000000;
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Default, BitflagExtras)]
    #[bitflags_repr(u8)]
    pub struct SpellHitType: u8 {
        const CRIT_DEBUG         = 0x01;
        const CRIT               = 0x02;
        const HIT_DEBUG         = 0x04;
        const SPLIT             = 0x08;
        const VICTIM_IS_ATTACKER= 0x10;
        const ATTACK_TABLE_DEBUG= 0x20;
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Default, BitflagExtras)]
    #[bitflags_repr(u32)]
    pub struct TargetFlags: u32 {
        const SELF             = 0x00000000;
        const UNUSED1         = 0x00000001;
        const UNIT            = 0x00000002;
        const UNIT_RAID       = 0x00000004;
        const UNIT_PARTY      = 0x00000008;
        const ITEM            = 0x00000010;
        const SOURCE_LOCATION= 0x00000020;
        const DEST_LOCATION  = 0x00000040;
        const UNIT_ENEMY     = 0x00000080;
        const UNIT_ALLY      = 0x00000100;
        const CORPSE_ENEMY   = 0x00000200;
        const UNIT_DEAD      = 0x00000400;
        const GAMEOBJECT    = 0x00000800;
        const TRADE_ITEM    = 0x00001000;
        const STRING        = 0x00002000;
        const LOCKED        = 0x00004000;
        const CORPSE_ALLY  = 0x00008000;
        const UNIT_MINIPET = 0x00010000;
        const GLYPH         = 0x00020000;
        const UNK3          = 0x00040000;
        const VISUAL_CHAIN = 0x00080000;
    }
}

#[derive(BinRead, Debug, Clone, FieldsMetadata, Serialize)]
pub struct SpellTargets {
    pub target_mask: TargetFlags,

    // CMaNGOS SpellCastTargets::write() emits at most one object target in this
    // section, with UNIT taking precedence over GAMEOBJECT, then CORPSE, then
    // the UNIT_MINIPET fallback. Semantic flags such as UNIT_ENEMY/ALLY do not
    // themselves add another packed GUID to the wire stream.
    #[br(if(target_mask.contains(TargetFlags::UNIT)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_target: Option<PackedGuid>,

    #[br(if(
        !target_mask.contains(TargetFlags::UNIT)
            && target_mask.contains(TargetFlags::GAMEOBJECT)
    ))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gameobject_target: Option<PackedGuid>,

    #[br(if(
        !target_mask.contains(TargetFlags::UNIT)
            && !target_mask.contains(TargetFlags::GAMEOBJECT)
            && (target_mask.contains(TargetFlags::CORPSE_ENEMY)
                || target_mask.contains(TargetFlags::CORPSE_ALLY))
    ))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corpse_target: Option<PackedGuid>,

    #[br(if(
        !target_mask.contains(TargetFlags::UNIT)
            && !target_mask.contains(TargetFlags::GAMEOBJECT)
            && !target_mask.contains(TargetFlags::CORPSE_ENEMY)
            && !target_mask.contains(TargetFlags::CORPSE_ALLY)
            && target_mask.contains(TargetFlags::UNIT_MINIPET)
    ))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minipet_target: Option<PackedGuid>,

    #[br(if(
        target_mask.contains(TargetFlags::ITEM)
            || target_mask.contains(TargetFlags::TRADE_ITEM)
    ))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_target: Option<PackedGuid>,

    // WotLK location targets are not bare vec3 values. Each location is
    // preceded by a packed transport GUID (usually a single zero byte).
    #[br(if(target_mask.contains(TargetFlags::SOURCE_LOCATION)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_transport: Option<PackedGuid>,

    #[br(if(target_mask.contains(TargetFlags::SOURCE_LOCATION)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_pos: Option<Point3D>,

    #[br(if(target_mask.contains(TargetFlags::DEST_LOCATION)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_transport: Option<PackedGuid>,

    #[br(if(target_mask.contains(TargetFlags::DEST_LOCATION)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_pos: Option<Point3D>,

    #[br(if(target_mask.contains(TargetFlags::STRING)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub string_target: Option<NullTerminated<String>>,

    #[br(calc = target_mask.contains(TargetFlags::DEST_LOCATION))]
    pub has_dest_location: bool,

    #[br(calc = target_mask.contains(TargetFlags::VISUAL_CHAIN))]
    pub has_visual_chain_targets: bool,
}

#[derive(BinRead, Debug, Clone, FieldsMetadata, Serialize, Default)]
pub struct PredictedRunes {
    pub rune_mask_before: u8,
    pub rune_mask_after: u8,

    // CMaNGOS writes one cooldown byte only for each rune that was ready before
    // the cast and is on cooldown afterwards. WotLK has six runes total.
    #[br(count = ((rune_mask_before & !rune_mask_after) & 0x3F).count_ones() as usize)]
    pub cooldowns: Vec<u8>,
}

#[derive(BinRead, Debug, Clone, FieldsMetadata, Serialize, Default)]
pub struct AdjustMissile {
    pub elevation: f32,
    pub delay: u32,
}

#[derive(BinRead, Debug, Clone, FieldsMetadata, Serialize, Default)]
pub struct VisualChain {
    pub spell_visual_id: u32,
    pub override_id: u32,
}

#[derive(BinRead, Debug, Clone, FieldsMetadata, Serialize, Default)]
pub struct AmmoInfo {
    // Spell::WriteAmmoToPacket() is exactly two uint32 values in WotLK.
    pub display_id: u32,
    pub inventory_type: u32,
}

#[derive(BinRead, Debug, Clone, FieldsMetadata, Serialize, Default)]
pub struct VisualChainTargets {
    pub count: u32,

    #[br(count = count)]
    pub entries: Vec<VisualChainTarget>,
}

#[derive(BinRead, Debug, Clone, FieldsMetadata, Serialize)]
pub struct VisualChainTarget {
    pub position: Point3D,
    pub guid: u64,
}

#[derive(BinRead, Debug, Clone, FieldsMetadata, Serialize, Default)]
pub struct SpellImmunity {
    pub school_immunity_mask: u32,
    pub mechanic_immunity_mask: u32,
}
