use prost::Message;

const MAX_GROUPS: usize = 10_000;
const MAX_GROUP_TEXT_BYTES: usize = 4_096;

/// One bounded QQ group directory entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupEntry {
    /// Numeric QQ group identifier.
    pub group_id: u32,
    /// Group display name.
    pub group_name: String,
    /// Current member count.
    pub member_count: u32,
    /// Configured member capacity.
    pub max_member_count: u32,
}

/// Encodes the Linux NT group-list OIDB request.
#[must_use]
pub fn encode_group_list_request() -> Vec<u8> {
    OidbEnvelope {
        command: 0x0fe5,
        subcommand: 2,
        body: Some(GroupRequest {
            config: Some(GroupConfig {
                first: Some(FirstConfig {
                    fields: true_fields(1..=32, &[5_001, 5_002, 5_003]),
                }),
                second: Some(SecondConfig {
                    fields: true_fields(1..=8, &[]),
                }),
                third: Some(ThirdConfig {
                    field5: true,
                    field6: true,
                }),
            }),
        }),
        reserved: 1,
    }
    .encode_to_vec()
}

/// Parses the Linux NT group-list OIDB response.
///
/// # Errors
///
/// Returns an error for rejected, malformed, excessive, duplicate or unsafe group data.
pub fn parse_group_list(input: &[u8]) -> Result<Vec<GroupEntry>, GroupDirectoryError> {
    let envelope = OidbResponse::decode(input).map_err(|_error| GroupDirectoryError)?;
    if envelope.error_code != 0 {
        return Err(GroupDirectoryError);
    }
    let groups = envelope.body.ok_or(GroupDirectoryError)?.groups;
    if groups.len() > MAX_GROUPS {
        return Err(GroupDirectoryError);
    }
    let mut seen = std::collections::BTreeSet::new();
    groups
        .into_iter()
        .map(|group| {
            let info = group.info.ok_or(GroupDirectoryError)?;
            if group.group_id == 0
                || !seen.insert(group.group_id)
                || info.member_count > info.max_member_count
                || !valid_text(&info.group_name)
            {
                return Err(GroupDirectoryError);
            }
            Ok(GroupEntry {
                group_id: group.group_id,
                group_name: info.group_name,
                member_count: info.member_count,
                max_member_count: info.max_member_count,
            })
        })
        .collect()
}

fn valid_text(value: &str) -> bool {
    value.len() <= MAX_GROUP_TEXT_BYTES && !value.chars().any(char::is_control)
}

fn true_fields(base: impl Iterator<Item = u32>, extras: &[u32]) -> Vec<BoolField> {
    base.chain(extras.iter().copied())
        .map(|number| BoolField {
            number,
            value: true,
        })
        .collect()
}

/// Opaque group-directory codec error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupDirectoryError;

impl core::fmt::Display for GroupDirectoryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("QQ group directory data is invalid")
    }
}

impl std::error::Error for GroupDirectoryError {}

// `prost` cannot express a runtime field number. Config messages below therefore use a compact
// manual encoder embedded as message bytes rather than duplicating 45 boolean struct fields.
#[derive(Clone, PartialEq, Message)]
struct OidbEnvelope {
    #[prost(uint32, tag = "1")]
    command: u32,
    #[prost(uint32, tag = "2")]
    subcommand: u32,
    #[prost(message, optional, tag = "4")]
    body: Option<GroupRequest>,
    #[prost(int32, tag = "12")]
    reserved: i32,
}

