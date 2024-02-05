use std::sync::mpsc::Receiver;

use geese::{GeeseSystem, GeeseContextHandle};
use log::{debug, error, info};
use notify::{Watcher, RecommendedWatcher};

pub mod events {
    pub struct FilesChanged {
        pub paths: Vec<std::path::PathBuf>
    }
    impl FilesChanged {
        pub fn from_event(event: &notify::Event) -> Self {
            Self {
                paths: event.paths.clone()
            }
        }
    }
}

pub struct FileWatcher {
    ctx: GeeseContextHandle<Self>,
    filewatcher: RecommendedWatcher,
    rx: Receiver<notify::Result<notify::Event>>
}
impl FileWatcher {
    pub fn watch<P: AsRef<std::path::Path>>(&mut self, path: P, recursive: bool) {
        let rec = match recursive {
            true => notify::RecursiveMode::Recursive,
            false => notify::RecursiveMode::NonRecursive
        };
        self.filewatcher.watch(path.as_ref(), rec).expect(format!("Cannot watch: {:?}", path.as_ref().display()).as_str());
        info!("Watching {}", path.as_ref().display());
    }

    pub fn poll(&self) {
        if let Ok(event) = self.rx.try_recv() {
            match event {
                Ok(event) => if let notify::EventKind::Modify(kind) = event.kind {
                    self.ctx.raise_event(events::FilesChanged::from_event(&event));
                },
                Err(e) => error!("Watch error: {:?}", e),
            }
        }
    }
}
impl GeeseSystem for FileWatcher {
    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let filewatcher = notify::recommended_watcher(tx).unwrap();
        Self {
            ctx,
            filewatcher,
            rx
        }
    }
}