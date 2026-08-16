use crate::utils::*;

pub mod events {
    pub struct FilesChanged {
        pub paths: Vec<std::path::PathBuf>,
    }
}

pub struct FileWatcher {
    ctx: GeeseContextHandle<Self>,
}
impl FileWatcher {
    pub fn watch<P: AsRef<std::path::Path>>(&mut self, path: P, recursive: bool) {}
}
impl GeeseSystem for FileWatcher {
    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        Self { ctx }
    }
}
