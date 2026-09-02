use core::fmt;

use sha2::{Digest, Sha256};

use crate::{
    AccountSlotId, Digest32, ExchangeId, MAX_OPAQUE_AGGREGATE_LEN, MAX_OPAQUE_SLOT_LEN,
    MAX_OPAQUE_SLOTS, OpaqueError,
};

const OPAQUE_BINDING_DOMAIN: &[u8] = b"ceylith/v2/opaque-binding/v1";

/// Numeric identifier for material whose private semantics stay in Ceylith.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpaqueSlotId(u32);

impl OpaqueSlotId {
    /// Creates a non-zero slot identifier.
    ///
    /// # Errors
    ///
    /// Returns an error for zero, which is reserved for an absent slot.
    pub const fn new(value: u32) -> Result<Self, OpaqueError> {
        if value == 0 {
            Err(OpaqueError::InvalidSlotId)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the public numeric identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One bounded opaque public-contract value.
#[derive(Clone, Eq, PartialEq)]
pub struct OpaqueSlot {
    id: OpaqueSlotId,
    value: Box<[u8]>,
}

impl OpaqueSlot {
    /// Creates one bounded slot.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is empty or exceeds the public slot bound.
    pub fn new(id: OpaqueSlotId, value: impl Into<Box<[u8]>>) -> Result<Self, OpaqueError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_OPAQUE_SLOT_LEN {
            return Err(OpaqueError::InvalidSlotValue);
        }
        Ok(Self { id, value })
    }

    /// Returns the numeric slot identifier.
    #[must_use]
    pub const fn id(&self) -> OpaqueSlotId {
        self.id
    }

    /// Borrows the opaque value.
    #[must_use]
    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

impl fmt::Debug for OpaqueSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpaqueSlot")
            .field("id", &self.id)
            .field("value", &"<opaque>")
            .field("value_len", &self.value.len())
            .finish()
    }
}

/// Sorted, unique and bounded opaque public-contract slots.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OpaqueSlots(Box<[OpaqueSlot]>);

impl OpaqueSlots {
    /// Validates, sorts and stores opaque slots.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate identifiers or exceeded collection bounds.
    pub fn new(mut slots: Vec<OpaqueSlot>) -> Result<Self, OpaqueError> {
        if slots.len() > MAX_OPAQUE_SLOTS {
            return Err(OpaqueError::Bounds);
        }
        slots.sort_unstable_by_key(OpaqueSlot::id);
        if slots.windows(2).any(|pair| pair[0].id() == pair[1].id()) {
            return Err(OpaqueError::DuplicateSlot);
        }
        let aggregate = slots
            .iter()
            .try_fold(0_usize, |total, slot| total.checked_add(slot.value().len()));
        if aggregate.is_none_or(|length| length > MAX_OPAQUE_AGGREGATE_LEN) {
            return Err(OpaqueError::Bounds);
        }
        Ok(Self(slots.into_boxed_slice()))
    }

    /// Decodes generated wire slots through the same collection validation.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identifiers, values, duplicates or bounds.
    pub fn from_wire(slots: &[crate::proto::OpaqueSlot]) -> Result<Self, OpaqueError> {
        let decoded = slots
            .iter()
            .map(|slot| {
                let id = OpaqueSlotId::new(slot.slot)?;
                OpaqueSlot::new(id, slot.value.clone())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(decoded)
    }

    /// Copies the validated slots into their generated wire representation.
    #[must_use]
    pub fn to_wire(&self) -> Vec<crate::proto::OpaqueSlot> {
        self.0
            .iter()
            .map(|slot| crate::proto::OpaqueSlot {
                slot: slot.id().get(),
                value: slot.value().to_vec(),
            })
            .collect()
    }

    /// Looks up one slot by its numeric identifier.
    #[must_use]
    pub fn get(&self, id: OpaqueSlotId) -> Option<&OpaqueSlot> {
        self.0
            .binary_search_by_key(&id, OpaqueSlot::id)
            .ok()
            .map(|index| &self.0[index])
    }

    /// Iterates over slots in numeric order.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &OpaqueSlot> {
        self.0.iter()
    }

    /// Returns the number of slots.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether no opaque material is present.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Computes the canonical digest for one bounded opaque exchange request.
///
/// Slot values remain semantically opaque. Their validated numeric order,
/// widths and bytes are included so a server can reject altered or omitted
/// material before dispatching it to a private implementation.
#[must_use]
pub fn opaque_binding_digest(
    exchange_id: ExchangeId,
    account_slot_id: AccountSlotId,
    generation: u64,
    expires_at_ms: u64,
    slots: &OpaqueSlots,
) -> Digest32 {
    let mut digest = Sha256::new();
    digest.update(OPAQUE_BINDING_DOMAIN);
    digest.update(exchange_id.as_bytes());
    digest.update(account_slot_id.as_bytes());
    digest.update(generation.to_be_bytes());
    digest.update(expires_at_ms.to_be_bytes());
    digest.update(u32::try_from(slots.len()).unwrap_or(u32::MAX).to_be_bytes());
    for slot in slots.iter() {
        digest.update(slot.id().get().to_be_bytes());
        digest.update(
            u32::try_from(slot.value().len())
                .unwrap_or(u32::MAX)
                .to_be_bytes(),
        );
        digest.update(slot.value());
    }
    Digest32::from_bytes(digest.finalize().into())
}
