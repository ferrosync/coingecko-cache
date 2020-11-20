use rand::rngs::ThreadRng;
use rand::RngCore;
use reqwest::Url;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct UrlCacheBuster<'a> {
    rng: ThreadRng,
    url: &'a Url,
}

impl<'a> UrlCacheBuster<'a> {
    pub fn new(url: &'a Url) -> UrlCacheBuster<'a> {
        UrlCacheBuster { rng: rand::thread_rng(), url }
    }

    pub fn next(&mut self) -> Url {
        let num = self.rng.next_u64();

        self.url.clone()
            .query_pairs_mut()
            .append_pair("_", num.to_string().as_str())
            .finish()
            .to_owned()
    }
}

#[derive(Clone)]
pub struct AtomicCancellation {
    value: Arc<AtomicBool>,
}

unsafe impl Send for AtomicCancellation { }
unsafe impl Sync for AtomicCancellation { }

impl AtomicCancellation {
    pub fn new() -> AtomicCancellation {
        AtomicCancellation {
            value: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        (*self.value).load(Ordering::SeqCst)
    }

    pub fn can_continue(&self) -> bool {
        !self.is_cancelled()
    }

    pub fn cancel(&self) {
        (*self.value).swap(true, Ordering::SeqCst);
    }
}
