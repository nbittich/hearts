pub static MAX_ROOMS: usize = 100;
pub static CORS_ALLOW_ORIGIN: &str = "CORS_ALLOW_ORIGIN";
pub static BODY_SIZE_LIMIT: &str = "BODY_SIZE_LIMIT";
pub static SERVICE_HOST: &str = "SERVICE_HOST";
pub static WS_ENDPOINT: &str = "WS_ENDPOINT";
pub static SERVICE_PORT: &str = "SERVICE_PORT";
pub static SERVICE_CONFIG_VOLUME: &str = "SERVICE_CONFIG_VOLUME";
pub static SERVICE_DATA_VOLUME: &str = "SERVICE_DATA_VOLUME";
pub static SERVICE_APPLICATION_NAME: &str = "SERVICE_APPLICATION_NAME";
pub static SERVICE_COLLECTION_NAME: &str = "SERVICE_COLLECTION_NAME";
pub static COOKIE: &str = "HeartsCookie";
pub static USER_ID: &str = "X_USER_ID";
// this may have to be increased
// broadcast channels are super weird and hard to debug
// thus if there's an issue, it probably means you have to increase this.
pub static ABRITRATRY_CHANNEL_CAPACITY: usize = 64;
pub static DEFAULT_HANDS: u8 = 3;
pub static TIMEOUT_SECS: usize = 5;
pub static BOT_SLEEP_SECS: u64 = 1;
pub static COMPUTE_SCORE_DELAY_SECS: u64 = 1;

#[cfg(test)]
mod test {}
