//! Bounded per-account `OneBot` message state for Lirvena.

use std::ffi::OsString;
use std::path::Path;

use account_runtime::AccountLocalId;
use local_state::open_private_wal;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

mod quote;

pub use quote::QuoteTarget;

const SCHEMA_VERSION: i64 = 4;
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
        /// Current peer numeric identifier when observed in this storage generation.
        ///
        /// Records migrated from schema v3 retain `None` and cannot be used by
        /// operations that require a numeric peer correlation.
        peer_uin: Option<u32>,
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
    quote: Option<QuoteTarget>,
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
            quote: None,
        })
    }

    /// Adds independently validated QQ quote material to this record.
    #[must_use]
    pub fn with_quote(mut self, quote: QuoteTarget) -> Self {
        self.quote = Some(quote);
        self
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

    /// Returns retained QQ quote material when this storage generation observed it.
    #[must_use]
    pub const fn quote(&self) -> Option<&QuoteTarget> {
        self.quote.as_ref()
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
        let quote = record.quote();
        let quote_elements = quote.map(QuoteTarget::encode_elements).transpose()?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT OR REPLACE INTO messages (
                 message_id, inserted_at_ms, response_json, recall_kind, group_code, uid,
                 sequence, client_sequence, random, timestamp, quote_sequence, quote_message_uid,
                 quote_sender_uin, quote_sender_uid, quote_timestamp, quote_elements, peer_uin
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
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
                quote.map(|value| i64::from(value.sequence())),
                quote.map(|value| value.message_uid().to_be_bytes().to_vec()),
                quote.map(|value| i64::from(value.sender_uin())),
                quote.map(QuoteTarget::sender_uid),
                quote.map(|value| i64::from(value.timestamp())),
                quote_elements,
                fields.peer_uin.map(i64::from),
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
                        client_sequence, random, timestamp, quote_sequence, quote_message_uid,
                        quote_sender_uin, quote_sender_uid, quote_timestamp, quote_elements,
                        peer_uin
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
                        quote_sequence: row.get(9)?,
                        quote_message_uid: row.get(10)?,
                        quote_sender_uin: row.get(11)?,
                        quote_sender_uid: row.get(12)?,
                        quote_timestamp: row.get(13)?,
                        quote_elements: row.get(14)?,
                        peer_uin: row.get(15)?,
                    })
                },
            )
            .optional()?;
        stored.map(|row| row.into_record(message_id)).transpose()
    }

    /// Finds the unique retained message carrying one QQ reply correlation.
    ///
    /// # Errors
    ///
    /// Returns an error for absent correlations, ambiguous retained state, or a
    /// persistence failure.
    pub fn find_quote(
        &self,
        message_uid: u64,
        sequence: u32,
    ) -> Result<Option<MessageRecord>, MessageStoreError> {
        if message_uid == 0 || sequence == 0 {
            return Err(MessageStoreError::Configuration);
        }
        let mut statement = self.connection.prepare(
            "SELECT message_id FROM messages
             WHERE quote_message_uid = ?1 AND quote_sequence = ?2
             ORDER BY inserted_at_ms DESC LIMIT 2",
        )?;
        let ids = statement
            .query_map(
                params![message_uid.to_be_bytes().to_vec(), i64::from(sequence)],
                |row| row.get::<_, i64>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        match ids.as_slice() {
            [] => Ok(None),
            [id] => {
                self.get(u32::try_from(*id).map_err(|_error| MessageStoreError::Configuration)?)
            }
            _ => Err(MessageStoreError::Configuration),
        }
    }

    /// Finds the unique retained local message for one QQ group sequence.
    ///
    /// # Errors
    ///
    /// Returns an error for missing correlations, persistence failure, or ambiguous retained rows.
    pub fn find_group(
        &self,
        group_code: u32,
        sequence: u64,
    ) -> Result<Option<MessageRecord>, MessageStoreError> {
        if group_code == 0 || sequence == 0 {
            return Err(MessageStoreError::Configuration);
        }
        let mut statement = self.connection.prepare(
            "SELECT message_id FROM messages
             WHERE recall_kind = 1 AND group_code = ?1 AND sequence = ?2
             ORDER BY inserted_at_ms DESC LIMIT 2",
        )?;
        let ids = statement
            .query_map(
                params![i64::from(group_code), sequence.to_be_bytes().to_vec()],
                |row| row.get::<_, i64>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        match ids.as_slice() {
            [] => Ok(None),
            [id] => {
                self.get(u32::try_from(*id).map_err(|_error| MessageStoreError::Configuration)?)
            }
            _ => Err(MessageStoreError::Configuration),
        }
    }

    /// Finds the unique retained local message for one complete direct-message correlation.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid fields, persistence failure, or ambiguous retained rows.
    pub fn find_private(
        &self,
        uid: &str,
        peer_uin: u32,
        sequence: u64,
        client_sequence: u64,
        random: u32,
        timestamp: u32,
    ) -> Result<Option<MessageRecord>, MessageStoreError> {
        let target = RecallTarget::Private {
            uid: uid.to_owned(),
            peer_uin: Some(peer_uin),
            sequence,
            client_sequence,
            random,
            timestamp,
        };
        if !valid_recall(&target) {
            return Err(MessageStoreError::Configuration);
        }
        let mut statement = self.connection.prepare(
            "SELECT message_id FROM messages
             WHERE recall_kind = 2 AND uid = ?1 AND peer_uin = ?2 AND sequence = ?3
               AND client_sequence = ?4 AND random = ?5 AND timestamp = ?6
             ORDER BY inserted_at_ms DESC LIMIT 2",
        )?;
        let ids = statement
            .query_map(
                params![
                    uid,
                    i64::from(peer_uin),
                    sequence.to_be_bytes().to_vec(),
                    client_sequence.to_be_bytes().to_vec(),
                    i64::from(random),
                    i64::from(timestamp),
                ],
                |row| row.get::<_, i64>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        match ids.as_slice() {
            [] => Ok(None),
            [id] => {
                self.get(u32::try_from(*id).map_err(|_error| MessageStoreError::Configuration)?)
            }
            _ => Err(MessageStoreError::Configuration),
        }
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
    peer_uin: Option<u32>,
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
                peer_uin: None,
                sequence: Some(*sequence),
                client_sequence: None,
                random: *random,
                timestamp: None,
            },
            RecallTarget::Private {
                uid,
                peer_uin,
                sequence,
                client_sequence,
                random,
                timestamp,
            } => Self {
                kind: 2,
                group_code: None,
                uid: Some(uid),
                peer_uin: *peer_uin,
                sequence: Some(*sequence),
                client_sequence: Some(*client_sequence),
                random: Some(*random),
                timestamp: Some(*timestamp),
            },
            RecallTarget::Unavailable => Self {
                kind: 0,
                group_code: None,
                uid: None,
                peer_uin: None,
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
    peer_uin: Option<i64>,
    sequence: Option<Vec<u8>>,
    client_sequence: Option<Vec<u8>>,
    random: Option<i64>,
    timestamp: Option<i64>,
    quote_sequence: Option<i64>,
    quote_message_uid: Option<Vec<u8>>,
    quote_sender_uin: Option<i64>,
    quote_sender_uid: Option<String>,
    quote_timestamp: Option<i64>,
    quote_elements: Option<Vec<u8>>,
}

impl StoredRow {
    fn into_record(self, message_id: u32) -> Result<MessageRecord, MessageStoreError> {
        let quote = self.load_quote()?;
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
                peer_uin: optional_u32(self.peer_uin)?,
                sequence: decode_u64(self.sequence)?,
                client_sequence: decode_u64(self.client_sequence)?,
                random: from_i64(self.random)?,
                timestamp: from_i64(self.timestamp)?,
            },
            _ => return Err(MessageStoreError::Configuration),
        };
        let response = serde_json::from_str(&self.response_json)
            .map_err(|_error| MessageStoreError::Configuration)?;
        let mut record = MessageRecord::new(
            message_id,
            u64::try_from(self.inserted_at_ms)
                .map_err(|_error| MessageStoreError::Configuration)?,
            response,
            recall,
        )?;
        record.quote = quote;
        Ok(record)
    }

    fn load_quote(&self) -> Result<Option<QuoteTarget>, MessageStoreError> {
        let values = [
            self.quote_sequence.is_some(),
            self.quote_message_uid.is_some(),
            self.quote_sender_uin.is_some(),
            self.quote_sender_uid.is_some(),
            self.quote_timestamp.is_some(),
            self.quote_elements.is_some(),
        ];
        if values.iter().all(|value| !value) {
            return Ok(None);
        }
        if !values.iter().all(|value| *value) {
            return Err(MessageStoreError::Configuration);
        }
        QuoteTarget::new(
            from_i64(self.quote_sequence)?,
            decode_u64(self.quote_message_uid.clone())?,
            from_i64(self.quote_sender_uin)?,
            self.quote_sender_uid
                .clone()
                .ok_or(MessageStoreError::Configuration)?,
            from_i64(self.quote_timestamp)?,
            QuoteTarget::decode_elements(
                self.quote_elements
                    .as_deref()
                    .ok_or(MessageStoreError::Configuration)?,
            )?,
        )
        .map(Some)
    }

    fn no_fields(&self) -> bool {
        self.group_code.is_none()
            && self.uid.is_none()
            && self.peer_uin.is_none()
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
                     timestamp INTEGER,
                     quote_sequence INTEGER,
                     quote_message_uid BLOB,
                     quote_sender_uin INTEGER,
                     quote_sender_uid TEXT,
                     quote_timestamp INTEGER,
                     quote_elements BLOB,
                     peer_uin INTEGER
                 );
                 CREATE INDEX messages_quote_correlation
                     ON messages(quote_message_uid, quote_sequence);
                 PRAGMA user_version = 4;",
            )?;
            transaction.commit()?;
            Ok(())
        }
        1 | 2 => {
            // Schema v1 already reserved the random column for private
            // correlations. Advancing the generation explicitly records that
            // new group rows may use it; existing NULL values stay unavailable.
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "ALTER TABLE messages ADD COLUMN quote_sequence INTEGER;
                 ALTER TABLE messages ADD COLUMN quote_message_uid BLOB;
                 ALTER TABLE messages ADD COLUMN quote_sender_uin INTEGER;
                 ALTER TABLE messages ADD COLUMN quote_sender_uid TEXT;
                 ALTER TABLE messages ADD COLUMN quote_timestamp INTEGER;
                 ALTER TABLE messages ADD COLUMN quote_elements BLOB;
                 ALTER TABLE messages ADD COLUMN peer_uin INTEGER;
                 CREATE INDEX messages_quote_correlation
                     ON messages(quote_message_uid, quote_sequence);
                 PRAGMA user_version = 4;",
            )?;
            transaction.commit()?;
            Ok(())
        }
        3 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "ALTER TABLE messages ADD COLUMN peer_uin INTEGER;
                 PRAGMA user_version = 4;",
            )?;
            transaction.commit()?;
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
            peer_uin,
            sequence,
            client_sequence,
            random,
            timestamp,
        } => {
            !uid.is_empty()
                && uid.len() <= MAX_UID_BYTES
                && !uid.chars().any(char::is_control)
                && peer_uin.is_none_or(|value| value != 0)
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
