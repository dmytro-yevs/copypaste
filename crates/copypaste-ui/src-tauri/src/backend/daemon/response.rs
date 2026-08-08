use copypaste_ipc::{
    BackupData, CloudStatusData, CloudSyncData, ConfigApplied, DiscoveredDevice, ExportData,
    ImagePreview, ImportData, Item, PeerInfo, Response, ResponseData, StatusData, SyncResult,
};

use super::super::{BackendError, Page, Result};

/// Branch on the stable error code; the message is only diagnostic context.
pub(super) fn into_data(response: Response) -> Result<Option<ResponseData>> {
    if response.ok {
        return Ok(response.data);
    }
    Err(BackendError::from_code(
        response.error_code,
        response.raw_error_code.as_deref(),
        response.error.as_deref(),
    ))
}

pub(super) fn expect_page(data: Option<ResponseData>) -> Result<Page> {
    match data {
        Some(ResponseData::Page(page)) => Ok(Page::from(page)),
        _ => Err(BackendError::wrong_shape("a page of items")),
    }
}

pub(super) fn expect_discovered(data: Option<ResponseData>) -> Result<Vec<DiscoveredDevice>> {
    match data {
        Some(ResponseData::Discovered(found)) => Ok(found.devices),
        _ => Err(BackendError::wrong_shape("a list of nearby devices")),
    }
}

pub(super) fn expect_item(data: Option<ResponseData>) -> Result<Item> {
    match data {
        Some(ResponseData::Item(item)) => Ok(item),
        _ => Err(BackendError::wrong_shape("an item")),
    }
}

pub(super) fn expect_image_preview(data: Option<ResponseData>) -> Result<ImagePreview> {
    match data {
        Some(ResponseData::ImagePreview(preview)) => Ok(preview),
        _ => Err(BackendError::wrong_shape("an image preview")),
    }
}

pub(super) fn expect_status(data: Option<ResponseData>) -> Result<StatusData> {
    match data {
        Some(ResponseData::Status(status)) => Ok(status),
        _ => Err(BackendError::wrong_shape("daemon status")),
    }
}

pub(super) fn expect_empty(data: Option<ResponseData>) -> Result<()> {
    match data {
        Some(ResponseData::Empty {}) | None => Ok(()),
        _ => Err(BackendError::wrong_shape("an empty response")),
    }
}

pub(super) fn expect_peers(data: Option<ResponseData>) -> Result<Vec<PeerInfo>> {
    match data {
        Some(ResponseData::Peers(peers)) => Ok(peers),
        _ => Err(BackendError::wrong_shape("a list of devices")),
    }
}

pub(super) fn expect_sync(data: Option<ResponseData>) -> Result<Vec<SyncResult>> {
    match data {
        Some(ResponseData::Sync(results)) => Ok(results),
        _ => Err(BackendError::wrong_shape("a sync report")),
    }
}

pub(super) fn expect_cloud_status(data: Option<ResponseData>) -> Result<CloudStatusData> {
    match data {
        Some(ResponseData::CloudStatus(status)) => Ok(status),
        _ => Err(BackendError::wrong_shape("cloud sync status")),
    }
}

pub(super) fn expect_cloud_sync(data: Option<ResponseData>) -> Result<CloudSyncData> {
    match data {
        Some(ResponseData::CloudSync(result)) => Ok(result),
        _ => Err(BackendError::wrong_shape("a cloud sync report")),
    }
}

pub(super) fn expect_config(data: Option<ResponseData>) -> Result<ConfigApplied> {
    match data {
        Some(ResponseData::Config(applied)) => Ok(applied),
        _ => Err(BackendError::wrong_shape("the service's settings")),
    }
}

pub(super) fn expect_export(data: Option<ResponseData>) -> Result<ExportData> {
    match data {
        Some(ResponseData::Export(export)) => Ok(export),
        _ => Err(BackendError::wrong_shape("an export")),
    }
}

pub(super) fn expect_import(data: Option<ResponseData>) -> Result<ImportData> {
    match data {
        Some(ResponseData::Import(result)) => Ok(result),
        _ => Err(BackendError::wrong_shape("an import report")),
    }
}

pub(super) fn expect_backup(data: Option<ResponseData>) -> Result<BackupData> {
    match data {
        Some(ResponseData::Backup(backup)) => Ok(backup),
        _ => Err(BackendError::wrong_shape("a backup report")),
    }
}

/// An omitted count still acknowledges a clear that deleted no rows.
pub(super) fn expect_count(data: Option<ResponseData>) -> Result<u64> {
    match data {
        Some(ResponseData::Count(count)) => Ok(count),
        Some(ResponseData::Empty {}) | None => Ok(0),
        _ => Err(BackendError::wrong_shape("a count")),
    }
}
