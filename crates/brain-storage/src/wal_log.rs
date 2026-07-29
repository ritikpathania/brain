//! Durable WAL append-only event log backend (`WalLogEventStore`) serving as write-ahead source of truth.

use brain_events::{EventStore, ReflectionEventEnvelope};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Mutex;

/// Calculate CRC32 checksum for a byte slice.
fn compute_crc32(data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

/// Durable WAL record framing header and payload.
#[derive(Debug, Clone, PartialEq)]
pub struct WalRecord {
    /// Monotonic sequence number.
    pub sequence_number: u64,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Schema format version.
    pub schema_version: u32,
    /// Encoded event envelope payload.
    pub payload: Vec<u8>,
    /// CRC32 integrity checksum.
    pub checksum: u32,
}

impl WalRecord {
    /// Creates a new `WalRecord` automatically calculating its CRC32 checksum.
    pub fn new(
        sequence_number: u64,
        timestamp_ms: u64,
        schema_version: u32,
        payload: Vec<u8>,
    ) -> Self {
        let mut check_data = Vec::new();
        check_data.extend_from_slice(&sequence_number.to_le_bytes());
        check_data.extend_from_slice(&timestamp_ms.to_le_bytes());
        check_data.extend_from_slice(&schema_version.to_le_bytes());
        check_data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        check_data.extend_from_slice(&payload);

        let checksum = compute_crc32(&check_data);
        Self {
            sequence_number,
            timestamp_ms,
            schema_version,
            payload,
            checksum,
        }
    }

    /// Encodes record into a binary framing byte vector.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.sequence_number.to_le_bytes());
        buf.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&self.schema_version.to_le_bytes());
        let payload_len = self.payload.len() as u32;
        buf.extend_from_slice(&payload_len.to_le_bytes());
        buf.extend_from_slice(&self.payload);
        buf.extend_from_slice(&self.checksum.to_le_bytes());
        buf
    }

    /// Attempts to decode a record from a byte reader stream.
    pub fn decode<R: Read>(reader: &mut R) -> std::io::Result<Option<Self>> {
        let mut seq_buf = [0u8; 8];
        if reader.read_exact(&mut seq_buf).is_err() {
            return Ok(None);
        }
        let sequence_number = u64::from_le_bytes(seq_buf);

        let mut ts_buf = [0u8; 8];
        reader.read_exact(&mut ts_buf)?;
        let timestamp_ms = u64::from_le_bytes(ts_buf);

        let mut ver_buf = [0u8; 4];
        reader.read_exact(&mut ver_buf)?;
        let schema_version = u32::from_le_bytes(ver_buf);

        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf)?;
        let payload_len = u32::from_le_bytes(len_buf) as usize;

        let mut payload = vec![0u8; payload_len];
        reader.read_exact(&mut payload)?;

        let mut crc_buf = [0u8; 4];
        reader.read_exact(&mut crc_buf)?;
        let checksum = u32::from_le_bytes(crc_buf);

        // Verify CRC32 checksum over header fields and payload
        let mut check_data = Vec::new();
        check_data.extend_from_slice(&sequence_number.to_le_bytes());
        check_data.extend_from_slice(&timestamp_ms.to_le_bytes());
        check_data.extend_from_slice(&schema_version.to_le_bytes());
        check_data.extend_from_slice(&(payload_len as u32).to_le_bytes());
        check_data.extend_from_slice(&payload);

        let expected_checksum = compute_crc32(&check_data);
        if checksum != expected_checksum {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Checksum mismatch: calculated {}, expected {}",
                    expected_checksum, checksum
                ),
            ));
        }

        Ok(Some(Self {
            sequence_number,
            timestamp_ms,
            schema_version,
            payload,
            checksum,
        }))
    }
}

/// Durable WAL append-only event store implementing `EventStore`.
pub struct WalLogEventStore {
    log_path: PathBuf,
    inner: Mutex<WalLogInner>,
}

struct WalLogInner {
    file: File,
    next_sequence: u64,
}