#[derive(Clone, PartialEq, Message)]
struct GroupRequest {
    #[prost(message, optional, tag = "1")]
    config: Option<GroupConfig>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupConfig {
    #[prost(message, optional, tag = "1")]
    first: Option<FirstConfig>,
    #[prost(message, optional, tag = "2")]
    second: Option<SecondConfig>,
    #[prost(message, optional, tag = "3")]
    third: Option<ThirdConfig>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FirstConfig {
    fields: Vec<BoolField>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct SecondConfig {
    fields: Vec<BoolField>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BoolField {
    number: u32,
    value: bool,
}

impl Message for FirstConfig {
    fn encode_raw(&self, buffer: &mut impl prost::bytes::BufMut) {
        encode_fields(&self.fields, buffer);
    }

    fn merge_field(
        &mut self,
        tag: u32,
        wire_type: prost::encoding::WireType,
        buffer: &mut impl prost::bytes::Buf,
        context: prost::encoding::DecodeContext,
    ) -> Result<(), prost::DecodeError> {
        merge_bool_field(&mut self.fields, tag, wire_type, buffer, context)
    }

    fn encoded_len(&self) -> usize {
        fields_len(&self.fields)
    }

    fn clear(&mut self) {
        self.fields.clear();
    }
}

impl Message for SecondConfig {
    fn encode_raw(&self, buffer: &mut impl prost::bytes::BufMut) {
        encode_fields(&self.fields, buffer);
    }

    fn merge_field(
        &mut self,
        tag: u32,
        wire_type: prost::encoding::WireType,
        buffer: &mut impl prost::bytes::Buf,
        context: prost::encoding::DecodeContext,
    ) -> Result<(), prost::DecodeError> {
        merge_bool_field(&mut self.fields, tag, wire_type, buffer, context)
    }

    fn encoded_len(&self) -> usize {
        fields_len(&self.fields)
    }

    fn clear(&mut self) {
        self.fields.clear();
    }
}

fn encode_fields(fields: &[BoolField], buffer: &mut impl prost::bytes::BufMut) {
    for field in fields {
        prost::encoding::bool::encode(field.number, &field.value, buffer);
    }
}

fn merge_bool_field(
    fields: &mut Vec<BoolField>,
    number: u32,
    wire_type: prost::encoding::WireType,
    buffer: &mut impl prost::bytes::Buf,
    context: prost::encoding::DecodeContext,
) -> Result<(), prost::DecodeError> {
    let mut value = false;
    prost::encoding::bool::merge(wire_type, &mut value, buffer, context)?;
    fields.push(BoolField { number, value });
    Ok(())
}

fn fields_len(fields: &[BoolField]) -> usize {
    fields
        .iter()
        .map(|field| prost::encoding::bool::encoded_len(field.number, &field.value))
        .sum()
}

#[derive(Clone, Copy, PartialEq, Message)]
struct ThirdConfig {
    #[prost(bool, tag = "5")]
    field5: bool,
    #[prost(bool, tag = "6")]
    field6: bool,
}

#[derive(Clone, PartialEq, Message)]
struct OidbResponse {
    #[prost(uint32, tag = "3")]
    error_code: u32,
    #[prost(message, optional, tag = "4")]
    body: Option<GroupResponse>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupResponse {
    #[prost(message, repeated, tag = "2")]
    groups: Vec<RawGroup>,
}

#[derive(Clone, PartialEq, Message)]
struct RawGroup {
    #[prost(uint32, tag = "3")]
    group_id: u32,
    #[prost(message, optional, tag = "4")]
    info: Option<GroupInfo>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupInfo {
    #[prost(uint32, tag = "3")]
    max_member_count: u32,
    #[prost(uint32, tag = "4")]
    member_count: u32,
    #[prost(string, tag = "5")]
    group_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_has_expected_command_and_all_config_fields() -> Result<(), Box<dyn std::error::Error>>
    {
        let request = OidbEnvelope::decode(encode_group_list_request().as_slice())?;
        assert_eq!(request.command, 0x0fe5);
        assert_eq!(request.subcommand, 2);
        assert_eq!(request.reserved, 1);

        let config = request
            .body
            .and_then(|body| body.config)
            .ok_or(GroupDirectoryError)?;
        assert_eq!(
            config.first.ok_or(GroupDirectoryError)?.fields,
            true_fields(1..=32, &[5_001, 5_002, 5_003])
        );
        assert_eq!(
            config.second.ok_or(GroupDirectoryError)?.fields,
            true_fields(1..=8, &[])
        );
        assert_eq!(
            config.third,
            Some(ThirdConfig {
                field5: true,
                field6: true,
            })
        );
        Ok(())
    }

    #[test]
    fn response_returns_bounded_group_entries() {
        let response = OidbResponse {
            error_code: 0,
            body: Some(GroupResponse {
                groups: vec![RawGroup {
                    group_id: 12_345,
                    info: Some(GroupInfo {
                        max_member_count: 500,
                        member_count: 27,
                        group_name: "Lirvena test group".to_owned(),
                    }),
                }],
            }),
        }
        .encode_to_vec();

        assert_eq!(
            parse_group_list(&response),
            Ok(vec![GroupEntry {
                group_id: 12_345,
                group_name: "Lirvena test group".to_owned(),
                member_count: 27,
                max_member_count: 500,
            }])
        );
    }

    #[test]
    fn response_rejects_duplicate_and_impossible_groups() {
        let duplicate = RawGroup {
            group_id: 7,
            info: Some(GroupInfo {
                max_member_count: 10,
                member_count: 1,
                group_name: "group".to_owned(),
            }),
        };
        let response = OidbResponse {
            error_code: 0,
            body: Some(GroupResponse {
                groups: vec![duplicate.clone(), duplicate],
            }),
        }
        .encode_to_vec();
        assert_eq!(parse_group_list(&response), Err(GroupDirectoryError));

        let response = OidbResponse {
            error_code: 0,
            body: Some(GroupResponse {
                groups: vec![RawGroup {
                    group_id: 8,
                    info: Some(GroupInfo {
                        max_member_count: 10,
                        member_count: 11,
                        group_name: "group".to_owned(),
                    }),
                }],
            }),
        }
        .encode_to_vec();
        assert_eq!(parse_group_list(&response), Err(GroupDirectoryError));
    }
}
