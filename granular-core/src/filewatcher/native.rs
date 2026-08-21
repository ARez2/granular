use crate::utils::*;
use notify::{RecommendedWatcher, Watcher};
use std::sync::mpsc::Receiver;

pub mod events {
    pub struct FilesChanged {
        pub paths: Vec<std::path::PathBuf>,
    }
    impl FilesChanged {
        pub fn from_event(event: &notify::Event) -> Self {
            Self {
                paths: event.paths.clone(),
            }
        }
    }
}

pub struct FileWatcher {
    ctx: GeeseContextHandle<Self>,
    filewatcher: RecommendedWatcher,
    rx: Receiver<notify::Result<notify::Event>>,
}
impl FileWatcher {
    /// Registers a new path to be watched. FileWatcher will emit an `event::FilesChanged` when the resource at this path changes.
    pub fn watch<P: AsRef<std::path::Path>>(&mut self, path: P, recursive: bool) {
        let rec = match recursive {
            true => notify::RecursiveMode::Recursive,
            false => notify::RecursiveMode::NonRecursive,
        };
        self.filewatcher
            .watch(path.as_ref(), rec)
            .unwrap_or_else(|_| warn!("Cannot watch: {:?}", path.as_ref().display()));
        info!("Watching {}", path.as_ref().display());
    }

    fn poll(&mut self, _event: &crate::events::timing::Tick<30>) {
        if let Ok(event) = self.rx.try_recv() {
            match event {
                Ok(event) => {
                    if let notify::EventKind::Modify(_kind) = event.kind {
                        self.ctx
                            .raise_event(events::FilesChanged::from_event(&event));
                    }
                }
                Err(e) => error!("Watch error: {:?}", e),
            }
        }
    }
}
impl GeeseSystem for FileWatcher {
    const EVENT_HANDLERS: geese::EventHandlers<Self> = event_handlers().with(Self::poll);

    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let filewatcher = notify::recommended_watcher(tx).unwrap();
        Self {
            ctx,
            filewatcher,
            rx,
        }
    }
}
