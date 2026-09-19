use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    error::{Error, Result},
    ident::{PackageId, VersionedId},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    IndexChunk {
        done: usize,
        total: usize,
    },
    DownloadStarted {
        package: VersionedId,
        total_bytes: Option<u64>,
    },
    /// `bytes` is the total received so far for this package.
    DownloadProgress {
        package: VersionedId,
        bytes: u64,
    },
    DownloadFinished {
        package: VersionedId,
    },
    Installed {
        package: VersionedId,
    },
    Removed {
        package: PackageId,
    },
}

/// Where long operations report what they do. A frontend keeps the receiver
/// and draws the events its own way. Dropping the receiver cancels the
/// operation at its next event.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    sender: Option<UnboundedSender<Event>>,
}

impl Progress {
    pub fn silent() -> Self {
        Self::default()
    }

    pub fn channel() -> (Self, UnboundedReceiver<Event>) {
        let (sender, receiver) = unbounded_channel();
        (
            Self {
                sender: Some(sender),
            },
            receiver,
        )
    }

    pub fn send(&self, event: Event) -> Result<()> {
        match &self.sender {
            Some(sender) => sender.send(event).map_err(|_| Error::Cancelled),
            None => Ok(()),
        }
    }
}
