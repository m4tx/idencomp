use std::sync::{Arc, Mutex};
use std::time::Duration;

use idencomp::progress::{ByteNum, ProgressNotifier};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

#[derive(Debug)]
struct IdnProgressBarState {
    length: u64,
    bytes: bool,
    initialized: bool,
}

impl IdnProgressBarState {
    fn new() -> Self {
        Self {
            length: 0,
            bytes: false,
            initialized: false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct IdnProgressBar {
    bar: ProgressBar,
    state: Arc<Mutex<IdnProgressBarState>>,
}

impl IdnProgressBar {
    pub fn new() -> IdnProgressBar {
        let init_bar = ProgressBar::hidden();
        init_bar.set_style(ProgressStyle::default_spinner());
        init_bar.enable_steady_tick(Duration::from_millis(50));
        init_bar.set_message("Initializing...");

        Self {
            bar: init_bar,
            state: Arc::new(Mutex::new(IdnProgressBarState::new())),
        }
    }

    pub fn show(&self) {
        self.bar.set_draw_target(ProgressDrawTarget::stderr());
    }

    pub fn is_hidden(&self) -> bool {
        self.bar.is_hidden()
    }

    pub fn finish(&self) {
        self.bar.finish_and_clear()
    }

    #[inline]
    fn init(&self) {
        let mut state = self.state.lock().unwrap();
        if state.initialized {
            return;
        }

        if state.length != 0 {
            self.bar.set_length(state.length);
        }
        self.bar.set_position(0);

        if state.bytes {
            if state.length == 0 {
                self.bar.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner} {bytes}/? ({bytes_per_sec}) {msg}")
                        .expect("Invalid progress bar template"),
                );
            } else {
                self.bar.set_style(
                    ProgressStyle::default_bar()
                        .template("{wide_bar} {bytes}/{total_bytes} [ETA {eta}]")
                        .expect("Invalid progress bar template"),
                );
            }
        } else if state.length == 0 {
            self.bar.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner} {pos}/? ({per_sec}) {msg}")
                    .expect("Invalid progress bar template"),
            );
        } else {
            self.bar.set_style(
                ProgressStyle::default_bar()
                    .template("{wide_bar} {pos}/{len} [ETA {eta}]")
                    .expect("Invalid progress bar template"),
            );
        }
        state.initialized = true;
    }

    pub fn set_total_bytes(&self, length: u64) {
        let mut state = self.state.lock().unwrap();

        state.initialized = false;
        state.bytes = true;
        state.length = length;
    }

    pub fn set_length(&self, length: u64) {
        let mut state = self.state.lock().unwrap();

        state.initialized = false;
        state.bytes = false;
        state.length = length;
    }

    pub fn inc(&self, value: u64) {
        self.init();
        self.bar.inc(value);
    }

    pub fn println<I: AsRef<str>>(&self, msg: I) {
        self.bar.println(msg);
    }
}

impl ProgressNotifier for IdnProgressBar {
    fn processed_bytes(&self, bytes: ByteNum) {
        self.init();
        self.bar.inc(bytes.get() as u64);
    }

    fn set_iter_num(&self, num_iter: u64) {
        self.set_length(num_iter);
    }

    fn inc_iter(&self) {
        self.inc(1);
    }
}
