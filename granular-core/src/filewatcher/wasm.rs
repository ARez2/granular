use crate::utils::*;

pub mod events {}

pub struct FileWatcher {}
impl FileWatcher {
    #[allow(unused)]
    pub fn watch<P: AsRef<std::path::Path>>(&mut self, _path: P, _recursive: bool) {}
}
impl GeeseSystem for FileWatcher {
    fn new(_ctx: geese::GeeseContextHandle<Self>) -> Self {
        Self {}
    }
}
