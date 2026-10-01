use std::collections::BTreeMap;

use crate::model::MethodRecord;

#[must_use]
pub fn group_by_declaring_type(records: &[MethodRecord]) -> BTreeMap<String, Vec<MethodRecord>> {
    let mut grouped = BTreeMap::<String, Vec<MethodRecord>>::new();
    for record in records {
        grouped
            .entry(record.declaring_type.clone())
            .or_default()
            .push(record.clone());
    }
    grouped
}

#[must_use]
pub fn search(records: &[MethodRecord], query: &str) -> Vec<MethodRecord> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return records.to_vec();
    }

    records
        .iter()
        .filter(|record| {
            record.symbol.to_ascii_lowercase().contains(&needle)
                || record.declaring_type.to_ascii_lowercase().contains(&needle)
                || record
                    .declaring_type_fqcn
                    .as_deref()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains(&needle)
                || record.member_name.to_ascii_lowercase().contains(&needle)
                || record
                    .parameters
                    .iter()
                    .any(|ty| ty.display_name().to_ascii_lowercase().contains(&needle))
                || record
                    .return_type
                    .as_ref()
                    .is_some_and(|ty| ty.display_name().to_ascii_lowercase().contains(&needle))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{parse_symbols, search};

    #[test]
    fn searches_core_fields() {
        let records = parse_symbols(
            "v",
            [
                "SystemBoolean.__TryParse__SystemString_SystemBooleanRef__SystemBoolean".to_owned(),
                "SystemArray.__Clear__SystemArray_SystemInt32_SystemInt32__SystemVoid".to_owned(),
            ],
        );

        assert_eq!(search(&records, "try").len(), 1);
        assert_eq!(search(&records, "system").len(), 2);
    }
}
