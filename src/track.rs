use std::{num::NonZeroU64, time::Duration};

use crate::protocol::connect::QueueItem;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Track {
    item: QueueItem,
    duration: Duration,
    buffered: Duration,
}

impl Track {
    #[must_use]
    pub fn id(&self) -> NonZeroU64 {
        self.item.track_id
    }

    #[must_use]
    pub fn item(&self) -> &QueueItem {
        &self.item
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        self.duration
    }

    #[must_use]
    pub fn buffered(&self) -> Duration {
        self.buffered
    }
}

impl From<QueueItem> for Track {
    fn from(item: QueueItem) -> Self {
        Self {
            item,

            // TODO: get actual data
            duration: Duration::default(),
            buffered: Duration::default(),
        }
    }
}
