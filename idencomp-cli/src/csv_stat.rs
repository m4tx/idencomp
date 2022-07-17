use std::io;
use std::sync::Mutex;

#[derive(Debug)]
struct CsvStatOutputState {
    writer: csv::Writer<io::Stdout>,
    initialized: bool,
}

#[derive(Debug)]
pub(crate) struct CsvStatOutput {
    writer: Option<Mutex<CsvStatOutputState>>,
}

impl CsvStatOutput {
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        let writer = if enabled {
            let state = CsvStatOutputState {
                writer: csv::Writer::from_writer(io::stdout()),
                initialized: false,
            };

            Some(Mutex::new(state))
        } else {
            None
        };

        Self { writer }
    }

    pub fn use_header(&self, header: &[&str]) -> anyhow::Result<()> {
        if let Some(writer) = &self.writer {
            let mut state = writer.lock().unwrap();

            if !state.initialized {
                state.writer.write_record(header)?;
                state.initialized = true;
            }
        }

        anyhow::Ok(())
    }

    pub fn add_record<I, T>(&self, values: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = T>,
        T: AsRef<[u8]>,
    {
        if let Some(writer) = &self.writer {
            let mut state = writer.lock().unwrap();

            state.writer.write_record(values)?;
        }

        anyhow::Ok(())
    }

    pub fn flush(&self) -> anyhow::Result<()> {
        if let Some(writer) = &self.writer {
            writer.lock().unwrap().writer.flush()?;
        }

        anyhow::Ok(())
    }
}

impl Default for CsvStatOutput {
    fn default() -> Self {
        Self::new(false)
    }
}
