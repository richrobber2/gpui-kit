//! The UWP host owns the UI thread and pumps this queue before each frame.
use gpui_kit::{PlatformDispatcher, Priority, RunnableVariant};
use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    sync::{Arc, mpsc},
    thread::{self, ThreadId},
    time::{Duration, Instant},
};

type Job = Box<dyn FnOnce() + Send>;
pub struct Dispatcher {
    main_thread: ThreadId,
    main: Mutex<VecDeque<RunnableVariant>>,
    delayed: Mutex<Vec<(Instant, RunnableVariant)>>,
    background: mpsc::Sender<Job>,
}
impl Dispatcher {
    pub fn new() -> Arc<Self> {
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        // A small, fixed pool keeps the UI within the console app budget.
        for index in 0..2 {
            let receiver = receiver.clone();
            thread::Builder::new()
                .name(format!("gpui-xbox-{index}"))
                .spawn(move || {
                    loop {
                        let job = receiver.lock().recv();
                        match job {
                            Ok(job) => job(),
                            Err(_) => break,
                        }
                    }
                })
                .expect("start GPUI worker");
        }
        Arc::new(Self {
            main_thread: thread::current().id(),
            main: Mutex::new(VecDeque::new()),
            delayed: Mutex::new(Vec::new()),
            background: sender,
        })
    }
    pub fn tick(&self) {
        assert!(self.is_main_thread());
        let ready = {
            let mut delayed = self.delayed.lock();
            let mut ready = Vec::new();
            let now = Instant::now();
            let mut index = 0;
            while index < delayed.len() {
                if delayed[index].0 <= now {
                    ready.push(delayed.swap_remove(index).1);
                } else {
                    index += 1;
                }
            }
            ready
        };
        for task in ready {
            self.dispatch(task, Priority::default());
        }
        // Bound each pump so self-rescheduling work cannot starve input/presentation.
        for _ in 0..256 {
            let task = self.main.lock().pop_front();
            match task {
                Some(task) => {
                    task.run();
                }
                None => break,
            }
        }
    }
}
impl PlatformDispatcher for Dispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.main_thread
    }
    fn dispatch(&self, runnable: RunnableVariant, _: Priority) {
        let _ = self.background.send(Box::new(move || {
            runnable.run();
        }));
    }
    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, _: Priority) {
        self.main.lock().push_back(runnable);
    }
    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        self.delayed
            .lock()
            .push((Instant::now() + duration, runnable));
    }
    fn spawn_realtime(&self, job: Job) {
        thread::spawn(job);
    }
}
