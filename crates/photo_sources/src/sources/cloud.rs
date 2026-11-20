//! Cloud account connectors. Every method is a documented stub and makes no network calls.

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloudKind {
    GoogleDrive,
    GooglePhotos,
    OneDrive,
    Dropbox,
    Icloud,
}

impl CloudKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CloudKind::GoogleDrive => "google_drive",
            CloudKind::GooglePhotos => "google_photos",
            CloudKind::OneDrive => "onedrive",
            CloudKind::Dropbox => "dropbox",
            CloudKind::Icloud => "icloud",
        }
    }

    pub fn all() -> [CloudKind; 5] {
        [
            CloudKind::GoogleDrive,
            CloudKind::GooglePhotos,
            CloudKind::OneDrive,
            CloudKind::Dropbox,
            CloudKind::Icloud,
        ]
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct CloudStub {
    pub kind: CloudKind,
    pub available: bool,
    pub route: &'static str,
}

impl CloudStub {
    pub fn describe(kind: CloudKind) -> Self {
        Self {
            kind,
            available: false,
            route: route(kind),
        }
    }
}

pub fn route(kind: CloudKind) -> &'static str {
    match kind {
        CloudKind::GoogleDrive => {
            "Requires an approved OAuth client and the Google Drive API / Picker (drive.file, user-selected files only). This build makes no network calls. Until that is configured, add a locally synced Drive folder as a folder source, or import a Google Takeout zip."
        }
        CloudKind::GooglePhotos => {
            "As of March 2025, the Google Photos API no longer allows a third-party app to scan a whole library; the user must pick items one by one (Photos Picker), or import a Google Takeout of Google Photos. This build makes no network calls."
        }
        CloudKind::OneDrive => {
            "Requires a Microsoft Graph app registration with Files.Read (or the OneDrive file picker). This build makes no network calls. Until configured, add a locally synced OneDrive folder as a folder source."
        }
        CloudKind::Dropbox => {
            "Requires a Dropbox app key and the Chooser or Files API. This build makes no network calls. Until configured, add a locally synced Dropbox folder as a folder source."
        }
        CloudKind::Icloud => {
            "iCloud has no public photos API. Do not use unofficial scrapers (they need repeated 2FA and violate reliability/license goals). On Windows, add the iCloud for Windows synced folder (typically %USERPROFILE%\\iCloudPhotos\\Photos). Otherwise use an iCloud/Photos export and import the folder or zip. Native PhotoKit is a future macOS/iOS route."
        }
    }
}

pub fn connect(kind: CloudKind) -> Result<CloudStub> {
    Err(Error::Stub {
        feature: match kind {
            CloudKind::GoogleDrive => "Google Drive",
            CloudKind::GooglePhotos => "Google Photos",
            CloudKind::OneDrive => "OneDrive",
            CloudKind::Dropbox => "Dropbox",
            CloudKind::Icloud => "iCloud",
        },
        route: route(kind),
    })
}

pub fn list_stubs() -> Vec<CloudStub> {
    CloudKind::all().into_iter().map(CloudStub::describe).collect()
}
