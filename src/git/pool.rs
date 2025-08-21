use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    static ref GIT_OPERATION_POOL: Arc<Mutex<VecDeque<()>>> = {
        let mut pool = VecDeque::new();
        for _ in 0..8 {
            pool.push_back(());
        }
        Arc::new(Mutex::new(pool))
    };
}

pub struct PoolGuard;

impl PoolGuard {
    pub fn acquire() -> Option<Self> {
        GIT_OPERATION_POOL
            .lock()
            .ok()?
            .pop_front()
            .map(|_| PoolGuard)
    }

    pub fn try_acquire_with_timeout(timeout_ms: u64) -> Option<Self> {
        let start = std::time::Instant::now();
        while start.elapsed().as_millis() < timeout_ms as u128 {
            if let Some(guard) = Self::acquire() {
                return Some(guard);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        None
    }
}

impl Drop for PoolGuard {
    fn drop(&mut self) {
        if let Ok(mut pool) = GIT_OPERATION_POOL.lock() {
            pool.push_back(());
        }
    }
}
