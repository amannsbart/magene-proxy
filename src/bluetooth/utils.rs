use embassy_time::{Duration, Instant, Timer};

pub struct PageBuffer {
    page1_data: Option<[u8; 8]>,
    page1_timestamp: Option<Instant>,
    page2_data: Option<[u8; 8]>,
    page2_timestamp: Option<Instant>,
    data_timeout: Duration,
}

impl PageBuffer {
    pub fn new(data_timeout: Duration) -> Self {
        Self {
            page1_data: None,
            page1_timestamp: None,
            page2_data: None,
            page2_timestamp: None,
            data_timeout,
        }
    }

    pub fn set_page1(&mut self, data: [u8; 8]) {
        self.page1_data = Some(data);
        self.page1_timestamp = Some(Instant::now());
    }

    pub fn set_page2(&mut self, data: [u8; 8]) {
        self.page2_data = Some(data);
        self.page2_timestamp = Some(Instant::now());
    }

    pub fn get(&mut self) -> Option<[u8; 16]> {
        let now = Instant::now();

        // Check if page1 data has expired
        if let Some(timestamp) = self.page1_timestamp {
            if now > timestamp + self.data_timeout {
                self.page1_data = None;
                self.page1_timestamp = None;
            }
        }

        // Check if page2 data has expired
        if let Some(timestamp) = self.page2_timestamp {
            if now > timestamp + self.data_timeout {
                self.page2_data = None;
                self.page2_timestamp = None;
            }
        }

        match (self.page1_data, self.page2_data) {
            (Some(page1), Some(page2)) => {
                // Both pages exist: page1 + page2
                let mut result = [0u8; 16];
                result[..8].copy_from_slice(&page1);
                result[8..].copy_from_slice(&page2);
                Some(result)
            }
            (Some(page1), None) => {
                // Only page1 exists: page1 + default pattern
                let mut result = [0u8; 16];
                result[..8].copy_from_slice(&page1);
                result[8..].copy_from_slice(&[0x31, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
                Some(result)
            }
            (None, Some(page2)) => {
                // Only page2 exists: default pattern + page2
                let mut result = [0u8; 16];
                result[..8].copy_from_slice(&[0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
                result[8..].copy_from_slice(&page2);
                Some(result)
            }
            (None, None) => {
                // Neither page exists - return zero-filled array
                None
            }
        }
    }

    pub fn get_timer(&self) -> Timer {
        let now = Instant::now();
        let mut min_expiry: Option<Instant> = None;

        // Check page1 expiry time (only if data exists)
        if let Some(timestamp) = self.page1_timestamp {
            if self.page1_data.is_some() {
                let expiry = timestamp + self.data_timeout;
                min_expiry = Some(match min_expiry {
                    Some(current_min) => current_min.min(expiry),
                    None => expiry,
                });
            }
        }

        // Check page2 expiry time (only if data exists)
        if let Some(timestamp) = self.page2_timestamp {
            if self.page2_data.is_some() {
                let expiry = timestamp + self.data_timeout;
                min_expiry = Some(match min_expiry {
                    Some(current_min) => current_min.min(expiry),
                    None => expiry,
                });
            }
        }

        let duration = match min_expiry {
            Some(expiry) => {
                if now >= expiry {
                    Duration::from_millis(0)
                } else {
                    expiry - now
                }
            }
            None => Duration::from_millis(0),
        };

        Timer::after(duration)
    }

    pub fn cleanup(&mut self) {
        self.page1_data = None;
        self.page1_timestamp = None;
        self.page2_data = None;
        self.page2_timestamp = None;
    }
}
