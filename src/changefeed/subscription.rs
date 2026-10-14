use super::{error, ChangeEvent, Changefeeds};
use crate::Result;
use std::time::{Duration, Instant};

/// Blocking cursor over the bounded durable log. A slow consumer gets an
/// explicit expired-cursor error instead of silently dropping events or growing
/// an unbounded per-subscriber queue. Each subscription has an independent cursor.
pub struct ChangeSubscription {
    feeds: Changefeeds,
    feed: String,
    generation: u64,
    cursor: u64,
    finished: bool,
}

impl ChangeSubscription {
    pub(super) fn new(feeds: Changefeeds, feed: String, generation: u64, cursor: u64) -> Self {
        Self {
            feeds,
            feed,
            generation,
            cursor,
            finished: false,
        }
    }

    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    pub fn recv(&mut self) -> Result<ChangeEvent> {
        Ok(self
            .receive(None)?
            .expect("unlimited wait returns an event"))
    }

    pub fn recv_timeout(&mut self, timeout: Duration) -> Result<Option<ChangeEvent>> {
        self.receive(Some(timeout))
    }

    pub fn try_recv(&mut self) -> Result<Option<ChangeEvent>> {
        self.recv_timeout(Duration::ZERO)
    }

    fn receive(&mut self, timeout: Option<Duration>) -> Result<Option<ChangeEvent>> {
        let started = Instant::now();
        let shared = &self.feeds.shared;
        let mut state = shared.state.lock().unwrap();
        loop {
            let definition = state
                .journal
                .validate_cursor(&self.feed, Some(self.cursor))?;
            if definition.generation != self.generation {
                return Err(error("The subscribed changefeed was dropped and recreated"));
            }
            if let Some(event) = state
                .journal
                .events
                .iter()
                .find(|event| event.feed == self.feed && event.sequence > self.cursor)
            {
                self.cursor = event.sequence;
                return Ok(Some(event.clone()));
            }
            if let Some(timeout) = timeout {
                let remaining = timeout.saturating_sub(started.elapsed());
                if remaining.is_zero() {
                    return Ok(None);
                }
                state = shared.changed.wait_timeout(state, remaining).unwrap().0;
            } else {
                state = shared.changed.wait(state).unwrap();
            }
        }
    }
}

impl Iterator for ChangeSubscription {
    type Item = Result<ChangeEvent>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let event = self.recv();
        if event.is_err() {
            self.finished = true;
        }
        Some(event)
    }
}
