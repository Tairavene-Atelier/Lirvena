use prost::Message;

/// Compact protobuf message for query schemas consisting of enabled boolean field numbers.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProtobufBoolFields {
    fields: Vec<BoolField>,
}

impl ProtobufBoolFields {
    pub(crate) fn enabled(numbers: impl IntoIterator<Item = u32>) -> Self {
        Self {
            fields: numbers
                .into_iter()
                .map(|number| BoolField {
                    number,
                    value: true,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BoolField {
    number: u32,
    value: bool,
}

impl Message for ProtobufBoolFields {
    fn encode_raw(&self, buffer: &mut impl prost::bytes::BufMut) {
        for field in &self.fields {
            prost::encoding::bool::encode(field.number, &field.value, buffer);
        }
    }

    fn merge_field(
        &mut self,
        number: u32,
        wire_type: prost::encoding::WireType,
        buffer: &mut impl prost::bytes::Buf,
        context: prost::encoding::DecodeContext,
    ) -> Result<(), prost::DecodeError> {
        let mut value = false;
        prost::encoding::bool::merge(wire_type, &mut value, buffer, context)?;
        self.fields.push(BoolField { number, value });
        Ok(())
    }

    fn encoded_len(&self) -> usize {
        self.fields
            .iter()
            .map(|field| prost::encoding::bool::encoded_len(field.number, &field.value))
            .sum()
    }

    fn clear(&mut self) {
        self.fields.clear();
    }
}
