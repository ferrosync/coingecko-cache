use rand::rngs::ThreadRng;
use rand::RngCore;
use reqwest::Url;

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
