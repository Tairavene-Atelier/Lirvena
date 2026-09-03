use super::MessageStoreError;

const MAX_UID_BYTES: usize = 128;
const MAX_ELEMENTS: usize = 512;
const MAX_ELEMENT_BYTES: usize = 256 * 1024;
const MAX_TOTAL_BYTES: usize = 1024 * 1024;

/// Retained QQ material required to build a real reply element.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuoteTarget {
    sequence: u32,
    message_uid: u64,
    sender_uin: u32,
    sender_uid: String,
    timestamp: u32,
    elements: Box<[Vec<u8>]>,
}

impl QuoteTarget {
    /// Creates one bounded, evidence-backed quote target.
    ///
    /// # Errors
    ///
    /// Returns an error when a required correlation is absent or the retained
    /// QQ elements exceed the message decoder's public bounds.
    pub fn new(
        sequence: u32,
        message_uid: u64,
        numeric_sender: u32,
        current_sender: String,
        timestamp: u32,
        elements: Vec<Vec<u8>>,
    ) -> Result<Self, MessageStoreError> {
        let total = elements.iter().try_fold(0usize, |total, element| {
            total
                .checked_add(element.len())
                .ok_or(MessageStoreError::Configuration)
        })?;
        if sequence == 0
            || message_uid == 0
            || numeric_sender == 0
            || current_sender.is_empty()
            || current_sender.len() > MAX_UID_BYTES
            || current_sender.chars().any(char::is_control)
            || timestamp == 0
            || elements.is_empty()
            || elements.len() > MAX_ELEMENTS
            || elements
                .iter()
                .any(|element| element.is_empty() || element.len() > MAX_ELEMENT_BYTES)
            || total > MAX_TOTAL_BYTES
        {
            return Err(MessageStoreError::Configuration);
        }
        Ok(Self {
            sequence,
            message_uid,
            sender_uin: numeric_sender,
            sender_uid: current_sender,
            timestamp,
            elements: elements.into_boxed_slice(),
        })
    }

    /// Returns the source sequence carried in QQ's reply element.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    /// Returns the original QQ message identifier.
    #[must_use]
    pub const fn message_uid(&self) -> u64 {
        self.message_uid
    }

    /// Returns the original sender's numeric identity.
    #[must_use]
    pub const fn sender_uin(&self) -> u32 {
        self.sender_uin
    }

    /// Returns the original sender's current UID.
    #[must_use]
    pub fn sender_uid(&self) -> &str {
        &self.sender_uid
    }

    /// Returns the original message timestamp.
    #[must_use]
    pub const fn timestamp(&self) -> u32 {
        self.timestamp
    }

    /// Returns the original encoded QQ elements in wire order.
    #[must_use]
    pub const fn elements(&self) -> &[Vec<u8>] {
        &self.elements
    }

    pub(super) fn encode_elements(&self) -> Result<Vec<u8>, MessageStoreError> {
        let mut output = Vec::new();
        for element in &self.elements {
            let length =
                u32::try_from(element.len()).map_err(|_error| MessageStoreError::Configuration)?;
            output.extend_from_slice(&length.to_be_bytes());
            output.extend_from_slice(element);
        }
        Ok(output)
    }

    pub(super) fn decode_elements(input: &[u8]) -> Result<Vec<Vec<u8>>, MessageStoreError> {
        if input.is_empty() || input.len() > MAX_TOTAL_BYTES + MAX_ELEMENTS * 4 {
            return Err(MessageStoreError::Configuration);
        }
        let mut remaining = input;
        let mut elements = Vec::new();
        while !remaining.is_empty() {
            let length_bytes: [u8; 4] = remaining
                .get(..4)
                .ok_or(MessageStoreError::Configuration)?
                .try_into()
                .map_err(|_error| MessageStoreError::Configuration)?;
            remaining = &remaining[4..];
            let length = usize::try_from(u32::from_be_bytes(length_bytes))
                .map_err(|_error| MessageStoreError::Configuration)?;
            if length == 0 || length > MAX_ELEMENT_BYTES || elements.len() == MAX_ELEMENTS {
                return Err(MessageStoreError::Configuration);
            }
            let element = remaining
                .get(..length)
                .ok_or(MessageStoreError::Configuration)?;
            elements.push(element.to_vec());
            remaining = &remaining[length..];
        }
        Ok(elements)
    }
}