impl WalLogEventStore {
    /// Opens or creates a `WalLogEventStore` at the specified file path.
    pub fn open(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        let (next_sequence, valid_len) = Self::scan_and_validate(&mut file)?;
        file.set_len(valid_len)?;
        file.seek(SeekFrom::End(0))?;

        Ok(Self {
            log_path: path,
            inner: Mutex::new(WalLogInner {
                file,
                next_sequence,
            }),
        })
    }

    /// Scans log file, validates checksums, and returns (next_sequence, valid_length_bytes).
    fn scan_and_validate(file: &mut File) -> std::io::Result<(u64, u64)> {
        file.seek(SeekFrom::Start(0))?;
        let mut valid_len = 0u64;
        let mut max_seq = 0u64;

        loop {
            match WalRecord::decode(file) {
                Ok(Some(record)) => {
                    max_seq = record.sequence_number;
                    valid_len = file.stream_position()?;
                }
                Ok(None) => break,
                Err(_) => {
                    // Truncate at first corrupted or partial tail record
                    break;
                }
            }
        }

        Ok((max_seq + 1, valid_len))
    }

    /// Manually triggers tail recovery and returns the count of valid records recovered.
    pub fn recover(&self) -> std::io::Result<usize> {
        let mut inner = self.inner.lock().expect("WAL log lock poisoned");
        let (next_seq, valid_len) = Self::scan_and_validate(&mut inner.file)?;
        inner.file.set_len(valid_len)?;
        inner.file.seek(SeekFrom::End(0))?;
        inner.next_sequence = next_seq;
        Ok((next_seq - 1) as usize)
    }
}

impl EventStore for WalLogEventStore {
    fn append(&self, envelope: ReflectionEventEnvelope) -> Result<(), String> {
        let mut inner = self.inner.lock().expect("WAL log lock poisoned");
        let payload = serde_json::to_vec(&envelope).map_err(|e| e.to_string())?;

        let record = WalRecord::new(
            inner.next_sequence,
            envelope.timestamp_ms,
            envelope.schema_version,
            payload,
        );

        let encoded = record.encode();
        inner.file.write_all(&encoded).map_err(|e| e.to_string())?;
        inner.file.flush().map_err(|e| e.to_string())?;
        inner.next_sequence += 1;

        Ok(())
    }

    fn query(&self, plan_id: &str) -> Vec<ReflectionEventEnvelope> {
        let stream = self.stream();
        stream
            .into_iter()
            .filter(|e| e.plan_id == plan_id)
            .collect()
    }

    fn stream(&self) -> Vec<ReflectionEventEnvelope> {
        let _inner = self.inner.lock().expect("WAL log lock poisoned");
        let mut file = match File::open(&self.log_path) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };

        let mut envelopes = Vec::new();
        while let Ok(Some(record)) = WalRecord::decode(&mut file) {
            if let Ok(env) = serde_json::from_slice::<ReflectionEventEnvelope>(&record.payload) {
                envelopes.push(env);
            }
        }
        envelopes
    }

    fn compact(&self, before_timestamp_ms: u64) -> usize {
        let stream = self.stream();
        let initial_len = stream.len();
        let remaining: Vec<_> = stream
            .into_iter()
            .filter(|e| e.timestamp_ms >= before_timestamp_ms)
            .collect();
        let removed = initial_len - remaining.len();

        let mut inner = self.inner.lock().expect("WAL log lock poisoned");
        let _ = inner.file.set_len(0);
        let _ = inner.file.seek(SeekFrom::Start(0));

        inner.next_sequence = 1;
        for env in remaining {
            let payload = serde_json::to_vec(&env).unwrap_or_default();
            let seq = inner.next_sequence;
            let ts = env.timestamp_ms;
            let ver = env.schema_version;

            let mut check_data = Vec::new();
            check_data.extend_from_slice(&seq.to_le_bytes());
            check_data.extend_from_slice(&ts.to_le_bytes());
            check_data.extend_from_slice(&ver.to_le_bytes());
            check_data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            check_data.extend_from_slice(&payload);

            let checksum = compute_crc32(&check_data);
            let record = WalRecord {
                sequence_number: seq,
                timestamp_ms: ts,
                schema_version: ver,
                payload,
                checksum,
            };
            let _ = inner.file.write_all(&record.encode());
            inner.next_sequence += 1;
        }
        let _ = inner.file.flush();

        removed
    }
}
