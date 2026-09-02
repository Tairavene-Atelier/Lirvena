//! Bounded per-account `OneBot` message state for Lirvena.

use std::ffi::OsString;
use std::path::Path;

use account_runtime::AccountLocalId;
use local_state::open_private_wal;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

const SCHEMA_VERSION: i64 = 2;
const MAX_MESSAGES: usize = 4_096;
const MAX_JSON_BYTES: usize = 64 * 1024;
const MAX_UID_BYTES: usize = 128;

/// Redacted message-state configuration or persistence failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageStoreError {
    /// A record, path, or schema generation was invalid.
    Configuration,
    /// Local state could not be durably read or written.
    Persistence,
}

impl core::fmt::Display for MessageStoreError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "account message state configuration rejected",
            Self::Persistence => "account message state persistence failed",
        })
    }
}

impl std::error::Error for MessageStoreError {}

impl From<rusqlite::Error> for MessageStoreError {
    fn from(_error: rusqlite::Error) -> Self {
        Self::Persistence
    }
}

impl From<local_state::LocalStateError> for MessageStoreError {
    fn from(error: local_state::LocalStateError) -> Self {
        match error {
            local_state::LocalStateError::Configuration => Self::Configuration,
            local_state::LocalStateError::Persistence => Self::Persistence,
        }
    }
}

/// QQ correlation required to recall a retained message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecallTarget {
    /// Group message correlation.
    Group {
        /// Numeric group identifier.
        group_code: u32,
        /// QQ message sequence.
        sequence: u64,
        /// QQ message random when observed in this storage generation.
        ///
        /// Records migrated from schema v1 retain `None` and remain valid for
        /// recall, but cannot be used by operations that require the random.
        random: Option<u32>,
    },
    /// Direct-message correlation.
    Private {
        /// Current peer UID.
        uid: String,
        /// QQ server sequence.
        sequence: u64,
        /// Original client sequence.
        client_sequence: u64,
        /// Original message random.
        random: u32,
        /// Original or accepted timestamp.
        timestamp: u32,
    },
    /// The message can be queried but lacks evidence for recall.
    Unavailable,
}

/// One canonical `OneBot` message response and its QQ recall correlation.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageRecord {
    message_id: u32,
    inserted_at_ms: u64,
    response: Value,
    recall: RecallTarget,
}

impl MessageRecord {
    /// Creates one validated retained message.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identifiers, timestamps, response size, or recall fields.
    pub fn new(
        message_id: u32,
        inserted_at_ms: u64,
        response: Value,
        recall: RecallTarget,
    ) -> Result<Self, MessageStoreError> {
        let encoded =
            serde_json::to_vec(&response).map_err(|_error| MessageStoreError::Configuration)?;
        if message_id == 0
            || message_id > i32::MAX.unsigned_abs()
            || inserted_at_ms == 0
            || !response.is_object()
            || encoded.len() > MAX_JSON_BYTES
            || !valid_recall(&recall)
        {
            return Err(MessageStoreError::Configuration);
        }
        Ok(Self {
            message_id,
            inserted_at_ms,
            response,
            recall,
        })
    }

    /// Returns the account-local message identifier.
    #[must_use]
    pub const fn message_id(&self) -> u32 {
        self.message_id
    }

    /// Returns the canonical action response data.
    #[must_use]
    pub const fn response(&self) -> &Value {
        &self.response
    }

    /// Returns the validated recall correlation.
    #[must_use]
    pub const fn recall(&self) -> &RecallTarget {
        &self.recall
    }
}

/// Private, bounded message state owned by one account actor.
pub struct MessageStore {
    connection: Connection,
}

impl MessageStore {
    /// Opens or migrates one account-local message database.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe permissions, an unknown schema, or `SQLite` failure.
    pub fn open(directory: &Path, local_id: AccountLocalId) -> Result<Self, MessageStoreError> {
        let file_name = database_name(local_id);
        let (_path, mut connection) = open_private_wal(directory, &file_name)?;
        migrate(&mut connection)?;
        let store = Self { connection };
        store.prune()?;
        Ok(store)
    }

