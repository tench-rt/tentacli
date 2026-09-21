use crate::client::prelude::*;
use crate::plugins::wow::wotlk::login::Secret;
use crate::plugins::wow::wotlk::opcodes::Opcode;
use crate::plugins::wow::wotlk::realm::rc4::{Decryptor, Encryptor};

const OPCODE_SIZE: usize = 2;
const SHORT_SERVER_HEADER_SIZE: usize = 4;
const LONG_SERVER_HEADER_SIZE: usize = 5;
const LONG_PACKET_FLAG: u8 = 0x80;

fn server_header_len(first_byte: u8) -> usize {
    if first_byte & LONG_PACKET_FLAG != 0 {
        LONG_SERVER_HEADER_SIZE
    } else {
        SHORT_SERVER_HEADER_SIZE
    }
}

fn decode_server_header(header: &[u8]) -> anyhow::Result<(usize, u16)> {
    if header.len() < SHORT_SERVER_HEADER_SIZE {
        anyhow::bail!("Incomplete header... continue reading.");
    }

    let is_long = header[0] & LONG_PACKET_FLAG != 0;
    let header_len = server_header_len(header[0]);
    if header.len() < header_len {
        anyhow::bail!("Incomplete header (long)... continue reading.");
    }

    // WotLK's long server header uses bit 7 of the first size byte only as the
    // "three-byte size" marker. It is not part of the numeric packet size.
    let declared_size = if is_long {
        (usize::from(header[0] & !LONG_PACKET_FLAG) << 16)
            | (usize::from(header[1]) << 8)
            | usize::from(header[2])
    } else {
        (usize::from(header[0]) << 8) | usize::from(header[1])
    };
    if declared_size < OPCODE_SIZE {
        anyhow::bail!("Invalid realm packet size {declared_size}");
    }

    let opcode_offset = if is_long { 3 } else { 2 };
    let opcode = u16::from_le_bytes([header[opcode_offset], header[opcode_offset + 1]]);
    Ok((declared_size - OPCODE_SIZE, opcode))
}

#[derive(Default)]
pub struct PacketReader {
    decryptor: Option<Decryptor>,
    // Number of bytes at the beginning of packet_buf that have already been
    // decrypted for the current header. read_next_packet retries `read()` while
    // it accumulates the body, so a boolean is insufficient for a 5-byte long
    // header: the long/short decision must survive those retries.
    decrypted_header_bytes: usize,
}

impl BytesRead for PacketReader {
    fn read(&mut self, buffer: &mut [u8], context: &CtxMap) -> anyhow::Result<Packet> {
        if buffer.len() < SHORT_SERVER_HEADER_SIZE {
            anyhow::bail!("Incomplete header... continue reading.");
        }

        if let Some(decryptor) = self.decryptor.as_mut() {
            if self.decrypted_header_bytes == 0 {
                // Decrypt exactly the marker byte first. If this proves to be a
                // long header but byte 5 has not arrived yet, the byte remains
                // decrypted in packet_buf and the RC4 cursor is remembered by
                // decrypted_header_bytes instead of being consumed twice.
                decryptor.decrypt(&mut buffer[..1]);
                self.decrypted_header_bytes = 1;
            }

            let header_len = server_header_len(buffer[0]);
            if buffer.len() < header_len {
                anyhow::bail!("Incomplete header (long)... continue reading.");
            }

            if self.decrypted_header_bytes < header_len {
                decryptor.decrypt(&mut buffer[self.decrypted_header_bytes..header_len]);
                self.decrypted_header_bytes = header_len;
            }
        }

        let header_len = server_header_len(buffer[0]);
        if buffer.len() < header_len {
            anyhow::bail!("Incomplete header (long)... continue reading.");
        }
        let (body_size, opcode) = decode_server_header(&buffer[..header_len])?;
        let packet_size = header_len
            .checked_add(body_size)
            .ok_or_else(|| anyhow::anyhow!("Realm packet size overflow"))?;
        if buffer.len() < packet_size {
            anyhow::bail!("Incomplete body... continue reading.");
        }
        let body = buffer[header_len..packet_size].to_vec();

        if self.decryptor.is_none()
            && let Some(secret) = context.get::<Secret>()
        {
            self.decryptor = Some(Decryptor::new(&secret.0.to_vec()));
        }

        self.decrypted_header_bytes = 0;

        let mut packet = Packet::default();
        packet.set_type(PacketType::Incoming);
        packet.set_opcode(PacketOpcode::U16(opcode));
        packet.set_packet_name(Opcode::get_opcode_name(opcode as u32).unwrap_or_default());
        packet.set_packet_size(packet_size);
        packet.set_body(body);

        Ok(packet)
    }
}

#[derive(Default)]
pub struct PacketSerializer {
    encryptor: Option<Encryptor>,
}

impl Serializer for PacketSerializer {
    fn serialize(&mut self, packet: &Packet, context: &CtxMap) -> anyhow::Result<Vec<u8>> {
        let opcode = match packet.metadata.opcode {
            PacketOpcode::U32(opcode) => opcode,
            _ => anyhow::bail!("Wrong opcode type !"),
        };

        let size = (4 + packet.content.body.len()) as u16;

        let mut buffer = Vec::with_capacity(2 + size as usize);
        buffer.extend_from_slice(&size.to_be_bytes());
        buffer.extend_from_slice(&opcode.to_le_bytes());

        if let Some(encryptor) = self.encryptor.as_mut() {
            encryptor.encrypt(&mut buffer);
        }

        buffer.extend_from_slice(&packet.content.body);

        if self.encryptor.is_none()
            && let Some(secret) = context.get::<Secret>()
        {
            self.encryptor = Some(Encryptor::new(&secret.0.to_vec()));
        }

        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_short_server_header() {
        let header = [0x00, 0x08, 0xA9, 0x00];
        assert_eq!(decode_server_header(&header).unwrap(), (6, 0x00A9));
    }

    #[test]
    fn decodes_long_server_header_without_counting_marker_bit() {
        // Regression from a real arena -> Dalaran snapshot. The old reader
        // retried this as a 4-byte header and produced UNKNOWN_0xF6C2. Correct
        // framing is opcode 0x01F6 (SMSG_COMPRESSED_UPDATE_OBJECT), with the
        // 0x80 bit excluded from the 24-bit size.
        let header = [0x80, 0xA2, 0xC2, 0xF6, 0x01];
        assert_eq!(server_header_len(header[0]), 5);
        assert_eq!(decode_server_header(&header).unwrap(), (0x00A2C2 - 2, 0x01F6));
    }
}
