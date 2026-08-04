//! Binary Message Framing & Integrity Checksumming (Phase 15 Milestone 15.1).
//!
//! ### Architectural Invariants:
//! 1. Logical Framing Decoupling: `MessageFramingCodec` operates on abstract byte streams independently of physical transport backends.
//! 2. Optional Integrity Checksumming: `IntegrityPolicy` permits raw byte transports to validate payload CRC32 checksums while avoiding double-checksumming over TLS/HTTP2.
//! 3. Oversized Frame Protection: Rejects frames exceeding `MAX_FRAME_SIZE` (16MB) to prevent memory exhaustion attacks.

use serde::{Deserialize, Serialize};

/// Constant magic byte header identifying valid brain cluster transport frames.
pub const TRANSPORT_FRAME_MAGIC: u8 = 0x42;
/// Constant schema version for binary framing.
pub const TRANSPORT_FRAME_SCHEMA_VERSION: u16 = 1;
/// Maximum allowed frame size limit (16MB).
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

/// Configurable integrity verification policy for framing codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum IntegrityPolicy {
    /// Skip checksum calculation (for TLS / HTTP2 transports with built-in integrity).
    None,
    /// Enforce CRC32 checksum validation (for raw UDP / QUIC byte streams).
    #[default]
    Crc32,
}

/// Binary header prefix for logical transport frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportFrameHeader {
    /// Magic byte identifier (`0x42`).
    pub magic: u8,
    /// Protocol schema version header.
    pub version: u16,
    /// Message type classification identifier.
    pub msg_type: u16,
    /// Length of payload in bytes.
    pub payload_len: u32,
    /// CRC32 payload checksum (0 if `IntegrityPolicy::None`).
    pub checksum: u32,
}

/// Errors occurring during logical frame encoding or decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FramingError {
    /// Frame magic byte does not match expected `0x42`.
    InvalidMagic(u8),
    /// Schema version header mismatch.
    VersionMismatch(u16),
    /// Frame payload length exceeds `MAX_FRAME_SIZE`.
    OversizedFrame(usize),
    /// Frame payload truncated or incomplete.
    TruncatedFrame,
    /// CRC32 integrity checksum validation failed.
    ChecksumMismatch {
        /// Expected CRC32 checksum from header.
        expected: u32,
        /// Actual computed CRC32 checksum over payload.
        actual: u32,
    },
}

impl std::fmt::Display for FramingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMagic(actual) => write!(f, "Invalid magic byte: {:#x}", actual),
            Self::VersionMismatch(v) => write!(f, "Unsupported schema version: {}", v),
            Self::OversizedFrame(size) => write!(f, "Oversized frame: {} bytes", size),
            Self::TruncatedFrame => write!(f, "Truncated or incomplete frame"),
            Self::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "CRC32 checksum mismatch: expected {}, got {}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for FramingError {}

/// Pure binary framing codec for logical stream framing.
pub struct MessageFramingCodec;

impl MessageFramingCodec {
    /// Computes CRC32 checksum over payload bytes.
    pub fn compute_checksum(payload: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &byte in payload {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xED88_8320 & mask);
            }
        }
        !crc
    }

    /// Encodes a payload into a length-prefixed binary frame buffer.
    pub fn encode_frame(
        msg_type: u16,
        payload: &[u8],
        policy: IntegrityPolicy,
    ) -> Result<Vec<u8>, FramingError> {
        if payload.len() > MAX_FRAME_SIZE {
            return Err(FramingError::OversizedFrame(payload.len()));
        }

        let checksum = match policy {
            IntegrityPolicy::None => 0,
            IntegrityPolicy::Crc32 => Self::compute_checksum(payload),
        };

        let mut buf = Vec::with_capacity(15 + payload.len());
        buf.push(TRANSPORT_FRAME_MAGIC);
        buf.extend_from_slice(&TRANSPORT_FRAME_SCHEMA_VERSION.to_be_bytes());
        buf.extend_from_slice(&msg_type.to_be_bytes());
        buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        buf.extend_from_slice(&checksum.to_be_bytes());
        buf.extend_from_slice(payload);

        Ok(buf)
    }

    /// Decodes a binary frame buffer into a header and payload tuple.
    pub fn decode_frame(
        buf: &[u8],
        policy: IntegrityPolicy,
    ) -> Result<(TransportFrameHeader, Vec<u8>), FramingError> {
        if buf.len() < 15 {
            return Err(FramingError::TruncatedFrame);
        }

        let magic = buf[0];
        if magic != TRANSPORT_FRAME_MAGIC {
            return Err(FramingError::InvalidMagic(magic));
        }

        let version = u16::from_be_bytes([buf[1], buf[2]]);
        if version != TRANSPORT_FRAME_SCHEMA_VERSION {
            return Err(FramingError::VersionMismatch(version));
        }

        let msg_type = u16::from_be_bytes([buf[3], buf[4]]);
        let payload_len = u32::from_be_bytes([buf[5], buf[6], buf[7], buf[8]]) as usize;
        let checksum = u32::from_be_bytes([buf[9], buf[10], buf[11], buf[12]]);

        if payload_len > MAX_FRAME_SIZE {
            return Err(FramingError::OversizedFrame(payload_len));
        }

        if buf.len() < 13 + payload_len {
            return Err(FramingError::TruncatedFrame);
        }

        let payload = buf[13..13 + payload_len].to_vec();

        if policy == IntegrityPolicy::Crc32 {
            let actual = Self::compute_checksum(&payload);
            if checksum != actual {
                return Err(FramingError::ChecksumMismatch {
                    expected: checksum,
                    actual,
                });
            }
        }

        let header = TransportFrameHeader {
            magic,
            version,
            msg_type,
            payload_len: payload_len as u32,
            checksum,
        };

        Ok((header, payload))
    }
}