    /// Returns whether an identifier is already retained.
    ///
    /// # Errors
    ///
    /// Returns an error when local state cannot be queried.
    pub fn contains(&self, message_id: u32) -> Result<bool, MessageStoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT 1 FROM messages WHERE message_id = ?1",
                [i64::from(message_id)],
                |_row| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// Inserts or replaces one record and prunes the oldest entries transactionally.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, serialization, or persistence fails.
    pub fn put(&mut self, record: &MessageRecord) -> Result<(), MessageStoreError> {
        let encoded = serde_json::to_string(record.response())
            .map_err(|_error| MessageStoreError::Configuration)?;
        let fields = RecallFields::from_target(record.recall());
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT OR REPLACE INTO messages (
                 message_id, inserted_at_ms, response_json, recall_kind, group_code, uid,
                 sequence, client_sequence, random, timestamp
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                i64::from(record.message_id),
                to_i64(record.inserted_at_ms)?,
                encoded,
                fields.kind,
                fields.group_code.map(i64::from),
                fields.uid,
                fields.sequence.map(u64::to_be_bytes).map(Vec::from),
                fields.client_sequence.map(u64::to_be_bytes).map(Vec::from),
                fields.random.map(i64::from),
                fields.timestamp.map(i64::from),
            ],
        )?;
        transaction.execute(
            "DELETE FROM messages WHERE message_id IN (
                 SELECT message_id FROM messages
                 ORDER BY inserted_at_ms DESC, message_id DESC LIMIT -1 OFFSET ?1
             )",
            [i64::try_from(MAX_MESSAGES).map_err(|_error| MessageStoreError::Configuration)?],
        )?;
        transaction.commit().map_err(Into::into)
    }

    /// Loads one retained message.
    ///
    /// # Errors
    ///
    /// Returns an error when stored data is malformed or cannot be read.
    pub fn get(&self, message_id: u32) -> Result<Option<MessageRecord>, MessageStoreError> {
        let stored = self
            .connection
            .query_row(
                "SELECT inserted_at_ms, response_json, recall_kind, group_code, uid, sequence,
                        client_sequence, random, timestamp
                 FROM messages WHERE message_id = ?1",
                [i64::from(message_id)],
                |row| {
                    Ok(StoredRow {
                        inserted_at_ms: row.get(0)?,
                        response_json: row.get(1)?,
                        kind: row.get(2)?,
                        group_code: row.get(3)?,
                        uid: row.get(4)?,
                        sequence: row.get(5)?,
                        client_sequence: row.get(6)?,
                        random: row.get(7)?,
                        timestamp: row.get(8)?,
                    })
                },
            )
            .optional()?;
        stored.map(|row| row.into_record(message_id)).transpose()
    }

    /// Removes one retained message after QQ accepts its recall.
    ///
    /// # Errors
    ///
    /// Returns an error when the deletion cannot be committed.
    pub fn remove(&mut self, message_id: u32) -> Result<(), MessageStoreError> {
        self.connection
            .execute(
                "DELETE FROM messages WHERE message_id = ?1",
                [i64::from(message_id)],
            )
            .map(|_count| ())
            .map_err(Into::into)
    }

    fn prune(&self) -> Result<(), MessageStoreError> {
        self.connection.execute(
            "DELETE FROM messages WHERE message_id IN (
                 SELECT message_id FROM messages
                 ORDER BY inserted_at_ms DESC, message_id DESC LIMIT -1 OFFSET ?1
             )",
            [i64::try_from(MAX_MESSAGES).map_err(|_error| MessageStoreError::Configuration)?],
        )?;
        Ok(())
    }
}

impl core::fmt::Debug for MessageStore {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MessageStore")
            .finish_non_exhaustive()
    }
}

struct RecallFields<'a> {
    kind: i64,
    group_code: Option<u32>,
    uid: Option<&'a str>,
    sequence: Option<u64>,
    client_sequence: Option<u64>,
    random: Option<u32>,
    timestamp: Option<u32>,
}

impl<'a> RecallFields<'a> {
    fn from_target(target: &'a RecallTarget) -> Self {
        match target {
            RecallTarget::Group {
                group_code,
                sequence,
                random,
            } => Self {
                kind: 1,
                group_code: Some(*group_code),
                uid: None,
                sequence: Some(*sequence),
                client_sequence: None,
                random: *random,
                timestamp: None,
            },
            RecallTarget::Private {
                uid,
                sequence,
                client_sequence,
                random,
                timestamp,
            } => Self {
                kind: 2,
                group_code: None,
                uid: Some(uid),
                sequence: Some(*sequence),
                client_sequence: Some(*client_sequence),
                random: Some(*random),
                timestamp: Some(*timestamp),
            },
            RecallTarget::Unavailable => Self {
                kind: 0,
                group_code: None,
                uid: None,
                sequence: None,
                client_sequence: None,
                random: None,
                timestamp: None,
            },
        }
    }
}

struct StoredRow {
    inserted_at_ms: i64,
    response_json: String,
    kind: i64,
    group_code: Option<i64>,
    uid: Option<String>,
    sequence: Option<Vec<u8>>,
    client_sequence: Option<Vec<u8>>,
    random: Option<i64>,
    timestamp: Option<i64>,
}

