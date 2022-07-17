use std::error::Error;
use std::mem;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Debug)]
struct ErrorReceiver<E> {
    error: Arc<Mutex<Option<E>>>,
}

impl<E> Clone for ErrorReceiver<E> {
    fn clone(&self) -> Self {
        Self {
            error: self.error.clone(),
        }
    }
}

impl<E: Default> ErrorReceiver<E> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            error: Arc::new(Mutex::new(None)),
        }
    }

    pub fn handle_result<T>(&self, result: Result<T, E>) {
        if let Err(error) = result {
            self.set_error(error);
        }
    }

    fn set_error(&self, error: E) {
        let mut guard = self.error.lock().expect("Could not acquire error lock");
        *guard = Some(error);
    }

    pub fn status(&self) -> Result<(), E> {
        let mut guard = self.error.lock().expect("Could not acquire error lock");

        if let Some(error) = &mut *guard {
            let error = mem::take(error);
            return Err(error);
        }

        Ok(())
    }
}

pub type ThreadPoolJobResult<E> = Result<(), E>;

#[derive(Debug)]
pub(in crate::idn) struct ThreadPool<E> {
    inner: Option<Arc<Mutex<threadpool::ThreadPool>>>,
    thread_num: usize,
    parent: bool,
    in_thread: bool,
    child_num: Arc<(Mutex<u8>, Condvar)>,
    error_receiver: ErrorReceiver<E>,
}

impl<E: Error + Default + Send + 'static> ThreadPool<E> {
    #[must_use]
    pub fn new(thread_num: usize, thread_name: &str) -> Self {
        let inner = if thread_num > 0 {
            let pool = threadpool::Builder::new()
                .num_threads(thread_num)
                .thread_name(thread_name.to_owned())
                .build();
            Some(Arc::new(Mutex::new(pool)))
        } else {
            None
        };

        Self {
            inner,
            thread_num,
            parent: true,
            in_thread: false,
            child_num: Arc::new((Mutex::new(0), Condvar::new())),
            error_receiver: ErrorReceiver::new(),
        }
    }

    #[must_use]
    pub fn make_child(&self) -> Self {
        let in_thread = if self.thread_num > 0 {
            let (lock, _) = &*self.child_num;
            let mut child_num = lock
                .lock()
                .expect("Could not acquire thread pool child lock");
            *child_num += 1;

            true
        } else {
            false
        };

        let thread_num = self.thread_num.saturating_sub(1);
        let inner = if thread_num > 0 {
            self.inner.clone()
        } else {
            None
        };

        Self {
            inner,
            thread_num,
            parent: false,
            in_thread,
            child_num: self.child_num.clone(),
            error_receiver: self.error_receiver.clone(),
        }
    }

    #[must_use]
    pub fn is_foreground(&self) -> bool {
        self.thread_num == 0
    }

    pub fn execute<'a, F>(&'a self, job: F) -> ThreadPoolJobResult<E>
    where
        F: FnOnce() -> ThreadPoolJobResult<E> + Send + 'a,
    {
        self.error_receiver.status()?;

        if let Some(pool) = &self.inner {
            let inner_guard = pool.lock().expect("Could not acquire thread pool lock");

            let inner_job: Box<dyn FnOnce() -> ThreadPoolJobResult<E> + Send + 'a> = Box::new(job);
            let inner_job: Box<dyn FnOnce() -> ThreadPoolJobResult<E> + Send + 'static> =
                unsafe { mem::transmute(inner_job) };
            let error_receiver = self.error_receiver.clone();
            let job = move || {
                error_receiver.handle_result(inner_job());
            };
            inner_guard.execute(job);
        } else {
            self.error_receiver.handle_result(job());
            self.error_receiver.status()?;
        }

        Ok(())
    }

    pub fn get_status(&self) -> Result<(), E> {
        let result = self.error_receiver.status();
        if let Err(error) = result {
            self.inner_join();
            return Err(error);
        }

        Ok(())
    }

    pub fn join(&self) -> Result<(), E> {
        self.inner_join();
        self.error_receiver.status()?;

        Ok(())
    }

    fn inner_join(&self) {
        if !self.parent {
            panic!("Can do join() only on parent ThreadPool");
        }

        let (lock, cvar) = &*self.child_num;
        let mut child_num = lock
            .lock()
            .expect("Could not acquire thread pool child lock");
        while *child_num > 0 {
            child_num = cvar
                .wait(child_num)
                .expect("Could not acquire thread pool child lock");
        }

        if let Some(pool) = &self.inner {
            let inner_guard = pool.lock().expect("Could not acquire thread pool lock");
            inner_guard.join();
        } else {
            // nothing can be running in the background
        }
    }
}

impl<E> Drop for ThreadPool<E> {
    fn drop(&mut self) {
        if self.in_thread {
            let (lock, cvar) = &*self.child_num;
            let mut child_num = lock
                .lock()
                .expect("Could not acquire thread pool child lock");
            *child_num -= 1;
            cvar.notify_all();
        }

        if !self.parent {
            return;
        }

        if let Some(pool) = &self.inner {
            let inner_guard = pool.lock().expect("Could not acquire thread pool lock");

            if inner_guard.active_count() != 0 || inner_guard.queued_count() != 0 {
                panic!("Cannot drop ThreadPool when any jobs are active");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fmt::{Display, Formatter};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use crate::idn::thread_pool::ThreadPool;

    #[derive(Debug, PartialEq, Eq)]
    struct TestError {
        message: &'static str,
    }

    impl TestError {
        #[must_use]
        pub fn new(message: &'static str) -> Self {
            Self { message }
        }
    }

    impl Display for TestError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl Default for TestError {
        fn default() -> Self {
            Self::new("TEST ERROR")
        }
    }

    impl Error for TestError {}

    #[test]
    fn test_thread_pool_foreground() {
        let pool: ThreadPool<TestError> = ThreadPool::new(0, "test");
        let current_id = thread::current().id();

        let result = Arc::new(Mutex::new(current_id));
        let result_thread = result.clone();
        pool.execute(move || {
            let mut result = result_thread.lock().unwrap();
            *result = thread::current().id();
            Ok(())
        })
        .unwrap();
        pool.join().unwrap();

        assert_eq!(*result.lock().unwrap(), current_id);
    }

    #[test]
    fn test_thread_pool_background() {
        let pool: ThreadPool<TestError> = ThreadPool::new(1, "test");
        let current_id = thread::current().id();

        let result = Arc::new(Mutex::new(current_id));
        let result_thread = result.clone();
        pool.execute(move || {
            let mut result = result_thread.lock().unwrap();
            *result = thread::current().id();
            Ok(())
        })
        .unwrap();
        pool.join().unwrap();

        assert_ne!(*result.lock().unwrap(), current_id);
    }

    #[test]
    fn test_thread_pool_error_on_join() {
        let pool: ThreadPool<TestError> = ThreadPool::new(1, "test");

        pool.execute(move || Err(TestError::new("error in execute")))
            .unwrap();
        let result = pool.join();

        assert_eq!(result.unwrap_err(), TestError::new("error in execute"));
    }

    #[test]
    #[should_panic(expected = "Cannot drop ThreadPool when any jobs are active")]
    fn test_thread_pool_drop_when_active() {
        let pool: ThreadPool<TestError> = ThreadPool::new(1, "test");

        pool.execute(move || {
            thread::sleep(Duration::from_millis(100));
            Ok(())
        })
        .unwrap();

        drop(pool);
    }
}
