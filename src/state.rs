use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use reqwest::Client;

use crate::activity::ActivityDb;
use crate::config::Config;
use crate::geo::Geo;
use crate::model::Snapshot;

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<Inner>,
}

pub struct Inner {
    pub config: Config,
    pub snapshot: RwLock<Arc<Snapshot>>,
    pub geo: Geo,
    pub activity: Mutex<ActivityDb>,
    pub heatmap: RwLock<Option<Vec<u8>>>,
    pub bans: RwLock<Vec<String>>,
    pub http: Client,
    /// Rendered front pages for the current snapshot. The age token is filled per request.
    pub pages: Mutex<PageCache>,
}

pub struct PageCache {
    pub ptr: usize,
    pub plain: String,
    pub rjz: String,
}

impl std::ops::Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.inner
    }
}

impl AppState {
    pub fn new(
        config: Config,
        snapshot: Snapshot,
        geo: Geo,
        activity: ActivityDb,
        heatmap: Option<Vec<u8>>,
        bans: Vec<String>,
        http: Client,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                snapshot: RwLock::new(Arc::new(snapshot)),
                geo,
                activity: Mutex::new(activity),
                heatmap: RwLock::new(heatmap),
                bans: RwLock::new(bans),
                http,
                pages: Mutex::new(PageCache {
                    ptr: 0,
                    plain: String::new(),
                    rjz: String::new(),
                }),
            }),
        }
    }
}