impl StoredRow {
    fn into_record(self, message_id: u32) -> Result<MessageRecord, MessageStoreError> {
        let recall = match self.kind {
            0 if self.no_fields() => RecallTarget::Unavailable,
            1 if self.uid.is_none()
                && self.client_sequence.is_none()
                && self.timestamp.is_none() =>
            {
                RecallTarget::Group {
                    group_code: from_i64(self.group_code)?,
                    sequence: decode_u64(self.sequence)?,
                    random: optional_u32(self.random)?,
                }
            }
            2 if self.group_code.is_none() => RecallTarget::Private {
                uid: self.uid.ok_or(MessageStoreError::Configuration)?,
                sequence: decode_u64(self.sequence)?,
                client_sequence: decode_u64(self.client_sequence)?,
                random: from_i64(self.random)?,
                timestamp: from_i64(self.timestamp)?,
            },
            _ => return Err(MessageStoreError::Configuration),
        };
        let response = serde_json::from_str(&self.response_json)
            .map_err(|_error| MessageStoreError::Configuration)?;
        MessageRecord::new(
            message_id,
            u64::try_from(self.inserted_at_ms)
                .map_err(|_error| MessageStoreError::Configuration)?,
            response,
            recall,
        )
    }

    fn no_fields(&self) -> bool {
        self.group_code.is_none()
            && self.uid.is_none()
            && self.sequence.is_none()
            && self.client_sequence.is_none()
            && self.random.is_none()
            && self.timestamp.is_none()
    }
}

fn migrate(connection: &mut Connection) -> Result<(), MessageStoreError> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    match version {
        0 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "CREATE TABLE messages (
                     message_id INTEGER PRIMARY KEY CHECK(message_id BETWEEN 1 AND 2147483647),
                     inserted_at_ms INTEGER NOT NULL CHECK(inserted_at_ms > 0),
                     response_json TEXT NOT NULL CHECK(length(response_json) <= 65536),
                     recall_kind INTEGER NOT NULL CHECK(recall_kind BETWEEN 0 AND 2),
                     group_code INTEGER,
                     uid TEXT,
                     sequence BLOB,
                     client_sequence BLOB,
                     random INTEGER,
                     timestamp INTEGER
                 );
                 PRAGMA user_version = 2;",
            )?;
            transaction.commit()?;
            Ok(())
        }
        1 => {
            // Schema v1 already reserved the random column for private
            // correlations. Advancing the generation explicitly records that
            // new group rows may use it; existing NULL values stay unavailable.
            connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            Ok(())
        }
        SCHEMA_VERSION => Ok(()),
        _ => Err(MessageStoreError::Configuration),
    }
}

fn database_name(local_id: AccountLocalId) -> OsString {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut name = String::from("messages-");
    for byte in local_id.as_bytes() {
        name.push(char::from(HEX[usize::from(byte >> 4)]));
        name.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    name.push_str(".sqlite3");
    OsString::from(name)
}

fn valid_recall(target: &RecallTarget) -> bool {
    match target {
        RecallTarget::Group {
            group_code,
            sequence,
            random,
        } => *group_code != 0 && *sequence != 0 && random.is_none_or(|value| value != 0),
        RecallTarget::Private {
            uid,
            sequence,
            client_sequence,
            random,
            timestamp,
        } => {
            !uid.is_empty()
                && uid.len() <= MAX_UID_BYTES
                && !uid.chars().any(char::is_control)
                && *sequence != 0
                && *client_sequence != 0
                && *random != 0
                && *timestamp != 0
        }
        RecallTarget::Unavailable => true,
    }
}

fn decode_u64(value: Option<Vec<u8>>) -> Result<u64, MessageStoreError> {
    let bytes: [u8; 8] = value
        .ok_or(MessageStoreError::Configuration)?
        .try_into()
        .map_err(|_error| MessageStoreError::Configuration)?;
    Ok(u64::from_be_bytes(bytes))
}

fn from_i64<T>(value: Option<i64>) -> Result<T, MessageStoreError>
where
    T: TryFrom<i64>,
{
    value
        .and_then(|value| T::try_from(value).ok())
        .ok_or(MessageStoreError::Configuration)
}

fn optional_u32(value: Option<i64>) -> Result<Option<u32>, MessageStoreError> {
    value
        .map(|value| u32::try_from(value).map_err(|_error| MessageStoreError::Configuration))
        .transpose()
}

fn to_i64(value: u64) -> Result<i64, MessageStoreError> {
    i64::try_from(value).map_err(|_error| MessageStoreError::Configuration)
}
