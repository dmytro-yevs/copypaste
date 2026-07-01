//! `delete_item`: remove a single item from a device's sync inbox.

use crate::error::RelayError;

impl super::super::RelayStore {
    /// Remove item `item_id` from `device_id`'s inbox (matched by id as string).
    pub fn delete_item(&mut self, device_id: &str, item_id: &str) -> Result<(), RelayError> {
        let parsed_id: i64 = item_id
            .parse()
            .map_err(|_| RelayError::BadRequest("item_id must be an integer".to_string()))?;

        let inbox = self
            .sync_items
            .get_mut(device_id)
            .ok_or(RelayError::DeviceNotFound)?;

        let before = inbox.len();
        inbox.retain(|item| item.id != parsed_id);
        if inbox.len() == before {
            return Err(RelayError::ItemNotFound);
        }
        // R1b write-through: remove the same row from the durable store. The
        // in-memory removal already succeeded (we only reach here when the item
        // existed), so propagate any persistence failure as 500.
        self.db.delete_item(device_id, parsed_id)?;
        Ok(())
    }
}
